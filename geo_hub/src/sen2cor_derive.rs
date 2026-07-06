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
    #[error(
        "index {0:?} is not derivable from Sen2Cor bands (supported: ndvi, ndwi, mndwi, ndmi, nbr)"
    )]
    UnsupportedIndex(String),
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
            Sen2CorDeriveError::UnsupportedIndex(_)
                | Sen2CorDeriveError::BandNotFound { .. }
                | Sen2CorDeriveError::NoArtifact { .. }
                | Sen2CorDeriveError::GridMismatch { .. }
                | Sen2CorDeriveError::SclGridMismatch { .. }
                | Sen2CorDeriveError::MetadataNotFound { .. }
                | Sen2CorDeriveError::BadGeocoding { .. }
        )
    }
}

/// One Sen2Cor index derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct Sen2CorIndexRequest {
    /// Scene id of a registered Sen2Cor L2A (the L1 band products' scene).
    pub scene_id: String,
    /// Index to derive: `ndvi` (default), `mndwi`, or `ndmi`.
    #[serde(default = "default_index")]
    pub index: String,
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

fn default_index() -> String {
    "ndvi".to_string()
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct Sen2CorIndexOutcome {
    pub index_product_id: String,
    /// Index key (`ndvi` / `mndwi` / `ndmi`) — also the catalog kind.
    pub index: String,
    pub scene_id: String,
    pub valid_pixels: usize,
    pub invalid_pixels: usize,
    pub sensor_profile: String,
    /// Whether Sen2Cor's SCL band masked clouds before the index.
    pub scl_applied: bool,
    pub index_artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

/// One input band of an index spec: catalog kind, index role, and whether
/// its 20 m grid must be block-replicated 2x onto a 10 m output grid.
struct SpecBand {
    role: IndexBandRole,
    kind: &'static str,
    upsample_2x: bool,
}

/// A derivable Sen2Cor index: the two bands it needs and the output grid
/// resolution (the `MTD_TL.xml` geoposition to read).
struct Sen2CorIndexSpec {
    key: &'static str,
    kind: IndexKind,
    bands: [SpecBand; 2],
    /// Output grid resolution in meters (10 or 20).
    resolution: u32,
}

/// Supported indices (batch 29, extended batch 30). MNDWI mixes
/// resolutions: B03 is 10 m, B11 (SWIR1) only exists at 20 m and is
/// block-replicated. NDMI and NBR run natively on the 20 m grid
/// (B8A + B11 / B8A + B12); NBR feeds the dNBR burn-severity path.
/// NDWI (green vs broad NIR) runs fully at 10 m (B03 + B08).
fn index_spec(index: &str) -> Option<Sen2CorIndexSpec> {
    match index.trim().to_ascii_lowercase().as_str() {
        "ndvi" => Some(Sen2CorIndexSpec {
            key: "ndvi",
            kind: IndexKind::Ndvi,
            bands: [
                SpecBand {
                    role: IndexBandRole::Red,
                    kind: "band_b04_10m",
                    upsample_2x: false,
                },
                SpecBand {
                    role: IndexBandRole::Nir,
                    kind: "band_b08_10m",
                    upsample_2x: false,
                },
            ],
            resolution: 10,
        }),
        "mndwi" => Some(Sen2CorIndexSpec {
            key: "mndwi",
            kind: IndexKind::Mndwi,
            bands: [
                SpecBand {
                    role: IndexBandRole::Green,
                    kind: "band_b03_10m",
                    upsample_2x: false,
                },
                SpecBand {
                    role: IndexBandRole::Swir1,
                    kind: "band_b11_20m",
                    upsample_2x: true,
                },
            ],
            resolution: 10,
        }),
        "ndwi" => Some(Sen2CorIndexSpec {
            key: "ndwi",
            kind: IndexKind::Ndwi,
            bands: [
                SpecBand {
                    role: IndexBandRole::Green,
                    kind: "band_b03_10m",
                    upsample_2x: false,
                },
                SpecBand {
                    role: IndexBandRole::Nir,
                    kind: "band_b08_10m",
                    upsample_2x: false,
                },
            ],
            resolution: 10,
        }),
        "nbr" => Some(Sen2CorIndexSpec {
            key: "nbr",
            kind: IndexKind::Nbr,
            bands: [
                SpecBand {
                    role: IndexBandRole::Nir,
                    kind: "band_b8a_20m",
                    upsample_2x: false,
                },
                SpecBand {
                    role: IndexBandRole::Swir2,
                    kind: "band_b12_20m",
                    upsample_2x: false,
                },
            ],
            resolution: 20,
        }),
        "ndmi" => Some(Sen2CorIndexSpec {
            key: "ndmi",
            kind: IndexKind::Ndmi,
            bands: [
                SpecBand {
                    role: IndexBandRole::Nir,
                    kind: "band_b8a_20m",
                    upsample_2x: false,
                },
                SpecBand {
                    role: IndexBandRole::Swir1,
                    kind: "band_b11_20m",
                    upsample_2x: false,
                },
            ],
            resolution: 20,
        }),
        _ => None,
    }
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

/// Derive one spectral index from a registered Sen2Cor scene's JP2 bands
/// and register it as an L2 GeoTIFF (kind = the index key) with lineage to
/// the band products (and the SCL mask when applied). Idempotent: identical
/// inputs re-register the same content-addressed id.
pub async fn derive_sen2cor_index(
    pool: &DbPool,
    data_root: &Path,
    request: &Sen2CorIndexRequest,
) -> Result<Sen2CorIndexOutcome, Sen2CorDeriveError> {
    let spec = index_spec(&request.index)
        .ok_or_else(|| Sen2CorDeriveError::UnsupportedIndex(request.index.clone()))?;

    // Load both bands; the first spec band is on the output grid by
    // construction (upsampled bands are never listed first), so it defines
    // the grid dimensions.
    let mut products = Vec::new();
    let mut rasters = Vec::new();
    let mut first_path = None;
    for band in &spec.bands {
        let product = band_product(pool, &request.scene_id, band.kind).await?;
        let (raster, path) = decode_band(&product)?;
        if first_path.is_none() {
            first_path = Some(path);
        }
        products.push(product);
        rasters.push(raster);
    }
    let (grid_width, grid_height) = (rasters[0].width, rasters[0].height);
    debug_assert!(
        !spec.bands[0].upsample_2x,
        "first spec band defines the grid"
    );

    // Bring every band onto the output grid: native bands must match it
    // exactly; 20 m bands feeding a 10 m grid block-replicate 2x.
    let mut band_values: Vec<Vec<u16>> = Vec::new();
    for (band, raster) in spec.bands.iter().zip(&rasters) {
        let expected = if band.upsample_2x {
            (grid_width / 2, grid_height / 2)
        } else {
            (grid_width, grid_height)
        };
        if (raster.width, raster.height) != expected {
            return Err(Sen2CorDeriveError::GridMismatch {
                red_dims: (grid_width, grid_height),
                nir_dims: (raster.width, raster.height),
            });
        }
        band_values.push(if band.upsample_2x {
            crate::satellite_derivation::resample_nearest(
                &raster.values,
                raster.width,
                raster.height,
                grid_width,
                grid_height,
            )
        } else {
            raster.values.clone()
        });
    }

    // Grid from the granule tile metadata at the output resolution.
    let metadata_path = find_tile_metadata(first_path.as_deref().expect("first band path"))?;
    let xml = std::fs::read_to_string(&metadata_path).map_err(|source| {
        Sen2CorDeriveError::MetadataUnreadable {
            path: metadata_path.clone(),
            source,
        }
    })?;
    let (epsg, transform) = parse_tile_geocoding(&xml, spec.resolution)?;

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
    let mut bands = BTreeMap::new();
    let mut fill_counts = serde_json::Map::new();
    for (band, values) in spec.bands.iter().zip(&band_values) {
        let scaled = apply_radiometric_scaling(profile, values);
        fill_counts.insert(
            format!("{}_fill_pixels", band.kind),
            serde_json::json!(scaled.fill_pixel_count),
        );
        bands.insert(band.role, scaled.pixels);
    }

    // Clear-sky mask from Sen2Cor's own SCL band when the scene has one
    // (native on a 20 m grid, block-replicated 2x onto a 10 m grid);
    // otherwise keep all.
    let scl_product = optional_band_product(pool, &request.scene_id, "band_scl_20m").await?;
    let (clear, scl_applied, scl_input) = match &scl_product {
        Some(product) => {
            let (scl, _) = decode_band(product)?;
            let codes = if (scl.width, scl.height) == (grid_width, grid_height) {
                scl.values
            } else if (scl.width * 2, scl.height * 2) == (grid_width, grid_height) {
                crate::satellite_derivation::resample_nearest(
                    &scl.values,
                    scl.width,
                    scl.height,
                    grid_width,
                    grid_height,
                )
            } else {
                return Err(Sen2CorDeriveError::SclGridMismatch {
                    scl_dims: (scl.width, scl.height),
                    band_dims: (grid_width, grid_height),
                });
            };
            (
                codes.iter().map(|code| scl_clear(*code)).collect(),
                true,
                Some(ProductInputRef {
                    product_id: product.product_id.clone(),
                    role: "scl_mask".to_string(),
                }),
            )
        }
        None => (
            vec![true; grid_width as usize * grid_height as usize],
            false,
            None,
        ),
    };
    let index = compute_masked_index(spec.kind, &bands, &clear)
        .map_err(|err| Sen2CorDeriveError::Index(err.to_string()))?;

    // Spatial reference mirrors raster_io's GeoTIFF reader construction so
    // the registered metadata matches what consumers re-read from disk.
    let (width_px, height_px) = (f64::from(grid_width), f64::from(grid_height));
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
        kind: spec.key.to_string(),
        algorithm_id: format!("sen2cor.{}", spec.key),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "scene_id": request.scene_id,
            "index": spec.key,
            "bands": spec.bands.iter().map(|b| b.kind).collect::<Vec<_>>(),
            "resolution_m": spec.resolution,
            "sensor_profile": profile_label,
            "scl_applied": scl_applied,
        }),
        inputs: spec
            .bands
            .iter()
            .zip(&products)
            .map(|(band, product)| ProductInputRef {
                product_id: product.product_id.clone(),
                role: band.role.key().to_string(),
            })
            .chain(scl_input)
            .collect(),
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: Some(request.scene_id.clone()),
            temporal_start: products[0]
                .temporal_start
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
            temporal_end: products[0]
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

    let index_dir = data_root.join("derived").join("sen2cor_index");
    std::fs::create_dir_all(&index_dir).map_err(|source| Sen2CorDeriveError::Store {
        what: "sen2cor index directory",
        source,
    })?;
    let index_path = index_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    write_geotiff_f32(
        &index_path,
        grid_width,
        grid_height,
        &index.values,
        &GeoTiffTags {
            epsg: Some(epsg),
            geo_transform: Some(transform),
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let checksum = file_checksum(&index_path, "sen2cor index readback")?;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: index_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    let mut quality = serde_json::json!({
        "valid_pixels": index.valid_pixels,
        "invalid_pixels": index.invalid_pixels,
        "reasons": index.reason_counts,
    });
    quality
        .as_object_mut()
        .expect("quality is an object")
        .extend(fill_counts);
    draft.quality_summary = Some(quality);

    let actor = provenance::ActorIdentity::system("geo_hub:sen2cor_derive");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let index_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(Sen2CorIndexOutcome {
        stac_item_href: format!(
            "/api/stac/collections/{}/items/{index_product_id}",
            spec.key
        ),
        tiles_href: format!("/api/catalog/products/{index_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"),
        index_product_id,
        index: spec.key.to_string(),
        scene_id: request.scene_id.clone(),
        valid_pixels: index.valid_pixels,
        invalid_pixels: index.invalid_pixels,
        sensor_profile: profile_label.to_string(),
        scl_applied,
        index_artifact: index_path,
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
