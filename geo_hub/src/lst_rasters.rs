//! LST (land surface temperature) L2 derivation (satellite pipeline
//! batch 23).
//!
//! Wires the pure `post_processor::lst` thermal engine to the product
//! catalog: a thermal-band DN GeoTIFF plus its radiometric calibration
//! becomes an emissivity-corrected LST raster in Kelvin, registered as an
//! `lst` L2 product. The drought path already maps `lst -> tci`
//! (`drought_rasters::drought_kind_for`), so a multi-year archive of these
//! products scores into TCI — and, blended with a same-grid VCI, into VHI.
//!
//! Emissivity comes from a cataloged same-grid NDVI product (per-pixel
//! threshold model, recorded as a lineage input) or a caller constant
//! (default soil emissivity). Sentinel-2 has no thermal band; the expected
//! feeders are Landsat Collection-2 thermal bands (B10) or drone thermal
//! rasters with known calibration.

use std::path::{Path, PathBuf};

use post_processor::lst::{
    compute_lst, EmissivitySource, LstCoefficients, LstError, LstPixelReason, LstRequest,
    LstStageStats, SOIL_EMISSIVITY,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use thiserror::Error;

use crate::catalog::{self, CatalogError};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, DroughtRasterError,
    LoadedRaster,
};
use crate::satellite_derivation::INDEX_NODATA;

#[derive(Debug, Error)]
pub enum LstRasterError {
    #[error("acquired_on {0:?} is not an ISO date (YYYY-MM-DD)")]
    BadDate(String),
    #[error("thermal raster {path} is unreadable: {source}")]
    ThermalUnreadable {
        path: String,
        #[source]
        source: raster_io::RasterIoError,
    },
    #[error("ndvi product {0} is not in the catalog")]
    NdviNotFound(String),
    #[error("ndvi product {product_id} is not on the thermal raster's grid")]
    GridMismatch { product_id: String },
    #[error("provide either ndvi_product_id or emissivity_constant, not both")]
    EmissivityConflict,
    #[error("LST computation failed: {0}")]
    Engine(#[from] LstError),
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

impl LstRasterError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            LstRasterError::BadDate(_)
                | LstRasterError::ThermalUnreadable { .. }
                | LstRasterError::NdviNotFound(_)
                | LstRasterError::GridMismatch { .. }
                | LstRasterError::EmissivityConflict
                | LstRasterError::Engine(_)
        )
    }
}

/// One LST derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct LstDeriveRequest {
    /// Server-local thermal-band DN GeoTIFF (fill masked via its nodata tag).
    pub thermal_tif: String,
    /// Scene the thermal band belongs to (identity-bearing).
    pub scene_id: String,
    /// Acquisition date `YYYY-MM-DD` (identity-bearing; the thermal GeoTIFF
    /// carries no date metadata).
    pub acquired_on: String,
    /// Radiometric calibration from the scene metadata (ML/AL/K1/K2/λ).
    pub coefficients: LstCoefficients,
    /// Cataloged same-grid NDVI product for per-pixel emissivity.
    #[serde(default)]
    pub ndvi_product_id: Option<String>,
    /// Constant emissivity in (0, 1] (default soil 0.97) when no NDVI
    /// product is given.
    #[serde(default)]
    pub emissivity_constant: Option<f32>,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
}

/// Outcome of one LST derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct LstDeriveOutcome {
    pub lst_product_id: String,
    pub valid_fraction: f32,
    /// Kelvin min/max/mean over computed pixels.
    pub lst_stats: LstStageStats,
    /// `constant` or `ndvi_thresholds`.
    pub emissivity_method: String,
    pub lst_artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

/// Resolve the emissivity source, returning the NDVI lineage input when a
/// cataloged product supplies it.
async fn resolve_emissivity(
    pool: &DbPool,
    request: &LstDeriveRequest,
    thermal: &LoadedRaster,
) -> Result<(EmissivitySource, Option<ProductInputRef>), LstRasterError> {
    match (&request.ndvi_product_id, request.emissivity_constant) {
        (Some(_), Some(_)) => Err(LstRasterError::EmissivityConflict),
        (Some(product_id), None) => {
            let product = catalog::get_product(pool, product_id)
                .await?
                .ok_or_else(|| LstRasterError::NdviNotFound(product_id.clone()))?;
            let ndvi = load_raster(Path::new(geotiff_artifact_path(&product)?))?;
            if ndvi.epsg != thermal.epsg
                || ndvi.geo_transform != thermal.geo_transform
                || (ndvi.width, ndvi.height) != (thermal.width, thermal.height)
            {
                return Err(LstRasterError::GridMismatch {
                    product_id: product_id.clone(),
                });
            }
            // Masked NDVI pixels become NaN so the engine's soil fallback
            // applies (a cloudy NDVI pixel must not zero out the LST).
            let ndvi_values = ndvi
                .values
                .iter()
                .zip(&ndvi.valid_mask)
                .map(|(v, valid)| if *valid { *v } else { f32::NAN })
                .collect();
            Ok((
                EmissivitySource::FromNdvi { ndvi: ndvi_values },
                Some(ProductInputRef {
                    product_id: product_id.clone(),
                    role: "emissivity_ndvi".to_string(),
                }),
            ))
        }
        (None, constant) => Ok((
            EmissivitySource::Constant(constant.unwrap_or(SOIL_EMISSIVITY)),
            None,
        )),
    }
}

