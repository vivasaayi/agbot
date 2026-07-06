//! Integration test for the drought-watch application run (satellite
//! pipeline batch 36): registered drought_index L3 rasters -> stress
//! findings with lineage -> a Track C alert fires on the stressed product.
//!
//! Fixture: two 2x2 VCI rasters registered for field-1. The stressed one
//! is [5, 25, 45, 80] (stressed fraction 0.5 -> warning finding); the
//! healthy one is [45, 50, 60, 80] (nominal). Alert evaluation fires
//! exactly one drought-stress warning.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u32 = 32643;
const TRANSFORM: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const NODATA: f32 = -9999.0;

async fn ctx(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("drought_watch.db").display()
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

async fn register_drought(
    pool: &db::DbPool,
    tmp: &TempDir,
    name: &str,
    values: Vec<f32>,
) -> Result<String> {
    register_product(
        pool,
        tmp,
        name,
        "drought_index",
        json!({ "index_kind": "vci" }),
        values,
    )
    .await
}

async fn register_product(
    pool: &db::DbPool,
    tmp: &TempDir,
    name: &str,
    kind: &str,
    parameters: serde_json::Value,
    values: Vec<f32>,
) -> Result<String> {
    let path = tmp.path().join(format!("{name}.tif"));
    write_geotiff_f32(
        &path,
        2,
        2,
        &values,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let mut parameters = parameters;
    parameters["fixture"] = json!(name);
    let draft = ProductRecordDraft {
        level: ProductLevel::L3,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.fixture"),
        algorithm_version: "1.0.0".to_string(),
        parameters,
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026-kharif".to_string()),
            scene_id: None,
            temporal_start: "2026-06-01T00:00:00Z".to_string(),
            temporal_end: "2026-06-30T23:59:59Z".to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
        artifact: Some(ProductArtifact {
            format: "tif".to_string(),
            path: path.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: None,
    };
    Ok(catalog::register_product(pool, &draft, "2026-07-05T00:00:00Z").await?)
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
async fn drought_rasters_become_findings_and_fire_an_alert() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;

    let stressed =
        register_drought(&pool, &tmp, "vci_stressed", vec![5.0, 25.0, 45.0, 80.0]).await?;
    let healthy =
        register_drought(&pool, &tmp, "vci_healthy", vec![45.0, 50.0, 60.0, 80.0]).await?;
    // SPI z-scores (McKee scale): [-2.5, -1.2, 0, 1] -> stressed 0.5.
    let spi = register_product(
        &pool,
        &tmp,
        "spi_dry",
        "spi",
        json!({ "window_months": 3 }),
        vec![-2.5, -1.2, 0.0, 1.0],
    )
    .await?;

    let (status, record) = send(
        &app,
        "POST",
        "/api/applications/drought-watch/runs",
        Some(json!({
            "field_id": "field-1",
            "product_ids": [stressed, healthy, spi],
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{record}");
    assert_eq!(record["app_id"], "drought_watch");
    assert_eq!(record["output_finding_ids"].as_array().unwrap().len(), 3);

    // Findings surface on the field with kinds, metrics, and lineage.
    let (status, findings) = send(&app, "GET", "/api/fields/field-1/findings", None).await?;
    assert_eq!(status, StatusCode::OK, "{findings}");
    let findings = findings.as_array().unwrap();
    assert_eq!(findings.len(), 3);

    // The SPI product classifies on the McKee z-score scale, not percent.
    let spi_finding = findings
        .iter()
        .find(|f| f["finding"]["metrics"]["index_kind"] == "spi")
        .expect("spi finding");
    assert_eq!(spi_finding["finding"]["kind"], "drought_stress_zone");
    assert!(
        (spi_finding["finding"]["metrics"]["stressed_fraction"]
            .as_f64()
            .unwrap()
            - 0.5)
            .abs()
            < 1e-6
    );
    assert_eq!(spi_finding["finding"]["evidence_refs"], json!([spi]));

    // Stressed VCI: stressed fraction 0.5 -> warning-priority drought zone.
    let drought = findings
        .iter()
        .find(|f| {
            f["finding"]["kind"] == "drought_stress_zone"
                && f["finding"]["metrics"]["index_kind"] == "vci"
        })
        .expect("drought finding");
    assert_eq!(drought["finding"]["severity"], "high");
    let metrics = &drought["finding"]["metrics"];
    assert!((metrics["stressed_fraction"].as_f64().unwrap() - 0.5).abs() < 1e-6);
    assert_eq!(metrics["reason_code"], "stressed_fraction_warning");
    assert_eq!(drought["finding"]["evidence_refs"], json!([stressed]));

    let nominal = findings
        .iter()
        .find(|f| f["finding"]["kind"] == "nominal_zone")
        .expect("nominal finding");
    assert_eq!(nominal["finding"]["evidence_refs"], json!([healthy]));

    // Track C: the default ruleset fires a warning on the stress finding.
    let (status, alerts) = send(&app, "POST", "/api/fields/field-1/alert-evaluation", None).await?;
    assert_eq!(status, StatusCode::OK, "{alerts}");
    let fired = alerts.as_array().unwrap();
    let drought_alerts: Vec<_> = fired
        .iter()
        .filter(|a| a["event_type"] == "drought_stress_zone")
        .collect();
    assert_eq!(drought_alerts.len(), 2, "vci + spi stress alerts: {alerts}");
    assert_eq!(drought_alerts[0]["severity"], "warning");

    // A non-drought product id is refused with a reason.
    let (status, body) = send(
        &app,
        "POST",
        "/api/applications/drought-watch/runs",
        Some(json!({
            "field_id": "field-1",
            "product_ids": ["catalog:nope:000000000000"],
        })),
    )
    .await?;
    assert_ne!(status, StatusCode::OK, "{body}");
    Ok(())
}
