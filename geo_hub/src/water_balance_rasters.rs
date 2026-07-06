//! Catalog-driven water-balance summary (satellite pipeline batch 41).
//!
//! Gathers a field's registered water products over a window and folds
//! them through the pure `post_processor::water_balance` engine:
//!
//! - supply: `water_extent` L3s (their identity-bearing `water_area_m2`
//!   parameter is the area series) + the latest `water_seasonality` L3
//!   anchor (permanent/seasonal areas);
//! - demand: `et_fraction` L2s — each raster's mean fraction over valid
//!   pixels, computed here from the artifact (deterministic);
//! - inflow: `precipitation` L2s (CHIRPS) — region-mean mm per product,
//!   summed (CHIRPS is regional, so these are gathered unscoped and
//!   labeled as region means).
//!
//! Registers a `water_balance` L3 with a JSON artifact and lineage to
//! every contributing product.

use std::path::{Path, PathBuf};

use post_processor::water_balance::{
    summarize_water_balance, water_balance_l3_draft, BalanceStatus, DemandLevel, DemandObservation,
    PrecipitationObservation, SeasonalityAnchor, SupplyObservation, SupplyTrend, WaterBalanceError,
    WaterBalanceInputs, WaterBalanceL3Scope,
};
use serde::{Deserialize, Serialize};
use shared::product_graph::{ProductArtifact, ProductLevel};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, observed_on,
    DroughtRasterError, SkippedObservation,
};

#[derive(Debug, Error)]
pub enum WaterBalanceRasterError {
    #[error("window start {0:?} is not an ISO date (YYYY-MM-DD)")]
    BadStart(String),
    #[error("window end {0:?} is not an ISO date (YYYY-MM-DD)")]
    BadEnd(String),
    #[error("balance summary failed: {0}")]
    Engine(#[from] WaterBalanceError),
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

impl WaterBalanceRasterError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            WaterBalanceRasterError::BadStart(_)
                | WaterBalanceRasterError::BadEnd(_)
                | WaterBalanceRasterError::Engine(_)
        )
    }
}

/// One balance derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct WaterBalanceDeriveRequest {
    pub field_id: String,
    pub season_id: String,
    /// Inclusive ISO date window selecting the product series.
    pub start: String,
    pub end: String,
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct WaterBalanceDeriveOutcome {
    pub water_balance_product_id: String,
    pub status: BalanceStatus,
    pub status_reason: String,
    pub supply_trend: SupplyTrend,
    pub demand_level: DemandLevel,
    pub relative_area_change: Option<f32>,
    pub mean_et_fraction: Option<f32>,
    pub total_precipitation_mm: Option<f32>,
    pub inputs_used: Vec<String>,
    pub inputs_skipped: Vec<SkippedObservation>,
    pub water_balance_artifact: PathBuf,
    pub stac_item_href: String,
}

async fn window_products(
    pool: &DbPool,
    kind: &str,
    level: ProductLevel,
    field_id: Option<&str>,
    start: &str,
    end: &str,
) -> Result<Vec<RegisteredProduct>, CatalogError> {
    let mut products = catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some(kind.to_string()),
            level: Some(level),
            status: Some("registered".to_string()),
            field_id: field_id.map(str::to_string),
            temporal_start: Some(format!("{start}T00:00:00Z")),
            temporal_end: Some(format!("{end}T23:59:59Z")),
            ..ProductFilter::default()
        },
    )
    .await?;
    products.sort_by_key(|p| {
        (
            p.temporal_start.clone().unwrap_or_default(),
            p.product_id.clone(),
        )
    });
    Ok(products)
}

/// A raster's mean over valid pixels, or `None` when unreadable/empty.
fn raster_mean(product: &RegisteredProduct) -> Option<f32> {
    let path = geotiff_artifact_path(product).ok()?;
    let raster = load_raster(Path::new(path)).ok()?;
    let mut sum = 0f64;
    let mut count = 0u32;
    for (value, valid) in raster.values.iter().zip(&raster.valid_mask) {
        if *valid && value.is_finite() {
            sum += f64::from(*value);
            count += 1;
        }
    }
    (count > 0).then(|| (sum / f64::from(count)) as f32)
}

