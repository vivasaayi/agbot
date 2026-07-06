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
    let path = tmp.path().join(format!("{kind}.tif"));
    write_geotiff_u16(
        &path,
        2,
        2,
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
        parameters: json!({ "scene_id": SCENE_ID, "band": kind }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(SCENE_ID.to_string()),
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
    Ok(catalog::register_product(&ctx.pool, &draft, "2026-07-05T00:00:00Z").await?)
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
