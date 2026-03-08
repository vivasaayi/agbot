use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use image::{GrayImage, Luma};
use serde_json::json;
use std::{path::Path, path::PathBuf, sync::Arc};
use tempfile::TempDir;
use tower::util::ServiceExt;
use uuid::Uuid;

const TEST_PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\ngeo-hub-test-png";

#[tokio::test]
async fn serves_file_backed_product_without_scene_row() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let app = ctx.app;

    let scene_id = "demo_scene";
    let product_path = tmp
        .path()
        .join("data")
        .join("scenes")
        .join(scene_id)
        .join("products")
        .join("ndvi")
        .join("sample.png");
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    std::fs::write(&product_path, TEST_PNG_BYTES)?;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/products/ndvi"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("image/png")
    );

    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    assert_eq!(body.as_ref(), TEST_PNG_BYTES);

    Ok(())
}

#[tokio::test]
async fn missing_scene_returns_not_found() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let app = ctx.app;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/scenes/unknown_scene/products/ndvi")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn scene_manifest_lists_available_products_from_disk() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let app = ctx.app;

    let scene_id = "manifest_scene";
    let scene_dir = tmp.path().join("data").join("scenes").join(scene_id);
    let product_path = scene_dir.join("products").join("ndvi").join("sample.png");
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    std::fs::write(&product_path, TEST_PNG_BYTES)?;
    let metadata_json = json!({
        "metadata": {
            "timestamp": "2025-02-01T00:00:00Z",
            "gps_position": {
                "latitude": 40.7128,
                "longitude": -74.0060,
                "altitude": 12.0
            },
            "bands": ["B4", "B5", "B6"],
            "exposure_time": 1.0,
            "gain": 1.0,
            "width": 512,
            "height": 256
        },
        "file_paths": {
            "B4": "B4.png",
            "B5": "B5.png",
            "B6": "B6.png"
        },
        "image_id": Uuid::new_v4()
    })
    .to_string();
    std::fs::write(scene_dir.join("metadata_ingested.json"), metadata_json)?;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        json.get("scene_id").and_then(|v| v.as_str()),
        Some(scene_id)
    );

    let products = json
        .get("available_products")
        .and_then(|v| v.as_array())
        .expect("products array should exist");
    assert_eq!(products.len(), 1);
    assert_eq!(json.get("width").and_then(|v| v.as_u64()), Some(512));
    assert_eq!(json.get("height").and_then(|v| v.as_u64()), Some(256));
    assert_eq!(
        json.get("bands")
            .and_then(|v| v.as_array())
            .map(|bands| bands.len()),
        Some(3)
    );
    assert_eq!(
        json.pointer("/gps_position/latitude")
            .and_then(|v| v.as_f64()),
        Some(40.7128)
    );
    assert_eq!(
        products[0].get("kind").and_then(|v| v.as_str()),
        Some("ndvi")
    );
    assert_eq!(
        products[0].get("url_path").and_then(|v| v.as_str()),
        Some(format!("/api/scenes/{scene_id}/products/ndvi").as_str())
    );

    Ok(())
}

#[tokio::test]
async fn generates_ndvi_via_db_fallback_and_persists_product_row() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let scene_id = "scene_from_db";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let b4_path = scene_dir.join("B4.png");
    let b5_path = scene_dir.join("B5.png");
    write_gray_png(&b4_path, 40)?;
    write_gray_png(&b5_path, 140)?;

    let image_id = Uuid::new_v4();
    let metadata_json = json!({
        "metadata": {
            "timestamp": "2025-01-01T00:00:00Z",
            "gps_position": null,
            "bands": ["B4", "B5"],
            "exposure_time": 1.0,
            "gain": 1.0,
            "width": 1,
            "height": 1
        },
        "file_paths": {
            "B4": b4_path.to_string_lossy().to_string(),
            "B5": b5_path.to_string_lossy().to_string()
        },
        "image_id": image_id
    })
    .to_string();

    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(scene_id)
    .bind("landsat8")
    .bind("2025-01-01T00:00:00Z")
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(metadata_json)
    .bind(None::<f64>)
    .bind("2025-01-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/products/ndvi"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = to_bytes(response.into_body(), 256 * 1024).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected status {status}; body: {}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(content_type.as_deref(), Some("image/png"));
    assert!(body.len() > 8);
    assert_eq!(&body.as_ref()[..8], b"\x89PNG\r\n\x1a\n");

    let product_path: String =
        sqlx::query_scalar("SELECT path FROM products WHERE scene_id = ?1 AND kind = ?2")
            .bind(scene_id)
            .bind("ndvi")
            .fetch_one(&ctx.pool)
            .await?;

    assert!(Path::new(&product_path).exists());
    assert!(product_path.ends_with(".png"));

    Ok(())
}

struct TestContext {
    app: axum::Router,
    pool: db::DbPool,
    data_root: PathBuf,
}

async fn test_app(tmp: &TempDir) -> Result<TestContext> {
    let data_root = tmp.path().join("data");
    let db_path = tmp.path().join("geo_hub_test.db");

    let config = HubConfig {
        bind_address: "127.0.0.1:0".to_string(),
        database_url: sqlite_url(&db_path),
        data_root: data_root.clone(),
    };

    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;

    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
    };

    Ok(TestContext {
        app: server::build_router(state),
        pool,
        data_root,
    })
}

fn sqlite_url(db_path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", db_path.display())
}

fn write_gray_png(path: &Path, value: u8) -> Result<()> {
    let img = GrayImage::from_pixel(2, 2, Luma([value]));
    img.save(path)?;
    Ok(())
}
