use anyhow::Result;
use geo_hub::{catalog, db, HubConfig};

#[tokio::test]
async fn mapped_scan_registers_with_complete_source_lineage() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("scans");
    let output = tmp.path().join("maps");
    std::fs::create_dir(&input)?;
    std::fs::write(
        input.join("scan_test.json"),
        serde_json::to_vec(&serde_json::json!({
            "timestamp":"2026-01-01T00:00:00Z",
            "scan_id":"00000000-0000-4000-8000-000000000001",
            "points":[{"timestamp":"2026-01-01T00:00:00Z","angle":0,"distance":2000,"quality":100}]
        }))?,
    )?;
    let mapper = lidar_mapper::LidarMapper::new(&lidar_mapper::Args {
        input_dir: input.clone(),
        output_dir: output.clone(),
        distance_threshold: None,
        quality_threshold: None,
        occupancy_threshold: None,
        flip_y: None,
    })
    .await?;
    mapper.process_directory(&input, &output).await?;
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("hub.db").display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let result = catalog::register_sidecar_dir(&pool, &output, "2026-01-01T00:00:00Z").await?;
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(result.registered.len(), 2);
    let edges: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM catalog_product_inputs i JOIN catalog_products p ON p.product_id=i.input_product_id WHERE p.level='l0'")
        .fetch_one(&pool).await?;
    assert_eq!(edges, 1);
    Ok(())
}

#[tokio::test]
async fn world_returns_preserve_location_and_elevation_in_geotiff() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("world.jsonl");
    std::fs::write(&input, serde_json::json!({"timestamp":"2026-01-01T00:00:00Z","status":"ok",
        "points":[{"x":0.0,"y":12.0,"z":0.0},{"x":1.0,"y":14.0,"z":0.0},{"x":0.0,"y":20.0,"z":2.0}]
    }).to_string())?;
    let output = tmp.path().join("maps");
    let id = lidar_mapper::sim_georef::export(&input, &output, 36.0, -119.0, 1.0)?;
    let mut reader = raster_io::GeoTiffReader::open(output.join("sim_elevation.tif"))?;
    let info = reader.info().clone();
    assert_eq!(info.epsg, Some(4326));
    let transform = info.geo_transform.unwrap();
    assert_eq!(transform[0], -119.0);
    assert!((transform[3] - (36.0 + 3.0 / 111_320.0)).abs() < 1e-10);
    assert!(transform[5] < 0.0);
    let values = reader.read_band()?;
    assert!(values.to_f32().contains(&12.0));
    assert!(values.to_f32().contains(&14.0));
    assert!(values.to_f32().contains(&20.0));
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("hub.db").display()),
        data_root: tmp.path().join("data"),
        ..Default::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let report = catalog::register_sidecar_dir(&pool, &output, "2026-01-01T00:00:00Z").await?;
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert!(report.registered.contains(&id));
    use tower::ServiceExt;
    let state = geo_hub::state::AppState {
        pool: pool.clone(),
        config: std::sync::Arc::new(config),
        scene_search_cache: Default::default(),
    };
    let z = 18u32;
    let n = 2f64.powi(z as i32);
    let x = (((-119.0 + 180.0) / 360.0) * n).floor() as u32;
    let y = ((1.0 - 36.0f64.to_radians().tan().asinh() / std::f64::consts::PI) / 2.0 * n).floor()
        as u32;
    let response = geo_hub::server::build_router(state)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/catalog/products/{id}/tiles/{z}/{x}/{y}.png"))
                .body(axum::body::Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1_000_000).await?;
    let tile = image::load_from_memory(&bytes)?.to_rgba8();
    assert!(
        tile.pixels().any(|p| p.0[3] > 0),
        "observed cells must appear in the web tile"
    );
    assert!(lidar_mapper::sim_georef::export(&input, &output, 90.0, -119.0, 1.0).is_err());
    assert!(lidar_mapper::sim_georef::export(&input, &output, 36.0, -119.0, 0.0).is_err());
    Ok(())
}
