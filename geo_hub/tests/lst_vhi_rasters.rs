//! End-to-end test of the LST -> TCI -> VHI path (satellite pipeline
//! batch 23): a thermal DN GeoTIFF + radiometric calibration registers as an
//! `lst` L2 via POST /api/thermal/lst/derive; a multi-year archive of those
//! scores into a TCI drought L3 through the existing
//! /api/drought-management/rasters/derive (which already maps lst -> tci);
//! and POST /api/drought-management/vhi/derive blends a same-grid VCI + TCI
//! into a VHI L3 with lineage to both components.
//!
//! Physics fixture: ml = 0.001, al = 0, K1 = 1, K2 = 300, emissivity 1.0.
//! Then LST = TB = 300 / ln(1 + 1000/DN) exactly, so DN chosen from the
//! inverse formula gives hand-picked Kelvin values:
//!   2024 -> 290 K, 2025 -> 310 K, current 2026 -> 300 K
//!   TCI = 100·(310 − 300)/(310 − 290) = 50.
//! NDVI years 0.2 / 0.6 / current 0.4 give VCI = 50 (batch-8 fixture), so
//! VHI(α = 0.5) = 50 everywhere both components are valid. Thermal pixel 0
//! is fill in every year, so TCI and VHI are nodata there while VCI is not.

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
const NODATA: f32 = -9999.0;

