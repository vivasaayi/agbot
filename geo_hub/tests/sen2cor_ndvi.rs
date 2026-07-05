//! End-to-end test of local Sen2Cor NDVI derivation (satellite pipeline
//! batch 24): a registered Sen2Cor L2A whose band artifacts are REAL
//! JPEG 2000 files (encoded by the reference OpenJPEG encoder via
//! `raster_io::test_util::write_jp2_gray`) derives an `ndvi` L2 GeoTIFF
//! through POST /api/ingest/sen2cor/ndvi/derive — JP2 decode, baseline
//! calibration, MTD_TL.xml geocoding, and lineage to both band products.
//!
//! Fixture: 8x8 bands on the 43PFN 10 m grid. Baseline >= 04.00 DN:
//! red 3000 -> (3000-1000)/10000 = 0.2, NIR 7000 -> 0.6, so NDVI = 0.5.
//! Pixel 0 is fill (DN 0) in both bands -> nodata. Legacy calibration of
//! the same DN gives 0.3/0.7 -> NDVI = 0.4 (a distinct product).

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog;
use geo_hub::sen2cor::{run_sen2cor, ProcessOutput, ProcessRunner, Sen2CorConfig};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use raster_io::{test_util::write_jp2_gray, GeoTiffReader};
use serde_json::json;
use shared::product_graph::ProductLevel;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const L1C_NAME: &str = "S2A_MSIL1C_20240601T051651_N0510_R062_T43PFN_20240601T072649.SAFE";
const L2A_NAME: &str = "S2A_MSIL2A_20240601T051651_N0510_R062_T43PFN_20240601T080000.SAFE";
const SCENE_ID: &str = "S2A_MSIL1C_20240601T051651_N0510_R062_T43PFN_20240601T072649";
const NODATA: f32 = -9999.0;
const TRANSFORM: [f64; 6] = [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];

const TILE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<n1:Level-2A_Tile_ID>
  <n1:Geometric_Info>
    <Tile_Geocoding metadataLevel="Brief">
      <HORIZONTAL_CS_CODE>EPSG:32643</HORIZONTAL_CS_CODE>
      <Geoposition resolution="10">
        <ULX>600000</ULX><ULY>1300020</ULY><XDIM>10</XDIM><YDIM>-10</YDIM>
      </Geoposition>
    </Tile_Geocoding>
  </n1:Geometric_Info>
</n1:Level-2A_Tile_ID>"#;

