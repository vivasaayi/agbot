//! Local index/LST derivation from registered Landsat Collection-2 bands
//! (satellite pipeline batch 35) — the Landsat parallel of
//! `sen2cor_derive`.
//!
//! The USGS ingest (Track A phase 5b) registers downloaded band GeoTIFFs as
//! L1 `band_*` products with free-form band tokens, so bands are located by
//! candidate kind lists (mirroring the STAC `asset_candidates` aliases).
//! Calibration uses the canonical Collection-2 Level-2 profiles: surface
//! reflectance `DN * 0.0000275 - 0.2` for NDVI, surface temperature
//! `DN * 0.00341802 + 149` Kelvin for LST. When the scene's QA_PIXEL band
//! is registered, cloud/shadow/cirrus/fill pixels are masked first
//! (`qa_applied` recorded, QA product in the lineage) — the Landsat
//! parallel of SCL/Fmask masking.
//!
//! The registered `lst` L2s feed the drought TCI path
//! (`drought_kind_for("lst")`), and the `ndvi` L2s join the same
//! phenology/climatology/composite series as every other source.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use imagery_processor::pipeline::calibration::{apply_radiometric_scaling, SensorProfile};
use imagery_processor::{IndexBandRole, IndexKind, IndexPixelValue};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, DroughtRasterError,
    LoadedRaster,
};
use crate::satellite_derivation::{compute_masked_index, INDEX_NODATA};

/// Candidate catalog kinds for each band role (USGS band tokens vary by
/// download path; these mirror the STAC asset aliases).
pub const RED_BAND_KINDS: &[&str] = &["band_sr_b4", "band_b4", "band_red"];
pub const NIR_BAND_KINDS: &[&str] = &["band_sr_b5", "band_b5", "band_nir08", "band_nir"];
pub const ST_BAND_KINDS: &[&str] = &["band_st_b10", "band_b10", "band_lwir11"];
pub const QA_BAND_KINDS: &[&str] = &["band_qa_pixel", "band_qa"];

/// QA_PIXEL bits rejected for a clear pixel: 0 fill, 1 dilated cloud,
/// 2 cirrus, 3 cloud, 4 cloud shadow. Snow (5), clear (6), and water (7)
/// are valid ground.
pub const QA_PIXEL_REJECT_BITS: u16 = 0b0001_1111;

/// `true` when a Collection-2 QA_PIXEL code marks usable ground.
pub fn qa_pixel_clear(code: u16) -> bool {
    code & QA_PIXEL_REJECT_BITS == 0
}

#[derive(Debug, Error)]
pub enum LandsatDeriveError {
    #[error("product {0:?} is not derivable from Landsat C2 bands (supported: ndvi, lst)")]
    UnsupportedProduct(String),
    #[error("scene {scene_id} has no registered band for {role} (looked for {kinds:?})")]
    BandNotFound {
        scene_id: String,
        role: &'static str,
        kinds: &'static [&'static str],
    },
    #[error("bands are not on the same grid ({reference:?} vs {other:?})")]
    GridMismatch {
        reference: (u32, u32),
        other: (u32, u32),
    },
    #[error("index computation failed: {0}")]
    Index(String),
    #[error("raster I/O failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error(transparent)]
    Shared(#[from] DroughtRasterError),
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] CatalogError),
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl LandsatDeriveError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            LandsatDeriveError::UnsupportedProduct(_)
                | LandsatDeriveError::BandNotFound { .. }
                | LandsatDeriveError::GridMismatch { .. }
        )
    }
}

/// One Landsat local derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct LandsatDeriveRequest {
    /// Scene id of a registered USGS Landsat scene (the L1 bands' scene).
    pub scene_id: String,
    /// `ndvi` (default) or `lst`.
    #[serde(default = "default_product")]
    pub product: String,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
}

