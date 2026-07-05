//! Integration test for the alert lifecycle (Track C phase C2): a fired alert
//! advances fired -> acknowledged -> resolved via governed transitions; illegal
//! ordering is rejected and repeated transitions are idempotent.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, server, HubConfig};
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
            tmp.path().join("lifecycle.db").display()
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

/// Fire an anomaly alert and return its alert id.
async fn fire_alert(app: &Router, l2_id: &str) -> Result<String> {
    let body = json!({
        "field_id": "field-1",
        "std_dev_multiplier": 1.5,
        "zones": [
            { "zone_id": "z1", "index_value": 0.50, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "z2", "index_value": 0.51, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "z3", "index_value": 0.49, "area_m2": 3000.0, "input_product_ids": [l2_id] },
            { "zone_id": "outlier", "index_value": 0.95, "area_m2": 15000.0, "input_product_ids": [l2_id] }
        ]
    });
    send(app, "POST", "/api/applications/anomaly/runs", Some(body)).await?;
    let (_, alerts) = send(app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    Ok(alerts[0]["alert_id"].as_str().unwrap().to_string())
}

fn enc(alert_id: &str) -> String {
    // The alert id contains ':' — percent-encode for the path segment.
    alert_id.replace(':', "%3A")
}

#[tokio::test]
async fn alert_advances_through_ack_and_resolve() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;
    let alert_id = fire_alert(&app, &l2_id).await?;
    let id = enc(&alert_id);

    // A fresh lifecycle opens at `fired`.
    let (status, life) = send(&app, "GET", &format!("/api/alerts/{id}/lifecycle"), None).await?;
    assert_eq!(status, StatusCode::OK, "{life}");
    assert_eq!(life["state"], "fired");

    // Acknowledge: fired -> acknowledged.
    let (status, action) = send(
        &app,
        "POST",
        &format!("/api/alerts/{id}/acknowledge"),
        Some(json!({ "actor_id": "agronomist-1" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{action}");
    assert_eq!(action["state"], "acknowledged");
    assert_eq!(action["idempotent"], false);

    // Resolve: acknowledged -> resolved.
    let (status, action) = send(
        &app,
        "POST",
        &format!("/api/alerts/{id}/resolve"),
        Some(json!({ "actor_id": "agronomist-1" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{action}");
    assert_eq!(action["state"], "resolved");

    // The persisted record reflects the resolved state + both transitions.
    let (_, life) = send(&app, "GET", &format!("/api/alerts/{id}/lifecycle"), None).await?;
    assert_eq!(life["state"], "resolved");
    assert_eq!(life["transitions"].as_array().unwrap().len(), 2);
    Ok(())
}

#[tokio::test]
async fn resolve_before_ack_is_rejected() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;
    let alert_id = fire_alert(&app, &l2_id).await?;
    let id = enc(&alert_id);

    // Resolving a still-`fired` alert is an illegal transition.
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/alerts/{id}/resolve"),
        Some(json!({ "actor_id": "agronomist-1" })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn repeated_acknowledge_is_idempotent() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;
    let alert_id = fire_alert(&app, &l2_id).await?;
    let id = enc(&alert_id);

    send(
        &app,
        "POST",
        &format!("/api/alerts/{id}/acknowledge"),
        Some(json!({ "actor_id": "agronomist-1" })),
    )
    .await?;
    let (status, action) = send(
        &app,
        "POST",
        &format!("/api/alerts/{id}/acknowledge"),
        Some(json!({ "actor_id": "agronomist-1" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(action["state"], "acknowledged");
    assert_eq!(action["idempotent"], true);

    // Only the first acknowledge is logged as a transition.
    let (_, life) = send(&app, "GET", &format!("/api/alerts/{id}/lifecycle"), None).await?;
    assert_eq!(life["transitions"].as_array().unwrap().len(), 1);
    Ok(())
}

#[tokio::test]
async fn lifecycle_of_unknown_alert_is_not_found() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;
    let (status, _) = send(
        &app,
        "POST",
        "/api/alerts/does-not-exist/acknowledge",
        Some(json!({ "actor_id": "a" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}
