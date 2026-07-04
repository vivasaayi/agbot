//! Integration test for the proposal queue (Track D phase D1): a pipeline
//! signal (alert / finding) is raised into a proposal that carries lineage back
//! to its source; accept/reject transitions are governed and idempotent.

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
            tmp.path().join("proposals.db").display()
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

fn enc(id: &str) -> String {
    id.replace(':', "%3A")
}

/// Fire an anomaly alert and return (alert_id, source_finding_id).
async fn fire_alert(app: &Router, l2: &str) -> Result<(String, String)> {
    let anomaly = json!({
        "field_id": "field-1",
        "std_dev_multiplier": 1.5,
        "zones": [
            { "zone_id": "z1", "index_value": 0.50, "area_m2": 3000.0, "input_product_ids": [l2] },
            { "zone_id": "z2", "index_value": 0.51, "area_m2": 3000.0, "input_product_ids": [l2] },
            { "zone_id": "z3", "index_value": 0.49, "area_m2": 3000.0, "input_product_ids": [l2] },
            { "zone_id": "outlier", "index_value": 0.95, "area_m2": 15000.0, "input_product_ids": [l2] }
        ]
    });
    send(app, "POST", "/api/applications/anomaly/runs", Some(anomaly)).await?;
    let (_, alerts) = send(app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    let alert = &alerts[0];
    Ok((
        alert["alert_id"].as_str().unwrap().to_string(),
        alert["source_finding_id"].as_str().unwrap().to_string(),
    ))
}

#[tokio::test]
async fn alert_raises_proposal_with_lineage_and_governed_accept() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2 = seed_l2(&pool).await?;
    let (alert_id, _finding_id) = fire_alert(&app, &l2).await?;

    // Raise a proposal from the alert.
    let (status, proposal) = send(
        &app,
        "POST",
        "/api/proposals",
        Some(json!({
            "source_kind": "alert",
            "source_id": alert_id,
            "field_id": "field-1",
            "title": "Scout anomalous zone",
            "action_category": "scouting",
            "priority": "critical",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{proposal}");
    assert_eq!(proposal["status"], "proposed");
    let proposal_id = proposal["proposal_id"].as_str().unwrap().to_string();

    // It shows up in the field's queue.
    let (_, queue) = send(&app, "GET", "/api/fields/field-1/proposals", None).await?;
    assert_eq!(queue.as_array().unwrap().len(), 1);

    // Lineage: proposal -> alert -> finding -> L2 -> L0, gap-free.
    let trace = provenance_store::trace_backward(&pool, &proposal_id).await?;
    assert!(
        trace.gaps.is_empty(),
        "proposal->L0 gap-free: {:?}",
        trace.gaps
    );
    assert!(trace.records.iter().any(|r| r.artifact_id == alert_id));
    assert!(trace.records.iter().any(|r| r.artifact_id == l2));

    // Creating a proposal for the same source is idempotent.
    let (_, again) = send(
        &app,
        "POST",
        "/api/proposals",
        Some(json!({
            "source_kind": "alert",
            "source_id": alert_id,
            "field_id": "field-1",
            "title": "duplicate attempt",
            "action_category": "scouting",
            "priority": "critical",
        })),
    )
    .await?;
    assert_eq!(again["proposal_id"], proposal_id);
    assert_eq!(again["status"], "proposed", "existing proposal unchanged");

    // Accept: proposed -> accepted, reviewer recorded.
    let pid = enc(&proposal_id);
    let (status, accepted) = send(
        &app,
        "POST",
        &format!("/api/proposals/{pid}/accept"),
        Some(json!({ "reviewer_id": "agronomist-1" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["reviewed_by"], "agronomist-1");

    // Flipping a decided proposal is rejected.
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/proposals/{pid}/reject"),
        Some(json!({ "reviewer_id": "agronomist-1" })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "cannot reject an accepted proposal"
    );
    Ok(())
}

#[tokio::test]
async fn proposal_from_finding_and_reject_flow() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2 = seed_l2(&pool).await?;
    let (_alert_id, finding_id) = fire_alert(&app, &l2).await?;

    let (status, proposal) = send(
        &app,
        "POST",
        "/api/proposals",
        Some(json!({
            "source_kind": "finding",
            "source_id": finding_id,
            "field_id": "field-1",
            "title": "Review declining zone",
            "action_category": "review",
            "priority": "high",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{proposal}");
    let pid = enc(proposal["proposal_id"].as_str().unwrap());

    let (status, rejected) = send(
        &app,
        "POST",
        &format!("/api/proposals/{pid}/reject"),
        Some(json!({ "reviewer_id": "agronomist-2" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{rejected}");
    assert_eq!(rejected["status"], "rejected");

    // Re-rejecting is idempotent.
    let (status, again) = send(
        &app,
        "POST",
        &format!("/api/proposals/{pid}/reject"),
        Some(json!({ "reviewer_id": "agronomist-2" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["status"], "rejected");

    let _ = pool;
    Ok(())
}

#[tokio::test]
async fn proposal_from_unknown_source_is_rejected() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;
    let (status, _) = send(
        &app,
        "POST",
        "/api/proposals",
        Some(json!({
            "source_kind": "alert",
            "source_id": "no-such-alert",
            "field_id": "field-1",
            "title": "x",
            "action_category": "scouting",
            "priority": "low",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unknown source rejected");
    Ok(())
}
