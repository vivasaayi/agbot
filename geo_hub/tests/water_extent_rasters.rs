//! End-to-end test of the water-extent pipeline (satellite batch 15):
//! cataloged MNDWI L2 GeoTIFF -> POST /api/water-management/extent/derive
//! -> `water_extent` L3 binary mask with lineage + area evidence, web-tiled
//! through the categorical water-mask colormap.
//!
//! Fixture: 4x4 MNDWI on the 43PFN 10 m grid: 8 water pixels at +0.5,
//! 7 land at -0.5, 1 nodata. Otsu must cut between the modes: 8 water
//! pixels = 800 m^2 at 10 m GSD.

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
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("water_extent.db").display()
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

async fn register_index(ctx: &Ctx, tmp: &TempDir, kind: &str, values: Vec<f32>) -> Result<String> {
    let path = tmp.path().join(format!("{kind}.tif"));
    write_geotiff_f32(
        &path,
        4,
        4,
        &values,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: kind.to_string(),
        algorithm_id: "test.water_extent".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "kind": kind }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(format!("scene-{kind}")),
            temporal_start: "2026-06-14T10:30:00Z".to_string(),
            temporal_end: "2026-06-14T10:30:00Z".to_string(),
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

fn derive_body(product_id: &str) -> serde_json::Value {
    json!({
        "product_id": product_id,
        "field_id": "field-1",
        "season_id": "season-2026",
    })
}

#[tokio::test]
async fn derives_water_extent_with_area_lineage_and_tiles() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let mut values = vec![-0.5f32; 16];
    for pixel in 0..8 {
        values[pixel] = 0.5;
    }
    values[15] = NODATA;
    let mndwi = register_index(&ctx, &tmp, "mndwi", values).await?;

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/water-management/extent/derive",
        Some(derive_body(&mndwi)),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(outcome["method"], "otsu");
    let threshold = outcome["threshold"].as_f64().unwrap();
    assert!(threshold > -0.5 && threshold < 0.5, "{threshold}");
    assert_eq!(outcome["water_pixels"], 8);
    assert!((outcome["water_fraction"].as_f64().unwrap() - 8.0 / 15.0).abs() < 1e-6);
    assert_eq!(outcome["water_area_m2"], 800.0);

    // Mask GeoTIFF: 1/0/nodata.
    let mask_path = outcome["water_extent_artifact"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(mask_path)?;
    let mask = reader.read_band()?.to_f32();
    assert_eq!(&mask[..8], &[1.0; 8]);
    assert_eq!(&mask[8..15], &[0.0; 7]);
    assert_eq!(mask[15], NODATA);

    // Lineage: exactly the MNDWI input; scene scope inherited.
    let extent_id = outcome["water_extent_product_id"].as_str().unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, extent_id).await?;
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].input_product_id, mndwi);
    let product = catalog::get_product(&ctx.pool, extent_id).await?.unwrap();
    assert_eq!(product.scene_id.as_deref(), Some("scene-mndwi"));
    assert_eq!(product.parameters["water_area_m2"], 800.0);

    // Web tile: only the two categorical mask colors appear.
    let (lat, lon) = utm_to_wgs84(
        600_020.0,
        1_299_980.0,
        UtmZone {
            zone: 43,
            north: true,
        },
    );
    let (x, y) = tile_containing(lat, lon, 17);
    let (status, bytes) = send(
        &ctx.app,
        "GET",
        &format!("/api/catalog/products/{extent_id}/tiles/17/{x}/{y}.png"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    let colormap = colormap_for_kind("water_extent");
    let allowed = [colormap.rgb(0.0), colormap.rgb(1.0)];
    let opaque: Vec<_> = png.pixels().filter(|px| px.0[3] == 255).collect();
    assert!(!opaque.is_empty(), "mask footprint must render");
    for px in &opaque {
        let rgb = [px.0[0], px.0[1], px.0[2]];
        assert!(allowed.contains(&rgb), "unexpected tile color {rgb:?}");
    }

    // Listing + idempotency.
    let (status, bytes) = send(&ctx.app, "GET", "/api/water-management/extent", None).await?;
    assert_eq!(status, StatusCode::OK);
    let listing: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(listing["water_extent"].as_array().unwrap().len(), 1);

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/water-management/extent/derive",
        Some(derive_body(&mndwi)),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["water_extent_product_id"], json!(extent_id));
    Ok(())
}

#[tokio::test]
async fn water_extent_error_paths_are_reason_coded() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Unknown product.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/water-management/extent/derive",
        Some(derive_body("missing")),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Not a water index.
    let ndvi = register_index(&ctx, &tmp, "ndvi", vec![0.5; 16]).await?;
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/water-management/extent/derive",
        Some(derive_body(&ndvi)),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("water index"));

    // Unimodal all-land scene: succeeds via the reason-coded fallback.
    let mndwi = register_index(&ctx, &tmp, "mndwi", vec![-0.4; 16]).await?;
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/water-management/extent/derive",
        Some(derive_body(&mndwi)),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(outcome["method"], "fixed_fallback");
    assert_eq!(outcome["water_pixels"], 0);
    Ok(())
}
