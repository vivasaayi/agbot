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

/// Batch 16: register a WorldCover reference tile on the same grid and
/// validate the tier-1 classification against it. Hand computation: our
/// classes are [crop, tree, bare, water] (codes 3,4,2,1); the reference is
/// [40 crop, 10 tree, 30 grass, 80 water] -> 3 of 4 agree (po = 0.75).
/// Marginals: ours put 0.25 on each of crop/tree/bare/water; the reference
/// puts 0.25 on each of crop/tree/grass/water, so
/// pe = 3 * (0.25 * 0.25) = 0.1875 (bare and grass contribute zero), giving
/// kappa = (0.75 - 0.1875) / (1 - 0.1875) = 9/13.
#[tokio::test]
async fn worldcover_reference_validates_the_tier1_classification() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Recreate the batch-10 archetype scene.
    for (stamp, pixels) in SERIES {
        register_l2(&ctx, &tmp, "ndvi", stamp, pixels.to_vec(), TRANSFORM).await?;
    }
    for stamp in ["2026-05-01", "2026-09-01"] {
        register_l2(
            &ctx,
            &tmp,
            "mndwi",
            stamp,
            vec![-0.3, -0.3, -0.2, 0.4],
            TRANSFORM,
        )
        .await?;
    }
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/derive",
        Some(derive_body()),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let derived: serde_json::Value = serde_json::from_slice(&bytes)?;
    let landcover_id = derived["landcover_product_id"]
        .as_str()
        .unwrap()
        .to_string();

    // WorldCover tile on the same grid: crop 40, tree 10, grass 30
    // (disagrees with our bare pixel), water 80.
    let reference_dir = tmp.path().join("worldcover");
    std::fs::create_dir_all(&reference_dir)?;
    write_geotiff_f32(
        &reference_dir.join("ESA_WorldCover_10m_2021_v200_N09E075_Map.tif"),
        2,
        2,
        &[40.0, 10.0, 30.0, 80.0],
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(0.0),
        },
    )?;
    std::fs::write(reference_dir.join("readme.txt"), b"not a tile")?;

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/reference/register",
        Some(json!({ "dir": reference_dir.to_string_lossy() })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let registered: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(registered["registered"].as_array().unwrap().len(), 1);
    assert_eq!(registered["skipped"], json!(["readme.txt"]));
    let reference_id = registered["registered"][0][1].as_str().unwrap().to_string();
    let reference = catalog::get_product(&ctx.pool, &reference_id)
        .await?
        .unwrap();
    assert_eq!(reference.kind, "landcover_reference");
    assert_eq!(
        reference.temporal_start.as_deref(),
        Some("2021-01-01T00:00:00Z")
    );

    // --- Validate.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/validate",
        Some(json!({
            "landcover_product_id": landcover_id,
            "reference_product_id": reference_id,
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
    assert_eq!(outcome["compared_pixels"], 4);
    assert!((outcome["overall_agreement"].as_f64().unwrap() - 0.75).abs() < 1e-12);
    assert!((outcome["kappa"].as_f64().unwrap() - 9.0 / 13.0).abs() < 1e-12);

    // The bare pixel is the disagreement: reference says grassland there.
    let per_class = outcome["result"]["per_class"].as_array().unwrap();
    let bare = per_class
        .iter()
        .find(|line| line["class"] == "bare_or_sparse")
        .unwrap();
    assert_eq!(bare["users_accuracy"], 0.0);

    // Agreement product registered with lineage to both inputs and the
    // agreement JSON artifact on disk.
    let agreement_id = outcome["agreement_product_id"].as_str().unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, agreement_id).await?;
    let mut inputs: Vec<&str> = edges
        .iter()
        .map(|edge| edge.input_product_id.as_str())
        .collect();
    inputs.sort_unstable();
    let mut expected = [landcover_id.as_str(), reference_id.as_str()];
    expected.sort_unstable();
    assert_eq!(inputs, expected);
    let artifact: serde_json::Value = serde_json::from_slice(&std::fs::read(
        outcome["agreement_artifact"].as_str().unwrap(),
    )?)?;
    assert_eq!(artifact["compared_pixels"], 4);

    // Idempotent + wrong-kind reason coding.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/validate",
        Some(json!({
            "landcover_product_id": landcover_id,
            "reference_product_id": reference_id,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["agreement_product_id"], json!(agreement_id));

    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/validate",
        Some(json!({
            "landcover_product_id": reference_id, // wrong kind
            "reference_product_id": reference_id,
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&bytes).contains("landcover_rule"));
    Ok(())
}

/// Batch 17: train a nearest-centroid model on WorldCover labels and
/// classify into a landcover_ml raster, then validate the LEARNED map
/// against the same reference with the batch-16 engine. Because each scene
/// self-trains on its own reference labels, agreement is high by
/// construction — the test asserts the round trip is wired, lineage-closed,
/// and produces a real raster, not a specific accuracy number.
#[tokio::test]
async fn ml_classify_learns_from_reference_and_registers_a_raster() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    for (stamp, pixels) in SERIES {
        register_l2(&ctx, &tmp, "ndvi", stamp, pixels.to_vec(), TRANSFORM).await?;
    }
    for stamp in ["2026-05-01", "2026-09-01"] {
        register_l2(
            &ctx,
            &tmp,
            "mndwi",
            stamp,
            vec![-0.3, -0.3, -0.2, 0.4],
            TRANSFORM,
        )
        .await?;
    }
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/derive",
        Some(derive_body()),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let derived: serde_json::Value = serde_json::from_slice(&bytes)?;
    let phenology_id = derived["phenology_product_id"]
        .as_str()
        .unwrap()
        .to_string();

    // WorldCover: crop / tree / grass / water across the four archetype
    // pixels (all four learnable classes present -> four centroids).
    let reference_dir = tmp.path().join("worldcover");
    std::fs::create_dir_all(&reference_dir)?;
    write_geotiff_f32(
        &reference_dir.join("ESA_WorldCover_10m_2021_v200_N09E075_Map.tif"),
        2,
        2,
        &[40.0, 10.0, 30.0, 80.0],
        &GeoTiffTags {
            epsg: Some(EPSG),
            geo_transform: Some(TRANSFORM),
            nodata: Some(0.0),
        },
    )?;
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/reference/register",
        Some(json!({ "dir": reference_dir.to_string_lossy() })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let registered: serde_json::Value = serde_json::from_slice(&bytes)?;
    let reference_id = registered["registered"][0][1].as_str().unwrap().to_string();

    // --- Learned classification.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/ml/classify",
        Some(json!({
            "phenology_product_id": phenology_id,
            "reference_product_id": reference_id,
            "field_id": "field-1",
            "season_id": "season-2026",
        })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let ml: serde_json::Value = serde_json::from_slice(&bytes)?;
    // Pixel 3 is flat -> no features; 3 feature-valid pixels, 3 samples
    // (crop/tree/water labels over those pixels).
    assert_eq!(ml["feature_pixels"], 3);
    assert_eq!(ml["training_sample_count"], 3);

    // landcover_ml raster: feature-valid pixels get a class code, flat pixel
    // is nodata.
    let ml_path = ml["landcover_ml_artifact"].as_str().unwrap();
    let mut reader = GeoTiffReader::open(ml_path)?;
    let codes = reader.read_band()?.to_f32();
    assert_eq!(codes.len(), 4);
    for code in &codes[..3] {
        assert!((1.0..=6.0).contains(code), "class code {code}");
    }
    assert_eq!(codes[3], -9999.0);

    // Lineage: phenology + reference.
    let ml_id = ml["landcover_ml_product_id"].as_str().unwrap();
    let edges = catalog::trace_inputs(&ctx.pool, ml_id).await?;
    let mut inputs: Vec<&str> = edges.iter().map(|e| e.input_product_id.as_str()).collect();
    inputs.sort_unstable();
    let mut expected = [phenology_id.as_str(), reference_id.as_str()];
    expected.sort_unstable();
    assert_eq!(inputs, expected);

    // The fitted model is embedded for reproducible inference.
    let product = catalog::get_product(&ctx.pool, ml_id).await?.unwrap();
    assert_eq!(product.parameters["training_sample_count"], 3);
    assert!(
        product.parameters["model"]["centroids"]
            .as_array()
            .unwrap()
            .len()
            >= 1
    );

    // The learned raster validates against the reference with the batch-16
    // engine (same route, kind landcover_ml is accepted... actually validate
    // expects landcover_rule; assert the raster tiles instead).
    let (status, bytes) = send(
        &ctx.app,
        "GET",
        &format!("/api/catalog/products/{ml_id}/tiles/18/0/0.png"),
        None,
    )
    .await?;
    // z18 tile 0/0 is far from the scene: transparent, but 200 OK proves
    // landcover_ml is web-tileable through the shared class colormap.
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );

    // Idempotent.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/ml/classify",
        Some(json!({
            "phenology_product_id": phenology_id,
            "reference_product_id": reference_id,
            "field_id": "field-1",
            "season_id": "season-2026",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let again: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(again["landcover_ml_product_id"], json!(ml_id));

    // Batch 20: the tier-3 learned map validates against the reference
    // through the same agreement engine as the tier-1 rule map — the
    // outcome records which tier was evaluated.
    let (status, bytes) = send(
        &ctx.app,
        "POST",
        "/api/landcover/validate",
        Some(json!({
            "landcover_product_id": ml_id,
            "reference_product_id": reference_id,
        })),
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let validation: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(validation["classification_kind"], "landcover_ml");
    assert!(validation["overall_agreement"].as_f64().unwrap() >= 0.0);
    // Self-trained on the reference labels -> perfect agreement on the
    // three feature-valid pixels.
    assert_eq!(validation["compared_pixels"], 3);
    assert!((validation["overall_agreement"].as_f64().unwrap() - 1.0).abs() < 1e-12);
    let agreement_product = catalog::get_product(
        &ctx.pool,
        validation["agreement_product_id"].as_str().unwrap(),
    )
    .await?
    .unwrap();
    assert_eq!(
        agreement_product.parameters["classification_kind"],
        "landcover_ml"
    );
    Ok(())
}
