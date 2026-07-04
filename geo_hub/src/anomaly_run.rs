//! Anomaly-detection application run (Track B phase B5).
//!
//! Composes the pure `post_processor::anomaly_app` per-zone detector into a
//! governed application run: per-zone index values -> `ZoneAnomalyFinding`s ->
//! catalog `ApplicationFinding`s recorded via [`applications::record_run`], so
//! every finding traces back to the L2/L3 catalog products it derived from.
//! The `index_anomaly_zone` findings this emits are the input Track C
//! (`alert_evaluation`) screens into alerts. Mirrors [`crate::water_priority_run`].

use crate::applications::{
    self, ApplicationError, ApplicationFinding, ApplicationRunRecord, ApplicationRunRequest,
};
use crate::db::DbPool;
use post_processor::anomaly_app::{
    run_anomaly_detection, AnomalyConfig, ZoneAnomalyFinding, ZoneIndexInput,
};
use serde::Deserialize;
use serde_json::json;
use shared::schemas::RecommendationPriority;

/// The application id all anomaly runs are attributed to.
pub const APP_ID: &str = "anomaly_detection";

/// Request to run the anomaly-detection application over per-zone index values.
#[derive(Debug, Clone, Deserialize)]
pub struct AnomalyRunRequest {
    #[serde(default)]
    pub org_id: Option<String>,
    pub field_id: String,
    /// Per-zone index values. Each zone's `input_product_ids` must reference
    /// cataloged L2/L3 products.
    pub zones: Vec<ZoneIndexInput>,
    #[serde(default)]
    pub low_threshold: Option<f32>,
    #[serde(default)]
    pub high_threshold: Option<f32>,
    #[serde(default)]
    pub std_dev_multiplier: Option<f32>,
}

/// Run the anomaly-detection application: compose findings, collect the union of
/// the zones' cataloged inputs, and record a governed run with per-finding
/// lineage.
pub async fn run(
    pool: &DbPool,
    request: &AnomalyRunRequest,
    created_at: &str,
) -> Result<ApplicationRunRecord, ApplicationError> {
    let config = AnomalyConfig {
        low_threshold: request.low_threshold,
        high_threshold: request.high_threshold,
        std_dev_multiplier: request.std_dev_multiplier,
    };

    let findings = run_anomaly_detection(&request.zones, &config);
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
        params: json!({
            "low_threshold": request.low_threshold,
            "high_threshold": request.high_threshold,
            "std_dev_multiplier": request.std_dev_multiplier,
        }),
        findings: application_findings,
    };

    applications::record_run(pool, APP_ID, &run_request, created_at).await
}

/// Map a pure `ZoneAnomalyFinding` into a catalog `ApplicationFinding`. Anomalous
/// zones become `index_anomaly_zone` findings (the Track C alert input); nominal
/// zones are recorded as `nominal_zone` for completeness.
fn to_application_finding(
    finding: &ZoneAnomalyFinding,
    input_product_ids: &[String],
) -> ApplicationFinding {
    let kind = if finding.is_anomaly {
        "index_anomaly_zone"
    } else {
        "nominal_zone"
    };
    ApplicationFinding {
        kind: kind.to_string(),
        severity: Some(severity_for(finding.priority).to_string()),
        confidence: None,
        zone_geometry: None,
        metrics: Some(json!({
            "zone_id": finding.zone_id,
            "index_value": finding.index_value,
            "is_anomaly": finding.is_anomaly,
            "reason_code": finding.reason_code,
            "z_score": finding.z_score,
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
