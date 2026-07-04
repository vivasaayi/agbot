//! Integration test for the crop-health application run (Track B phase B3):
//! two scenes' NDVI zone stats compose into findings that surface a declining
//! zone, recorded with lineage back to each scene's cataloged L2 product.

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
            tmp.path().join("crop_health.db").display()
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

fn draft(
    scene_id: &str,
    level: ProductLevel,
    kind: &str,
    inputs: Vec<ProductInputRef>,
) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.compute"),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "scene_id": scene_id }),
        inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some(scene_id.to_string()),
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

/// Register an L0->L1->L2 NDVI chain for one scene and return the L2 id.
async fn seed_scene_ndvi(pool: &db::DbPool, scene_id: &str) -> Result<String> {
    let l0 = catalog::register_product(
        pool,
        &draft(scene_id, ProductLevel::L0, "raw_capture", vec![]),
        T0,
    )
    .await?;
    let l1 = catalog::register_product(
        pool,
        &draft(
            scene_id,
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
            scene_id,
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
async fn two_scenes_surface_declining_zone_with_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;

    // Two epochs, each a cataloged L2 NDVI product.
    let scene_1_ndvi = seed_scene_ndvi(&pool, "scene-1").await?;
    let scene_2_ndvi = seed_scene_ndvi(&pool, "scene-2").await?;

    // Zone A improved; zone B declined into a critical NDVI band. The zone stats
    // reference both epochs' L2 products as evidence.
    let body = json!({
        "field_id": "field-1",
        "trend_epsilon": 0.02,
        "zones": [
            {
                "zone_id": "zone-a",
                "mean_ndvi": 0.72,
                "ndvi_delta": 0.05,
                "area_m2": 4000.0,
                "input_product_ids": [scene_1_ndvi, scene_2_ndvi],
            },
            {
                "zone_id": "zone-b",
                "mean_ndvi": 0.28,
                "ndvi_delta": -0.12,
                "area_m2": 15000.0,
                "input_product_ids": [scene_1_ndvi, scene_2_ndvi],
            }
        ]
    });

    let (status, run) = send(
        &app,
        "POST",
        "/api/applications/crop-health/runs",
        Some(body),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["app_id"], "crop_health");
    assert_eq!(run["output_finding_ids"].as_array().unwrap().len(), 2);

    // The field's findings include the declining zone, flagged critical.
    let (_, findings) = send(&app, "GET", "/api/fields/field-1/findings", None).await?;
    let findings = findings.as_array().unwrap();
    assert_eq!(findings.len(), 2);
    let declining = findings
        .iter()
        .find(|f| f["finding"]["kind"] == "declining_zone")
        .expect("a declining_zone finding must be surfaced");
    assert_eq!(declining["finding"]["severity"], "critical");
    assert_eq!(declining["finding"]["metrics"]["zone_id"], "zone-b");

    // Lineage: the declining finding traces gap-free back to both scenes' L2.
    let finding_id = declining["finding_id"].as_str().unwrap();
    let trace = provenance_store::trace_backward(&pool, finding_id).await?;
    assert!(
        trace.gaps.is_empty(),
        "finding->L0 gap-free: {:?}",
        trace.gaps
    );
    assert!(trace.records.iter().any(|r| r.artifact_id == scene_1_ndvi));
    assert!(trace.records.iter().any(|r| r.artifact_id == scene_2_ndvi));

    Ok(())
}

#[tokio::test]
async fn crop_health_run_rejects_uncataloged_zone_inputs() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;
    let body = json!({
        "field_id": "field-1",
        "zones": [
            {
                "zone_id": "zone-a",
                "mean_ndvi": 0.5,
                "ndvi_delta": 0.0,
                "area_m2": 1000.0,
                "input_product_ids": ["not-in-catalog"],
            }
        ]
    });
    let (status, _) = send(
        &app,
        "POST",
        "/api/applications/crop-health/runs",
        Some(body),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "crop-health inputs must be cataloged L2/L3"
    );
    Ok(())
}
