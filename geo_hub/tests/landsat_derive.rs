//! End-to-end test of local Landsat Collection-2 derivation (satellite
//! pipeline batch 35): registered `band_*` L1 GeoTIFFs (u16 DN) derive an
//! `ndvi` L2 (C2L2 SR calibration `DN*0.0000275 - 0.2`) and an `lst` L2
//! (C2L2 ST calibration `DN*0.00341802 + 149` Kelvin), each masked by the
//! scene's QA_PIXEL band with lineage to every input band.
//!
//! Fixture DN (2x2): red 14545 -> ~0.2, NIR 29091 -> ~0.6 => NDVI ~0.5;
//! ST 44177 -> ~300.0 K. QA_PIXEL: pixel 1 is cloud (bit 3), the rest
//! clear (bit 6) -> pixel 1 nodata in both products.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog;
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use raster_io::{write_geotiff_u16, GeoTiffReader, GeoTiffTags};
use serde_json::json;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const EPSG: u32 = 32643;
// Landsat 30 m grid.
const TRANSFORM: [f64; 6] = [600_000.0, 30.0, 0.0, 1_300_060.0, 0.0, -30.0];
const NODATA: f32 = -9999.0;
const SCENE_ID: &str = "LC08_L2SP_144051_20240601_02_T1";

struct Ctx {
    app: Router,
    pool: geo_hub::db::DbPool,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("landsat_derive.db").display()
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

async fn register_band(ctx: &Ctx, tmp: &TempDir, kind: &str, dn: Vec<u16>) -> Result<String> {
    register_band_for(ctx, tmp, SCENE_ID, kind, dn).await
}

async fn register_band_for(
    ctx: &Ctx,
    tmp: &TempDir,
    scene_id: &str,
    kind: &str,
    dn: Vec<u16>,
) -> Result<String> {
    register_band_sized_for(ctx, tmp, scene_id, kind, dn, 2, 2).await
}

async fn register_band_sized(
    ctx: &Ctx,
    tmp: &TempDir,
    kind: &str,
    dn: Vec<u16>,
    width: u32,
    height: u32,
) -> Result<String> {
    register_band_sized_for(ctx, tmp, SCENE_ID, kind, dn, width, height).await
}

async fn register_band_sized_for(
    ctx: &Ctx,
    tmp: &TempDir,
    scene_id: &str,
    kind: &str,
    dn: Vec<u16>,
    width: u32,
    height: u32,
) -> Result<String> {
    register_band_sized_for_variant(
        ctx,
        tmp,
        scene_id,
        kind,
        dn,
        width,
        height,
        "original",
        "2026-07-05T00:00:00Z",
    )
    .await
}

async fn register_band_sized_for_variant(
    ctx: &Ctx,
    tmp: &TempDir,
    scene_id: &str,
    kind: &str,
    dn: Vec<u16>,
    width: u32,
    height: u32,
    variant: &str,
    created_at: &str,
) -> Result<String> {
    let path = tmp.path().join(format!("{scene_id}_{kind}_{variant}.tif"));
    write_geotiff_u16(
        &path,
        width,
        height,
        &dn,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: None,
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L1,
        kind: kind.to_string(),
        algorithm_id: "usgs.landsat.surface_reflectance".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "scene_id": scene_id, "band": kind, "variant": variant }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(scene_id.to_string()),
            temporal_start: "2024-06-01T05:16:51Z".to_string(),
            temporal_end: "2024-06-01T05:16:51Z".to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(30.0),
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
        source_id: Some("usgs-landsat".to_string()),
    };
    Ok(catalog::register_product(&ctx.pool, &draft, created_at).await?)
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
async fn landsat_bands_derive_qa_masked_ndvi_and_lst() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    let red = register_band(&ctx, &tmp, "band_sr_b4", vec![14545; 4]).await?;
    let nir = register_band(&ctx, &tmp, "band_sr_b5", vec![29091; 4]).await?;
    let st = register_band(&ctx, &tmp, "band_st_b10", vec![44177; 4]).await?;
    // QA_PIXEL: pixel 1 cloud (bit 3 = 8), rest clear (bit 6 = 64).
    let qa = register_band(&ctx, &tmp, "band_qa_pixel", vec![64, 8, 64, 64]).await?;

