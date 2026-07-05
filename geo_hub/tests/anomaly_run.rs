//! Integration test for the anomaly-detection application run (Track B phase B5):
//! per-zone index values compose into findings that surface an outlier zone as
//! an `index_anomaly_zone` (the Track C alert input), recorded with lineage back
//! to the zone's cataloged L2 product.

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
            tmp.path().join("anomaly.db").display()
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

/// Register an L0->L1->L2 NDVI chain and return the L2 id.
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

#[tokio::test]
async fn outlier_zone_surfaces_index_anomaly_with_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;

    // Four clustered zones + one high outlier; statistical band flags the outlier.
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

    let (status, run) = send(&app, "POST", "/api/applications/anomaly/runs", Some(body)).await?;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["app_id"], "anomaly_detection");
    assert_eq!(run["output_finding_ids"].as_array().unwrap().len(), 5);

    let (_, findings) = send(&app, "GET", "/api/fields/field-1/findings", None).await?;
    let findings = findings.as_array().unwrap();
    let anomalies: Vec<_> = findings
        .iter()
        .filter(|f| f["finding"]["kind"] == "index_anomaly_zone")
        .collect();
    assert_eq!(anomalies.len(), 1, "exactly one outlier zone is anomalous");
    let anomaly = anomalies[0];
    assert_eq!(anomaly["finding"]["metrics"]["zone_id"], "outlier");
    assert_eq!(
        anomaly["finding"]["metrics"]["reason_code"],
        "above_statistical_band"
    );
    assert_eq!(anomaly["finding"]["severity"], "critical");

    // Lineage: the anomaly finding traces gap-free back to the L2 product.
    let finding_id = anomaly["finding_id"].as_str().unwrap();
    let trace = provenance_store::trace_backward(&pool, finding_id).await?;
    assert!(
        trace.gaps.is_empty(),
        "finding->L0 gap-free: {:?}",
        trace.gaps
    );
    assert!(trace.records.iter().any(|r| r.artifact_id == l2_id));

    Ok(())
}

#[tokio::test]
async fn anomaly_run_rejects_uncataloged_inputs() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;
    let body = json!({
        "field_id": "field-1",
        "low_threshold": 0.2,
        "zones": [
            { "zone_id": "z1", "index_value": 0.1, "area_m2": 1000.0, "input_product_ids": ["not-in-catalog"] }
        ]
    });
    let (status, _) = send(&app, "POST", "/api/applications/anomaly/runs", Some(body)).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "anomaly inputs must be cataloged L2/L3"
    );
    Ok(())
}