/// Derive an emissivity-corrected LST raster from a thermal DN GeoTIFF and
/// register it as an `lst` L2 product (Kelvin, index-nodata on disk).
/// Idempotent: identical inputs re-register the same content-addressed id.
pub async fn derive_lst_raster(
    pool: &DbPool,
    data_root: &Path,
    request: &LstDeriveRequest,
) -> Result<LstDeriveOutcome, LstRasterError> {
    let acquired_on = chrono::NaiveDate::parse_from_str(request.acquired_on.trim(), "%Y-%m-%d")
        .map_err(|_| LstRasterError::BadDate(request.acquired_on.clone()))?;
    let thermal = load_raster(Path::new(&request.thermal_tif)).map_err(|source| {
        LstRasterError::ThermalUnreadable {
            path: request.thermal_tif.clone(),
            source,
        }
    })?;
    let (emissivity, ndvi_input) = resolve_emissivity(pool, request, &thermal).await?;
    let emissivity_method = emissivity.method();

    let result = compute_lst(&LstRequest {
        width: thermal.width,
        height: thermal.height,
        spatial_ref: thermal.spatial_ref.clone(),
        dn: thermal.values.clone(),
        valid_mask: thermal.valid_mask.clone(),
        emissivity,
        coefficients: request.coefficients,
    })?;

    let mut reasons = std::collections::BTreeMap::new();
    for reason in &result.reason_codes {
        let key = match reason {
            LstPixelReason::Computed => "computed",
            LstPixelReason::NoObservation => "no_observation",
            LstPixelReason::NonPositiveRadiance => "non_positive_radiance",
        };
        *reasons.entry(key).or_insert(0u32) += 1;
    }

    let day = acquired_on.format("%Y-%m-%d");
    let mut draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "lst".to_string(),
        algorithm_id: "thermal.lst".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "scene_id": request.scene_id,
            "acquired_on": day.to_string(),
            "coefficients": request.coefficients,
            "emissivity_method": emissivity_method,
            "emissivity_constant": request.emissivity_constant,
            "emissivity_ndvi_product_id": request.ndvi_product_id,
            "unit": "kelvin",
        }),
        inputs: ndvi_input.into_iter().collect(),
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: Some(request.scene_id.clone()),
            temporal_start: format!("{day}T00:00:00Z"),
            temporal_end: format!("{day}T23:59:59Z"),
        },
        spatial_ref: Some(thermal.spatial_ref.clone()),
        gsd_m_per_px: thermal.geo_transform.map(|t| t[1].abs()),
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: vec![result.evidence.input_hash.clone()],
        source_id: request.source_id.clone(),
    };

    let lst_dir = data_root.join("derived").join("lst");
    std::fs::create_dir_all(&lst_dir).map_err(|source| LstRasterError::Store {
        what: "lst directory",
        source,
    })?;
    let lst_path = lst_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    let disk_values: Vec<f32> = result
        .values
        .iter()
        .map(|v| if v.is_finite() { *v } else { INDEX_NODATA })
        .collect();
    write_geotiff_f32(
        &lst_path,
        result.width,
        result.height,
        &disk_values,
        &GeoTiffTags {
            epsg: thermal.epsg,
            geo_transform: thermal.geo_transform,
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let checksum = file_checksum(&lst_path, "lst raster readback")?;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: lst_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    draft.quality_summary = Some(serde_json::json!({
        "valid_fraction": result.valid_fraction,
        "reasons": reasons,
        "radiance": result.radiance_stats,
        "brightness_temperature": result.brightness_temperature_stats,
        "lst": result.lst_stats,
        "emissivity": result.emissivity_stats,
    }));

    let actor = provenance::ActorIdentity::system("geo_hub:lst_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let lst_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(LstDeriveOutcome {
        stac_item_href: format!("/api/stac/collections/lst/items/{lst_product_id}"),
        tiles_href: format!("/api/catalog/products/{lst_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"),
        lst_product_id,
        valid_fraction: result.valid_fraction,
        lst_stats: result.lst_stats,
        emissivity_method: emissivity_method.to_string(),
        lst_artifact: lst_path,
    })
}
