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
    extract_water_extent, water_extent_l3_draft, ThresholdMethod, WaterClass, WaterExtentError,
    WaterExtentL3Scope, WaterIndexRaster,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductArtifact;
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, DroughtRasterError,
};

/// Nodata for water-extent GeoTIFFs (invalid pixels).
pub const WATER_EXTENT_NODATA: f32 = crate::satellite_derivation::INDEX_NODATA;
/// Index kinds accepted as water indices.
pub const WATER_INDEX_KINDS: &[&str] = &["mndwi", "ndwi", "aweinsh", "aweish"];

#[derive(Debug, Error)]
pub enum WaterExtentRasterError {
    #[error("product {0} is not in the catalog")]
    NotFound(String),
    #[error("product {product_id} kind {kind:?} is not a water index (expected one of {WATER_INDEX_KINDS:?})")]
    NotWaterIndex { product_id: String, kind: String },
    #[error("water-extent computation failed: {0}")]
    Extent(#[from] WaterExtentError),
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

impl WaterExtentRasterError {
    pub fn is_client_error(&self) -> bool {
        match self {
            WaterExtentRasterError::NotFound(_)
            | WaterExtentRasterError::NotWaterIndex { .. }
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
    if !WATER_INDEX_KINDS.contains(&product.kind.as_str()) {
        return Err(WaterExtentRasterError::NotWaterIndex {
            product_id: product.product_id.clone(),
            kind: product.kind.clone(),
        });
    }
    let raster = load_raster(Path::new(geotiff_artifact_path(&product)?))?;

    let result = extract_water_extent(&WaterIndexRaster {
        product_id: product.product_id.clone(),
        index_kind: product.kind.clone(),
        width: raster.width,
        height: raster.height,
        spatial_ref: raster.spatial_ref.clone(),
        values: raster.values.clone(),
        valid_mask: raster.valid_mask.clone(),
        gsd_m_per_px: product.gsd_m_per_px,
    })?;

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
