//! Catalog-to-simulator terrain package acceptance tests.
//!
//! The C++ world compiler is an external process boundary, so these tests use
//! a deterministic fake compiler while exercising the real raster
//! normalization, catalog registration, and lineage code.

use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use geo_hub::elevation_ingest::{ingest_elevation, ElevationIngestRequest, ElevationSurfaceModel};
use geo_hub::state::AppState;
use geo_hub::terrain_derive::{
    derive_sim_terrain, ProcessTerrainCompiler, TerrainCompileOutput, TerrainCompileSpec,
    TerrainCompiler, TerrainDeriveRequest,
};
use geo_hub::{catalog, db, server, HubConfig};
use raster_io::{write_geotiff_f32, GeoTiffReader, GeoTiffTags, RasterDtype};
use shared::product_graph::{ProductLevel, ProductScope};
use shared::schemas::GeoBounds;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tower::util::ServiceExt;

const ACQUIRED_AT: &str = "2026-07-01T00:00:00Z";
const TRANSFORM: [f64; 6] = [-76.0, 0.001, 0.0, 40.0, 0.0, -0.001];

#[derive(Default)]
struct FakeCompiler {
    calls: Mutex<Vec<TerrainCompileSpec>>,
    escape_outputs: bool,
}

impl TerrainCompiler for FakeCompiler {
    fn compile(
        &self,
        spec: &TerrainCompileSpec,
    ) -> Result<TerrainCompileOutput, geo_hub::terrain_derive::TerrainDeriveError> {
        self.calls.lock().expect("calls lock").push(spec.clone());
        let output_dir = if self.escape_outputs {
            spec.output_dir.join("..").join("escaped-compiler-output")
        } else {
            spec.output_dir.clone()
        };
        std::fs::create_dir_all(&output_dir)?;
        let manifest_path = output_dir.join(format!("{}.agbworld", spec.name));
        let scene_path = output_dir.join(format!("{}.agbscn", spec.name));
        let validation_path = output_dir.join(format!("{}.validation.json", spec.name));
        std::fs::write(
            &manifest_path,
            format!(
                "{{\"world_hash\":4242,\"crs_policy\":{{\"horizontal\":\"EPSG:4326\",\
                 \"vertical_datum\":\"{}\"}},\"tiles\":[{{\"scene_path\":\"terrain.agbscn\",\
                 \"elevation_state\":\"authoritative\"}}]}}",
                spec.vertical_datum
            ),
        )?;
        std::fs::write(&scene_path, b"AGBSCN-test")?;
        std::fs::write(&validation_path, b"{\"ok\":true}")?;
        Ok(TerrainCompileOutput {
            manifest_path,
            scene_path,
            validation_path,
            world_hash: 4242,
            elevation_state: "authoritative".to_string(),
            terrain_min_m: 100.0,
            terrain_max_m: 163.0,
            terrain_cell_count: u64::from(spec.resolution).pow(2),
            terrain_nodata_cells: 0,
        })
    }
}

async fn pool_and_root(tmp: &TempDir) -> Result<(db::DbPool, std::path::PathBuf)> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("terrain_derive.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    Ok((pool, config.data_root))
}

