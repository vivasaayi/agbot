//! Integration test for the water-priority application run (Track B phase B4):
//! per-zone soil-moisture stats compose into findings that surface a water
//! deficit, recorded with lineage back to the zone's cataloged L2 product.

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
            tmp.path().join("water.db").display()
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
        source_id: Some("sentinel-2".to_string()),
    }
}

/// Register an L0->L1->L2 soil-moisture chain and return the L2 id.
async fn seed_l2_soil_moisture(pool: &db::DbPool) -> Result<String> {
    let l0 = catalog::register_product(pool, &draft(ProductLevel::L0, "raw_capture", vec![]), T0)
        .await?;
    let l1 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L1,
            "band_swir",
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
            "soil_moisture",
            vec![ProductInputRef {
                product_id: l1,
                role: "band:swir".into(),
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
async fn dry_zone_surfaces_water_deficit_with_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let l2_id = seed_l2_soil_moisture(&pool).await?;

    // Zone A adequate; zone B severely dry with a large deficit over a big area.
    let body = json!({
        "field_id": "field-1",
        "zones": [
            {
                "zone_id": "zone-a",
                "mean_soil_moisture": 0.33,
                "water_deficit_mm": 1.0,
                "area_m2": 3000.0,
                "input_product_ids": [l2_id],
            },
            {
                "zone_id": "zone-b",
                "mean_soil_moisture": 0.09,
                "water_deficit_mm": 18.0,
                "area_m2": 15000.0,
                "input_product_ids": [l2_id],
            }
        ]
    });

    let (status, run) = send(
        &app,
        "POST",
        "/api/applications/water-priority/runs",
        Some(body),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["app_id"], "water_priority");
    assert_eq!(run["output_finding_ids"].as_array().unwrap().len(), 2);

    let (_, findings) = send(&app, "GET", "/api/fields/field-1/findings", None).await?;
    let findings = findings.as_array().unwrap();
    assert_eq!(findings.len(), 2);
    let deficit = findings
        .iter()
        .find(|f| f["finding"]["kind"] == "water_deficit_zone")
        .expect("a water_deficit_zone finding must be surfaced");
    assert_eq!(deficit["finding"]["severity"], "critical");
    assert_eq!(deficit["finding"]["metrics"]["zone_id"], "zone-b");
    assert_eq!(deficit["finding"]["metrics"]["stress"], "severe");

    // Lineage: the deficit finding traces gap-free back to the L2 product.
    let finding_id = deficit["finding_id"].as_str().unwrap();
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
async fn water_priority_run_rejects_uncataloged_inputs() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;
    let body = json!({
        "field_id": "field-1",
        "zones": [
            {
                "zone_id": "zone-a",
                "mean_soil_moisture": 0.1,
                "water_deficit_mm": 10.0,
                "area_m2": 1000.0,
                "input_product_ids": ["not-in-catalog"],
            }
        ]
    });
    let (status, _) = send(
        &app,
        "POST",
        "/api/applications/water-priority/runs",
        Some(body),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "water-priority inputs must be cataloged L2/L3"
    );
    Ok(())
}
