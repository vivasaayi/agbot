//! End-to-end test of the water-balance watch loop (satellite pipeline
//! batch 42): a registered `water_balance` verdict (batch 41) becomes a
//! governed finding, which alert evaluation screens into a critical alert
//! with an evidence-graded severity and an auto-enqueued irrigation
//! proposal — closing the water detect -> warn -> propose chain.
//!
//! Fixture: one deficit-risk balance product (supply declining -2/3,
//! demand high). The watch emits a `water_balance_deficit_zone` finding;
//! alert evaluation with propose_action fires `water-balance-deficit-
//! critical` (graded emergency by the -0.6667 supply-decline rate) and
//! enqueues one irrigation proposal.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, server, HubConfig};
use serde_json::json;
use shared::product_graph::{ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

async fn ctx(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("wb_watch.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };
    Ok((server::build_router(state), pool))
}

/// Register a `water_balance` L3 verdict for field-1 (parameters carry the
/// summary the watch reads; no raster is needed).
async fn register_balance(
    pool: &db::DbPool,
    name: &str,
    parameters: serde_json::Value,
) -> Result<String> {
    let draft = ProductRecordDraft {
        level: ProductLevel::L3,
        kind: "water_balance".to_string(),
        algorithm_id: "water.balance_summary".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters,
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some(format!("balance-{name}")),
            temporal_start: "2026-01-01T00:00:00Z".to_string(),
            temporal_end: "2026-06-30T23:59:59Z".to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: None,
    };
    Ok(catalog::register_product(pool, &draft, "2026-07-07T00:00:00Z").await?)
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> Result<(StatusCode, serde_json::Value)> {
    let req = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&b)?))?,
        None => req.body(Body::empty())?,
    };
    let response = app.clone().oneshot(req).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024).await?;
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).to_string()));
    Ok((status, value))
}

#[tokio::test]
async fn deficit_verdict_fires_a_graded_alert_and_proposes_irrigation() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;

    let deficit = register_balance(
        &pool,
        "deficit",
        json!({
            "status": "deficit_risk",
            "status_reason": "supply_declining_demand_high",
            "supply_trend": "declining",
            "demand_level": "high",
            "relative_area_change": -2.0 / 3.0,
            "mean_et_fraction": 0.75,
            "total_precipitation_mm": 20.0,
        }),
    )
    .await?;
    // A second, healthy verdict must NOT fire an alert.
    register_balance(
        &pool,
        "adequate",
        json!({
            "status": "adequate",
            "status_reason": "no_stress_signal",
            "supply_trend": "stable",
            "demand_level": "low",
            "relative_area_change": 0.02,
        }),
    )
    .await?;

    // --- Watch run: verdicts become findings.
    let (status, record) = send(
        &app,
        "POST",
        "/api/applications/water-balance-watch/runs",
        Some(json!({
            "field_id": "field-1",
            "product_ids": [deficit],
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["app_id"], "water_balance_watch");
    assert_eq!(record["output_finding_ids"].as_array().unwrap().len(), 1);

    let (status, findings) = send(&app, "GET", "/api/fields/field-1/findings", None).await?;
    assert_eq!(status, StatusCode::OK, "{findings}");
    let deficit_finding = findings
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["finding"]["kind"] == "water_balance_deficit_zone")
        .expect("deficit finding");
    assert_eq!(deficit_finding["finding"]["severity"], "critical");
    assert_eq!(
        deficit_finding["finding"]["metrics"]["demand_level"],
        "high"
    );
    assert_eq!(
        deficit_finding["finding"]["evidence_refs"],
        json!([deficit])
    );

    // --- Alert evaluation with propose_action: critical alert + proposal.
    let (status, alerts) = send(
        &app,
        "POST",
        "/api/fields/field-1/alert-evaluation",
        Some(json!({ "propose_action": true })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{alerts}");
    let deficit_alerts: Vec<_> = alerts
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["event_type"] == "water_balance_deficit_zone")
        .collect();
    assert_eq!(
        deficit_alerts.len(),
        1,
        "exactly one deficit alert: {alerts}"
    );
    assert_eq!(deficit_alerts[0]["severity"], "critical");
    // The graded severity escalates by the supply-decline rate: -2/3 area
    // change -> 0.667 >= 0.50 emergency threshold.
    assert_eq!(deficit_alerts[0]["classified_severity"], "emergency");
    let alert_id = deficit_alerts[0]["alert_id"].as_str().unwrap();

    let (status, classification) = send(
        &app,
        "GET",
        &format!("/api/alerts/{alert_id}/severity"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{classification}");
    assert_eq!(classification["metric"], "supply_decline_rate");
    assert_eq!(classification["classified_severity"], "emergency");
    assert!(
        (classification["observed_value"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-6,
        "{classification}"
    );

    // An irrigation proposal is enqueued from the deficit finding.
    let (_, queue) = send(&app, "GET", "/api/fields/field-1/proposals", None).await?;
    let queue = queue.as_array().unwrap();
    assert_eq!(queue.len(), 1, "one irrigation proposal: {queue:?}");
    assert_eq!(queue[0]["status"], "proposed");
    assert_eq!(queue[0]["action_category"], "irrigation");

    // Re-evaluation is idempotent (no duplicate proposal).
    send(
        &app,
        "POST",
        "/api/fields/field-1/alert-evaluation",
        Some(json!({ "propose_action": true })),
    )
    .await?;
    let (_, again) = send(&app, "GET", "/api/fields/field-1/proposals", None).await?;
    assert_eq!(again.as_array().unwrap().len(), 1);

    // Wrong-kind input is refused.
    let (status, _) = send(
        &app,
        "POST",
        "/api/applications/water-balance-watch/runs",
        Some(json!({
            "field_id": "field-1",
            "product_ids": ["catalog:nope:000000000000"],
        })),
    )
    .await?;
    assert_ne!(status, StatusCode::OK);

    Ok(())
}
