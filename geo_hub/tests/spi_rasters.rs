//! End-to-end test of the CHIRPS + SPI pipeline (satellite batch 9):
//! CHIRPS-named monthly precipitation GeoTIFFs on an EPSG:4326 grid ->
//! POST /api/drought-management/chirps/register -> POST
//! /api/drought-management/spi/derive -> `spi` L3 GeoTIFF with lineage,
//! web-tiled through the geographic-grid catalog tiler.
//!
//! Fixture: a 2x2 CHIRPS-style 0.05 degree grid, six Junes 2020-2025.
//! Record per pixel {0, 0, 0, 10, 20, 30} and current (2025) = 0: the zero
//! fraction is q = 0.5 and a zero-rain current gives cumulative probability
//! exactly q, so SPI = probit(0.5) = 0 on every pixel — hand-computed.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog;
use geo_hub::product_tiler::{colormap_for_kind, tile_containing};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffReader, GeoTiffTags};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

/// CHIRPS-style geographic grid: 0.05 degree pixels from (76.0, 11.1) down.
const TRANSFORM: [f64; 6] = [76.0, 0.05, 0.0, 11.1, 0.0, -0.05];
const NODATA: f32 = -9999.0;

struct Ctx {
    app: Router,
    pool: geo_hub::db::DbPool,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("spi_rasters.db").display()
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

fn write_chirps_month(dir: &Path, year: i32, month: u32, values: Vec<f32>) -> Result<()> {
    write_geotiff_f32(
        &dir.join(format!("chirps-v2.0.{year}.{month:02}.tif")),
        2,
        2,
        &values,
        &GeoTiffTags {
            epsg: Some(4326),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    Ok(())
}

fn write_chirps(dir: &Path, year: i32, value: f32) -> Result<()> {
    let mut values = vec![value; 4];
    if year == 2025 {
        values[3] = NODATA; // one nodata pixel in the current month
    }
    write_chirps_month(dir, year, 6, values)
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
async fn chirps_register_then_spi_derive_end_to_end() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let chirps_dir = tmp.path().join("chirps");
    std::fs::create_dir_all(&chirps_dir)?;
    // Record (current included): {0, 0, 10, 20, 30, 0} -> three zeros of
    // six samples, q = 0.5; a zero-rain current gives H = q = 0.5, so
    // SPI = probit(0.5) = 0 exactly.
    for (year, value) in [
        (2020, 0.0f32),
        (2021, 0.0),
        (2022, 10.0),
        (2023, 20.0),
        (2024, 30.0),
        (2025, 0.0),
    ] {
        write_chirps(&chirps_dir, year, value)?;
    }
    // A different month must not enter the June record.
    std::fs::copy(
        chirps_dir.join("chirps-v2.0.2024.06.tif"),
        chirps_dir.join("chirps-v2.0.2024.07.tif"),
    )?;
    // A non-CHIRPS file is skipped, not an error.
    std::fs::write(chirps_dir.join("notes.txt"), b"not a raster")?;

    // --- Register the directory.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/chirps/register",
        Some(json!({ "dir": chirps_dir.to_string_lossy() })),
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
    assert_eq!(registered.len(), 7, "6 Junes + 1 July");
    assert_eq!(outcome["skipped"], json!(["notes.txt"]));
    // Registration is idempotent.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/chirps/register",
        Some(json!({ "dir": chirps_dir.to_string_lossy() })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["registered"], outcome["registered"]);

    let product_id_of = |name: &str| -> String {
        registered
            .iter()
            .find(|entry| entry[0] == name)
            .unwrap_or_else(|| panic!("{name} not registered"))[1]
            .as_str()
            .unwrap()
            .to_string()
    };
    let current = product_id_of("chirps-v2.0.2025.06.tif");
    let july = product_id_of("chirps-v2.0.2024.07.tif");

    // --- Derive SPI for June 2025.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/spi/derive",
        Some(json!({
            "current_product_id": current,
            "field_id": "field-1",
            "season_id": "season-2025",
            "min_years": 5,
        })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(outcome["month"], 6);
    assert_eq!(
        outcome["record_years"],
        json!([2020, 2021, 2022, 2023, 2024, 2025])
    );
    let used = outcome["observations_used"].as_array().unwrap();
    assert_eq!(used.len(), 6, "all six Junes, July excluded: {used:?}");
    assert!(!used.iter().any(|id| id == &json!(july.clone())));
    // Pixel 3 is nodata in the current month -> 3/4 valid.
    assert!((outcome["valid_fraction"].as_f64().unwrap() - 0.75).abs() < 1e-6);
    assert_eq!(outcome["class_counts"]["near_normal"], 3);
    assert_eq!(outcome["class_counts"]["invalid"], 1);

    // SPI GeoTIFF: exactly 0 on valid pixels, nodata on pixel 3.
    let spi_path = outcome["spi_artifact"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(spi_path)?;
    assert_eq!(reader.info().epsg, Some(4326));
    let values = reader.read_band()?.to_f32();
    for pixel in 0..3 {
        assert!(
            values[pixel].abs() < 1e-6,
            "SPI must be 0, got {}",
            values[pixel]
        );
    }
    assert_eq!(values[3], NODATA);

    // Lineage: the SPI L3 consumes the current + all record products.
    let spi_id = outcome["spi_product_id"].as_str().unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, spi_id).await?;
    assert_eq!(edges.len(), 6, "current + five record Junes (deduped)");

    // Web-tiles through the geographic-grid tiler: SPI 0 = exact neutral
    // ramp color.
    let (x, y) = tile_containing(11.075, 76.025, 13);
    let (status, bytes) = send(
        &ctx.app,
        "GET",
        &format!("/api/catalog/products/{spi_id}/tiles/13/{x}/{y}.png"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    let expected = colormap_for_kind("spi").rgb(0.0);
    let opaque: Vec<_> = png.pixels().filter(|px| px.0[3] == 255).collect();
    assert!(!opaque.is_empty(), "SPI footprint must render");
    for px in &opaque {
        assert_eq!(&px.0[..3], &expected);
    }

    // Listed under the drought-management raster namespace.
    let (status, bytes) = send(&ctx.app, "GET", "/api/drought-management/rasters", None).await?;
    assert_eq!(status, StatusCode::OK);
    let listing: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(listing["spi"].as_array().unwrap().len(), 1);
    assert_eq!(listing["spi"][0]["product_id"], json!(spi_id));
    Ok(())
}

#[tokio::test]
async fn spi_error_paths_are_reason_coded() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Unknown current product.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/spi/derive",
        Some(json!({
            "current_product_id": "no-such-product",
            "field_id": "f",
            "season_id": "s",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Registering a missing directory is a client-visible store error.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/chirps/register",
        Some(json!({ "dir": tmp.path().join("nope").to_string_lossy() })),
    )
    .await?;
    assert_ne!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn spi3_accumulates_windows_and_gets_a_distinct_identity() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let chirps_dir = tmp.path().join("chirps");
    std::fs::create_dir_all(&chirps_dir)?;
    // Apr+May+Jun per year; window sums (current included):
    // {0, 0, 30, 60, 90, 0} -> three zeros of six, q = 0.5; a zero current
    // window gives SPI = probit(0.5) = 0 exactly (same mixture argument as
    // the SPI-1 test, now over accumulated windows).
    for (year, monthly) in [
        (2020, 0.0f32),
        (2021, 0.0),
        (2022, 10.0),
        (2023, 20.0),
        (2024, 30.0),
        (2025, 0.0),
    ] {
        for month in [4u32, 5, 6] {
            write_chirps_month(&chirps_dir, year, month, vec![monthly; 4])?;
        }
    }
    // 2019 has May+Jun but no April: its SPI-3 window is incomplete and the
    // year must be skipped with a reason, not silently folded in.
    for month in [5u32, 6] {
        write_chirps_month(&chirps_dir, 2019, month, vec![5.0; 4])?;
    }

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/chirps/register",
        Some(json!({ "dir": chirps_dir.to_string_lossy() })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let registered: serde_json::Value = serde_json::from_slice(&bytes)?;
    let product_id_of = |name: &str| -> String {
        registered["registered"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry[0] == name)
            .unwrap_or_else(|| panic!("{name} not registered"))[1]
            .as_str()
            .unwrap()
            .to_string()
    };
    let current = product_id_of("chirps-v2.0.2025.06.tif");

    let derive = |window: u32| {
        json!({
            "current_product_id": current,
            "field_id": "field-1",
            "season_id": "season-2025",
            "min_years": 5,
            "window_months": window,
        })
    };

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/spi/derive",
        Some(derive(3)),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let spi3: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(spi3["window_months"], 3);
    assert_eq!(spi3["month"], 6);
    assert_eq!(
        spi3["record_years"],
        json!([2020, 2021, 2022, 2023, 2024, 2025])
    );
    assert_eq!(
        spi3["observations_skipped"],
        json!([{ "product_id": "window:2019-06", "reason": "incomplete_window" }])
    );
    // SPI exactly 0 everywhere (all pixels valid in this fixture).
    assert_eq!(spi3["valid_fraction"], 1.0);
    let spi_path = spi3["spi_artifact"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(spi_path)?;
    let values = reader.read_band()?.to_f32();
    for value in &values {
        assert!(value.abs() < 1e-6, "SPI-3 must be 0, got {value}");
    }

    // Lineage reaches every window member month: 6 complete years x 3
    // months = 18 monthly products (the current product IS the 2025 June
    // ending product, so it dedups into the same set).
    let spi3_id = spi3["spi_product_id"].as_str().unwrap();
    let edges = geo_hub::catalog::trace_inputs(&ctx.pool, spi3_id).await?;
    assert_eq!(edges.len(), 18, "all window members: {edges:?}");

    // SPI-1 for the same end month is a distinct product identity.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/spi/derive",
        Some(derive(1)),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let spi1: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_ne!(spi1["spi_product_id"], spi3["spi_product_id"]);

    // Window validation is reason-coded.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/spi/derive",
        Some(derive(13)),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("1..=12"));

    // A window reaching before the record start is a client error naming
    // the missing month (December 2024 for a 7-month window... actually
    // window 12 needs Jul 2024: absent).
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/spi/derive",
        Some(derive(12)),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        String::from_utf8_lossy(&bytes).contains("incomplete"),
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    Ok(())
}
