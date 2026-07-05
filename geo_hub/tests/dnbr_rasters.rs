//! End-to-end test of the dNBR burn-severity pipeline (satellite batch 14):
//! two cataloged NBR L2 GeoTIFFs -> POST /api/change-detection/dnbr/derive
//! -> `dnbr` L3 GeoTIFF with lineage, web-tiled through the diverging dNBR
//! colormap.
//!
//! Fixture: a 2x2 grid on the Sentinel-2 43PFN 10 m grid.
//! - pixel 0: 0.5 -> 0.5  dNBR  0.0  unburned
//! - pixel 1: 0.6 -> 0.1  dNBR  0.5  moderate-high severity
//! - pixel 2: 0.2 -> 0.5  dNBR -0.3  enhanced regrowth (high)
//! - pixel 3: pre nodata             invalid

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog;
use geo_hub::product_tiler::{colormap_for_kind, tile_containing};
use geo_hub::state::AppState;
use geo_hub::utm::{utm_to_wgs84, UtmZone};
use geo_hub::{db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffReader, GeoTiffTags};
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
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("dnbr.db").display()),
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

async fn register_l2(
    ctx: &Ctx,
    tmp: &TempDir,
    kind: &str,
    stamp: &str,
    values: Vec<f32>,
    transform: [f64; 6],
) -> Result<String> {
    let path = tmp.path().join(format!("{kind}_{stamp}.tif"));
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
        kind: kind.to_string(),
        algorithm_id: "test.dnbr".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "stamp": stamp, "kind": kind }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(format!("scene-{kind}-{stamp}")),
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
) -> Result<(StatusCode, Vec<u8>)> {
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
    Ok((status, bytes.to_vec()))
}

fn derive_body(pre: &str, post: &str) -> serde_json::Value {
    json!({
        "pre_product_id": pre,
        "post_product_id": post,
        "field_id": "field-1",
        "season_id": "season-2026",
    })
}

#[tokio::test]
async fn derives_dnbr_with_lineage_and_web_tiles() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    let mut pre_values = vec![0.5, 0.6, 0.2, 0.4];
    pre_values[3] = NODATA;
    let pre = register_l2(&ctx, &tmp, "nbr", "2026-05-01", pre_values, TRANSFORM).await?;
    let post = register_l2(
        &ctx,
        &tmp,
        "nbr",
        "2026-07-01",
        vec![0.5, 0.1, 0.5, 0.4],
        TRANSFORM,
    )
    .await?;

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/change-detection/dnbr/derive",
        Some(derive_body(&pre, &post)),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(outcome["class_counts"]["unburned"], 1);
    assert_eq!(outcome["class_counts"]["moderate_high_severity"], 1);
    assert_eq!(outcome["class_counts"]["enhanced_regrowth_high"], 1);
    assert_eq!(outcome["class_counts"]["invalid"], 1);
    assert!((outcome["disturbed_fraction"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-6);
    assert!((outcome["valid_fraction"].as_f64().unwrap() - 0.75).abs() < 1e-6);

    // dNBR GeoTIFF: hand-computed values, nodata where pre was missing.
    let dnbr_path = outcome["dnbr_artifact"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(dnbr_path)?;
    assert_eq!(reader.info().epsg, Some(EPSG));
    let values = reader.read_band()?.to_f32();
    assert!(values[0].abs() < 1e-6);
    assert!((values[1] - 0.5).abs() < 1e-6);
    assert!((values[2] + 0.3).abs() < 1e-6);
    assert_eq!(values[3], NODATA);

    // Lineage: exactly the pre + post NBR products; scope spans both dates.
    let dnbr_id = outcome["dnbr_product_id"].as_str().unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, dnbr_id).await?;
    let mut inputs: Vec<&str> = edges
        .iter()
        .map(|edge| edge.input_product_id.as_str())
        .collect();
    inputs.sort_unstable();
    let mut expected = [pre.as_str(), post.as_str()];
    expected.sort_unstable();
    assert_eq!(inputs, expected);
    let dnbr_product = catalog::get_product(&ctx.pool, dnbr_id).await?.unwrap();
    assert_eq!(
        dnbr_product.temporal_start.as_deref(),
        Some("2026-05-01T10:30:00Z")
    );
    assert_eq!(
        dnbr_product.temporal_end.as_deref(),
        Some("2026-07-01T10:30:00Z")
    );

    // Web tile: every opaque pixel carries a diverging-ramp color for one
    // of the three valid dNBR values.
    let (lat, lon) = utm_to_wgs84(
        600_010.0,
        1_300_010.0,
        UtmZone {
            zone: 43,
            north: true,
        },
    );
    let (x, y) = tile_containing(lat, lon, 18);
    let (status, bytes) = send(
        &ctx.app,
        "GET",
        &format!("/api/catalog/products/{dnbr_id}/tiles/18/{x}/{y}.png"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    let colormap = colormap_for_kind("dnbr");
    let allowed: Vec<[u8; 3]> = [0.0f32, 0.5, -0.3]
        .iter()
        .map(|value| colormap.rgb(*value))
        .collect();
    let opaque: Vec<_> = png.pixels().filter(|px| px.0[3] == 255).collect();
    assert!(!opaque.is_empty(), "dNBR footprint must render");
    for px in &opaque {
        let rgb = [px.0[0], px.0[1], px.0[2]];
        assert!(allowed.contains(&rgb), "unexpected tile color {rgb:?}");
    }

    // Listing + idempotency.
    let (status, bytes) = send(&ctx.app, "GET", "/api/change-detection/dnbr", None).await?;
    assert_eq!(status, StatusCode::OK);
    let listing: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(listing["dnbr"].as_array().unwrap().len(), 1);
    assert_eq!(listing["dnbr"][0]["product_id"], json!(dnbr_id));

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/change-detection/dnbr/derive",
        Some(derive_body(&pre, &post)),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["dnbr_product_id"], json!(dnbr_id));
    Ok(())
}

#[tokio::test]
async fn dnbr_error_paths_are_reason_coded() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let pre = register_l2(&ctx, &tmp, "nbr", "2026-05-01", vec![0.5; 4], TRANSFORM).await?;
    let post = register_l2(&ctx, &tmp, "nbr", "2026-07-01", vec![0.1; 4], TRANSFORM).await?;

    // Unknown product.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/change-detection/dnbr/derive",
        Some(derive_body("missing", &post)),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Wrong kind.
    let ndvi = register_l2(&ctx, &tmp, "ndvi", "2026-05-01", vec![0.5; 4], TRANSFORM).await?;
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/change-detection/dnbr/derive",
        Some(derive_body(&ndvi, &post)),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("nbr"));

    // Chronology: pre must precede post.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/change-detection/dnbr/derive",
        Some(derive_body(&post, &pre)),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("not before"));

    // Grid mismatch.
    let shifted = [601_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
    let offgrid = register_l2(&ctx, &tmp, "nbr", "2026-08-01", vec![0.1; 4], shifted).await?;
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/change-detection/dnbr/derive",
        Some(derive_body(&pre, &offgrid)),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("same grid"));
    Ok(())
}