    // --- NDVI: (0.6 - 0.2)/(0.6 + 0.2) = 0.5, cloud pixel masked.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": SCENE_ID })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["product"], "ndvi");
    assert_eq!(outcome["qa_applied"], true);
    assert_eq!(outcome["valid_pixels"], 3);
    let ndvi = catalog::get_product(&ctx.pool, outcome["product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(ndvi.kind, "ndvi");
    assert_eq!(ndvi.gsd_m_per_px, Some(30.0));
    assert_eq!(ndvi.parameters["sensor_profile"], "landsat_c2l2_sr");
    let values = {
        let mut reader = GeoTiffReader::open(ndvi.path.as_deref().unwrap())?;
        assert_eq!(reader.info().geo_transform, Some(TRANSFORM));
        reader.read_band()?.to_f32()
    };
    assert_eq!(values[1], NODATA, "cloud pixel masked");
    for pixel in [0usize, 2, 3] {
        assert!(
            (values[pixel] - 0.5).abs() < 1e-3,
            "NDVI ~0.5, got {}",
            values[pixel]
        );
    }
    let edges = catalog::trace_inputs(&ctx.pool, &ndvi.product_id).await?;
    let mut roles: Vec<(&str, &str)> = edges
        .iter()
        .map(|e| (e.role.as_str(), e.input_product_id.as_str()))
        .collect();
    roles.sort_unstable();
    assert_eq!(
        roles,
        vec![
            ("nir", nir.as_str()),
            ("qa_pixel", qa.as_str()),
            ("red", red.as_str())
        ]
    );

    // --- LST: 44177 * 0.00341802 + 149 = ~300.0 K, cloud pixel masked.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": SCENE_ID, "product": "lst" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["product"], "lst");
    let lst = catalog::get_product(&ctx.pool, outcome["product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(lst.kind, "lst");
    assert_eq!(lst.parameters["unit"], "kelvin");
    let values = {
        let mut reader = GeoTiffReader::open(lst.path.as_deref().unwrap())?;
        reader.read_band()?.to_f32()
    };
    assert_eq!(values[1], NODATA);
    for pixel in [0usize, 2, 3] {
        assert!(
            (values[pixel] - 300.0).abs() < 0.01,
            "LST ~300 K, got {}",
            values[pixel]
        );
    }
    let edges = catalog::trace_inputs(&ctx.pool, &lst.product_id).await?;
    assert!(edges
        .iter()
        .any(|e| e.role == "surface_temperature" && e.input_product_id == st));

    // Unknown product / missing scene are reason-coded.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": SCENE_ID, "product": "evi9" })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": "LC08_NOPE" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn landsat_derivation_uses_the_newest_registered_band_revision() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    let old_red = register_band(&ctx, &tmp, "band_sr_b4", vec![14545; 4]).await?;
    let new_red = register_band_sized_for_variant(
        &ctx,
        &tmp,
        SCENE_ID,
        "band_sr_b4",
        vec![29091; 4],
        2,
        2,
        "corrected",
        "2026-07-06T00:00:00Z",
    )
    .await?;
    register_band(&ctx, &tmp, "band_sr_b5", vec![29091; 4]).await?;

    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": SCENE_ID, "product": "ndvi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    let product = catalog::get_product(&ctx.pool, outcome["product_id"].as_str().unwrap())
        .await?
        .unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, &product.product_id).await?;
    assert!(edges
        .iter()
        .any(|edge| edge.role == "red" && edge.input_product_id == new_red));
    assert!(!edges
        .iter()
        .any(|edge| edge.role == "red" && edge.input_product_id == old_red));
    let mut reader = GeoTiffReader::open(product.path.as_deref().unwrap())?;
    assert!(reader
        .read_band()?
        .to_f32()
        .iter()
        .all(|value| value.abs() < 1e-3));
    Ok(())
}

/// Batch 38 (Landsat parity): the spec table derives the water/moisture/
/// burn/enhanced-vegetation indices from the already-ingested OLI bands —
/// hand-computed MNDWI/NBR 0.5 and EVI ~0.4124 — and the ST_QA band turns
/// into an honest LST confidence (low-uncertainty fraction).
#[tokio::test]
async fn parity_indices_and_st_qa_confidence_derive_from_oli_bands() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    register_band(&ctx, &tmp, "band_sr_b2", vec![9091; 4]).await?; // blue ~0.05
    register_band(&ctx, &tmp, "band_sr_b3", vec![29091; 4]).await?; // green ~0.6
    register_band(&ctx, &tmp, "band_sr_b4", vec![14545; 4]).await?; // red ~0.2
    register_band(&ctx, &tmp, "band_sr_b5", vec![29091; 4]).await?; // nir ~0.6
    register_band(&ctx, &tmp, "band_sr_b6", vec![14545; 4]).await?; // swir1 ~0.2
    register_band(&ctx, &tmp, "band_sr_b7", vec![14545; 4]).await?; // swir2 ~0.2
    register_band(&ctx, &tmp, "band_st_b10", vec![44177; 4]).await?; // ~300 K
    register_band(&ctx, &tmp, "band_qa_pixel", vec![64, 8, 64, 64]).await?;
    // ST_QA (Kelvin*100): pixels 0/1/3 at 1.5 K, pixel 2 at 3.5 K.
    register_band(&ctx, &tmp, "band_st_qa", vec![150, 150, 350, 150]).await?;

    // MNDWI = (0.6-0.2)/0.8 = 0.5; NBR likewise; EVI =
    // 2.5*(0.6-0.2)/(0.6 + 6*0.2 - 7.5*0.05 + 1) ~= 0.4124.
    for (product, expected) in [
        ("mndwi", 0.5f32),
        ("nbr", 0.5),
        ("ndmi", 0.5),
        ("evi", 0.4124),
    ] {
        let (status, outcome) = send(
            &ctx.app,
            "POST",
            "/api/ingest/landsat/derive",
            Some(json!({ "scene_id": SCENE_ID, "product": product })),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{product}: {outcome}");
        assert_eq!(outcome["instrument"], "oli");
        assert_eq!(outcome["qa_applied"], true);
        let registered = catalog::get_product(&ctx.pool, outcome["product_id"].as_str().unwrap())
            .await?
            .unwrap();
        assert_eq!(registered.kind, product);
        let values = {
            let mut reader = GeoTiffReader::open(registered.path.as_deref().unwrap())?;
            reader.read_band()?.to_f32()
        };
        assert_eq!(values[1], NODATA, "{product}: cloud pixel masked");
        for pixel in [0usize, 2, 3] {
            assert!(
                (values[pixel] - expected).abs() < 1e-3,
                "{product} pixel {pixel}: {} != {expected}",
                values[pixel]
            );
        }
    }

    // LST with ST_QA: computed pixels are 0/2/3 (pixel 1 is cloud);
    // uncertainties 1.5/3.5/1.5 K -> low-uncertainty fraction 2/3, recorded
    // as the product confidence with the ST_QA product in the lineage.
    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": SCENE_ID, "product": "lst" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    let fraction = outcome["st_qa_low_uncertainty_fraction"].as_f64().unwrap();
    assert!((fraction - 2.0 / 3.0).abs() < 1e-6, "{fraction}");
    let lst = catalog::get_product(&ctx.pool, outcome["product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert!((lst.confidence.unwrap() - 2.0 / 3.0).abs() < 1e-6);
    let edges = catalog::trace_inputs(&ctx.pool, &lst.product_id).await?;
    assert!(edges.iter().any(|e| e.role == "st_qa"));

    Ok(())
}

/// Batch 38: TM/ETM+ scenes resolve through their own band numbering —
/// SR_B3/SR_B4 are red/NIR on Landsat 5 (they would be green/red on OLI),
/// unlocking the pre-2013 archive with the same C2 calibration.
#[tokio::test]
async fn tm_scene_resolves_red_nir_through_its_own_band_numbering() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let scene = "LT05_L2SP_144051_19950601_02_T1";

    register_band_for(&ctx, &tmp, scene, "band_sr_b3", vec![14545; 4]).await?; // TM red
    register_band_for(&ctx, &tmp, scene, "band_sr_b4", vec![29091; 4]).await?; // TM nir

    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": scene, "product": "ndvi" })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["instrument"], "tm_etm");
    assert_eq!(outcome["qa_applied"], false);
    let ndvi = catalog::get_product(&ctx.pool, outcome["product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(ndvi.parameters["instrument"], "tm_etm");
    let values = {
        let mut reader = GeoTiffReader::open(ndvi.path.as_deref().unwrap())?;
        reader.read_band()?.to_f32()
    };
    for value in &values {
        assert!((value - 0.5).abs() < 1e-3, "TM NDVI 0.5, got {value}");
    }

    // The same scene refuses an OLI-only thermal request: TM thermal is
    // ST_B6, and no band matches.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/ingest/landsat/derive",
        Some(json!({ "scene_id": scene, "product": "lst" })),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

/// Batch 40: the water-availability demand side. LST + NDVI (both from
/// the Landsat local derive on this scene) feed the Ts-VI triangle with
/// self-calibrated edges. Fixture (4x2): pixels 0-3 bare (NDVI ~0.1),
/// pixels 4-7 vegetated (~0.8); LST [310, 300, 305, 290 | 300, 290,
/// 295, 292.5] K -> wet edge 290; bare dry edge 310, vegetated 300 ->
/// fractions [0, 0.5, 0.25, 1 | 0, 1, 0.5, 0.75].
#[tokio::test]
async fn landsat_lst_and_ndvi_pair_derives_the_triangle_et_fraction() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // DN for the target reflectances/temperatures (SR: DN*0.0000275-0.2;
    // ST: DN*0.00341802+149). Bare: red 0.45 (23636) / nir 0.55 (27273)
    // -> NDVI 0.1; vegetated: red 0.05 (9091) / nir 0.45 (23636) -> 0.8.
    register_band_sized(
        &ctx,
        &tmp,
        "band_sr_b4",
        vec![23636, 23636, 23636, 23636, 9091, 9091, 9091, 9091],
        4,
        2,
    )
    .await?;
    register_band_sized(
        &ctx,
        &tmp,
        "band_sr_b5",
        vec![27273, 27273, 27273, 27273, 23636, 23636, 23636, 23636],
        4,
        2,
    )
    .await?;
    // ST DN: 310 K -> 47103, 300 -> 44177, 305 -> 45640, 290 -> 41252,
    // 295 -> 42715, 292.5 -> 41983.
    register_band_sized(
        &ctx,
        &tmp,
        "band_st_b10",
        vec![47103, 44177, 45640, 41252, 44177, 41252, 42715, 41983],
        4,
        2,
    )
    .await?;

    let mut ids = std::collections::BTreeMap::new();
    for product in ["ndvi", "lst"] {
        let (status, outcome) = send(
            &ctx.app,
            "POST",
            "/api/ingest/landsat/derive",
            Some(json!({ "scene_id": SCENE_ID, "product": product })),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{product}: {outcome}");
        ids.insert(
            product.to_string(),
            outcome["product_id"].as_str().unwrap().to_string(),
        );
    }

    let (status, outcome) = send(
        &ctx.app,
        "POST",
        "/api/water-management/et/derive",
        Some(json!({
            "lst_product_id": ids["lst"],
            "ndvi_product_id": ids["ndvi"],
            "field_id": "field-1",
            "season_id": "2024",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert!((outcome["wet_edge_k"].as_f64().unwrap() - 290.0).abs() < 0.01);
    assert_eq!(outcome["valid_fraction"], 1.0);

    let et = catalog::get_product(&ctx.pool, outcome["et_product_id"].as_str().unwrap())
        .await?
        .unwrap();
    assert_eq!(et.kind, "et_fraction");
    assert_eq!(
        et.parameters["method"],
        "jiang_islam_triangle_phi_normalized"
    );
    let values = {
        let mut reader = GeoTiffReader::open(et.path.as_deref().unwrap())?;
        reader.read_band()?.to_f32()
    };
    for (pixel, expected) in [
        (0usize, 0.0f32),
        (1, 0.5),
        (2, 0.25),
        (3, 1.0),
        (4, 0.0),
        (5, 1.0),
        (6, 0.5),
        (7, 0.75),
    ] {
        assert!(
            (values[pixel] - expected).abs() < 0.01,
            "pixel {pixel}: {} != {expected}",
            values[pixel]
        );
    }

    // Lineage covers both inputs under their roles.
    let edges = catalog::trace_inputs(&ctx.pool, &et.product_id).await?;
    let mut roles: Vec<&str> = edges.iter().map(|e| e.role.as_str()).collect();
    roles.sort_unstable();
    assert_eq!(roles, vec!["lst", "ndvi"]);

    // Wrong-kind input is refused.
    let (status, _) = send(
        &ctx.app,
        "POST",
        "/api/water-management/et/derive",
        Some(json!({
            "lst_product_id": ids["ndvi"],
            "ndvi_product_id": ids["ndvi"],
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    Ok(())
}
