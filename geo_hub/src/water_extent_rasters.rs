//! Catalog-driven water-extent derivation (satellite pipeline batch 15 —
//! the water-availability half of Phase 3 item 9).
//!
//! `POST /api/water-management/extent/derive` takes the catalog id of a
//! water-index L2 GeoTIFF (`mndwi`, `ndwi`, `aweinsh`, `aweish`), runs the
//! pure `post_processor::water_extent` Otsu engine, writes the binary mask
//! GeoTIFF (1 water / 0 land / nodata invalid), and registers it as a
//! `water_extent` L3 with lineage and area evidence. Repeating the derive
//! over a scene series yields the water-availability time series
//! (`water_area_m2` per product) that `water_priority_app` consumes.

use std::path::{Path, PathBuf};

use post_processor::water_extent::{
    extract_water_extent_with_prior, water_extent_l3_draft, PriorGate, ThresholdMethod, WaterClass,
    WaterExtentConfig, WaterExtentError, WaterExtentL3Scope, WaterIndexRaster,
    WaterOccurrencePrior,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, DroughtRasterError,
};

/// Nodata for water-extent GeoTIFFs (invalid pixels).
pub const WATER_EXTENT_NODATA: f32 = crate::satellite_derivation::INDEX_NODATA;
/// Optical index kinds (high value = water).
pub const OPTICAL_WATER_INDEX_KINDS: &[&str] = &["mndwi", "ndwi", "aweinsh", "aweish"];
/// SAR backscatter kinds (low value = water; VV/VH sigma0 in dB).
pub const SAR_WATER_KINDS: &[&str] = &["sar_vv", "sar_vh", "sar_backscatter"];
/// Source id stamped on Sentinel-1 backscatter registrations.
pub const SENTINEL1_SOURCE_ID: &str = "sentinel-1-grd";
/// Source id stamped on JRC Global Surface Water registrations.
pub const JRC_GSW_SOURCE_ID: &str = "jrc-gsw";
/// Catalog kind of a registered occurrence prior raster.
pub const WATER_OCCURRENCE_KIND: &str = "water_occurrence";

/// Pick the extraction config for a product kind, or `None` if the kind is
/// not a supported water source.
pub fn config_for_kind(kind: &str) -> Option<WaterExtentConfig> {
    if OPTICAL_WATER_INDEX_KINDS.contains(&kind) {
        Some(WaterExtentConfig::optical())
    } else if SAR_WATER_KINDS.contains(&kind) {
        Some(WaterExtentConfig::sar_backscatter())
    } else {
        None
    }
}

