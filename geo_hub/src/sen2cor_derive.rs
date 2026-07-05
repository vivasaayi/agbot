//! Local NDVI derivation from Sen2Cor L2A JP2 bands (satellite pipeline
//! batch 24).
//!
//! Batch 13 registered Sen2Cor's L2A output as L1 `band_*` catalog products
//! whose artifacts are JPEG 2000 files — readable now that `raster_io`
//! decodes JP2 (pure-Rust OpenJPEG port). This module closes the local
//! loop: the cataloged 10 m red/NIR band products of a scene decode to DN,
//! calibrate to surface reflectance through the canonical
//! `imagery_processor` sensor profiles (baseline >= 04.00 offset or legacy),
//! and register an `ndvi` L2 GeoTIFF with lineage to both band products.
//!
//! Geo-referencing comes from the granule's `MTD_TL.xml` tile geocoding
//! (EPSG code + per-resolution ULX/ULY/XDIM/YDIM) — Sentinel-2 JP2s do not
//! carry a GeoTIFF-style grid, and the decoder is deliberately a pure pixel
//! reader.
//!
//! **SCL cloud masking (batch 28):** when the scene's registered
//! `band_scl_20m` product (Sen2Cor's own Scene Classification Layer) is
//! present, cloud/shadow/defective pixels are masked before the index —
//! the Sen2Cor parallel of the HLS Fmask path. SCL is 20 m against the
//! 10 m bands, so codes are block-replicated 2x by deterministic
//! nearest-neighbor before masking; `scl_applied` is recorded in the
//! product parameters and the SCL product joins the lineage.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use imagery_processor::pipeline::calibration::{apply_radiometric_scaling, SensorProfile};
use imagery_processor::{IndexBandRole, IndexKind};
use raster_io::{read_jp2_gray, write_geotiff_f32, GeoTiffTags, Jp2Gray};
use serde::{Deserialize, Serialize};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::schemas::{GeoBounds, RasterSpatialRef};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{artifact_file_component, file_checksum};
use crate::satellite_derivation::{compute_masked_index, INDEX_NODATA};
use crate::sen2cor::SEN2COR_SOURCE_ID;

/// How many directory levels above a band file the granule's `MTD_TL.xml`
/// may sit (`IMG_DATA/R10m/band.jp2` -> granule dir is 2..3 levels up).
const METADATA_SEARCH_DEPTH: usize = 4;

/// Sen2Cor SCL codes kept as clear-sky ground for index math: vegetation
/// (4), not-vegetated (5), water (6), unclassified (7 — not positively
/// cloudy), and snow/ice (11 — valid ground, matching the HLS Fmask
/// decision). Rejected: no-data (0), saturated/defective (1), cast/dark
/// shadows (2), cloud shadows (3), cloud medium/high probability (8/9),
/// thin cirrus (10), and any out-of-range code.
pub fn scl_clear(code: u16) -> bool {
    matches!(code, 4 | 5 | 6 | 7 | 11)
}

#[derive(Debug, Error)]
pub enum Sen2CorDeriveError {
    #[error("scene {scene_id} has no registered {kind} product (run `geo_hub sen2cor run` first)")]
    BandNotFound { scene_id: String, kind: String },
    #[error("band product {product_id} has no artifact path")]
    NoArtifact { product_id: String },
    #[error("JP2 band decode failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error("red and NIR bands are not on the same grid ({red_dims:?} vs {nir_dims:?})")]
    GridMismatch {
        red_dims: (u32, u32),
        nir_dims: (u32, u32),
    },
    #[error(
        "SCL band {scl_dims:?} is not the 20 m half-resolution grid of the {band_dims:?} bands"
    )]
    SclGridMismatch {
        scl_dims: (u32, u32),
        band_dims: (u32, u32),
    },
    #[error("no MTD_TL.xml found within {depth} levels above {band_path}")]
    MetadataNotFound { band_path: PathBuf, depth: usize },
    #[error("MTD_TL.xml at {path} is unreadable: {source}")]
    MetadataUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("MTD_TL.xml is missing {what} (resolution {resolution} m)")]
    BadGeocoding { what: &'static str, resolution: u32 },
    #[error("index computation failed: {0}")]
    Index(String),
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Shared(#[from] crate::drought_rasters::DroughtRasterError),
}

impl Sen2CorDeriveError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Sen2CorDeriveError::BandNotFound { .. }
                | Sen2CorDeriveError::NoArtifact { .. }
                | Sen2CorDeriveError::GridMismatch { .. }
                | Sen2CorDeriveError::SclGridMismatch { .. }
                | Sen2CorDeriveError::MetadataNotFound { .. }
                | Sen2CorDeriveError::BadGeocoding { .. }
        )
    }
}