/// Derive a field's water-balance summary over a window and register it as
/// a `water_balance` L3 (JSON artifact) with lineage to every contributing
/// product. Idempotent (content-addressed ids).
pub async fn derive_water_balance(
    pool: &DbPool,
    data_root: &Path,
    request: &WaterBalanceDeriveRequest,
) -> Result<WaterBalanceDeriveOutcome, WaterBalanceRasterError> {
    chrono::NaiveDate::parse_from_str(request.start.trim(), "%Y-%m-%d")
        .map_err(|_| WaterBalanceRasterError::BadStart(request.start.clone()))?;
    chrono::NaiveDate::parse_from_str(request.end.trim(), "%Y-%m-%d")
        .map_err(|_| WaterBalanceRasterError::BadEnd(request.end.clone()))?;

    let mut skipped = Vec::new();
    let skip = |id: &str, reason: &str, list: &mut Vec<SkippedObservation>| {
        list.push(SkippedObservation {
            product_id: id.to_string(),
            reason: reason.to_string(),
        });
    };

    // Supply: water_extent areas from identity-bearing parameters.
    let mut supply = Vec::new();
    for product in window_products(
        pool,
        "water_extent",
        ProductLevel::L3,
        Some(&request.field_id),
        &request.start,
        &request.end,
    )
    .await?
    {
        let (Some(date), Some(area)) = (
            observed_on(&product),
            product
                .parameters
                .get("water_area_m2")
                .and_then(|v| v.as_f64()),
        ) else {
            skip(&product.product_id, "missing_date_or_area", &mut skipped);
            continue;
        };
        supply.push(SupplyObservation {
            product_id: product.product_id.clone(),
            observed_on: date,
            water_area_m2: area,
        });
    }

    // Demand: mean ET fraction per et_fraction raster.
    let mut demand = Vec::new();
    for product in window_products(
        pool,
        "et_fraction",
        ProductLevel::L2,
        Some(&request.field_id),
        &request.start,
        &request.end,
    )
    .await?
    {
        let (Some(date), Some(mean)) = (observed_on(&product), raster_mean(&product)) else {
            skip(&product.product_id, "missing_date_or_raster", &mut skipped);
            continue;
        };
        demand.push(DemandObservation {
            product_id: product.product_id.clone(),
            observed_on: date,
            mean_et_fraction: mean,
        });
    }

    // Inflow: region-mean precipitation per CHIRPS product (unscoped).
    let mut precipitation = Vec::new();
    for product in window_products(
        pool,
        "precipitation",
        ProductLevel::L2,
        None,
        &request.start,
        &request.end,
    )
    .await?
    {
        let (Some(date), Some(mean)) = (observed_on(&product), raster_mean(&product)) else {
            skip(&product.product_id, "missing_date_or_raster", &mut skipped);
            continue;
        };
        precipitation.push(PrecipitationObservation {
            product_id: product.product_id.clone(),
            observed_on: date,
            mean_mm: mean,
        });
    }

    // Seasonality anchor: the latest registered product for the field.
    let seasonality = window_products(
        pool,
        "water_seasonality",
        ProductLevel::L3,
        Some(&request.field_id),
        &request.start,
        &request.end,
    )
    .await?
    .pop()
    .map(|product| SeasonalityAnchor {
        permanent_area_m2: product
            .parameters
            .get("permanent_area_m2")
            .and_then(|v| v.as_f64()),
        seasonal_area_m2: product
            .parameters
            .get("seasonal_area_m2")
            .and_then(|v| v.as_f64()),
        product_id: product.product_id,
    });

    let summary = summarize_water_balance(&WaterBalanceInputs {
        supply,
        demand,
        precipitation,
        seasonality,
    })?;

    let mut draft = water_balance_l3_draft(
        &summary,
        &WaterBalanceL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
        },
    );
    let balance_dir = data_root.join("derived").join("water_balance");
    std::fs::create_dir_all(&balance_dir).map_err(|source| WaterBalanceRasterError::Store {
        what: "water_balance directory",
        source,
    })?;
    let artifact_path = balance_dir.join(format!(
        "{}.json",
        artifact_file_component(&draft.product_id())
    ));
    let payload =
        serde_json::to_vec_pretty(&summary).map_err(|err| WaterBalanceRasterError::Store {
            what: "water_balance serialization",
            source: std::io::Error::other(err),
        })?;
    std::fs::write(&artifact_path, payload).map_err(|source| WaterBalanceRasterError::Store {
        what: "water_balance artifact",
        source,
    })?;
    let checksum = file_checksum(&artifact_path, "water_balance readback")?;
    draft.artifact = Some(ProductArtifact {
        format: "json".to_string(),
        path: artifact_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);

    let actor = provenance::ActorIdentity::system("geo_hub:water_balance");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(WaterBalanceDeriveOutcome {
        stac_item_href: format!("/api/stac/collections/water_balance/items/{product_id}"),
        water_balance_product_id: product_id,
        status: summary.status,
        status_reason: summary.status_reason.to_string(),
        supply_trend: summary.supply_trend,
        demand_level: summary.demand_level,
        relative_area_change: summary.relative_area_change,
        mean_et_fraction: summary.mean_et_fraction,
        total_precipitation_mm: summary.total_precipitation_mm,
        inputs_used: summary.input_product_ids.clone(),
        inputs_skipped: skipped,
        water_balance_artifact: artifact_path,
    })
}
