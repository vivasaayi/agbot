//! End-to-end test of catalog-driven temporal compositing (satellite
//! pipeline batch 32): a June window of cataloged NDVI L2 GeoTIFFs ->
//! POST /api/composites/derive -> per-pixel median `temporal_composite` L3
//! with lineage to every observation and cloud gaps filled across dates.
//!
//! Fixture: 2x2 NDVI on the 43PFN 10 m grid, three June observations.
//! Pixel-wise series (columns = pixels 0..3, `--` = nodata/cloud):
//!   Jun 01: 0.20  0.40  --   0.60
//!   Jun 11: 0.40  0.20  --   0.80
//!   Jun 21: 0.60  0.80  --   0.70
//! Median:  0.40  0.40  gap  0.70; gap_fraction = 1/4. A July observation
//! is outside the window and a mis-gridded June one is skipped with a
//! reason — neither contributes.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog;
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffReader, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u32 = 32643;
const TRANSFORM: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const SHIFTED: [f64; 6] = [600_100.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const NODATA: f32 = -9999.0;

struct Ctx {
    app: Router,
    pool: geo_hub::db::DbPool,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("composite.db").display()
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

async fn register_ndvi(
    ctx: &Ctx,
    tmp: &TempDir,
    stamp: &str,
    values: Vec<f32>,
    transform: [f64; 6],
) -> Result<String> {
    let path = tmp.path().join(format!("ndvi_{stamp}.tif"));
    write_geotiff_f32(
        &path,
        2,
        2,
        &values,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(transform),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "test.composite".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "stamp": stamp, "transform0": transform[0] }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(format!("scene-{stamp}")),
            temporal_start: format!("{stamp}T10:30:00Z"),
            temporal_end: format!("{stamp}T10:30:00Z"),
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
    Ok(catalog::register_product(&ctx.pool, &draft, "2026-07-05T00:00:00Z").await?)
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
async fn june_window_composites_to_the_per_pixel_median_with_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Three June observations; pixel 2 is cloud/nodata in every one.
    let a = register_ndvi(
        &ctx,
        &tmp,
        "2026-06-01",
        vec![0.2, 0.4, NODATA, 0.6],
        TRANSFORM,
    )
    .await?;
    let b = register_ndvi(
        &ctx,
        &tmp,
        "2026-06-11",
        vec![0.4, 0.2, NODATA, 0.8],
        TRANSFORM,
    )
    .await?;
    let c = register_ndvi(
        &ctx,
        &tmp,
        "2026-06-21",
        vec![0.6, 0.8, NODATA, 0.7],
        TRANSFORM,
    )
    .await?;
    // Outside the window and off-grid: both must be excluded.
    let july = register_ndvi(
        &ctx,
        &tmp,
        "2026-07-10",
        vec![0.9, 0.9, 0.9, 0.9],
        TRANSFORM,
    )
    .await?;
    let shifted =
        register_ndvi(&ctx, &tmp, "2026-06-15", vec![0.9, 0.9, 0.9, 0.9], SHIFTED).await?;

    let body = json!({
        "kind": "ndvi",
        "start": "2026-06-01",
        "end": "2026-06-30",
        "field_id": "field-1",
        "season_id": "2026-kharif",
    });
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/composites/derive",
        Some(body.clone()),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["method"], "median");
    assert_eq!(outcome["period_start"], "2026-06-01");
    assert_eq!(outcome["period_end"], "2026-06-21");
    assert_eq!(outcome["gap_fraction"].as_f64().unwrap(), 0.25);
    let used: Vec<&str> = outcome["observations_used"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(used, vec![a.as_str(), b.as_str(), c.as_str()]);
    assert!(!used.contains(&july.as_str()), "july is outside the window");
    let skipped = outcome["observations_skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["product_id"], json!(shifted));
    assert_eq!(skipped[0]["reason"], "grid_mismatch");

    // Hand-computed per-pixel medians; the all-cloud pixel stays nodata.
    let composite_id = outcome["composite_product_id"].as_str().unwrap();
    let product = catalog::get_product(&ctx.pool, composite_id)
        .await?
        .unwrap();
    assert_eq!(product.kind, "temporal_composite");
    assert_eq!(product.level, ProductLevel::L3);
    assert_eq!(
        product.temporal_start.as_deref(),
        Some("2026-06-01T00:00:00Z")
    );
    assert!((product.confidence.unwrap() - 0.75).abs() < 1e-9);
    let values = {
        let mut reader = GeoTiffReader::open(product.path.as_deref().unwrap())?;
        assert_eq!(reader.info().geo_transform, Some(TRANSFORM));
        reader.read_band()?.to_f32()
    };
    for (pixel, expected) in [(0usize, 0.4f32), (1, 0.4), (3, 0.7)] {
        assert!(
            (values[pixel] - expected).abs() < 1e-6,
            "pixel {pixel}: {} != {expected}",
            values[pixel]
        );
    }
    assert_eq!(values[2], NODATA, "all-cloud pixel is a gap");

    // Lineage covers exactly the three used observations.
    let edges = catalog::trace_inputs(&ctx.pool, composite_id).await?;
    let mut inputs: Vec<&str> = edges.iter().map(|e| e.input_product_id.as_str()).collect();
    inputs.sort_unstable();
    let mut expected = [a.as_str(), b.as_str(), c.as_str()];
    expected.sort_unstable();
    assert_eq!(inputs, expected);

    // Idempotent; listable; bad method + empty window reason-coded.
    let (status, again) = send(&ctx.app, "POST", "/api/composites/derive", Some(body)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["composite_product_id"], json!(composite_id));

    let (status, listing) = send(&ctx.app, "GET", "/api/composites", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listing["composites"].as_array().unwrap().len(), 1);

    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/composites/derive",
        Some(json!({
            "kind": "ndvi", "start": "2026-06-01", "end": "2026-06-30",
            "method": "max_ndvi", "field_id": "f", "season_id": "s",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/composites/derive",
        Some(json!({
            "kind": "ndvi", "start": "2001-01-01", "end": "2001-01-31",
            "field_id": "f", "season_id": "s",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    Ok(())
}