fn write_dem(path: &Path) -> Result<()> {
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

fn ingest_request(path: &Path) -> ElevationIngestRequest {
    ElevationIngestRequest {
        profile_id: "copernicus_dem_glo30".to_string(),
        scene_id: "cop-dem-n40-w076".to_string(),
        artifact_path: path.to_string_lossy().to_string(),
        acquired_at: ACQUIRED_AT.to_string(),
        surface_model: Some(ElevationSurfaceModel::Dtm),
        vertical_datum: Some("EGM2008".to_string()),
        checksum_sha256: None,
        scope: ProductScope {
            field_id: Some("field-1".to_string()),
            temporal_start: ACQUIRED_AT.to_string(),
            temporal_end: ACQUIRED_AT.to_string(),
            ..ProductScope::default()
        },
    }
}

#[tokio::test]
async fn derives_traceable_l3_world_package_from_l1_elevation() -> Result<()> {
    let tmp = TempDir::new()?;
    let (pool, data_root) = pool_and_root(&tmp).await?;
    let source_path = tmp.path().join("cop-dem.tif");
    write_dem(&source_path)?;
    let elevation = ingest_elevation(&pool, &data_root, &ingest_request(&source_path)).await?;
    let compiler = Arc::new(FakeCompiler::default());

    let request = TerrainDeriveRequest {
        elevation_product_id: elevation.elevation_product_id.clone(),
        aoi: Some(GeoBounds {
            min_lon: -75.999,
            min_lat: 39.993,
            max_lon: -75.993,
            max_lat: 39.999,
        }),
        resolution: Some(16),
        target_gsd_m: Some(30.0),
        expected_vertical_datum: Some("EGM2008".to_string()),
        seed: Some(7),
    };
    let outcome = derive_sim_terrain(&pool, &data_root, &request, compiler.clone()).await?;

    assert_eq!(outcome.product_kind, "sim_terrain_package");
    assert_eq!(outcome.vertical_datum, "EGM2008");
    assert_eq!(outcome.world_hash, 4242);
    assert_eq!(outcome.elevation_state, "authoritative");
    assert!(outcome.manifest_path.exists());
    assert!(outcome.scene_path.exists());
    assert!(!outcome.reused_existing);

    {
        let calls = compiler.calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        let compile = &calls[0];
        assert_eq!(compile.aoi, request.aoi.clone().expect("request AOI"));
        assert_eq!(compile.resolution, 16);
        assert_eq!(compile.vertical_datum, "EGM2008");
        assert_ne!(compile.dem_path, elevation.artifact_path);
        assert!(compile.dem_path.starts_with(&data_root));
        let normalized = GeoTiffReader::open(&compile.dem_path)?;
        assert_eq!(normalized.info().dtype, RasterDtype::F32);
        assert_eq!(normalized.info().epsg, Some(4326));
    }

    let product = catalog::get_product(&pool, &outcome.terrain_product_id)
        .await?
        .expect("L3 product");
    assert_eq!(product.level, ProductLevel::L3);
    assert_eq!(product.kind, "sim_terrain_package");
    assert_eq!(product.format.as_deref(), Some("agbworld"));
    assert_eq!(product.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(product.parameters["vertical_datum"], "EGM2008");
    assert_eq!(product.parameters["scene_file"], "terrain.agbscn");
    assert_eq!(
        product.quality_summary.as_ref().expect("quality summary")["world_hash"],
        4242
    );
    let inputs = catalog::trace_inputs(&pool, &outcome.terrain_product_id).await?;
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].input_product_id, elevation.elevation_product_id);
    assert_eq!(inputs[0].role, "elevation");

    let repeated = derive_sim_terrain(&pool, &data_root, &request, compiler.clone()).await?;
    assert_eq!(repeated.terrain_product_id, outcome.terrain_product_id);
    assert!(repeated.reused_existing);
    assert_eq!(compiler.calls.lock().expect("calls lock").len(), 1);

    std::fs::write(&outcome.manifest_path, b"{\"tampered\":true}")?;
    let error = derive_sim_terrain(&pool, &data_root, &request, compiler.clone())
        .await
        .expect_err("a changed L3 package must not be silently reused");
    assert!(error.to_string().contains("package checksum mismatch"));
    assert_eq!(compiler.calls.lock().expect("calls lock").len(), 1);
    Ok(())
}

#[tokio::test]
async fn rejects_datum_mismatch_before_compilation() -> Result<()> {
    let tmp = TempDir::new()?;
    let (pool, data_root) = pool_and_root(&tmp).await?;
    let source_path = tmp.path().join("cop-dem.tif");
    write_dem(&source_path)?;
    let elevation = ingest_elevation(&pool, &data_root, &ingest_request(&source_path)).await?;
    let compiler = Arc::new(FakeCompiler::default());
    let request = TerrainDeriveRequest {
        elevation_product_id: elevation.elevation_product_id,
        expected_vertical_datum: Some("EGM96".to_string()),
        ..TerrainDeriveRequest::default()
    };

    let error = derive_sim_terrain(&pool, &data_root, &request, compiler.clone())
        .await
        .expect_err("datum mismatch must be rejected");
    assert!(error.to_string().contains("vertical datum mismatch"));
    assert!(compiler.calls.lock().expect("calls lock").is_empty());
    Ok(())
}