/// One Sen2Cor NDVI derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct Sen2CorNdviRequest {
    /// Scene id of a registered Sen2Cor L2A (the L1 band products' scene).
    pub scene_id: String,
    /// Processing-baseline calibration: `true` (default, baseline >= 04.00)
    /// applies the BOA offset `refl = (DN - 1000)/10000`; `false` uses the
    /// legacy `DN/10000`.
    #[serde(default = "default_true")]
    pub baseline_ge_0400: bool,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct Sen2CorNdviOutcome {
    pub ndvi_product_id: String,
    pub scene_id: String,
    pub valid_pixels: usize,
    pub invalid_pixels: usize,
    pub sensor_profile: String,
    /// Whether Sen2Cor's SCL band masked clouds before the index.
    pub scl_applied: bool,
    pub ndvi_artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

/// First `<TAG>text</TAG>` payload in an XML fragment. Sentinel tile
/// metadata is machine-written with unique simple tags, so a deterministic
/// scan beats a full XML dependency here.
fn xml_tag_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim())
}

/// The `<Geoposition resolution="N">...</Geoposition>` block for one
/// resolution.
fn geoposition_block(xml: &str, resolution: u32) -> Option<&str> {
    let open = format!("<Geoposition resolution=\"{resolution}\">");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find("</Geoposition>")? + start;
    Some(&xml[start..end])
}

/// Parse the tile geocoding for one resolution from `MTD_TL.xml`:
/// EPSG code + GDAL-style geotransform `[ULX, XDIM, 0, ULY, 0, YDIM]`.
pub fn parse_tile_geocoding(
    xml: &str,
    resolution: u32,
) -> Result<(u32, [f64; 6]), Sen2CorDeriveError> {
    let bad = |what: &'static str| Sen2CorDeriveError::BadGeocoding { what, resolution };
    let epsg = xml_tag_text(xml, "HORIZONTAL_CS_CODE")
        .and_then(|code| code.trim().strip_prefix("EPSG:"))
        .and_then(|code| code.parse::<u32>().ok())
        .ok_or_else(|| bad("HORIZONTAL_CS_CODE (EPSG:<code>)"))?;
    let block = geoposition_block(xml, resolution)
        .ok_or_else(|| bad("Geoposition block for the resolution"))?;
    let coord = |tag: &'static str| {
        xml_tag_text(block, tag)
            .and_then(|value| value.parse::<f64>().ok())
            .ok_or(Sen2CorDeriveError::BadGeocoding {
                what: tag,
                resolution,
            })
    };
    let (ulx, uly) = (coord("ULX")?, coord("ULY")?);
    let (xdim, ydim) = (coord("XDIM")?, coord("YDIM")?);
    Ok((epsg, [ulx, xdim, 0.0, uly, 0.0, ydim]))
}

