//! End-to-end test of the phenology + tier-1 land-cover pipeline
//! (satellite batch 10): cataloged dated NDVI + MNDWI L2 GeoTIFFs ->
//! POST /api/landcover/derive -> `phenology` (JSON) + `landcover_rule`
//! (GeoTIFF) L3 products with lineage, web-tiled through the categorical
//! land-cover colormap.
//!
//! Fixture: a 2x2 grid on the Sentinel-2 43PFN 10 m grid, five dates through
//! 2026, one archetype per pixel:
//! - pixel 0 crop pulse 0.2 -> 0.8 -> 0.2   => annual_crop (code 3)
//! - pixel 1 perennial floor ~0.6-0.72      => tree_or_perennial (code 4)
//! - pixel 2 never greens (max 0.18)        => bare_or_sparse (code 2)
//! - pixel 3 bare NDVI but MNDWI mean 0.4   => water (code 1, rule override)

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
            tmp.path().join("landcover.db").display()
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

async fn register_l2(
    ctx: &Ctx,
    tmp: &TempDir,
    kind: &str,
    stamp: &str,
    values: Vec<f32>,
    transform: [f64; 6],
) -> Result<String> {
    let path = tmp.path().join(format!("{kind}_{stamp}.tif"));
    write_geotiff_f32(
        &path,
        2,
        2,
        &values,
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(transform),
            nodata: Some(f64::from(NODATA)),
        },
    )?;
    let draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: kind.to_string(),
        algorithm_id: "test.landcover".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "stamp": stamp, "kind": kind }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(format!("scene-{kind}-{stamp}")),
            temporal_start: format!("{stamp}T10:30:00Z"),
            temporal_end: format!("{stamp}T10:30:00Z"),
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

/// NDVI per date: [crop, tree, bare, water-adjacent-bare].
const SERIES: [(&str, [f32; 4]); 5] = [
    ("2026-02-01", [0.2, 0.62, 0.10, 0.08]),
    ("2026-04-01", [0.5, 0.60, 0.15, 0.10]),
    ("2026-06-01", [0.8, 0.72, 0.18, 0.12]),
    ("2026-08-01", [0.5, 0.65, 0.12, 0.09]),
    ("2026-10-01", [0.2, 0.63, 0.11, 0.07]),
];

fn derive_body() -> serde_json::Value {
    json!({
        "field_id": "field-1",
        "season_id": "season-2026",
        "start": "2026-01-01",
        "end": "2026-12-31",
        "min_observations": 4,
    })
}

