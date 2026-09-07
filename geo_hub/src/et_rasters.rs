//! Catalog-driven ET-fraction derivation (satellite pipeline batch 40).
//!
//! Wires the pure `post_processor::et_fraction` Ts–VI triangle engine to
//! the catalog: a registered same-grid `lst` + `ndvi` product pair (any
//! source — Landsat local derive produces both) becomes an `et_fraction`
//! L2 GeoTIFF in [0, 1] with lineage to both inputs and the self-
//! calibrated edges in the evidence. This is the demand side of water
//! availability; conversion to mm/day awaits a reference-ET source
//! (Hargreaves–Samani over the FAO-56 Ra already implemented).

use std::path::{Path, PathBuf};

use post_processor::et_fraction::{
    compute_et_fraction, EtFractionError, EtFractionRequest, EtPixelReason,
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
pub enum EtRasterError {
    #[error("product {0} is not in the catalog")]
    NotFound(String),
    #[error("product {product_id} kind {kind:?} is not {expected:?}")]
    WrongKind {
        product_id: String,
        kind: String,
        expected: &'static str,
    },
    #[error("lst and ndvi products are not on the same grid")]
    GridMismatch,
    #[error("ET-fraction computation failed: {0}")]
    Engine(#[from] EtFractionError),
    #[error(transparent)]
    Shared(#[from] DroughtRasterError),
    #[error("raster I/O failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] CatalogError),
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl EtRasterError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            EtRasterError::NotFound(_)
                | EtRasterError::WrongKind { .. }
                | EtRasterError::GridMismatch
                | EtRasterError::Engine(_)
        )
    }
}

/// One ET-fraction derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct EtDeriveRequest {
    /// Catalog id of a registered `lst` L2 (Kelvin).
    pub lst_product_id: String,
    /// Catalog id of the same-grid `ndvi` L2.
    pub ndvi_product_id: String,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct EtDeriveOutcome {
    pub et_product_id: String,
    pub lst_product_id: String,
    pub ndvi_product_id: String,
    pub valid_fraction: f32,
    pub mean_fraction: f32,
    /// Self-calibrated scene wet edge (Kelvin).
    pub wet_edge_k: f32,
    pub et_artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

async fn expect_product(
    pool: &DbPool,
    product_id: &str,
    expected: &'static str,
) -> Result<(crate::catalog::RegisteredProduct, LoadedRaster), EtRasterError> {
    let product = catalog::get_product(pool, product_id)
        .await?
        .ok_or_else(|| EtRasterError::NotFound(product_id.to_string()))?;
    if product.kind != expected {
        return Err(EtRasterError::WrongKind {
            product_id: product_id.to_string(),
            kind: product.kind.clone(),
            expected,
        });
    }
    let raster = load_raster(Path::new(geotiff_artifact_path(&product)?))?;
    Ok((product, raster))
}

/// Derive the triangle ET fraction from a same-grid LST + NDVI pair and
/// register it as an `et_fraction` L2 (dimensionless [0, 1], index-nodata
/// on disk). Idempotent (content-addressed ids).
pub async fn derive_et_fraction(
    pool: &DbPool,
    data_root: &Path,
    request: &EtDeriveRequest,
) -> Result<EtDeriveOutcome, EtRasterError> {
    let (lst_product, lst) = expect_product(pool, &request.lst_product_id, "lst").await?;
    let (ndvi_product, ndvi) = expect_product(pool, &request.ndvi_product_id, "ndvi").await?;
    if lst.epsg != ndvi.epsg
        || lst.geo_transform != ndvi.geo_transform
        || (lst.width, lst.height) != (ndvi.width, ndvi.height)
    {
        return Err(EtRasterError::GridMismatch);
    }

    let result = compute_et_fraction(&EtFractionRequest {
        width: lst.width,
        height: lst.height,
        spatial_ref: lst.spatial_ref.clone(),
        lst: lst.values.clone(),
        lst_valid: lst.valid_mask.clone(),
        ndvi: ndvi.values.clone(),
        ndvi_valid: ndvi.valid_mask.clone(),
    })?;

    let mut reasons = std::collections::BTreeMap::new();
    for reason in &result.reason_codes {
        let key = match reason {
            EtPixelReason::Computed => "computed",
            EtPixelReason::NoObservation => "no_observation",
            EtPixelReason::ThinBin => "thin_bin",
            EtPixelReason::DegenerateEdge => "degenerate_edge",
        };
        *reasons.entry(key).or_insert(0u32) += 1;
    }

    let mut draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "et_fraction".to_string(),
        algorithm_id: "et.ts_vi_triangle".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "method": "jiang_islam_triangle_phi_normalized",
            "lst_product_id": lst_product.product_id,
            "ndvi_product_id": ndvi_product.product_id,
            "wet_edge_k": result.evidence.wet_edge_k,
            "dry_edge_k": result.evidence.dry_edge_k,
            "ndvi_bins": result.evidence.ndvi_bins,
            "unit": "evaporative_fraction_0_1",
            "note": "instantaneous EF proxy; mm/day conversion awaits reference ET",
        }),
        inputs: vec![
            ProductInputRef {
                product_id: lst_product.product_id.clone(),
                role: "lst".to_string(),
            },
            ProductInputRef {
                product_id: ndvi_product.product_id.clone(),
                role: "ndvi".to_string(),
            },
        ],
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: lst_product.scene_id.clone(),
            temporal_start: lst_product
                .temporal_start
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
            temporal_end: lst_product
                .temporal_end
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        },
        spatial_ref: Some(lst.spatial_ref.clone()),
        gsd_m_per_px: lst_product.gsd_m_per_px,
        artifact: None,
        quality_mask: None,
        confidence: Some(f64::from(result.valid_fraction)),
        confidence_method: Some("valid_coverage_fraction".to_string()),
        quality_summary: None,
        evidence_digests: vec![result.evidence.input_hash.clone()],
        source_id: lst_product.source_id.clone(),
    };

    let et_dir = data_root.join("derived").join("et");
    std::fs::create_dir_all(&et_dir).map_err(|source| EtRasterError::Store {
        what: "et directory",
        source,
    })?;
    let et_path = et_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    let disk_values: Vec<f32> = result
        .values
        .iter()
        .map(|v| if v.is_finite() { *v } else { INDEX_NODATA })
        .collect();
    write_geotiff_f32(
        &et_path,
        result.width,
        result.height,
        &disk_values,
        &GeoTiffTags {
            epsg: lst.epsg,
            geo_transform: lst.geo_transform,
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let checksum = file_checksum(&et_path, "et readback")?;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: et_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    draft.quality_summary = Some(serde_json::json!({
        "valid_fraction": result.valid_fraction,
        "mean_fraction": result.mean_fraction,
        "reasons": reasons,
    }));

    let actor = provenance::ActorIdentity::system("geo_hub:et_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let et_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(EtDeriveOutcome {
        stac_item_href: format!("/api/stac/collections/et_fraction/items/{et_product_id}"),
        tiles_href: format!("/api/catalog/products/{et_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"),
        et_product_id,
        lst_product_id: lst_product.product_id,
        ndvi_product_id: ndvi_product.product_id,
        valid_fraction: result.valid_fraction,
        mean_fraction: result.mean_fraction,
        wet_edge_k: result.evidence.wet_edge_k,
        et_artifact: et_path,
    })
}