/// Locate the granule's `MTD_TL.xml` by walking up from a band file.
fn find_tile_metadata(band_path: &Path) -> Result<PathBuf, Sen2CorDeriveError> {
    let mut dir = band_path.parent();
    for _ in 0..METADATA_SEARCH_DEPTH {
        let Some(current) = dir else { break };
        let candidate = current.join("MTD_TL.xml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        dir = current.parent();
    }
    Err(Sen2CorDeriveError::MetadataNotFound {
        band_path: band_path.to_path_buf(),
        depth: METADATA_SEARCH_DEPTH,
    })
}

async fn band_product(
    pool: &DbPool,
    scene_id: &str,
    kind: &str,
) -> Result<RegisteredProduct, Sen2CorDeriveError> {
    let mut products = catalog::list_products(
        pool,
        &ProductFilter {
            scene_id: Some(scene_id.to_string()),
            kind: Some(kind.to_string()),
            level: Some(ProductLevel::L1),
            status: Some("registered".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    products
        .pop()
        .ok_or_else(|| Sen2CorDeriveError::BandNotFound {
            scene_id: scene_id.to_string(),
            kind: kind.to_string(),
        })
}

/// Like [`band_product`] but absence is not an error (quality bands are
/// optional).
async fn optional_band_product(
    pool: &DbPool,
    scene_id: &str,
    kind: &str,
) -> Result<Option<RegisteredProduct>, Sen2CorDeriveError> {
    match band_product(pool, scene_id, kind).await {
        Ok(product) => Ok(Some(product)),
        Err(Sen2CorDeriveError::BandNotFound { .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

fn decode_band(product: &RegisteredProduct) -> Result<(Jp2Gray, PathBuf), Sen2CorDeriveError> {
    let path = product
        .path
        .as_deref()
        .ok_or_else(|| Sen2CorDeriveError::NoArtifact {
            product_id: product.product_id.clone(),
        })?;
    let path = PathBuf::from(path);
    Ok((read_jp2_gray(&path)?, path))
}

/// Derive NDVI from a registered Sen2Cor scene's 10 m red/NIR JP2 bands and
/// register it as an `ndvi` L2 GeoTIFF with lineage to both band products.
/// Idempotent: identical inputs re-register the same content-addressed id.
pub async fn derive_sen2cor_ndvi(
    pool: &DbPool,
    data_root: &Path,
    request: &Sen2CorNdviRequest,
) -> Result<Sen2CorNdviOutcome, Sen2CorDeriveError> {
    let red_product = band_product(pool, &request.scene_id, "band_b04_10m").await?;
    let nir_product = band_product(pool, &request.scene_id, "band_b08_10m").await?;
    let (red, red_path) = decode_band(&red_product)?;
    let (nir, _) = decode_band(&nir_product)?;
    if (red.width, red.height) != (nir.width, nir.height) {
        return Err(Sen2CorDeriveError::GridMismatch {
            red_dims: (red.width, red.height),
            nir_dims: (nir.width, nir.height),
        });
    }

    // Grid from the granule tile metadata (10 m resolution for B04/B08).
    let metadata_path = find_tile_metadata(&red_path)?;
    let xml = std::fs::read_to_string(&metadata_path).map_err(|source| {
        Sen2CorDeriveError::MetadataUnreadable {
            path: metadata_path.clone(),
            source,
        }
    })?;
    let (epsg, transform) = parse_tile_geocoding(&xml, 10)?;

    // DN -> surface reflectance through the canonical sensor profiles
    // (fill DN 0 is reason-coded, reflectance clamped to [0, 1]).
    let profile = if request.baseline_ge_0400 {
        SensorProfile::Sentinel2L2ABaseline0400
    } else {
        SensorProfile::Sentinel2L2ALegacy
    };
    let profile_label = if request.baseline_ge_0400 {
        "sentinel2_l2a_baseline_0400"
    } else {
        "sentinel2_l2a_legacy"
    };
    let scaled_red = apply_radiometric_scaling(profile, &red.values);
    let scaled_nir = apply_radiometric_scaling(profile, &nir.values);
    let bands = BTreeMap::from([
        (IndexBandRole::Red, scaled_red.pixels),
        (IndexBandRole::Nir, scaled_nir.pixels),
    ]);

    // Clear-sky mask from Sen2Cor's own SCL band when the scene has one
    // (20 m codes block-replicated to the 10 m grid); otherwise keep all.
    let scl_product = optional_band_product(pool, &request.scene_id, "band_scl_20m").await?;
    let (clear, scl_applied, scl_input) = match &scl_product {
        Some(product) => {
            let (scl, _) = decode_band(product)?;
            if (scl.width * 2, scl.height * 2) != (red.width, red.height) {
                return Err(Sen2CorDeriveError::SclGridMismatch {
                    scl_dims: (scl.width, scl.height),
                    band_dims: (red.width, red.height),
                });
            }
            let codes = crate::satellite_derivation::resample_nearest(
                &scl.values,
                scl.width,
                scl.height,
                red.width,
                red.height,
            );
            (
                codes.iter().map(|code| scl_clear(*code)).collect(),
                true,
                Some(ProductInputRef {
                    product_id: product.product_id.clone(),
                    role: "scl_mask".to_string(),
                }),
            )
        }
        None => (vec![true; red.values.len()], false, None),
    };
    let index = compute_masked_index(IndexKind::Ndvi, &bands, &clear)
        .map_err(|err| Sen2CorDeriveError::Index(err.to_string()))?;

    // Spatial reference mirrors raster_io's GeoTIFF reader construction so
    // the registered metadata matches what consumers re-read from disk.
    let (width_px, height_px) = (f64::from(red.width), f64::from(red.height));
    let (min_x, max_x) = (transform[0], transform[0] + width_px * transform[1]);
    let (max_y, min_y) = (transform[3], transform[3] + height_px * transform[5]);
    let spatial_ref = RasterSpatialRef {
        georeferenced: true,
        crs: Some(format!("EPSG:{epsg}")),
        bbox: Some(GeoBounds {
            min_lon: min_x.min(max_x),
            min_lat: min_y.min(max_y),
            max_lon: min_x.max(max_x),
            max_lat: min_y.max(max_y),
        }),
        geo_transform: Some(transform),
        resolution: None,
    };

    let mut draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "sen2cor.ndvi".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "scene_id": request.scene_id,
            "index": "ndvi",
            "red_band": "B04_10m",
            "nir_band": "B08_10m",
            "sensor_profile": profile_label,
            "scl_applied": scl_applied,
        }),
        inputs: vec![
            ProductInputRef {
                product_id: red_product.product_id.clone(),
                role: "red".to_string(),
            },
            ProductInputRef {
                product_id: nir_product.product_id.clone(),
                role: "nir".to_string(),
            },
        ]
        .into_iter()
        .chain(scl_input)
        .collect(),
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: Some(request.scene_id.clone()),
            temporal_start: red_product
                .temporal_start
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
            temporal_end: red_product
                .temporal_end
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        },
        spatial_ref: Some(spatial_ref),
        gsd_m_per_px: Some(transform[1].abs()),
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(SEN2COR_SOURCE_ID.to_string()),
    };

    let ndvi_dir = data_root.join("derived").join("sen2cor_ndvi");
    std::fs::create_dir_all(&ndvi_dir).map_err(|source| Sen2CorDeriveError::Store {
        what: "sen2cor ndvi directory",
        source,
    })?;
    let ndvi_path = ndvi_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    write_geotiff_f32(
        &ndvi_path,
        red.width,
        red.height,
        &index.values,
        &GeoTiffTags {
            epsg: Some(epsg),
            geo_transform: Some(transform),
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let checksum = file_checksum(&ndvi_path, "sen2cor ndvi readback")?;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: ndvi_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    draft.quality_summary = Some(serde_json::json!({
        "valid_pixels": index.valid_pixels,
        "invalid_pixels": index.invalid_pixels,
        "reasons": index.reason_counts,
        "red_fill_pixels": scaled_red.fill_pixel_count,
        "nir_fill_pixels": scaled_nir.fill_pixel_count,
    }));

    let actor = provenance::ActorIdentity::system("geo_hub:sen2cor_derive");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let ndvi_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(Sen2CorNdviOutcome {
        stac_item_href: format!("/api/stac/collections/ndvi/items/{ndvi_product_id}"),
        tiles_href: format!("/api/catalog/products/{ndvi_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"),
        ndvi_product_id,
        scene_id: request.scene_id.clone(),
        valid_pixels: index.valid_pixels,
        invalid_pixels: index.invalid_pixels,
        sensor_profile: profile_label.to_string(),
        scl_applied,
        ndvi_artifact: ndvi_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<n1:Level-2A_Tile_ID>
  <n1:Geometric_Info>
    <Tile_Geocoding metadataLevel="Brief">
      <HORIZONTAL_CS_NAME>WGS84 / UTM zone 43N</HORIZONTAL_CS_NAME>
      <HORIZONTAL_CS_CODE>EPSG:32643</HORIZONTAL_CS_CODE>
      <Size resolution="10"><NROWS>10980</NROWS><NCOLS>10980</NCOLS></Size>
      <Size resolution="20"><NROWS>5490</NROWS><NCOLS>5490</NCOLS></Size>
      <Geoposition resolution="10">
        <ULX>600000</ULX><ULY>1300020</ULY><XDIM>10</XDIM><YDIM>-10</YDIM>
      </Geoposition>
      <Geoposition resolution="20">
        <ULX>600000</ULX><ULY>1300020</ULY><XDIM>20</XDIM><YDIM>-20</YDIM>
      </Geoposition>
    </Tile_Geocoding>
  </n1:Geometric_Info>
</n1:Level-2A_Tile_ID>"#;

    #[test]
    fn tile_geocoding_parses_per_resolution() {
        let (epsg, transform) = parse_tile_geocoding(TILE_XML, 10).unwrap();
        assert_eq!(epsg, 32643);
        assert_eq!(transform, [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0]);
        let (_, transform20) = parse_tile_geocoding(TILE_XML, 20).unwrap();
        assert_eq!(transform20[1], 20.0);
        assert_eq!(transform20[5], -20.0);
    }

    #[test]
    fn missing_geocoding_pieces_are_typed_errors() {
        assert!(matches!(
            parse_tile_geocoding(TILE_XML, 60),
            Err(Sen2CorDeriveError::BadGeocoding {
                what: "Geoposition block for the resolution",
                resolution: 60,
            })
        ));
        assert!(matches!(
            parse_tile_geocoding("<Tile_Geocoding/>", 10),
            Err(Sen2CorDeriveError::BadGeocoding {
                what: "HORIZONTAL_CS_CODE (EPSG:<code>)",
                ..
            })
        ));
        let no_ulx = TILE_XML.replace("<ULX>600000</ULX>", "");
        assert!(matches!(
            parse_tile_geocoding(&no_ulx, 10),
            Err(Sen2CorDeriveError::BadGeocoding { what: "ULX", .. })
        ));
    }

    #[test]
    fn scl_clear_codes_are_pinned() {
        // 0 nodata, 1 saturated, 2 cast shadow, 3 cloud shadow, 8/9 cloud,
        // 10 cirrus -> rejected; 4 vegetation, 5 bare, 6 water,
        // 7 unclassified, 11 snow -> kept; out-of-range rejected.
        for rejected in [0u16, 1, 2, 3, 8, 9, 10, 12, 255] {
            assert!(!scl_clear(rejected), "{rejected}");
        }
        for kept in [4u16, 5, 6, 7, 11] {
            assert!(scl_clear(kept), "{kept}");
        }
    }

    #[test]
    fn tile_metadata_is_found_above_band_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let granule = tmp.path().join("GRANULE").join("L2A_T43PFN_A1");
        let img = granule.join("IMG_DATA").join("R10m");
        std::fs::create_dir_all(&img).unwrap();
        std::fs::write(granule.join("MTD_TL.xml"), TILE_XML).unwrap();
        let band = img.join("T43PFN_20240601T051651_B04_10m.jp2");
        std::fs::write(&band, b"x").unwrap();
        assert_eq!(
            find_tile_metadata(&band).unwrap(),
            granule.join("MTD_TL.xml")
        );
        // A band with no metadata anywhere above it is a typed error.
        let stray = tmp.path().join("stray.jp2");
        std::fs::write(&stray, b"x").unwrap();
        assert!(matches!(
            find_tile_metadata(&stray),
            Err(Sen2CorDeriveError::MetadataNotFound { .. })
        ));
    }
}
