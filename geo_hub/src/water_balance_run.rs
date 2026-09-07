//! Water-balance watch application run (satellite pipeline batch 42).
//!
//! Closes the water loop on the Track C rails: registered `water_balance`
//! L3 verdicts (batch 41) become governed application findings, which
//! alert evaluation screens into early warnings and irrigation proposals —
//! the same detect -> warn -> propose chain drought watch has.
//!
//! No new engine: the balance summary IS the evaluation. This run maps
//! each verdict's identity-bearing parameters to a finding:
//!
//! - `deficit_risk`  -> `water_balance_deficit_zone` (critical)
//! - `watch`         -> `water_balance_watch_zone` (medium)
//! - `adequate` / `surplus` -> `nominal_zone` (low)
//! - `insufficient_evidence` -> `insufficient_coverage_zone` (low)
//!
//! Mirrors [`crate::drought_watch_run`].

use serde::Deserialize;
use serde_json::json;

use crate::applications::{
    self, ApplicationError, ApplicationFinding, ApplicationRunRecord, ApplicationRunRequest,
};
use crate::catalog;
use crate::db::DbPool;

/// The application id all water-balance watch runs are attributed to.
pub const APP_ID: &str = "water_balance_watch";

/// Request to run the watch over registered balance verdicts.
#[derive(Debug, Clone, Deserialize)]
pub struct WaterBalanceRunRequest {
    #[serde(default)]
    pub org_id: Option<String>,
    pub field_id: String,
    /// Catalog ids of registered `water_balance` L3 products.
    pub product_ids: Vec<String>,
}

/// Map one verdict's parameters to (finding kind, severity).
fn finding_kind(status: &str) -> (&'static str, &'static str) {
    match status {
        "deficit_risk" => ("water_balance_deficit_zone", "critical"),
        "watch" => ("water_balance_watch_zone", "medium"),
        "insufficient_evidence" => ("insufficient_coverage_zone", "low"),
        _ => ("nominal_zone", "low"),
    }
}

/// Run the watch: read each registered verdict, record a governed run with
/// per-finding lineage to the balance product (and through it to every
/// extent/ET/precipitation input).
pub async fn run(
    pool: &DbPool,
    request: &WaterBalanceRunRequest,
    created_at: &str,
) -> Result<ApplicationRunRecord, ApplicationError> {
    let mut findings = Vec::new();
    for product_id in &request.product_ids {
        let product = catalog::get_product(pool, product_id)
            .await?
            .ok_or_else(|| ApplicationError::InputNotFound(product_id.clone()))?;
        if product.kind != "water_balance" {
            return Err(ApplicationError::InputNotL2OrL3 {
                product_id: product_id.clone(),
                level: format!(
                    "kind {} (water_balance_watch consumes water_balance L3s)",
                    product.kind
                ),
            });
        }
        let status = product
            .parameters
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("insufficient_evidence")
            .to_string();
        let (kind, severity) = finding_kind(&status);
        findings.push(ApplicationFinding {
            kind: kind.to_string(),
            severity: Some(severity.to_string()),
            confidence: None,
            zone_geometry: None,
            metrics: Some(json!({
                "status": status,
                "status_reason": product.parameters.get("status_reason"),
                "supply_trend": product.parameters.get("supply_trend"),
                "demand_level": product.parameters.get("demand_level"),
                "relative_area_change": product.parameters.get("relative_area_change"),
                "mean_et_fraction": product.parameters.get("mean_et_fraction"),
                "total_precipitation_mm": product.parameters.get("total_precipitation_mm"),
            })),
            evidence_refs: vec![product.product_id.clone()],
        });
    }

    let mut input_product_ids = request.product_ids.clone();
    input_product_ids.sort();
    input_product_ids.dedup();

    applications::record_run(
        pool,
        APP_ID,
        &ApplicationRunRequest {
            org_id: request.org_id.clone(),
            field_id: request.field_id.clone(),
            input_product_ids,
            params: json!({}),
            findings,
        },
        created_at,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_to_finding_mapping_is_pinned() {
        assert_eq!(
            finding_kind("deficit_risk"),
            ("water_balance_deficit_zone", "critical")
        );
        assert_eq!(
            finding_kind("watch"),
            ("water_balance_watch_zone", "medium")
        );
        assert_eq!(finding_kind("adequate"), ("nominal_zone", "low"));
        assert_eq!(finding_kind("surplus"), ("nominal_zone", "low"));
        assert_eq!(
            finding_kind("insufficient_evidence"),
            ("insufficient_coverage_zone", "low")
        );
        // Unknown statuses degrade to a non-alerting nominal finding.
        assert_eq!(finding_kind("garbage"), ("nominal_zone", "low"));
    }
}