#[tokio::test]
async fn rejects_an_elevation_artifact_changed_after_ingest() -> Result<()> {
    let tmp = TempDir::new()?;
    let (pool, data_root) = pool_and_root(&tmp).await?;
    let source_path = tmp.path().join("cop-dem.tif");
    write_dem(&source_path)?;
    let elevation = ingest_elevation(&pool, &data_root, &ingest_request(&source_path)).await?;
    std::fs::write(&elevation.artifact_path, b"tampered")?;
    let compiler = Arc::new(FakeCompiler::default());
    let request = TerrainDeriveRequest {
        elevation_product_id: elevation.elevation_product_id,
        ..TerrainDeriveRequest::default()
    };

    let error = derive_sim_terrain(&pool, &data_root, &request, compiler.clone())
        .await
        .expect_err("changed evidence must not produce terrain");
    assert!(error.to_string().contains("checksum mismatch"));
    assert!(compiler.calls.lock().expect("calls lock").is_empty());
    Ok(())
}

#[tokio::test]
async fn rejects_compiler_artifacts_outside_the_product_package() -> Result<()> {
    let tmp = TempDir::new()?;
    let (pool, data_root) = pool_and_root(&tmp).await?;
    let source_path = tmp.path().join("cop-dem.tif");
    write_dem(&source_path)?;
    let elevation = ingest_elevation(&pool, &data_root, &ingest_request(&source_path)).await?;
    let compiler = Arc::new(FakeCompiler {
        calls: Mutex::default(),
        escape_outputs: true,
    });
    let request = TerrainDeriveRequest {
        elevation_product_id: elevation.elevation_product_id,
        ..TerrainDeriveRequest::default()
    };

    let error = derive_sim_terrain(&pool, &data_root, &request, compiler)
        .await
        .expect_err("escaped compiler artifacts must not be cataloged");
    assert!(error.to_string().contains("missing or outside its package"));
    Ok(())
}

#[tokio::test]
async fn terrain_derive_route_reports_an_unknown_elevation_product() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("terrain_route.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let app = server::build_router(AppState {
        pool,
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/terrain/derive")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&TerrainDeriveRequest {
                    elevation_product_id: "missing-elevation-product".to_string(),
                    ..TerrainDeriveRequest::default()
                })?))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn real_cpp_compiler_consumes_catalog_dem_when_available() -> Result<()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let compiler_path = workspace.join("flight_sim_cpp/build/worldgen/agbot_terrain_compile");
    if !compiler_path.is_file() {
        eprintln!(
            "SKIP real C++ terrain bridge: build {} first",
            compiler_path.display()
        );
        return Ok(());
    }

    let tmp = TempDir::new()?;
    let (pool, data_root) = pool_and_root(&tmp).await?;
    let source_path = tmp.path().join("cop-dem.tif");
    write_dem(&source_path)?;
    let elevation = ingest_elevation(&pool, &data_root, &ingest_request(&source_path)).await?;
    let request = TerrainDeriveRequest {
        elevation_product_id: elevation.elevation_product_id,
        resolution: Some(16),
        target_gsd_m: Some(30.0),
        expected_vertical_datum: Some("EGM2008".to_string()),
        ..TerrainDeriveRequest::default()
    };

    let outcome = derive_sim_terrain(
        &pool,
        &data_root,
        &request,
        Arc::new(ProcessTerrainCompiler::new(compiler_path)),
    )
    .await?;
    assert_eq!(outcome.elevation_state, "authoritative");
    assert_eq!(outcome.terrain_cell_count, 256);
    assert_eq!(outcome.terrain_nodata_cells, 0);
    assert!(outcome.world_hash > 0);
    assert!(outcome.manifest_path.is_file());
    assert!(outcome.scene_path.is_file());
    assert!(outcome.validation_path.is_file());
    Ok(())
}
