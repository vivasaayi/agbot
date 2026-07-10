//! Route test for the global Web Mercator catalog product tiler
//! (`GET /api/catalog/products/:product_id/tiles/:z/:x/:y.png`,
//! satellite pipeline batch 7).
//!
//! A small NDVI GeoTIFF on the Sentinel-2 43PFN grid is written with
//! `raster_io` and registered as an L2 catalog product; the route must
//! render a true slippy-map tile (colormapped footprint, transparent
//! background/nodata), disk-cache it, and reason-code bad requests.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog;
use geo_hub::product_tiler::{colormap_for_kind, tile_containing, TILE_SIZE};
use geo_hub::state::AppState;
use geo_hub::utm::{utm_to_wgs84, UtmZone};
use geo_hub::{db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u32 = 32643;
const TRANSFORM: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
const NODATA: f32 = -9999.0;
const NDVI_VALUE: f32 = 0.8;

fn zone() -> UtmZone {
    UtmZone {
        zone: 43,
        north: true,
    }
}

struct Ctx {
    app: Router,
    pool: geo_hub::db::DbPool,
    data_root: std::path::PathBuf,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let db_path = tmp.path().join("product_web_tiles.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let data_root = config.data_root.clone();
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };
    Ok(Ctx {
        app: server::build_router(state),
        pool,
        data_root,
    })
}

/// Write an 8x8 NDVI GeoTIFF (top-left pixel nodata) and register it as an
/// L2 catalog product; returns the product id.
async fn register_ndvi_product(ctx: &Ctx, tmp: &TempDir) -> Result<String> {
    let path = tmp.path().join("ndvi.tif");
    let mut values = vec![NDVI_VALUE; 64];
    values[0] = NODATA;
    write_geotiff_f32(
        &path,
        8,
        8,
        &values,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "test.web_tiles".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({ "fixture": "product_web_tiles" }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some("scene-webtile".to_string()),
            temporal_start: "2026-07-01T00:00:00Z".to_string(),
            temporal_end: "2026-07-01T00:00:00Z".to_string(),
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

/// Write three single-band GeoTIFFs (red=3000, green=1500, blue=0) on the same
/// grid and register a true-color `rgb` product whose parameters reference them
/// with an explicit 0..3000 stretch. Returns the product id.
async fn register_rgb_product(ctx: &Ctx, tmp: &TempDir) -> Result<String> {
    let mut band_paths = std::collections::BTreeMap::new();
    for (role, value) in [("red", 3000.0f32), ("green", 1500.0), ("blue", 0.0)] {
        let path = tmp.path().join(format!("{role}.tif"));
        write_geotiff_f32(
            &path,
            8,
            8,
            &vec![value; 64],
            &GeoTiffTags {
                epsg: Some(EPSG),
                geo_transform: Some(TRANSFORM),
                nodata: Some(f64::from(NODATA)),
            },
        )?;
        band_paths.insert(role, path.to_string_lossy().to_string());
    }
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "rgb".to_string(),
        algorithm_id: "test.rgb".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "bands": {
                "red": band_paths["red"],
                "green": band_paths["green"],
                "blue": band_paths["blue"],
            },
            "stretch": { "lo": [0.0, 0.0, 0.0], "hi": [3000.0, 3000.0, 3000.0] },
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some("scene-webtile-rgb".to_string()),
            temporal_start: "2026-07-01T00:00:00Z".to_string(),
            temporal_end: "2026-07-01T00:00:00Z".to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: None,
    };
    Ok(catalog::register_product(&ctx.pool, &draft, "2026-07-05T00:00:00Z").await?)
}

async fn get_tile(app: &Router, uri: &str) -> Result<(StatusCode, Vec<u8>)> {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty())?)
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await?;
    Ok((status, bytes.to_vec()))
}

#[tokio::test]
async fn renders_colormapped_web_tile_and_caches_it() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let product_id = register_ndvi_product(&ctx, &tmp).await?;

    // Tile containing the raster center (600040, 1299980 UTM 43N).
    let (lat, lon) = utm_to_wgs84(600_040.0, 1_299_980.0, zone());
    let z = 16u8;
    let (x, y) = tile_containing(lat, lon, z);
    let uri = format!("/api/catalog/products/{product_id}/tiles/{z}/{x}/{y}.png");

    let (status, bytes) = get_tile(&ctx.app, &uri).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    assert_eq!(png.dimensions(), (TILE_SIZE, TILE_SIZE));

    let opaque: Vec<_> = png.pixels().filter(|px| px.0[3] == 255).collect();
    assert!(
        !opaque.is_empty(),
        "the raster footprint must render opaque pixels"
    );
    assert!(
        opaque.len() < (TILE_SIZE * TILE_SIZE) as usize,
        "an 80 m raster cannot fill a z16 tile"
    );
    // Every opaque pixel carries the exact deterministic NDVI ramp color.
    let expected = colormap_for_kind("ndvi").rgb(NDVI_VALUE);
    for px in &opaque {
        assert_eq!(&px.0[..3], &expected);
    }

    // The encoded tile is disk-cached under the catalog tile cache.
    let cache_root = ctx.data_root.join("tile_cache").join("catalog");
    let cached = walk_pngs(&cache_root);
    assert_eq!(cached.len(), 1, "one cached tile expected: {cached:?}");
    // A second request serves the identical bytes.
    let (status2, bytes2) = get_tile(&ctx.app, &uri).await?;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(bytes2, bytes);
    Ok(())
}

