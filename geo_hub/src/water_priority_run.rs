//! Water-priority application run (Track B phase B4).
//!
//! Composes the pure `post_processor::water_priority_app` analysis into a
//! governed application run: per-zone soil-moisture / deficit stats ->
//! `WaterPriorityFinding`s -> catalog `ApplicationFinding`s recorded via
//! [`applications::record_run`], so every finding traces back to the L2/L3
//! catalog products it derived from. Mirrors [`crate::crop_health_run`].

use crate::applications::{
    self, ApplicationError, ApplicationFinding, ApplicationRunRecord, ApplicationRunRequest,
};
use crate::db::DbPool;
use post_processor::water_priority_app::{
    run_water_priority, WaterPriorityFinding, WaterStressThresholds, ZoneWaterInput,
};
use serde::Deserialize;
use serde_json::json;
use shared::schemas::RecommendationPriority;

/// The application id all water-priority runs are attributed to.
pub const APP_ID: &str = "water_priority";

/// Default water deficit (mm) above which a zone is flagged for irrigation.
const DEFAULT_DEFICIT_THRESHOLD_MM: f32 = 5.0;

/// Request to run the water-priority application over per-zone soil-moisture
/// statistics.
#[derive(Debug, Clone, Deserialize)]
pub struct WaterPriorityRunRequest {
    #[serde(default)]
    pub org_id: Option<String>,
    pub field_id: String,
    /// Per-zone soil-moisture / deficit stats. Each zone's `input_product_ids`
    /// must reference cataloged L2/L3 products.
    pub zones: Vec<ZoneWaterInput>,
    /// Deficit (mm) above which a zone needs irrigation. Defaults to 5.0.
    #[serde(default)]
    pub deficit_threshold_mm: Option<f32>,
}

/// Run the water-priority application: compose findings, collect the union of
/// the zones' cataloged inputs, and record a governed run with per-finding
/// lineage.
pub async fn run(
    pool: &DbPool,
    request: &WaterPriorityRunRequest,
    created_at: &str,
) -> Result<ApplicationRunRecord, ApplicationError> {
    let thresholds = WaterStressThresholds::default();
    let deficit_threshold_mm = request
        .deficit_threshold_mm
        .unwrap_or(DEFAULT_DEFICIT_THRESHOLD_MM);

    let findings = run_water_priority(&request.zones, &thresholds, deficit_threshold_mm);
    let application_findings: Vec<ApplicationFinding> = request
        .zones
        .iter()
        .zip(findings.iter())
        .map(|(zone, finding)| to_application_finding(finding, &zone.input_product_ids))
        .collect();

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
        params: json!({ "deficit_threshold_mm": deficit_threshold_mm }),
        findings: application_findings,
    };

    applications::record_run(pool, APP_ID, &run_request, created_at).await
}

/// Map a pure `WaterPriorityFinding` into a catalog `ApplicationFinding`. The
/// kind surfaces whether the zone needs water; the zone's cataloged inputs
/// become evidence refs and drive lineage.
fn to_application_finding(
    finding: &WaterPriorityFinding,
    input_product_ids: &[String],
) -> ApplicationFinding {
    let kind = if finding.needs_irrigation {
        "water_deficit_zone"
    } else {
        "adequate_moisture_zone"
    };
    ApplicationFinding {
        kind: kind.to_string(),
        severity: Some(severity_for(finding.priority).to_string()),
        confidence: None,
        zone_geometry: None,
        metrics: Some(json!({
            "zone_id": finding.zone_id,
            "stress": finding.stress,
            "needs_irrigation": finding.needs_irrigation,
            "mean_soil_moisture": finding.mean_soil_moisture,
            "water_deficit_mm": finding.water_deficit_mm,
            "area_m2": finding.area_m2,
        })),
        evidence_refs: input_product_ids.to_vec(),
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
