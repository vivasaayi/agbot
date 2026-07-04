//! Crop-health application run (Track B phase B3).
//!
//! Composes the pure `post_processor::crop_health_app` analysis into a governed
//! application run: per-zone NDVI stats -> `CropHealthFinding`s -> catalog
//! `ApplicationFinding`s recorded via [`applications::record_run`], so every
//! finding traces back to the L2 catalog products it derived from.
//!
//! The zone stats and their `input_product_ids` (cataloged L2/L3 products) are
//! supplied by the caller; the run collects their union as the run inputs so
//! `record_run` enforces the L2/L3 invariant and writes lineage.

use crate::applications::{
    self, ApplicationError, ApplicationFinding, ApplicationRunRecord, ApplicationRunRequest,
};
use crate::db::DbPool;
use post_processor::crop_health_app::{
    run_crop_health, CropHealthFinding, TrendDirection, ZoneHealthInput,
};
use post_processor::ndvi_analysis::{NdviThresholds, VegetationHealth};
use serde::Deserialize;
use serde_json::json;
use shared::schemas::RecommendationPriority;

/// The application id all crop-health runs are attributed to.
pub const APP_ID: &str = "crop_health";

/// Default trend dead-band (NDVI delta within which a zone is "stable").
const DEFAULT_TREND_EPSILON: f32 = 0.02;

/// Request to run the crop-health application over per-zone NDVI statistics.
#[derive(Debug, Clone, Deserialize)]
pub struct CropHealthRunRequest {
    #[serde(default)]
    pub org_id: Option<String>,
    pub field_id: String,
    /// Per-zone NDVI stats. Each zone's `input_product_ids` must reference
    /// cataloged L2/L3 products.
    pub zones: Vec<ZoneHealthInput>,
    /// NDVI dead-band for trend classification. Defaults to 0.02.
    #[serde(default)]
    pub trend_epsilon: Option<f32>,
    /// Override for the no-vegetation NDVI floor. Defaults to `NdviThresholds`.
    #[serde(default)]
    pub no_vegetation_threshold: Option<f32>,
}

/// Run the crop-health application: compose findings, collect the union of the
/// zones' cataloged inputs, and record a governed run with per-finding lineage.
pub async fn run(
    pool: &DbPool,
    request: &CropHealthRunRequest,
    created_at: &str,
) -> Result<ApplicationRunRecord, ApplicationError> {
    let thresholds = NdviThresholds {
        no_vegetation: request
            .no_vegetation_threshold
            .unwrap_or_else(|| NdviThresholds::default().no_vegetation),
        ..NdviThresholds::default()
    };
    let epsilon = request.trend_epsilon.unwrap_or(DEFAULT_TREND_EPSILON);

    let findings = run_crop_health(&request.zones, &thresholds, epsilon);
    let application_findings: Vec<ApplicationFinding> = request
        .zones
        .iter()
        .zip(findings.iter())
        .map(|(zone, finding)| to_application_finding(finding, &zone.input_product_ids))
        .collect();

    // Run inputs = the deduplicated union of every zone's cataloged inputs;
    // record_run enforces the L2/L3 invariant and writes lineage against them.
    let mut input_product_ids: Vec<String> = request
        .zones
        .iter()
        .flat_map(|zone| zone.input_product_ids.iter().cloned())
        .collect();
    input_product_ids.sort();
    input_product_ids.dedup();

    let run_request = ApplicationRunRequest {
        org_id: request.org_id.clone(),
        field_id: request.field_id.clone(),
        input_product_ids,
        params: json!({
            "trend_epsilon": epsilon,
            "no_vegetation_threshold": thresholds.no_vegetation,
        }),
        findings: application_findings,
    };

    applications::record_run(pool, APP_ID, &run_request, created_at).await
}

/// Map a pure `CropHealthFinding` into a catalog `ApplicationFinding`. The kind
/// surfaces the actionable class (declining / unhealthy / healthy); the zone's
/// cataloged inputs become evidence refs and drive lineage.
fn to_application_finding(
    finding: &CropHealthFinding,
    input_product_ids: &[String],
) -> ApplicationFinding {
    ApplicationFinding {
        kind: finding_kind(finding).to_string(),
        severity: Some(severity_for(finding.priority).to_string()),
        confidence: None,
        zone_geometry: None,
        metrics: Some(json!({
            "zone_id": finding.zone_id,
            "health": finding.health,
            "trend": finding.trend,
            "mean_ndvi": finding.mean_ndvi,
            "ndvi_delta": finding.ndvi_delta,
            "area_m2": finding.area_m2,
        })),
        evidence_refs: input_product_ids.to_vec(),
    }
}

/// The actionable class for a zone: a declining trend takes precedence, then a
/// poor/critical health class, else the zone is healthy.
fn finding_kind(finding: &CropHealthFinding) -> &'static str {
    if finding.trend == TrendDirection::Declining {
        "declining_zone"
    } else if matches!(
        finding.health,
        VegetationHealth::Poor | VegetationHealth::Critical
    ) {
        "unhealthy_zone"
    } else {
        "healthy_zone"
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