#[tokio::test]
async fn renders_true_color_rgb_composite_tile() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let product_id = register_rgb_product(&ctx, &tmp).await?;

    let (lat, lon) = utm_to_wgs84(600_040.0, 1_299_980.0, zone());
    let z = 16u8;
    let (x, y) = tile_containing(lat, lon, z);
    let uri = format!("/api/catalog/products/{product_id}/tiles/{z}/{x}/{y}.png");

    let (status, bytes) = get_tile(&ctx.app, &uri).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    assert_eq!(png.dimensions(), (TILE_SIZE, TILE_SIZE));

    let opaque: Vec<_> = png.pixels().filter(|px| px.0[3] == 255).collect();
    assert!(!opaque.is_empty(), "the composite footprint must render");
    // red=3000 -> 255, green=1500 -> 128, blue=0 -> 0 under the 0..3000 stretch.
    for px in &opaque {
        assert_eq!(&px.0[..3], &[255u8, 128, 0]);
    }
    Ok(())
}

#[tokio::test]
async fn tile_away_from_the_raster_is_fully_transparent() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let product_id = register_ndvi_product(&ctx, &tmp).await?;

    let (x, y) = tile_containing(0.0, 0.0, 10);
    let uri = format!("/api/catalog/products/{product_id}/tiles/10/{x}/{y}.png");
    let (status, bytes) = get_tile(&ctx.app, &uri).await?;
    assert_eq!(status, StatusCode::OK);
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    assert!(png.pixels().all(|px| px.0[3] == 0));
    Ok(())
}

#[tokio::test]
async fn unknown_product_and_bad_requests_are_reason_coded() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let product_id = register_ndvi_product(&ctx, &tmp).await?;

    let (status, _) = get_tile(&ctx.app, "/api/catalog/products/nope/tiles/5/0/0.png").await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Tile coordinates outside the zoom grid.
    let (status, body) = get_tile(
        &ctx.app,
        &format!("/api/catalog/products/{product_id}/tiles/2/9/0.png"),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        String::from_utf8_lossy(&body).contains("outside"),
        "{body:?}"
    );

    // Missing .png suffix.
    let (status, _) = get_tile(
        &ctx.app,
        &format!("/api/catalog/products/{product_id}/tiles/2/0/0"),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

fn walk_pngs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk_pngs(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("png") {
            found.push(path);
        }
    }
    found
}