#[derive(Debug, Error)]
pub enum WaterExtentRasterError {
    #[error("product {0} is not in the catalog")]
    NotFound(String),
    #[error("product {product_id} kind {kind:?} is not a water index or SAR backscatter (optical {OPTICAL_WATER_INDEX_KINDS:?} / SAR {SAR_WATER_KINDS:?})")]
    NotWaterIndex { product_id: String, kind: String },
    #[error("water-extent computation failed: {0}")]
    Extent(#[from] WaterExtentError),
    #[error(transparent)]
    Shared(#[from] DroughtRasterError),
    #[error("raster I/O failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] CatalogError),
    #[error("prior product {product_id} kind {kind:?} is not a {WATER_OCCURRENCE_KIND} raster")]
    NotAPrior { product_id: String, kind: String },
    #[error("prior product {product_id} is not on the water-index raster's grid")]
    PriorGridMismatch { product_id: String },
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl WaterExtentRasterError {
    pub fn is_client_error(&self) -> bool {
        match self {
            WaterExtentRasterError::NotFound(_)
            | WaterExtentRasterError::NotWaterIndex { .. }
            | WaterExtentRasterError::NotAPrior { .. }
            | WaterExtentRasterError::PriorGridMismatch { .. }
            | WaterExtentRasterError::Extent(_) => true,
            WaterExtentRasterError::Shared(shared) => shared.is_client_error(),
            _ => false,
        }
    }
}

/// A water-extent derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct WaterExtentDeriveRequest {
    /// Catalog id of the water-index L2 product.
    pub product_id: String,
    pub field_id: String,
    pub season_id: String,
    /// Catalog id of a registered `water_occurrence` prior (JRC Global
    /// Surface Water) on the same grid, gating Otsu polarity flips.
    #[serde(default)]
    pub prior_product_id: Option<String>,
}

/// Outcome of one derivation.
#[derive(Debug, Clone, Serialize)]
pub struct WaterExtentDeriveOutcome {
    pub water_extent_product_id: String,
    pub input_product_id: String,
    pub method: ThresholdMethod,
    pub threshold: f32,
    pub water_pixels: u32,
    pub water_fraction: f32,
    pub water_area_m2: Option<f64>,
    pub valid_fraction: f32,
    /// Prior-gate outcome when a prior was supplied.
    pub prior_gate: Option<PriorGate>,
    pub water_extent_artifact: PathBuf,
    pub water_extent_stac_item_href: String,
    pub water_extent_tiles_href: String,
}

/// Derive a water-extent mask from one cataloged water-index product.
/// Idempotent on identical inputs.
pub async fn derive_water_extent(
    pool: &DbPool,
    data_root: &Path,
    request: &WaterExtentDeriveRequest,
) -> Result<WaterExtentDeriveOutcome, WaterExtentRasterError> {
    let product = catalog::get_product(pool, &request.product_id)
        .await?
        .ok_or_else(|| WaterExtentRasterError::NotFound(request.product_id.clone()))?;
    let config =
        config_for_kind(&product.kind).ok_or_else(|| WaterExtentRasterError::NotWaterIndex {
            product_id: product.product_id.clone(),
            kind: product.kind.clone(),
        })?;
    let raster = load_raster(Path::new(geotiff_artifact_path(&product)?))?;

    // Optional JRC occurrence prior on the exact same grid.
    let prior = match &request.prior_product_id {
        None => None,
        Some(prior_id) => {
            let prior_product = catalog::get_product(pool, prior_id)
                .await?
                .ok_or_else(|| WaterExtentRasterError::NotFound(prior_id.clone()))?;
            if prior_product.kind != WATER_OCCURRENCE_KIND {
                return Err(WaterExtentRasterError::NotAPrior {
                    product_id: prior_id.clone(),
                    kind: prior_product.kind.clone(),
                });
            }
            let prior_raster = load_raster(Path::new(geotiff_artifact_path(&prior_product)?))?;
            if prior_raster.epsg != raster.epsg
                || prior_raster.geo_transform != raster.geo_transform
                || (prior_raster.width, prior_raster.height) != (raster.width, raster.height)
            {
                return Err(WaterExtentRasterError::PriorGridMismatch {
                    product_id: prior_id.clone(),
                });
            }
            Some(WaterOccurrencePrior {
                product_id: prior_product.product_id.clone(),
                occurrence: prior_raster.values,
                valid_mask: prior_raster.valid_mask,
            })
        }
    };

    let result = extract_water_extent_with_prior(
        &WaterIndexRaster {
            product_id: product.product_id.clone(),
            index_kind: product.kind.clone(),
            width: raster.width,
            height: raster.height,
            spatial_ref: raster.spatial_ref.clone(),
            values: raster.values.clone(),
            valid_mask: raster.valid_mask.clone(),
            gsd_m_per_px: product.gsd_m_per_px,
        },
        &config,
        prior.as_ref(),
    )?;

    // --- Mask GeoTIFF + L3 registration.
    let mut draft = water_extent_l3_draft(
        &result,
        &WaterExtentL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: product.scene_id.clone(),
            temporal_start: product
                .temporal_start
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
            temporal_end: product
                .temporal_end
                .clone()
                .unwrap_or_else(|| "1970-01-01T23:59:59Z".to_string()),
            source_id: product.source_id.clone(),
        },
    );
    let extent_dir = data_root.join("derived").join("water_extent");
    std::fs::create_dir_all(&extent_dir).map_err(|source| WaterExtentRasterError::Store {
        what: "water_extent directory",
        source,
    })?;
    let extent_path = extent_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    let mask_values: Vec<f32> = result
        .classes
        .iter()
        .map(|class| match class {
            WaterClass::Water => 1.0,
            WaterClass::Land => 0.0,
            WaterClass::Invalid => WATER_EXTENT_NODATA,
        })
        .collect();
    write_geotiff_f32(
        &extent_path,
        result.width,
        result.height,
        &mask_values,
        &GeoTiffTags {
            epsg: raster.epsg,
            geo_transform: raster.geo_transform,
            nodata: Some(f64::from(WATER_EXTENT_NODATA)),
        },
    )?;
    let checksum = file_checksum(&extent_path, "water extent readback")?;
    draft.spatial_ref = Some(raster.spatial_ref.clone());
    draft.gsd_m_per_px = product.gsd_m_per_px;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: extent_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    let actor = provenance::ActorIdentity::system("geo_hub:water_extent_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let water_extent_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(WaterExtentDeriveOutcome {
        water_extent_stac_item_href: format!(
            "/api/stac/collections/water_extent/items/{water_extent_product_id}"
        ),
        water_extent_tiles_href: format!(
            "/api/catalog/products/{water_extent_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
        water_extent_product_id,
        input_product_id: product.product_id,
        prior_gate: result.evidence.prior.as_ref().map(|prior| prior.gate),
        method: result.evidence.method,
        threshold: result.evidence.threshold,
        water_pixels: result.water_pixels,
        water_fraction: result.water_fraction,
        water_area_m2: result.water_area_m2,
        valid_fraction: result.valid_fraction,
        water_extent_artifact: extent_path,
    })
}

/// List registered water-extent L3 products, optionally by field — the
/// per-scene `water_area_m2` in each product's parameters is the
/// water-availability time series.
pub async fn list_water_extent_products(
    pool: &DbPool,
    field_id: Option<String>,
) -> Result<Vec<RegisteredProduct>, CatalogError> {
    catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some("water_extent".to_string()),
            level: Some(shared::product_graph::ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id,
            ..ProductFilter::default()
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// Sentinel-1 backscatter registration
// ---------------------------------------------------------------------------
//
// Raw S1 GRD -> calibrated VV/VH sigma0 (dB) needs SNAP/pyroSAR (calibration
// + speckle filter + terrain flattening), run out-of-band as a subprocess
// like Sen2Cor. This registers those pre-processed GeoTIFFs; deriving water
// extent over them (low-value polarity) is the all-weather water layer.

/// (scene_id, polarization kind, acquired_at) parsed from a calibrated S1
/// backscatter filename, e.g.
/// `S1A_IW_GRDH_1SDV_20240601T051651_..._VV.tif` -> the `sar_vv` kind.
pub fn parse_sentinel1_filename(name: &str) -> Option<(String, &'static str, String)> {
    let stem = name
        .strip_suffix(".tif")
        .or_else(|| name.strip_suffix(".tiff"))?;
    let (base, kind) = if let Some(base) = stem.strip_suffix("_VV") {
        (base, "sar_vv")
    } else if let Some(base) = stem.strip_suffix("_VH") {
        (base, "sar_vh")
    } else {
        return None;
    };
    if !base.starts_with("S1") {
        return None;
    }
    // First YYYYMMDDTHHMMSS segment is the sensing start.
    let stamp = base.split('_').find(|segment| {
        segment.len() == 15
            && segment.as_bytes()[8] == b'T'
            && segment[..8].chars().all(|c| c.is_ascii_digit())
    })?;
    let datetime = chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%S").ok()?;
    Some((
        stem.to_string(),
        kind,
        format!("{}Z", datetime.format("%Y-%m-%dT%H:%M:%S")),
    ))
}

/// Build the L2 draft for one calibrated S1 backscatter GeoTIFF.
pub fn sentinel1_draft(
    path: &Path,
    scene_id: &str,
    kind: &str,
    acquired_at: &str,
) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: kind.to_string(),
        algorithm_id: "sentinel1.backscatter.ingest".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "dataset": "Sentinel-1 GRD calibrated backscatter",
            "provider": "ESA / ASF (calibrated out-of-band)",
            "polarization": kind.strip_prefix("sar_").unwrap_or(kind).to_uppercase(),
            "units": "dB (sigma0)",
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(scene_id.to_string()),
            temporal_start: acquired_at.to_string(),
            temporal_end: acquired_at.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
        artifact: Some(ProductArtifact {
            format: "tif".to_string(),
            path: path.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(SENTINEL1_SOURCE_ID.to_string()),
    }
}

/// Outcome of a Sentinel-1 directory registration.
#[derive(Debug, Clone, Serialize)]
pub struct SarRegisterOutcome {
    pub registered: Vec<(String, String)>,
    pub skipped: Vec<String>,
}

/// Register every calibrated S1 backscatter GeoTIFF in a local directory
/// (files calibrated out-of-band; idempotent; non-matching names skipped).
pub async fn register_sentinel1_dir(
    pool: &DbPool,
    dir: &Path,
) -> Result<SarRegisterOutcome, WaterExtentRasterError> {
    let mut names: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .map_err(|source| WaterExtentRasterError::Store {
            what: "sentinel-1 directory listing",
            source,
        })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            Some((
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            ))
        })
        .collect();
    names.sort();
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut outcome = SarRegisterOutcome {
        registered: Vec::new(),
        skipped: Vec::new(),
    };
    for (name, path) in names {
        match parse_sentinel1_filename(&name) {
            Some((scene_id, kind, acquired_at)) => {
                let draft = sentinel1_draft(&path, &scene_id, kind, &acquired_at);
                let product_id = catalog::register_product_with_actor(
                    pool,
                    &draft,
                    &provenance::ActorIdentity::system("geo_hub:sentinel1_ingest"),
                    &created_at,
                )
                .await?;
                outcome.registered.push((name, product_id));
            }
            None => outcome.skipped.push(name),
        }
    }
    Ok(outcome)
}

// ---------------------------------------------------------------------------
// JRC Global Surface Water registration (batch 25)
// ---------------------------------------------------------------------------
//
// The JRC GSW `occurrence` band (Pekel et al., percent of valid observations
// water over 1984-2021) is the long-term reference that gates Otsu polarity
// flips. Download is out-of-band (tiles from the JRC data portal, or
// clipped/reprojected onto the working grid with GDAL); this registers those
// occurrence GeoTIFFs as `water_occurrence` catalog products the derive
// route accepts as `prior_product_id`.

/// (tile token, dataset version) parsed from a JRC GSW occurrence filename,
/// e.g. `occurrence_70E_20Nv1_4_2021.tif` -> (`70E_20N`, `1_4_2021`).
pub fn parse_jrc_filename(name: &str) -> Option<(String, String)> {
    let stem = name
        .strip_suffix(".tif")
        .or_else(|| name.strip_suffix(".tiff"))?;
    let rest = stem.strip_prefix("occurrence_")?;
    // Tile token ends at the `v` introducing the version.
    let v_at = rest.rfind('v')?;
    let (tile, version) = (rest.get(..v_at)?, rest.get(v_at + 1..)?);
    if tile.is_empty() || version.is_empty() {
        return None;
    }
    Some((tile.to_string(), version.to_string()))
}

/// Build the draft for one JRC occurrence GeoTIFF. The record period is the
/// dataset's own (1984-2021 for v1.4); occurrence is percent 0-100.
pub fn jrc_occurrence_draft(path: &Path, tile: &str, version: &str) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: WATER_OCCURRENCE_KIND.to_string(),
        algorithm_id: "jrc.gsw.occurrence.ingest".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "dataset": "JRC Global Surface Water",
            "provider": "EC Joint Research Centre",
            "band": "occurrence",
            "units": "percent_of_observations_water",
            "tile": tile,
            "version": version,
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(format!("jrc-gsw-occurrence-{tile}-v{version}")),
            // The GSW v1.x record period (Landsat archive coverage).
            temporal_start: "1984-03-16T00:00:00Z".to_string(),
            temporal_end: "2021-12-31T23:59:59Z".to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: Some(ProductArtifact {
            format: "tif".to_string(),
            path: path.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(JRC_GSW_SOURCE_ID.to_string()),
    }
}

/// Outcome of a JRC directory registration.
#[derive(Debug, Clone, Serialize)]
pub struct JrcRegisterOutcome {
    pub registered: Vec<(String, String)>,
    pub skipped: Vec<String>,
}

/// Register every JRC occurrence GeoTIFF in a local directory (idempotent;
/// non-matching names skipped).
pub async fn register_jrc_dir(
    pool: &DbPool,
    dir: &Path,
) -> Result<JrcRegisterOutcome, WaterExtentRasterError> {
    let mut names: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .map_err(|source| WaterExtentRasterError::Store {
            what: "jrc directory listing",
            source,
        })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            Some((
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            ))
        })
        .collect();
    names.sort();
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut outcome = JrcRegisterOutcome {
        registered: Vec::new(),
        skipped: Vec::new(),
    };
    for (name, path) in names {
        match parse_jrc_filename(&name) {
            Some((tile, version)) => {
                let draft = jrc_occurrence_draft(&path, &tile, &version);
                let product_id = catalog::register_product_with_actor(
                    pool,
                    &draft,
                    &provenance::ActorIdentity::system("geo_hub:jrc_ingest"),
                    &created_at,
                )
                .await?;
                outcome.registered.push((name, product_id));
            }
            None => outcome.skipped.push(name),
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jrc_filename_parsing_is_pinned() {
        assert_eq!(
            parse_jrc_filename("occurrence_70E_20Nv1_4_2021.tif"),
            Some(("70E_20N".to_string(), "1_4_2021".to_string()))
        );
        assert_eq!(
            parse_jrc_filename("occurrence_80W_10Sv1_4_2021.tiff"),
            Some(("80W_10S".to_string(), "1_4_2021".to_string()))
        );
        for bad in [
            "seasonality_70E_20Nv1_4_2021.tif", // other GSW band
            "occurrence_70E_20N.tif",           // no version
            "occurrencev1_4_2021.tif",          // no tile
            "readme.txt",
        ] {
            assert_eq!(parse_jrc_filename(bad), None, "{bad}");
        }
    }
}
