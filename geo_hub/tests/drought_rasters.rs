//! End-to-end test of the drought raster pipeline (satellite batch 8):
//! cataloged multi-year NDVI L2 GeoTIFFs -> POST
//! /api/drought-management/rasters/derive -> index_climatology + drought_index
//! (VCI) L3 products with lineage -> VCI GeoTIFF web-tiled through the
//! batch-7 catalog tiler.
//!
//! Fixture: a 4x4 grid on the Sentinel-2 43PFN 10 m grid, three Junes:
//! 2024 NDVI=0.2, 2025 NDVI=0.6, current 2026 NDVI=0.4 (pixel 0 nodata).
//! With min_years=2 the baseline is min=0.2/max=0.6 everywhere (three
//! distinct years; pixel 0 still has 2024+2025), so VCI(0.4) =
//! 100*(0.4-0.2)/(0.6-0.2) = 50 on 15 pixels and pixel 0 is
//! no-current-observation nodata.

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
            tmp.path().join("drought_rasters.db").display()
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

/// Write a 4x4 NDVI GeoTIFF and register it as an L2 for `year`'s June.
async fn register_ndvi(
    ctx: &Ctx,
    tmp: &TempDir,
    year: i32,
    values: Vec<f32>,
    transform: [f64; 6],
) -> Result<String> {
    let path = tmp.path().join(format!("ndvi_{year}.tif"));
    write_geotiff_f32(
        &path,
        4,
        4,
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
        algorithm_id: "test.drought_rasters".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "fixture_year": year }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(format!("scene-{year}-06")),
            temporal_start: format!("{year}-06-14T10:30:00Z"),
            temporal_end: format!("{year}-06-14T10:30:00Z"),
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

fn derive_body(current: &str) -> serde_json::Value {
    json!({
        "current_product_id": current,
        "field_id": "field-1",
        "season_id": "season-2026",
        "min_years": 2,
    })
}

#[tokio::test]
async fn derives_vci_raster_with_lineage_and_web_tiles() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    let p2024 = register_ndvi(&ctx, &tmp, 2024, vec![0.2; 16], TRANSFORM).await?;
    let p2025 = register_ndvi(&ctx, &tmp, 2025, vec![0.6; 16], TRANSFORM).await?;
    let mut current_values = vec![0.4f32; 16];
    current_values[0] = NODATA;
    let current = register_ndvi(&ctx, &tmp, 2026, current_values, TRANSFORM).await?;

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/rasters/derive",
        Some(derive_body(&current)),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(outcome["drought_index_kind"], "vci");
    assert_eq!(outcome["period"], "m06");
    let used: Vec<String> = serde_json::from_value(outcome["observations_used"].clone())?;
    for id in [&p2024, &p2025, &current] {
        assert!(used.contains(id), "{id} must feed the climatology");
    }
    assert!(outcome["observations_skipped"]
        .as_array()
        .unwrap()
        .is_empty());
    // 15/16 valid: pixel 0 has no current observation.
    let valid_fraction = outcome["valid_fraction"].as_f64().unwrap();
    assert!((valid_fraction - 15.0 / 16.0).abs() < 1e-6);

    // VCI GeoTIFF: 15 pixels at exactly 50, pixel 0 nodata.
    let drought_path = outcome["drought_artifact"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(drought_path)?;
    assert_eq!(reader.info().epsg, Some(EPSG));
    assert_eq!(reader.info().geo_transform, Some(TRANSFORM));
    let values = reader.read_band()?.to_f32();
    assert_eq!(values[0], NODATA);
    for value in &values[1..] {
        assert!((value - 50.0).abs() < 1e-4, "VCI must be 50, got {value}");
    }

    // Lineage: the drought L3 consumes the current + baseline L2s and the
    // registered climatology L3.
    let drought_id = outcome["drought_product_id"].as_str().unwrap();
    let climatology_id = outcome["climatology_product_id"].as_str().unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, drought_id).await?;
    let inputs: Vec<&str> = edges
        .iter()
        .map(|edge| edge.input_product_id.as_str())
        .collect();
    for id in [
        current.as_str(),
        p2024.as_str(),
        p2025.as_str(),
        climatology_id,
    ] {
        assert!(inputs.contains(&id), "missing lineage edge to {id}");
    }

    // The VCI raster web-tiles through the batch-7 tiler with the condition
    // colormap (kind drought_index, domain 0..100 -> value 50 = mid ramp).
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
        &format!("/api/catalog/products/{drought_id}/tiles/17/{x}/{y}.png"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    let expected = colormap_for_kind("drought_index").rgb(50.0);
    let opaque: Vec<_> = png.pixels().filter(|px| px.0[3] == 255).collect();
    assert!(!opaque.is_empty(), "VCI footprint must render");
    for px in &opaque {
        assert_eq!(&px.0[..3], &expected);
    }

    // Rasters are listable via the drought-management namespace.
    let (status, bytes) = send(&ctx.app, "GET", "/api/drought-management/rasters", None).await?;
    assert_eq!(status, StatusCode::OK);
    let listing: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(listing["climatologies"].as_array().unwrap().len(), 1);
    assert_eq!(listing["drought_indices"].as_array().unwrap().len(), 1);
    assert_eq!(
        listing["drought_indices"][0]["product_id"],
        json!(drought_id)
    );

    // Idempotent: re-deriving identical inputs returns the same
    // content-addressed product ids.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/rasters/derive",
        Some(derive_body(&current)),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["drought_product_id"], json!(drought_id));
    assert_eq!(again["climatology_product_id"], json!(climatology_id));
    Ok(())
}

#[tokio::test]
async fn mismatched_grids_are_skipped_with_reason() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    register_ndvi(&ctx, &tmp, 2024, vec![0.2; 16], TRANSFORM).await?;
    // Different grid origin: must be skipped, not resampled.
    let shifted = [601_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
    let offgrid = register_ndvi(&ctx, &tmp, 2025, vec![0.6; 16], shifted).await?;
    let current = register_ndvi(&ctx, &tmp, 2026, vec![0.4; 16], TRANSFORM).await?;

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/rasters/derive",
        Some(derive_body(&current)),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        outcome["observations_skipped"],
        json!([{ "product_id": offgrid, "reason": "grid_mismatch" }])
    );
    // Only 2024 + 2026 remain = 2 distinct years, still >= min_years 2:
    // baseline min 0.2 / max 0.4 -> VCI(0.4) = 100.
    let valid_fraction = outcome["valid_fraction"].as_f64().unwrap();
    assert!((valid_fraction - 1.0).abs() < 1e-6);
    Ok(())
}

#[tokio::test]
async fn error_paths_are_reason_coded() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Unknown current product.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/rasters/derive",
        Some(derive_body("no-such-product")),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Unsupported index kind (mndwi has no drought mapping).
    let path = tmp.path().join("mndwi.tif");
    write_geotiff_f32(
        &path,
        2,
        2,
        &[0.1, 0.2, 0.3, 0.4],
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "mndwi".to_string(),
        algorithm_id: "test.drought_rasters".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({}),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some("scene-mndwi".to_string()),
            temporal_start: "2026-06-14T00:00:00Z".to_string(),
            temporal_end: "2026-06-14T00:00:00Z".to_string(),
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
    let mndwi = catalog::register_product(&ctx.pool, &draft, "2026-07-05T00:00:00Z").await?;
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/rasters/derive",
        Some(derive_body(&mndwi)),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("no drought mapping"));
    Ok(())
}
