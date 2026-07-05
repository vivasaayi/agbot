//! End-to-end test of HLS ingestion (satellite pipeline batch 19): a
//! directory of HLSL30 + HLSS30 band GeoTIFFs registers as harmonized
//! `ndvi` L2 products on one grid, so both instruments feed a single
//! densified NDVI series (the phenology/climatology consumers pick them up
//! through the existing catalog window query).
//!
//! Fixture: two granules on tile 43PFN, one Sentinel (S30) and one Landsat
//! (L30), on the same 30 m grid. Red reflectance 0.2, NIR 0.6 everywhere
//! except one fill pixel -> NDVI (0.6-0.2)/(0.6+0.2) = 0.5, one nodata.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog::{self, ProductFilter};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use raster_io::{write_geotiff_f32, write_geotiff_i16, GeoTiffReader, GeoTiffTags};
use serde_json::json;
use shared::product_graph::ProductLevel;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u32 = 32643;
// HLS 30 m grid on tile 43PFN.
const TRANSFORM: [f64; 6] = [600_000.0, 30.0, 0.0, 1_300_020.0, 0.0, -30.0];
const NODATA: f32 = -9999.0;

async fn ctx(tmp: &TempDir) -> Result<(Router, geo_hub::db::DbPool)> {
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("hls.db").display()),
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

fn write_band(dir: &Path, name: &str, value: f32, fill_pixel: Option<usize>) -> Result<()> {
    let mut values = vec![value; 16];
    if let Some(pixel) = fill_pixel {
        values[pixel] = NODATA;
    }
    write_geotiff_f32(
        &dir.join(name),
        4,
        4,
        &values,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    Ok(())
}

/// Write a real-HLS-style Int16 surface-reflectance band (scaled DN, -9999
/// fill), so the ingest path exercises the Int16 read + 1e-4 reflectance
/// scaling rather than pre-scaled f32.
fn write_band_i16(dir: &Path, name: &str, dn: i16) -> Result<()> {
    write_geotiff_i16(
        &dir.join(name),
        4,
        4,
        &[dn; 16],
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(-9999.0),
        },
    )?;
    Ok(())
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

#[tokio::test]
async fn hls_l30_and_s30_register_as_one_harmonized_ndvi_series() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let hls_dir = tmp.path().join("hls");
    std::fs::create_dir_all(&hls_dir)?;

    // S30 granule (Sentinel): NIR = B08. NIR band has one fill pixel.
    write_band(
        &hls_dir,
        "HLS.S30.T43PFN.2024152T051651.v2.0.B04.tif",
        0.2,
        None,
    )?;
    write_band(
        &hls_dir,
        "HLS.S30.T43PFN.2024152T051651.v2.0.B08.tif",
        0.6,
        Some(15),
    )?;
    // L30 granule (Landsat), next day: NIR = B05. Written as REAL HLS Int16
    // DN (red 2000 -> 0.2 refl, nir 6000 -> 0.6 refl after the 1e-4 scale),
    // exercising the batch-21 Int16 read + reflectance scaling.
    write_band_i16(&hls_dir, "HLS.L30.T43PFN.2024153T052015.v2.0.B04.tif", 2000)?;
    write_band_i16(&hls_dir, "HLS.L30.T43PFN.2024153T052015.v2.0.B05.tif", 6000)?;
    // An incomplete S30 granule (red only) and a non-HLS file.
    write_band(
        &hls_dir,
        "HLS.S30.T43PFN.2024160T051651.v2.0.B04.tif",
        0.3,
        None,
    )?;
    std::fs::write(hls_dir.join("readme.txt"), b"not hls")?;

    let (status, bytes) = send(
        &app,
        "POST",
        "/api/ingest/hls/register",
        Some(json!({ "dir": hls_dir.to_string_lossy() })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;

    let registered = outcome["registered"].as_array().unwrap();
    assert_eq!(registered.len(), 2, "S30 + L30 complete granules");
    // The red-only granule is reported incomplete, not an error.
    let incomplete = outcome["incomplete"].as_array().unwrap();
    assert_eq!(incomplete.len(), 1);
    assert_eq!(incomplete[0][0], "HLS.S30.T43PFN.2024160T051651");
    assert_eq!(outcome["skipped"], json!(["readme.txt"]));

    // Both products are NDVI L2 on the same 30 m grid but distinct scenes,
    // and both carry the HLS source — one harmonized series.
    let ndvi = catalog::list_products(
        &pool,
        &ProductFilter {
            kind: Some("ndvi".to_string()),
            level: Some(ProductLevel::L2),
            status: Some("registered".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(ndvi.len(), 2);
    for product in &ndvi {
        assert_eq!(product.source_id.as_deref(), Some("hls-v2.0"));
        assert_eq!(product.gsd_m_per_px, Some(30.0));
    }
    let mut scenes: Vec<String> = ndvi.iter().filter_map(|p| p.scene_id.clone()).collect();
    scenes.sort();
    assert_eq!(
        scenes,
        vec![
            "HLS.L30.T43PFN.2024153T052015".to_string(),
            "HLS.S30.T43PFN.2024152T051651".to_string(),
        ]
    );
    // Instruments differ, dates are adjacent -> densified series.
    let instruments: std::collections::BTreeSet<String> = ndvi
        .iter()
        .map(|p| p.parameters["instrument"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(instruments.len(), 2);

    // NDVI values: 0.5 everywhere; the S30 NIR-fill pixel is nodata.
    let s30 = ndvi
        .iter()
        .find(|p| p.scene_id.as_deref() == Some("HLS.S30.T43PFN.2024152T051651"))
        .unwrap();
    let mut reader = GeoTiffReader::open(s30.path.as_deref().unwrap())?;
    assert_eq!(reader.info().geo_transform, Some(TRANSFORM));
    let values = reader.read_band()?.to_f32();
    for value in &values[..15] {
        assert!((value - 0.5).abs() < 1e-6, "NDVI must be 0.5, got {value}");
    }
    assert_eq!(values[15], NODATA);

    // The L30 granule is fully valid.
    let l30 = ndvi
        .iter()
        .find(|p| p.scene_id.as_deref() == Some("HLS.L30.T43PFN.2024153T052015"))
        .unwrap();
    let mut reader = GeoTiffReader::open(l30.path.as_deref().unwrap())?;
    let values = reader.read_band()?.to_f32();
    assert!(values.iter().all(|v| (v - 0.5).abs() < 1e-6));

    // Idempotent.
    let (status, bytes) = send(
        &app,
        "POST",
        "/api/ingest/hls/register",
        Some(json!({ "dir": hls_dir.to_string_lossy() })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["registered"], outcome["registered"]);
    Ok(())
}
