//! End-to-end test of the water-balance summary (satellite pipeline
//! batch 41): a field's registered water_extent series (supply),
//! et_fraction rasters (demand), and CHIRPS precipitation fold into a
//! reason-coded `water_balance` L3 with a JSON artifact and lineage to
//! every contributing product.
//!
//! Fixture: extents shrink 300 -> 100 m^2 (declining, -2/3), two ET
//! rasters averaging 0.75 (high demand) -> deficit_risk; precipitation
//! region means 5 + 15 mm -> 20 mm total.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog;
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u32 = 32643;
const TRANSFORM: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const NODATA: f32 = -9999.0;

struct Ctx {
    app: Router,
    pool: geo_hub::db::DbPool,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("water_balance.db").display()
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
    Ok(Ctx {
        app: server::build_router(state),
        pool,
    })
}

#[allow(clippy::too_many_arguments)]
async fn register(
    ctx: &Ctx,
    tmp: &TempDir,
    kind: &str,
    level: ProductLevel,
    stamp: &str,
    field_id: Option<&str>,
    parameters: serde_json::Value,
    raster: Option<Vec<f32>>,
) -> Result<String> {
    let artifact = match raster {
        Some(values) => {
            let path = tmp.path().join(format!("{kind}_{stamp}.tif"));
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
            Some(ProductArtifact {
                format: "tif".to_string(),
                path: path.to_string_lossy().to_string(),
                checksum_sha256: None,
            })
        }
        None => None,
    };
    let draft = ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("test.{kind}"),
        algorithm_version: "1.0.0".to_string(),
        parameters,
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: field_id.map(str::to_string),
            season_id: Some("2026".to_string()),
            scene_id: Some(format!("scene-{kind}-{stamp}")),
            temporal_start: format!("{stamp}T10:30:00Z"),
            temporal_end: format!("{stamp}T10:30:00Z"),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
        artifact,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: None,
    };
    Ok(catalog::register_product(&ctx.pool, &draft, "2026-07-06T00:00:00Z").await?)
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
async fn supply_demand_and_rainfall_summarize_to_a_deficit_risk() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let field = Some("field-1");

    // Supply: shrinking extents (area in identity-bearing parameters).
    let w1 = register(
        &ctx,
        &tmp,
        "water_extent",
        ProductLevel::L3,
        "2026-01-15",
        field,
        json!({ "water_area_m2": 300.0 }),
        None,
    )
    .await?;
    let w2 = register(
        &ctx,
        &tmp,
        "water_extent",
        ProductLevel::L3,
        "2026-06-15",
        field,
        json!({ "water_area_m2": 100.0 }),
        None,
    )
    .await?;
    // Seasonality anchor.
    let s1 = register(
        &ctx,
        &tmp,
        "water_seasonality",
        ProductLevel::L3,
        "2026-06-20",
        field,
        json!({ "permanent_area_m2": 100.0, "seasonal_area_m2": 200.0 }),
        None,
    )
    .await?;
    // Demand: two ET rasters with means 0.8 and 0.7 (one nodata pixel).
    let e1 = register(
        &ctx,
        &tmp,
        "et_fraction",
        ProductLevel::L2,
        "2026-03-01",
        field,
        json!({ "unit": "evaporative_fraction_0_1", "stamp": "2026-03-01" }),
        Some(vec![0.8, 0.8, 0.8, NODATA]),
    )
    .await?;
    let e2 = register(
        &ctx,
        &tmp,
        "et_fraction",
        ProductLevel::L2,
        "2026-05-01",
        field,
        json!({ "unit": "evaporative_fraction_0_1", "stamp": "2026-05-01" }),
        Some(vec![0.7, 0.7, 0.7, 0.7]),
    )
    .await?;
    // Inflow: two CHIRPS months, region means 5 and 15 mm.
    let p1 = register(
        &ctx,
        &tmp,
        "precipitation",
        ProductLevel::L2,
        "2026-02-01",
        None,
        json!({ "units": "mm", "stamp": "2026-02-01" }),
        Some(vec![5.0; 4]),
    )
    .await?;
    let p2 = register(
        &ctx,
        &tmp,
        "precipitation",
        ProductLevel::L2,
        "2026-03-01",
        None,
        json!({ "units": "mm", "stamp": "2026-03-01" }),
        Some(vec![15.0; 4]),
    )
    .await?;

    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/water-management/balance/derive",
        Some(json!({
            "field_id": "field-1",
            "season_id": "2026",
            "start": "2026-01-01",
            "end": "2026-12-31",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["status"], "deficit_risk");
    assert_eq!(outcome["status_reason"], "supply_declining_demand_high");
    assert_eq!(outcome["supply_trend"], "declining");
    assert_eq!(outcome["demand_level"], "high");
    assert!(
        (outcome["relative_area_change"].as_f64().unwrap() - (-2.0 / 3.0)).abs() < 1e-6,
        "{outcome}"
    );
    assert!(
        (outcome["mean_et_fraction"].as_f64().unwrap() - 0.75).abs() < 1e-6,
        "{outcome}"
    );
    assert!((outcome["total_precipitation_mm"].as_f64().unwrap() - 20.0).abs() < 1e-4);

    // The L3 registers with a JSON artifact carrying the summary and
    // lineage to all seven contributing products.
    let balance_id = outcome["water_balance_product_id"].as_str().unwrap();
    let product = catalog::get_product(&ctx.pool, balance_id).await?.unwrap();
    assert_eq!(product.kind, "water_balance");
    assert_eq!(product.parameters["status"], "deficit_risk");
    assert_eq!(product.parameters["permanent_area_m2"], 100.0);
    let artifact: serde_json::Value =
        serde_json::from_slice(&std::fs::read(product.path.as_deref().unwrap())?)?;
    assert_eq!(artifact["status"], "deficit_risk");
    assert_eq!(artifact["first_water_area_m2"], 300.0);
    assert_eq!(artifact["last_water_area_m2"], 100.0);

    let edges = catalog::trace_inputs(&ctx.pool, balance_id).await?;
    let mut inputs: Vec<&str> = edges.iter().map(|e| e.input_product_id.as_str()).collect();
    inputs.sort_unstable();
    let mut expected = vec![
        w1.as_str(),
        w2.as_str(),
        s1.as_str(),
        e1.as_str(),
        e2.as_str(),
        p1.as_str(),
        p2.as_str(),
    ];
    expected.sort_unstable();
    assert_eq!(inputs, expected);

    // Idempotent.
    let (status, again) = send(
        &ctx.app,
        "POST",
        "/api/water-management/balance/derive",
        Some(json!({
            "field_id": "field-1",
            "season_id": "2026",
            "start": "2026-01-01",
            "end": "2026-12-31",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["water_balance_product_id"], json!(balance_id));

    // A field with no supply/demand evidence (only the unscoped regional
    // precipitation exists) records an honest insufficient_evidence
    // verdict — never a guessed balance.
    let (status, body) = send(
        &ctx.app,
        "POST",
        "/api/water-management/balance/derive",
        Some(json!({
            "field_id": "field-empty",
            "season_id": "2026",
            "start": "2026-01-01",
            "end": "2026-12-31",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "insufficient_evidence");
    assert_eq!(body["status_reason"], "missing_supply_or_demand_evidence");
    assert_eq!(body["supply_trend"], "unknown");

    // A truly empty window (no products of any kind) is the 400.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/water-management/balance/derive",
        Some(json!({
            "field_id": "field-empty",
            "season_id": "2026",
            "start": "1999-01-01",
            "end": "1999-12-31",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    Ok(())
}
