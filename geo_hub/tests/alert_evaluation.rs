//! Integration test for alert evaluation (Track C phase C1): application
//! findings (anomaly / water deficit) are screened into alerts by a rule set,
//! persisted with lineage back to the source finding, so a backward trace
//! closes alert -> finding -> L2 -> L0 gap-free. Evaluation is idempotent.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, provenance_store, server, HubConfig};
use serde_json::json;
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const T0: &str = "2026-06-01T00:00:00Z";

async fn ctx(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("alerts.db").display()
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

fn draft(level: ProductLevel, kind: &str, inputs: Vec<ProductInputRef>) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.compute"),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "scene_id": "scene-1" }),
        inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some("scene-1".to_string()),
            temporal_start: T0.to_string(),
            temporal_end: T0.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some("landsat-9".to_string()),
    }
}

async fn seed_l2(pool: &db::DbPool) -> Result<String> {
    let l0 = catalog::register_product(pool, &draft(ProductLevel::L0, "raw_capture", vec![]), T0)
        .await?;
    let l1 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L1,
            "band_nir",
            vec![ProductInputRef {
                product_id: l0,
                role: "raw".into(),
            }],
        ),
        T0,
    )
    .await?;
    let l2 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L2,
            "ndvi",
            vec![ProductInputRef {
                product_id: l1,
                role: "band:nir".into(),
            }],
        ),
        T0,
    )
    .await?;
    Ok(l2)
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
    let bytes = to_bytes(response.into_body(), 256 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    ))
}

/// Run the anomaly application so an `index_anomaly_zone` finding exists.
async fn run_anomaly(app: &Router, l2_id: &str) -> Result<()> {
    let body = json!({
        "field_id": "field-1",
        "std_dev_multiplier": 1.5,
        "zones": [
            { "zone_id": "z1", "index_value": 0.50, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "z2", "index_value": 0.51, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "z3", "index_value": 0.49, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "z4", "index_value": 0.52, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "outlier", "index_value": 0.95, "area_m2": 15000.0, "input_product_ids": [l2_id] }
        ]
    });
    let (status, _) = send(app, "POST", "/api/applications/anomaly/runs", Some(body)).await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn anomaly_finding_fires_alert_with_lineage_to_source() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;
    run_anomaly(&app, &l2_id).await?;

    // Evaluate the default rule set: the anomaly finding fires a critical alert.
    let (status, alerts) = send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    assert_eq!(status, StatusCode::OK, "{alerts}");
    let alerts = alerts.as_array().unwrap();
    assert_eq!(alerts.len(), 1, "exactly one anomaly alert fires");
    let alert = &alerts[0];
    assert_eq!(alert["event_type"], "index_anomaly_zone");
    assert_eq!(alert["severity"], "critical");
    assert_eq!(alert["subject_ref"], "field:field-1");
    let alert_id = alert["alert_id"].as_str().unwrap();
    let source_finding_id = alert["source_finding_id"].as_str().unwrap();

    // Lineage: alert -> finding -> L2 -> L0, gap-free.
    let trace = provenance_store::trace_backward(&pool, alert_id).await?;
    assert!(
        trace.gaps.is_empty(),
        "alert->L0 gap-free: {:?}",
        trace.gaps
    );
    assert!(trace
        .records
        .iter()
        .any(|r| r.artifact_id == source_finding_id));
    assert!(trace.records.iter().any(|r| r.artifact_id == l2_id));

    // The alert is listed for the field.
    let (_, listed) = send(&app, "GET", "/api/fields/field-1/alerts", None).await?;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["alert_id"], alert_id);

    Ok(())
}

#[tokio::test]
async fn re_evaluation_is_idempotent() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;
    run_anomaly(&app, &l2_id).await?;

    send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;

    let (_, listed) = send(&app, "GET", "/api/fields/field-1/alerts", None).await?;
    assert_eq!(
        listed.as_array().unwrap().len(),
        1,
        "re-evaluation must not duplicate the alert"
    );
    // Lineage stays single-edged too.
    let alert_id = listed[0]["alert_id"].as_str().unwrap();
    let trace = provenance_store::trace_backward(&pool, alert_id).await?;
    let alert_records = trace
        .records
        .iter()
        .filter(|r| r.artifact_id == alert_id)
        .count();
    assert_eq!(alert_records, 1, "one lineage record for the alert");
    Ok(())
}

#[tokio::test]
async fn nominal_findings_do_not_fire_alerts() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;

    // A uniform anomaly run yields only nominal_zone findings (no rule matches).
    let body = json!({
        "field_id": "field-1",
        "zones": [
            { "zone_id": "z1", "index_value": 0.50, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "z2", "index_value": 0.50, "area_m2": 3000.0, "input_product_ids": [l2_id] }
        ]
    });
    send(&app, "POST", "/api/applications/anomaly/runs", Some(body)).await?;

    let (status, alerts) = send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        alerts.as_array().unwrap().len(),
        0,
        "nominal zones fire no alerts"
    );
    Ok(())
}

/// propose_action (Track C phase C3): firing an alert on an actionable finding
/// enqueues a Proposed proposal from that finding, traceable to it. Idempotent.
#[tokio::test]
async fn actionable_alert_enqueues_a_proposal_from_its_finding() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;
    run_anomaly(&app, &l2_id).await?;

    // No proposals before evaluation.
    let (_, before) = send(&app, "GET", "/api/fields/field-1/proposals", None).await?;
    assert_eq!(before.as_array().unwrap().len(), 0);

    // Evaluate: the anomaly alert fires and auto-proposes.
    let (status, alerts) = send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    assert_eq!(status, StatusCode::OK, "{alerts}");
    let source_finding_id = alerts[0]["source_finding_id"].as_str().unwrap().to_string();

    // A Proposed proposal now sits in the field's queue, sourced from the finding.
    let (_, queue) = send(&app, "GET", "/api/fields/field-1/proposals", None).await?;
    let queue = queue.as_array().unwrap();
    assert_eq!(queue.len(), 1, "one auto-proposal from the anomaly finding");
    assert_eq!(queue[0]["status"], "proposed");
    assert_eq!(queue[0]["source_id"], json!(source_finding_id));
    assert_eq!(queue[0]["action_category"], "scout");

    // Re-evaluation does not duplicate the proposal (idempotent per finding).
    send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    let (_, again) = send(&app, "GET", "/api/fields/field-1/proposals", None).await?;
    assert_eq!(again.as_array().unwrap().len(), 1, "still exactly one proposal");
    Ok(())
}
