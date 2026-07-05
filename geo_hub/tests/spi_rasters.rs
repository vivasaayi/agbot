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
use geo_hub::spi_rasters::{ChirpsFetcher, ChirpsFetcherHandle};
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

/// In-memory fetcher: URL -> bytes; anything else is a 404-style error.
struct MapFetcher(std::collections::BTreeMap<String, Vec<u8>>);

impl ChirpsFetcher for MapFetcher {
    fn fetch<'a>(
        &'a self,
        url: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send + 'a>>
    {
        Box::pin(async move {
            self.0
                .get(url)
                .cloned()
                .ok_or_else(|| format!("HTTP 404 Not Found: {url}"))
        })
    }
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn small_precip_tif(tmp: &TempDir, value: f32) -> Result<Vec<u8>> {
    let path = tmp.path().join(format!("fixture_{value}.tif"));
    write_geotiff_f32(
        &path,
        2,
        2,
        &[value; 4],
        &GeoTiffTags {
            epsg: Some(4326),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    Ok(std::fs::read(path)?)
}

#[tokio::test]
async fn chirps_fetch_downloads_registers_and_resumes() -> Result<()> {
    let tmp = TempDir::new()?;
    let base_ctx = ctx(&tmp).await?;

    // Fake archive: Jan+Feb 2024 monthly (gzipped, as CHIRPS serves them)
    // and the three June 2024 dekads; March is absent -> per-file failure.
    let base = "https://chirps.test/CHIRPS-2.0";
    let mut archive = std::collections::BTreeMap::new();
    for (month, value) in [(1u32, 10.0f32), (2, 20.0)] {
        archive.insert(
            format!("{base}/global_monthly/tifs/chirps-v2.0.2024.{month:02}.tif.gz"),
            gzip(&small_precip_tif(&tmp, value)?),
        );
    }
    for dekad in 1u8..=3 {
        archive.insert(
            format!("{base}/global_dekad/tifs/chirps-v2.0.2024.06.{dekad}.tif.gz"),
            gzip(&small_precip_tif(&tmp, f32::from(dekad))?),
        );
    }
    let app = base_ctx
        .app
        .clone()
        .layer(axum::extract::Extension(ChirpsFetcherHandle(
            std::sync::Arc::new(MapFetcher(archive)),
        )));

    // --- Monthly fetch Jan..Mar: two fetched, one failed.
    let body = json!({
        "start_year": 2024, "end_year": 2024,
        "months": [1, 2, 3],
        "base_url": base,
    });
    let (status, bytes) = send(
        &app,
        "POST",
        "/api/drought-management/chirps/fetch",
        Some(body.clone()),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(outcome["fetched"].as_array().unwrap().len(), 2);
    assert!(outcome["already_present"].as_array().unwrap().is_empty());
    let failed = outcome["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0][0], "chirps-v2.0.2024.03.tif");
    assert!(failed[0][1].as_str().unwrap().contains("404"));

    // Files landed gunzipped and readable; the January product is a real
    // catalog product with the right month bounds.
    let jan_id = outcome["fetched"][0][1].as_str().unwrap();
    let jan = catalog::get_product(&base_ctx.pool, jan_id).await?.unwrap();
    assert_eq!(jan.temporal_start.as_deref(), Some("2024-01-01T00:00:00Z"));
    assert_eq!(jan.temporal_end.as_deref(), Some("2024-01-31T23:59:59Z"));
    let mut reader = GeoTiffReader::open(jan.path.as_deref().unwrap())?;
    assert_eq!(reader.read_band()?.to_f32(), vec![10.0; 4]);

    // --- Re-fetch resumes: everything already present, nothing downloaded.
    let (status, bytes) = send(
        &app,
        "POST",
        "/api/drought-management/chirps/fetch",
        Some(json!({
            "start_year": 2024, "end_year": 2024,
            "months": [1, 2],
            "base_url": base,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert!(again["fetched"].as_array().unwrap().is_empty());
    assert_eq!(again["already_present"].as_array().unwrap().len(), 2);
    assert_eq!(again["already_present"][0][1], json!(jan_id));

    // --- Dekad fetch registers dekad-bounded products.
    let (status, bytes) = send(
        &app,
        "POST",
        "/api/drought-management/chirps/fetch",
        Some(json!({
            "start_year": 2024, "end_year": 2024,
            "months": [6],
            "cadence": "dekad",
            "base_url": base,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let dekads: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(dekads["fetched"].as_array().unwrap().len(), 3);
    let d2_id = dekads["fetched"][1][1].as_str().unwrap();
    let d2 = catalog::get_product(&base_ctx.pool, d2_id).await?.unwrap();
    assert_eq!(d2.algorithm_id, "chirps.ingest.dekad");
    assert_eq!(d2.temporal_start.as_deref(), Some("2024-06-11T00:00:00Z"));
    assert_eq!(d2.temporal_end.as_deref(), Some("2024-06-20T23:59:59Z"));

    // --- Validation errors are reason-coded.
    for (bad, needle) in [
        (
            json!({"start_year": 2025, "end_year": 2024, "base_url": base}),
            "after",
        ),
        (
            json!({"start_year": 2024, "end_year": 2024, "months": [13], "base_url": base}),
            "1..=12",
        ),
        (
            json!({"start_year": 2024, "end_year": 2024, "cadence": "hourly", "base_url": base}),
            "cadence",
        ),
        (
            json!({"start_year": 1800, "end_year": 2024, "base_url": base}),
            "ceiling",
        ),
    ] {
        let (status, bytes) = send(
            &app,
            "POST",
            "/api/drought-management/chirps/fetch",
            Some(bad),
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            String::from_utf8_lossy(&bytes).contains(needle),
            "{needle}: {}",
            String::from_utf8_lossy(&bytes)
        );
    }
    Ok(())
}

#[tokio::test]
async fn dekads_register_but_stay_out_of_monthly_spi_records() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let chirps_dir = tmp.path().join("chirps");
    std::fs::create_dir_all(&chirps_dir)?;
    // Six monthly Junes (the SPI-1 fixture) PLUS three June-2024 dekads.
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
    for dekad in 1u8..=3 {
        std::fs::write(
            chirps_dir.join(format!("chirps-v2.0.2024.06.{dekad}.tif")),
            small_precip_tif(&tmp, 10.0)?,
        )?;
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
    assert_eq!(
        registered["registered"].as_array().unwrap().len(),
        9,
        "6 monthlies + 3 dekads"
    );
    let current = registered["registered"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry[0] == "chirps-v2.0.2025.06.tif")
        .unwrap()[1]
        .as_str()
        .unwrap()
        .to_string();

    // Monthly SPI must use exactly the six monthlies — dekads neither enter
    // the record nor pollute the skip report.
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
    assert_eq!(outcome["observations_used"].as_array().unwrap().len(), 6);
    assert!(outcome["observations_skipped"]
        .as_array()
        .unwrap()
        .is_empty());
    Ok(())
}
