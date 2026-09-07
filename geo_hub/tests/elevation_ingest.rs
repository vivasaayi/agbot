//! Elevation-source ingestion acceptance tests.
//!
//! A provider DEM/DSM GeoTIFF must enter through the normalized source
//! contract, retain provider and vertical-datum evidence, publish an L1
//! elevation product with an L0 input edge, and render through the global GIS
//! tile endpoint.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::elevation_ingest::{
    ingest_elevation, supported_elevation_sources, ElevationIngestRequest, ElevationSurfaceModel,
};
use geo_hub::product_tiler::{tile_containing, TILE_SIZE};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use shared::product_graph::{ProductLevel, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const ACQUIRED_AT: &str = "2026-07-01T00:00:00Z";
const TRANSFORM: [f64; 6] = [-76.0, 0.001, 0.0, 40.0, 0.0, -0.001];

struct Ctx {
    app: Router,
    pool: db::DbPool,
    data_root: std::path::PathBuf,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("elevation_ingest.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let data_root = config.data_root.clone();
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };
    Ok(Ctx {
        app: server::build_router(state),
        pool,
        data_root,
    })
}

fn write_dem(path: &std::path::Path) -> Result<()> {
    let values: Vec<f32> = (0..64).map(|index| 100.0 + index as f32).collect();
    write_geotiff_f32(
        path,
        8,
        8,
        &values,
        &GeoTiffTags {
            epsg: Some(4326),
            geo_transform: Some(TRANSFORM),
            nodata: Some(-9999.0),
        },
    )?;
    Ok(())
}

fn request(path: &std::path::Path) -> ElevationIngestRequest {
    ElevationIngestRequest {
        profile_id: "copernicus_dem_glo30".to_string(),
        scene_id: "cop-dem-n40-w076".to_string(),
        artifact_path: path.to_string_lossy().to_string(),
        acquired_at: ACQUIRED_AT.to_string(),
        surface_model: None,
        vertical_datum: None,
        checksum_sha256: None,
        scope: ProductScope {
            field_id: Some("field-1".to_string()),
            temporal_start: ACQUIRED_AT.to_string(),
            temporal_end: ACQUIRED_AT.to_string(),
            ..ProductScope::default()
        },
    }
}

#[test]
fn source_registry_includes_initial_global_dem_profiles() {
    let profiles = supported_elevation_sources();
    for expected in [
        "copernicus_dem_glo30",
        "nasadem_hgt",
        "srtm_gl1",
        "alos_aw3d30",
        "tandem_x_dem",
        "usgs_3dep",
    ] {
        assert!(
            profiles.iter().any(|profile| profile.id == expected),
            "missing elevation source profile {expected}"
        );
    }
    let copernicus = profiles
        .iter()
        .find(|profile| profile.id == "copernicus_dem_glo30")
        .expect("Copernicus profile");
    assert_eq!(copernicus.default_surface, ElevationSurfaceModel::Dsm);
    assert_eq!(copernicus.nominal_resolution_m, 30.0);
    assert!(!copernicus.license_url.is_empty());
}

#[tokio::test]
async fn source_profiles_are_discoverable_through_the_ingest_api() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .uri("/api/ingest/elevation/sources")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
    let profiles: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert!(profiles.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|profile| profile["id"] == "copernicus_dem_glo30")
    }));
    Ok(())
}

#[tokio::test]
async fn ingest_registers_source_scene_and_traceable_elevation_product() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let source_path = tmp.path().join("cop-dem.tif");
    write_dem(&source_path)?;

    let outcome = ingest_elevation(&ctx.pool, &ctx.data_root, &request(&source_path)).await?;
    assert_eq!(outcome.source_id, "esa:cop-dem-glo-30");
    assert_eq!(outcome.product_kind, "elevation_dsm");
    assert!(
        outcome.artifact_path.starts_with(&ctx.data_root),
        "ingest must stage the source under the managed data root"
    );
    assert!(outcome.artifact_path.exists());
    assert_eq!(
        outcome.tile_url_template,
        format!(
            "/api/catalog/products/{}/tiles/{{z}}/{{x}}/{{y}}.png",
            outcome.elevation_product_id
        )
    );

    let source_config: String =
        sqlx::query_scalar("SELECT config_json FROM catalog_sources WHERE source_id = ?1")
            .bind(&outcome.source_id)
            .fetch_one(&ctx.pool)
            .await?;
    let source_config: serde_json::Value = serde_json::from_str(&source_config)?;
    assert_eq!(source_config["profile_id"], "copernicus_dem_glo30");
    assert_eq!(source_config["vertical_datum"], "EGM2008");

    let product = catalog::get_product(&ctx.pool, &outcome.elevation_product_id)
        .await?
        .expect("elevation product registered");
    assert_eq!(product.level, ProductLevel::L1);
    assert_eq!(product.kind, "elevation_dsm");
    assert_eq!(product.source_id.as_deref(), Some("esa:cop-dem-glo-30"));
    assert_eq!(product.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(product.gsd_m_per_px, Some(30.0));
    assert_eq!(
        product.parameters["vertical_datum"],
        serde_json::json!("EGM2008")
    );

    let inputs = catalog::trace_inputs(&ctx.pool, &outcome.elevation_product_id).await?;
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].input_product_id, outcome.raw_product_id);
    assert_eq!(inputs[0].role, "raw_elevation_source");
    Ok(())
}

#[tokio::test]
async fn ingested_elevation_renders_through_catalog_gis_tiles() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let source_path = tmp.path().join("cop-dem.tif");
    write_dem(&source_path)?;
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ingest/elevation")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request(&source_path))?))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
    let outcome: geo_hub::elevation_ingest::ElevationIngestOutcome =
        serde_json::from_slice(&bytes)?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/stac/collections/scenes/items/{}",
                    outcome.elevation_product_id
                ))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
    let stac_item: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        stac_item["assets"]["tiles_web"]["href"],
        outcome.tile_url_template
    );

    let z = 15;
    let (x, y) = tile_containing(39.996, -75.996, z);
    let uri = format!(
        "/api/catalog/products/{}/tiles/{z}/{x}/{y}.png",
        outcome.elevation_product_id
    );
    let response = ctx
        .app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await?;
    let image = image::load_from_memory(&bytes)?.to_rgba8();
    assert_eq!(image.dimensions(), (TILE_SIZE, TILE_SIZE));
    let opaque: Vec<_> = image.pixels().filter(|pixel| pixel.0[3] == 255).collect();
    assert!(!opaque.is_empty(), "elevation footprint must be visible");
    assert!(
        opaque.iter().any(|pixel| pixel.0[0] != pixel.0[1]),
        "elevation must use a terrain ramp, not grayscale"
    );
    Ok(())
}

#[tokio::test]
async fn ingest_rejects_a_source_without_gis_compatible_georeferencing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;
    let source_path = tmp.path().join("unreferenced.tif");
    write_geotiff_f32(
        &source_path,
        2,
        2,
        &[1.0, 2.0, 3.0, 4.0],
        &GeoTiffTags::default(),
    )?;

    let error = ingest_elevation(&ctx.pool, &ctx.data_root, &request(&source_path))
        .await
        .expect_err("missing CRS/transform must fail before catalog registration");
    assert!(
        error.to_string().contains("georeferencing"),
        "unexpected error: {error}"
    );
    assert!(
        catalog::list_products(&ctx.pool, &catalog::ProductFilter::default())
            .await?
            .is_empty()
    );
    Ok(())
}