fn default_product() -> String {
    "ndvi".to_string()
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct LandsatDeriveOutcome {
    pub product_id: String,
    /// `ndvi` or `lst` — also the catalog kind.
    pub product: String,
    pub scene_id: String,
    pub valid_pixels: usize,
    pub invalid_pixels: usize,
    /// Whether the scene's QA_PIXEL band masked clouds first.
    pub qa_applied: bool,
    pub artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

/// Find the scene's registered L1 band product for a role, trying each
/// candidate kind in order.
async fn find_band(
    pool: &DbPool,
    scene_id: &str,
    role: &'static str,
    kinds: &'static [&'static str],
) -> Result<Option<RegisteredProduct>, LandsatDeriveError> {
    for kind in kinds {
        let mut products = catalog::list_products(
            pool,
            &ProductFilter {
                scene_id: Some(scene_id.to_string()),
                kind: Some((*kind).to_string()),
                level: Some(ProductLevel::L1),
                status: Some("registered".to_string()),
                ..ProductFilter::default()
            },
        )
        .await?;
        if let Some(product) = products.pop() {
            return Ok(Some(product));
        }
    }
    let _ = role;
    Ok(None)
}

async fn require_band(
    pool: &DbPool,
    scene_id: &str,
    role: &'static str,
    kinds: &'static [&'static str],
) -> Result<RegisteredProduct, LandsatDeriveError> {
    find_band(pool, scene_id, role, kinds)
        .await?
        .ok_or(LandsatDeriveError::BandNotFound {
            scene_id: scene_id.to_string(),
            role,
            kinds,
        })
}

fn load_band(product: &RegisteredProduct) -> Result<LoadedRaster, LandsatDeriveError> {
    Ok(load_raster(Path::new(geotiff_artifact_path(product)?))?)
}

/// Raster f32 values back to the u16 DN the C2 calibration expects
/// (masked/non-finite/fill map to DN 0, the profile's fill value).
fn band_to_dn(raster: &LoadedRaster) -> Vec<u16> {
    raster
        .values
        .iter()
        .zip(&raster.valid_mask)
        .map(|(value, valid)| {
            if *valid && value.is_finite() && (0.0..=65535.0).contains(value) {
                value.round() as u16
            } else {
                0
            }
        })
        .collect()
}

fn same_grid(a: &LoadedRaster, b: &LoadedRaster) -> bool {
    a.epsg == b.epsg
        && a.geo_transform == b.geo_transform
        && (a.width, a.height) == (b.width, b.height)
}

