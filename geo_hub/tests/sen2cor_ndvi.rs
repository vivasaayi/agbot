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
      <Geoposition resolution="20">
        <ULX>600000</ULX><ULY>1300020</ULY><XDIM>20</XDIM><YDIM>-20</YDIM>
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
    // Batch 29: green at 10 m, SWIR1 + narrow NIR at their native 20 m.
    write_jp2_gray(
        &img.join("T43PFN_20240601T051651_B03_10m.jp2"),
        8,
        8,
        &band_dn(7000),
    );
    let img20 = granule.join("IMG_DATA").join("R20m");
    std::fs::create_dir_all(&img20)?;
    write_jp2_gray(
        &img20.join("T43PFN_20240601T051651_B11_20m.jp2"),
        4,
        4,
        &[3000u16; 16],
    );
    write_jp2_gray(
        &img20.join("T43PFN_20240601T051651_B8A_20m.jp2"),
        4,
        4,
        &[7000u16; 16],
    );
    write_jp2_gray(
        &img20.join("T43PFN_20240601T051651_B12_20m.jp2"),
        4,
        4,
        &[3000u16; 16],
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
    let ndvi_id = outcome["index_product_id"].as_str().unwrap().to_string();

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
    assert_eq!(again["index_product_id"], ndvi_id);

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
    let legacy_id = legacy["index_product_id"].as_str().unwrap();
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

/// Batch 28: Sen2Cor's own SCL band masks clouds before the index — the
/// sen2cor parallel of HLS Fmask. The SCL is 20 m (4x4 against the 8x8
/// bands); its top-left cell is cloud (code 9), so after 2x nearest
/// block-replication the four top-left 10 m pixels are nodata. The SCL
/// product joins the lineage as `scl_mask`, `scl_applied` is recorded,
/// and the product id differs from an unmasked derivation.
#[tokio::test]
async fn scl_band_masks_clouds_before_the_index() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Fabricate the SAFE with an SCL band this time: 4x4 codes, cell 0 is
    // cloud-high (9), the rest vegetation (4).
    let input = tmp.path().join(L1C_NAME);
    std::fs::create_dir_all(&input)?;
    std::fs::write(input.join("MTD_MSIL1C.xml"), b"<l1c/>")?;
    let output_dir = tmp.path().join("l2a_out");
    fabricate_l2a(&output_dir)?;
    let granule = output_dir
        .join(L2A_NAME)
        .join("GRANULE")
        .join("L2A_T43PFN_A046739_20240601T051651");
    let scl_dir = granule.join("IMG_DATA").join("R20m");
    std::fs::create_dir_all(&scl_dir)?;
    let mut scl = vec![4u16; 16];
    scl[0] = 9;
    write_jp2_gray(
        &scl_dir.join("T43PFN_20240601T051651_SCL_20m.jp2"),
        4,
        4,
        &scl,
    );
    let config = Sen2CorConfig {
        command: vec![
            "noop".to_string(),
            "{input}".to_string(),
            "{output_dir}".to_string(),
        ],
    };
    run_sen2cor(&ctx.pool, &NoopRunner, &config, &input, &output_dir).await?;

    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/ndvi/derive",
        Some(json!({ "scene_id": SCENE_ID })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["scl_applied"], true);
    // 8x8 grid: SCL cell (0,0) covers 10 m pixels (0,0),(0,1),(1,0),(1,1);
    // the band fill pixel 0 overlaps one of them. 64 - 4 cloud = 60 valid.
    assert_eq!(outcome["valid_pixels"], 60);
    assert_eq!(outcome["invalid_pixels"], 4);
    let ndvi_id = outcome["index_product_id"].as_str().unwrap().to_string();

    let product = catalog::get_product(&ctx.pool, &ndvi_id).await?.unwrap();
    assert_eq!(product.parameters["scl_applied"], true);
    let values = {
        let mut reader = GeoTiffReader::open(product.path.as_deref().unwrap())?;
        reader.read_band()?.to_f32()
    };
    // Cloud-masked corner: pixels (0,0),(0,1),(1,0),(1,1) row-major.
    for masked in [0usize, 1, 8, 9] {
        assert_eq!(values[masked], NODATA, "pixel {masked} must be masked");
    }
    for (pixel, value) in values.iter().enumerate() {
        if ![0usize, 1, 8, 9].contains(&pixel) {
            assert!((value - 0.5).abs() < 1e-6, "pixel {pixel}: {value}");
        }
    }

    // The SCL product is in the lineage under its role.
    let edges = catalog::trace_inputs(&ctx.pool, &ndvi_id).await?;
    let scl_edge = edges
        .iter()
        .find(|edge| edge.role == "scl_mask")
        .expect("scl lineage edge");
    let scl_product = catalog::get_product(&ctx.pool, &scl_edge.input_product_id)
        .await?
        .unwrap();
    assert_eq!(scl_product.kind, "band_scl_20m");

    // On the native 20 m grid (NDMI) the SCL applies without replication:
    // exactly the one cloud cell is nodata.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/index/derive",
        Some(json!({ "scene_id": SCENE_ID, "index": "ndmi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["scl_applied"], true);
    assert_eq!(outcome["valid_pixels"], 15);
    assert_eq!(outcome["invalid_pixels"], 1);
    let ndmi = catalog::get_product(&ctx.pool, outcome["index_product_id"].as_str().unwrap())
        .await?
        .unwrap();
    let values = {
        let mut reader = GeoTiffReader::open(ndmi.path.as_deref().unwrap())?;
        reader.read_band()?.to_f32()
    };
    assert_eq!(values[0], NODATA, "native-resolution SCL cloud cell");
    assert!((values[1] - 0.5).abs() < 1e-6);

    Ok(())
}

/// Batch 29: MNDWI and NDMI derive from the same scene across resolutions.
/// MNDWI mixes 10 m green (B03) with 20 m SWIR1 (B11, block-replicated 2x)
/// on the 10 m grid; NDMI runs natively on the 20 m grid (B8A + B11) with
/// the 20 m geoposition. Baseline-04.00 DN: B03 7000 -> 0.6, B11 3000 ->
/// 0.2, B8A 7000 -> 0.6, so MNDWI = (0.6-0.2)/0.8 = 0.5 and NDMI =
/// (0.6-0.2)/0.8 = 0.5. Unknown indices are refused.
#[tokio::test]
async fn mndwi_and_ndmi_derive_across_resolutions() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    register_scene(&ctx, &tmp).await?;

    // --- MNDWI on the 10 m grid with the 20 m SWIR replicated.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/index/derive",
        Some(json!({ "scene_id": SCENE_ID, "index": "mndwi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["index"], "mndwi");
    let mndwi = catalog::get_product(&ctx.pool, outcome["index_product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(mndwi.kind, "mndwi");
    assert_eq!(mndwi.gsd_m_per_px, Some(10.0));
    assert_eq!(mndwi.parameters["resolution_m"], 10);
    let mut reader = GeoTiffReader::open(mndwi.path.as_deref().unwrap())?;
    assert_eq!(reader.info().geo_transform, Some(TRANSFORM));
    let values = reader.read_band()?.to_f32();
    assert_eq!(values.len(), 64);
    assert_eq!(values[0], NODATA, "B03 fill pixel is nodata");
    for value in &values[1..] {
        assert!((value - 0.5).abs() < 1e-6, "MNDWI must be 0.5, got {value}");
    }
    // Lineage roles are the index roles, not hardcoded red/nir.
    let edges = catalog::trace_inputs(&ctx.pool, &mndwi.product_id).await?;
    let mut roles: Vec<&str> = edges.iter().map(|e| e.role.as_str()).collect();
    roles.sort_unstable();
    assert_eq!(roles, vec!["green", "swir1"]);

    // --- NDMI natively on the 20 m grid.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/index/derive",
        Some(json!({ "scene_id": SCENE_ID, "index": "ndmi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    let ndmi = catalog::get_product(&ctx.pool, outcome["index_product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(ndmi.kind, "ndmi");
    assert_eq!(ndmi.gsd_m_per_px, Some(20.0));
    let mut reader = GeoTiffReader::open(ndmi.path.as_deref().unwrap())?;
    assert_eq!(
        reader.info().geo_transform,
        Some([600_000.0, 20.0, 0.0, 1_300_020.0, 0.0, -20.0]),
        "NDMI must carry the 20 m geoposition"
    );
    let values = reader.read_band()?.to_f32();
    assert_eq!(values.len(), 16);
    for value in &values {
        assert!((value - 0.5).abs() < 1e-6, "NDMI must be 0.5, got {value}");
    }

    // --- NBR natively on the 20 m grid (batch 30; feeds dNBR):
    // (B8A 0.6 - B12 0.2) / 0.8 = 0.5.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/index/derive",
        Some(json!({ "scene_id": SCENE_ID, "index": "nbr" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    let nbr = catalog::get_product(&ctx.pool, outcome["index_product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(nbr.kind, "nbr");
    assert_eq!(nbr.gsd_m_per_px, Some(20.0));
    let values = {
        let mut reader = GeoTiffReader::open(nbr.path.as_deref().unwrap())?;
        reader.read_band()?.to_f32()
    };
    for value in &values {
        assert!((value - 0.5).abs() < 1e-6, "NBR must be 0.5, got {value}");
    }
    let edges = catalog::trace_inputs(&ctx.pool, &nbr.product_id).await?;
    let mut roles: Vec<&str> = edges.iter().map(|e| e.role.as_str()).collect();
    roles.sort_unstable();
    assert_eq!(roles, vec!["nir", "swir2"]);

    // --- NDWI at 10 m (batch 30): green 0.6, NIR 0.6 -> exactly 0.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/index/derive",
        Some(json!({ "scene_id": SCENE_ID, "index": "ndwi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    let ndwi = catalog::get_product(&ctx.pool, outcome["index_product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(ndwi.kind, "ndwi");
    assert_eq!(ndwi.gsd_m_per_px, Some(10.0));
    let values = {
        let mut reader = GeoTiffReader::open(ndwi.path.as_deref().unwrap())?;
        reader.read_band()?.to_f32()
    };
    assert_eq!(values[0], NODATA, "fill pixel is nodata");
    for value in &values[1..] {
        assert!(value.abs() < 1e-6, "NDWI must be 0, got {value}");
    }

    // Unknown index is a caller error.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/ingest/sen2cor/index/derive",
        Some(json!({ "scene_id": SCENE_ID, "index": "evi9" })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}