#[tokio::test]
async fn derives_phenology_and_landcover_with_lineage_and_tiles() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    let mut ndvi_ids = Vec::new();
    for (stamp, pixels) in SERIES {
        ndvi_ids.push(register_l2(&ctx, &tmp, "ndvi", stamp, pixels.to_vec(), TRANSFORM).await?);
    }
    // Two MNDWI scenes: pixel 3 is open water, everything else dry land.
    let mut mndwi_ids = Vec::new();
    for stamp in ["2026-05-01", "2026-09-01"] {
        mndwi_ids.push(
            register_l2(
                &ctx,
                &tmp,
                "mndwi",
                stamp,
                vec![-0.3, -0.3, -0.2, 0.4],
                TRANSFORM,
            )
            .await?,
        );
    }
    // An off-grid NDVI in-window must be skipped with a reason, not used.
    let shifted = [601_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0];
    let offgrid = register_l2(&ctx, &tmp, "ndvi", "2026-05-15", vec![0.5; 4], shifted).await?;

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/derive",
        Some(derive_body()),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&bytes)?;

    let used: Vec<String> = serde_json::from_value(outcome["ndvi_observations_used"].clone())?;
    assert_eq!(used.len(), 5);
    for id in &ndvi_ids {
        assert!(used.contains(id));
    }
    assert_eq!(
        outcome["observations_skipped"],
        json!([{ "product_id": offgrid, "reason": "grid_mismatch" }])
    );
    assert_eq!(
        outcome["water_observations_used"],
        serde_json::to_value(&mndwi_ids)?
    );
    assert_eq!(outcome["phenology_valid_fraction"], 1.0);
    assert_eq!(outcome["landcover_valid_fraction"], 1.0);

    // Class raster: [annual_crop 3, tree 4, bare 2, water 1].
    let landcover_path = outcome["landcover_artifact"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(landcover_path)?;
    let values = reader.read_band()?.to_f32();
    assert_eq!(values, vec![3.0, 4.0, 2.0, 1.0]);

    // Phenology artifact is self-describing JSON with the crop pixel's
    // hand-verifiable metrics (peak DOY 152 = Jun 1).
    let phenology: serde_json::Value = serde_json::from_slice(&std::fs::read(
        outcome["phenology_artifact"].as_str().unwrap(),
    )?)?;
    assert_eq!(
        phenology["evidence"]["smoothing"],
        "v_dip_despike_0.1_neighbor_mean"
    );
    assert!((phenology["peak_doy"][0].as_f64().unwrap() - 152.0).abs() < 1e-3);
    assert!((phenology["amplitude"][0].as_f64().unwrap() - 0.6).abs() < 1e-5);

    // Lineage: land cover -> phenology product + MNDWI + direct NDVI edges.
    let landcover_id = outcome["landcover_product_id"].as_str().unwrap();
    let phenology_id = outcome["phenology_product_id"].as_str().unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, landcover_id).await?;
    let inputs: Vec<&str> = edges
        .iter()
        .map(|edge| edge.input_product_id.as_str())
        .collect();
    assert!(inputs.contains(&phenology_id));
    for id in ndvi_ids.iter().chain(&mndwi_ids) {
        assert!(inputs.contains(&id.as_str()), "missing edge to {id}");
    }

    // Web tile: every opaque pixel carries one of the four class colors.
    let (lat, lon) = utm_to_wgs84(
        600_010.0,
        1_300_010.0,
        UtmZone {
            zone: 43,
            north: true,
        },
    );
    let (x, y) = tile_containing(lat, lon, 18);
    let (status, bytes) = send(
        &ctx.app,
        "GET",
        &format!("/api/catalog/products/{landcover_id}/tiles/18/{x}/{y}.png"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let png = image::load_from_memory(&bytes)?.to_rgba8();
    let colormap = colormap_for_kind("landcover_rule");
    let allowed: Vec<[u8; 3]> = [1.0f32, 2.0, 3.0, 4.0]
        .iter()
        .map(|code| colormap.rgb(*code))
        .collect();
    let opaque: Vec<_> = png.pixels().filter(|px| px.0[3] == 255).collect();
    assert!(!opaque.is_empty(), "land-cover footprint must render");
    for px in &opaque {
        let rgb = [px.0[0], px.0[1], px.0[2]];
        assert!(allowed.contains(&rgb), "unexpected tile color {rgb:?}");
    }

    // Listing route + idempotent re-derive.
    let (status, bytes) = send(&ctx.app, "GET", "/api/landcover/rasters", None).await?;
    assert_eq!(status, StatusCode::OK);
    let listing: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(listing["phenology"].as_array().unwrap().len(), 1);
    assert_eq!(listing["landcover"].as_array().unwrap().len(), 1);

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/derive",
        Some(derive_body()),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["landcover_product_id"], json!(landcover_id));
    assert_eq!(again["phenology_product_id"], json!(phenology_id));
    Ok(())
}

#[tokio::test]
async fn landcover_error_paths_are_reason_coded() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Inverted window.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/derive",
        Some(json!({
            "field_id": "f", "season_id": "s",
            "start": "2026-12-31", "end": "2026-01-01",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("not before"));

    // Empty catalog window.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/derive",
        Some(derive_body()),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("no cataloged"));
    Ok(())
}