struct Ctx {
    app: Router,
    pool: geo_hub::db::DbPool,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("lst_vhi.db").display()
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

/// DN whose radiance gives exactly `kelvin` under the fixture calibration:
/// TB = 300/ln(1 + 1000/DN) = kelvin  =>  DN = 1000/(e^(300/kelvin) − 1).
fn dn_for_kelvin(kelvin: f64) -> f32 {
    (1000.0 / ((300.0 / kelvin).exp() - 1.0)) as f32
}

fn coefficients() -> serde_json::Value {
    json!({ "ml": 0.001, "al": 0.0, "k1": 1.0, "k2": 300.0, "lambda_um": 10.895 })
}

/// Write a 4x4 thermal DN GeoTIFF with pixel 0 as fill.
fn write_thermal(tmp: &TempDir, name: &str, dn: f32) -> Result<String> {
    let mut values = vec![dn; 16];
    values[0] = NODATA;
    let path = tmp.path().join(name);
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
    Ok(path.to_string_lossy().to_string())
}

/// Derive + register one `lst` L2 for a June acquisition.
async fn derive_lst(ctx: &Ctx, tmp: &TempDir, year: i32, kelvin: f64) -> Result<String> {
    let thermal = write_thermal(tmp, &format!("thermal_{year}.tif"), dn_for_kelvin(kelvin))?;
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/thermal/lst/derive",
        Some(json!({
            "thermal_tif": thermal,
            "scene_id": format!("L8-{year}-06"),
            "acquired_on": format!("{year}-06-14"),
            "coefficients": coefficients(),
            "emissivity_constant": 1.0,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    Ok(outcome["lst_product_id"].as_str().unwrap().to_string())
}

/// Write a 4x4 NDVI GeoTIFF and register it as an L2 (batch-8 fixture).
async fn register_ndvi(ctx: &Ctx, tmp: &TempDir, year: i32, value: f32) -> Result<String> {
    let path = tmp.path().join(format!("ndvi_{year}.tif"));
    write_geotiff_f32(
        &path,
        4,
        4,
        &vec![value; 16],
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "test.lst_vhi".to_string(),
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

fn read_values(path: &str) -> Result<Vec<f32>> {
    let mut reader = GeoTiffReader::open(path)?;
    Ok(reader.read_band()?.to_f32())
}

#[tokio::test]
async fn lst_scores_into_tci_and_blends_with_vci_into_vhi() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // --- LST L2: DN calibrated for 300 K, unit emissivity.
    let lst_current = derive_lst(&ctx, &tmp, 2026, 300.0).await?;
    let product = catalog::get_product(&ctx.pool, &lst_current)
        .await?
        .expect("lst registered");
    assert_eq!(product.kind, "lst");
    assert_eq!(product.level, ProductLevel::L2);
    assert_eq!(product.parameters["emissivity_method"], "constant");
    assert_eq!(product.parameters["unit"], "kelvin");
    let lst_values = read_values(product.path.as_deref().unwrap())?;
    assert_eq!(lst_values[0], NODATA, "thermal fill pixel stays nodata");
    for value in &lst_values[1..] {
        assert!(
            (value - 300.0).abs() < 1e-2,
            "LST must be 300 K, got {value}"
        );
    }

    // Idempotent: same request re-registers the same content-addressed id.
    let again = derive_lst(&ctx, &tmp, 2026, 300.0).await?;
    assert_eq!(again, lst_current);

    // NDVI-driven emissivity is a distinct product with NDVI lineage.
    let ndvi_current = register_ndvi(&ctx, &tmp, 2026, 0.4).await?;
    let thermal = write_thermal(&tmp, "thermal_ndvi_eps.tif", dn_for_kelvin(300.0))?;
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/thermal/lst/derive",
        Some(json!({
            "thermal_tif": thermal,
            "scene_id": "L8-2026-06",
            "acquired_on": "2026-06-14",
            "coefficients": coefficients(),
            "ndvi_product_id": ndvi_current,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["emissivity_method"], "ndvi_thresholds");
    let lst_ndvi_eps = outcome["lst_product_id"].as_str().unwrap().to_string();
    assert_ne!(lst_ndvi_eps, lst_current);
    // Sub-unit emissivity (NDVI 0.4 -> eps ~0.9789) reads warmer than TB.
    let eps_product = catalog::get_product(&ctx.pool, &lst_ndvi_eps)
        .await?
        .unwrap();
    let eps_values = read_values(eps_product.path.as_deref().unwrap())?;
    assert!(eps_values[1] > 300.1, "got {}", eps_values[1]);
    let edges = catalog::trace_inputs(&ctx.pool, &lst_ndvi_eps).await?;
    assert!(
        edges.iter().any(|e| e.input_product_id == ndvi_current),
        "emissivity NDVI must be a lineage input"
    );

    // Both ndvi_product_id and emissivity_constant is a caller error.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/thermal/lst/derive",
        Some(json!({
            "thermal_tif": write_thermal(&tmp, "thermal_conflict.tif", 500.0)?,
            "scene_id": "L8-2026-06",
            "acquired_on": "2026-06-14",
            "coefficients": coefficients(),
            "ndvi_product_id": ndvi_current,
            "emissivity_constant": 0.97,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // --- TCI: the lst kind lights up the existing drought raster path.
    derive_lst(&ctx, &tmp, 2024, 290.0).await?;
    derive_lst(&ctx, &tmp, 2025, 310.0).await?;
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/rasters/derive",
        Some(json!({
            "current_product_id": lst_current,
            "field_id": "field-1",
            "season_id": "2026-kharif",
            "min_years": 2,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["drought_index_kind"], "tci");
    let tci_id = outcome["drought_product_id"].as_str().unwrap().to_string();
    let tci_path = outcome["drought_artifact"].as_str().unwrap().to_string();
    let tci_values = read_values(&tci_path)?;
    assert_eq!(tci_values[0], NODATA, "no thermal history at fill pixel");
    for value in &tci_values[1..] {
        // TCI = 100·(310 − 300)/(310 − 290) = 50.
        assert!((value - 50.0).abs() < 0.5, "TCI must be ~50, got {value}");
    }

    // --- VCI from the NDVI archive (0.2 / 0.6 baseline, current 0.4).
    register_ndvi(&ctx, &tmp, 2024, 0.2).await?;
    register_ndvi(&ctx, &tmp, 2025, 0.6).await?;
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/rasters/derive",
        Some(json!({
            "current_product_id": ndvi_current,
            "field_id": "field-1",
            "season_id": "2026-kharif",
            "min_years": 2,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["drought_index_kind"], "vci");
    let vci_id = outcome["drought_product_id"].as_str().unwrap().to_string();

    // --- VHI blend of the two registered components.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/vhi/derive",
        Some(json!({
            "vci_product_id": vci_id,
            "tci_product_id": tci_id,
            "field_id": "field-1",
            "season_id": "2026-kharif",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["alpha"], 0.5);
    let vhi_id = outcome["vhi_product_id"].as_str().unwrap().to_string();
    let vhi_values = read_values(outcome["vhi_artifact"].as_str().unwrap())?;
    // Pixel 0: VCI valid but TCI nodata -> component-invalid nodata.
    assert_eq!(vhi_values[0], NODATA);
    for value in &vhi_values[1..] {
        // VHI = 0.5·50 + 0.5·50 = 50.
        assert!((value - 50.0).abs() < 0.5, "VHI must be ~50, got {value}");
    }
    // 15 of 16 pixels valid, all no_drought (>= 40).
    assert_eq!(outcome["valid_fraction"].as_f64().unwrap(), 15.0 / 16.0);
    assert_eq!(outcome["severity_counts"]["no_drought"], 15);
    assert_eq!(outcome["severity_counts"]["invalid"], 1);

    // The VHI L3 carries lineage to both components.
    let vhi = catalog::get_product(&ctx.pool, &vhi_id)
        .await?
        .expect("vhi");
    assert_eq!(vhi.kind, "drought_index");
    assert_eq!(vhi.level, ProductLevel::L3);
    assert_eq!(vhi.parameters["index_kind"], "vhi");
    let edges = catalog::trace_inputs(&ctx.pool, &vhi_id).await?;
    let inputs: Vec<&str> = edges.iter().map(|e| e.input_product_id.as_str()).collect();
    assert!(inputs.contains(&vci_id.as_str()), "vci in lineage");
    assert!(inputs.contains(&tci_id.as_str()), "tci in lineage");

    // Component misuse is refused: an lst L2 is not a tci component.
    let (status, body) = send(
        &ctx.app,
        "POST",
        "/api/drought-management/vhi/derive",
        Some(json!({
            "vci_product_id": vci_id,
            "tci_product_id": lst_current,
            "field_id": "field-1",
            "season_id": "2026-kharif",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    Ok(())
}
