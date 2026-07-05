//! Integration test for evidence-based alert severity (Track C phase C3): an
//! alert's severity is classified from the source finding's metrics, overriding
//! the static rule severity. A large water deficit escalates to emergency; a
//! marginal anomaly stays a warning; the classification is persisted + surfaced.

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
            tmp.path().join("severity.db").display()
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

fn enc(alert_id: &str) -> String {
    alert_id.replace(':', "%3A")
}

#[tokio::test]
async fn evidence_classifies_severity_overriding_rule() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2 = seed_l2(&pool).await?;

    // Water-priority run: a severe zone with a 35 mm deficit (>= emergency=30).
    let water = json!({
        "field_id": "field-1",
        "zones": [
            { "zone_id": "dry", "mean_soil_moisture": 0.09, "water_deficit_mm": 35.0,
              "area_m2": 15000.0, "input_product_ids": [l2] }
        ]
    });
    send(
        &app,
        "POST",
        "/api/applications/water-priority/runs",
        Some(water),
    )
    .await?;

    // Anomaly run: a single outlier among 5 zones has z ~= 2.0 (warning band).
    let anomaly = json!({
        "field_id": "field-1",
        "std_dev_multiplier": 1.5,
        "zones": [
            { "zone_id": "z1", "index_value": 0.50, "area_m2": 3000.0, "input_product_ids": [l2] },
            { "zone_id": "z2", "index_value": 0.51, "area_m2": 3000.0, "input_product_ids": [l2] },
            { "zone_id": "z3", "index_value": 0.49, "area_m2": 3000.0, "input_product_ids": [l2] },
            { "zone_id": "z4", "index_value": 0.52, "area_m2": 3000.0, "input_product_ids": [l2] },
            { "zone_id": "outlier", "index_value": 0.90, "area_m2": 15000.0, "input_product_ids": [l2] }
        ]
    });
    send(
        &app,
        "POST",
        "/api/applications/anomaly/runs",
        Some(anomaly),
    )
    .await?;

    // Evaluate: both findings fire alerts, each classified from its metrics.
    send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    let (_, alerts) = send(&app, "GET", "/api/fields/field-1/alerts", None).await?;
    let alerts = alerts.as_array().unwrap();

    let water_alert = alerts
        .iter()
        .find(|a| a["event_type"] == "water_deficit_zone")
        .expect("water deficit alert");
    // Rule severity is `warning`; the 35 mm deficit escalates it to emergency.
    assert_eq!(water_alert["severity"], "warning", "rule severity retained");
    assert_eq!(
        water_alert["classified_severity"], "emergency",
        "35mm deficit -> emergency"
    );

    let anomaly_alert = alerts
        .iter()
        .find(|a| a["event_type"] == "index_anomaly_zone")
        .expect("anomaly alert");
    // Rule severity is `critical`; a marginal z ~= 2.0 classifies as warning.
    assert_eq!(
        anomaly_alert["severity"], "critical",
        "rule severity retained"
    );
    assert_eq!(
        anomaly_alert["classified_severity"], "warning",
        "z~2.0 -> warning"
    );

    // The full classification is persisted + fetchable for the water alert.
    let water_id = enc(water_alert["alert_id"].as_str().unwrap());
    let (status, classification) = send(
        &app,
        "GET",
        &format!("/api/alerts/{water_id}/severity"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{classification}");
    assert_eq!(classification["metric"], "water_deficit_mm");
    assert_eq!(classification["classified_severity"], "emergency");
    assert_eq!(classification["hard_override_downstream"], true);
    assert_eq!(classification["observed_value"], 35.0);

    Ok(())
}

#[tokio::test]
async fn severity_of_unclassified_alert_is_not_found() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;
    let (status, _) = send(&app, "GET", "/api/alerts/nope/severity", None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}