/// Derive NDVI (surface reflectance) or LST (surface temperature, Kelvin)
/// locally from a registered Landsat C2 scene's band products, QA-masked
/// when the scene has a QA_PIXEL band. Idempotent (content-addressed ids).
pub async fn derive_landsat_product(
    pool: &DbPool,
    data_root: &Path,
    request: &LandsatDeriveRequest,
) -> Result<LandsatDeriveOutcome, LandsatDeriveError> {
    let product_key = request.product.trim().to_ascii_lowercase();
    if product_key != "ndvi" && product_key != "lst" {
        return Err(LandsatDeriveError::UnsupportedProduct(
            request.product.clone(),
        ));
    }

    // Load the role bands; the first defines the reference grid.
    let band_products: Vec<(IndexBandRole, RegisteredProduct)> = if product_key == "ndvi" {
        vec![
            (
                IndexBandRole::Red,
                require_band(pool, &request.scene_id, "red", RED_BAND_KINDS).await?,
            ),
            (
                IndexBandRole::Nir,
                require_band(pool, &request.scene_id, "nir", NIR_BAND_KINDS).await?,
            ),
        ]
    } else {
        vec![(
            IndexBandRole::Nir, // role is unused for LST; any slot works
            require_band(
                pool,
                &request.scene_id,
                "surface_temperature",
                ST_BAND_KINDS,
            )
            .await?,
        )]
    };
    let mut rasters = Vec::new();
    for (_, product) in &band_products {
        let raster = load_band(product)?;
        if let Some(reference) = rasters.first() {
            let reference: &LoadedRaster = reference;
            if !same_grid(reference, &raster) {
                return Err(LandsatDeriveError::GridMismatch {
                    reference: (reference.width, reference.height),
                    other: (raster.width, raster.height),
                });
            }
        }
        rasters.push(raster);
    }
    let reference = &rasters[0];
    let pixel_count = reference.width as usize * reference.height as usize;

    // Clear mask from the scene's QA_PIXEL band when registered.
    let qa_product = find_band(pool, &request.scene_id, "qa_pixel", QA_BAND_KINDS).await?;
    let (clear, qa_applied, qa_input) = match &qa_product {
        Some(product) => {
            let qa = load_band(product)?;
            if !same_grid(reference, &qa) {
                return Err(LandsatDeriveError::GridMismatch {
                    reference: (reference.width, reference.height),
                    other: (qa.width, qa.height),
                });
            }
            let codes = band_to_dn(&qa);
            (
                codes.iter().map(|code| qa_pixel_clear(*code)).collect(),
                true,
                Some(ProductInputRef {
                    product_id: product.product_id.clone(),
                    role: "qa_pixel".to_string(),
                }),
            )
        }
        None => (vec![true; pixel_count], false, None),
    };

    // Calibrate + compute.
    let (values, valid_pixels, invalid_pixels, reasons, unit) = if product_key == "ndvi" {
        let mut bands = BTreeMap::new();
        for ((role, _), raster) in band_products.iter().zip(&rasters) {
            let scaled =
                apply_radiometric_scaling(SensorProfile::LandsatC2L2Sr, &band_to_dn(raster));
            bands.insert(*role, scaled.pixels);
        }
        let index = compute_masked_index(IndexKind::Ndvi, &bands, &clear)
            .map_err(|err| LandsatDeriveError::Index(err.to_string()))?;
        (
            index.values,
            index.valid_pixels,
            index.invalid_pixels,
            serde_json::json!(index.reason_counts),
            "ratio",
        )
    } else {
        let scaled =
            apply_radiometric_scaling(SensorProfile::LandsatC2L2St, &band_to_dn(&rasters[0]));
        let mut values = vec![INDEX_NODATA; pixel_count];
        let mut valid = 0usize;
        for (pixel, (sample, keep)) in scaled.pixels.iter().zip(&clear).enumerate() {
            if let (IndexPixelValue::Valid(kelvin), true) = (sample, keep) {
                values[pixel] = *kelvin;
                valid += 1;
            }
        }
        (
            values,
            valid,
            pixel_count - valid,
            serde_json::json!({ "fill_or_masked": pixel_count - valid }),
            "kelvin",
        )
    };

    let scene_product = &band_products[0].1;
    let mut draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: product_key.clone(),
        algorithm_id: format!("landsat_c2.{product_key}"),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "scene_id": request.scene_id,
            "product": product_key,
            "bands": band_products
                .iter()
                .map(|(_, product)| product.kind.clone())
                .collect::<Vec<_>>(),
            "sensor_profile": if product_key == "ndvi" {
                "landsat_c2l2_sr"
            } else {
                "landsat_c2l2_st"
            },
            "qa_applied": qa_applied,
            "unit": unit,
        }),
        inputs: band_products
            .iter()
            .map(|(role, product)| ProductInputRef {
                product_id: product.product_id.clone(),
                role: if product_key == "lst" {
                    "surface_temperature".to_string()
                } else {
                    role.key().to_string()
                },
            })
            .chain(qa_input)
            .collect(),
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: Some(request.scene_id.clone()),
            temporal_start: scene_product
                .temporal_start
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
            temporal_end: scene_product
                .temporal_end
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        },
        spatial_ref: Some(reference.spatial_ref.clone()),
        gsd_m_per_px: reference.geo_transform.map(|t| t[1].abs()),
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: scene_product.source_id.clone(),
    };

    let derived_dir = data_root.join("derived").join("landsat_index");
    std::fs::create_dir_all(&derived_dir).map_err(|source| LandsatDeriveError::Store {
        what: "landsat index directory",
        source,
    })?;
    let out_path = derived_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    write_geotiff_f32(
        &out_path,
        reference.width,
        reference.height,
        &values,
        &GeoTiffTags {
            epsg: reference.epsg,
            geo_transform: reference.geo_transform,
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let checksum = file_checksum(&out_path, "landsat derive readback")?;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: out_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    draft.quality_summary = Some(serde_json::json!({
        "valid_pixels": valid_pixels,
        "invalid_pixels": invalid_pixels,
        "reasons": reasons,
        "qa_applied": qa_applied,
    }));

    let actor = provenance::ActorIdentity::system("geo_hub:landsat_derive");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(LandsatDeriveOutcome {
        stac_item_href: format!("/api/stac/collections/{product_key}/items/{product_id}"),
        tiles_href: format!("/api/catalog/products/{product_id}/tiles/{{z}}/{{x}}/{{y}}.png"),
        product_id,
        product: product_key,
        scene_id: request.scene_id.clone(),
        valid_pixels,
        invalid_pixels,
        qa_applied,
        artifact: out_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qa_pixel_bits_are_pinned() {
        // fill(1), dilated cloud(2), cirrus(4), cloud(8), shadow(16) reject;
        // snow(32), clear(64), water(128) keep.
        for rejected in [1u16, 2, 4, 8, 16, 8 | 64, 21824 | 8] {
            assert!(!qa_pixel_clear(rejected), "{rejected}");
        }
        for kept in [0u16, 32, 64, 128, 64 | 128, 32 | 64] {
            assert!(qa_pixel_clear(kept), "{kept}");
        }
    }
}
