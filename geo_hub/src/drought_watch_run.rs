//! Drought-watch application run (satellite pipeline batch 36).
//!
//! Composes the pure `post_processor::drought_watch_app` evaluator into a
//! governed application run: registered `drought_index` L3 rasters
//! (VCI/TCI/VHI) -> per-product drought findings -> catalog
//! `ApplicationFinding`s via [`applications::record_run`], so every finding
//! traces to the drought product (and through it to the climatology and
//! index observations). The `drought_stress_zone` findings this emits are
//! screened by Track C's alert evaluation into early-warning alerts.
//! Mirrors [`crate::anomaly_run`].

use std::path::Path;

use post_processor::drought_watch_app::{
    evaluate_drought_watch, DroughtProductReading, DroughtWatchConfig, DroughtWatchFinding,
};
use serde::Deserialize;
use serde_json::json;
use shared::schemas::RecommendationPriority;

use crate::applications::{
    self, ApplicationError, ApplicationFinding, ApplicationRunRecord, ApplicationRunRequest,
};
use crate::catalog;
use crate::db::DbPool;
use crate::drought_rasters::{geotiff_artifact_path, load_raster};

/// The application id all drought-watch runs are attributed to.
pub const APP_ID: &str = "drought_watch";

/// Request to run drought watch over registered drought products.
#[derive(Debug, Clone, Deserialize)]
pub struct DroughtWatchRunRequest {
    #[serde(default)]
    pub org_id: Option<String>,
    pub field_id: String,
    /// Catalog ids of registered `drought_index` L3 products (VCI/TCI/VHI).
    pub product_ids: Vec<String>,
    #[serde(default)]
    pub warning_stressed_fraction: Option<f32>,
    #[serde(default)]
    pub critical_stressed_fraction: Option<f32>,
    #[serde(default)]
    pub min_valid_fraction: Option<f32>,
}

/// Run drought watch: load each drought raster, evaluate stress fractions,
/// and record a governed run with per-finding lineage to the products.
pub async fn run(
    pool: &DbPool,
    request: &DroughtWatchRunRequest,
    created_at: &str,
) -> Result<ApplicationRunRecord, ApplicationError> {
    let mut readings = Vec::new();
    for product_id in &request.product_ids {
        let product = catalog::get_product(pool, product_id)
            .await?
            .ok_or_else(|| ApplicationError::InputNotFound(product_id.clone()))?;
        if product.kind != "drought_index" {
            return Err(ApplicationError::InputNotL2OrL3 {
                product_id: product_id.clone(),
                level: format!(
                    "kind {} (drought_watch consumes drought_index L3s)",
                    product.kind
                ),
            });
        }
        let path = geotiff_artifact_path(&product)
            .map_err(|_| ApplicationError::InputNotFound(product_id.clone()))?;
        let raster = load_raster(Path::new(path))
            .map_err(|_| ApplicationError::InputNotFound(product_id.clone()))?;
        readings.push(DroughtProductReading {
            product_id: product_id.clone(),
            index_kind: product
                .parameters
                .get("index_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            values: raster.values,
            valid_mask: raster.valid_mask,
        });
    }

    let config = DroughtWatchConfig {
        warning_stressed_fraction: request.warning_stressed_fraction,
        critical_stressed_fraction: request.critical_stressed_fraction,
        min_valid_fraction: request.min_valid_fraction,
    };
    let findings = evaluate_drought_watch(&readings, &config);
    let application_findings: Vec<ApplicationFinding> =
        findings.iter().map(to_application_finding).collect();

    let mut input_product_ids = request.product_ids.clone();
    input_product_ids.sort();
    input_product_ids.dedup();

    let run_request = ApplicationRunRequest {
        org_id: request.org_id.clone(),
        field_id: request.field_id.clone(),
        input_product_ids,
        params: json!({
            "warning_stressed_fraction": request.warning_stressed_fraction,
            "critical_stressed_fraction": request.critical_stressed_fraction,
            "min_valid_fraction": request.min_valid_fraction,
        }),
        findings: application_findings,
    };
    applications::record_run(pool, APP_ID, &run_request, created_at).await
}

/// Map a pure drought finding into a catalog `ApplicationFinding`. Drought
/// products in stress become `drought_stress_zone` (the Track C alert
/// input); nominal / thin-coverage products are recorded for completeness.
fn to_application_finding(finding: &DroughtWatchFinding) -> ApplicationFinding {
    let kind = if finding.is_drought {
        "drought_stress_zone"
    } else if finding.reason_code == "insufficient_coverage" {
        "insufficient_coverage_zone"
    } else {
        "nominal_zone"
    };
    ApplicationFinding {
        kind: kind.to_string(),
        severity: Some(severity_for(finding.priority).to_string()),
        confidence: Some(f64::from(finding.valid_fraction)),
        zone_geometry: None,
        metrics: Some(json!({
            "index_kind": finding.index_kind,
            "stressed_fraction": finding.stressed_fraction,
            "extreme_fraction": finding.extreme_fraction,
            "mean_index": finding.mean_index,
            "valid_fraction": finding.valid_fraction,
            "reason_code": finding.reason_code,
        })),
        evidence_refs: vec![finding.product_id.clone()],
    }
}

fn severity_for(priority: RecommendationPriority) -> &'static str {
    match priority {
        RecommendationPriority::Critical => "critical",
        RecommendationPriority::High => "high",
        RecommendationPriority::Medium => "medium",
        RecommendationPriority::Low => "low",
    }
}
