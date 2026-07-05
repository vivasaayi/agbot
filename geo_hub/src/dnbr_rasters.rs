//! Catalog-driven dNBR burn-severity derivation (satellite pipeline
//! batch 14, Phase 4).
//!
//! `POST /api/change-detection/dnbr/derive` takes the catalog ids of a
//! pre-event and post-event NBR L2 GeoTIFF on the same grid, runs the pure
//! `post_processor::burn_severity` engine, writes the dNBR GeoTIFF, and
//! registers it as a `dnbr` L3 with lineage to both inputs. The raster
//! web-tiles through the catalog tiler's diverging dNBR colormap and shows
//! in `/api/stac` + `/browse` like every other product.

use std::path::{Path, PathBuf};

use post_processor::burn_severity::{
    compute_dnbr, dnbr_l3_draft, BurnSeverityCounts, DnbrError, DnbrL3Scope, NbrRaster,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::ProductArtifact;
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, observed_on,
    DroughtRasterError, LoadedRaster,
};

/// Nodata for dNBR GeoTIFFs (invalid pixels).
pub const DNBR_NODATA: f32 = crate::satellite_derivation::INDEX_NODATA;
/// The L2 kind consumed.
pub const NBR_KIND: &str = "nbr";

#[derive(Debug, Error)]
pub enum DnbrRasterError {
    #[error("product {0} is not in the catalog")]
    NotFound(String),
    #[error("product {product_id} kind {kind:?} is not {NBR_KIND:?}")]
    NotNbr { product_id: String, kind: String },
    #[error("pre and post rasters are not on the same grid (no resampling)")]
    GridMismatch,
    #[error("pre product {pre} ({pre_date}) is not before post product {post} ({post_date})")]
    OrderViolation {
        pre: String,
        pre_date: String,
        post: String,
        post_date: String,
    },
    #[error("dNBR computation failed: {0}")]
    Dnbr(#[from] DnbrError),
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

impl DnbrRasterError {
    pub fn is_client_error(&self) -> bool {
        match self {
            DnbrRasterError::NotFound(_)
            | DnbrRasterError::NotNbr { .. }
            | DnbrRasterError::GridMismatch
            | DnbrRasterError::OrderViolation { .. }
            | DnbrRasterError::Dnbr(_) => true,
            DnbrRasterError::Shared(shared) => shared.is_client_error(),
            _ => false,
        }
    }
}

/// A dNBR derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct DnbrDeriveRequest {
    /// Catalog id of the pre-event NBR L2 product.
    pub pre_product_id: String,
    /// Catalog id of the post-event NBR L2 product.
    pub post_product_id: String,
    pub field_id: String,
    pub season_id: String,
}

/// Outcome of one derivation.
#[derive(Debug, Clone, Serialize)]
pub struct DnbrDeriveOutcome {
    pub dnbr_product_id: String,
    pub pre_product_id: String,
    pub post_product_id: String,
    pub class_counts: BurnSeverityCounts,
    pub disturbed_fraction: f32,
    pub valid_fraction: f32,
    pub dnbr_artifact: PathBuf,
    pub dnbr_stac_item_href: String,
    pub dnbr_tiles_href: String,
}

async fn load_nbr(
    pool: &DbPool,
    product_id: &str,
) -> Result<(RegisteredProduct, LoadedRaster), DnbrRasterError> {
    let product = catalog::get_product(pool, product_id)
        .await?
        .ok_or_else(|| DnbrRasterError::NotFound(product_id.to_string()))?;
    if product.kind != NBR_KIND {
        return Err(DnbrRasterError::NotNbr {
            product_id: product.product_id.clone(),
            kind: product.kind.clone(),
        });
    }
    let raster = load_raster(Path::new(geotiff_artifact_path(&product)?))?;
    Ok((product, raster))
}

/// Derive a dNBR burn-severity raster from two cataloged NBR products.
/// Idempotent on identical inputs (content-addressed identity).
pub async fn derive_dnbr(
    pool: &DbPool,
    data_root: &Path,
    request: &DnbrDeriveRequest,
) -> Result<DnbrDeriveOutcome, DnbrRasterError> {
    let (pre_product, pre_raster) = load_nbr(pool, &request.pre_product_id).await?;
    let (post_product, post_raster) = load_nbr(pool, &request.post_product_id).await?;

    // Chronology is part of the algorithm's meaning: dNBR is pre − post.
    let pre_date = observed_on(&pre_product).ok_or_else(|| DroughtRasterError::BadTemporal {
        product_id: pre_product.product_id.clone(),
        value: pre_product.temporal_start.clone(),
    })?;
    let post_date = observed_on(&post_product).ok_or_else(|| DroughtRasterError::BadTemporal {
        product_id: post_product.product_id.clone(),
        value: post_product.temporal_start.clone(),
    })?;
    if pre_date >= post_date {
        return Err(DnbrRasterError::OrderViolation {
            pre: pre_product.product_id.clone(),
            pre_date: pre_date.to_string(),
            post: post_product.product_id.clone(),
            post_date: post_date.to_string(),
        });
    }
    if pre_raster.epsg != post_raster.epsg
        || pre_raster.geo_transform != post_raster.geo_transform
        || (pre_raster.width, pre_raster.height) != (post_raster.width, post_raster.height)
    {
        return Err(DnbrRasterError::GridMismatch);
    }

    let to_engine = |product: &RegisteredProduct, raster: &LoadedRaster| NbrRaster {
        product_id: product.product_id.clone(),
        width: raster.width,
        height: raster.height,
        spatial_ref: raster.spatial_ref.clone(),
        values: raster.values.clone(),
        valid_mask: raster.valid_mask.clone(),
    };
    let result = compute_dnbr(
        &to_engine(&pre_product, &pre_raster),
        &to_engine(&post_product, &post_raster),
    )?;

    // --- GeoTIFF + L3 registration.
    let mut draft = dnbr_l3_draft(
        &result,
        &DnbrL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            temporal_start: pre_product
                .temporal_start
                .clone()
                .unwrap_or_else(|| format!("{pre_date}T00:00:00Z")),
            temporal_end: post_product
                .temporal_end
                .clone()
                .unwrap_or_else(|| format!("{post_date}T23:59:59Z")),
            source_id: post_product.source_id.clone(),
        },
    );
    let dnbr_dir = data_root.join("derived").join("dnbr");
    std::fs::create_dir_all(&dnbr_dir).map_err(|source| DnbrRasterError::Store {
        what: "dnbr directory",
        source,
    })?;
    let dnbr_path = dnbr_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    let disk_values: Vec<f32> = result
        .values
        .iter()
        .map(|v| if v.is_finite() { *v } else { DNBR_NODATA })
        .collect();
    write_geotiff_f32(
        &dnbr_path,
        result.width,
        result.height,
        &disk_values,
        &GeoTiffTags {
            epsg: pre_raster.epsg,
            geo_transform: pre_raster.geo_transform,
            nodata: Some(f64::from(DNBR_NODATA)),
        },
    )?;
    let checksum = file_checksum(&dnbr_path, "dnbr raster readback")?;
    draft.spatial_ref = Some(pre_raster.spatial_ref.clone());
    draft.gsd_m_per_px = pre_product.gsd_m_per_px;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: dnbr_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    let actor = provenance::ActorIdentity::system("geo_hub:dnbr_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let dnbr_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(DnbrDeriveOutcome {
        dnbr_stac_item_href: format!("/api/stac/collections/dnbr/items/{dnbr_product_id}"),
        dnbr_tiles_href: format!(
            "/api/catalog/products/{dnbr_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
        dnbr_product_id,
        pre_product_id: pre_product.product_id,
        post_product_id: post_product.product_id,
        class_counts: result.class_counts,
        disturbed_fraction: result.disturbed_fraction,
        valid_fraction: result.valid_fraction,
        dnbr_artifact: dnbr_path,
    })
}

/// List registered dNBR L3 products, optionally by field.
pub async fn list_dnbr_products(
    pool: &DbPool,
    field_id: Option<String>,
) -> Result<Vec<RegisteredProduct>, CatalogError> {
    catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some("dnbr".to_string()),
            level: Some(shared::product_graph::ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id,
            ..ProductFilter::default()
        },
    )
    .await
}
