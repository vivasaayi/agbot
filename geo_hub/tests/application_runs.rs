//! Integration tests for application runs (Track B phase B1): runs consume
//! cataloged L2/L3 products, emit findings with provenance, and reject raw/
//! unknown inputs.

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
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("apps.db").display()),
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

/// Register an L0->L1->L2 chain and return the L2 product id.
async fn seed_l2(pool: &db::DbPool) -> Result<String> {
    let l0 = draft(ProductLevel::L0, "raw_capture", vec![]);
    let l0_id = catalog::register_product(pool, &l0, T0).await?;
    let l1 = draft(
        ProductLevel::L1,
        "band_nir",
        vec![ProductInputRef {
            product_id: l0_id,
            role: "raw".into(),
        }],
    );
    let l1_id = catalog::register_product(pool, &l1, T0).await?;
    let l2 = draft(
        ProductLevel::L2,
        "ndvi",
        vec![ProductInputRef {
            product_id: l1_id,
            role: "band:nir".into(),
        }],
    );
    Ok(catalog::register_product(pool, &l2, T0).await?)
}

#[tokio::test]
async fn run_emits_findings_with_lineage_to_l2() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2(&pool).await?;

    let body = json!({
        "field_id": "field-1",
        "input_product_ids": [l2_id],
        "params": { "threshold": 0.3 },
        "findings": [
            { "kind": "declining_zone", "severity": "high", "confidence": 0.82,
              "metrics": { "ndvi_drop": 0.15 }, "evidence_refs": [] }
        ]
    });
    let (status, run) = send(
        &app,
        "POST",
        "/api/applications/crop_health/runs",
        Some(body),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{run}");
    let run_id = run["run_id"].as_str().unwrap().to_string();
    assert_eq!(run["output_finding_ids"].as_array().unwrap().len(), 1);

    // The finding is listed for the field.
    let (_, findings) = send(&app, "GET", "/api/fields/field-1/findings", None).await?;
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["finding"]["kind"], "declining_zone");
    let finding_id = findings[0]["finding_id"].as_str().unwrap();

    // The finding traces back to the L2 input (and thus to L0).
    let trace = provenance_store::trace_backward(&pool, finding_id).await?;
    assert!(
        trace.gaps.is_empty(),
        "finding->L0 gap-free: {:?}",
        trace.gaps
    );
    assert!(trace.records.iter().any(|r| r.artifact_id == l2_id));

    // The run is fetchable.
    let (rs, one) = send(
        &app,
        "GET",
        &format!("/api/application-runs/{run_id}"),
        None,
    )
    .await?;
    assert_eq!(rs, StatusCode::OK);
    assert_eq!(one["app_id"], "crop_health");
    Ok(())
}

#[tokio::test]
async fn run_rejects_unknown_input() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;
    let body = json!({
        "field_id": "field-1",
        "input_product_ids": ["does-not-exist"],
        "params": {},
        "findings": []
    });
    let (status, _) = send(
        &app,
        "POST",
        "/api/applications/crop_health/runs",
        Some(body),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unknown input rejected");
    Ok(())
}

#[tokio::test]
async fn run_rejects_l0_input() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l0_id =
        catalog::register_product(&pool, &draft(ProductLevel::L0, "raw_capture", vec![]), T0)
            .await?;
    let body = json!({
        "field_id": "field-1",
        "input_product_ids": [l0_id],
        "params": {},
        "findings": []
    });
    let (status, _) = send(
        &app,
        "POST",
        "/api/applications/crop_health/runs",
        Some(body),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "applications only consume L2/L3"
    );
    Ok(())
}