struct Ctx {
    app: Router,
    pool: geo_hub::db::DbPool,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("sen2cor_ndvi.db").display()
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

/// 8x8 DN band: `dn` everywhere except pixel 0 = 0 (Sentinel fill).
fn band_dn(dn: u16) -> Vec<u16> {
    let mut values = vec![dn; 64];
    values[0] = 0;
    values
}

/// Fabricate the L2A SAFE with real JP2 bands + tile metadata.
fn fabricate_l2a(output_dir: &Path) -> Result<()> {
    let granule = output_dir
        .join(L2A_NAME)
        .join("GRANULE")
        .join("L2A_T43PFN_A046739_20240601T051651");
    let img = granule.join("IMG_DATA").join("R10m");
    std::fs::create_dir_all(&img)?;
    write_jp2_gray(
        &img.join("T43PFN_20240601T051651_B04_10m.jp2"),
        8,
        8,
        &band_dn(3000),
    );
    write_jp2_gray(
        &img.join("T43PFN_20240601T051651_B08_10m.jp2"),
        8,
        8,
        &band_dn(7000),
    );
    std::fs::write(granule.join("MTD_TL.xml"), TILE_XML)?;
    std::fs::write(output_dir.join(L2A_NAME).join("MTD_MSIL2A.xml"), b"<l2a/>")?;
    Ok(())
}

/// Runner that succeeds without executing anything (the SAFE is
/// pre-fabricated in the output dir).
struct NoopRunner;

impl ProcessRunner for NoopRunner {
    fn run(&self, _program: &str, _args: &[String]) -> Result<ProcessOutput, std::io::Error> {
        Ok(ProcessOutput {
            status: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

/// Register the scene: L1C input + pre-fabricated L2A through run_sen2cor.
async fn register_scene(ctx: &Ctx, tmp: &TempDir) -> Result<PathBuf> {
    let input = tmp.path().join(L1C_NAME);
    std::fs::create_dir_all(&input)?;
    std::fs::write(input.join("MTD_MSIL1C.xml"), b"<l1c/>")?;
    let output_dir = tmp.path().join("l2a_out");
    std::fs::create_dir_all(&output_dir)?;
    fabricate_l2a(&output_dir)?;
    let config = Sen2CorConfig {
        command: vec![
            "noop".to_string(),
            "{input}".to_string(),
            "{output_dir}".to_string(),
        ],
    };
    let outcome = run_sen2cor(&ctx.pool, &NoopRunner, &config, &input, &output_dir).await?;
    assert_eq!(outcome.scene_id, SCENE_ID);
    Ok(output_dir)
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
async fn sen2cor_jp2_bands_derive_local_ndvi_with_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    register_scene(&ctx, &tmp).await?;

    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/ndvi/derive",
        Some(json!({ "scene_id": SCENE_ID })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["sensor_profile"], "sentinel2_l2a_baseline_0400");
    assert_eq!(outcome["valid_pixels"], 63);
    assert_eq!(outcome["invalid_pixels"], 1);
    let ndvi_id = outcome["ndvi_product_id"].as_str().unwrap().to_string();

    // Registered as an ndvi L2 on the MTD_TL grid, scoped to the scene.
    let product = catalog::get_product(&ctx.pool, &ndvi_id)
        .await?
        .expect("ndvi registered");
    assert_eq!(product.kind, "ndvi");
    assert_eq!(product.level, ProductLevel::L2);
    assert_eq!(product.scene_id.as_deref(), Some(SCENE_ID));
    assert_eq!(product.source_id.as_deref(), Some("sen2cor:l2a"));
    assert_eq!(product.gsd_m_per_px, Some(10.0));

    // The GeoTIFF carries the MTD_TL geocoding and the hand-computed NDVI:
    // (0.6 - 0.2) / (0.6 + 0.2) = 0.5, fill pixel nodata.
    let mut reader = GeoTiffReader::open(product.path.as_deref().unwrap())?;
    assert_eq!(reader.info().epsg, Some(32643));
    assert_eq!(reader.info().geo_transform, Some(TRANSFORM));
    let values = reader.read_band()?.to_f32();
    assert_eq!(values.len(), 64);
    assert_eq!(values[0], NODATA, "fill DN pixel is nodata");
    for value in &values[1..] {
        assert!((value - 0.5).abs() < 1e-6, "NDVI must be 0.5, got {value}");
    }

    // Lineage to both JP2 band L1 products.
    let edges = catalog::trace_inputs(&ctx.pool, &ndvi_id).await?;
    let mut roles: Vec<(&str, &str)> = edges
        .iter()
        .map(|e| (e.role.as_str(), e.input_product_id.as_str()))
        .collect();
    roles.sort();
    assert_eq!(roles.len(), 2);
    assert_eq!(roles[0].0, "nir");
    assert_eq!(roles[1].0, "red");
    for (_, band_id) in &roles {
        let band = catalog::get_product(&ctx.pool, band_id).await?.unwrap();
        assert_eq!(band.level, ProductLevel::L1);
        assert!(band.kind.starts_with("band_b0"), "{}", band.kind);
    }

    // Idempotent.
    let (status, again) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/ndvi/derive",
        Some(json!({ "scene_id": SCENE_ID })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["ndvi_product_id"], ndvi_id);

    // Legacy calibration is a DISTINCT product with different values:
    // 3000/10000 = 0.3, 7000/10000 = 0.7 -> NDVI = 0.4.
    let (status, legacy) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/ndvi/derive",
        Some(json!({ "scene_id": SCENE_ID, "baseline_ge_0400": false })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{legacy}");
    assert_eq!(legacy["sensor_profile"], "sentinel2_l2a_legacy");
    let legacy_id = legacy["ndvi_product_id"].as_str().unwrap();
    assert_ne!(legacy_id, ndvi_id);
    let legacy_product = catalog::get_product(&ctx.pool, legacy_id).await?.unwrap();
    let mut reader = GeoTiffReader::open(legacy_product.path.as_deref().unwrap())?;
    let values = reader.read_band()?.to_f32();
    assert!(
        (values[1] - 0.4).abs() < 1e-6,
        "legacy NDVI 0.4, got {}",
        values[1]
    );

    // Unknown scene is a 404, not a server error.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/ndvi/derive",
        Some(json!({ "scene_id": "S2A_MSIL1C_NOPE" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}
