use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    response::Response,
};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use image::{GrayImage, Luma};
use interop::reopen_raster_geotiff;
use serde_json::json;
use sqlx::Row;
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
async fn serves_png_tile_and_writes_tile_cache() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let app = ctx.app;

    let scene_id = "tile_scene";
    let product_path = tmp
        .path()
        .join("data")
        .join("scenes")
        .join(scene_id)
        .join("products")
        .join("ndvi")
        .join("sample.png");
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    write_gray_png(&product_path, 180)?;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/products/ndvi/tiles/0/0/0.png"
                ))
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
    assert!(body.starts_with(b"\x89PNG\r\n\x1a\n"));

    let tile_cache_dir = ctx
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("tile_cache")
        .join("ndvi");
    assert!(tile_cache_dir.exists());
    assert!(std::fs::read_dir(&tile_cache_dir)?.next().is_some());

    Ok(())
}

#[tokio::test]
async fn tile_request_outside_zoom_grid_returns_not_found() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let app = ctx.app;

    let scene_id = "tile_scene_oob";
    let product_path = tmp
        .path()
        .join("data")
        .join("scenes")
        .join(scene_id)
        .join("products")
        .join("ndvi")
        .join("sample.png");
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    write_gray_png(&product_path, 80)?;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/products/ndvi/tiles/0/1/0.png"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn ingest_health_endpoint_reports_counts_and_last_error() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_ingest_health_row(
        &ctx,
        "scene-active",
        "Downloading",
        None,
        "2026-05-03T00:00:00Z",
    )
    .await?;
    seed_ingest_health_row(&ctx, "scene-stored", "Stored", None, "2026-05-04T00:00:00Z").await?;
    seed_ingest_health_row(
        &ctx,
        "scene-failed",
        "Failed",
        Some("download_error"),
        "2026-05-05T00:00:00Z",
    )
    .await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ingest/health")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        json.get("in_flight").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        json.get("succeeded").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(json.get("failed").and_then(|value| value.as_u64()), Some(1));
    assert_eq!(
        json.pointer("/last_error/scene_id")
            .and_then(|value| value.as_str()),
        Some("scene-failed")
    );
    assert_eq!(
        json.pointer("/last_error/reason_code")
            .and_then(|value| value.as_str()),
        Some("download_error")
    );

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
async fn creating_field_and_linking_scene_exposes_field_scoped_gis_data() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": "north-80",
                        "name": "North 80",
                        "crop": "corn",
                        "season": "2026",
                        "notes": "test field",
                        "boundary": {
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.4 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(create_response.status(), StatusCode::OK);
    let body = to_bytes(create_response.into_body(), 64 * 1024).await?;
    let field_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        field_json.get("field_id").and_then(|v| v.as_str()),
        Some("north-80")
    );
    assert_eq!(
        field_json
            .pointer("/extent/max_lat")
            .and_then(|v| v.as_f64()),
        Some(41.4)
    );

    let scene_id = "scene_with_field";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let metadata_json = json!({
        "metadata": {
            "timestamp": "2025-01-01T00:00:00Z",
            "gps_position": {
                "latitude": 41.25,
                "longitude": -96.45,
                "altitude": 350.0
            },
            "bands": ["B4", "B5"],
            "exposure_time": 1.0,
            "gain": 1.0,
            "width": 4,
            "height": 4,
            "spatial_ref": {
                "georeferenced": true,
                "crs": "EPSG:4326",
                "bbox": {
                    "min_lon": -96.8,
                    "min_lat": 41.0,
                    "max_lon": -96.1,
                    "max_lat": 41.5
                }
            }
        },
        "file_paths": {
            "B4": "B4.png",
            "B5": "B5.png"
        },
        "image_id": Uuid::new_v4()
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

    let link_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scenes/{scene_id}/field/north-80"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(link_response.status(), StatusCode::OK);
    let body = to_bytes(link_response.into_body(), 64 * 1024).await?;
    let linked_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        linked_json.get("field_id").and_then(|v| v.as_str()),
        Some("north-80")
    );
    assert_eq!(
        linked_json.get("season_id").and_then(|v| v.as_str()),
        Some("2026")
    );
    assert!(linked_json
        .get("linked_at")
        .and_then(|v| v.as_str())
        .is_some_and(|value| !value.trim().is_empty()));

    let audit_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/audit"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(audit_response.status(), StatusCode::OK);
    let audit_body = to_bytes(audit_response.into_body(), 64 * 1024).await?;
    let audit_json: serde_json::Value = serde_json::from_slice(&audit_body)?;
    assert_eq!(
        audit_json.get("scene_id").and_then(|value| value.as_str()),
        Some(scene_id)
    );
    assert_eq!(
        audit_json
            .pointer("/link_audits/0/mutation")
            .and_then(|value| value.as_str()),
        Some("link_scene_to_field")
    );
    assert_eq!(
        audit_json
            .pointer("/link_audits/0/new_field_id")
            .and_then(|value| value.as_str()),
        Some("north-80")
    );
    assert!(audit_json
        .pointer("/link_audits/0/audit_id")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value.starts_with("scene-link-audit-")));

    let scenes_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/north-80/scenes")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(scenes_response.status(), StatusCode::OK);
    let body = to_bytes(scenes_response.into_body(), 64 * 1024).await?;
    let scenes_json: serde_json::Value = serde_json::from_slice(&body)?;
    let scenes = scenes_json.as_array().expect("scenes should be an array");
    assert_eq!(scenes.len(), 1);
    assert_eq!(
        scenes[0].get("scene_id").and_then(|v| v.as_str()),
        Some(scene_id)
    );
    assert_eq!(
        scenes[0].get("season_id").and_then(|v| v.as_str()),
        Some("2026")
    );
    assert!(scenes[0]
        .get("linked_at")
        .and_then(|v| v.as_str())
        .is_some_and(|value| !value.trim().is_empty()));

    let manifest_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(manifest_response.status(), StatusCode::OK);
    let body = to_bytes(manifest_response.into_body(), 64 * 1024).await?;
    let manifest_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        manifest_json
            .pointer("/field/name")
            .and_then(|v| v.as_str()),
        Some("North 80")
    );
    assert_eq!(
        manifest_json
            .pointer("/field/boundary/coordinates")
            .and_then(|v| v.as_array())
            .map(|coords| coords.len()),
        Some(4)
    );

    Ok(())
}

#[tokio::test]
async fn scene_refresh_advisory_appears_for_fresher_lower_cloud_scene() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_advisory_field(&ctx, "advisory-field", "2026").await?;

    let current_scene_dir = ctx.data_root.join("scenes").join("current-scene");
    insert_advisory_scene(
        &ctx,
        "current-scene",
        Some("advisory-field"),
        Some("2026"),
        "2026-05-01T00:00:00Z",
        Some(62.0),
        &current_scene_dir,
        advisory_spatial_ref(),
    )
    .await?;

    let candidate_scene_dir = ctx.data_root.join("scenes").join("candidate-scene");
    insert_advisory_scene(
        &ctx,
        "candidate-scene",
        None,
        Some("2026"),
        "2026-06-01T00:00:00Z",
        Some(18.0),
        &candidate_scene_dir,
        advisory_spatial_ref(),
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/advisory-field/scene-refresh-advisories")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let advisories: serde_json::Value = serde_json::from_slice(&body)?;

    assert_eq!(
        advisories
            .get("advisory_enabled")
            .and_then(|value| value.as_bool()),
        Some(true),
        "response: {advisories}"
    );
    assert!(advisories
        .get("reason")
        .and_then(|value| value.as_str())
        .is_none());
    let items = advisories
        .get("advisories")
        .and_then(|value| value.as_array())
        .expect("advisories should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]
            .get("current_scene_id")
            .and_then(|value| value.as_str()),
        Some("current-scene")
    );
    assert_eq!(
        items[0]
            .get("candidate_scene_id")
            .and_then(|value| value.as_str()),
        Some("candidate-scene")
    );
    assert_eq!(
        items[0]
            .get("uncertainty")
            .and_then(|value| value.as_bool()),
        Some(false)
    );

    Ok(())
}

#[tokio::test]
async fn scene_refresh_advisory_is_disabled_when_current_metadata_integrity_fails() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_advisory_field(&ctx, "advisory-guard-field", "2026").await?;

    let current_scene_dir = ctx.data_root.join("scenes").join("guard-current");
    insert_advisory_scene(
        &ctx,
        "guard-current",
        Some("advisory-guard-field"),
        Some("2026"),
        "2026-05-01T00:00:00Z",
        Some(65.0),
        &current_scene_dir,
        advisory_spatial_ref(),
    )
    .await?;
    upsert_scene_spatial_ref(
        &ctx,
        "guard-current",
        json!({
            "georeferenced": true,
            "crs": "EPSG:4326",
            "bbox": {
                "min_lon": -1.0,
                "min_lat": 2.0,
                "max_lon": 3.0,
                "max_lat": 4.0
            },
            "geo_transform": [-1.0, 0.05, 0.0, 4.0, 0.0, -0.05],
            "resolution": {
                "x": 0.05,
                "y": 0.05
            }
        }),
    )
    .await?;

    let candidate_scene_dir = ctx.data_root.join("scenes").join("guard-candidate");
    insert_advisory_scene(
        &ctx,
        "guard-candidate",
        None,
        Some("2026"),
        "2026-06-01T00:00:00Z",
        Some(20.0),
        &candidate_scene_dir,
        advisory_spatial_ref(),
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/advisory-guard-field/scene-refresh-advisories")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let advisories: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        advisories
            .get("advisory_enabled")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert!(advisories
        .get("reason")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value.contains("advisory-gated: metadata-integrity")));
    assert!(advisories
        .get("advisories")
        .and_then(|value| value.as_array())
        .is_some_and(|items| items.is_empty()));

    Ok(())
}

#[tokio::test]
async fn scene_refresh_advisory_returns_empty_when_no_fresher_scene_exists() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_advisory_field(&ctx, "advisory-empty-field", "2026").await?;

    let current_scene_dir = ctx.data_root.join("scenes").join("current-old");
    insert_advisory_scene(
        &ctx,
        "current-old",
        Some("advisory-empty-field"),
        Some("2026"),
        "2026-07-01T00:00:00Z",
        Some(20.0),
        &current_scene_dir,
        advisory_spatial_ref(),
    )
    .await?;

    let older_scene_dir = ctx.data_root.join("scenes").join("older-scene");
    insert_advisory_scene(
        &ctx,
        "older-scene",
        None,
        Some("2026"),
        "2026-06-01T00:00:00Z",
        Some(10.0),
        &older_scene_dir,
        advisory_spatial_ref(),
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/advisory-empty-field/scene-refresh-advisories")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let advisories: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        advisories
            .get("advisory_enabled")
            .and_then(|value| value.as_bool()),
        Some(true),
        "response: {advisories}"
    );
    assert!(advisories
        .get("reason")
        .and_then(|value| value.as_str())
        .is_none());
    assert!(advisories
        .get("advisories")
        .and_then(|value| value.as_array())
        .is_some_and(|items| items.is_empty()));

    Ok(())
}

#[tokio::test]
async fn scene_change_advisory_summarizes_comparable_linked_scenes() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_advisory_field(&ctx, "change-field", "2026").await?;

    let baseline_dir = ctx.data_root.join("scenes").join("change-baseline");
    insert_advisory_scene(
        &ctx,
        "change-baseline",
        Some("change-field"),
        Some("2026"),
        "2026-05-01T00:00:00Z",
        Some(20.0),
        &baseline_dir,
        advisory_spatial_ref(),
    )
    .await?;
    let comparison_dir = ctx.data_root.join("scenes").join("change-comparison");
    insert_advisory_scene(
        &ctx,
        "change-comparison",
        Some("change-field"),
        Some("2026"),
        "2026-06-01T00:00:00Z",
        Some(45.0),
        &comparison_dir,
        advisory_spatial_ref(),
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/change-field/scene-change-advisories")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let advisories: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        advisories
            .get("advisory_enabled")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    let item = advisories
        .get("advisories")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .expect("change advisory should be emitted");
    assert_eq!(
        item.get("baseline_scene_id")
            .and_then(|value| value.as_str()),
        Some("change-baseline")
    );
    assert_eq!(
        item.get("comparison_scene_id")
            .and_then(|value| value.as_str()),
        Some("change-comparison")
    );
    assert_eq!(
        item.get("reason").and_then(|value| value.as_str()),
        Some("aligned-common-extent")
    );
    assert_eq!(
        item.get("confidence").and_then(|value| value.as_str()),
        Some("medium")
    );
    assert_eq!(
        item.get("coverage_fraction")
            .and_then(|value| value.as_f64()),
        Some(1.0)
    );
    assert_eq!(
        item.get("change_score").and_then(|value| value.as_f64()),
        Some(0.25)
    );
    assert!(item
        .get("common_extent")
        .is_some_and(|value| value.is_object()));

    Ok(())
}

#[tokio::test]
async fn scene_change_advisory_marks_spatial_mismatch_low_confidence() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_advisory_field(&ctx, "change-mismatch-field", "2026").await?;

    let baseline_dir = ctx
        .data_root
        .join("scenes")
        .join("change-mismatch-baseline");
    insert_advisory_scene(
        &ctx,
        "change-mismatch-baseline",
        Some("change-mismatch-field"),
        Some("2026"),
        "2026-05-01T00:00:00Z",
        Some(20.0),
        &baseline_dir,
        advisory_spatial_ref(),
    )
    .await?;
    let comparison_dir = ctx
        .data_root
        .join("scenes")
        .join("change-mismatch-comparison");
    insert_advisory_scene(
        &ctx,
        "change-mismatch-comparison",
        Some("change-mismatch-field"),
        Some("2026"),
        "2026-06-01T00:00:00Z",
        Some(45.0),
        &comparison_dir,
        json!({
            "georeferenced": true,
            "crs": "EPSG:3857",
            "bbox": {
                "min_lon": -96.8,
                "min_lat": 41.0,
                "max_lon": -96.2,
                "max_lat": 41.6
            },
            "geo_transform": [-96.8, 0.3, 0.0, 41.6, 0.0, -0.3],
            "resolution": {
                "x": 0.3,
                "y": 0.3
            }
        }),
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/change-mismatch-field/scene-change-advisories")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let advisories: serde_json::Value = serde_json::from_slice(&body)?;
    let item = advisories
        .get("advisories")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .expect("low-confidence advisory should be emitted");
    assert_eq!(
        item.get("confidence").and_then(|value| value.as_str()),
        Some("low")
    );
    assert!(item
        .get("reason")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value.contains("spatial-ref-mismatch")));
    assert!(item
        .get("common_extent")
        .is_some_and(|value| value.is_null()));
    assert_eq!(
        item.get("coverage_fraction")
            .and_then(|value| value.as_f64()),
        Some(0.0)
    );

    Ok(())
}

#[tokio::test]
async fn scene_change_advisory_returns_no_comparison_for_single_scene() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_advisory_field(&ctx, "change-single-field", "2026").await?;

    let scene_dir = ctx.data_root.join("scenes").join("change-single");
    insert_advisory_scene(
        &ctx,
        "change-single",
        Some("change-single-field"),
        Some("2026"),
        "2026-05-01T00:00:00Z",
        Some(20.0),
        &scene_dir,
        advisory_spatial_ref(),
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/change-single-field/scene-change-advisories")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let advisories: serde_json::Value = serde_json::from_slice(&body)?;
    assert!(advisories
        .get("reason")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value.contains("single-linked-scene")));
    assert!(advisories
        .get("advisories")
        .and_then(|value| value.as_array())
        .is_some_and(|items| items.is_empty()));

    Ok(())
}

#[tokio::test]
async fn linking_scene_to_field_rejects_non_overlapping_extent() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": "overlap-field",
                        "name": "Overlap Field",
                        "season": "2026",
                        "boundary": {
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.4 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let scene_id = "non-overlap-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
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
    .bind(
        json!({
            "metadata": {
                "timestamp": "2025-01-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 4,
                "height": 4,
                "spatial_ref": {
                    "georeferenced": true,
                    "crs": "EPSG:4326",
                    "bbox": {
                        "min_lon": -90.8,
                        "min_lat": 35.0,
                        "max_lon": -90.1,
                        "max_lat": 35.5
                    }
                }
            },
            "file_paths": {
                "B4": "B4.png",
                "B5": "B5.png"
            },
            "image_id": Uuid::new_v4()
        })
        .to_string(),
    )
    .bind(None::<f64>)
    .bind("2025-01-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    let link_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scenes/{scene_id}/field/overlap-field"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(link_response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(link_response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("no-overlap"));

    let linked_field: Option<String> =
        sqlx::query_scalar("SELECT field_id FROM scenes WHERE scene_id = ?1")
            .bind(scene_id)
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(linked_field, None);

    Ok(())
}

#[tokio::test]
async fn create_field_rejects_invalid_boundary() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "name": "Broken field",
                        "boundary": {
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn import_fields_geojson_creates_fields_from_feature_collection() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields/import/geojson")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "type": "FeatureCollection",
                        "features": [
                            {
                                "type": "Feature",
                                "id": "field-geojson-1",
                                "properties": {
                                    "name": "West Pivot",
                                    "crop": "soybean",
                                    "season": "2026",
                                    "notes": "imported from geojson",
                                    "crs": "EPSG:4326"
                                },
                                "geometry": {
                                    "type": "Polygon",
                                    "coordinates": [[
                                        [-96.7, 41.1],
                                        [-96.2, 41.1],
                                        [-96.2, 41.4],
                                        [-96.7, 41.4],
                                        [-96.7, 41.1]
                                    ]]
                                }
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let imported_json: serde_json::Value = serde_json::from_slice(&body)?;
    let imported = imported_json
        .as_array()
        .expect("import response should be an array");
    assert_eq!(imported.len(), 1);
    assert_eq!(
        imported[0].get("field_id").and_then(|v| v.as_str()),
        Some("field-geojson-1")
    );
    assert_eq!(
        imported[0].get("name").and_then(|v| v.as_str()),
        Some("West Pivot")
    );
    assert_eq!(
        imported_json
            .pointer("/0/boundary/crs")
            .and_then(|v| v.as_str()),
        Some("EPSG:4326")
    );

    let list_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let fields_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        fields_json
            .pointer("/items")
            .and_then(|fields| fields.as_array())
            .map(|fields| fields.len()),
        Some(1)
    );
    assert_eq!(
        fields_json
            .pointer("/items/0/extent/max_lat")
            .and_then(|v| v.as_f64()),
        Some(41.4)
    );

    Ok(())
}

#[tokio::test]
async fn export_fields_geojson_returns_feature_collection() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": "export-me",
                        "name": "Export Field",
                        "crop": "corn",
                        "season": "2026",
                        "notes": "geojson export test",
                        "boundary": {
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.4 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let export_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/export/geojson")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(export_response.status(), StatusCode::OK);

    let body = to_bytes(export_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson.get("type").and_then(|v| v.as_str()),
        Some("FeatureCollection")
    );
    assert_eq!(
        geojson.pointer("/features/0/id").and_then(|v| v.as_str()),
        Some("export-me")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/name")
            .and_then(|v| v.as_str()),
        Some("Export Field")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/geometry/type")
            .and_then(|v| v.as_str()),
        Some("Polygon")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/geometry/coordinates/0")
            .and_then(|v| v.as_array())
            .map(|ring| ring.len()),
        Some(5)
    );

    Ok(())
}

#[tokio::test]
async fn import_fields_geojson_rejects_non_polygon_geometry() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields/import/geojson")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "type": "Feature",
                        "properties": { "name": "Bad import" },
                        "geometry": {
                            "type": "LineString",
                            "coordinates": [
                                [-96.7, 41.1],
                                [-96.2, 41.1]
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn import_fields_shapefile_creates_fields_from_polygon_records() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let shapefile_path = tmp.path().join("north_field.shp");
    write_polygon_shapefile(
        &shapefile_path,
        &[vec![
            (-96.7, 41.1),
            (-96.2, 41.1),
            (-96.2, 41.4),
            (-96.7, 41.4),
            (-96.7, 41.1),
        ]],
    )?;
    write_wgs84_prj(&shapefile_path)?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields/import/shapefile")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "path": shapefile_path.to_string_lossy().to_string(),
                        "name_prefix": "North Boundary",
                        "crop": "corn",
                        "season": "2026",
                        "notes": "shapefile import"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let fields_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(fields_json.as_array().map(|items| items.len()), Some(1));
    assert_eq!(
        fields_json
            .pointer("/0/name")
            .and_then(|value| value.as_str()),
        Some("North Boundary")
    );
    assert_eq!(
        fields_json
            .pointer("/0/crop")
            .and_then(|value| value.as_str()),
        Some("corn")
    );
    assert_eq!(
        fields_json
            .pointer("/0/season")
            .and_then(|value| value.as_str()),
        Some("2026")
    );
    assert_eq!(
        fields_json
            .pointer("/0/boundary/coordinates")
            .and_then(|value| value.as_array())
            .map(|coords| coords.len()),
        Some(4)
    );
    assert_eq!(
        fields_json
            .pointer("/0/boundary/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );

    let list_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn import_fields_shapefile_rejects_missing_crs() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let shapefile_path = tmp.path().join("no_crs.shp");
    write_polygon_shapefile(
        &shapefile_path,
        &[vec![
            (-96.7, 41.1),
            (-96.2, 41.1),
            (-96.2, 41.4),
            (-96.7, 41.4),
            (-96.7, 41.1),
        ]],
    )?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields/import/shapefile")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "path": shapefile_path.to_string_lossy().to_string()
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("missing CRS"));

    Ok(())
}

#[tokio::test]
async fn import_fields_shapefile_rejects_unsupported_geometry() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let shapefile_path = tmp.path().join("points.shp");
    write_point_shapefile(&shapefile_path, &[(-96.45, 41.25)])?;
    write_wgs84_prj(&shapefile_path)?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields/import/shapefile")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "path": shapefile_path.to_string_lossy().to_string()
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("only polygon field boundaries are supported"));

    Ok(())
}

#[tokio::test]
async fn import_fields_shapefile_rejects_non_geographic_coordinates() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let shapefile_path = tmp.path().join("projected.shp");
    write_polygon_shapefile(
        &shapefile_path,
        &[vec![
            (500_000.0, 4_500_000.0),
            (500_100.0, 4_500_000.0),
            (500_100.0, 4_500_100.0),
            (500_000.0, 4_500_000.0),
        ]],
    )?;
    write_wgs84_prj(&shapefile_path)?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields/import/shapefile")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "path": shapefile_path.to_string_lossy().to_string()
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("EPSG:4326"));

    Ok(())
}

#[tokio::test]
async fn farm_crud_and_field_history_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-1",
                        "name": "River Bend",
                        "notes": "primary client farm"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let get_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/farms/farm-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get_response.status(), StatusCode::OK);

    for (field_id, season) in [("field-a", "2026"), ("field-b", "2025")] {
        let response = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/fields")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "farm_id": "farm-1",
                            "field_id": field_id,
                            "name": format!("Field {}", field_id),
                            "crop": "corn",
                            "season": season,
                            "boundary": {
                                "coordinates": [
                                    { "longitude": -96.7, "latitude": 41.1 },
                                    { "longitude": -96.2, "latitude": 41.1 },
                                    { "longitude": -96.2, "latitude": 41.4 }
                                ]
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let fields_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/farms/farm-1/fields")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(fields_response.status(), StatusCode::OK);
    let body = to_bytes(fields_response.into_body(), 64 * 1024).await?;
    let fields_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        fields_json
            .pointer("/items")
            .and_then(|items| items.as_array())
            .map(|items| items.len()),
        Some(2)
    );
    assert_eq!(
        fields_json
            .pointer("/items/0/farm_id")
            .and_then(|value| value.as_str()),
        Some("farm-1")
    );

    let history_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/farms/farm-1/fields/history")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(history_response.status(), StatusCode::OK);
    let body = to_bytes(history_response.into_body(), 64 * 1024).await?;
    let history_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(history_json.as_array().map(|items| items.len()), Some(2));

    let update_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/farms/farm-1")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "name": "River Bend Updated",
                        "notes": "updated farm notes"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(update_response.status(), StatusCode::OK);

    let delete_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/farms/farm-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    Ok(())
}

#[tokio::test]
async fn farm_field_lists_paginate_scope_and_filter_lifecycle_status() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    for (farm_id, owner, name, status) in [
        ("farm-active", "org-alpha", "Active Farm", None),
        (
            "farm-archived",
            "org-alpha",
            "Archived Farm",
            Some("archived"),
        ),
        ("farm-foreign", "org-beta", "Foreign Farm", None),
    ] {
        let mut body = json!({
            "farm_id": farm_id,
            "owner": owner,
            "name": name
        });
        if let Some(status) = status {
            body["status"] = json!(status);
        }
        let response = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/farms")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    for (field_id, farm_id, name, status) in [
        ("field-alpha", "farm-active", "Alpha Field", None),
        ("field-beta", "farm-active", "Beta Field", None),
        ("field-gamma", "farm-active", "Gamma Field", None),
        (
            "field-archived",
            "farm-active",
            "Archived Field",
            Some("archived"),
        ),
        ("field-foreign", "farm-foreign", "Foreign Field", None),
    ] {
        let mut body = json!({
            "farm_id": farm_id,
            "field_id": field_id,
            "name": name,
            "boundary": {
                "coordinates": [
                    { "longitude": -96.7, "latitude": 41.1 },
                    { "longitude": -96.2, "latitude": 41.1 },
                    { "longitude": -96.2, "latitude": 41.4 }
                ]
            }
        });
        if let Some(status) = status {
            body["status"] = json!(status);
        }
        let response = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/fields")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let page_two = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields?org_id=org-alpha&page=2&page_size=2")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(page_two.status(), StatusCode::OK);
    let body = to_bytes(page_two.into_body(), 64 * 1024).await?;
    let page_two_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        page_two_json
            .get("total_count")
            .and_then(|value| value.as_u64()),
        Some(3)
    );
    assert_eq!(
        page_two_json.get("page").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        page_two_json
            .get("page_size")
            .and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        page_two_json
            .pointer("/items/0/field_id")
            .and_then(|value| value.as_str()),
        Some("field-gamma")
    );
    assert_eq!(
        page_two_json
            .pointer("/items/0/status")
            .and_then(|value| value.as_str()),
        Some("active")
    );
    assert!(page_two_json
        .pointer("/items/0/updated_at")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.is_empty()));

    let beyond = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields?org_id=org-alpha&page=4&page_size=2")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(beyond.status(), StatusCode::OK);
    let body = to_bytes(beyond.into_body(), 64 * 1024).await?;
    let beyond_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        beyond_json
            .get("total_count")
            .and_then(|value| value.as_u64()),
        Some(3)
    );
    assert_eq!(
        beyond_json
            .pointer("/items")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(0)
    );

    let archived_fields = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields?org_id=org-alpha&status=archived")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(archived_fields.status(), StatusCode::OK);
    let body = to_bytes(archived_fields.into_body(), 64 * 1024).await?;
    let archived_fields_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        archived_fields_json
            .pointer("/items/0/field_id")
            .and_then(|value| value.as_str()),
        Some("field-archived")
    );

    let active_farms = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/farms?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(active_farms.status(), StatusCode::OK);
    let body = to_bytes(active_farms.into_body(), 64 * 1024).await?;
    let active_farms_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        active_farms_json
            .pointer("/items/0/farm_id")
            .and_then(|value| value.as_str()),
        Some("farm-active")
    );
    assert_eq!(
        active_farms_json
            .pointer("/items/0/status")
            .and_then(|value| value.as_str()),
        Some("active")
    );
    assert_eq!(
        active_farms_json
            .pointer("/items")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(1)
    );

    let archived_farms = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/farms?org_id=org-alpha&status=archived")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(archived_farms.status(), StatusCode::OK);
    let body = to_bytes(archived_farms.into_body(), 64 * 1024).await?;
    let archived_farms_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        archived_farms_json
            .pointer("/items/0/farm_id")
            .and_then(|value| value.as_str()),
        Some("farm-archived")
    );

    let archived_boundaries = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/boundaries?org_id=org-alpha&status=archived")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(archived_boundaries.status(), StatusCode::OK);
    let body = to_bytes(archived_boundaries.into_body(), 64 * 1024).await?;
    let archived_boundaries_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        archived_boundaries_json
            .pointer("/items/0/field_id")
            .and_then(|value| value.as_str()),
        Some("field-archived")
    );
    assert_eq!(
        archived_boundaries_json
            .pointer("/items/0/boundary/coordinates")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(3)
    );

    Ok(())
}

#[tokio::test]
async fn farm_field_scene_identity_persists_after_restart() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create_farm = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-owned",
                        "owner": "org-alpha",
                        "name": "Owned Farm"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_farm.status(), StatusCode::OK);
    let body = to_bytes(create_farm.into_body(), 64 * 1024).await?;
    let farm_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        farm_json.get("owner").and_then(|value| value.as_str()),
        Some("org-alpha")
    );
    let farm_created_at = farm_json
        .get("created_at")
        .and_then(|value| value.as_str())
        .expect("farm created_at should be returned")
        .to_string();
    assert!(!farm_created_at.trim().is_empty());

    let create_field = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-owned",
                        "field_id": "field-owned",
                        "name": "Owned Field",
                        "season": "2026",
                        "boundary": {
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_field.status(), StatusCode::OK);
    let body = to_bytes(create_field.into_body(), 64 * 1024).await?;
    let field_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        field_json.get("owner").and_then(|value| value.as_str()),
        Some("org-alpha")
    );
    let field_created_at = field_json
        .get("created_at")
        .and_then(|value| value.as_str())
        .expect("field created_at should be returned")
        .to_string();
    assert!(!field_created_at.trim().is_empty());

    let scene_id = "scene-owned";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at, field_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(scene_id)
    .bind("org-alpha")
    .bind("landsat8")
    .bind("2026-05-01T00:00:00Z")
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(
        json!({
            "metadata": {
                "timestamp": "2026-05-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 1,
                "height": 1,
                "spatial_ref": {
                    "georeferenced": true,
                    "crs": "EPSG:4326",
                    "bbox": {
                        "min_lon": -96.8,
                        "min_lat": 41.0,
                        "max_lon": -96.1,
                        "max_lat": 41.5
                    }
                }
            },
            "file_paths": {
                "B4": "B4.png",
                "B5": "B5.png"
            },
            "image_id": Uuid::new_v4()
        })
        .to_string(),
    )
    .bind(None::<f64>)
    .bind("2026-05-01T00:00:00Z")
    .bind("field-owned")
    .execute(&ctx.pool)
    .await?;

    let restarted =
        test_app_with_paths(ctx.data_root.clone(), tmp.path().join("geo_hub_test.db")).await?;

    let get_farm = restarted
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/farms/farm-owned")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get_farm.status(), StatusCode::OK);
    let body = to_bytes(get_farm.into_body(), 64 * 1024).await?;
    let farm_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        farm_json.get("owner").and_then(|value| value.as_str()),
        Some("org-alpha")
    );
    assert_eq!(
        farm_json.get("created_at").and_then(|value| value.as_str()),
        Some(farm_created_at.as_str())
    );

    let get_field = restarted
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/field-owned")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get_field.status(), StatusCode::OK);
    let body = to_bytes(get_field.into_body(), 64 * 1024).await?;
    let field_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        field_json.get("owner").and_then(|value| value.as_str()),
        Some("org-alpha")
    );
    assert_eq!(
        field_json
            .get("created_at")
            .and_then(|value| value.as_str()),
        Some(field_created_at.as_str())
    );

    let get_scene = restarted
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get_scene.status(), StatusCode::OK);
    let body = to_bytes(get_scene.into_body(), 64 * 1024).await?;
    let scene_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        scene_json.get("owner").and_then(|value| value.as_str()),
        Some("org-alpha")
    );
    assert_eq!(
        scene_json
            .get("created_at")
            .and_then(|value| value.as_str()),
        Some("2026-05-01T00:00:00Z")
    );

    Ok(())
}

#[tokio::test]
async fn fleet_node_enrollment_lists_gets_and_rebinds_duplicate_hardware() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let enroll_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/nodes/enroll")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "hardware_id": "hw-drone-001",
                        "kind": "drone",
                        "capabilities": ["multispectral", " lidar ", "lidar"],
                        "owner_org_id": "org-alpha",
                        "runtime_mode": "simulation"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(enroll_response.status(), StatusCode::OK);
    let body = to_bytes(enroll_response.into_body(), 64 * 1024).await?;
    let enrolled: serde_json::Value = serde_json::from_slice(&body)?;
    let node_id = enrolled
        .get("node_id")
        .and_then(|value| value.as_str())
        .expect("node_id should be returned")
        .to_string();
    assert!(!node_id.trim().is_empty());
    assert_eq!(
        enrolled.get("hardware_id").and_then(|value| value.as_str()),
        Some("hw-drone-001")
    );
    assert_eq!(
        enrolled.get("kind").and_then(|value| value.as_str()),
        Some("drone")
    );
    assert_eq!(
        enrolled
            .get("owner_org_id")
            .and_then(|value| value.as_str()),
        Some("org-alpha")
    );
    assert_eq!(
        enrolled
            .get("runtime_mode")
            .and_then(|value| value.as_str()),
        Some("simulation")
    );
    assert_eq!(
        enrolled
            .get("capabilities")
            .and_then(|value| value.as_array()),
        Some(&vec![json!("lidar"), json!("multispectral")])
    );
    assert_eq!(
        enrolled.get("status").and_then(|value| value.as_str()),
        Some("enrolled")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fleet/nodes?owner_org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("node_id").and_then(|value| value.as_str()),
        Some(node_id.as_str())
    );

    let get_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/fleet/nodes/{node_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get_response.status(), StatusCode::OK);
    let body = to_bytes(get_response.into_body(), 64 * 1024).await?;
    let fetched: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        fetched.get("node_id").and_then(|value| value.as_str()),
        Some(node_id.as_str())
    );

    let duplicate_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/nodes/enroll")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "hardware_id": "hw-drone-001",
                        "kind": "drone",
                        "capabilities": ["thermal"],
                        "owner_org_id": "org-beta",
                        "runtime_mode": "flight"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(duplicate_response.status(), StatusCode::OK);
    let body = to_bytes(duplicate_response.into_body(), 64 * 1024).await?;
    let duplicate: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        duplicate.get("node_id").and_then(|value| value.as_str()),
        Some(node_id.as_str())
    );
    assert_eq!(
        duplicate
            .get("owner_org_id")
            .and_then(|value| value.as_str()),
        Some("org-alpha")
    );

    let node_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM fleet_nodes WHERE hardware_id = ?1")
            .bind("hw-drone-001")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(node_count, 1);

    Ok(())
}

#[tokio::test]
async fn fleet_node_enrollment_rejects_missing_hardware_identity() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/nodes/enroll")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "hardware_id": "  ",
                        "kind": "edge",
                        "capabilities": ["compute"],
                        "owner_org_id": "org-alpha",
                        "runtime_mode": "simulation"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let node_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fleet_nodes")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(node_count, 0);

    Ok(())
}

#[tokio::test]
async fn tractor_registry_registers_lists_and_audits_rejected_motion_commands() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_tractor_registry_field(&ctx).await?;

    let register_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tractors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "tractor_id": "tractor-001",
                        "org_id": "org-alpha",
                        "field_id": "field-tractor",
                        "capabilities": ["RTK", "planter", "rtk"],
                        "implement_ref": {
                            "implement_id": "implement-planter-1",
                            "implement_type": "Planter",
                            "working_width_m": 9.1
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(register_response.status(), StatusCode::OK);
    let body = to_bytes(register_response.into_body(), 64 * 1024).await?;
    let registered: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        registered
            .get("tractor_id")
            .and_then(|value| value.as_str()),
        Some("tractor-001")
    );
    assert_eq!(
        registered
            .pointer("/capabilities/0")
            .and_then(|value| value.as_str()),
        Some("planter")
    );
    assert_eq!(
        registered
            .pointer("/capabilities/1")
            .and_then(|value| value.as_str()),
        Some("rtk")
    );
    assert_eq!(
        registered.get("status").and_then(|value| value.as_str()),
        Some("registered")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/tractors?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("tractor_id").and_then(|value| value.as_str()),
        Some("tractor-001")
    );

    let unknown_command = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tractors/tractor-missing/motion-commands/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "command_id": "cmd-unknown",
                        "command_type": "move",
                        "requested_by": "ops@example.com"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(unknown_command.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(unknown_command.into_body(), 64 * 1024).await?;
    let unknown: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        unknown.get("reason").and_then(|value| value.as_str()),
        Some("unknown_tractor")
    );
    assert_eq!(
        unknown
            .pointer("/audit/reason_code")
            .and_then(|value| value.as_str()),
        Some("tractor_not_registered")
    );

    let out_of_service_register = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tractors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "tractor_id": "tractor-oos",
                        "org_id": "org-alpha",
                        "field_id": "field-tractor",
                        "capabilities": ["sprayer"],
                        "implement_ref": {
                            "implement_id": "implement-sprayer-1",
                            "implement_type": "sprayer"
                        },
                        "status": "out_of_service"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(out_of_service_register.status(), StatusCode::OK);

    let out_of_service_command = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tractors/tractor-oos/motion-commands/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "command_id": "cmd-oos",
                        "command_type": "move",
                        "requested_by": "ops@example.com"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(out_of_service_command.status(), StatusCode::CONFLICT);
    let body = to_bytes(out_of_service_command.into_body(), 64 * 1024).await?;
    let out_of_service: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        out_of_service
            .get("reason")
            .and_then(|value| value.as_str()),
        Some("tractor_out_of_service")
    );

    let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tractor_command_audits")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(audit_count, 2);

    Ok(())
}

#[tokio::test]
async fn tractor_registration_rejects_cross_tenant_field_link() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_tractor_registry_field(&ctx).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tractors")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "tractor_id": "tractor-cross-tenant",
                        "org_id": "org-beta",
                        "field_id": "field-tractor",
                        "capabilities": ["rtk"],
                        "implement_ref": {
                            "implement_id": "implement-1",
                            "implement_type": "planter"
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let tractor_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tractor_vehicles")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(tractor_count, 0);

    Ok(())
}

#[tokio::test]
async fn weather_forecast_pull_normalizes_values_with_per_value_evidence() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_weather_forecast_field(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/weather/forecasts/pull")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": "field-weather",
                        "provider": "sample",
                        "latitude": 41.2,
                        "longitude": -96.5,
                        "fetched_at": "2026-06-13T10:00:00Z",
                        "valid_time": "2026-06-13T11:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let records: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        records
            .pointer("/0/field_ref")
            .and_then(|value| value.as_str()),
        Some("field:field-weather")
    );
    assert_eq!(
        records
            .pointer("/0/source")
            .and_then(|value| value.as_str()),
        Some("sample")
    );
    assert_eq!(
        records
            .pointer("/0/vars/temperature_celsius/source")
            .and_then(|value| value.as_str()),
        Some("sample")
    );
    assert_eq!(
        records
            .pointer("/0/vars/temperature_celsius/fetched_at")
            .and_then(|value| value.as_str()),
        Some("2026-06-13T10:00:00Z")
    );
    assert_eq!(
        records
            .pointer("/0/vars/wind_speed_mps/unit")
            .and_then(|value| value.as_str()),
        Some("m/s")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/weather/forecasts?field_id=field-weather")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        listed
            .pointer("/0/forecast_id")
            .and_then(|value| value.as_str()),
        Some("weather:field-field-weather:sample:2026-06-13T11-00-00Z")
    );

    let forecast_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM weather_forecasts")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(forecast_count, 1);
    let series_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM time_series_points WHERE entity_ref = ?1")
            .bind("field:field-weather")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(series_count, 5);

    let wind_series = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/time-series/points?entity_ref=field:field-weather&metric=wind_speed_mps")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(wind_series.status(), StatusCode::OK);
    let body = to_bytes(wind_series.into_body(), 64 * 1024).await?;
    let wind: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        wind.pointer("/0/source_ref")
            .and_then(|value| value.as_str()),
        Some("weather:field-field-weather:sample:2026-06-13T11-00-00Z")
    );
    assert_eq!(
        wind.pointer("/0/metadata/source")
            .and_then(|value| value.as_str()),
        Some("sample")
    );

    Ok(())
}

#[tokio::test]
async fn weather_forecast_pull_records_provider_failure_without_partial_insert() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_weather_forecast_field(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/weather/forecasts/pull")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": "field-weather",
                        "provider": "unreachable",
                        "latitude": 41.2,
                        "longitude": -96.5,
                        "fetched_at": "2026-06-13T10:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let failure: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        failure.get("field_ref").and_then(|value| value.as_str()),
        Some("field:field-weather")
    );
    assert_eq!(
        failure.get("reason").and_then(|value| value.as_str()),
        Some("provider unreachable")
    );

    let forecast_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM weather_forecasts")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(forecast_count, 0);
    let failure_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM weather_fetch_failures")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(failure_count, 1);
    let series_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM time_series_points")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(series_count, 0);

    let list_failures = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/weather/fetch-failures?field_id=field-weather")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_failures.status(), StatusCode::OK);
    let body = to_bytes(list_failures.into_body(), 64 * 1024).await?;
    let failures: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        failures
            .pointer("/0/reason")
            .and_then(|value| value.as_str()),
        Some("provider unreachable")
    );

    Ok(())
}

#[tokio::test]
async fn water_management_moisture_reading_persists_field_zone_qa_and_series() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_water_management_field(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/water-management/moisture-readings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "reading_id": "moisture-001",
                        "field_id": "field-water",
                        "zone_ref": "zone:north",
                        "value": 31.25,
                        "source": "probe:soil-001",
                        "captured_at": "2026-06-13T09:30:00Z",
                        "qa_flag": "valid"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let reading: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        reading.get("reading_id").and_then(|value| value.as_str()),
        Some("moisture-001")
    );
    assert_eq!(
        reading.get("field_id").and_then(|value| value.as_str()),
        Some("field-water")
    );
    assert_eq!(
        reading.get("zone_ref").and_then(|value| value.as_str()),
        Some("zone:north")
    );
    assert_eq!(
        reading.get("qa_flag").and_then(|value| value.as_str()),
        Some("valid")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/water-management/moisture-readings?field_id=field-water&zone_ref=zone:north",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0]
            .get("captured_at")
            .and_then(|value| value.as_str()),
        Some("2026-06-13T09:30:00Z")
    );

    let accepted_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM water_moisture_readings")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(accepted_count, 1);

    let series_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/time-series/points?entity_ref=field:field-water:zone:zone:north&metric=soil_moisture_percent",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(series_response.status(), StatusCode::OK);
    let body = to_bytes(series_response.into_body(), 64 * 1024).await?;
    let points: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]
            .pointer("/value/value")
            .and_then(|value| value.as_f64()),
        Some(31.25)
    );
    assert_eq!(
        points[0]
            .pointer("/metadata/qa_flag")
            .and_then(|value| value.as_str()),
        Some("valid")
    );

    Ok(())
}

#[tokio::test]
async fn water_management_moisture_reading_rejects_unlinked_reading_with_audit() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_water_management_field(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/water-management/moisture-readings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "reading_id": "moisture-orphan",
                        "field_id": "field-water",
                        "value": 31.25,
                        "source": "probe:soil-001",
                        "captured_at": "2026-06-13T09:30:00Z",
                        "qa_flag": "valid"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let rejection: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        rejection.get("reason").and_then(|value| value.as_str()),
        Some("missing_zone_linkage")
    );

    let accepted_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM water_moisture_readings")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(accepted_count, 0);
    let rejection_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM water_moisture_reading_rejections")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(rejection_count, 1);
    let series_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM time_series_points")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(series_count, 0);

    let list_rejections = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/water-management/moisture-reading-rejections?field_id=field-water")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_rejections.status(), StatusCode::OK);
    let body = to_bytes(list_rejections.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("zone_ref").and_then(|value| value.as_str()),
        None
    );

    Ok(())
}

#[tokio::test]
async fn drought_index_compute_persists_field_linked_traceable_index() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_drought_management_field(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/drought-management/indices/compute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "index_id": "drought-spi-001",
                        "field_or_region_ref": "field:field-drought",
                        "index_type": "spi",
                        "period": {
                            "start": "2026-04-01",
                            "end": "2026-06-30",
                            "accumulation_days": 90
                        },
                        "observed_value": 42.0,
                        "baseline_mean": 60.0,
                        "baseline_std_dev": 12.0,
                        "input_refs": [
                            "weather:field-drought:precip:2026-Q2",
                            "water:field-drought:balance:2026-Q2"
                        ],
                        "computed_at": "2026-06-13T10:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let index: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        index.get("index_id").and_then(|value| value.as_str()),
        Some("drought-spi-001")
    );
    assert_eq!(
        index
            .get("field_or_region_ref")
            .and_then(|value| value.as_str()),
        Some("field:field-drought")
    );
    assert_eq!(
        index.get("index_type").and_then(|value| value.as_str()),
        Some("spi")
    );
    assert_eq!(
        index.get("value").and_then(|value| value.as_f64()),
        Some(-1.5)
    );
    assert_eq!(
        index
            .pointer("/input_refs/0")
            .and_then(|value| value.as_str()),
        Some("water:field-drought:balance:2026-Q2")
    );
    assert_eq!(
        index.get("method").and_then(|value| value.as_str()),
        Some("standardized_anomaly_v1")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/drought-management/indices?field_or_region_ref=field:field-drought")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("index_id").and_then(|value| value.as_str()),
        Some("drought-spi-001")
    );

    let index_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drought_indices")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(index_count, 1);

    Ok(())
}

#[tokio::test]
async fn drought_index_compute_rejects_untraceable_index_without_persisting() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_drought_management_field(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/drought-management/indices/compute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "index_id": "drought-spi-untraceable",
                        "field_or_region_ref": "field:field-drought",
                        "index_type": "spi",
                        "period": {
                            "start": "2026-04-01",
                            "end": "2026-06-30",
                            "accumulation_days": 90
                        },
                        "observed_value": 42.0,
                        "baseline_mean": 60.0,
                        "baseline_std_dev": 12.0,
                        "input_refs": [],
                        "computed_at": "2026-06-13T10:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let index_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drought_indices")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(index_count, 0);

    Ok(())
}

#[tokio::test]
async fn marketplace_accounts_create_list_suspend_and_deny_cross_tenant_read() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_org(&ctx, "farm-market-beta", "org-beta").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "account_id": "supplier-001",
                        "org_id": "org-alpha",
                        "party_type": "supplier",
                        "role_refs": ["marketplace:seller", "inventory-admin"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let account: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        account.get("account_id").and_then(|value| value.as_str()),
        Some("supplier-001")
    );
    assert_eq!(
        account.get("party_type").and_then(|value| value.as_str()),
        Some("supplier")
    );
    assert_eq!(
        account.get("status").and_then(|value| value.as_str()),
        Some("active")
    );

    let unscoped_list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/accounts")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(unscoped_list.status(), StatusCode::BAD_REQUEST);

    let alpha_list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/accounts?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(alpha_list.status(), StatusCode::OK);
    let body = to_bytes(alpha_list.into_body(), 64 * 1024).await?;
    let accounts: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(accounts.len(), 1);

    let beta_list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/accounts?org_id=org-beta")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(beta_list.status(), StatusCode::OK);
    let body = to_bytes(beta_list.into_body(), 64 * 1024).await?;
    let beta_accounts: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert!(beta_accounts.is_empty());

    let cross_tenant = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/accounts/supplier-001?org_id=org-beta")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_tenant.status(), StatusCode::NOT_FOUND);

    let suspend = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/accounts/supplier-001/status")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "org_id": "org-alpha",
                        "status": "suspended"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(suspend.status(), StatusCode::OK);
    let body = to_bytes(suspend.into_body(), 64 * 1024).await?;
    let suspended: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        suspended.get("status").and_then(|value| value.as_str()),
        Some("suspended")
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_account_create_rejects_unknown_org_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "account_id": "supplier-missing",
                        "org_id": "org-missing",
                        "party_type": "supplier",
                        "role_refs": ["marketplace:seller"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_accounts")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(account_count, 0);

    Ok(())
}

#[tokio::test]
async fn marketplace_catalog_items_create_get_and_list_by_org() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(&ctx, "supplier-001", "org-alpha", "supplier").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/catalog/items")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "kind": "input",
                        "category": "seed",
                        "name": "Hybrid corn seed",
                        "unit_of_measure": "bag",
                        "owner_account_id": "supplier-001"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let item: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        item.get("item_id").and_then(|value| value.as_str()),
        Some("seed-corn-001")
    );
    assert_eq!(
        item.get("unit_of_measure").and_then(|value| value.as_str()),
        Some("bag")
    );

    let fetched = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/catalog/items/seed-corn-001?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(fetched.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/catalog/items?org_id=org-alpha&kind=input")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(items.len(), 1);

    let cross_tenant = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/catalog/items/seed-corn-001?org_id=org-beta")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_tenant.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn marketplace_catalog_item_rejects_invalid_unit_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(&ctx, "supplier-001", "org-alpha", "supplier").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/catalog/items")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "kind": "input",
                        "category": "seed",
                        "name": "Hybrid corn seed",
                        "unit_of_measure": "pallet",
                        "owner_account_id": "supplier-001"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert!(response.status().is_client_error());

    let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_catalog_items")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(item_count, 0);

    Ok(())
}

#[tokio::test]
async fn marketplace_portal_entry_is_visible_only_with_access_role() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(&ctx, "grower-001", "org-alpha", "grower").await?;
    seed_marketplace_account_with_roles(
        &ctx,
        "viewer-001",
        "org-alpha",
        "grower",
        &["portal:viewer"],
    )
    .await?;

    let entry = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/marketplace-entry?org_id=org-alpha&account_id=grower-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(entry.status(), StatusCode::OK);
    let body = to_bytes(entry.into_body(), 64 * 1024).await?;
    let entry: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        entry.get("label").and_then(|value| value.as_str()),
        Some("Marketplace")
    );
    assert_eq!(
        entry.get("href").and_then(|value| value.as_str()),
        Some("/marketplace?org_id=org-alpha")
    );
    assert_eq!(
        entry.get("visible").and_then(|value| value.as_bool()),
        Some(true)
    );

    let forbidden = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/marketplace-entry?org_id=org-alpha&account_id=viewer-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn marketplace_listings_publish_get_list_and_close() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(&ctx, "supplier-001", "org-alpha", "supplier").await?;
    seed_marketplace_catalog_item(&ctx, "seed-corn-001", "org-alpha", "supplier-001").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/listings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "listing_id": "listing-seed-corn-001",
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "price": 125.0,
                        "currency": "USD",
                        "available_qty": 40.0,
                        "window": {
                            "from": "2026-06-14T09:00:00Z",
                            "to": "2026-07-14T09:00:00Z"
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let listing: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        listing.get("status").and_then(|value| value.as_str()),
        Some("published")
    );

    let fetched = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/listings/listing-seed-corn-001?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(fetched.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/listings?org_id=org-alpha&status=published")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let listings: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listings.len(), 1);

    let close = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/listings/listing-seed-corn-001/close")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "org_id": "org-alpha" }).to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(close.status(), StatusCode::OK);
    let body = to_bytes(close.into_body(), 64 * 1024).await?;
    let closed: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        closed.get("status").and_then(|value| value.as_str()),
        Some("closed")
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_listing_rejects_inverted_window_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(&ctx, "supplier-001", "org-alpha", "supplier").await?;
    seed_marketplace_catalog_item(&ctx, "seed-corn-001", "org-alpha", "supplier-001").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/listings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "listing_id": "listing-seed-corn-001",
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "price": 125.0,
                        "currency": "USD",
                        "available_qty": 40.0,
                        "window": {
                            "from": "2026-07-14T09:00:00Z",
                            "to": "2026-06-14T09:00:00Z"
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let listing_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_listings")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(listing_count, 0);

    Ok(())
}

#[tokio::test]
async fn marketplace_inventory_create_list_reserve_fulfill_and_release() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(&ctx, "supplier-001", "org-alpha", "supplier").await?;
    seed_marketplace_catalog_item(&ctx, "seed-corn-001", "org-alpha", "supplier-001").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/inventory")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "inventory_id": "inventory-seed-corn-001",
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "on_hand": 40.0,
                        "reserved": 0.0
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);

    let reserve = marketplace_inventory_adjust(
        &ctx,
        "inventory-seed-corn-001",
        "reserve",
        "org-alpha",
        25.0,
    )
    .await?;
    assert_eq!(reserve.status(), StatusCode::OK);
    let body = to_bytes(reserve.into_body(), 64 * 1024).await?;
    let reserved: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        reserved.get("reserved").and_then(|value| value.as_f64()),
        Some(25.0)
    );

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/inventory?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let inventory: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(inventory.len(), 1);

    let fulfill = marketplace_inventory_adjust(
        &ctx,
        "inventory-seed-corn-001",
        "fulfill",
        "org-alpha",
        10.0,
    )
    .await?;
    assert_eq!(fulfill.status(), StatusCode::OK);
    let body = to_bytes(fulfill.into_body(), 64 * 1024).await?;
    let fulfilled: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        fulfilled.get("on_hand").and_then(|value| value.as_f64()),
        Some(30.0)
    );
    assert_eq!(
        fulfilled.get("reserved").and_then(|value| value.as_f64()),
        Some(15.0)
    );

    let release = marketplace_inventory_adjust(
        &ctx,
        "inventory-seed-corn-001",
        "release",
        "org-alpha",
        15.0,
    )
    .await?;
    assert_eq!(release.status(), StatusCode::OK);
    let body = to_bytes(release.into_body(), 64 * 1024).await?;
    let released: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        released.get("on_hand").and_then(|value| value.as_f64()),
        Some(30.0)
    );
    assert_eq!(
        released.get("reserved").and_then(|value| value.as_f64()),
        Some(0.0)
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_inventory_rejects_parallel_over_reserve_without_oversell() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_org(&ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(&ctx, "supplier-001", "org-alpha", "supplier").await?;
    seed_marketplace_catalog_item(&ctx, "seed-corn-001", "org-alpha", "supplier-001").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/inventory")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "inventory_id": "inventory-seed-corn-001",
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "on_hand": 40.0,
                        "reserved": 0.0
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);

    let (first, second) = tokio::join!(
        marketplace_inventory_adjust(
            &ctx,
            "inventory-seed-corn-001",
            "reserve",
            "org-alpha",
            30.0,
        ),
        marketplace_inventory_adjust(
            &ctx,
            "inventory-seed-corn-001",
            "reserve",
            "org-alpha",
            30.0,
        )
    );
    let statuses = [first?.status(), second?.status()];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::BAD_REQUEST));

    let row: (f64, f64) = sqlx::query_as(
        "SELECT on_hand, reserved FROM marketplace_inventory WHERE inventory_id = ?1",
    )
    .bind("inventory-seed-corn-001")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.0, 40.0);
    assert_eq!(row.1, 30.0);

    Ok(())
}

#[tokio::test]
async fn marketplace_orders_place_transition_and_audit() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_order_dependencies(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/orders")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "order_id": "order-seed-corn-001",
                        "org_id": "org-alpha",
                        "listing_ref": "listing-seed-corn-001",
                        "buyer_account_id": "buyer-001",
                        "qty": 3.0
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let order: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        order.get("status").and_then(|value| value.as_str()),
        Some("placed")
    );
    assert_eq!(
        order.get("line_total").and_then(|value| value.as_f64()),
        Some(375.0)
    );

    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "confirmed").await?,
        StatusCode::OK
    );
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "fulfilled").await?,
        StatusCode::OK
    );
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "closed").await?,
        StatusCode::OK
    );

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/orders?org_id=org-alpha&status=closed")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let orders: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(orders.len(), 1);

    let audits = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/orders/order-seed-corn-001/audits?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(audits.status(), StatusCode::OK);
    let body = to_bytes(audits.into_body(), 64 * 1024).await?;
    let audits: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(audits.len(), 4);

    let row: (f64, f64) = sqlx::query_as(
        "SELECT on_hand, reserved FROM marketplace_inventory WHERE inventory_id = ?1",
    )
    .bind("inventory-seed-corn-001")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.0, 37.0);
    assert_eq!(row.1, 0.0);

    Ok(())
}

#[tokio::test]
async fn marketplace_orders_reject_illegal_transition_without_state_change() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_order_dependencies(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/orders")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "order_id": "order-seed-corn-001",
                        "org_id": "org-alpha",
                        "listing_ref": "listing-seed-corn-001",
                        "buyer_account_id": "buyer-001",
                        "qty": 3.0
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);

    let rejected = marketplace_order_transition(&ctx, "order-seed-corn-001", "closed").await?;
    assert_eq!(rejected, StatusCode::BAD_REQUEST);

    let row: (String, i64) = sqlx::query_as(
        "SELECT status, (SELECT COUNT(*) FROM marketplace_order_audits WHERE order_id = ?1) \
         FROM marketplace_orders WHERE order_id = ?1",
    )
    .bind("order-seed-corn-001")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.0, "placed");
    assert_eq!(row.1, 1);

    Ok(())
}

#[tokio::test]
async fn marketplace_fulfillments_record_and_advance_confirmed_order() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_order_dependencies(&ctx).await?;
    seed_marketplace_order(&ctx, "order-seed-corn-001", 3.0).await?;
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "confirmed").await?,
        StatusCode::OK
    );

    let response =
        create_marketplace_fulfillment(&ctx, "fulfillment-001", "order-seed-corn-001", "org-alpha")
            .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let fulfillment: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        fulfillment.get("status").and_then(|value| value.as_str()),
        Some("pending")
    );

    let order_status: String =
        sqlx::query_scalar("SELECT status FROM marketplace_orders WHERE order_id = ?1")
            .bind("order-seed-corn-001")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(order_status, "fulfilled");

    let transition = marketplace_fulfillment_transition(&ctx, "fulfillment-001", "shipped").await?;
    assert_eq!(transition, StatusCode::OK);
    let audits = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/fulfillments/fulfillment-001/audits?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(audits.status(), StatusCode::OK);
    let body = to_bytes(audits.into_body(), 64 * 1024).await?;
    let audits: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(audits.len(), 2);

    Ok(())
}

#[tokio::test]
async fn marketplace_fulfillments_reject_cross_tenant_order_link() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_order_dependencies(&ctx).await?;
    seed_marketplace_order(&ctx, "order-seed-corn-001", 3.0).await?;
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "confirmed").await?,
        StatusCode::OK
    );

    let response =
        create_marketplace_fulfillment(&ctx, "fulfillment-001", "order-seed-corn-001", "org-beta")
            .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let fulfillment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_fulfillments")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(fulfillment_count, 0);

    Ok(())
}

#[tokio::test]
async fn marketplace_fulfillments_reject_missing_order_without_write() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response =
        create_marketplace_fulfillment(&ctx, "fulfillment-001", "missing-order", "org-alpha")
            .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let fulfillment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_fulfillments")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(fulfillment_count, 0);

    Ok(())
}

#[tokio::test]
async fn marketplace_ratings_persist_and_aggregate_for_participants() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_order_dependencies(&ctx).await?;
    seed_marketplace_order(&ctx, "order-seed-corn-001", 3.0).await?;
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "confirmed").await?,
        StatusCode::OK
    );
    let fulfillment =
        create_marketplace_fulfillment(&ctx, "fulfillment-001", "order-seed-corn-001", "org-alpha")
            .await?;
    assert_eq!(fulfillment.status(), StatusCode::OK);

    let rating = create_marketplace_rating(
        &ctx,
        "rating-order-001-buyer",
        "order-seed-corn-001",
        "buyer-001",
        "supplier-001",
        5.0,
    )
    .await?;
    assert_eq!(rating.status(), StatusCode::OK);

    let aggregate = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/ratings/accounts/supplier-001/aggregate?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(aggregate.status(), StatusCode::OK);
    let body = to_bytes(aggregate.into_body(), 64 * 1024).await?;
    let aggregate: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        aggregate
            .get("rating_count")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        aggregate
            .get("average_score")
            .and_then(|value| value.as_f64()),
        Some(5.0)
    );

    let duplicate = create_marketplace_rating(
        &ctx,
        "rating-order-001-buyer-2",
        "order-seed-corn-001",
        "buyer-001",
        "supplier-001",
        4.0,
    )
    .await?;
    assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn marketplace_ratings_reject_non_participant_without_write() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_order_dependencies(&ctx).await?;
    seed_marketplace_order(&ctx, "order-seed-corn-001", 3.0).await?;
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "confirmed").await?,
        StatusCode::OK
    );
    let fulfillment =
        create_marketplace_fulfillment(&ctx, "fulfillment-001", "order-seed-corn-001", "org-alpha")
            .await?;
    assert_eq!(fulfillment.status(), StatusCode::OK);

    let rejected = create_marketplace_rating(
        &ctx,
        "rating-order-001-viewer",
        "order-seed-corn-001",
        "viewer-001",
        "supplier-001",
        4.0,
    )
    .await?;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    let rating_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marketplace_ratings")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(rating_count, 0);

    Ok(())
}

#[tokio::test]
async fn marketplace_demand_forecast_uses_field_and_product_evidence() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_demand_field(&ctx, "field-alpha", "org-alpha").await?;
    seed_marketplace_demand_product(&ctx, "yield-map-001", "field-alpha", "yield_map").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/demand-forecasts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "forecast_id": "forecast-seed-001",
                        "org_id": "org-alpha",
                        "field_id": "field-alpha",
                        "item_kind": "input",
                        "horizon": "2026-season"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let forecast: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        forecast.get("status").and_then(|value| value.as_str()),
        Some("ready")
    );
    assert!(forecast
        .get("value")
        .and_then(|value| value.as_f64())
        .is_some_and(|value| value > 0.0));
    assert!(forecast
        .get("evidence_refs")
        .and_then(|value| value.as_array())
        .is_some_and(|refs| refs.iter().any(|value| value == "product:yield-map-001")));

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/demand-forecasts?org_id=org-alpha&field_id=field-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let forecasts: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(forecasts.len(), 1);

    Ok(())
}

#[tokio::test]
async fn marketplace_demand_forecast_ai_includes_uncertainty() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_demand_field(&ctx, "field-alpha", "org-alpha").await?;
    seed_marketplace_demand_product(&ctx, "health-ndvi-001", "field-alpha", "ndvi").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/demand-forecasts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "forecast_id": "forecast-produce-001",
                        "org_id": "org-alpha",
                        "field_id": "field-alpha",
                        "item_kind": "produce",
                        "horizon": "2026-season",
                        "ai_assisted": true
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let forecast: serde_json::Value = serde_json::from_slice(&body)?;
    assert!(forecast.get("uncertainty_band").is_some());
    assert!(forecast
        .get("evidence_refs")
        .and_then(|value| value.as_array())
        .is_some_and(|refs| refs.iter().any(|value| value == "product:health-ndvi-001")));

    Ok(())
}

#[tokio::test]
async fn marketplace_demand_forecast_returns_no_basis_without_evidence() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_demand_field(&ctx, "field-alpha", "org-alpha").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/demand-forecasts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "forecast_id": "forecast-empty-001",
                        "org_id": "org-alpha",
                        "field_id": "field-alpha",
                        "item_kind": "input",
                        "horizon": "2026-season"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let forecast: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        forecast.get("status").and_then(|value| value.as_str()),
        Some("no_basis")
    );
    assert_eq!(forecast.get("value"), Some(&serde_json::Value::Null));

    Ok(())
}

#[tokio::test]
async fn marketplace_org_report_aggregates_period_activity() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_marketplace_order_dependencies(&ctx).await?;
    seed_marketplace_order(&ctx, "order-seed-corn-001", 3.0).await?;
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "confirmed").await?,
        StatusCode::OK
    );
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "fulfilled").await?,
        StatusCode::OK
    );
    assert_eq!(
        marketplace_order_transition(&ctx, "order-seed-corn-001", "closed").await?,
        StatusCode::OK
    );

    let report = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/reports/org?org_id=org-alpha&from=2026-01-01T00:00:00Z&to=2026-12-31T23:59:59Z")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(report.status(), StatusCode::OK);
    let body = to_bytes(report.into_body(), 64 * 1024).await?;
    let report: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        report.get("sales_total").and_then(|value| value.as_f64()),
        Some(375.0)
    );
    assert_eq!(
        report
            .get("source_order_ids")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(1)
    );
    assert_eq!(
        report
            .get("order_counts_by_status")
            .and_then(|value| value.as_array())
            .and_then(|items| items.first())
            .and_then(|item| item.get("status"))
            .and_then(|value| value.as_str()),
        Some("closed")
    );

    Ok(())
}

#[tokio::test]
async fn marketplace_org_report_empty_period_returns_zeroes() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let report = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/marketplace/reports/org?org_id=org-alpha&from=2026-01-01T00:00:00Z&to=2026-12-31T23:59:59Z")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(report.status(), StatusCode::OK);
    let body = to_bytes(report.into_body(), 64 * 1024).await?;
    let report: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        report.get("sales_total").and_then(|value| value.as_f64()),
        Some(0.0)
    );
    assert_eq!(
        report
            .get("source_order_ids")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(0)
    );

    Ok(())
}

#[tokio::test]
async fn sustainability_records_create_get_and_list_field_scoped() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-sustain", "field-sustain", "season-2026").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "record_id": "sustain-001",
                        "field_id": "field-sustain",
                        "season_id": "season-2026",
                        "operation_id": "operation-planting-001",
                        "metric_type": "carbon_footprint",
                        "method_version": "carbon.identity.v1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let record: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        record.get("record_id").and_then(|value| value.as_str()),
        Some("sustain-001")
    );
    assert_eq!(
        record.get("field_id").and_then(|value| value.as_str()),
        Some("field-sustain")
    );
    assert_eq!(
        record.get("season_id").and_then(|value| value.as_str()),
        Some("season-2026")
    );
    assert_eq!(
        record.get("operation_id").and_then(|value| value.as_str()),
        Some("operation-planting-001")
    );
    assert_eq!(
        record.get("metric_type").and_then(|value| value.as_str()),
        Some("carbon_footprint")
    );
    assert_eq!(
        record
            .get("method_version")
            .and_then(|value| value.as_str()),
        Some("carbon.identity.v1")
    );
    assert!(record
        .get("audit_id")
        .and_then(|value| value.as_str())
        .is_some_and(|audit_id| !audit_id.is_empty()));

    let unscoped_list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/records")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(unscoped_list.status(), StatusCode::BAD_REQUEST);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/records?field_id=field-sustain")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let records: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(records.len(), 1);

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/records/sustain-001?field_id=field-sustain")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let cross_field = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/records/sustain-001?field_id=field-other")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_field.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn sustainability_record_create_rejects_unknown_field_or_season_without_writing() -> Result<()>
{
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-sustain", "field-sustain", "season-2026").await?;

    let missing_field = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "record_id": "sustain-missing-field",
                        "field_id": "field-missing",
                        "season_id": "season-2026",
                        "operation_id": "operation-planting-001",
                        "metric_type": "carbon_footprint",
                        "method_version": "carbon.identity.v1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(missing_field.status(), StatusCode::BAD_REQUEST);

    let wrong_season = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "record_id": "sustain-wrong-season",
                        "field_id": "field-sustain",
                        "season_id": "season-2027",
                        "operation_id": "operation-planting-001",
                        "metric_type": "carbon_footprint",
                        "method_version": "carbon.identity.v1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(wrong_season.status(), StatusCode::BAD_REQUEST);

    let record_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sustainability_records")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(record_count, 0);

    Ok(())
}

#[tokio::test]
async fn carbon_footprints_compute_get_and_list_with_stable_hash() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_record(
        &ctx,
        "farm-carbon",
        "field-carbon",
        "season-2026",
        "sustain-carbon-001",
        "operation-carbon-001",
    )
    .await?;

    let first = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/carbon-footprints")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    carbon_footprint_payload("footprint-carbon-001", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first.status(), StatusCode::OK);
    let body = to_bytes(first.into_body(), 64 * 1024).await?;
    let first: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        first.get("status").and_then(|value| value.as_str()),
        Some("computed")
    );
    assert_eq!(
        first.get("value_co2e").and_then(|value| value.as_f64()),
        Some(168.8)
    );
    let first_hash = first
        .get("result_hash")
        .and_then(|value| value.as_str())
        .expect("hash should be present")
        .to_string();

    let second = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/carbon-footprints")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    carbon_footprint_payload("footprint-carbon-002", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(second.status(), StatusCode::OK);
    let body = to_bytes(second.into_body(), 64 * 1024).await?;
    let second: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        second.get("result_hash").and_then(|value| value.as_str()),
        Some(first_hash.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/carbon-footprints/footprint-carbon-001?record_id=sustain-carbon-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/carbon-footprints?record_id=sustain-carbon-001&status=computed")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let footprints: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(footprints.len(), 2);

    Ok(())
}

#[tokio::test]
async fn carbon_footprint_missing_inputs_persists_insufficient_without_value() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_record(
        &ctx,
        "farm-carbon-missing",
        "field-carbon-missing",
        "season-2026",
        "sustain-carbon-missing",
        "operation-carbon-001",
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/carbon-footprints")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    carbon_footprint_payload("footprint-carbon-missing", false).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let footprint: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        footprint.get("status").and_then(|value| value.as_str()),
        Some("insufficient_inputs")
    );
    assert!(footprint
        .get("value_co2e")
        .is_some_and(|value| value.is_null()));

    let stored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM carbon_footprints WHERE status = 'insufficient_inputs' AND value_co2e IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(stored_count, 1);

    Ok(())
}

#[tokio::test]
async fn biomass_estimates_compute_get_and_list_with_georeference() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_metric_record(
        &ctx,
        "farm-biomass",
        "field-biomass",
        "season-2026",
        "sustain-biomass-001",
        "operation-biomass-001",
        "biomass",
    )
    .await?;

    let first = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/biomass-estimates")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    biomass_estimate_payload("biomass-001", false).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first.status(), StatusCode::OK);
    let body = to_bytes(first.into_body(), 64 * 1024).await?;
    let first: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        first.get("biomass_value").and_then(|value| value.as_f64()),
        Some(48.0)
    );
    assert_eq!(
        first.get("area").and_then(|value| value.as_f64()),
        Some(200.0)
    );
    assert_eq!(
        first.get("crs").and_then(|value| value.as_str()),
        Some("EPSG:32614")
    );
    assert_eq!(
        first
            .get("extent")
            .and_then(|value| value.get("max_lon"))
            .and_then(|value| value.as_f64()),
        Some(20.0)
    );
    let first_hash = first
        .get("result_hash")
        .and_then(|value| value.as_str())
        .expect("hash should be present")
        .to_string();

    let second = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/biomass-estimates")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    biomass_estimate_payload("biomass-002", false).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(second.status(), StatusCode::OK);
    let body = to_bytes(second.into_body(), 64 * 1024).await?;
    let second: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        second.get("result_hash").and_then(|value| value.as_str()),
        Some(first_hash.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/biomass-estimates/biomass-001?record_id=sustain-biomass-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/biomass-estimates?record_id=sustain-biomass-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let estimates: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(estimates.len(), 2);

    Ok(())
}

#[tokio::test]
async fn biomass_estimate_rejects_mismatched_georeference_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_metric_record(
        &ctx,
        "farm-biomass-mismatch",
        "field-biomass-mismatch",
        "season-2026",
        "sustain-biomass-mismatch",
        "operation-biomass-001",
        "biomass",
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/biomass-estimates")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    biomass_estimate_payload("biomass-mismatch", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let stored_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM biomass_estimates")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(stored_count, 0);

    Ok(())
}

#[tokio::test]
async fn sustainability_baselines_compare_get_and_list_with_stable_hash() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_sustainability_record_row(
        &ctx,
        "sustain-baseline-2025",
        "field-baseline",
        "season-2025",
        "operation-baseline",
        "biomass",
    )
    .await?;
    insert_sustainability_record_row(
        &ctx,
        "sustain-current-2026",
        "field-baseline",
        "season-2026",
        "operation-current",
        "biomass",
    )
    .await?;

    let baseline = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/baselines")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_baseline_payload("baseline-001").to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(baseline.status(), StatusCode::OK);

    let first = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/comparisons")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_comparison_payload(
                        "comparison-001",
                        "field-baseline",
                        "sustain-current-2026",
                        130.0,
                    )
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first.status(), StatusCode::OK);
    let body = to_bytes(first.into_body(), 64 * 1024).await?;
    let first: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        first.get("status").and_then(|value| value.as_str()),
        Some("compared")
    );
    assert_eq!(
        first.get("delta").and_then(|value| value.as_f64()),
        Some(30.0)
    );
    assert_eq!(
        first.get("trend").and_then(|value| value.as_str()),
        Some("increased")
    );
    let first_hash = first
        .get("result_hash")
        .and_then(|value| value.as_str())
        .expect("hash should be present")
        .to_string();

    let second = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/comparisons")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_comparison_payload(
                        "comparison-002",
                        "field-baseline",
                        "sustain-current-2026",
                        130.0,
                    )
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(second.status(), StatusCode::OK);
    let body = to_bytes(second.into_body(), 64 * 1024).await?;
    let second: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        second.get("result_hash").and_then(|value| value.as_str()),
        Some(first_hash.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/comparisons/comparison-001?field_id=field-baseline")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/comparisons?field_id=field-baseline&status=compared")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let comparisons: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(comparisons.len(), 2);

    Ok(())
}

#[tokio::test]
async fn sustainability_comparison_without_baseline_persists_no_baseline_without_delta(
) -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_sustainability_record_row(
        &ctx,
        "sustain-current-nobaseline",
        "field-nobaseline",
        "season-2026",
        "operation-current",
        "biomass",
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/comparisons")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_comparison_payload(
                        "comparison-nobaseline",
                        "field-nobaseline",
                        "sustain-current-nobaseline",
                        130.0,
                    )
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let comparison: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        comparison.get("status").and_then(|value| value.as_str()),
        Some("no_baseline")
    );
    assert!(comparison.get("delta").is_some_and(|value| value.is_null()));
    assert!(comparison
        .get("baseline_value")
        .is_some_and(|value| value.is_null()));

    let stored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sustainability_comparisons WHERE status = 'no_baseline' AND delta IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(stored_count, 1);

    Ok(())
}

#[tokio::test]
async fn sustainability_mrv_trails_create_get_and_list_certification_ready() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_biomass_estimate_row(&ctx, "biomass-mrv-001", "sustain-biomass-mrv").await?;

    let first = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/mrv-trails")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_mrv_payload("mrv-001", "biomass-mrv-001", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first.status(), StatusCode::OK);
    let body = to_bytes(first.into_body(), 64 * 1024).await?;
    let first: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        first.get("output_ref").and_then(|value| value.as_str()),
        Some("biomass-mrv-001")
    );
    assert_eq!(
        first
            .get("certification_ready")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    let rederived_hash = first
        .get("rederived_result_hash")
        .and_then(|value| value.as_str())
        .expect("rederived hash should be present")
        .to_string();

    let second = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/mrv-trails")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_mrv_payload("mrv-002", "biomass-mrv-001", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(second.status(), StatusCode::OK);
    let body = to_bytes(second.into_body(), 64 * 1024).await?;
    let second: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        second
            .get("rederived_result_hash")
            .and_then(|value| value.as_str()),
        Some(rederived_hash.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/mrv-trails/mrv-001?output_ref=biomass-mrv-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/mrv-trails?output_ref=biomass-mrv-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let trails: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(trails.len(), 2);

    Ok(())
}

#[tokio::test]
async fn sustainability_mrv_trail_rejects_incomplete_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_biomass_estimate_row(&ctx, "biomass-mrv-incomplete", "sustain-biomass-mrv").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/mrv-trails")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_mrv_payload("mrv-incomplete", "biomass-mrv-incomplete", false)
                        .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let stored_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sustainability_mrv_trails")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(stored_count, 0);

    Ok(())
}

#[tokio::test]
async fn sustainability_certification_packs_create_and_get_verifiable_bundle() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_biomass_estimate_row(&ctx, "biomass-cert-001", "sustain-biomass-cert").await?;

    let mrv = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/mrv-trails")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_mrv_payload("mrv-cert-001", "biomass-cert-001", true)
                        .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(mrv.status(), StatusCode::OK);

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/certification-packs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_certification_pack_payload(
                        "cert-pack-001",
                        "claim-regenerative-001",
                        vec!["biomass-cert-001"],
                    )
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let pack: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        pack.get("claim_id").and_then(|value| value.as_str()),
        Some("claim-regenerative-001")
    );
    assert_eq!(
        pack.get("claimed_output_refs")
            .and_then(|value| value.as_array())
            .and_then(|values| values.first())
            .and_then(|value| value.as_str()),
        Some("biomass-cert-001")
    );
    assert_eq!(
        pack.get("outputs")
            .and_then(|value| value.as_array())
            .and_then(|values| values.first())
            .and_then(|value| value.get("result_hash"))
            .and_then(|value| value.as_str()),
        Some("result-hash-biomass-001")
    );
    assert_eq!(
        pack.get("mrv_trails")
            .and_then(|value| value.as_array())
            .and_then(|values| values.first())
            .and_then(|value| value.get("trail_id"))
            .and_then(|value| value.as_str()),
        Some("mrv-cert-001")
    );
    assert!(pack
        .get("evidence_layer_refs")
        .and_then(|value| value.as_array())
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("layer:ndvi-001"))));
    let pack_hash = pack
        .get("pack_hash")
        .and_then(|value| value.as_str())
        .expect("pack hash should be present")
        .to_string();

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/certification-packs/cert-pack-001?claim_id=claim-regenerative-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);
    let body = to_bytes(get.into_body(), 64 * 1024).await?;
    let stored: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        stored.get("pack_hash").and_then(|value| value.as_str()),
        Some(pack_hash.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn sustainability_certification_pack_rejects_missing_mrv_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_biomass_estimate_row(&ctx, "biomass-cert-missing-mrv", "sustain-biomass-cert").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/certification-packs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_certification_pack_payload(
                        "cert-pack-missing-mrv",
                        "claim-missing-mrv",
                        vec!["biomass-cert-missing-mrv"],
                    )
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let stored_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sustainability_certification_packs")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(stored_count, 0);

    Ok(())
}

#[tokio::test]
async fn sustainability_field_exports_csv_geojson_and_pdf_summary() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-export", "field-export", "season-2026").await?;
    insert_sustainability_record_row(
        &ctx,
        "sustain-carbon-001",
        "field-export",
        "season-2026",
        "operation-carbon-001",
        "carbon_footprint",
    )
    .await?;
    insert_sustainability_record_row(
        &ctx,
        "sustain-biomass-001",
        "field-export",
        "season-2026",
        "operation-biomass-001",
        "biomass",
    )
    .await?;

    let carbon = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/carbon-footprints")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    carbon_footprint_payload("footprint-export-001", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(carbon.status(), StatusCode::OK);

    let biomass = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/biomass-estimates")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    biomass_estimate_payload("biomass-export-001", false).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(biomass.status(), StatusCode::OK);
    insert_sustainability_kpi_row(&ctx, "kpi-export-001", "field-export", "season-2026").await?;

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/exports/field/field-export/summary.csv?season_id=season-2026")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    assert_eq!(
        csv_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/csv; charset=utf-8")
    );
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let mut csv_reader = csv::Reader::from_reader(body.as_ref());
    assert_eq!(
        csv_reader.headers()?.iter().collect::<Vec<_>>(),
        vec![
            "record_type",
            "record_id",
            "field_id",
            "season_id",
            "metric_ref",
            "value",
            "unit",
            "status",
            "crs",
            "extent_json",
            "method_version",
            "evidence_refs",
            "result_hash",
            "computed_at"
        ]
    );
    let rows = csv_reader.records().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().any(|row| {
        row.get(0) == Some("carbon_footprint")
            && row.get(10) == Some("agbot-carbon-factors-v1")
            && row
                .get(11)
                .is_some_and(|value| value.contains("input:fuel-log-001"))
    }));
    assert!(rows.iter().any(|row| {
        row.get(0) == Some("biomass_estimate")
            && row.get(8) == Some("EPSG:32614")
            && row
                .get(9)
                .is_some_and(|value| value.contains("\"max_lon\":20.0"))
    }));
    assert!(rows.iter().any(|row| {
        row.get(0) == Some("sustainability_kpi")
            && row.get(10) == Some("sustainability.kpi.v1")
            && row
                .get(11)
                .is_some_and(|value| value.contains("target:cover-2026"))
    }));

    let geojson_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/exports/field/field-export/summary.geojson?season_id=season-2026")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson
            .pointer("/crs/properties/name")
            .and_then(|value| value.as_str()),
        Some("EPSG:32614")
    );
    assert_eq!(
        geojson.get("record_count").and_then(|value| value.as_u64()),
        Some(3)
    );
    let features = geojson
        .get("features")
        .and_then(|value| value.as_array())
        .expect("features should exist");
    assert_eq!(features.len(), 3);
    assert!(features.iter().any(|feature| {
        feature
            .pointer("/properties/record_type")
            .and_then(|value| value.as_str())
            == Some("biomass_estimate")
            && feature
                .pointer("/geometry/type")
                .and_then(|value| value.as_str())
                == Some("Polygon")
    }));

    let pdf_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/exports/field/field-export/summary.pdf?season_id=season-2026")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(pdf_response.status(), StatusCode::OK);
    assert_eq!(
        pdf_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/pdf")
    );
    let body = to_bytes(pdf_response.into_body(), 64 * 1024).await?;
    let pdf = String::from_utf8_lossy(&body);
    assert!(pdf.starts_with("%PDF-1.4"));
    assert!(pdf.contains("biomass.canopy_ndvi.v1"));
    assert!(pdf.contains("layer:ndvi-001"));
    assert!(pdf.contains("sustainability.kpi.v1"));

    Ok(())
}

#[tokio::test]
async fn empty_sustainability_field_exports_valid_empty_artifacts() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(
        &ctx,
        "farm-export-empty",
        "field-export-empty",
        "season-2026",
    )
    .await?;

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/exports/field/field-export-empty/summary.csv")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let mut csv_reader = csv::Reader::from_reader(body.as_ref());
    assert_eq!(csv_reader.records().count(), 0);

    let geojson_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/exports/field/field-export-empty/summary.geojson")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson.get("type").and_then(|value| value.as_str()),
        Some("FeatureCollection")
    );
    assert_eq!(
        geojson
            .get("features")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        geojson.get("empty").and_then(|value| value.as_bool()),
        Some(true)
    );

    let pdf_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/exports/field/field-export-empty/summary.pdf")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(pdf_response.status(), StatusCode::OK);
    let body = to_bytes(pdf_response.into_body(), 64 * 1024).await?;
    let pdf = String::from_utf8_lossy(&body);
    assert!(pdf.contains("empty: true"));
    assert!(pdf.contains("No sustainability records were available"));

    Ok(())
}

#[tokio::test]
async fn biodiversity_proxies_compute_get_and_list_georeferenced_metrics() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let first = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/biodiversity-proxies")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    biodiversity_proxy_payload("biodiversity-001", vec![0.1, 0.4, 0.6, 0.9])
                        .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first.status(), StatusCode::OK);
    let body = to_bytes(first.into_body(), 64 * 1024).await?;
    let first: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        first.get("status").and_then(|value| value.as_str()),
        Some("computed")
    );
    assert_eq!(
        first.get("cover_fraction").and_then(|value| value.as_f64()),
        Some(0.75)
    );
    assert_eq!(
        first.get("crs").and_then(|value| value.as_str()),
        Some("EPSG:32614")
    );
    let first_hash = first
        .get("result_hash")
        .and_then(|value| value.as_str())
        .expect("hash should be present")
        .to_string();

    let second = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/biodiversity-proxies")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    biodiversity_proxy_payload("biodiversity-002", vec![0.1, 0.4, 0.6, 0.9])
                        .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(second.status(), StatusCode::OK);
    let body = to_bytes(second.into_body(), 64 * 1024).await?;
    let second: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        second.get("result_hash").and_then(|value| value.as_str()),
        Some(first_hash.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/biodiversity-proxies/biodiversity-001?field_id=field-biodiversity")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/biodiversity-proxies?field_id=field-biodiversity&status=computed")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let proxies: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(proxies.len(), 2);

    Ok(())
}

#[tokio::test]
async fn biodiversity_proxy_uniform_grid_persists_no_signal_without_score() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/biodiversity-proxies")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    biodiversity_proxy_payload("biodiversity-nosignal", vec![0.4, 0.4, 0.4, 0.4])
                        .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let proxy: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        proxy.get("status").and_then(|value| value.as_str()),
        Some("no_signal")
    );
    assert!(proxy
        .get("heterogeneity_score")
        .is_some_and(|value| value.is_null()));
    assert!(proxy
        .get("cover_fraction")
        .is_some_and(|value| value.is_null()));

    let stored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM biodiversity_proxies WHERE status = 'no_signal' AND heterogeneity_score IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(stored_count, 1);

    Ok(())
}

#[tokio::test]
async fn soil_carbon_proxies_compute_get_and_list_with_uncertainty_band() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let first = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/soil-carbon-proxies")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    soil_carbon_proxy_payload("soil-carbon-001", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first.status(), StatusCode::OK);
    let body = to_bytes(first.into_body(), 64 * 1024).await?;
    let first: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        first.get("status").and_then(|value| value.as_str()),
        Some("computed")
    );
    let proxy_value = first
        .get("proxy_value")
        .and_then(|value| value.as_f64())
        .expect("computed proxy value should be present");
    let band = first
        .get("uncertainty_band")
        .expect("band should always be present for computed proxy");
    assert!(band.get("low").and_then(|value| value.as_f64()) < Some(proxy_value));
    assert!(band.get("high").and_then(|value| value.as_f64()) > Some(proxy_value));
    let first_hash = first
        .get("result_hash")
        .and_then(|value| value.as_str())
        .expect("hash should be present")
        .to_string();

    let second = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/soil-carbon-proxies")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    soil_carbon_proxy_payload("soil-carbon-002", true).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(second.status(), StatusCode::OK);
    let body = to_bytes(second.into_body(), 64 * 1024).await?;
    let second: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        second.get("result_hash").and_then(|value| value.as_str()),
        Some(first_hash.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/soil-carbon-proxies/soil-carbon-001?field_id=field-soil-carbon")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/soil-carbon-proxies?field_id=field-soil-carbon&status=computed")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let proxies: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(proxies.len(), 2);

    Ok(())
}

#[tokio::test]
async fn soil_carbon_proxy_insufficient_evidence_persists_unavailable_without_band() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/soil-carbon-proxies")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    soil_carbon_proxy_payload("soil-carbon-insufficient", false).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let proxy: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        proxy.get("status").and_then(|value| value.as_str()),
        Some("insufficient_evidence")
    );
    assert!(proxy
        .get("proxy_value")
        .is_some_and(|value| value.is_null()));
    assert!(proxy
        .get("uncertainty_band")
        .is_some_and(|value| value.is_null()));

    let stored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM soil_carbon_proxies WHERE status = 'insufficient_evidence' AND proxy_value IS NULL AND uncertainty_low IS NULL AND uncertainty_high IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(stored_count, 1);

    Ok(())
}

#[tokio::test]
async fn sustainability_kpis_compute_get_and_list_with_stable_hash() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let first = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/kpis")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_kpi_payload("kpi-cover-001", Some(0.72)).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first.status(), StatusCode::OK);
    let body = to_bytes(first.into_body(), 64 * 1024).await?;
    let first: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        first.get("status").and_then(|value| value.as_str()),
        Some("on_track")
    );
    assert_eq!(
        first.get("metric_ref").and_then(|value| value.as_str()),
        Some("biodiversity:biodiversity-001")
    );
    let first_hash = first
        .get("result_hash")
        .and_then(|value| value.as_str())
        .expect("hash should be present")
        .to_string();

    let second = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/kpis")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_kpi_payload("kpi-cover-002", Some(0.72)).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(second.status(), StatusCode::OK);
    let body = to_bytes(second.into_body(), 64 * 1024).await?;
    let second: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        second.get("result_hash").and_then(|value| value.as_str()),
        Some(first_hash.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/kpis/kpi-cover-001?field_id=field-sustainability-kpi")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sustainability/kpis?field_id=field-sustainability-kpi&season_id=season-2026&status=on_track")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let kpis: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(kpis.len(), 2);

    Ok(())
}

#[tokio::test]
async fn sustainability_kpi_no_data_persists_without_current_value() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/kpis")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    sustainability_kpi_payload("kpi-cover-nodata", None).to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let kpi: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        kpi.get("status").and_then(|value| value.as_str()),
        Some("no_data")
    );
    assert!(kpi
        .get("current_value")
        .is_some_and(|value| value.is_null()));

    let stored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sustainability_kpis WHERE status = 'no_data' AND current_value IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(stored_count, 1);

    Ok(())
}

#[tokio::test]
async fn content_items_create_edit_get_list_and_deny_cross_org_read() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "content_id": "article-001",
                        "content_type": "article",
                        "author_id": "author-001",
                        "org_id": "org-alpha",
                        "body": "First draft"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);
    let body = to_bytes(create.into_body(), 64 * 1024).await?;
    let created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created
            .pointer("/content/content_id")
            .and_then(|value| value.as_str()),
        Some("article-001")
    );
    assert_eq!(
        created
            .pointer("/content/current_version")
            .and_then(|value| value.as_str()),
        created
            .pointer("/versions/0/version_id")
            .and_then(|value| value.as_str())
    );
    assert_eq!(
        created
            .pointer("/versions")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(1)
    );

    let edit = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-001/versions?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "body": "Second draft with agronomy notes"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(edit.status(), StatusCode::OK);
    let body = to_bytes(edit.into_body(), 64 * 1024).await?;
    let edited: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        edited
            .pointer("/versions")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        edited
            .pointer("/content/current_version")
            .and_then(|value| value.as_str()),
        edited
            .pointer("/versions/1/version_id")
            .and_then(|value| value.as_str())
    );

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/items/article-001?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);
    let body = to_bytes(get.into_body(), 64 * 1024).await?;
    let fetched: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        fetched
            .pointer("/versions")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/items?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(items.len(), 1);

    let cross_org = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/items/article-001?org_id=org-beta")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_org.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn content_item_create_rejects_empty_body_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "content_id": "article-empty",
                        "content_type": "article",
                        "author_id": "author-001",
                        "org_id": "org-alpha",
                        "body": "   "
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::BAD_REQUEST);

    let content_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cms_contents")
        .fetch_one(&ctx.pool)
        .await?;
    let version_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cms_content_versions")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(content_count, 0);
    assert_eq!(version_count, 0);

    Ok(())
}

#[tokio::test]
async fn content_workflow_submit_and_publish_audits_transitions() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture(&ctx, "article-workflow-001").await?;

    let review = post_content_workflow(
        &ctx,
        "article-workflow-001",
        json!({
            "action": "submit_for_review",
            "actor_id": "author-001",
            "actor_role": "author"
        }),
    )
    .await?;
    assert_eq!(
        review
            .pointer("/content/status")
            .and_then(|value| value.as_str()),
        Some("in_review")
    );
    assert_eq!(
        review
            .pointer("/audit/from_status")
            .and_then(|value| value.as_str()),
        Some("draft")
    );
    assert_eq!(
        review
            .pointer("/audit/to_status")
            .and_then(|value| value.as_str()),
        Some("in_review")
    );
    assert_eq!(
        review
            .pointer("/audit/actor_id")
            .and_then(|value| value.as_str()),
        Some("author-001")
    );

    let publish = post_content_workflow(
        &ctx,
        "article-workflow-001",
        json!({
            "action": "publish",
            "actor_id": "editor-001",
            "actor_role": "editor"
        }),
    )
    .await?;
    assert_eq!(
        publish
            .pointer("/content/status")
            .and_then(|value| value.as_str()),
        Some("published")
    );
    assert_eq!(
        publish
            .pointer("/audit/from_status")
            .and_then(|value| value.as_str()),
        Some("in_review")
    );
    assert_eq!(
        publish
            .pointer("/audit/action")
            .and_then(|value| value.as_str()),
        Some("publish")
    );

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cms_content_workflow_audits WHERE content_id = ?1",
    )
    .bind("article-workflow-001")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 2);

    Ok(())
}

#[tokio::test]
async fn content_workflow_denies_author_publish_and_skip_review() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture(&ctx, "article-workflow-denied").await?;

    let skip_review = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-workflow-denied/workflow?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "publish",
                        "actor_id": "editor-001",
                        "actor_role": "editor"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(skip_review.status(), StatusCode::BAD_REQUEST);

    post_content_workflow(
        &ctx,
        "article-workflow-denied",
        json!({
            "action": "submit_for_review",
            "actor_id": "author-001",
            "actor_role": "author"
        }),
    )
    .await?;

    let author_publish = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-workflow-denied/workflow?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "publish",
                        "actor_id": "author-001",
                        "actor_role": "author"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(author_publish.status(), StatusCode::FORBIDDEN);

    let status: String =
        sqlx::query_scalar("SELECT status FROM cms_contents WHERE content_id = ?1")
            .bind("article-workflow-denied")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(status, "in_review");
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cms_content_workflow_audits WHERE content_id = ?1",
    )
    .bind("article-workflow-denied")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    Ok(())
}

#[tokio::test]
async fn content_workflow_scheduled_publish_stays_in_review_until_effective() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture(&ctx, "article-workflow-scheduled").await?;
    post_content_workflow(
        &ctx,
        "article-workflow-scheduled",
        json!({
            "action": "submit_for_review",
            "actor_id": "author-001",
            "actor_role": "author"
        }),
    )
    .await?;

    let scheduled = post_content_workflow(
        &ctx,
        "article-workflow-scheduled",
        json!({
            "action": "publish",
            "actor_id": "editor-001",
            "actor_role": "editor",
            "scheduled_effective_at": "2999-01-01T00:00:00Z"
        }),
    )
    .await?;
    assert_eq!(
        scheduled
            .pointer("/content/status")
            .and_then(|value| value.as_str()),
        Some("in_review")
    );
    assert_eq!(
        scheduled
            .pointer("/audit/scheduled_effective_at")
            .and_then(|value| value.as_str()),
        Some("2999-01-01T00:00:00Z")
    );

    let row = sqlx::query(
        "SELECT status, scheduled_effective_at FROM cms_contents c JOIN cms_content_workflow_audits a ON a.content_id = c.content_id WHERE c.content_id = ?1 AND a.action = 'publish'",
    )
    .bind("article-workflow-scheduled")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "in_review");
    assert_eq!(
        row.get::<Option<String>, _>("scheduled_effective_at")
            .as_deref(),
        Some("2999-01-01T00:00:00Z")
    );

    Ok(())
}

#[tokio::test]
async fn content_permissions_resolve_editor_and_cross_org_scope() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let editor = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/permissions/resolve?org_id=org-alpha&actor_org_id=org-alpha&role_refs=cms:editor")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(editor.status(), StatusCode::OK);
    let body = to_bytes(editor.into_body(), 64 * 1024).await?;
    let editor: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        editor.get("can_publish").and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        editor.get("can_moderate").and_then(|value| value.as_bool()),
        Some(true)
    );

    let cross_org = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/permissions/resolve?org_id=org-alpha&actor_org_id=org-beta&role_refs=cms:editor")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_org.status(), StatusCode::OK);
    let body = to_bytes(cross_org.into_body(), 64 * 1024).await?;
    let cross_org: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        cross_org
            .get("can_publish")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        cross_org.get("can_read").and_then(|value| value.as_bool()),
        Some(false)
    );

    Ok(())
}

#[tokio::test]
async fn content_permissions_viewer_write_is_denied_and_audited() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture(&ctx, "article-permission-denied").await?;

    let denied = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-permission-denied/workflow?org_id=org-alpha&actor_org_id=org-alpha&role_refs=cms:viewer")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "submit_for_review",
                        "actor_id": "viewer-001",
                        "actor_role": "viewer"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let row = sqlx::query(
        "SELECT status, from_status, to_status, actor_id FROM cms_contents c JOIN cms_content_workflow_audits a ON a.content_id = c.content_id WHERE c.content_id = ?1",
    )
    .bind("article-permission-denied")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "draft");
    assert_eq!(row.get::<String, _>("from_status"), "draft");
    assert_eq!(row.get::<String, _>("to_status"), "draft");
    assert_eq!(row.get::<String, _>("actor_id"), "viewer-001");

    Ok(())
}

#[tokio::test]
async fn content_search_returns_ranked_published_org_matches() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-search-a",
        "Cover crop planning guide. Cover crop residue protects soil.",
        "org-alpha",
    )
    .await?;
    create_content_fixture_body(
        &ctx,
        "article-search-b",
        "Crop scouting guide with one cover note.",
        "org-alpha",
    )
    .await?;
    create_content_fixture_body(
        &ctx,
        "article-search-draft",
        "Cover crop draft should not appear.",
        "org-alpha",
    )
    .await?;
    create_content_fixture_body(
        &ctx,
        "article-search-beta",
        "Cover crop from another org should not leak.",
        "org-beta",
    )
    .await?;
    publish_content_fixture(&ctx, "article-search-a", "org-alpha").await?;
    publish_content_fixture(&ctx, "article-search-b", "org-alpha").await?;
    publish_content_fixture(&ctx, "article-search-beta", "org-beta").await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/search?org_id=org-alpha&q=cover%20crop")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let results: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0]
            .get("content_id")
            .and_then(|value| value.as_str()),
        Some("article-search-a")
    );
    assert_eq!(
        results[1]
            .get("content_id")
            .and_then(|value| value.as_str()),
        Some("article-search-b")
    );
    assert!(results[0]
        .get("score")
        .and_then(|value| value.as_u64())
        .zip(results[1].get("score").and_then(|value| value.as_u64()))
        .is_some_and(|(first, second)| first > second));
    assert!(results.iter().all(|result| {
        result.get("org_id").and_then(|value| value.as_str()) == Some("org-alpha")
    }));

    Ok(())
}

#[tokio::test]
async fn content_search_no_match_returns_empty_results() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-search-empty",
        "Cover crop planning guide.",
        "org-alpha",
    )
    .await?;
    publish_content_fixture(&ctx, "article-search-empty", "org-alpha").await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/search?org_id=org-alpha&q=irrigation")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let results: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert!(results.is_empty());

    Ok(())
}

#[tokio::test]
async fn content_tags_persist_and_filter_by_taxonomy() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-tagged-001",
        "Cover crop planning guide.",
        "org-alpha",
    )
    .await?;

    let apply = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-tagged-001/tags?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "tags": [
                            { "kind": "crop", "value": "corn" },
                            { "kind": "topic", "value": "cover crops" }
                        ],
                        "suggested_by_ai": true,
                        "editor_confirmed": true
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(apply.status(), StatusCode::OK);
    let body = to_bytes(apply.into_body(), 64 * 1024).await?;
    let tags: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|tag| {
        tag.get("kind").and_then(|value| value.as_str()) == Some("topic")
            && tag.get("value").and_then(|value| value.as_str()) == Some("cover_crops")
            && tag.get("source").and_then(|value| value.as_str())
                == Some("ai_suggested_editor_confirmed")
    }));

    let list = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/tags?org_id=org-alpha&kind=topic&value=cover_crops")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].get("content_id").and_then(|value| value.as_str()),
        Some("article-tagged-001")
    );

    Ok(())
}

#[tokio::test]
async fn content_tags_reject_unconfirmed_ai_and_off_taxonomy_values() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture(&ctx, "article-tagged-reject").await?;

    let unconfirmed = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-tagged-reject/tags?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "tags": [{ "kind": "topic", "value": "soil_health" }],
                        "suggested_by_ai": true,
                        "editor_confirmed": false
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(unconfirmed.status(), StatusCode::BAD_REQUEST);

    let invalid = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-tagged-reject/tags?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "tags": [{ "kind": "crop", "value": "dragonfruit" }],
                        "suggested_by_ai": false,
                        "editor_confirmed": true
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let tag_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cms_content_tags")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(tag_count, 0);

    Ok(())
}

#[tokio::test]
async fn content_portal_embed_lists_and_opens_published_org_items_read_only() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-portal-a",
        "Published portal cover crop guide.",
        "org-alpha",
    )
    .await?;
    create_content_fixture_body(
        &ctx,
        "article-portal-draft",
        "Draft portal guide should not leak.",
        "org-alpha",
    )
    .await?;
    create_content_fixture_body(
        &ctx,
        "article-portal-beta",
        "Other org published guide should not leak.",
        "org-beta",
    )
    .await?;
    publish_content_fixture(&ctx, "article-portal-a", "org-alpha").await?;
    publish_content_fixture(&ctx, "article-portal-beta", "org-beta").await?;

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/knowledge-base?org_id=org-alpha&actor_org_id=org-alpha&role_refs=cms:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let embed: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        embed.get("read_only").and_then(|value| value.as_bool()),
        Some(true)
    );
    let items = embed
        .get("items")
        .and_then(|value| value.as_array())
        .expect("embed should include items");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].get("content_id").and_then(|value| value.as_str()),
        Some("article-portal-a")
    );
    assert_eq!(
        items[0].get("read_only").and_then(|value| value.as_bool()),
        Some(true)
    );
    assert!(items[0]
        .get("evidence_refs")
        .and_then(|value| value.as_array())
        .is_some_and(|refs| refs
            .iter()
            .any(|reference| reference.as_str() == Some("content:article-portal-a"))));

    let opened = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/knowledge-base/article-portal-a?org_id=org-alpha&actor_org_id=org-alpha&role_refs=cms:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(opened.status(), StatusCode::OK);
    let body = to_bytes(opened.into_body(), 64 * 1024).await?;
    let opened: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        opened.get("content_id").and_then(|value| value.as_str()),
        Some("article-portal-a")
    );
    assert_eq!(
        opened.get("current_body").and_then(|value| value.as_str()),
        Some("Published portal cover crop guide.")
    );

    Ok(())
}

#[tokio::test]
async fn content_portal_embed_denies_unreadable_or_unpublished_direct_hits() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-portal-visible",
        "Published portal guide.",
        "org-alpha",
    )
    .await?;
    create_content_fixture_body(
        &ctx,
        "article-portal-hidden-draft",
        "Draft portal guide should not leak.",
        "org-alpha",
    )
    .await?;
    create_content_fixture_body(
        &ctx,
        "article-portal-hidden-foreign",
        "Foreign portal guide should not leak.",
        "org-beta",
    )
    .await?;
    publish_content_fixture(&ctx, "article-portal-visible", "org-alpha").await?;
    publish_content_fixture(&ctx, "article-portal-hidden-foreign", "org-beta").await?;

    let cross_org_reader = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/knowledge-base?org_id=org-alpha&actor_org_id=org-beta&role_refs=cms:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_org_reader.status(), StatusCode::FORBIDDEN);

    let draft = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/knowledge-base/article-portal-hidden-draft?org_id=org-alpha&actor_org_id=org-alpha&role_refs=cms:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(draft.status(), StatusCode::NOT_FOUND);

    let foreign = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/knowledge-base/article-portal-hidden-foreign?org_id=org-alpha&actor_org_id=org-alpha&role_refs=cms:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn content_engagement_aggregates_reader_events_by_period() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-engagement-001",
        "Published portal guide with engagement.",
        "org-alpha",
    )
    .await?;
    publish_content_fixture(&ctx, "article-engagement-001", "org-alpha").await?;

    for (event_type, actor_id) in [
        ("view", "grower-001"),
        ("view", "grower-002"),
        ("read", "grower-001"),
        ("helpful_vote", "grower-001"),
    ] {
        let response = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/content/items/article-engagement-001/engagement-events?org_id=org-alpha")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "event_type": event_type,
                            "actor_id": actor_id,
                            "period": "2026-06",
                            "occurred_at": "2026-06-17T06:30:00Z"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let summary = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/items/article-engagement-001/engagement?org_id=org-alpha&period=2026-06")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(summary.status(), StatusCode::OK);
    let body = to_bytes(summary.into_body(), 64 * 1024).await?;
    let summary: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        summary.get("views").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        summary.get("reads").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        summary
            .get("helpful_votes")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        summary.get("event_count").and_then(|value| value.as_u64()),
        Some(4)
    );
    assert!(summary
        .get("evidence_refs")
        .and_then(|value| value.as_array())
        .is_some_and(|refs| refs.iter().any(|reference| reference
            .as_str()
            .is_some_and(|reference| reference.starts_with("content-engagement-event:")))));

    let row = sqlx::query(
        "SELECT views, reads, helpful_votes, event_count FROM cms_content_engagement_summaries WHERE content_id = ?1 AND org_id = ?2 AND period = ?3",
    )
    .bind("article-engagement-001")
    .bind("org-alpha")
    .bind("2026-06")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<i64, _>("views"), 2);
    assert_eq!(row.get::<i64, _>("reads"), 1);
    assert_eq!(row.get::<i64, _>("helpful_votes"), 1);
    assert_eq!(row.get::<i64, _>("event_count"), 4);

    Ok(())
}

#[tokio::test]
async fn content_engagement_no_activity_persists_zero_summary() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-engagement-empty",
        "Published guide with no activity.",
        "org-alpha",
    )
    .await?;
    publish_content_fixture(&ctx, "article-engagement-empty", "org-alpha").await?;

    let summary = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/items/article-engagement-empty/engagement?org_id=org-alpha&period=2026-06")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(summary.status(), StatusCode::OK);
    let body = to_bytes(summary.into_body(), 64 * 1024).await?;
    let summary: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        summary.get("views").and_then(|value| value.as_u64()),
        Some(0)
    );
    assert_eq!(
        summary.get("reads").and_then(|value| value.as_u64()),
        Some(0)
    );
    assert_eq!(
        summary
            .get("helpful_votes")
            .and_then(|value| value.as_u64()),
        Some(0)
    );
    assert_eq!(
        summary.get("event_count").and_then(|value| value.as_u64()),
        Some(0)
    );

    let event_count: i64 = sqlx::query_scalar(
        "SELECT event_count FROM cms_content_engagement_summaries WHERE content_id = ?1 AND period = ?2",
    )
    .bind("article-engagement-empty")
    .bind("2026-06")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(event_count, 0);

    Ok(())
}

#[tokio::test]
async fn content_success_story_persists_and_reuses_search_and_embed() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/success-stories")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "content_id": "success-story-001",
                        "author_id": "author-001",
                        "org_id": "org-alpha",
                        "body": "Cover crops increased soil organic matter for corn.",
                        "fields": {
                            "grower": "North Ridge Farm",
                            "crop": "corn",
                            "region": "midwest",
                            "outcome_summary": "Soil organic matter improved after cover crop adoption.",
                            "metrics": [{
                                "metric": "soil organic matter",
                                "value": "+0.4",
                                "unit": "percentage_points",
                                "evidence_ref": "kpi:soil-organic-matter"
                            }]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);
    let body = to_bytes(create.into_body(), 64 * 1024).await?;
    let created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created
            .pointer("/content/content_type")
            .and_then(|value| value.as_str()),
        Some("success_story")
    );
    assert_eq!(
        created
            .pointer("/success_story/crop")
            .and_then(|value| value.as_str()),
        Some("corn")
    );

    let fetched = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/success-stories/success-story-001?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(fetched.status(), StatusCode::OK);
    let body = to_bytes(fetched.into_body(), 64 * 1024).await?;
    let fetched: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        fetched
            .pointer("/success_story/metrics/0/evidence_ref")
            .and_then(|value| value.as_str()),
        Some("kpi:soil-organic-matter")
    );

    publish_content_fixture(&ctx, "success-story-001", "org-alpha").await?;

    let search = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/search?org_id=org-alpha&q=soil%20organic")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(search.status(), StatusCode::OK);
    let body = to_bytes(search.into_body(), 64 * 1024).await?;
    let results: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0]
            .get("content_id")
            .and_then(|value| value.as_str()),
        Some("success-story-001")
    );
    assert_eq!(
        results[0]
            .get("content_type")
            .and_then(|value| value.as_str()),
        Some("success_story")
    );

    let embed = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/portal/knowledge-base?org_id=org-alpha&actor_org_id=org-alpha&role_refs=cms:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(embed.status(), StatusCode::OK);
    let body = to_bytes(embed.into_body(), 64 * 1024).await?;
    let embed: serde_json::Value = serde_json::from_slice(&body)?;
    let items = embed
        .get("items")
        .and_then(|value| value.as_array())
        .expect("embed should include items");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0]
            .get("content_type")
            .and_then(|value| value.as_str()),
        Some("success_story")
    );

    Ok(())
}

#[tokio::test]
async fn content_success_story_rejects_missing_structured_field_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/success-stories")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "content_id": "success-story-invalid",
                        "author_id": "author-001",
                        "org_id": "org-alpha",
                        "body": "Incomplete success story.",
                        "fields": {
                            "grower": "North Ridge Farm",
                            "crop": "corn",
                            "region": "midwest",
                            "outcome_summary": " ",
                            "metrics": [{
                                "metric": "soil organic matter",
                                "value": "+0.4",
                                "unit": "percentage_points",
                                "evidence_ref": "kpi:soil-organic-matter"
                            }]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::BAD_REQUEST);

    let content_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cms_contents WHERE content_id = ?1")
            .bind("success-story-invalid")
            .fetch_one(&ctx.pool)
            .await?;
    let story_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cms_success_stories WHERE content_id = ?1")
            .bind("success-story-invalid")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(content_count, 0);
    assert_eq!(story_count, 0);

    Ok(())
}

#[tokio::test]
async fn content_community_contribution_submits_hidden_until_moderated() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let submit = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/community-contributions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "contribution_id": "community-001",
                        "org_id": "org-alpha",
                        "contributor_id": "grower-001",
                        "content_type": "post",
                        "body": "Grower note about cover crops."
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(submit.status(), StatusCode::OK);
    let body = to_bytes(submit.into_body(), 64 * 1024).await?;
    let contribution: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        contribution.get("status").and_then(|value| value.as_str()),
        Some("submitted")
    );
    assert!(contribution
        .get("content_id")
        .is_none_or(|value| value.is_null()));

    let search = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/search?org_id=org-alpha&q=cover%20crops")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(search.status(), StatusCode::OK);
    let body = to_bytes(search.into_body(), 64 * 1024).await?;
    let results: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert!(results.is_empty());

    let content_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM cms_contents WHERE author_id = ?1")
            .bind("grower-001")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(content_count, 0);

    Ok(())
}

#[tokio::test]
async fn content_community_contribution_moderator_approval_creates_draft_and_audit() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    submit_community_contribution(&ctx, "community-approve").await?;

    let approve = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/community-contributions/community-approve/moderation?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "approve",
                        "moderator_id": "editor-001",
                        "actor_org_id": "org-alpha",
                        "role_refs": ["cms:editor"],
                        "reason": "Useful field note"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(approve.status(), StatusCode::OK);
    let body = to_bytes(approve.into_body(), 64 * 1024).await?;
    let result: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        result
            .pointer("/contribution/status")
            .and_then(|value| value.as_str()),
        Some("approved")
    );
    let content_id = result
        .pointer("/content/content/content_id")
        .and_then(|value| value.as_str())
        .expect("approval should create content");
    assert_eq!(
        result
            .pointer("/content/content/status")
            .and_then(|value| value.as_str()),
        Some("draft")
    );

    let row = sqlx::query(
        "SELECT c.status, c.author_id, q.status AS contribution_status FROM cms_contents c JOIN cms_community_contributions q ON q.content_id = c.content_id WHERE c.content_id = ?1",
    )
    .bind(content_id)
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "draft");
    assert_eq!(row.get::<String, _>("author_id"), "grower-001");
    assert_eq!(row.get::<String, _>("contribution_status"), "approved");

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cms_community_contribution_audits WHERE contribution_id = ?1 AND action = 'approve'",
    )
    .bind("community-approve")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    let search = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/search?org_id=org-alpha&q=cover%20crops")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(search.status(), StatusCode::OK);
    let body = to_bytes(search.into_body(), 64 * 1024).await?;
    let results: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert!(results.is_empty(), "approved draft must not be public");

    Ok(())
}

#[tokio::test]
async fn content_community_contribution_non_moderator_approval_is_denied() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    submit_community_contribution(&ctx, "community-denied").await?;

    let denied = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/community-contributions/community-denied/moderation?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "approve",
                        "moderator_id": "viewer-001",
                        "actor_org_id": "org-alpha",
                        "role_refs": ["cms:viewer"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let row = sqlx::query(
        "SELECT status, content_id FROM cms_community_contributions WHERE contribution_id = ?1",
    )
    .bind("community-denied")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "submitted");
    assert_eq!(row.get::<Option<String>, _>("content_id"), None);

    Ok(())
}

#[tokio::test]
async fn content_community_contribution_moderator_rejects_with_audit_without_content() -> Result<()>
{
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    submit_community_contribution(&ctx, "community-reject").await?;

    let reject = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/community-contributions/community-reject/moderation?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "reject",
                        "moderator_id": "editor-001",
                        "actor_org_id": "org-alpha",
                        "role_refs": ["cms:editor"],
                        "reason": "Duplicate content"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(reject.status(), StatusCode::OK);
    let body = to_bytes(reject.into_body(), 64 * 1024).await?;
    let result: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        result
            .pointer("/contribution/status")
            .and_then(|value| value.as_str()),
        Some("rejected")
    );
    assert!(result.get("content").is_none());

    let row = sqlx::query(
        "SELECT q.status, q.content_id, a.action, a.to_status, a.reason FROM cms_community_contributions q JOIN cms_community_contribution_audits a ON a.contribution_id = q.contribution_id WHERE q.contribution_id = ?1",
    )
    .bind("community-reject")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<String, _>("status"), "rejected");
    assert_eq!(row.get::<Option<String>, _>("content_id"), None);
    assert_eq!(row.get::<String, _>("action"), "reject");
    assert_eq!(row.get::<String, _>("to_status"), "rejected");
    assert_eq!(
        row.get::<Option<String>, _>("reason").as_deref(),
        Some("Duplicate content")
    );

    Ok(())
}

#[tokio::test]
async fn content_localization_serves_requested_published_locale() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-localized-fr",
        "Canonical cover crop guide.",
        "org-alpha",
    )
    .await?;
    publish_content_fixture(&ctx, "article-localized-fr", "org-alpha").await?;

    let create_locale = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items/article-localized-fr/locales?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "locale": "fr-FR",
                        "body": "Guide en francais sur les cultures de couverture.",
                        "status": "published"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_locale.status(), StatusCode::OK);

    let localized = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/items/article-localized-fr/localized?org_id=org-alpha&locale=fr_FR")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(localized.status(), StatusCode::OK);
    let body = to_bytes(localized.into_body(), 64 * 1024).await?;
    let localized: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        localized.get("locale").and_then(|value| value.as_str()),
        Some("fr-fr")
    );
    assert_eq!(
        localized.get("body").and_then(|value| value.as_str()),
        Some("Guide en francais sur les cultures de couverture.")
    );
    assert_eq!(
        localized
            .get("fallback_used")
            .and_then(|value| value.as_bool()),
        Some(false)
    );

    Ok(())
}

#[tokio::test]
async fn content_localization_missing_locale_falls_back_to_canonical() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    create_content_fixture_body(
        &ctx,
        "article-localized-fallback",
        "Canonical cover crop guide.",
        "org-alpha",
    )
    .await?;
    publish_content_fixture(&ctx, "article-localized-fallback", "org-alpha").await?;

    let localized = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/content/items/article-localized-fallback/localized?org_id=org-alpha&locale=es-MX")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(localized.status(), StatusCode::OK);
    let body = to_bytes(localized.into_body(), 64 * 1024).await?;
    let localized: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        localized.get("locale").and_then(|value| value.as_str()),
        Some("canonical")
    );
    assert_eq!(
        localized.get("body").and_then(|value| value.as_str()),
        Some("Canonical cover crop guide.")
    );
    assert_eq!(
        localized
            .get("fallback_used")
            .and_then(|value| value.as_bool()),
        Some(true)
    );

    Ok(())
}

#[tokio::test]
async fn collaboration_channels_create_post_list_and_deny_cross_org_read() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-collab", "field-collab", "season-2026").await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "channel_id": "channel-001",
                        "org_id": "org-alpha",
                        "field_ref": "field:field-collab",
                        "member_account_ids": ["user-a", "user-b"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);

    let post = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels/channel-001/messages?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "message_id": "message-001",
                        "author_id": "user-a",
                        "body": "Scout north pivot"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(post.status(), StatusCode::OK);

    let get = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/channels/channel-001?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get.status(), StatusCode::OK);
    let body = to_bytes(get.into_body(), 64 * 1024).await?;
    let thread: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        thread
            .pointer("/channel/channel_id")
            .and_then(|value| value.as_str()),
        Some("channel-001")
    );
    assert_eq!(
        thread
            .pointer("/messages")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(1)
    );
    let audit_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collab_message_audits
        WHERE message_id = 'message-001'
          AND org_id = 'org-alpha'
          AND actor_id = 'user-a'
          AND event_type = 'message_posted'
        "#,
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    let list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/channels?org_id=org-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), 64 * 1024).await?;
    let channels: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(channels.len(), 1);

    let cross_org = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/channels/channel-001?org_id=org-beta")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_org.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn collaboration_message_rejects_missing_channel_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let post = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels/channel-missing/messages?org_id=org-alpha")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "message_id": "message-missing",
                        "author_id": "user-a",
                        "body": "hello"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(post.status(), StatusCode::BAD_REQUEST);

    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM collab_messages")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(message_count, 0);

    Ok(())
}

#[tokio::test]
async fn collaboration_permissions_resolve_operator_and_cross_org_scope() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let operator = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/permissions/resolve?org_id=org-alpha&actor_org_id=org-alpha&role_refs=collab:operator")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(operator.status(), StatusCode::OK);
    let body = to_bytes(operator.into_body(), 64 * 1024).await?;
    let operator: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        operator.get("can_stream").and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        operator
            .get("can_dispatch")
            .and_then(|value| value.as_bool()),
        Some(true)
    );

    let cross_org = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/permissions/resolve?org_id=org-alpha&actor_org_id=org-beta&role_refs=collab:operator")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_org.status(), StatusCode::OK);
    let body = to_bytes(cross_org.into_body(), 64 * 1024).await?;
    let cross_org: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        cross_org
            .get("can_dispatch")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        cross_org
            .get("can_stream")
            .and_then(|value| value.as_bool()),
        Some(false)
    );

    Ok(())
}

#[tokio::test]
async fn collaboration_viewer_stream_and_dispatch_are_denied_and_audited() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    for action in ["stream", "dispatch"] {
        let response = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/collaboration/actions/authorize")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "org_id": "org-alpha",
                            "actor_org_id": "org-alpha",
                            "actor_id": "viewer-1",
                            "role_refs": ["collab:viewer"],
                            "action": action,
                            "channel_id": "channel-001"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle request");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    let denied_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collab_permission_audits
        WHERE org_id = 'org-alpha'
          AND actor_id = 'viewer-1'
          AND allowed = 0
          AND reason_code = 'role_not_permitted'
          AND action IN ('stream', 'dispatch')
        "#,
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(denied_count, 2);

    Ok(())
}

#[tokio::test]
async fn collaboration_viewer_post_is_denied_and_does_not_write_message() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-collab", "field-collab", "season-2026").await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "channel_id": "channel-viewer-denied",
                        "org_id": "org-alpha",
                        "field_ref": "field:field-collab",
                        "member_account_ids": ["viewer-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);

    let post = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels/channel-viewer-denied/messages?org_id=org-alpha&actor_org_id=org-alpha&actor_id=viewer-1&role_refs=collab:viewer")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "message_id": "message-viewer-denied",
                        "author_id": "viewer-1",
                        "body": "I should not post with viewer-only rights"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(post.status(), StatusCode::FORBIDDEN);

    let message_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_messages WHERE message_id = 'message-viewer-denied'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(message_count, 0);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collab_permission_audits
        WHERE actor_id = 'viewer-1'
          AND action = 'post'
          AND permission = 'can_post'
          AND allowed = 0
        "#,
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    Ok(())
}

#[tokio::test]
async fn collaboration_presence_expires_and_notifications_fan_out() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-collab", "field-collab", "season-2026").await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "channel_id": "channel-presence",
                        "org_id": "org-alpha",
                        "field_ref": "field:field-collab",
                        "member_account_ids": ["user-a", "user-b"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);

    let presence = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels/channel-presence/presence?org_id=org-alpha&actor_org_id=org-alpha&actor_id=user-a&role_refs=collab:member")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "account_id": "user-a",
                        "state": "online",
                        "last_seen": "2026-06-13T15:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(presence.status(), StatusCode::OK);

    let notifications = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels/channel-presence/notifications?org_id=org-alpha&actor_org_id=org-alpha&actor_id=user-a&role_refs=collab:member")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "event_id": "event-presence-001",
                        "event_type": "field_event",
                        "source_ref": "field:field-collab",
                        "body": "Scout event"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(notifications.status(), StatusCode::OK);
    let body = to_bytes(notifications.into_body(), 64 * 1024).await?;
    let notifications: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(notifications.len(), 2);
    assert!(notifications.iter().all(|notification| {
        notification
            .get("delivery_state")
            .and_then(|value| value.as_str())
            == Some("delivered")
    }));

    let listed = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/channels/channel-presence/presence?org_id=org-alpha&stale_before=2026-06-13T15:01:00Z")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(listed.status(), StatusCode::OK);
    let body = to_bytes(listed.into_body(), 64 * 1024).await?;
    let presence: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(
        presence
            .iter()
            .find(
                |record| record.get("account_id").and_then(|value| value.as_str())
                    == Some("user-a")
            )
            .and_then(|record| record.get("state"))
            .and_then(|value| value.as_str()),
        Some("offline")
    );

    let notification_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_notifications WHERE event_id = 'event-presence-001'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(notification_count, 2);

    Ok(())
}

#[tokio::test]
async fn collaboration_stream_relays_frames_reconnects_and_denies_cross_org_viewer() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let start = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/streams?actor_org_id=org-alpha&actor_id=operator-1&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "stream_id": "stream-001",
                        "org_id": "org-alpha",
                        "mission_ref": "mission:mission-001",
                        "source_ref": "camera:rgb-01",
                        "latency_budget_ms": 500,
                        "source_active": true
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(start.status(), StatusCode::OK);
    let body = to_bytes(start.into_body(), 64 * 1024).await?;
    let stream: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        stream.get("state").and_then(|value| value.as_str()),
        Some("live")
    );

    let frame = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/streams/stream-001/frames?org_id=org-alpha&actor_org_id=org-alpha&actor_id=operator-1&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "frame_id": "frame-001",
                        "captured_at": "2026-06-13T15:00:00Z",
                        "relayed_at": "2026-06-13T15:00:00.250Z",
                        "payload_ref": "camera-frame:001",
                        "dropped": false
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(frame.status(), StatusCode::OK);
    let body = to_bytes(frame.into_body(), 64 * 1024).await?;
    let frame: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        frame
            .pointer("/stream/state")
            .and_then(|value| value.as_str()),
        Some("live")
    );
    assert_eq!(
        frame
            .pointer("/frame/latency_ms")
            .and_then(|value| value.as_u64()),
        Some(250)
    );
    assert_eq!(
        frame
            .pointer("/frame/view_ref")
            .and_then(|value| value.as_str()),
        Some("view:stream-001:1")
    );

    let frames = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/streams/stream-001/frames?org_id=org-alpha&actor_org_id=org-alpha&actor_id=viewer-1&role_refs=collab:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(frames.status(), StatusCode::OK);
    let body = to_bytes(frames.into_body(), 64 * 1024).await?;
    let frames: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(frames.len(), 1);

    let dropped = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/streams/stream-001/frames?org_id=org-alpha&actor_org_id=org-alpha&actor_id=operator-1&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "frame_id": "frame-002",
                        "captured_at": "2026-06-13T15:00:01Z",
                        "relayed_at": "2026-06-13T15:00:01.100Z",
                        "payload_ref": "camera-frame:002",
                        "dropped": true
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(dropped.status(), StatusCode::OK);
    let body = to_bytes(dropped.into_body(), 64 * 1024).await?;
    let dropped: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        dropped
            .pointer("/stream/state")
            .and_then(|value| value.as_str()),
        Some("reconnecting")
    );
    assert!(dropped.get("frame").is_none());

    let cross_org = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/streams/stream-001/frames?org_id=org-alpha&actor_org_id=org-beta&actor_id=viewer-1&role_refs=collab:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_org.status(), StatusCode::FORBIDDEN);

    let persisted_state: String =
        sqlx::query_scalar("SELECT state FROM collab_streams WHERE stream_id = 'stream-001'")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(persisted_state, "reconnecting");
    let persisted_frames: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_stream_frames WHERE stream_id = 'stream-001'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(persisted_frames, 1);

    Ok(())
}

#[tokio::test]
async fn collaboration_stream_rejects_unavailable_source_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let start = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/streams?actor_org_id=org-alpha&actor_id=operator-1&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "stream_id": "stream-missing-source",
                        "org_id": "org-alpha",
                        "mission_ref": "mission:mission-001",
                        "source_ref": "camera:missing",
                        "latency_budget_ms": 500,
                        "source_active": false
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(start.status(), StatusCode::BAD_REQUEST);

    let stream_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_streams WHERE stream_id = 'stream-missing-source'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(stream_count, 0);

    Ok(())
}

#[tokio::test]
async fn collaboration_emergency_alert_fans_out_transitions_and_audits() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-collab", "field-collab", "season-2026").await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "channel_id": "channel-alerts",
                        "org_id": "org-alpha",
                        "field_ref": "field:field-collab",
                        "member_account_ids": ["ops-1", "grower-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);

    let raise = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels/channel-alerts/emergency-alerts?org_id=org-alpha&actor_org_id=org-alpha&actor_id=ops-1&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "alert_id": "alert-001",
                        "source": "safety01",
                        "severity": "critical",
                        "trigger_ref": "01:geofence:breach-001",
                        "body": "Geofence breach",
                        "failed_recipient_account_ids": ["grower-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(raise.status(), StatusCode::OK);
    let body = to_bytes(raise.into_body(), 64 * 1024).await?;
    let raise: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        raise
            .pointer("/alert/state")
            .and_then(|value| value.as_str()),
        Some("raised")
    );
    assert_eq!(
        raise
            .pointer("/deliveries")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );
    assert!(raise
        .pointer("/deliveries")
        .and_then(|value| value.as_array())
        .expect("deliveries should be array")
        .iter()
        .any(|delivery| {
            delivery
                .get("recipient_account_id")
                .and_then(|value| value.as_str())
                == Some("grower-1")
                && delivery
                    .get("delivery_state")
                    .and_then(|value| value.as_str())
                    == Some("retry_pending")
                && delivery.get("retry_count").and_then(|value| value.as_u64()) == Some(1)
        }));

    let acknowledge = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/emergency-alerts/alert-001?org_id=org-alpha&actor_org_id=org-alpha&actor_id=ops-2&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "acknowledge",
                        "actor_id": "ops-2"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(acknowledge.status(), StatusCode::OK);

    let resolve = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/emergency-alerts/alert-001?org_id=org-alpha&actor_org_id=org-alpha&actor_id=ops-2&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "action": "resolve",
                        "actor_id": "ops-2"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(resolve.status(), StatusCode::OK);

    let state: String = sqlx::query_scalar(
        "SELECT state FROM collab_emergency_alerts WHERE alert_id = 'alert-001'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(state, "resolved");
    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM collab_alert_audits WHERE alert_id = 'alert-001'")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(audit_count, 3);
    let retry_pending_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_alert_deliveries WHERE alert_id = 'alert-001' AND delivery_state = 'retry_pending' AND retry_count = 1",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(retry_pending_count, 1);

    Ok(())
}

#[tokio::test]
async fn collaboration_emergency_alert_viewer_is_denied_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-collab", "field-collab", "season-2026").await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "channel_id": "channel-alert-denied",
                        "org_id": "org-alpha",
                        "field_ref": "field:field-collab",
                        "member_account_ids": ["viewer-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);

    let raise = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/channels/channel-alert-denied/emergency-alerts?org_id=org-alpha&actor_org_id=org-alpha&actor_id=viewer-1&role_refs=collab:viewer")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "alert_id": "alert-denied",
                        "source": "fleet12",
                        "severity": "warning",
                        "trigger_ref": "12:fleet:battery-low",
                        "body": "Battery low"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(raise.status(), StatusCode::FORBIDDEN);

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_emergency_alerts WHERE alert_id = 'alert-denied'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(count, 0);

    Ok(())
}

#[tokio::test]
async fn collaboration_session_records_replays_ordered_events_and_denies_cross_org() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let record = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/sessions?actor_org_id=org-alpha&actor_id=ops-1&role_refs=collab:member")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "session_id": "session-001",
                        "org_id": "org-alpha",
                        "events": [
                            {
                                "event_id": "event-alert",
                                "kind": "alert",
                                "occurred_at": "2026-06-13T15:00:03Z",
                                "actor_id": "ops-1",
                                "subject_ref": "alert:alert-001",
                                "note": "alert raised"
                            },
                            {
                                "event_id": "event-gap",
                                "kind": "stream_gap",
                                "occurred_at": "2026-06-13T15:00:02Z",
                                "actor_id": "relay",
                                "subject_ref": "stream:stream-001",
                                "note": "dropped frame"
                            },
                            {
                                "event_id": "event-frame",
                                "kind": "stream_frame",
                                "occurred_at": "2026-06-13T15:00:01Z",
                                "actor_id": "camera:rgb-01",
                                "subject_ref": "stream:stream-001:frame-001",
                                "note": "frame relayed"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(record.status(), StatusCode::OK);

    let replay = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/sessions/session-001/replay?org_id=org-alpha&actor_org_id=org-alpha&actor_id=viewer-1&role_refs=collab:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(replay.status(), StatusCode::OK);
    let body = to_bytes(replay.into_body(), 64 * 1024).await?;
    let replay: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        replay
            .pointer("/session/has_explicit_gap")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        replay
            .pointer("/events/0/event_id")
            .and_then(|value| value.as_str()),
        Some("event-frame")
    );
    assert_eq!(
        replay
            .pointer("/events/1/kind")
            .and_then(|value| value.as_str()),
        Some("stream_gap")
    );
    assert_eq!(
        replay
            .pointer("/events/2/event_id")
            .and_then(|value| value.as_str()),
        Some("event-alert")
    );

    let cross_org = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/collaboration/sessions/session-001/replay?org_id=org-alpha&actor_org_id=org-beta&actor_id=viewer-1&role_refs=collab:viewer")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(cross_org.status(), StatusCode::FORBIDDEN);

    let event_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_session_events WHERE session_id = 'session-001'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(event_count, 3);

    Ok(())
}

#[tokio::test]
async fn collaboration_mission_edit_conflicts_and_dispatch_guardrails() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/mission-plans?actor_org_id=org-alpha&actor_id=expert-1&role_refs=collab:expert")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "plan_id": "plan-001",
                        "org_id": "org-alpha",
                        "mission_ref": "mission:mission-001",
                        "waypoints": [
                            {
                                "waypoint_id": "wp-1",
                                "latitude": 42.35,
                                "longitude": -71.08,
                                "altitude_m": 32.0
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);
    let body = to_bytes(create.into_body(), 64 * 1024).await?;
    let plan: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        plan.get("version").and_then(|value| value.as_u64()),
        Some(1)
    );

    let edit = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/mission-plans/plan-001/edits?org_id=org-alpha&actor_org_id=org-alpha&actor_id=expert-1&role_refs=collab:expert")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "actor_id": "expert-1",
                        "base_version": 1,
                        "waypoint": {
                            "waypoint_id": "wp-1",
                            "latitude": 42.351,
                            "longitude": -71.081,
                            "altitude_m": 36.0
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(edit.status(), StatusCode::OK);
    let body = to_bytes(edit.into_body(), 64 * 1024).await?;
    let edit: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        edit.pointer("/plan/version")
            .and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        edit.pointer("/audit/decision")
            .and_then(|value| value.as_str()),
        Some("accepted")
    );

    let stale = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/mission-plans/plan-001/edits?org_id=org-alpha&actor_org_id=org-alpha&actor_id=expert-2&role_refs=collab:expert")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "actor_id": "expert-2",
                        "base_version": 1,
                        "waypoint": {
                            "waypoint_id": "wp-1",
                            "latitude": 42.5,
                            "longitude": -71.2,
                            "altitude_m": 40.0
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(stale.status(), StatusCode::OK);
    let body = to_bytes(stale.into_body(), 64 * 1024).await?;
    let stale: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        stale
            .pointer("/plan/version")
            .and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        stale
            .pointer("/audit/decision")
            .and_then(|value| value.as_str()),
        Some("rejected")
    );
    assert_eq!(
        stale
            .pointer("/audit/reason_code")
            .and_then(|value| value.as_str()),
        Some("version_conflict")
    );

    let allowed = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/mission-plans/plan-001/dispatch?org_id=org-alpha&actor_org_id=org-alpha&actor_id=operator-1&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "actor_id": "operator-1",
                        "guardrails": {
                            "geofence_clear": true,
                            "altitude_clear": true,
                            "battery_clear": true,
                            "abort_path_available": true
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(allowed.status(), StatusCode::OK);
    let body = to_bytes(allowed.into_body(), 64 * 1024).await?;
    let allowed: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        allowed.get("allowed").and_then(|value| value.as_bool()),
        Some(true)
    );

    let blocked = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/mission-plans/plan-001/dispatch?org_id=org-alpha&actor_org_id=org-alpha&actor_id=operator-1&role_refs=collab:operator")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "actor_id": "operator-1",
                        "guardrails": {
                            "geofence_clear": false,
                            "altitude_clear": true,
                            "battery_clear": true,
                            "abort_path_available": true
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(blocked.status(), StatusCode::OK);
    let body = to_bytes(blocked.into_body(), 64 * 1024).await?;
    let blocked: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        blocked.get("allowed").and_then(|value| value.as_bool()),
        Some(false)
    );
    assert_eq!(
        blocked
            .pointer("/blocking_guardrails/0")
            .and_then(|value| value.as_str()),
        Some("geofence")
    );

    let stored_version: i64 =
        sqlx::query_scalar("SELECT version FROM collab_mission_plans WHERE plan_id = 'plan-001'")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(stored_version, 2);
    let edit_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_mission_edit_audits WHERE plan_id = 'plan-001'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(edit_audits, 2);
    let dispatch_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_mission_dispatch_audits WHERE plan_id = 'plan-001'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(dispatch_audits, 2);

    Ok(())
}

#[tokio::test]
async fn collaboration_mission_viewer_dispatch_is_denied_without_dispatch_audit() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/mission-plans?actor_org_id=org-alpha&actor_id=expert-1&role_refs=collab:expert")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "plan_id": "plan-denied",
                        "org_id": "org-alpha",
                        "mission_ref": "mission:mission-denied",
                        "waypoints": [
                            {
                                "waypoint_id": "wp-1",
                                "latitude": 42.35,
                                "longitude": -71.08,
                                "altitude_m": 32.0
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create.status(), StatusCode::OK);

    let denied = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/collaboration/mission-plans/plan-denied/dispatch?org_id=org-alpha&actor_org_id=org-alpha&actor_id=viewer-1&role_refs=collab:viewer")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "actor_id": "viewer-1",
                        "guardrails": {
                            "geofence_clear": true,
                            "altitude_clear": true,
                            "battery_clear": true,
                            "abort_path_available": true
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let dispatch_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM collab_mission_dispatch_audits WHERE plan_id = 'plan-denied'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(dispatch_audits, 0);
    let permission_audits: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM collab_permission_audits
        WHERE actor_id = 'viewer-1'
          AND action = 'dispatch'
          AND allowed = 0
        "#,
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(permission_audits, 1);

    Ok(())
}

#[tokio::test]
async fn fleet_health_component_registry_links_airframe_and_rejects_double_install() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let airframe_a = enroll_test_fleet_node(&ctx, "hw-drone-health-001").await?;
    let airframe_b = enroll_test_fleet_node(&ctx, "hw-drone-health-002").await?;

    let register_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/components")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "component_id": "battery-pack-001",
                        "component_type": "battery",
                        "serial": "BAT-2026-001",
                        "airframe_id": airframe_a,
                        "installed_at": "2026-06-01T10:00:00Z",
                        "service_history": [{
                            "service_id": "svc-001",
                            "performed_at": "2026-06-01T09:30:00Z",
                            "technician": "tech-1",
                            "action": "incoming_inspection",
                            "notes": "capacity check passed"
                        }]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(register_response.status(), StatusCode::OK);
    let body = to_bytes(register_response.into_body(), 64 * 1024).await?;
    let registered: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        registered
            .get("component_id")
            .and_then(|value| value.as_str()),
        Some("battery-pack-001")
    );
    assert_eq!(
        registered.get("serial").and_then(|value| value.as_str()),
        Some("BAT-2026-001")
    );
    assert_eq!(
        registered
            .get("airframe_id")
            .and_then(|value| value.as_str()),
        Some(airframe_a.as_str())
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/fleet-health/components?airframe_id={airframe_a}"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0]
            .get("component_id")
            .and_then(|value| value.as_str()),
        Some("battery-pack-001")
    );

    let history_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fleet-health/components/battery-pack-001/history")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(history_response.status(), StatusCode::OK);
    let body = to_bytes(history_response.into_body(), 64 * 1024).await?;
    let history: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert!(history.iter().any(|event| {
        event.get("event_type").and_then(|value| value.as_str()) == Some("installed")
    }));
    assert!(history.iter().any(|event| {
        event.get("event_type").and_then(|value| value.as_str()) == Some("service_recorded")
    }));

    let double_install_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/components/battery-pack-001/install")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "airframe_id": airframe_b,
                        "installed_at": "2026-06-02T10:00:00Z",
                        "actor": "tech-2"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(double_install_response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(double_install_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("already installed"));

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fleet_component_events WHERE component_id = ?1 AND event_type = ?2",
    )
    .bind("battery-pack-001")
    .bind("double_install_rejected")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    let active_airframe: String =
        sqlx::query_scalar("SELECT airframe_id FROM fleet_components WHERE component_id = ?1")
            .bind("battery-pack-001")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(active_airframe, airframe_a);

    Ok(())
}

#[tokio::test]
async fn fleet_health_duty_accrual_is_idempotent_per_session() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let airframe_id = enroll_test_fleet_node(&ctx, "hw-drone-duty-001").await?;
    register_test_component(&ctx, "battery-pack-duty-001", &airframe_id).await?;

    let accrual_payload = json!({
        "session_id": "session-duty-001",
        "airframe_id": airframe_id,
        "flight_hours": 1.25,
        "cycles": 1,
        "duty_score": 0.80,
        "ended_at": "2026-06-03T12:15:00Z"
    });

    let first_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/duty-accruals")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(accrual_payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(first_response.status(), StatusCode::OK);
    let body = to_bytes(first_response.into_body(), 64 * 1024).await?;
    let first_accruals: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(first_accruals.len(), 1);
    assert_eq!(
        first_accruals[0]
            .get("component_id")
            .and_then(|value| value.as_str()),
        Some("battery-pack-duty-001")
    );

    let replay_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/duty-accruals")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(accrual_payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(replay_response.status(), StatusCode::OK);
    let body = to_bytes(replay_response.into_body(), 64 * 1024).await?;
    let replay_accruals: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(replay_accruals.len(), 1);

    let component_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/fleet-health/components?airframe_id={airframe_id}"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(component_response.status(), StatusCode::OK);
    let body = to_bytes(component_response.into_body(), 64 * 1024).await?;
    let components: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(components.len(), 1);
    assert_eq!(
        components[0]
            .get("flight_hours")
            .and_then(|value| value.as_f64()),
        Some(1.25)
    );
    assert_eq!(
        components[0].get("cycles").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        components[0]
            .get("duty_score")
            .and_then(|value| value.as_f64()),
        Some(0.80)
    );

    let accrual_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fleet_component_duty_accruals WHERE session_id = ?1 AND component_id = ?2",
    )
    .bind("session-duty-001")
    .bind("battery-pack-duty-001")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(accrual_count, 1);

    Ok(())
}

#[tokio::test]
async fn fleet_health_indicators_persist_timeseries_and_explicit_gaps() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let airframe_id = enroll_test_fleet_node(&ctx, "hw-drone-health-indicators-001").await?;
    register_test_component_type(&ctx, "battery-pack-health-001", "battery", &airframe_id).await?;
    register_test_component_type(&ctx, "motor-front-left", "motor", &airframe_id).await?;
    register_test_component_type(&ctx, "esc-front-left", "esc", &airframe_id).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/health-indicators")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "source_ref": "telemetry:session-health-001",
                        "created_at": "2026-06-12T12:20:00Z",
                        "samples": [
                            {
                                "component_id": "battery-pack-health-001",
                                "component_type": "battery",
                                "ts": "2026-06-12T12:00:00Z",
                                "battery_open_circuit_voltage_v": 16.8,
                                "battery_voltage_v": 15.96,
                                "battery_current_a": 28.0
                            },
                            {
                                "component_id": "motor-front-left",
                                "component_type": "motor",
                                "ts": "2026-06-12T12:00:00Z",
                                "motor_vibration_g": 0.42
                            },
                            {
                                "component_id": "esc-front-left",
                                "component_type": "esc",
                                "ts": "2026-06-12T12:00:00Z",
                                "esc_temperature_c": 54.5
                            }
                        ],
                        "telemetry_gaps": [
                            {
                                "component_id": "battery-pack-health-001",
                                "started_at": "2026-06-12T12:01:00Z",
                                "ended_at": "2026-06-12T12:05:00Z",
                                "reason": "mavlink-radio-dropout"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let derived: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        derived
            .get("samples")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(
        derived
            .pointer("/gaps/0/reason")
            .and_then(|value| value.as_str()),
        Some("mavlink-radio-dropout")
    );
    let battery_sample = derived
        .get("samples")
        .and_then(|value| value.as_array())
        .and_then(|samples| {
            samples.iter().find(|sample| {
                sample.get("indicator").and_then(|value| value.as_str())
                    == Some("battery_internal_resistance")
            })
        })
        .expect("battery indicator should exist");
    assert_eq!(
        battery_sample
            .get("freshness")
            .and_then(|value| value.as_str()),
        Some("stale")
    );
    assert!(
        (battery_sample
            .get("value")
            .and_then(|value| value.as_f64())
            .expect("value should be numeric")
            - 30.0)
            .abs()
            < 1e-9
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fleet-health/health-indicators?component_id=battery-pack-health-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("freshness").and_then(|value| value.as_str()),
        Some("stale")
    );

    let time_series_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM time_series_points WHERE entity_ref LIKE 'component:%'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(time_series_count, 3);

    let gap_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fleet_health_telemetry_gaps WHERE component_id = ?1",
    )
    .bind("battery-pack-health-001")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(gap_count, 1);

    let backfilled_points: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM time_series_points WHERE t = ?1")
            .bind("2026-06-12T12:01:00Z")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(backfilled_points, 0);

    Ok(())
}

#[tokio::test]
async fn fleet_health_ota_rollout_evaluates_stage_and_rollback() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/ota-rollouts/evaluate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "rollout_id": "rollout-2026-06-12",
                        "evaluated_at": "2026-06-12T13:00:00Z",
                        "current_stage": "staged",
                        "target_version": {
                            "artifact": "agbot-edge",
                            "version": "2.0.0",
                            "signed": true
                        },
                        "rollback_version": {
                            "artifact": "agbot-edge",
                            "version": "1.9.0",
                            "signed": true
                        },
                        "nodes": [
                            {
                                "node_id": "node-staged-1",
                                "stage": "staged",
                                "current_version": "2.0.0",
                                "previous_version": "1.9.0"
                            },
                            {
                                "node_id": "node-staged-2",
                                "stage": "staged",
                                "current_version": "2.0.0",
                                "previous_version": "1.9.0"
                            }
                        ],
                        "health_reports": [
                            {
                                "node_id": "node-staged-1",
                                "status": "ok",
                                "blocking_alerts": ["alert:disk-full"],
                                "checked_at": "2026-06-12T13:02:00Z"
                            },
                            {
                                "node_id": "node-staged-2",
                                "status": "ok",
                                "blocking_alerts": [],
                                "checked_at": "2026-06-12T13:02:00Z"
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let decision: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        decision.get("status").and_then(|value| value.as_str()),
        Some("halted_rolled_back")
    );
    assert_eq!(
        decision.get("reason_code").and_then(|value| value.as_str()),
        Some("health_regression")
    );
    assert_eq!(
        decision
            .pointer("/rollback_actions/0/node_id")
            .and_then(|value| value.as_str()),
        Some("node-staged-1")
    );
    assert_eq!(
        decision
            .pointer("/rollback_actions/0/to_version")
            .and_then(|value| value.as_str()),
        Some("1.9.0")
    );

    Ok(())
}

#[tokio::test]
async fn fleet_health_rollout_control_actions_return_audit() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let start = post_rollout_control(&ctx, "start", true, true).await?;
    let pause = post_rollout_control(&ctx, "pause", true, true).await?;
    let abort = post_rollout_control(&ctx, "abort", true, true).await?;

    assert_eq!(
        start.get("status").and_then(|value| value.as_str()),
        Some("started")
    );
    assert_eq!(
        pause.get("status").and_then(|value| value.as_str()),
        Some("paused")
    );
    assert_eq!(
        abort.get("status").and_then(|value| value.as_str()),
        Some("aborted")
    );
    assert_eq!(
        abort
            .pointer("/audit/actor")
            .and_then(|value| value.as_str()),
        Some("ops@example.com")
    );
    assert_eq!(
        abort
            .pointer("/audit/action")
            .and_then(|value| value.as_str()),
        Some("abort")
    );
    assert_eq!(
        abort
            .pointer("/audit/stage")
            .and_then(|value| value.as_str()),
        Some("staged")
    );

    Ok(())
}

#[tokio::test]
async fn fleet_health_rollout_control_refuses_unsimulated_flight_target() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let decision = post_rollout_control(&ctx, "start", false, true).await?;

    assert_eq!(
        decision.get("status").and_then(|value| value.as_str()),
        Some("refused")
    );
    assert_eq!(
        decision.get("reason_code").and_then(|value| value.as_str()),
        Some("simulation_validation_required")
    );
    assert_eq!(
        decision
            .pointer("/audit/result")
            .and_then(|value| value.as_str()),
        Some("refused")
    );

    Ok(())
}

#[tokio::test]
async fn fired_alert_history_is_filterable_paginable_and_not_fabricated() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    post_fired_alert(
        &ctx,
        json!({
            "alert_id": "alert-field-critical-001",
            "matched_rule_id": "rule-sensor-stale-critical",
            "source_event_ref": "alert-candidate-000010",
            "source_domain": "27-soil-iot-sensor-network",
            "event_type": "sensor_stale",
            "subject_ref": "field:field-alert-001",
            "field_id": "field-alert-001",
            "evidence_refs": [
                "reading:soil-probe-001:latest",
                "gap:mavlink:2026-06-12T10:00:00Z"
            ],
            "severity": "critical",
            "channels": ["in_app"],
            "fired_at": "2026-06-12T10:00:00Z",
            "explanation": "rule-sensor-stale-critical matched sensor_stale; evidence refs: reading:soil-probe-001:latest,gap:mavlink:2026-06-12T10:00:00Z"
        }),
    )
    .await?;
    post_fired_alert(
        &ctx,
        json!({
            "alert_id": "alert-field-critical-002",
            "matched_rule_id": "rule-sensor-stale-critical",
            "source_event_ref": "alert-candidate-000011",
            "source_domain": "27-soil-iot-sensor-network",
            "event_type": "sensor_stale",
            "subject_ref": "field:field-alert-001",
            "field_id": "field-alert-001",
            "evidence_refs": ["reading:soil-probe-002:latest"],
            "severity": "critical",
            "channels": ["in_app"],
            "fired_at": "2026-06-12T10:05:00Z",
            "explanation": "rule-sensor-stale-critical matched sensor_stale; evidence refs: reading:soil-probe-002:latest"
        }),
    )
    .await?;
    post_fired_alert(
        &ctx,
        json!({
            "alert_id": "alert-field-warning-001",
            "matched_rule_id": "rule-sensor-stale-warning",
            "source_event_ref": "alert-candidate-000012",
            "source_domain": "25-predictive-maintenance-fleet-health",
            "event_type": "component_stale",
            "subject_ref": "component:battery-pack-001",
            "field_id": null,
            "evidence_refs": ["component:battery-pack-001"],
            "severity": "warning",
            "channels": ["in_app"],
            "fired_at": "2026-06-12T10:10:00Z",
            "explanation": "rule-sensor-stale-warning matched component_stale"
        }),
    )
    .await?;

    let page_one = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/alerting/fired-alerts?source_domain=27-soil-iot-sensor-network&field_id=field-alert-001&severity=critical&start=2026-06-12T09:59:00Z&end=2026-06-12T10:06:00Z&page=1&page_size=1",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(page_one.status(), StatusCode::OK);
    let body = to_bytes(page_one.into_body(), 64 * 1024).await?;
    let page_one_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        page_one_json.get("total").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        page_one_json.get("page").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        page_one_json
            .pointer("/alerts/0/alert_id")
            .and_then(|value| value.as_str()),
        Some("alert-field-critical-002")
    );
    assert_eq!(
        page_one_json
            .pointer("/alerts/0/matched_rule_id")
            .and_then(|value| value.as_str()),
        Some("rule-sensor-stale-critical")
    );
    assert_eq!(
        page_one_json
            .pointer("/alerts/0/evidence_refs/0")
            .and_then(|value| value.as_str()),
        Some("reading:soil-probe-002:latest")
    );
    assert!(page_one_json
        .pointer("/alerts/0/explanation")
        .and_then(|value| value.as_str())
        .is_some_and(|explanation| explanation.contains("rule-sensor-stale-critical")));

    let page_two = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/alerting/fired-alerts?source_domain=27-soil-iot-sensor-network&field_id=field-alert-001&severity=critical&start=2026-06-12T09:59:00Z&end=2026-06-12T10:06:00Z&page=2&page_size=1",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(page_two.status(), StatusCode::OK);
    let body = to_bytes(page_two.into_body(), 64 * 1024).await?;
    let page_two_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        page_two_json
            .pointer("/alerts/0/alert_id")
            .and_then(|value| value.as_str()),
        Some("alert-field-critical-001")
    );

    let unknown_source = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/alerting/fired-alerts?source_domain=15-weather&page=1&page_size=10")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(unknown_source.status(), StatusCode::OK);
    let body = to_bytes(unknown_source.into_body(), 64 * 1024).await?;
    let empty_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        empty_json
            .get("alerts")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(0)
    );

    let missing_alert = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/alerting/fired-alerts/never-fired")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(missing_alert.status(), StatusCode::NOT_FOUND);

    let stored_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM alert_fired_alerts")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(stored_count, 3);

    Ok(())
}

#[tokio::test]
async fn alert_rules_create_version_disable_and_subscribe_with_audit() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/alerting/rules")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "rule_id": "rule-sensor-stale",
                        "event_type": "sensor_stale",
                        "subject_ref": "field:field-alert-001",
                        "severity": "critical",
                        "channels": ["in_app"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);
    let body = to_bytes(create_response.into_body(), 64 * 1024).await?;
    let created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created.get("version").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        created.get("status").and_then(|value| value.as_str()),
        Some("active")
    );

    let subscription_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/alerting/rules/rule-sensor-stale/subscriptions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "subscription_id": "subscription-ops-stale",
                        "recipient_id": "ops-user-001",
                        "recipient_role": "operator",
                        "channels": ["in_app", "email"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(subscription_response.status(), StatusCode::OK);

    let edit_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/alerting/rules/rule-sensor-stale")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "event_type": "sensor_stale",
                        "severity": "warning",
                        "channels": ["email"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(edit_response.status(), StatusCode::OK);
    let body = to_bytes(edit_response.into_body(), 64 * 1024).await?;
    let edited: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        edited.get("version").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        edited.get("severity").and_then(|value| value.as_str()),
        Some("warning")
    );

    let disable_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/alerting/rules/rule-sensor-stale/status")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "disabled",
                        "actor_id": "ops-admin",
                        "occurred_at": "2026-06-12T10:30:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(disable_response.status(), StatusCode::OK);
    let body = to_bytes(disable_response.into_body(), 64 * 1024).await?;
    let disabled: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        disabled.get("version").and_then(|value| value.as_u64()),
        Some(3)
    );
    assert_eq!(
        disabled.get("status").and_then(|value| value.as_str()),
        Some("disabled")
    );

    let versions_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/alerting/rules/rule-sensor-stale")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(versions_response.status(), StatusCode::OK);
    let body = to_bytes(versions_response.into_body(), 64 * 1024).await?;
    let versions: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(versions.len(), 3);

    let active_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/alerting/rules?status=active")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(active_response.status(), StatusCode::OK);
    let body = to_bytes(active_response.into_body(), 64 * 1024).await?;
    let active_rules: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert!(active_rules.is_empty());

    let subscriptions_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/alerting/rules/rule-sensor-stale/subscriptions")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(subscriptions_response.status(), StatusCode::OK);
    let body = to_bytes(subscriptions_response.into_body(), 64 * 1024).await?;
    let subscriptions: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(subscriptions.len(), 1);
    assert_eq!(
        subscriptions[0]
            .get("recipient_role")
            .and_then(|value| value.as_str()),
        Some("operator")
    );

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM alert_rule_audits WHERE rule_id = 'rule-sensor-stale'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    Ok(())
}

#[tokio::test]
async fn alert_rule_create_rejects_invalid_predicate_without_persisting() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/alerting/rules")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "rule_id": "rule-invalid",
                        "event_type": " ",
                        "severity": "warning",
                        "channels": ["in_app"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let stored_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM alert_rules")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(stored_count, 0);

    Ok(())
}

#[tokio::test]
async fn plugin_lifecycle_lists_filters_refuses_disabled_execution_and_audits() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let register_index = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/plugins")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "plugin_id": "plugin.custom_ndvi",
                        "name": "Custom NDVI",
                        "version": "1.2.3",
                        "kind": "index",
                        "host_api_version": "2026.1",
                        "capabilities": ["read:scene", "write:product"],
                        "entrypoint": "custom_ndvi::run"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(register_index.status(), StatusCode::OK);
    let body = to_bytes(register_index.into_body(), 64 * 1024).await?;
    let registered: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        registered.get("status").and_then(|value| value.as_str()),
        Some("registered")
    );

    let register_map_layer = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/plugins")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "plugin_id": "plugin.field_heatmap",
                        "name": "Field Heatmap",
                        "version": "0.3.0",
                        "kind": "map_layer",
                        "host_api_version": "2026.1",
                        "capabilities": ["read:scene"],
                        "entrypoint": "field_heatmap::layer"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(register_map_layer.status(), StatusCode::OK);

    let filtered_list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/plugins?kind=index&status=registered&page=1&page_size=1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(filtered_list.status(), StatusCode::OK);
    let body = to_bytes(filtered_list.into_body(), 64 * 1024).await?;
    let page: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(page.get("total").and_then(|value| value.as_u64()), Some(1));
    assert_eq!(
        page.get("plugins")
            .and_then(|value| value.as_array())
            .and_then(|plugins| plugins.first())
            .and_then(|plugin| plugin.get("plugin_id"))
            .and_then(|value| value.as_str()),
        Some("plugin.custom_ndvi")
    );

    let enable_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/plugins/plugin.custom_ndvi/status")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "enabled",
                        "actor_id": "pa-admin-1",
                        "actor_kind": "platform_admin",
                        "occurred_at": "2026-06-12T13:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(enable_response.status(), StatusCode::OK);
    let body = to_bytes(enable_response.into_body(), 64 * 1024).await?;
    let enabled: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        enabled.get("status").and_then(|value| value.as_str()),
        Some("enabled")
    );

    let execution_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/plugins/plugin.custom_ndvi/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "required_capabilities": ["read:scene"],
                        "estimated_runtime_ms": 25,
                        "estimated_memory_mb": 64,
                        "result": "ndvi complete",
                        "attempted_at": "2026-06-12T13:01:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(execution_response.status(), StatusCode::OK);
    let body = to_bytes(execution_response.into_body(), 64 * 1024).await?;
    let execution: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        execution.get("status").and_then(|value| value.as_str()),
        Some("completed")
    );

    let disable_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/plugins/plugin.custom_ndvi/status")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "disabled",
                        "actor_id": "pa-admin-1",
                        "actor_kind": "platform_admin",
                        "occurred_at": "2026-06-12T13:05:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(disable_response.status(), StatusCode::OK);
    let body = to_bytes(disable_response.into_body(), 64 * 1024).await?;
    let disabled: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        disabled.get("status").and_then(|value| value.as_str()),
        Some("disabled")
    );

    let disabled_execution = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/plugins/plugin.custom_ndvi/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "required_capabilities": ["read:scene"],
                        "estimated_runtime_ms": 25,
                        "estimated_memory_mb": 64,
                        "result": "should not run",
                        "attempted_at": "2026-06-12T13:06:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(disabled_execution.status(), StatusCode::FORBIDDEN);

    let disabled_list = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/plugins?status=disabled&page=1&page_size=10")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(disabled_list.status(), StatusCode::OK);
    let body = to_bytes(disabled_list.into_body(), 64 * 1024).await?;
    let disabled_page: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        disabled_page.get("total").and_then(|value| value.as_u64()),
        Some(1)
    );

    let lifecycle_audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM plugin_lifecycle_audits WHERE plugin_id = 'plugin.custom_ndvi'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(lifecycle_audit_count, 2);

    let disabled_audit_actor: String = sqlx::query_scalar(
        "SELECT actor_id FROM plugin_lifecycle_audits WHERE plugin_id = 'plugin.custom_ndvi' AND new_status = 'disabled'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(disabled_audit_actor, "pa-admin-1");

    let provenance_audit = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/provenance/audit?artifact_id=plugin:plugin.custom_ndvi&actor_id=pa-admin-1&page=1&page_size=10",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(provenance_audit.status(), StatusCode::OK);
    let body = to_bytes(provenance_audit.into_body(), 64 * 1024).await?;
    let provenance_page: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        provenance_page
            .get("total")
            .and_then(|value| value.as_u64()),
        Some(2)
    );
    assert!(provenance_page
        .get("entries")
        .and_then(|value| value.as_array())
        .expect("entries should be present")
        .iter()
        .all(|entry| entry
            .get("action")
            .and_then(|action| action.get("action_kind"))
            .and_then(|value| value.as_str())
            == Some("plugin_lifecycle_transition")));

    Ok(())
}

#[tokio::test]
async fn provenance_ledger_lists_filters_and_retrieves_after_restart() -> Result<()> {
    let tmp = TempDir::new()?;
    let data_root = tmp.path().join("data");
    let db_path = tmp.path().join("geo_hub_test.db");
    let ctx = test_app_with_paths(data_root.clone(), db_path.clone()).await?;
    seed_provenance_ledger_fixture(&ctx).await?;
    drop(ctx);

    let restarted = test_app_with_paths(data_root, db_path).await?;

    let lineage_page = restarted
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/provenance/lineage?actor_id=operator:dsp-7&start=2026-06-12T09:00:00Z&end=2026-06-12T11:00:00Z&page=1&page_size=1",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(lineage_page.status(), StatusCode::OK);
    let body = to_bytes(lineage_page.into_body(), 64 * 1024).await?;
    let lineage_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        lineage_json.get("total").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        lineage_json
            .get("page_size")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        lineage_json
            .pointer("/records/0/artifact_id")
            .and_then(|value| value.as_str()),
        Some("finding:09:stress-ne-zone")
    );

    let lineage_record = restarted
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/provenance/lineage/finding:09:stress-ne-zone")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(lineage_record.status(), StatusCode::OK);
    let body = to_bytes(lineage_record.into_body(), 64 * 1024).await?;
    let lineage_record_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        lineage_record_json
            .pointer("/inputs/0")
            .and_then(|value| value.as_str()),
        Some("product:ndvi:alpha-2026-06-12")
    );
    assert_eq!(
        lineage_record_json
            .pointer("/parameters/threshold")
            .and_then(|value| value.as_f64()),
        Some(0.42)
    );

    let audit_page = restarted
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/provenance/audit?artifact_id=finding:09:stress-ne-zone&actor_id=operator:dsp-7&start=2026-06-12T09:00:00Z&end=2026-06-12T11:00:00Z&page=1&page_size=10",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(audit_page.status(), StatusCode::OK);
    let body = to_bytes(audit_page.into_body(), 64 * 1024).await?;
    let audit_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        audit_json.get("total").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        audit_json
            .pointer("/entries/0/action/action_ref")
            .and_then(|value| value.as_str()),
        Some("action:record-finding")
    );

    let audit_entry = restarted
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/provenance/audit/audit-entry-hash-0002")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(audit_entry.status(), StatusCode::OK);
    let body = to_bytes(audit_entry.into_body(), 64 * 1024).await?;
    let audit_entry_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        audit_entry_json
            .pointer("/action/artifact_ref")
            .and_then(|value| value.as_str()),
        Some("finding:09:stress-ne-zone")
    );

    Ok(())
}

#[tokio::test]
async fn soil_iot_device_registry_registers_and_lists_geolocated_devices() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let register_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/soil-iot/devices")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "device_id": "soil-probe-001",
                        "org_id": "org-soil-001",
                        "field_id": "field-soil-001",
                        "zone_id": "zone-ne",
                        "sensor_type": "soil_moisture",
                        "position": {
                            "latitude": 38.5816,
                            "longitude": -121.4944,
                            "crs": "EPSG:4326"
                        },
                        "calibration_profile_ref": "calibration:soil-probe-001:v1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(register_response.status(), StatusCode::OK);
    let body = to_bytes(register_response.into_body(), 64 * 1024).await?;
    let registered: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        registered.get("device_id").and_then(|value| value.as_str()),
        Some("soil-probe-001")
    );
    assert_eq!(
        registered.get("status").and_then(|value| value.as_str()),
        Some("active")
    );
    assert_eq!(
        registered
            .get("position")
            .and_then(|value| value.get("crs"))
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/soil-iot/devices?field_id=field-soil-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("device_id").and_then(|value| value.as_str()),
        Some("soil-probe-001")
    );

    Ok(())
}

#[tokio::test]
async fn soil_iot_config_push_history_records_and_lists_status_versions() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    register_soil_iot_test_device(&ctx, "soil-probe-config").await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/soil-iot/devices/soil-probe-config/config-pushes")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "push_id": "config-push-001",
                        "device_id": "soil-probe-config",
                        "config_version": "soil-fw:v3",
                        "pushed_at": "2026-06-12T11:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);
    let body = to_bytes(create_response.into_body(), 64 * 1024).await?;
    let created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created.get("push_status").and_then(|value| value.as_str()),
        Some("pending")
    );
    assert_eq!(
        created
            .get("config_version")
            .and_then(|value| value.as_str()),
        Some("soil-fw:v3")
    );

    let update_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/soil-iot/devices/soil-probe-config/config-pushes/config-push-001/status")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "push_status": "applied",
                        "updated_at": "2026-06-12T11:00:05Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(update_response.status(), StatusCode::OK);
    let body = to_bytes(update_response.into_body(), 64 * 1024).await?;
    let updated: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        updated.get("push_status").and_then(|value| value.as_str()),
        Some("applied")
    );
    assert_eq!(
        updated
            .get("failure_reason")
            .and_then(|value| value.as_str()),
        None
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/soil-iot/devices/soil-probe-config/config-pushes")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("push_id").and_then(|value| value.as_str()),
        Some("config-push-001")
    );
    assert_eq!(
        listed[0]
            .get("push_status")
            .and_then(|value| value.as_str()),
        Some("applied")
    );
    assert_eq!(
        listed[0]
            .get("config_version")
            .and_then(|value| value.as_str()),
        Some("soil-fw:v3")
    );

    let stored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM soil_iot_config_pushes WHERE device_id = 'soil-probe-config'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(stored_count, 1);

    Ok(())
}

#[tokio::test]
async fn soil_iot_config_push_without_ack_is_marked_failed_with_reason() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    register_soil_iot_test_device(&ctx, "soil-probe-timeout").await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/soil-iot/devices/soil-probe-timeout/config-pushes")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "push_id": "config-push-timeout",
                        "config_version": "soil-fw:v4",
                        "pushed_at": "2026-06-12T12:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let timeout_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(
                    "/api/soil-iot/devices/soil-probe-timeout/config-pushes/config-push-timeout/status",
                )
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "push_status": "failed",
                        "failure_reason": "ack_timeout",
                        "updated_at": "2026-06-12T12:15:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(timeout_response.status(), StatusCode::OK);
    let body = to_bytes(timeout_response.into_body(), 64 * 1024).await?;
    let failed: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        failed.get("push_status").and_then(|value| value.as_str()),
        Some("failed")
    );
    assert_eq!(
        failed
            .get("failure_reason")
            .and_then(|value| value.as_str()),
        Some("ack_timeout")
    );

    let persisted_reason: String = sqlx::query_scalar(
        "SELECT failure_reason FROM soil_iot_config_pushes WHERE push_id = 'config-push-timeout'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(persisted_reason, "ack_timeout");

    Ok(())
}

#[tokio::test]
async fn soil_iot_readings_inherit_geolocation_and_persist_via_timeseries() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    register_soil_iot_test_device(&ctx, "soil-probe-001").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/soil-iot/readings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "payload_id": "payload-soil-001",
                        "device_id": "soil-probe-001",
                        "metric": "soil_moisture_percent",
                        "raw_value": 34.5,
                        "gateway_ts": "2026-06-12T10:00:00Z",
                        "received_at": "2026-06-12T10:00:03Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let reading: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        reading.get("field_id").and_then(|value| value.as_str()),
        Some("field-soil-001")
    );
    assert_eq!(
        reading.get("zone_id").and_then(|value| value.as_str()),
        Some("zone-ne")
    );
    assert_eq!(
        reading
            .get("geolocation_status")
            .and_then(|value| value.as_str()),
        Some("located")
    );
    assert_eq!(
        reading
            .pointer("/position/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );

    let series_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(
                    "/api/time-series/points?entity_ref=device:soil-probe-001&metric=soil_moisture_percent",
                )
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(series_response.status(), StatusCode::OK);
    let body = to_bytes(series_response.into_body(), 64 * 1024).await?;
    let points: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0].get("entity_ref").and_then(|value| value.as_str()),
        Some("device:soil-probe-001")
    );
    assert_eq!(
        points[0]
            .pointer("/value/value")
            .and_then(|value| value.as_f64()),
        Some(34.5)
    );
    assert_eq!(
        points[0]
            .pointer("/metadata/field_id")
            .and_then(|value| value.as_str()),
        Some("field-soil-001")
    );
    assert_eq!(
        points[0]
            .pointer("/metadata/position/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );

    Ok(())
}

#[tokio::test]
async fn soil_iot_reading_with_invalid_device_position_is_flagged_not_defaulted() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    register_soil_iot_test_device(&ctx, "soil-probe-bad-geo").await?;
    sqlx::query("UPDATE soil_iot_devices SET latitude = ?1 WHERE device_id = ?2")
        .bind(120.0)
        .bind("soil-probe-bad-geo")
        .execute(&ctx.pool)
        .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/soil-iot/readings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "payload_id": "payload-soil-bad-geo",
                        "device_id": "soil-probe-bad-geo",
                        "metric": "soil_moisture_percent",
                        "raw_value": 34.5,
                        "gateway_ts": "2026-06-12T10:00:00Z",
                        "received_at": "2026-06-12T10:00:03Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let reading: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        reading
            .get("geolocation_status")
            .and_then(|value| value.as_str()),
        Some("no_geolocation")
    );
    assert!(reading.get("position").is_some_and(|value| value.is_null()));
    assert_eq!(
        reading
            .get("excluded_from_geospatial_products")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        reading
            .pointer("/qa_flags/0")
            .and_then(|value| value.as_str()),
        Some("no_geolocation")
    );

    let point_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM time_series_points WHERE entity_ref = ?1")
            .bind("device:soil-probe-bad-geo")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(point_count, 0);

    Ok(())
}

#[tokio::test]
async fn orthomosaic_frame_set_ingest_lists_pose_metadata() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_orthomosaic_scene(&ctx, "ortho-scene-1", "ortho-field-1", "season-2026").await?;

    let ingest_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/orthomosaic/frame-sets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "frame_set_id": "frame-set-001",
                        "scene_id": "ortho-scene-1",
                        "field_id": "ortho-field-1",
                        "season_id": "season-2026",
                        "crs_hint": "EPSG:4326",
                        "frames": [
                            {
                                "frame_id": "frame-001",
                                "capture_ts": "2026-06-01T12:00:00Z",
                                "gps": {
                                    "latitude": 41.10,
                                    "longitude": -96.70,
                                    "altitude": 120.0
                                },
                                "imu": {
                                    "roll_deg": 1.2,
                                    "pitch_deg": -0.4,
                                    "yaw_deg": 87.0
                                },
                                "exif": {
                                    "camera_model": "MicaSense RedEdge",
                                    "focal_length_mm": 5.4,
                                    "image_width_px": 1280,
                                    "image_height_px": 960
                                }
                            },
                            {
                                "frame_id": "frame-002",
                                "capture_ts": "2026-06-01T12:00:02Z",
                                "gps": {
                                    "latitude": 41.1005,
                                    "longitude": -96.6995,
                                    "altitude": 121.0
                                },
                                "imu": {
                                    "roll_deg": 1.0,
                                    "pitch_deg": -0.2,
                                    "yaw_deg": 88.0
                                },
                                "exif": {
                                    "camera_model": "MicaSense RedEdge",
                                    "focal_length_mm": 5.4,
                                    "image_width_px": 1280,
                                    "image_height_px": 960
                                }
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(ingest_response.status(), StatusCode::OK);
    let body = to_bytes(ingest_response.into_body(), 64 * 1024).await?;
    let frame_set: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        frame_set
            .get("frame_set_id")
            .and_then(|value| value.as_str()),
        Some("frame-set-001")
    );
    assert_eq!(
        frame_set.get("scene_id").and_then(|value| value.as_str()),
        Some("ortho-scene-1")
    );
    assert_eq!(
        frame_set.get("crs_hint").and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    let frames = frame_set
        .get("frames")
        .and_then(|value| value.as_array())
        .expect("frames should be returned");
    assert_eq!(frames.len(), 2);
    assert_eq!(
        frames[0]
            .pointer("/gps/latitude")
            .and_then(|value| value.as_f64()),
        Some(41.10)
    );
    assert_eq!(
        frames[0]
            .pointer("/imu/yaw_deg")
            .and_then(|value| value.as_f64()),
        Some(87.0)
    );
    assert_eq!(
        frames[0]
            .pointer("/exif/camera_model")
            .and_then(|value| value.as_str()),
        Some("MicaSense RedEdge")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/orthomosaic/frame-sets?scene_id=ortho-scene-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0]
            .get("frame_set_id")
            .and_then(|value| value.as_str()),
        Some("frame-set-001")
    );

    Ok(())
}

#[tokio::test]
async fn orthomosaic_frame_set_ingest_rejects_no_pose_frames() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_orthomosaic_scene(&ctx, "ortho-scene-2", "ortho-field-2", "season-2026").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/orthomosaic/frame-sets")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "frame_set_id": "frame-set-no-pose",
                        "scene_id": "ortho-scene-2",
                        "field_id": "ortho-field-2",
                        "season_id": "season-2026",
                        "frames": [
                            {
                                "frame_id": "frame-001",
                                "capture_ts": "2026-06-01T12:00:00Z",
                                "exif": {
                                    "camera_model": "MicaSense RedEdge"
                                }
                            }
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("no camera pose"));

    let frame_set_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orthomosaic_frame_sets")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(frame_set_count, 0);

    Ok(())
}

#[tokio::test]
async fn orthomosaic_reconstruction_submit_status_and_failure_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_orthomosaic_frame_set(
        &ctx,
        "recon-scene-1",
        "recon-field-1",
        "season-2026",
        "frame-set-recon-1",
    )
    .await?;

    let submit_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/orthomosaic/reconstructions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recon_id": "recon-001",
                        "frame_set_id": "frame-set-recon-1",
                        "params": {
                            "feature_detector": "orb",
                            "max_features": 4000
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(submit_response.status(), StatusCode::OK);
    let body = to_bytes(submit_response.into_body(), 64 * 1024).await?;
    let submitted: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        submitted.get("recon_id").and_then(|value| value.as_str()),
        Some("recon-001")
    );
    assert_eq!(
        submitted.get("status").and_then(|value| value.as_str()),
        Some("queued")
    );
    assert_eq!(
        submitted
            .pointer("/params/feature_detector")
            .and_then(|value| value.as_str()),
        Some("orb")
    );

    let status_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/orthomosaic/reconstructions/recon-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(status_response.status(), StatusCode::OK);
    let body = to_bytes(status_response.into_body(), 64 * 1024).await?;
    let status: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        status.get("status").and_then(|value| value.as_str()),
        Some("queued")
    );

    let fail_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/orthomosaic/reconstructions/recon-001/status")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "failed",
                        "failure_reason": "feature-match-insufficient-overlap"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(fail_response.status(), StatusCode::OK);
    let body = to_bytes(fail_response.into_body(), 64 * 1024).await?;
    let failed: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        failed.get("status").and_then(|value| value.as_str()),
        Some("failed")
    );
    assert_eq!(
        failed
            .get("failure_reason")
            .and_then(|value| value.as_str()),
        Some("feature-match-insufficient-overlap")
    );

    Ok(())
}

#[tokio::test]
async fn orthomosaic_reconstruction_rejects_unknown_frame_set() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/orthomosaic/reconstructions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recon_id": "recon-missing-frame-set",
                        "frame_set_id": "missing-frame-set",
                        "params": {
                            "feature_detector": "orb"
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("frame_set_id missing-frame-set does not exist"));

    let recon_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orthomosaic_reconstructions")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(recon_count, 0);

    Ok(())
}

#[tokio::test]
async fn orthomosaic_tile_handoff_publishes_geo_hub_layers() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "ortho-handoff-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = orthomosaic_tile_spatial_ref_json();
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, spatial_ref.clone()).await?;
    link_scene_context(&ctx, scene_id, "ortho-field-1", "season-2026").await?;
    seed_completed_orthomosaic_reconstruction(
        &ctx,
        scene_id,
        "ortho-field-1",
        "season-2026",
        "frame-set-handoff",
        "recon-ortho-handoff",
    )
    .await?;

    let mosaic_path = scene_dir
        .join("products")
        .join("orthomosaic")
        .join("orthomosaic.png");
    let dsm_path = scene_dir.join("products").join("dsm").join("dsm.png");
    std::fs::create_dir_all(mosaic_path.parent().expect("mosaic parent exists"))?;
    std::fs::create_dir_all(dsm_path.parent().expect("dsm parent exists"))?;
    write_gray_png(&mosaic_path, 180)?;
    write_gray_png(&dsm_path, 90)?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/orthomosaic/reconstructions/recon-ortho-handoff/handoff")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "scene_id": scene_id,
                        "recon_id": "recon-ortho-handoff",
                        "generated_at": "2026-06-01T12:08:00Z",
                        "source_image_ids": ["frame-001", "frame-002"],
                        "tile_size_px": 256,
                        "mosaic": {
                            "uri": mosaic_path.to_string_lossy().to_string(),
                            "width_px": 2,
                            "height_px": 2,
                            "spatial_ref": spatial_ref.clone(),
                            "gsd_m_per_px": 0.05
                        },
                        "dsm": {
                            "uri": dsm_path.to_string_lossy().to_string(),
                            "width_px": 2,
                            "height_px": 2,
                            "spatial_ref": spatial_ref.clone(),
                            "gsd_m_per_px": 0.05
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let handoff: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        handoff
            .pointer("/layers/0/product_kind")
            .and_then(|value| value.as_str()),
        Some("orthomosaic")
    );
    assert_eq!(
        handoff
            .pointer("/layers/0/tile_url_template")
            .and_then(|value| value.as_str()),
        Some(
            format!("/api/scenes/{scene_id}/products/orthomosaic/tiles/{{z}}/{{x}}/{{y}}.png")
                .as_str()
        )
    );
    assert_eq!(
        handoff
            .pointer("/layers/0/spatial_ref/bbox/min_lon")
            .and_then(|value| value.as_f64()),
        Some(-96.7)
    );

    let layer_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/layers/{scene_id}/orthomosaic"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(layer_response.status(), StatusCode::OK);
    let body = to_bytes(layer_response.into_body(), 64 * 1024).await?;
    let layer_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        layer_json
            .get("product_id")
            .and_then(|value| value.as_str()),
        Some("ortho-handoff-scene:orthomosaic")
    );
    assert_eq!(
        layer_json
            .pointer("/spatial_ref/resolution/x")
            .and_then(|value| value.as_f64()),
        Some(0.05)
    );
    assert_eq!(
        layer_json
            .get("gsd_m_per_px")
            .and_then(|value| value.as_f64()),
        Some(0.05)
    );
    assert_eq!(
        layer_json
            .get("tile_url_template")
            .and_then(|value| value.as_str()),
        Some(
            format!("/api/scenes/{scene_id}/products/orthomosaic/tiles/{{z}}/{{x}}/{{y}}.png")
                .as_str()
        )
    );
    assert_eq!(
        layer_json
            .pointer("/source_scan_ids/0")
            .and_then(|value| value.as_str()),
        Some("frame-001")
    );

    let orthomosaic_row = sqlx::query(
        r#"
        SELECT source_image_ids_json, source_scan_ids_json
        FROM products
        WHERE scene_id = ?1 AND kind = ?2
        "#,
    )
    .bind(scene_id)
    .bind("orthomosaic")
    .fetch_one(&ctx.pool)
    .await?;
    let persisted_source_image_ids: Vec<String> =
        serde_json::from_str(&orthomosaic_row.get::<String, _>("source_image_ids_json"))?;
    let persisted_source_scan_ids: Vec<String> =
        serde_json::from_str(&orthomosaic_row.get::<String, _>("source_scan_ids_json"))?;
    assert_eq!(
        persisted_source_image_ids,
        vec!["frame-001".to_string(), "frame-002".to_string()]
    );
    assert_eq!(
        persisted_source_scan_ids,
        vec!["frame-001".to_string(), "frame-002".to_string()]
    );

    let tile_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/products/orthomosaic/tiles/0/0/0.png"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(tile_response.status(), StatusCode::OK);
    assert_eq!(
        tile_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("image/png")
    );

    let dsm_export_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/layers/{scene_id}/dsm/export/geotiff"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(dsm_export_response.status(), StatusCode::OK);
    let body = to_bytes(dsm_export_response.into_body(), 64 * 1024).await?;
    let reopened = reopen_raster_geotiff(&body)?;
    assert_eq!(reopened.product_id, "ortho-handoff-scene:dsm");
    assert_eq!(reopened.width, 2);
    assert_eq!(reopened.height, 2);
    assert_eq!(reopened.spatial_ref.crs.as_deref(), Some("EPSG:4326"));

    Ok(())
}

#[tokio::test]
async fn orthomosaic_tile_handoff_rejects_missing_field_and_season_linkage() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "ortho-handoff-orphan";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = orthomosaic_tile_spatial_ref_json();
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, spatial_ref.clone()).await?;

    seed_completed_orthomosaic_reconstruction(
        &ctx,
        scene_id,
        "",
        "",
        "frame-set-handoff-orphan",
        "recon-ortho-handoff-orphan",
    )
    .await?;

    let mosaic_path = scene_dir
        .join("products")
        .join("orthomosaic")
        .join("orthomosaic.png");
    let dsm_path = scene_dir.join("products").join("dsm").join("dsm.png");
    std::fs::create_dir_all(mosaic_path.parent().expect("mosaic parent exists"))?;
    std::fs::create_dir_all(dsm_path.parent().expect("dsm parent exists"))?;
    write_gray_png(&mosaic_path, 180)?;
    write_gray_png(&dsm_path, 90)?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/orthomosaic/reconstructions/recon-ortho-handoff-orphan/handoff")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "scene_id": scene_id,
                        "recon_id": "recon-ortho-handoff-orphan",
                        "generated_at": "2026-06-01T12:08:00Z",
                        "source_image_ids": ["frame-001", "frame-002"],
                        "tile_size_px": 256,
                        "mosaic": {
                            "uri": mosaic_path.to_string_lossy().to_string(),
                            "width_px": 2,
                            "height_px": 2,
                            "spatial_ref": spatial_ref.clone(),
                            "gsd_m_per_px": 0.05
                        },
                        "dsm": {
                            "uri": dsm_path.to_string_lossy().to_string(),
                            "width_px": 2,
                            "height_px": 2,
                            "spatial_ref": spatial_ref,
                            "gsd_m_per_px": 0.05
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("missing field_id") || message.contains("missing scene_id"));

    let persisted_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE scene_id = ?1 AND kind = ?2")
            .bind(scene_id)
            .bind("orthomosaic")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(persisted_count, 0);

    Ok(())
}

#[tokio::test]
async fn orthomosaic_tile_handoff_refuses_missing_crs_without_product_rows() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "ortho-handoff-missing-crs";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = orthomosaic_tile_spatial_ref_json();
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, spatial_ref.clone()).await?;
    link_scene_context(&ctx, scene_id, "ortho-field-1", "season-2026").await?;
    seed_completed_orthomosaic_reconstruction(
        &ctx,
        scene_id,
        "ortho-field-1",
        "season-2026",
        "frame-set-handoff-missing-crs",
        "recon-ortho-handoff",
    )
    .await?;

    let mosaic_path = scene_dir
        .join("products")
        .join("orthomosaic")
        .join("orthomosaic.png");
    let dsm_path = scene_dir.join("products").join("dsm").join("dsm.png");
    std::fs::create_dir_all(mosaic_path.parent().expect("mosaic parent exists"))?;
    std::fs::create_dir_all(dsm_path.parent().expect("dsm parent exists"))?;
    write_gray_png(&mosaic_path, 180)?;
    write_gray_png(&dsm_path, 90)?;

    let mut missing_crs = spatial_ref.clone();
    missing_crs
        .as_object_mut()
        .expect("spatial ref should be an object")
        .remove("crs");

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/orthomosaic/reconstructions/recon-ortho-handoff/handoff")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "scene_id": scene_id,
                        "recon_id": "recon-ortho-handoff",
                        "generated_at": "2026-06-01T12:08:00Z",
                        "source_image_ids": ["frame-001", "frame-002"],
                        "tile_size_px": 256,
                        "mosaic": {
                            "uri": mosaic_path.to_string_lossy().to_string(),
                            "width_px": 2,
                            "height_px": 2,
                            "spatial_ref": missing_crs,
                            "gsd_m_per_px": 0.05
                        },
                        "dsm": {
                            "uri": dsm_path.to_string_lossy().to_string(),
                            "width_px": 2,
                            "height_px": 2,
                            "spatial_ref": spatial_ref,
                            "gsd_m_per_px": 0.05
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("georeferencing missing CRS"));

    let product_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE scene_id = ?1")
            .bind(scene_id)
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(product_count, 0);

    Ok(())
}

#[tokio::test]
async fn orthomosaic_publish_gate_marks_publishable_product_with_provenance() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "ortho-publish-scene";
    seed_orthomosaic_publish_product(&ctx, scene_id, "orthomosaic").await?;

    let first = post_orthomosaic_publish_gate(&ctx, scene_id, "orthomosaic", "publishable").await?;
    let repeated =
        post_orthomosaic_publish_gate(&ctx, scene_id, "orthomosaic", "publishable").await?;

    assert_eq!(
        first.get("status").and_then(|value| value.as_str()),
        Some("published")
    );
    assert_eq!(
        first.get("qa_report_ref").and_then(|value| value.as_str()),
        Some("qa-report-001")
    );
    assert_eq!(
        first
            .get("provenance_hash")
            .and_then(|value| value.as_str()),
        repeated
            .get("provenance_hash")
            .and_then(|value| value.as_str())
    );
    assert!(first
        .get("provenance_hash")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(
        first
            .pointer("/downstream_consumers/0")
            .and_then(|value| value.as_str()),
        Some("imagery_processor")
    );

    let row = sqlx::query(
        "SELECT publish_status, qa_report_ref, provenance_hash, downstream_consumers_json FROM products WHERE scene_id = ?1 AND kind = ?2",
    )
    .bind(scene_id)
    .bind("orthomosaic")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<String, _>("publish_status"), "published");
    assert_eq!(row.get::<String, _>("qa_report_ref"), "qa-report-001");
    assert_eq!(
        row.get::<String, _>("provenance_hash"),
        first
            .get("provenance_hash")
            .and_then(|value| value.as_str())
            .expect("provenance hash should exist")
    );

    let layer_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/layers/{scene_id}/orthomosaic"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(layer_response.status(), StatusCode::OK);
    let body = to_bytes(layer_response.into_body(), 64 * 1024).await?;
    let layer_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        layer_json
            .get("publish_status")
            .and_then(|value| value.as_str()),
        Some("published")
    );
    assert_eq!(
        layer_json
            .get("provenance_hash")
            .and_then(|value| value.as_str()),
        first
            .get("provenance_hash")
            .and_then(|value| value.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn orthomosaic_publish_gate_blocks_failed_quality_without_consumers() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "ortho-publish-blocked";
    seed_orthomosaic_publish_product(&ctx, scene_id, "orthomosaic").await?;

    let decision =
        post_orthomosaic_publish_gate(&ctx, scene_id, "orthomosaic", "not_publishable").await?;

    assert_eq!(
        decision.get("status").and_then(|value| value.as_str()),
        Some("blocked")
    );
    assert_eq!(
        decision
            .get("blocked_reason")
            .and_then(|value| value.as_str()),
        Some("quality_report_not_publishable")
    );
    assert!(decision
        .get("downstream_consumers")
        .and_then(|value| value.as_array())
        .is_some_and(Vec::is_empty));

    let row = sqlx::query(
        "SELECT publish_status, downstream_consumers_json FROM products WHERE scene_id = ?1 AND kind = ?2",
    )
    .bind(scene_id)
    .bind("orthomosaic")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(row.get::<String, _>("publish_status"), "blocked");
    let consumers: Vec<String> =
        serde_json::from_str(&row.get::<String, _>("downstream_consumers_json"))?;
    assert!(consumers.is_empty());

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_model_registry_registers_and_lists_versions() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let register_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/models")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "model_id": "lesion-detector",
                        "version": "2026.06.1",
                        "task": "disease_detection",
                        "training_set_ref": "dataset:lesion-v3",
                        "metrics": {
                            "precision": 0.91,
                            "recall": 0.87,
                            "iou": 0.73
                        },
                        "provenance_ref": "provenance:model/lesion-detector/2026.06.1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(register_response.status(), StatusCode::OK);
    let body = to_bytes(register_response.into_body(), 64 * 1024).await?;
    let registered: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        registered.get("model_id").and_then(|value| value.as_str()),
        Some("lesion-detector")
    );
    assert_eq!(
        registered.get("version").and_then(|value| value.as_str()),
        Some("2026.06.1")
    );
    assert_eq!(
        registered.get("task").and_then(|value| value.as_str()),
        Some("disease_detection")
    );
    assert_eq!(
        registered
            .pointer("/metrics/precision")
            .and_then(|value| value.as_f64()),
        Some(0.91)
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/crop-intelligence/models?task=disease_detection")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("version").and_then(|value| value.as_str()),
        Some("2026.06.1")
    );

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_unregistered_model_inference_is_rejected_and_audited() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/inference-requests/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "model_id": "unknown-model",
                        "version": "v0"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("unregistered model unknown-model@v0"));

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM crop_model_events WHERE model_id = ?1 AND version = ?2 AND event_type = ?3",
    )
    .bind("unknown-model")
    .bind("v0")
    .bind("unregistered_model_rejected")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_inference_run_submit_status_and_result_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "crop-run-scene";
    seed_orthomosaic_publish_product(&ctx, scene_id, "orthomosaic").await?;
    post_orthomosaic_publish_gate(&ctx, scene_id, "orthomosaic", "publishable").await?;

    let submit = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/inference-runs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "run_id": "crop-run-001",
                        "mosaic_ref": format!("{scene_id}:orthomosaic"),
                        "field_id": "ortho-field-1",
                        "season_id": "season-2026"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(submit.status(), StatusCode::OK);
    let body = to_bytes(submit.into_body(), 64 * 1024).await?;
    let queued: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        queued.get("status").and_then(|value| value.as_str()),
        Some("queued")
    );
    assert_eq!(
        queued.get("model_version").and_then(|value| value.as_str()),
        Some("deterministic")
    );

    let status = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/crop-intelligence/inference-runs/crop-run-001")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(status.status(), StatusCode::OK);

    let early_result = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/crop-intelligence/inference-runs/crop-run-001/result")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(early_result.status(), StatusCode::BAD_REQUEST);

    for status in ["running", "completed"] {
        let response = ctx
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/crop-intelligence/inference-runs/crop-run-001/status")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({ "status": status }).to_string()))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle request");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let result = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/crop-intelligence/inference-runs/crop-run-001/result")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(result.status(), StatusCode::OK);
    let body = to_bytes(result.into_body(), 64 * 1024).await?;
    let completed: serde_json::Value = serde_json::from_slice(&body)?;
    let expected_mosaic_ref = format!("{scene_id}:orthomosaic");
    assert_eq!(
        completed.get("status").and_then(|value| value.as_str()),
        Some("completed")
    );
    assert_eq!(
        completed.get("mosaic_ref").and_then(|value| value.as_str()),
        Some(expected_mosaic_ref.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_inference_run_rejects_unpublished_mosaic_without_writing() -> Result<()>
{
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "crop-run-unpublished";
    seed_orthomosaic_publish_product(&ctx, scene_id, "orthomosaic").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/inference-runs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "run_id": "crop-run-blocked",
                        "mosaic_ref": format!("{scene_id}:orthomosaic"),
                        "field_id": "ortho-field-1",
                        "season_id": "season-2026"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("not published"));

    let run_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM crop_inference_runs WHERE run_id = 'crop-run-blocked'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(run_count, 0);

    Ok(())
}

#[tokio::test]
async fn copilot_conversation_create_turn_and_list_are_field_scoped() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-copilot", "field-copilot", "season-2026").await?;

    let start = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/copilot/conversations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "conversation_id": "conversation-001",
                        "field_id": "field-copilot"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(start.status(), StatusCode::OK);
    let body = to_bytes(start.into_body(), 64 * 1024).await?;
    let conversation: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        conversation
            .get("conversation_id")
            .and_then(|value| value.as_str()),
        Some("conversation-001")
    );
    assert_eq!(
        conversation
            .get("field_id")
            .and_then(|value| value.as_str()),
        Some("field-copilot")
    );

    let turn = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/copilot/conversations/conversation-001/turns")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "turn_id": "turn-001",
                        "field_id": "field-copilot",
                        "role": "user"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(turn.status(), StatusCode::OK);
    let body = to_bytes(turn.into_body(), 64 * 1024).await?;
    let turn: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        turn.get("turn_id").and_then(|value| value.as_str()),
        Some("turn-001")
    );
    assert_eq!(
        turn.get("role").and_then(|value| value.as_str()),
        Some("user")
    );

    let list_conversations = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/copilot/conversations?field_id=field-copilot")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_conversations.status(), StatusCode::OK);
    let body = to_bytes(list_conversations.into_body(), 64 * 1024).await?;
    let conversations: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(conversations.len(), 1);

    let list_turns = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/copilot/conversations/conversation-001/turns")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_turns.status(), StatusCode::OK);
    let body = to_bytes(list_turns.into_body(), 64 * 1024).await?;
    let turns: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(turns.len(), 1);
    assert_eq!(
        turns[0].get("field_id").and_then(|value| value.as_str()),
        Some("field-copilot")
    );

    Ok(())
}

#[tokio::test]
async fn copilot_conversation_rejects_missing_field_scope_without_writing() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_sustainability_field(&ctx, "farm-copilot", "field-copilot", "season-2026").await?;

    let missing_field = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/copilot/conversations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "conversation_id": "conversation-missing",
                        "field_id": "field-missing"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(missing_field.status(), StatusCode::BAD_REQUEST);

    let start = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/copilot/conversations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "conversation_id": "conversation-002",
                        "field_id": "field-copilot"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(start.status(), StatusCode::OK);

    let wrong_field_turn = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/copilot/conversations/conversation-002/turns")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "turn_id": "turn-wrong-field",
                        "field_id": "field-missing",
                        "role": "assistant"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(wrong_field_turn.status(), StatusCode::BAD_REQUEST);

    let missing_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM copilot_conversations WHERE conversation_id = 'conversation-missing'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(missing_count, 0);
    let wrong_turn_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM copilot_turns WHERE turn_id = 'turn-wrong-field'")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(wrong_turn_count, 0);

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_verifies_and_corrects_detections_with_label_feedback() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/detections/disease:tile-1:1/verification")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "task": "disease_detection",
                        "label": "northern_leaf_blight",
                        "confidence": 0.82,
                        "evidence_tile_refs": ["tile-1"],
                        "zone_geometry": {
                            "crs": "EPSG:32614",
                            "bbox": {
                                "min_lon": 5.0,
                                "min_lat": 5.0,
                                "max_lon": 15.0,
                                "max_lat": 15.0
                            }
                        },
                        "action": "corrected",
                        "actor": "agronomist-7",
                        "verified_at": "2026-06-12T14:00:00Z",
                        "corrected_label": "nitrogen_stress",
                        "corrected_geometry": {
                            "crs": "EPSG:32614",
                            "bbox": {
                                "min_lon": 6.0,
                                "min_lat": 6.0,
                                "max_lon": 16.0,
                                "max_lat": 16.0
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let verification: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        verification
            .get("verification_state")
            .and_then(|value| value.as_str()),
        Some("corrected")
    );
    assert_eq!(
        verification.get("actor").and_then(|value| value.as_str()),
        Some("agronomist-7")
    );
    assert_eq!(
        verification
            .get("corrected_label")
            .and_then(|value| value.as_str()),
        Some("nitrogen_stress")
    );
    assert_eq!(
        verification
            .pointer("/correction_label/label")
            .and_then(|value| value.as_str()),
        Some("nitrogen_stress")
    );

    let state: String = sqlx::query_scalar(
        "SELECT verification_state FROM crop_detection_verifications WHERE detection_id = ?1",
    )
    .bind("disease:tile-1:1")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(state, "corrected");

    let label_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM crop_detection_correction_labels WHERE source_detection_id = ?1 AND label = ?2",
    )
    .bind("disease:tile-1:1")
    .bind("nitrogen_stress")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(label_count, 1);

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_blocks_unverified_detection_finding_promotion_by_default() -> Result<()>
{
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let blocked = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/detections/weed:tile-1:1/finding-promotion/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "allow_unverified": false
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(blocked.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(blocked.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("unverified detection weed:tile-1:1 cannot be promoted"));

    let allowed = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/detections/weed:tile-1:1/finding-promotion/validate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "allow_unverified": true
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(allowed.status(), StatusCode::OK);
    let body = to_bytes(allowed.into_body(), 64 * 1024).await?;
    let decision: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        decision
            .get("verification_state")
            .and_then(|value| value.as_str()),
        Some("unverified")
    );
    assert_eq!(
        decision
            .get("promotion_allowed")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        decision.get("reason").and_then(|value| value.as_str()),
        Some("unverified_override")
    );

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_emits_verified_detection_finding_into_recommendations() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "crop-finding-scene";
    seed_orthomosaic_scene(&ctx, scene_id, "crop-field-1", "season-2026").await?;

    let verify_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/crop-intelligence/detections/disease:tile-1:1/verification")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "task": "disease_detection",
                        "label": "northern_leaf_blight",
                        "confidence": 0.82,
                        "evidence_tile_refs": ["tile-1"],
                        "zone_geometry": {
                            "crs": "EPSG:4326",
                            "bbox": {
                                "min_lon": -96.60,
                                "min_lat": 41.18,
                                "max_lon": -96.55,
                                "max_lat": 41.22
                            }
                        },
                        "action": "confirmed",
                        "actor": "agronomist-7",
                        "verified_at": "2026-06-12T14:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(verify_response.status(), StatusCode::OK);

    let emit_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/scenes/{scene_id}/crop-intelligence/detections/disease:tile-1:1/findings"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "finding_id": "crop-finding-1",
                        "zone_id": "zone-a",
                        "model_id": "lesion-detector",
                        "version": "2026.06.1",
                        "emitted_at": "2026-06-12T15:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(emit_response.status(), StatusCode::OK);
    let body = to_bytes(emit_response.into_body(), 64 * 1024).await?;
    let recommendation: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        recommendation
            .get("recommendation_id")
            .and_then(|value| value.as_str()),
        Some("crop-finding-1")
    );
    assert_eq!(
        recommendation
            .get("field_id")
            .and_then(|value| value.as_str()),
        Some("crop-field-1")
    );
    assert_eq!(
        recommendation
            .get("category")
            .and_then(|value| value.as_str()),
        Some("crop_intelligence_finding")
    );
    let evidence_refs = recommendation
        .get("evidence_refs")
        .and_then(|value| value.as_array())
        .expect("recommendation should cite evidence");
    assert!(evidence_refs.iter().any(|value| value == "tile:tile-1"));
    assert!(evidence_refs
        .iter()
        .any(|value| value == "model:lesion-detector@2026.06.1"));
    assert_eq!(
        recommendation
            .pointer("/annotation_ids/0")
            .and_then(|value| value.as_str()),
        Some("crop-finding-1-zone")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let recommendations: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(recommendations.len(), 1);
    assert_eq!(
        recommendations[0]
            .pointer("/evidence_refs/0")
            .and_then(|value| value.as_str()),
        Some("annotation:crop-finding-1-zone")
    );
    assert!(recommendations[0]
        .get("evidence_refs")
        .and_then(|value| value.as_array())
        .expect("listed recommendation should cite evidence")
        .iter()
        .any(|value| value == "tile:tile-1"));

    Ok(())
}

#[tokio::test]
async fn crop_intelligence_rejects_uncited_finding_emission() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "crop-finding-uncited";
    seed_orthomosaic_scene(&ctx, scene_id, "crop-field-2", "season-2026").await?;
    sqlx::query(
        r#"
        INSERT INTO crop_detection_verifications (
            detection_id, task, label, confidence, evidence_tile_refs_json,
            zone_geometry_json, verification_state, actor, verified_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind("disease:tile-empty:1")
    .bind("disease_detection")
    .bind("northern_leaf_blight")
    .bind(0.82)
    .bind("[]")
    .bind(
        json!({
            "crs": "EPSG:4326",
            "bbox": {
                "min_lon": -96.60,
                "min_lat": 41.18,
                "max_lon": -96.55,
                "max_lat": 41.22
            }
        })
        .to_string(),
    )
    .bind("confirmed")
    .bind("agronomist-7")
    .bind("2026-06-12T14:00:00Z")
    .execute(&ctx.pool)
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/scenes/{scene_id}/crop-intelligence/detections/disease:tile-empty:1/findings"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "finding_id": "crop-finding-uncited",
                        "model_id": "lesion-detector",
                        "version": "2026.06.1",
                        "emitted_at": "2026-06-12T15:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("finding evidence_tile_refs cannot be empty"));

    Ok(())
}

#[tokio::test]
async fn compliance_records_create_list_append_versions_and_refuse_delete() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_compliance_field(&ctx, "field-north", "org-alpha").await?;
    seed_compliance_field(&ctx, "field-south", "org-alpha").await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "record_id": "comp-rec-1",
                        "record_type": "compliance_report",
                        "org_id": "org-alpha",
                        "field_id": "field-north",
                        "flight_id": "flight-77",
                        "actor": "compliance-officer-1",
                        "provenance_ref": "provenance:compliance/comp-rec-1/v1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);
    let body = to_bytes(create_response.into_body(), 64 * 1024).await?;
    let created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created.get("record_id").and_then(|value| value.as_str()),
        Some("comp-rec-1")
    );
    assert_eq!(
        created.get("version").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        created
            .get("provenance_ref")
            .and_then(|value| value.as_str()),
        Some("provenance:compliance/comp-rec-1/v1")
    );

    let append_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/records/comp-rec-1/versions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": "field-south",
                        "flight_id": "flight-77",
                        "actor": "compliance-officer-2",
                        "provenance_ref": "provenance:compliance/comp-rec-1/v2",
                        "change_reason": "corrected field linkage"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(append_response.status(), StatusCode::OK);
    let body = to_bytes(append_response.into_body(), 64 * 1024).await?;
    let appended: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        appended.get("version").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        appended.get("field_id").and_then(|value| value.as_str()),
        Some("field-south")
    );
    assert_eq!(
        appended
            .get("prior_version")
            .and_then(|value| value.as_u64()),
        Some(1)
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/compliance/records?record_id=comp-rec-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let versions: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(versions.len(), 2);
    assert_eq!(
        versions[0].get("version").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        versions[0].get("field_id").and_then(|value| value.as_str()),
        Some("field-north")
    );
    assert_eq!(
        versions[1].get("version").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        versions[1].get("field_id").and_then(|value| value.as_str()),
        Some("field-south")
    );

    let delete_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/compliance/records/comp-rec-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(delete_response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(delete_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("append-only"));

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM compliance_record_events WHERE record_id = ?1 AND event_type = ?2",
    )
    .bind("comp-rec-1")
    .bind("delete_refused")
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(audit_count, 1);

    let retained_versions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM compliance_records WHERE record_id = ?1")
            .bind("comp-rec-1")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(retained_versions, 2);

    Ok(())
}

#[tokio::test]
async fn compliance_remote_id_and_chemical_payloads_are_persisted_and_validated() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_compliance_field(&ctx, "field-north", "org-alpha").await?;

    let remote_payload = json!({
        "record_id": "remote-log-1",
        "record_type": "remote_id_log",
        "org_id": "org-alpha",
        "field_id": "field-north",
        "actor": "operator-17",
        "provenance_ref": "provenance:remote-id/remote-log-1/v1",
        "payload": {
            "flight_id": "flight-77",
            "operator_id": "operator-17",
            "aircraft_id": "aircraft-ag-9",
            "started_at": "2026-06-12T12:00:00Z",
            "ended_at": "2026-06-12T12:18:00Z",
            "track": [
                {
                    "observed_at": "2026-06-12T12:02:00Z",
                    "longitude": -96.61,
                    "latitude": 41.21,
                    "altitude_m": 118.0
                },
                {
                    "observed_at": "2026-06-12T12:10:00Z",
                    "longitude": -96.58,
                    "latitude": 41.24,
                    "altitude_m": 116.0
                }
            ],
            "telemetry_gaps": [
                {
                    "started_at": "2026-06-12T12:04:00Z",
                    "ended_at": "2026-06-12T12:08:00Z",
                    "reason": "remote-id-broadcast-dropout"
                }
            ]
        }
    });

    let remote_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(remote_payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(remote_response.status(), StatusCode::OK);
    let body = to_bytes(remote_response.into_body(), 64 * 1024).await?;
    let remote_created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        remote_created
            .get("flight_id")
            .and_then(|value| value.as_str()),
        Some("flight-77")
    );
    assert_eq!(
        remote_created
            .pointer("/payload/telemetry_gaps/0/reason")
            .and_then(|value| value.as_str()),
        Some("remote-id-broadcast-dropout")
    );

    let remote_list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/compliance/records?record_id=remote-log-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(remote_list_response.status(), StatusCode::OK);
    let body = to_bytes(remote_list_response.into_body(), 64 * 1024).await?;
    let remote_versions: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(
        remote_versions[0]
            .pointer("/payload/operator_id")
            .and_then(|value| value.as_str()),
        Some("operator-17")
    );

    let chemical_payload = json!({
        "record_id": "chem-app-1",
        "record_type": "chemical_application",
        "org_id": "org-alpha",
        "field_id": "field-north",
        "flight_id": "flight-77",
        "actor": "operator-17",
        "provenance_ref": "provenance:application/chem-app-1/v1",
        "payload": {
            "application_id": "chem-app-1",
            "product": "Example Herbicide",
            "epa_or_label_ref": "EPA-12345-LBL",
            "field_id": "field-north",
            "geometry": {
                "crs": "EPSG:4326",
                "coordinates": [
                    { "longitude": -96.70, "latitude": 41.10 },
                    { "longitude": -96.20, "latitude": 41.10 },
                    { "longitude": -96.20, "latitude": 41.40 },
                    { "longitude": -96.70, "latitude": 41.40 },
                    { "longitude": -96.70, "latitude": 41.10 }
                ]
            },
            "applied_at": "2026-06-12T13:00:00Z",
            "rate": 1.75,
            "units": "L/ha",
            "operator_id": "operator-17"
        }
    });

    let chemical_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(chemical_payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(chemical_response.status(), StatusCode::OK);
    let body = to_bytes(chemical_response.into_body(), 64 * 1024).await?;
    let chemical_created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        chemical_created
            .pointer("/payload/product")
            .and_then(|value| value.as_str()),
        Some("Example Herbicide")
    );
    assert_eq!(
        chemical_created
            .pointer("/payload/geometry/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        chemical_created
            .pointer("/payload/rate")
            .and_then(|value| value.as_f64()),
        Some(1.75)
    );

    let invalid_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "record_id": "chem-app-invalid",
                        "record_type": "chemical_application",
                        "org_id": "org-alpha",
                        "field_id": "field-north",
                        "actor": "operator-17",
                        "provenance_ref": "provenance:application/chem-app-invalid/v1",
                        "payload": {
                            "application_id": "chem-app-invalid",
                            "product": " ",
                            "epa_or_label_ref": "EPA-12345-LBL",
                            "field_id": "field-north",
                            "geometry": {
                                "crs": "EPSG:4326",
                                "coordinates": [
                                    { "longitude": -96.70, "latitude": 41.10 },
                                    { "longitude": -96.20, "latitude": 41.10 },
                                    { "longitude": -96.20, "latitude": 41.40 },
                                    { "longitude": -96.70, "latitude": 41.40 },
                                    { "longitude": -96.70, "latitude": 41.10 }
                                ]
                            },
                            "applied_at": "2026-06-12T13:00:00Z",
                            "rate": 1.75,
                            "units": "L/ha",
                            "operator_id": "operator-17"
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(invalid_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("product cannot be empty"));

    let payload_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM compliance_records WHERE payload_json IS NOT NULL",
    )
    .fetch_one(&ctx.pool)
    .await?;
    assert_eq!(payload_rows, 2);

    let rejected_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM compliance_records WHERE record_id = ?1")
            .bind("chem-app-invalid")
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(rejected_rows, 0);

    Ok(())
}

#[tokio::test]
async fn compliance_audit_report_export_includes_records_and_provenance() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_compliance_field(&ctx, "field-north", "org-alpha").await?;
    seed_compliance_export_records(&ctx).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/reports/export")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "report_id": "report-field-north",
                        "org_id": "org-alpha",
                        "field_id": "field-north",
                        "generated_at": "2026-06-13T12:00:00Z",
                        "mandatory_record_types": [
                            "remote_id_log",
                            "chemical_application",
                            "operator_certification",
                            "authorization_decision"
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 128 * 1024).await?;
    let report: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        report
            .get("schema_version")
            .and_then(|value| value.as_str()),
        Some("compliance.audit_report.v1")
    );
    assert_eq!(
        report.get("record_count").and_then(|value| value.as_u64()),
        Some(4)
    );
    assert_eq!(
        report
            .pointer("/record_type_counts/remote_id_log")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert!(report
        .get("provenance_refs")
        .and_then(|value| value.as_array())
        .expect("report should include provenance refs")
        .iter()
        .any(|value| value == "provenance:application/chem-app-1/v1"));
    assert_eq!(
        report
            .pointer("/records/0/org_id")
            .and_then(|value| value.as_str()),
        Some("org-alpha")
    );

    Ok(())
}

#[tokio::test]
async fn compliance_audit_report_export_rejects_missing_mandatory_records() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    seed_compliance_field(&ctx, "field-north", "org-alpha").await?;
    post_compliance_record(
        &ctx,
        json!({
            "record_id": "remote-log-1",
            "record_type": "remote_id_log",
            "org_id": "org-alpha",
            "field_id": "field-north",
            "actor": "operator-17",
            "provenance_ref": "provenance:remote-id/remote-log-1/v1",
            "payload": remote_id_payload()
        }),
    )
    .await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/reports/export")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "report_id": "report-field-north",
                        "org_id": "org-alpha",
                        "field_id": "field-north",
                        "generated_at": "2026-06-13T12:00:00Z",
                        "mandatory_record_types": [
                            "remote_id_log",
                            "chemical_application"
                        ]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("missing mandatory compliance records: chemical_application"));

    Ok(())
}

#[tokio::test]
async fn compliance_airspace_zones_ingest_query_and_reject_invalid_crs() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let zone_payload = json!({
        "zone_id": "nfz-1",
        "zone_class": "no_fly",
        "crs": "EPSG:4326",
        "coordinates": [
            { "longitude": -96.70, "latitude": 41.10 },
            { "longitude": -96.20, "latitude": 41.10 },
            { "longitude": -96.20, "latitude": 41.40 },
            { "longitude": -96.70, "latitude": 41.40 },
            { "longitude": -96.70, "latitude": 41.10 }
        ],
        "effective_from": "2026-06-01T00:00:00Z",
        "effective_to": "2026-07-01T00:00:00Z",
        "source": "faa-uasfm-2026-06"
    });

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/airspace-zones")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(zone_payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);
    let body = to_bytes(create_response.into_body(), 64 * 1024).await?;
    let created: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created.get("zone_id").and_then(|value| value.as_str()),
        Some("nfz-1")
    );
    assert_eq!(
        created.get("crs").and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        created
            .pointer("/extent/min_lon")
            .and_then(|value| value.as_f64()),
        Some(-96.70)
    );
    assert_eq!(
        created
            .pointer("/extent/max_lat")
            .and_then(|value| value.as_f64()),
        Some(41.40)
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/compliance/airspace-zones?zone_id=nfz-1")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].get("source").and_then(|value| value.as_str()),
        Some("faa-uasfm-2026-06")
    );

    let query_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/compliance/airspace-zones/query?longitude=-96.45&latitude=41.20")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(query_response.status(), StatusCode::OK);
    let body = to_bytes(query_response.into_body(), 64 * 1024).await?;
    let containing_zones: Vec<serde_json::Value> = serde_json::from_slice(&body)?;
    assert_eq!(containing_zones.len(), 1);
    assert_eq!(
        containing_zones[0]
            .get("zone_id")
            .and_then(|value| value.as_str()),
        Some("nfz-1")
    );

    let invalid_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/airspace-zones")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "zone_id": "nfz-invalid",
                        "zone_class": "no_fly",
                        "crs": "EPSG:3857",
                        "coordinates": [
                            { "longitude": -96.70, "latitude": 41.10 },
                            { "longitude": -96.20, "latitude": 41.10 },
                            { "longitude": -96.20, "latitude": 41.40 },
                            { "longitude": -96.70, "latitude": 41.40 },
                            { "longitude": -96.70, "latitude": 41.10 }
                        ],
                        "effective_from": "2026-06-01T00:00:00Z",
                        "source": "bad-crs-fixture"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(invalid_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("CRS"));

    let zone_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM compliance_airspace_zones")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(zone_count, 1);

    Ok(())
}

#[tokio::test]
async fn create_field_rejects_orphan_farm_reference() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "missing-farm",
                        "field_id": "orphan-field",
                        "name": "Orphan Field",
                        "boundary": {
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("farm missing-farm does not exist"));

    Ok(())
}

#[tokio::test]
async fn farm_field_scene_relationships_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let create_farm = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-scene",
                        "name": "South Block"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_farm.status(), StatusCode::OK);

    let create_field = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-scene",
                        "field_id": "field-scene",
                        "name": "South Pivot",
                        "season": "2026",
                        "boundary": {
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_field.status(), StatusCode::OK);

    let scene_id = "farm-scene-1";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
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
    .bind(
        json!({
            "metadata": {
                "timestamp": "2025-01-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 1,
                "height": 1,
                "spatial_ref": {
                    "georeferenced": true,
                    "crs": "EPSG:4326",
                    "bbox": {
                        "min_lon": -96.8,
                        "min_lat": 41.0,
                        "max_lon": -96.1,
                        "max_lat": 41.5
                    }
                }
            },
            "file_paths": {
                "B4": "B4.png",
                "B5": "B5.png"
            },
            "image_id": Uuid::new_v4()
        })
        .to_string(),
    )
    .bind(None::<f64>)
    .bind("2025-01-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    let link_scene = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scenes/{scene_id}/field/field-scene"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(link_scene.status(), StatusCode::OK);

    let field_scenes = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/field-scene/scenes")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(field_scenes.status(), StatusCode::OK);
    let body = to_bytes(field_scenes.into_body(), 64 * 1024).await?;
    let scenes_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(scenes_json.as_array().map(|items| items.len()), Some(1));

    let history_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/farms/farm-scene/fields/history")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(history_response.status(), StatusCode::OK);
    let body = to_bytes(history_response.into_body(), 64 * 1024).await?;
    let history_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        history_json
            .pointer("/0/fields/0/field_id")
            .and_then(|value| value.as_str()),
        Some("field-scene")
    );

    Ok(())
}

#[tokio::test]
async fn acceptance_import_field_link_scene_and_load_layer() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let fixture = setup_golden_acceptance_fixture(&ctx, &tmp).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{}/products/ndvi", fixture.scene_id))
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
            .and_then(|value| value.to_str().ok()),
        Some("image/png")
    );
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    assert_eq!(body.as_ref(), TEST_PNG_BYTES);

    Ok(())
}

#[tokio::test]
async fn acceptance_annotation_lifecycle_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let fixture = setup_golden_acceptance_fixture(&ctx, &tmp).await?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{}/annotations", fixture.scene_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "accept-ann-1",
                        "label": "Stress patch",
                        "severity": "medium",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let update_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/scenes/{}/annotations/accept-ann-1",
                    fixture.scene_id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "label": "Stress polygon",
                        "note": "Expanded after review",
                        "severity": "high",
                        "geometry": {
                            "type": "polygon",
                            "coordinates": [
                                { "longitude": -96.46, "latitude": 41.24 },
                                { "longitude": -96.44, "latitude": 41.24 },
                                { "longitude": -96.44, "latitude": 41.26 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(update_response.status(), StatusCode::OK);

    let delete_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/scenes/{}/annotations/accept-ann-1",
                    fixture.scene_id
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    Ok(())
}

#[tokio::test]
async fn acceptance_create_recommendation_from_annotation() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let fixture = setup_golden_acceptance_fixture(&ctx, &tmp).await?;
    create_acceptance_annotation(&ctx, &fixture.scene_id).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{}/recommendations", fixture.scene_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recommendation_id": "accept-rec-1",
                        "title": "Inspect irrigation lane",
                        "category": "irrigation",
                        "priority": "high",
                        "status": "open",
                        "annotation_ids": ["accept-ann-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let recommendation_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        recommendation_json
            .pointer("/annotation_ids/0")
            .and_then(|value| value.as_str()),
        Some("accept-ann-1")
    );

    Ok(())
}

#[tokio::test]
async fn acceptance_generate_and_retrieve_report() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let fixture = setup_golden_acceptance_fixture(&ctx, &tmp).await?;
    create_acceptance_annotation(&ctx, &fixture.scene_id).await?;

    let recommendation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{}/recommendations", fixture.scene_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Check soil moisture",
                        "category": "irrigation",
                        "priority": "medium",
                        "status": "open",
                        "evidence_refs": ["finding:09:retracted-zone"],
                        "annotation_ids": ["accept-ann-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(recommendation_response.status(), StatusCode::OK);

    let generate_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{}/reports", fixture.scene_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Acceptance report"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(generate_response.status(), StatusCode::OK);
    let body = to_bytes(generate_response.into_body(), 128 * 1024).await?;
    let report_json: serde_json::Value = serde_json::from_slice(&body)?;
    let report_id = report_json
        .get("report_id")
        .and_then(|value| value.as_str())
        .expect("report_id should exist");

    let download_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{}/reports/{}",
                    fixture.scene_id, report_id
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(download_response.status(), StatusCode::OK);

    let lineage_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{}/reports/{}/lineage",
                    fixture.scene_id, report_id
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(lineage_response.status(), StatusCode::OK);
    let body = to_bytes(lineage_response.into_body(), 128 * 1024).await?;
    let lineage_json: serde_json::Value = serde_json::from_slice(&body)?;
    let artifact_ids = lineage_json
        .get("records")
        .and_then(|value| value.as_array())
        .expect("lineage records should exist")
        .iter()
        .filter_map(|record| record.get("artifact_id").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();
    assert!(artifact_ids
        .iter()
        .any(|artifact_id| *artifact_id == format!("report:{report_id}")));
    assert!(artifact_ids
        .iter()
        .any(|artifact_id| artifact_id.starts_with("scene:")));
    assert!(artifact_ids.contains(&"annotation:accept-ann-1"));
    assert!(artifact_ids
        .iter()
        .any(|artifact_id| artifact_id.starts_with("recommendation:")));
    assert_eq!(
        lineage_json
            .pointer("/gaps/0/missing_artifact_id")
            .and_then(|value| value.as_str()),
        Some("finding:09:retracted-zone")
    );

    Ok(())
}

#[tokio::test]
async fn acceptance_geojson_export_returns_expected_field_geometry() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let fixture = setup_golden_acceptance_fixture(&ctx, &tmp).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/fields/export/geojson")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    let features = geojson
        .get("features")
        .and_then(|value| value.as_array())
        .expect("feature collection should contain features");
    let feature = features
        .iter()
        .find(|feature| {
            feature.get("id").and_then(|value| value.as_str()) == Some(&fixture.field_id)
        })
        .expect("fixture field should be exported");
    assert_eq!(
        feature
            .pointer("/geometry/coordinates/0")
            .and_then(|value| value.as_array())
            .map(|ring| ring.len()),
        Some(5)
    );

    Ok(())
}

#[tokio::test]
async fn create_and_list_scene_annotations_for_file_backed_scene() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "annotated-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "label": "Water stress",
                        "note": "North corner looks stressed",
                        "severity": "high",
                        "author": "operator-1",
                        "crs": "EPSG:4326",
                        "audit_id": "audit-ann-1",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let body = to_bytes(create_response.into_body(), 64 * 1024).await?;
    let created_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created_json.get("label").and_then(|v| v.as_str()),
        Some("Water stress")
    );
    assert_eq!(
        created_json
            .pointer("/geometry/type")
            .and_then(|v| v.as_str()),
        Some("point")
    );
    assert_eq!(
        created_json.get("author").and_then(|v| v.as_str()),
        Some("operator-1")
    );
    assert_eq!(
        created_json.get("crs").and_then(|v| v.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        created_json.get("audit_id").and_then(|v| v.as_str()),
        Some("audit-ann-1")
    );

    let list_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);

    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let list_json: serde_json::Value = serde_json::from_slice(&body)?;
    let items = list_json
        .as_array()
        .expect("annotations should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].get("scene_id").and_then(|v| v.as_str()),
        Some(scene_id)
    );
    assert_eq!(
        items[0].pointer("/note").and_then(|v| v.as_str()),
        Some("North corner looks stressed")
    );
    assert_eq!(
        items[0].get("author").and_then(|v| v.as_str()),
        Some("operator-1")
    );
    assert_eq!(
        items[0].get("crs").and_then(|v| v.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        items[0].get("audit_id").and_then(|v| v.as_str()),
        Some("audit-ann-1")
    );

    Ok(())
}

#[tokio::test]
async fn create_annotation_rejects_missing_scene() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scenes/nope/annotations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "label": "Missing scene",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn update_and_delete_scene_annotation_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "annotation-update-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "ann-update-1",
                        "label": "Initial point",
                        "severity": "low",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let update_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scenes/{scene_id}/annotations/ann-update-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "label": "Updated polygon",
                        "note": "Expanded to polygon",
                        "severity": "medium",
                        "geometry": {
                            "type": "polygon",
                            "coordinates": [
                                { "longitude": -96.46, "latitude": 41.24 },
                                { "longitude": -96.44, "latitude": 41.24 },
                                { "longitude": -96.44, "latitude": 41.26 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(update_response.status(), StatusCode::OK);

    let body = to_bytes(update_response.into_body(), 64 * 1024).await?;
    let updated_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        updated_json.get("label").and_then(|v| v.as_str()),
        Some("Updated polygon")
    );
    assert_eq!(
        updated_json
            .pointer("/geometry/type")
            .and_then(|v| v.as_str()),
        Some("polygon")
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let list_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(list_json.as_array().map(|items| items.len()), Some(1));
    assert_eq!(
        list_json.pointer("/0/severity").and_then(|v| v.as_str()),
        Some("medium")
    );

    let delete_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/scenes/{scene_id}/annotations/ann-update-1"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let list_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let list_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(list_json.as_array().map(|items| items.len()), Some(0));

    Ok(())
}

#[tokio::test]
async fn recommendation_crud_roundtrip_with_annotation_linkage() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "recommendation-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let annotation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "ann-rec-1",
                        "label": "Stress zone",
                        "severity": "high",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(annotation_response.status(), StatusCode::OK);

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recommendation_id": "rec-1",
                        "title": "Scout irrigation line",
                        "note": "Verify nozzle coverage",
                        "category": "irrigation",
                        "priority": "high",
                        "status": "open",
                        "annotation_ids": ["ann-rec-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);
    let body = to_bytes(create_response.into_body(), 64 * 1024).await?;
    let created_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        created_json.get("title").and_then(|v| v.as_str()),
        Some("Scout irrigation line")
    );
    assert_eq!(
        created_json.get("priority").and_then(|v| v.as_str()),
        Some("high")
    );
    assert_eq!(
        created_json
            .pointer("/annotation_ids/0")
            .and_then(|v| v.as_str()),
        Some("ann-rec-1")
    );

    let empty_evidence_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recommendation_id": "rec-no-evidence",
                        "title": "Scout without evidence",
                        "category": "irrigation",
                        "priority": "medium",
                        "status": "open",
                        "annotation_ids": []
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(empty_evidence_response.status(), StatusCode::BAD_REQUEST);

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 64 * 1024).await?;
    let list_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(list_json.as_array().map(|items| items.len()), Some(1));

    let get_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/recommendations/rec-1"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(get_response.status(), StatusCode::OK);

    let update_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scenes/{scene_id}/recommendations/rec-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Close irrigation gap",
                        "note": "Action assigned to operator",
                        "category": "irrigation",
                        "priority": "critical",
                        "status": "reviewed",
                        "annotation_ids": ["ann-rec-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(update_response.status(), StatusCode::OK);
    let body = to_bytes(update_response.into_body(), 64 * 1024).await?;
    let updated_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        updated_json.get("status").and_then(|v| v.as_str()),
        Some("reviewed")
    );
    assert_eq!(
        updated_json.get("priority").and_then(|v| v.as_str()),
        Some("critical")
    );

    let delete_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/scenes/{scene_id}/recommendations/rec-1"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    Ok(())
}

#[tokio::test]
async fn recommendation_creation_rejects_dangling_annotation_reference() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "recommendation-dangling-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let annotation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "ann-rec-stale-1",
                        "label": "Stress zone",
                        "severity": "high",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(annotation_response.status(), StatusCode::OK);

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recommendation_id": "rec-stale-1",
                        "title": "Scout irrigation line",
                        "note": "Verify nozzle coverage",
                        "category": "irrigation",
                        "priority": "high",
                        "status": "open",
                        "annotation_ids": ["ann-rec-stale-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let delete_annotation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/scenes/{scene_id}/annotations/ann-rec-stale-1"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(delete_annotation_response.status(), StatusCode::NO_CONTENT);

    let dangling_create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recommendation_id": "rec-stale-2",
                        "title": "Missing annotation recommendation",
                        "category": "irrigation",
                        "priority": "medium",
                        "status": "open",
                        "annotation_ids": ["ann-rec-stale-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(dangling_create_response.status(), StatusCode::BAD_REQUEST);
    let dangling_create_body = to_bytes(dangling_create_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&dangling_create_body).contains("does not exist on this scene"));

    let dangling_update_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/scenes/{scene_id}/recommendations/rec-stale-1"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Close irrigation gap",
                        "note": "Action assigned to operator",
                        "category": "irrigation",
                        "priority": "critical",
                        "status": "reviewed",
                        "annotation_ids": ["ann-rec-stale-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(dangling_update_response.status(), StatusCode::BAD_REQUEST);
    let dangling_update_body = to_bytes(dangling_update_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&dangling_update_body).contains("does not exist on this scene"));

    Ok(())
}

#[tokio::test]
async fn mobile_spa_serves_search_analyze_ui_with_error_surface() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/app")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 256 * 1024).await?;
    let html = String::from_utf8_lossy(&body);

    assert!(html.contains("/api/mobile/scenes/search"));
    assert!(html.contains("/api/mobile/analyze"));
    assert!(html.contains("renderSceneList(state.scenes)"));
    assert!(html.contains("scene.cloud_cover"));
    assert!(html.contains("formatSceneDate(scene.acquired_at)"));
    assert!(html.contains("setStatus(error.message || \"Scene search failed.\", \"error\")"));
    assert!(html.contains("setStatus(error.message || \"Analysis failed.\", \"error\")"));
    assert!(html.contains("Server response is not valid JSON."));

    Ok(())
}

#[tokio::test]
async fn mobile_sample_endpoints_return_products_and_handle_invalid_inputs() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let search_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mobile/scenes/search")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "latitude": 36.7783,
                        "longitude": -119.4179,
                        "date": "2026-06-14",
                        "days": 14,
                        "source": "sample"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(search_response.status(), StatusCode::OK);
    let search_body = to_bytes(search_response.into_body(), 64 * 1024).await?;
    let search_json: serde_json::Value = serde_json::from_slice(&search_body)?;
    assert_eq!(
        search_json.pointer("/search_days").and_then(|v| v.as_u64()),
        Some(14)
    );
    assert_eq!(
        search_json
            .get("scenes")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(0)
    );

    let analyze_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mobile/analyze")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "latitude": 36.7783,
                        "longitude": -119.4179,
                        "date": "2026-06-14",
                        "days": 14,
                        "source": "sample",
                        "products": ["ndvi", "ndmi", "nbr", "mndwi", "evi2"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    let analyze_status = analyze_response.status();
    let analyze_body = to_bytes(analyze_response.into_body(), 64 * 1024).await?;
    assert_eq!(
        analyze_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&analyze_body)
    );
    let analyze_json: serde_json::Value = serde_json::from_slice(&analyze_body)?;
    assert_eq!(
        analyze_json.get("acquired_at").and_then(|v| v.as_str()),
        Some("2026-06-14")
    );
    assert_eq!(
        analyze_json
            .get("real_products_ready")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        analyze_json.get("source").and_then(|v| v.as_str()),
        Some("backend-generated Landsat-style sample selected by user")
    );
    let products = analyze_json
        .get("products")
        .and_then(|v| v.as_array())
        .expect("products should be an array");
    assert!(products.len() >= 6);
    assert_eq!(
        products[0].get("kind").and_then(|v| v.as_str()),
        Some("rgb")
    );

    let invalid_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mobile/analyze")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "latitude": 999,
                        "longitude": -119.4179,
                        "date": "2026-06-14",
                        "days": 14,
                        "source": "sample"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    let invalid_body = to_bytes(invalid_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&invalid_body)
        .contains("latitude or longitude outside valid range"));

    let mismatch_payload = json!({
        "latitude": 36.7783,
        "longitude": -119.4179,
        "date": "2026-06-14",
        "days": 14,
        "source": "landsat",
        "external_scene_id": "selected-scene-id",
        "selected_scene": {
            "external_scene_id": "other-scene-id",
            "dataset": "landsat",
            "dataset_label": "landsat",
            "provider": "mock-provider",
            "collection": "mock-collection",
            "acquired_at": "2026-06-14T00:00:00Z",
            "resolution_m": 30.0,
            "bbox": null,
            "asset_count": 0
        }
    });
    let mismatch_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mobile/analyze")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(mismatch_payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(mismatch_response.status(), StatusCode::BAD_REQUEST);
    let mismatch_body = to_bytes(mismatch_response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&mismatch_body)
        .contains("selected scene payload does not match selected scene id"));

    Ok(())
}

#[tokio::test]
async fn annotation_exports_return_csv_and_geojson() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "annotation-export-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let create_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "ann-export-1",
                        "field_id": "field-alpha",
                        "author": "agronomist@example.com",
                        "crs": "EPSG:4326",
                        "audit_id": "audit-ann-export-1",
                        "label": "Stress pocket",
                        "note": "West edge looks dry",
                        "severity": "high",
                        "geometry": {
                            "type": "polygon",
                            "coordinates": [
                                { "longitude": -96.46, "latitude": 41.24 },
                                { "longitude": -96.44, "latitude": 41.24 },
                                { "longitude": -96.44, "latitude": 41.26 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_response.status(), StatusCode::OK);

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/exports/annotations.csv"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    assert_eq!(
        csv_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/csv; charset=utf-8")
    );
    assert_eq!(
        csv_response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok()),
        Some("attachment; filename=\"annotations.csv\"")
    );
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let csv_text = String::from_utf8(body.to_vec())?;
    let mut csv_reader = csv::Reader::from_reader(csv_text.as_bytes());
    assert_eq!(
        csv_reader.headers()?.iter().collect::<Vec<_>>(),
        vec![
            "annotation_id",
            "scene_id",
            "field_id",
            "author",
            "crs",
            "audit_id",
            "label",
            "severity",
            "note",
            "geometry_type",
            "geometry_json",
            "created_at",
            "updated_at"
        ]
    );
    let rows = csv_reader.records().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get(0), Some("ann-export-1"));
    assert_eq!(row.get(1), Some(scene_id));
    assert_eq!(row.get(2), Some("field-alpha"));
    assert_eq!(row.get(3), Some("agronomist@example.com"));
    assert_eq!(row.get(4), Some("EPSG:4326"));
    assert_eq!(row.get(5), Some("audit-ann-export-1"));
    assert_eq!(row.get(6), Some("Stress pocket"));
    assert_eq!(row.get(7), Some("high"));
    assert_eq!(row.get(8), Some("West edge looks dry"));
    assert_eq!(row.get(9), Some("polygon"));
    let geometry_json: serde_json::Value =
        serde_json::from_str(row.get(10).expect("geometry json should be present"))?;
    assert_eq!(
        geometry_json.get("type").and_then(|value| value.as_str()),
        Some("polygon")
    );

    let geojson_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/exports/annotations.geojson"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    assert_eq!(
        geojson_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/geo+json")
    );
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson.get("type").and_then(|value| value.as_str()),
        Some("FeatureCollection")
    );
    assert_eq!(
        geojson
            .pointer("/crs/properties/name")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/id")
            .and_then(|value| value.as_str()),
        Some("ann-export-1")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/geometry/type")
            .and_then(|value| value.as_str()),
        Some("Polygon")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/field_id")
            .and_then(|value| value.as_str()),
        Some("field-alpha")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/author")
            .and_then(|value| value.as_str()),
        Some("agronomist@example.com")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/audit_id")
            .and_then(|value| value.as_str()),
        Some("audit-ann-export-1")
    );

    Ok(())
}

#[tokio::test]
async fn empty_annotation_exports_are_schema_valid() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "empty-annotation-export-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/exports/annotations.csv"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let mut csv_reader = csv::Reader::from_reader(body.as_ref());
    assert_eq!(
        csv_reader.headers()?.iter().collect::<Vec<_>>(),
        vec![
            "annotation_id",
            "scene_id",
            "field_id",
            "author",
            "crs",
            "audit_id",
            "label",
            "severity",
            "note",
            "geometry_type",
            "geometry_json",
            "created_at",
            "updated_at"
        ]
    );
    assert_eq!(csv_reader.records().count(), 0);

    let geojson_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/exports/annotations.geojson"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson.get("type").and_then(|value| value.as_str()),
        Some("FeatureCollection")
    );
    assert_eq!(
        geojson
            .pointer("/crs/properties/name")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        geojson
            .get("features")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(0)
    );

    Ok(())
}

#[tokio::test]
async fn recommendation_exports_return_csv_and_geojson() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "recommendation-export-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let annotation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": "field-alpha",
                        "crs": "EPSG:4326",
                        "annotation_id": "ann-export-rec-1",
                        "label": "Irrigation gap",
                        "severity": "medium",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(annotation_response.status(), StatusCode::OK);

    let recommendation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recommendation_id": "rec-export-1",
                        "title": "Inspect irrigation line",
                        "note": "Dispatch operator before noon",
                        "category": "irrigation",
                        "priority": "critical",
                        "status": "open",
                        "annotation_ids": ["ann-export-rec-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(recommendation_response.status(), StatusCode::OK);

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/exports/recommendations.csv"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    assert_eq!(
        csv_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/csv; charset=utf-8")
    );
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let csv_text = String::from_utf8(body.to_vec())?;
    let mut csv_reader = csv::Reader::from_reader(csv_text.as_bytes());
    assert_eq!(
        csv_reader.headers()?.iter().collect::<Vec<_>>(),
        vec![
            "recommendation_id",
            "scene_id",
            "field_id",
            "org_id",
            "author_user_id",
            "title",
            "category",
            "action_category",
            "priority",
            "status",
            "evidence_refs",
            "annotation_ids",
            "note",
            "created_at",
            "updated_at"
        ]
    );
    let rows = csv_reader.records().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get(0), Some("rec-export-1"));
    assert_eq!(row.get(1), Some(scene_id));
    assert_eq!(row.get(2), Some("field-alpha"));
    assert_eq!(row.get(5), Some("Inspect irrigation line"));
    assert_eq!(row.get(6), Some("irrigation"));
    assert_eq!(row.get(7), Some("irrigation"));
    assert_eq!(row.get(8), Some("critical"));
    assert_eq!(row.get(9), Some("open"));
    assert_eq!(row.get(10), Some("annotation:ann-export-rec-1"));
    assert_eq!(row.get(11), Some("ann-export-rec-1"));
    assert_eq!(row.get(12), Some("Dispatch operator before noon"));

    let geojson_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/exports/recommendations.geojson"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    assert_eq!(
        geojson_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/geo+json")
    );
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson.get("type").and_then(|value| value.as_str()),
        Some("FeatureCollection")
    );
    assert_eq!(
        geojson
            .pointer("/crs/properties/name")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/recommendation_id")
            .and_then(|value| value.as_str()),
        Some("rec-export-1")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/annotation_id")
            .and_then(|value| value.as_str()),
        Some("ann-export-rec-1")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/geometry/type")
            .and_then(|value| value.as_str()),
        Some("Point")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/field_id")
            .and_then(|value| value.as_str()),
        Some("field-alpha")
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/evidence_refs/0")
            .and_then(|value| value.as_str()),
        Some("annotation:ann-export-rec-1")
    );

    Ok(())
}

#[tokio::test]
async fn empty_recommendation_exports_are_schema_valid() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "empty-recommendation-export-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/exports/recommendations.csv"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let mut csv_reader = csv::Reader::from_reader(body.as_ref());
    assert_eq!(
        csv_reader.headers()?.iter().collect::<Vec<_>>(),
        vec![
            "recommendation_id",
            "scene_id",
            "field_id",
            "org_id",
            "author_user_id",
            "title",
            "category",
            "action_category",
            "priority",
            "status",
            "evidence_refs",
            "annotation_ids",
            "note",
            "created_at",
            "updated_at"
        ]
    );
    assert_eq!(csv_reader.records().count(), 0);

    let geojson_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/scenes/{scene_id}/exports/recommendations.geojson"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson.get("type").and_then(|value| value.as_str()),
        Some("FeatureCollection")
    );
    assert_eq!(
        geojson
            .pointer("/crs/properties/name")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        geojson
            .get("features")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(0)
    );

    Ok(())
}

#[tokio::test]
async fn field_record_bundle_exports_csv_and_geojson() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let fixture = setup_golden_acceptance_fixture(&ctx, &tmp).await?;
    create_acceptance_annotation(&ctx, &fixture.scene_id).await?;

    let recommendation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{}/recommendations", fixture.scene_id))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recommendation_id": "field-bundle-rec-1",
                        "title": "Bundle irrigation check",
                        "category": "irrigation",
                        "priority": "high",
                        "status": "open",
                        "annotation_ids": ["accept-ann-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(recommendation_response.status(), StatusCode::OK);

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/fields/{}/exports/records.csv",
                    fixture.field_id
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    assert_eq!(
        csv_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/csv; charset=utf-8")
    );
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let mut csv_reader = csv::Reader::from_reader(body.as_ref());
    assert_eq!(
        csv_reader.headers()?.iter().collect::<Vec<_>>(),
        vec![
            "record_type",
            "record_id",
            "scene_id",
            "field_id",
            "crs",
            "title",
            "label",
            "status",
            "priority",
            "evidence_refs",
            "annotation_ids",
            "geometry_type",
            "geometry_json",
            "created_at",
            "updated_at"
        ]
    );
    let rows = csv_reader.records().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get(0), Some("annotation"));
    assert_eq!(rows[0].get(1), Some("accept-ann-1"));
    assert_eq!(rows[0].get(3), Some(fixture.field_id.as_str()));
    assert_eq!(rows[1].get(0), Some("recommendation"));
    assert_eq!(rows[1].get(1), Some("field-bundle-rec-1"));
    assert_eq!(rows[1].get(5), Some("Bundle irrigation check"));
    assert_eq!(rows[1].get(10), Some("accept-ann-1"));

    let geojson_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/fields/{}/exports/records.geojson",
                    fixture.field_id
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson
            .pointer("/crs/properties/name")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    let features = geojson
        .get("features")
        .and_then(|value| value.as_array())
        .expect("features should exist");
    assert_eq!(features.len(), 3);
    assert!(features.iter().any(|feature| {
        feature
            .pointer("/properties/field_id")
            .and_then(|value| value.as_str())
            == Some(fixture.field_id.as_str())
    }));
    assert!(features.iter().any(|feature| {
        feature
            .pointer("/properties/annotation_id")
            .and_then(|value| value.as_str())
            == Some("accept-ann-1")
    }));
    assert!(features.iter().any(|feature| {
        feature
            .pointer("/properties/recommendation_id")
            .and_then(|value| value.as_str())
            == Some("field-bundle-rec-1")
    }));

    Ok(())
}

#[tokio::test]
async fn empty_field_record_bundle_exports_valid_empty_records() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let fixture = setup_golden_acceptance_fixture(&ctx, &tmp).await?;

    let csv_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/fields/{}/exports/records.csv",
                    fixture.field_id
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(csv_response.status(), StatusCode::OK);
    let body = to_bytes(csv_response.into_body(), 64 * 1024).await?;
    let mut csv_reader = csv::Reader::from_reader(body.as_ref());
    assert_eq!(csv_reader.records().count(), 0);

    let geojson_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/fields/{}/exports/records.geojson",
                    fixture.field_id
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(geojson_response.status(), StatusCode::OK);
    let body = to_bytes(geojson_response.into_body(), 64 * 1024).await?;
    let geojson: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        geojson.get("type").and_then(|value| value.as_str()),
        Some("FeatureCollection")
    );
    assert_eq!(
        geojson
            .get("features")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        geojson
            .pointer("/features/0/properties/field_id")
            .and_then(|value| value.as_str()),
        Some(fixture.field_id.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn layer_geotiff_export_round_trips_spatial_metadata() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "geotiff-export-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    });
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, spatial_ref).await?;
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, scene_id, "ndvi").await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/layers/{scene_id}/ndvi/export/geotiff"))
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
            .and_then(|value| value.to_str().ok()),
        Some("image/tiff")
    );
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let reopened = reopen_raster_geotiff(&body)?;

    assert_eq!(reopened.product_id, "geotiff-export-scene:ndvi");
    assert_eq!(reopened.width, 2);
    assert_eq!(reopened.height, 2);
    assert_eq!(reopened.cells.len(), 4);
    assert_eq!(reopened.spatial_ref.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(
        reopened.spatial_ref.bbox.as_ref().map(|bbox| (
            bbox.min_lon,
            bbox.min_lat,
            bbox.max_lon,
            bbox.max_lat
        )),
        Some((-96.7, 41.1, -96.6, 41.2))
    );

    Ok(())
}

#[tokio::test]
async fn report_generation_and_download_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "report-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let annotation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "ann-report-1",
                        "label": "Dry patch",
                        "severity": "medium",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(annotation_response.status(), StatusCode::OK);

    let recommendation_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/recommendations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Check irrigation pressure",
                        "category": "irrigation",
                        "priority": "high",
                        "status": "open",
                        "annotation_ids": ["ann-report-1"]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(recommendation_response.status(), StatusCode::OK);

    let generate_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/reports"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "North field agronomy report"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(generate_response.status(), StatusCode::OK);
    let body = to_bytes(generate_response.into_body(), 256 * 1024).await?;
    let report_json: serde_json::Value = serde_json::from_slice(&body)?;
    let report_id = report_json
        .get("report_id")
        .and_then(|value| value.as_str())
        .expect("report_id should exist")
        .to_string();
    assert_eq!(
        report_json.get("format").and_then(|value| value.as_str()),
        Some("html")
    );
    assert_eq!(
        report_json
            .get("annotation_count")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        report_json
            .get("recommendation_count")
            .and_then(|value| value.as_u64()),
        Some(1)
    );

    let list_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/reports"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(list_response.status(), StatusCode::OK);

    let download_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}/reports/{report_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(download_response.status(), StatusCode::OK);
    assert_eq!(
        download_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/html; charset=utf-8")
    );
    let body = to_bytes(download_response.into_body(), 256 * 1024).await?;
    let html = String::from_utf8(body.to_vec())?;
    assert!(html.contains("North field agronomy report"));
    assert!(html.contains("Check irrigation pressure"));

    Ok(())
}

#[tokio::test]
async fn shared_report_link_allows_public_access_until_revoked() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "shared-report-scene";
    std::fs::create_dir_all(ctx.data_root.join("scenes").join(scene_id))?;
    let report_id =
        generate_report(&ctx, scene_id, "Shared agronomy report", Some("shared")).await?;

    let share_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/reports/{report_id}/shares"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "expires_at": "2099-01-01T00:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(share_response.status(), StatusCode::OK);
    let body = to_bytes(share_response.into_body(), 64 * 1024).await?;
    let share_json: serde_json::Value = serde_json::from_slice(&body)?;
    let share_token = share_json
        .get("share_token")
        .and_then(|value| value.as_str())
        .expect("share token should exist")
        .to_string();
    let url_path = share_json
        .get("url_path")
        .and_then(|value| value.as_str())
        .expect("url path should exist")
        .to_string();

    let public_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&url_path)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(public_response.status(), StatusCode::OK);
    let body = to_bytes(public_response.into_body(), 256 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("Shared agronomy report"));

    let revoke_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/scenes/{scene_id}/reports/{report_id}/shares/{share_token}"
                ))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(revoke_response.status(), StatusCode::NO_CONTENT);

    let denied_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&url_path)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(denied_response.status(), StatusCode::FORBIDDEN);

    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM report_share_events WHERE share_token = ?1")
            .bind(&share_token)
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(event_count, 3);

    Ok(())
}

#[tokio::test]
async fn shared_report_link_unknown_token_is_rejected() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let denied_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/report-shares/not-a-real-token")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(denied_response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn expired_report_share_link_is_denied() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "expired-report-scene";
    std::fs::create_dir_all(ctx.data_root.join("scenes").join(scene_id))?;
    let report_id = generate_report(&ctx, scene_id, "Expired report", Some("shared")).await?;

    let share_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/reports/{report_id}/shares"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "expires_at": "2000-01-01T00:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(share_response.status(), StatusCode::OK);
    let body = to_bytes(share_response.into_body(), 64 * 1024).await?;
    let share_json: serde_json::Value = serde_json::from_slice(&body)?;
    let url_path = share_json
        .get("url_path")
        .and_then(|value| value.as_str())
        .expect("url path should exist");

    let denied_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(url_path)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(denied_response.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn org_only_report_does_not_produce_public_share_link() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "org-only-report-scene";
    std::fs::create_dir_all(ctx.data_root.join("scenes").join(scene_id))?;
    let report_id = generate_report(&ctx, scene_id, "Org only report", None).await?;

    let share_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/reports/{report_id}/shares"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "expires_at": "2099-01-01T00:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(share_response.status(), StatusCode::BAD_REQUEST);

    let share_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM report_shares")
        .fetch_one(&ctx.pool)
        .await?;
    assert_eq!(share_count, 0);

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
            "height": 256,
            "spatial_ref": {
                "georeferenced": true,
                "crs": "EPSG:4326",
                "bbox": {
                    "min_lon": -74.1,
                    "min_lat": 40.6,
                    "max_lon": -73.9,
                    "max_lat": 40.8
                },
                "geo_transform": [-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]
            }
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
        json.pointer("/geospatial/georeferenced")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        json.pointer("/geospatial/crs").and_then(|v| v.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        json.pointer("/geospatial/center/latitude")
            .and_then(|v| v.as_f64()),
        Some(40.7)
    );
    assert_eq!(
        json.pointer("/geospatial/extent/min_lon")
            .and_then(|v| v.as_f64()),
        Some(-74.1)
    );
    assert_eq!(
        products[0].get("kind").and_then(|v| v.as_str()),
        Some("ndvi")
    );
    assert_eq!(
        products[0].get("url_path").and_then(|v| v.as_str()),
        Some(format!("/api/scenes/{scene_id}/products/ndvi").as_str())
    );
    assert_eq!(
        products[0]
            .get("tile_url_template")
            .and_then(|v| v.as_str()),
        Some(format!("/api/scenes/{scene_id}/products/ndvi/tiles/{{z}}/{{x}}/{{y}}.png").as_str())
    );

    Ok(())
}

#[tokio::test]
async fn scene_detail_returns_persisted_spatial_ref_roundtrip() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "spatial-roundtrip-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    });
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, spatial_ref.clone()).await?;

    let response = ctx
        .app
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
    let scene_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        scene_json
            .pointer("/geospatial/spatial_ref/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        scene_json
            .pointer("/geospatial/spatial_ref/geo_transform/1")
            .and_then(|value| value.as_f64()),
        Some(0.05)
    );
    assert_eq!(
        scene_json
            .pointer("/geospatial/spatial_ref/resolution/x")
            .and_then(|value| value.as_f64()),
        Some(0.05)
    );

    Ok(())
}

#[tokio::test]
async fn list_layers_filters_and_returns_spatial_ref_metadata() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    });
    insert_layer_field(
        &ctx,
        "field-alpha",
        json!({
            "crs": "EPSG:4326",
            "coordinates": [
                { "longitude": -96.7, "latitude": 41.1 },
                { "longitude": -96.6, "latitude": 41.1 },
                { "longitude": -96.6, "latitude": 41.2 },
                { "longitude": -96.7, "latitude": 41.2 },
                { "longitude": -96.7, "latitude": 41.1 }
            ]
        }),
    )
    .await?;

    let first_scene = "layer-scene-older";
    let first_dir = ctx.data_root.join("scenes").join(first_scene);
    std::fs::create_dir_all(&first_dir)?;
    insert_scene_with_spatial_ref(&ctx, first_scene, &first_dir, spatial_ref.clone()).await?;
    link_scene_context(&ctx, first_scene, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, first_scene, "ndvi").await?;
    insert_ingest_source(&ctx, first_scene, "landsat:/older-source").await?;

    let second_scene = "layer-scene-newer";
    let second_dir = ctx.data_root.join("scenes").join(second_scene);
    std::fs::create_dir_all(&second_dir)?;
    insert_scene_with_spatial_ref(&ctx, second_scene, &second_dir, spatial_ref.clone()).await?;
    sqlx::query("UPDATE scenes SET acquired_at = ?1 WHERE scene_id = ?2")
        .bind("2026-05-02T00:00:00Z")
        .bind(second_scene)
        .execute(&ctx.pool)
        .await?;
    link_scene_context(&ctx, second_scene, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, second_scene, "ndvi").await?;
    insert_ingest_source(&ctx, second_scene, "landsat:/newer-source").await?;

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/layers?field_id=field-alpha&season_id=2026&product_kind=ndvi&page=1&page_size=1&stale_after_days=7")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let layers_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        layers_json.get("total").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        layers_json
            .get("page_size")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/scene_id")
            .and_then(|value| value.as_str()),
        Some(second_scene)
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/product_kind")
            .and_then(|value| value.as_str()),
        Some("ndvi")
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/spatial_ref/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/freshness/acquired_at")
            .and_then(|value| value.as_str()),
        Some("2026-05-02T00:00:00Z")
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/freshness/stale_after_days")
            .and_then(|value| value.as_i64()),
        Some(7)
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/freshness/field_coverage_fraction")
            .and_then(|value| value.as_f64()),
        Some(1.0)
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/freshness/field_coverage_status")
            .and_then(|value| value.as_str()),
        Some("full")
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/source")
            .and_then(|value| value.as_str()),
        Some("landsat:/newer-source")
    );

    let date_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/layers?field_id=field-alpha&date=2026-05-01")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(date_response.status(), StatusCode::OK);
    let body = to_bytes(date_response.into_body(), 64 * 1024).await?;
    let date_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        date_json.get("total").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        date_json
            .pointer("/layers/0/scene_id")
            .and_then(|value| value.as_str()),
        Some(first_scene)
    );

    Ok(())
}

#[tokio::test]
async fn list_layers_reports_no_field_coverage_for_non_intersecting_extent() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    insert_layer_field(
        &ctx,
        "field-alpha",
        json!({
            "crs": "EPSG:4326",
            "coordinates": [
                { "longitude": -96.7, "latitude": 41.1 },
                { "longitude": -96.6, "latitude": 41.1 },
                { "longitude": -96.6, "latitude": 41.2 },
                { "longitude": -96.7, "latitude": 41.2 },
                { "longitude": -96.7, "latitude": 41.1 }
            ]
        }),
    )
    .await?;
    let scene_id = "layer-scene-outside-field";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    insert_scene_with_spatial_ref(
        &ctx,
        scene_id,
        &scene_dir,
        json!({
            "georeferenced": true,
            "crs": "EPSG:4326",
            "bbox": {
                "min_lon": -97.7,
                "min_lat": 40.1,
                "max_lon": -97.6,
                "max_lat": 40.2
            },
            "geo_transform": [-97.7, 0.05, 0.0, 40.2, 0.0, -0.05],
            "resolution": {
                "x": 0.05,
                "y": 0.05
            }
        }),
    )
    .await?;
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, scene_id, "ndvi").await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/layers?field_id=field-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let layers_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        layers_json
            .pointer("/layers/0/freshness/field_coverage_fraction")
            .and_then(|value| value.as_f64()),
        Some(0.0)
    );
    assert_eq!(
        layers_json
            .pointer("/layers/0/freshness/field_coverage_status")
            .and_then(|value| value.as_str()),
        Some("no_coverage")
    );

    Ok(())
}

#[tokio::test]
async fn layer_metadata_endpoint_returns_asserted_spatial_ref() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "layer-detail-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    });
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, spatial_ref).await?;
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, scene_id, "ndvi").await?;
    insert_ingest_source(&ctx, scene_id, "landsat:/layer-detail-source").await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/layers/{scene_id}/ndvi"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let layer_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        layer_json.get("layer_id").and_then(|value| value.as_str()),
        Some("layer-detail-scene:ndvi")
    );
    assert_eq!(
        layer_json
            .pointer("/spatial_ref/resolution/x")
            .and_then(|value| value.as_f64()),
        Some(0.05)
    );
    assert_eq!(
        layer_json.get("url_path").and_then(|value| value.as_str()),
        Some("/api/scenes/layer-detail-scene/products/ndvi")
    );
    assert_eq!(
        layer_json.get("dataset").and_then(|value| value.as_str()),
        Some("landsat8")
    );
    assert_eq!(
        layer_json.get("source").and_then(|value| value.as_str()),
        Some("landsat:/layer-detail-source")
    );
    assert_eq!(
        layer_json
            .pointer("/freshness/ingested_at")
            .and_then(|value| value.as_str()),
        Some("2026-05-01T00:00:00Z")
    );

    Ok(())
}

#[tokio::test]
async fn open_data_publish_lists_anonymized_layer_with_license_and_extent() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "open-data-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    });
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, spatial_ref).await?;
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, scene_id, "ndvi").await?;

    let publish = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/layers/{scene_id}/ndvi/open-data"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "license": "CC-BY-4.0",
                        "attribution": "AGBot open data"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(publish.status(), StatusCode::OK);
    let publish_body = to_bytes(publish.into_body(), 64 * 1024).await?;
    let published: serde_json::Value = serde_json::from_slice(&publish_body)?;
    assert_eq!(
        published.get("license").and_then(|value| value.as_str()),
        Some("CC-BY-4.0")
    );
    assert_eq!(
        published
            .pointer("/spatial_ref/bbox/min_lon")
            .and_then(|value| value.as_f64()),
        Some(-96.7)
    );
    assert!(published.get("field_id").is_none());
    assert!(published.get("owner").is_none());

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/open-data/layers")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let catalog: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        catalog
            .pointer("/layers/0/open_data_id")
            .and_then(|value| value.as_str()),
        Some("open-data:open-data-scene:ndvi")
    );
    assert_eq!(
        catalog
            .pointer("/layers/0/license")
            .and_then(|value| value.as_str()),
        Some("CC-BY-4.0")
    );
    assert_eq!(
        catalog
            .pointer("/layers/0/spatial_ref/crs")
            .and_then(|value| value.as_str()),
        Some("EPSG:4326")
    );
    assert!(
        catalog.pointer("/layers/0/field_id").is_none(),
        "catalog must not expose field identifiers"
    );

    Ok(())
}

#[tokio::test]
async fn open_data_publish_refuses_missing_license() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "open-data-missing-license";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, layer_spatial_ref_json()).await?;
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, scene_id, "ndvi").await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/layers/{scene_id}/ndvi/open-data"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "license": " ",
                        "attribution": "AGBot open data"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("missinglicense"));

    Ok(())
}

#[tokio::test]
async fn open_data_publish_refuses_deanonymizing_field_identifier() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "open-data-field-ref";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, layer_spatial_ref_json()).await?;
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, scene_id, "ndvi").await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/layers/{scene_id}/ndvi/open-data"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "license": "CC-BY-4.0",
                        "attribution": "AGBot open data",
                        "field_identifier": "field-alpha"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("fieldidentifierpresent"));

    Ok(())
}

#[tokio::test]
async fn list_layers_excludes_spatial_ref_integrity_mismatch() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "invalid-layer-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let metadata_spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    });
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, metadata_spatial_ref).await?;
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;
    insert_layer_product(&ctx, scene_id, "ndvi").await?;

    let corrupted_spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.58,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.06, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.06,
            "y": 0.05
        }
    });
    upsert_scene_spatial_ref(&ctx, scene_id, corrupted_spatial_ref).await?;

    let response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/layers?field_id=field-alpha")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let layers_json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(
        layers_json.get("total").and_then(|value| value.as_u64()),
        Some(0)
    );
    assert_eq!(
        layers_json
            .get("layers")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(0)
    );

    Ok(())
}

#[tokio::test]
async fn product_request_rejects_spatial_ref_integrity_mismatch() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;
    let scene_id = "spatial-integrity-scene";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    let product_path = scene_dir.join("products").join("ndvi").join("sample.png");
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    std::fs::write(&product_path, TEST_PNG_BYTES)?;
    let metadata_spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    });
    insert_scene_with_spatial_ref(&ctx, scene_id, &scene_dir, metadata_spatial_ref).await?;

    let corrupted_spatial_ref = json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.58,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.06, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.06,
            "y": 0.05
        }
    });
    upsert_scene_spatial_ref(&ctx, scene_id, corrupted_spatial_ref).await?;

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
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let message = String::from_utf8(body.to_vec())?;
    assert!(message.contains("metadata-integrity"));

    Ok(())
}

#[tokio::test]
async fn generates_ndvi_via_db_fallback_and_persists_product_provenance() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let scene_id = "scene_from_db";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let b4_path = scene_dir.join("B4.png");
    let b5_path = scene_dir.join("B5.png");
    let b6_path = scene_dir.join("B6.png");
    write_gray_png(&b4_path, 40)?;
    write_gray_png(&b5_path, 140)?;
    write_gray_png(&b6_path, 90)?;

    let image_id = Uuid::new_v4();
    let metadata_json = json!({
        "metadata": {
            "timestamp": "2025-01-01T00:00:00Z",
            "gps_position": null,
            "bands": ["B4", "B5", "B6"],
            "exposure_time": 1.0,
            "gain": 1.0,
            "width": 2,
            "height": 2,
            "spatial_ref": {
                "georeferenced": true,
                "crs": "LOCAL_TEST",
                "bbox": {
                    "min_lon": 0.0,
                    "min_lat": 0.0,
                    "max_lon": 2.0,
                    "max_lat": 2.0
                },
                "geo_transform": [0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                "resolution": {
                    "x": 1.0,
                    "y": 1.0
                }
            }
        },
        "file_paths": {
            "B4": b4_path.to_string_lossy().to_string(),
            "B5": b5_path.to_string_lossy().to_string(),
            "B6": b6_path.to_string_lossy().to_string()
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
    link_scene_context(&ctx, scene_id, "field-alpha", "2026").await?;

    let response = ctx
        .app
        .clone()
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

    let row = sqlx::query(
        r#"
        SELECT
            product_id, field_id, season_id, spatial_ref_json,
            source_image_ids_json, source_scan_ids_json, path
        FROM products
        WHERE scene_id = ?1 AND kind = ?2
        "#,
    )
    .bind(scene_id)
    .bind("ndvi")
    .fetch_one(&ctx.pool)
    .await?;
    let product_path: String = row.get("path");
    assert!(Path::new(&product_path).exists());
    assert!(product_path.ends_with(".png"));
    assert_eq!(
        row.get::<String, _>("product_id"),
        "scene_from_db:ndvi".to_string()
    );
    assert_eq!(
        row.get::<Option<String>, _>("field_id").as_deref(),
        Some("field-alpha")
    );
    assert_eq!(
        row.get::<Option<String>, _>("season_id").as_deref(),
        Some("2026")
    );
    let persisted_spatial_ref: serde_json::Value =
        serde_json::from_str(&row.get::<String, _>("spatial_ref_json"))?;
    assert_eq!(
        persisted_spatial_ref
            .pointer("/bbox/max_lon")
            .and_then(|value| value.as_f64()),
        Some(2.0)
    );
    let source_image_ids: Vec<String> =
        serde_json::from_str(&row.get::<String, _>("source_image_ids_json"))?;
    assert_eq!(source_image_ids, vec![image_id.to_string()]);
    let source_scan_ids: Vec<String> =
        serde_json::from_str(&row.get::<String, _>("source_scan_ids_json"))?;
    assert!(source_scan_ids.is_empty());

    let scene_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scenes/{scene_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(scene_response.status(), StatusCode::OK);
    let scene_body = to_bytes(scene_response.into_body(), 64 * 1024).await?;
    let scene_json: serde_json::Value = serde_json::from_slice(&scene_body)?;
    assert_eq!(
        scene_json
            .pointer("/available_products/0/product_id")
            .and_then(|value| value.as_str()),
        Some("scene_from_db:ndvi")
    );
    assert_eq!(
        scene_json
            .pointer("/available_products/0/field_id")
            .and_then(|value| value.as_str()),
        Some("field-alpha")
    );
    assert_eq!(
        scene_json
            .pointer("/available_products/0/season_id")
            .and_then(|value| value.as_str()),
        Some("2026")
    );
    assert_eq!(
        scene_json
            .pointer("/available_products/0/source_image_ids/0")
            .and_then(|value| value.as_str()),
        Some(image_id.to_string().as_str())
    );
    assert_eq!(
        scene_json
            .pointer("/available_products/0/source_scan_ids")
            .and_then(|value| value.as_array())
            .map(|scan_ids| scan_ids.len()),
        Some(0)
    );

    let layer_response = ctx
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/layers/{scene_id}/ndvi"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(layer_response.status(), StatusCode::OK);
    let layer_body = to_bytes(layer_response.into_body(), 64 * 1024).await?;
    let layer_json: serde_json::Value = serde_json::from_slice(&layer_body)?;
    assert_eq!(
        layer_json
            .get("product_id")
            .and_then(|value| value.as_str()),
        Some("scene_from_db:ndvi")
    );
    assert_eq!(
        layer_json
            .pointer("/source_image_ids/0")
            .and_then(|value| value.as_str()),
        Some(image_id.to_string().as_str())
    );
    assert_eq!(
        layer_json
            .pointer("/source_scan_ids")
            .and_then(|value| value.as_array())
            .map(|scan_ids| scan_ids.len()),
        Some(0)
    );

    Ok(())
}

#[tokio::test]
async fn unlinked_scene_product_publish_is_rejected_without_orphan_row() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = test_app(&tmp).await?;

    let scene_id = "scene_unlinked";
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;

    let b4_path = scene_dir.join("B4.png");
    let b5_path = scene_dir.join("B5.png");
    let b6_path = scene_dir.join("B6.png");
    write_gray_png(&b4_path, 40)?;
    write_gray_png(&b5_path, 140)?;
    write_gray_png(&b6_path, 90)?;

    let metadata_json = json!({
        "metadata": {
            "timestamp": "2025-01-01T00:00:00Z",
            "gps_position": null,
            "bands": ["B4", "B5", "B6"],
            "exposure_time": 1.0,
            "gain": 1.0,
            "width": 2,
            "height": 2,
            "spatial_ref": {
                "georeferenced": true,
                "crs": "LOCAL_TEST",
                "bbox": {
                    "min_lon": 0.0,
                    "min_lat": 0.0,
                    "max_lon": 2.0,
                    "max_lat": 2.0
                },
                "geo_transform": [0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                "resolution": {
                    "x": 1.0,
                    "y": 1.0
                }
            }
        },
        "file_paths": {
            "B4": b4_path.to_string_lossy().to_string(),
            "B5": b5_path.to_string_lossy().to_string(),
            "B6": b6_path.to_string_lossy().to_string()
        },
        "image_id": Uuid::new_v4()
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("unlinked scene"));

    let product_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE scene_id = ?1")
            .bind(scene_id)
            .fetch_one(&ctx.pool)
            .await?;
    assert_eq!(product_count, 0);

    Ok(())
}

struct TestContext {
    app: axum::Router,
    pool: db::DbPool,
    data_root: PathBuf,
}

struct AcceptanceFixture {
    field_id: String,
    scene_id: String,
}

async fn enroll_test_fleet_node(ctx: &TestContext, hardware_id: &str) -> Result<String> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet/nodes/enroll")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "hardware_id": hardware_id,
                        "kind": "drone",
                        "capabilities": ["multispectral"],
                        "owner_org_id": "org-alpha",
                        "runtime_mode": "simulation"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    let enrolled: serde_json::Value = serde_json::from_slice(&body)?;
    Ok(enrolled
        .get("node_id")
        .and_then(|value| value.as_str())
        .expect("node_id should exist")
        .to_string())
}

async fn seed_tractor_registry_field(ctx: &TestContext) -> Result<()> {
    let farm_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-tractor",
                        "owner": "org-alpha",
                        "name": "Tractor Farm"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(farm_response.status(), StatusCode::OK);

    let field_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-tractor",
                        "field_id": "field-tractor",
                        "name": "Tractor Field",
                        "boundary": {
                            "crs": "EPSG:4326",
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.1 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(field_response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_weather_forecast_field(ctx: &TestContext) -> Result<()> {
    let farm_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-weather",
                        "owner": "org-alpha",
                        "name": "Weather Farm"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(farm_response.status(), StatusCode::OK);

    let field_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-weather",
                        "field_id": "field-weather",
                        "name": "Weather Field",
                        "boundary": {
                            "crs": "EPSG:4326",
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.1 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(field_response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_water_management_field(ctx: &TestContext) -> Result<()> {
    let farm_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-water",
                        "owner": "org-alpha",
                        "name": "Water Farm"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(farm_response.status(), StatusCode::OK);

    let field_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-water",
                        "field_id": "field-water",
                        "name": "Water Field",
                        "boundary": {
                            "crs": "EPSG:4326",
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.1 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(field_response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_drought_management_field(ctx: &TestContext) -> Result<()> {
    let farm_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-drought",
                        "owner": "org-alpha",
                        "name": "Drought Farm"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(farm_response.status(), StatusCode::OK);

    let field_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": "farm-drought",
                        "field_id": "field-drought",
                        "name": "Drought Field",
                        "boundary": {
                            "crs": "EPSG:4326",
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.1 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(field_response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_marketplace_org(ctx: &TestContext, farm_id: &str, org_id: &str) -> Result<()> {
    let farm_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": farm_id,
                        "owner": org_id,
                        "name": format!("Marketplace {org_id}")
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(farm_response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_marketplace_account(
    ctx: &TestContext,
    account_id: &str,
    org_id: &str,
    party_type: &str,
) -> Result<()> {
    seed_marketplace_account_with_roles(
        ctx,
        account_id,
        org_id,
        party_type,
        &["marketplace:seller"],
    )
    .await
}

async fn seed_marketplace_account_with_roles(
    ctx: &TestContext,
    account_id: &str,
    org_id: &str,
    party_type: &str,
    role_refs: &[&str],
) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/accounts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "account_id": account_id,
                        "org_id": org_id,
                        "party_type": party_type,
                        "role_refs": role_refs
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_marketplace_catalog_item(
    ctx: &TestContext,
    item_id: &str,
    org_id: &str,
    owner_account_id: &str,
) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/catalog/items")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "item_id": item_id,
                        "org_id": org_id,
                        "kind": "input",
                        "category": "seed",
                        "name": "Hybrid corn seed",
                        "unit_of_measure": "bag",
                        "owner_account_id": owner_account_id
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn marketplace_inventory_adjust(
    ctx: &TestContext,
    inventory_id: &str,
    action: &str,
    org_id: &str,
    qty: f64,
) -> Result<Response> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/marketplace/inventory/{inventory_id}/{action}"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "org_id": org_id,
                        "qty": qty
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    Ok(response)
}

async fn seed_marketplace_order_dependencies(ctx: &TestContext) -> Result<()> {
    seed_marketplace_org(ctx, "farm-market-alpha", "org-alpha").await?;
    seed_marketplace_account(ctx, "supplier-001", "org-alpha", "supplier").await?;
    seed_marketplace_account_with_roles(
        ctx,
        "buyer-001",
        "org-alpha",
        "grower",
        &["marketplace:buyer"],
    )
    .await?;
    seed_marketplace_catalog_item(ctx, "seed-corn-001", "org-alpha", "supplier-001").await?;

    let listing = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/listings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "listing_id": "listing-seed-corn-001",
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "price": 125.0,
                        "currency": "USD",
                        "available_qty": 40.0,
                        "window": {
                            "from": "2026-06-14T09:00:00Z",
                            "to": "2026-07-14T09:00:00Z"
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(listing.status(), StatusCode::OK);

    let inventory = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/inventory")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "inventory_id": "inventory-seed-corn-001",
                        "item_id": "seed-corn-001",
                        "org_id": "org-alpha",
                        "on_hand": 40.0,
                        "reserved": 0.0
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(inventory.status(), StatusCode::OK);

    Ok(())
}

async fn marketplace_order_transition(
    ctx: &TestContext,
    order_id: &str,
    status: &str,
) -> Result<StatusCode> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/marketplace/orders/{order_id}/transition"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "org_id": "org-alpha",
                        "actor_id": "supplier-001",
                        "status": status
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    Ok(response.status())
}

async fn seed_marketplace_order(ctx: &TestContext, order_id: &str, qty: f64) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/orders")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "order_id": order_id,
                        "org_id": "org-alpha",
                        "listing_ref": "listing-seed-corn-001",
                        "buyer_account_id": "buyer-001",
                        "qty": qty
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn create_marketplace_fulfillment(
    ctx: &TestContext,
    fulfillment_id: &str,
    order_ref: &str,
    org_id: &str,
) -> Result<Response> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/fulfillments")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "fulfillment_id": fulfillment_id,
                        "order_ref": order_ref,
                        "org_id": org_id,
                        "carrier_ref": "carrier:opaque",
                        "tracking_ref": "tracking:opaque",
                        "actor_id": "ops-001"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    Ok(response)
}

async fn marketplace_fulfillment_transition(
    ctx: &TestContext,
    fulfillment_id: &str,
    status: &str,
) -> Result<StatusCode> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/marketplace/fulfillments/{fulfillment_id}/transition"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "org_id": "org-alpha",
                        "actor_id": "ops-001",
                        "status": status
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    Ok(response.status())
}

async fn create_marketplace_rating(
    ctx: &TestContext,
    rating_id: &str,
    order_ref: &str,
    rater_account_id: &str,
    ratee_account_id: &str,
    score: f64,
) -> Result<Response> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/marketplace/ratings")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "rating_id": rating_id,
                        "order_ref": order_ref,
                        "rater_account_id": rater_account_id,
                        "ratee_account_id": ratee_account_id,
                        "score": score,
                        "comment": "Reliable counterparty",
                        "org_scope": "org-alpha"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    Ok(response)
}

async fn seed_marketplace_demand_field(
    ctx: &TestContext,
    field_id: &str,
    org_id: &str,
) -> Result<()> {
    seed_marketplace_org(ctx, "farm-market-alpha", org_id).await?;
    let field = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "field_id": field_id,
                        "farm_id": "farm-market-alpha",
                        "owner": org_id,
                        "name": "Demand Field",
                        "crop": "corn",
                        "season": "2026",
                        "boundary": {
                            "crs": "EPSG:4326",
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.1 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(field.status(), StatusCode::OK);
    Ok(())
}

async fn seed_marketplace_demand_product(
    ctx: &TestContext,
    product_id: &str,
    field_id: &str,
    kind: &str,
) -> Result<()> {
    let scene_id = format!("scene-{product_id}");
    sqlx::query(
        r#"
        INSERT INTO scenes (
            scene_id, owner, sensor, acquired_at, data_path, metadata_json,
            cloud_cover, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&scene_id)
    .bind("org-alpha")
    .bind("multispectral")
    .bind("2026-06-15T09:00:00Z")
    .bind(format!("/tmp/{scene_id}"))
    .bind("{}")
    .bind(0.0_f64)
    .bind("2026-06-15T09:00:00Z")
    .execute(&ctx.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO products (
            product_id, scene_id, field_id, season_id, kind, path, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(product_id)
    .bind(scene_id)
    .bind(field_id)
    .bind("2026")
    .bind(kind)
    .bind(format!("/tmp/{product_id}.tif"))
    .bind("2026-06-15T09:00:00Z")
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

async fn seed_sustainability_field(
    ctx: &TestContext,
    farm_id: &str,
    field_id: &str,
    season_id: &str,
) -> Result<()> {
    let farm_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": farm_id,
                        "owner": "org-alpha",
                        "name": format!("Sustainability {farm_id}")
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(farm_response.status(), StatusCode::OK);

    let field_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": farm_id,
                        "field_id": field_id,
                        "name": format!("Sustainability {field_id}"),
                        "season": season_id,
                        "boundary": {
                            "crs": "EPSG:4326",
                            "coordinates": [
                                { "longitude": -96.7, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.1 },
                                { "longitude": -96.2, "latitude": 41.4 },
                                { "longitude": -96.7, "latitude": 41.1 }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(field_response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_sustainability_record(
    ctx: &TestContext,
    farm_id: &str,
    field_id: &str,
    season_id: &str,
    record_id: &str,
    operation_id: &str,
) -> Result<()> {
    seed_sustainability_metric_record(
        ctx,
        farm_id,
        field_id,
        season_id,
        record_id,
        operation_id,
        "carbon_footprint",
    )
    .await
}

async fn seed_sustainability_metric_record(
    ctx: &TestContext,
    farm_id: &str,
    field_id: &str,
    season_id: &str,
    record_id: &str,
    operation_id: &str,
    metric_type: &str,
) -> Result<()> {
    seed_sustainability_field(ctx, farm_id, field_id, season_id).await?;
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sustainability/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "record_id": record_id,
                        "field_id": field_id,
                        "season_id": season_id,
                        "operation_id": operation_id,
                        "metric_type": metric_type,
                        "method_version": "carbon.identity.v1"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

fn carbon_footprint_payload(footprint_id: &str, include_required: bool) -> serde_json::Value {
    let mut inputs = vec![
        json!({
            "kind": "diesel_liters",
            "quantity": 10.0,
            "unit": "liters",
            "evidence_ref": "input:fuel-log-001"
        }),
        json!({
            "kind": "fertilizer_nitrogen_kg",
            "quantity": 20.0,
            "unit": "kg_n",
            "evidence_ref": "input:fertilizer-ticket-001"
        }),
        json!({
            "kind": "electricity_kwh",
            "quantity": 15.0,
            "unit": "kwh",
            "evidence_ref": "input:meter-001"
        }),
        json!({
            "kind": "field_passes",
            "quantity": 2.0,
            "unit": "passes",
            "evidence_ref": "input:coverage-log-001"
        }),
    ];
    let mut factors = vec![
        json!({
            "input_kind": "diesel_liters",
            "factor_kg_co2e_per_unit": 2.68,
            "factor_ref": "factor:diesel:v1"
        }),
        json!({
            "input_kind": "fertilizer_nitrogen_kg",
            "factor_kg_co2e_per_unit": 6.3,
            "factor_ref": "factor:nitrogen:v1"
        }),
        json!({
            "input_kind": "electricity_kwh",
            "factor_kg_co2e_per_unit": 0.4,
            "factor_ref": "factor:electricity:v1"
        }),
        json!({
            "input_kind": "field_passes",
            "factor_kg_co2e_per_unit": 5.0,
            "factor_ref": "factor:field-pass:v1"
        }),
    ];
    if !include_required {
        inputs.retain(|input| {
            input.get("kind").and_then(|kind| kind.as_str()) != Some("field_passes")
        });
        factors.retain(|factor| {
            factor.get("input_kind").and_then(|kind| kind.as_str()) != Some("field_passes")
        });
    }

    json!({
        "footprint_id": footprint_id,
        "record_id": if include_required { "sustain-carbon-001" } else { "sustain-carbon-missing" },
        "operation_id": "operation-carbon-001",
        "inputs": inputs,
        "factor_set": {
            "version": "agbot-carbon-factors-v1",
            "factors": factors
        }
    })
}

fn biomass_estimate_payload(estimate_id: &str, mismatched_extent: bool) -> serde_json::Value {
    let index_spatial_ref = if mismatched_extent {
        json!({
            "georeferenced": true,
            "crs": "EPSG:32614",
            "bbox": {
                "min_lon": 0.0,
                "min_lat": 10.0,
                "max_lon": 20.0,
                "max_lat": 30.0
            },
            "geo_transform": [0.0, 10.0, 0.0, 30.0, 0.0, -10.0],
            "resolution": { "x": 10.0, "y": 10.0 }
        })
    } else {
        biomass_spatial_ref()
    };

    json!({
        "estimate_id": estimate_id,
        "record_id": if mismatched_extent { "sustain-biomass-mismatch" } else { "sustain-biomass-001" },
        "canopy_layer": {
            "layer_ref": "layer:canopy-height-001",
            "width": 2,
            "height": 2,
            "values": [1.0, 2.0, 0.0, 4.0],
            "spatial_ref": biomass_spatial_ref()
        },
        "vegetation_index_layer": {
            "layer_ref": "layer:ndvi-001",
            "width": 2,
            "height": 2,
            "values": [0.5, 0.25, 0.8, -0.1],
            "spatial_ref": index_spatial_ref
        },
        "method_version": "biomass.canopy_ndvi.v1",
        "biomass_coefficient": 0.48
    })
}

fn biomass_spatial_ref() -> serde_json::Value {
    json!({
        "georeferenced": true,
        "crs": "EPSG:32614",
        "bbox": {
            "min_lon": 0.0,
            "min_lat": 0.0,
            "max_lon": 20.0,
            "max_lat": 20.0
        },
        "geo_transform": [0.0, 10.0, 0.0, 20.0, 0.0, -10.0],
        "resolution": { "x": 10.0, "y": 10.0 }
    })
}

async fn insert_sustainability_record_row(
    ctx: &TestContext,
    record_id: &str,
    field_id: &str,
    season_id: &str,
    operation_id: &str,
    metric_type: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sustainability_records (
            record_id, field_id, season_id, operation_id, metric_type, method_version,
            created_at, audit_id
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(record_id)
    .bind(field_id)
    .bind(season_id)
    .bind(operation_id)
    .bind(metric_type)
    .bind("sustainability.fixture.v1")
    .bind("2026-06-16T00:00:00Z")
    .bind(format!("audit:{record_id}"))
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

fn sustainability_baseline_payload(baseline_id: &str) -> serde_json::Value {
    json!({
        "baseline_id": baseline_id,
        "field_id": "field-baseline",
        "season_id": "season-2025",
        "metric_type": "biomass",
        "metric_value": 100.0,
        "source_record_id": "sustain-baseline-2025",
        "method_version": "sustainability.baseline.v1",
        "evidence_refs": ["biomass:baseline-2025"]
    })
}

fn sustainability_comparison_payload(
    comparison_id: &str,
    field_id: &str,
    current_source_record_id: &str,
    current_value: f64,
) -> serde_json::Value {
    json!({
        "comparison_id": comparison_id,
        "field_id": field_id,
        "baseline_season_id": "season-2025",
        "current_season_id": "season-2026",
        "metric_type": "biomass",
        "current_value": current_value,
        "current_source_record_id": current_source_record_id,
        "method_version": "sustainability.baseline_compare.v1"
    })
}

async fn insert_biomass_estimate_row(
    ctx: &TestContext,
    estimate_id: &str,
    record_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO biomass_estimates (
            estimate_id, record_id, biomass_value, area, crs, extent_json,
            resolution_json, source_layer_refs_json, method_version, result_hash, computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(estimate_id)
    .bind(record_id)
    .bind(48.0_f64)
    .bind(200.0_f64)
    .bind("EPSG:32614")
    .bind(
        json!({
            "min_lon": 0.0,
            "min_lat": 0.0,
            "max_lon": 20.0,
            "max_lat": 20.0
        })
        .to_string(),
    )
    .bind(json!({ "x": 10.0, "y": 10.0 }).to_string())
    .bind(json!(["layer:canopy-height-001", "layer:ndvi-001"]).to_string())
    .bind("biomass.canopy_ndvi.v1")
    .bind("result-hash-biomass-001")
    .bind("2026-06-17T00:00:00Z")
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

fn sustainability_mrv_payload(
    trail_id: &str,
    output_ref: &str,
    complete: bool,
) -> serde_json::Value {
    json!({
        "trail_id": trail_id,
        "output_ref": output_ref,
        "output_kind": "biomass_estimate",
        "input_layer_refs": if complete {
            json!(["layer:canopy-height-001", "layer:ndvi-001"])
        } else {
            json!([])
        },
        "method": "biomass_canopy_ndvi",
        "method_version": "biomass.canopy_ndvi.v1",
        "crs": "EPSG:32614",
        "extent": {
            "min_lon": 0.0,
            "min_lat": 0.0,
            "max_lon": 20.0,
            "max_lat": 20.0
        },
        "parameters": {
            "biomass_coefficient": "0.48",
            "source_record_id": "sustain-biomass-mrv"
        },
        "audit_id": "audit-biomass-mrv",
        "result_hash": "result-hash-biomass-001"
    })
}

fn biodiversity_proxy_payload(proxy_id: &str, values: Vec<f64>) -> serde_json::Value {
    json!({
        "proxy_id": proxy_id,
        "field_id": "field-biodiversity",
        "layer": {
            "layer_ref": "layer:ndvi-biodiversity",
            "width": 2,
            "height": 2,
            "values": values,
            "spatial_ref": {
                "georeferenced": true,
                "crs": "EPSG:32614",
                "bbox": {
                    "min_lon": 0.0,
                    "min_lat": 0.0,
                    "max_lon": 20.0,
                    "max_lat": 20.0
                },
                "geo_transform": [0.0, 10.0, 0.0, 20.0, 0.0, -10.0],
                "resolution": { "x": 10.0, "y": 10.0 }
            }
        },
        "method_version": "biodiversity.imagery_proxy.v1",
        "cover_threshold": 0.3
    })
}

fn soil_carbon_proxy_payload(proxy_id: &str, include_sufficient: bool) -> serde_json::Value {
    json!({
        "proxy_id": proxy_id,
        "record_id": "sustain-soil-carbon-001",
        "field_id": "field-soil-carbon",
        "index_inputs": [{
            "evidence_ref": "layer:ndvi-soil-carbon",
            "value": 3.2,
            "weight": 0.8
        }],
        "biomass_inputs": if include_sufficient {
            json!([{
                "evidence_ref": "biomass:estimate-001",
                "value": 5.6,
                "weight": 1.2
            }])
        } else {
            json!([])
        },
        "practice_inputs": if include_sufficient {
            json!([{
                "practice_ref": "practice:cover-crop-2026",
                "carbon_delta": 0.7
            }])
        } else {
            json!([])
        },
        "method_version": "soil_carbon.proxy.v1"
    })
}

fn sustainability_kpi_payload(kpi_id: &str, current_value: Option<f64>) -> serde_json::Value {
    json!({
        "kpi_id": kpi_id,
        "field_id": "field-sustainability-kpi",
        "season_id": "season-2026",
        "metric_ref": "biodiversity:biodiversity-001",
        "current_value": current_value,
        "target_value": 0.8,
        "direction": "higher_is_better",
        "at_risk_fraction": 0.8,
        "method_version": "sustainability.kpi.v1",
        "evidence_refs": ["biodiversity:biodiversity-001", "target:cover-2026"]
    })
}

async fn insert_sustainability_kpi_row(
    ctx: &TestContext,
    kpi_id: &str,
    field_id: &str,
    season_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO sustainability_kpis (
            kpi_id, field_id, season_id, metric_ref, current_value, target_value,
            direction, at_risk_fraction, status, evidence_refs_json, method_version,
            result_hash, computed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        "#,
    )
    .bind(kpi_id)
    .bind(field_id)
    .bind(season_id)
    .bind("biodiversity:biodiversity-001")
    .bind(0.72_f64)
    .bind(0.8_f64)
    .bind("higher_is_better")
    .bind(0.8_f64)
    .bind("on_track")
    .bind(json!(["biodiversity:biodiversity-001", "target:cover-2026"]).to_string())
    .bind("sustainability.kpi.v1")
    .bind("result-hash-kpi-export")
    .bind("2026-06-18T00:00:00Z")
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

fn sustainability_certification_pack_payload(
    pack_id: &str,
    claim_id: &str,
    claimed_output_refs: Vec<&str>,
) -> serde_json::Value {
    json!({
        "pack_id": pack_id,
        "claim_id": claim_id,
        "claim_type": "regenerative_biomass_gain",
        "field_id": "field-certification",
        "season_id": "season-2026",
        "claimed_output_refs": claimed_output_refs,
        "method_version": "sustainability.certification_pack.v1"
    })
}

async fn create_content_fixture(ctx: &TestContext, content_id: &str) -> Result<serde_json::Value> {
    create_content_fixture_body(ctx, content_id, "First draft", "org-alpha").await
}

async fn create_content_fixture_body(
    ctx: &TestContext,
    content_id: &str,
    body: &str,
    org_id: &str,
) -> Result<serde_json::Value> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/items")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "content_id": content_id,
                        "content_type": "article",
                        "author_id": "author-001",
                        "org_id": org_id,
                        "body": body
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    serde_json::from_slice(&body).map_err(Into::into)
}

async fn publish_content_fixture(ctx: &TestContext, content_id: &str, org_id: &str) -> Result<()> {
    post_content_workflow_for_org(
        ctx,
        content_id,
        org_id,
        json!({
            "action": "submit_for_review",
            "actor_id": "author-001",
            "actor_role": "author"
        }),
    )
    .await?;
    post_content_workflow_for_org(
        ctx,
        content_id,
        org_id,
        json!({
            "action": "publish",
            "actor_id": "editor-001",
            "actor_role": "editor"
        }),
    )
    .await?;
    Ok(())
}

async fn submit_community_contribution(ctx: &TestContext, contribution_id: &str) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/content/community-contributions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "contribution_id": contribution_id,
                        "org_id": "org-alpha",
                        "contributor_id": "grower-001",
                        "content_type": "post",
                        "body": "Grower note about cover crops."
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn post_content_workflow(
    ctx: &TestContext,
    content_id: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value> {
    post_content_workflow_for_org(ctx, content_id, "org-alpha", payload).await
}

async fn post_content_workflow_for_org(
    ctx: &TestContext,
    content_id: &str,
    org_id: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/content/items/{content_id}/workflow?org_id={org_id}"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    serde_json::from_slice(&body).map_err(Into::into)
}

async fn register_test_component(
    ctx: &TestContext,
    component_id: &str,
    airframe_id: &str,
) -> Result<()> {
    register_test_component_type(ctx, component_id, "battery", airframe_id).await
}

async fn post_rollout_control(
    ctx: &TestContext,
    action: &str,
    simulation_validated: bool,
    targets_flight_nodes: bool,
) -> Result<serde_json::Value> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/ota-rollouts/control")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "rollout_id": "rollout-2026-06-12",
                        "actor": "ops@example.com",
                        "action": action,
                        "version": "2.0.0",
                        "stage": "staged",
                        "requested_at": "2026-06-12T14:00:00Z",
                        "simulation_validated": simulation_validated,
                        "targets_flight_nodes": targets_flight_nodes
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    serde_json::from_slice(&body).map_err(Into::into)
}

async fn register_test_component_type(
    ctx: &TestContext,
    component_id: &str,
    component_type: &str,
    airframe_id: &str,
) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fleet-health/components")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "component_id": component_id,
                        "component_type": component_type,
                        "serial": format!("SERIAL-{component_id}"),
                        "airframe_id": airframe_id,
                        "installed_at": "2026-06-01T10:00:00Z"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn register_soil_iot_test_device(ctx: &TestContext, device_id: &str) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/soil-iot/devices")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "device_id": device_id,
                        "org_id": "org-soil-001",
                        "field_id": "field-soil-001",
                        "zone_id": "zone-ne",
                        "sensor_type": "soil_moisture",
                        "position": {
                            "latitude": 38.5816,
                            "longitude": -121.4944,
                            "crs": "EPSG:4326"
                        },
                        "calibration_profile_ref": format!("calibration:{device_id}:v1")
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_provenance_ledger_fixture(ctx: &TestContext) -> Result<()> {
    insert_provenance_lineage_fixture(
        ctx,
        "product:ndvi:alpha-2026-06-12",
        "product",
        json!([]),
        json!({
            "scene_id": "scene-alpha",
            "index": "ndvi"
        }),
        "2026-06-12T10:00:00Z",
    )
    .await?;
    insert_provenance_lineage_fixture(
        ctx,
        "finding:09:stress-ne-zone",
        "finding",
        json!(["product:ndvi:alpha-2026-06-12"]),
        json!({
            "index": "ndvi",
            "threshold": 0.42,
            "zone": "NE"
        }),
        "2026-06-12T10:05:00Z",
    )
    .await?;

    insert_provenance_audit_fixture(
        ctx,
        1,
        "audit-entry-hash-0001",
        None,
        "action:publish-product",
        "product_publish",
        Some("product:ndvi:alpha-2026-06-12"),
        json!({"publish_status": "published"}),
        "2026-06-12T10:01:00Z",
    )
    .await?;
    insert_provenance_audit_fixture(
        ctx,
        2,
        "audit-entry-hash-0002",
        Some("audit-entry-hash-0001"),
        "action:record-finding",
        "finding_record",
        Some("finding:09:stress-ne-zone"),
        json!({"finding": "stress-ne-zone"}),
        "2026-06-12T10:06:00Z",
    )
    .await?;

    Ok(())
}

async fn insert_provenance_lineage_fixture(
    ctx: &TestContext,
    artifact_id: &str,
    kind: &str,
    inputs: serde_json::Value,
    parameters: serde_json::Value,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO provenance_lineage_records (
            artifact_id, kind, inputs_json, method, parameters_json, operator, actor_id,
            actor_kind, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(artifact_id)
    .bind(kind)
    .bind(inputs.to_string())
    .bind("09.crop_stress_finding")
    .bind(parameters.to_string())
    .bind("operator:dsp-7")
    .bind("operator:dsp-7")
    .bind("drone_service_provider")
    .bind(created_at)
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn insert_provenance_audit_fixture(
    ctx: &TestContext,
    seq: i64,
    entry_hash: &str,
    prev_hash: Option<&str>,
    action_ref: &str,
    action_kind: &str,
    artifact_ref: Option<&str>,
    payload: serde_json::Value,
    ts: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO provenance_audit_entries (
            entry_hash, seq, prev_hash, payload_hash, actor_id, actor_kind, ts, action_ref,
            action_kind, artifact_ref, payload_json, occurred_at, outcome, refusal_reason
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(entry_hash)
    .bind(seq)
    .bind(prev_hash)
    .bind(format!("payload-hash-{seq:04}"))
    .bind("operator:dsp-7")
    .bind("drone_service_provider")
    .bind(ts)
    .bind(action_ref)
    .bind(action_kind)
    .bind(artifact_ref)
    .bind(payload.to_string())
    .bind(ts)
    .bind("accepted")
    .bind(Option::<String>::None)
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn post_fired_alert(ctx: &TestContext, payload: serde_json::Value) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/alerting/fired-alerts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn seed_compliance_field(ctx: &TestContext, field_id: &str, owner: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fields (field_id, owner, name, crop, season, notes, boundary_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(field_id)
    .bind(owner)
    .bind(format!("Compliance {field_id}"))
    .bind("corn")
    .bind("2026")
    .bind(None::<String>)
    .bind(
        json!({
            "crs": "EPSG:4326",
            "coordinates": [
                { "longitude": -96.7, "latitude": 41.1 },
                { "longitude": -96.2, "latitude": 41.1 },
                { "longitude": -96.2, "latitude": 41.4 },
                { "longitude": -96.7, "latitude": 41.1 }
            ]
        })
        .to_string(),
    )
    .bind("2026-06-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn seed_compliance_export_records(ctx: &TestContext) -> Result<()> {
    post_compliance_record(
        ctx,
        json!({
            "record_id": "remote-log-1",
            "record_type": "remote_id_log",
            "org_id": "org-alpha",
            "field_id": "field-north",
            "actor": "operator-17",
            "provenance_ref": "provenance:remote-id/remote-log-1/v1",
            "payload": remote_id_payload()
        }),
    )
    .await?;
    post_compliance_record(
        ctx,
        json!({
            "record_id": "chem-app-1",
            "record_type": "chemical_application",
            "org_id": "org-alpha",
            "field_id": "field-north",
            "flight_id": "flight-77",
            "actor": "operator-17",
            "provenance_ref": "provenance:application/chem-app-1/v1",
            "payload": chemical_application_payload()
        }),
    )
    .await?;
    post_compliance_record(
        ctx,
        json!({
            "record_id": "cert-operator-17",
            "record_type": "operator_certification",
            "org_id": "org-alpha",
            "field_id": "field-north",
            "actor": "compliance-officer-1",
            "provenance_ref": "provenance:cert/operator-17/v1"
        }),
    )
    .await?;
    post_compliance_record(
        ctx,
        json!({
            "record_id": "auth-flight-77",
            "record_type": "authorization_decision",
            "org_id": "org-alpha",
            "field_id": "field-north",
            "flight_id": "flight-77",
            "actor": "compliance-officer-1",
            "provenance_ref": "provenance:authorization/flight-77/v1"
        }),
    )
    .await?;
    Ok(())
}

async fn post_compliance_record(ctx: &TestContext, payload: serde_json::Value) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/compliance/records")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

fn remote_id_payload() -> serde_json::Value {
    json!({
        "flight_id": "flight-77",
        "operator_id": "operator-17",
        "aircraft_id": "aircraft-ag-9",
        "started_at": "2026-06-12T12:00:00Z",
        "ended_at": "2026-06-12T12:18:00Z",
        "track": [
            {
                "observed_at": "2026-06-12T12:02:00Z",
                "longitude": -96.61,
                "latitude": 41.21,
                "altitude_m": 118.0
            }
        ],
        "telemetry_gaps": [
            {
                "started_at": "2026-06-12T12:04:00Z",
                "ended_at": "2026-06-12T12:08:00Z",
                "reason": "remote-id-broadcast-dropout"
            }
        ]
    })
}

fn chemical_application_payload() -> serde_json::Value {
    json!({
        "application_id": "chem-app-1",
        "product": "Example Herbicide",
        "epa_or_label_ref": "EPA-12345-LBL",
        "field_id": "field-north",
        "geometry": {
            "crs": "EPSG:4326",
            "coordinates": [
                { "longitude": -96.70, "latitude": 41.10 },
                { "longitude": -96.20, "latitude": 41.10 },
                { "longitude": -96.20, "latitude": 41.40 },
                { "longitude": -96.70, "latitude": 41.40 },
                { "longitude": -96.70, "latitude": 41.10 }
            ]
        },
        "applied_at": "2026-06-12T13:00:00Z",
        "rate": 1.75,
        "units": "L/ha",
        "operator_id": "operator-17"
    })
}

async fn seed_orthomosaic_scene(
    ctx: &TestContext,
    scene_id: &str,
    field_id: &str,
    season_id: &str,
) -> Result<()> {
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    sqlx::query(
        r#"
        INSERT INTO fields (field_id, owner, name, crop, season, notes, boundary_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(field_id)
    .bind("org-alpha")
    .bind("Orthomosaic Field")
    .bind("corn")
    .bind(season_id)
    .bind(None::<String>)
    .bind(
        json!({
            "crs": "EPSG:4326",
            "coordinates": [
                { "longitude": -96.7, "latitude": 41.1 },
                { "longitude": -96.2, "latitude": 41.1 },
                { "longitude": -96.2, "latitude": 41.4 },
                { "longitude": -96.7, "latitude": 41.1 }
            ]
        })
        .to_string(),
    )
    .bind("2026-06-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at, field_id, season_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(scene_id)
    .bind("org-alpha")
    .bind("micasense-rededge")
    .bind("2026-06-01T12:00:00Z")
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(
        json!({
            "metadata": {
                "timestamp": "2026-06-01T12:00:00Z",
                "gps_position": {
                    "latitude": 41.10,
                    "longitude": -96.70,
                    "altitude": 120.0
                },
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 1280,
                "height": 960
            },
            "file_paths": {
                "B4": "B4.tif",
                "B5": "B5.tif"
            },
            "image_id": Uuid::new_v4()
        })
        .to_string(),
    )
    .bind(None::<f64>)
    .bind("2026-06-01T12:00:00Z")
    .bind(field_id)
    .bind(season_id)
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn seed_orthomosaic_frame_set(
    ctx: &TestContext,
    scene_id: &str,
    field_id: &str,
    season_id: &str,
    frame_set_id: &str,
) -> Result<()> {
    seed_orthomosaic_scene(ctx, scene_id, field_id, season_id).await?;
    sqlx::query(
        r#"
        INSERT INTO orthomosaic_frame_sets
            (frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(frame_set_id)
    .bind(scene_id)
    .bind(field_id)
    .bind(season_id)
    .bind(
        json!([
            {
                "frame_id": "frame-001",
                "capture_ts": "2026-06-01T12:00:00Z",
                "gps": {
                    "latitude": 41.10,
                    "longitude": -96.70,
                    "altitude": 120.0
                },
                "imu": {
                    "roll_deg": 1.2,
                    "pitch_deg": -0.4,
                    "yaw_deg": 87.0
                },
                "exif": {
                    "camera_model": "MicaSense RedEdge"
                }
            }
        ])
        .to_string(),
    )
    .bind("EPSG:4326")
    .bind("2026-06-01T12:05:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn seed_completed_orthomosaic_reconstruction(
    ctx: &TestContext,
    scene_id: &str,
    field_id: &str,
    season_id: &str,
    frame_set_id: &str,
    recon_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO orthomosaic_frame_sets
            (frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(frame_set_id)
    .bind(scene_id)
    .bind(field_id)
    .bind(season_id)
    .bind(
        json!([
            {
                "frame_id": "frame-001",
                "capture_ts": "2026-06-01T12:00:00Z",
                "gps": {
                    "latitude": 41.10,
                    "longitude": -96.70,
                    "altitude": 120.0
                }
            },
            {
                "frame_id": "frame-002",
                "capture_ts": "2026-06-01T12:00:02Z",
                "gps": {
                    "latitude": 41.11,
                    "longitude": -96.69,
                    "altitude": 121.0
                }
            }
        ])
        .to_string(),
    )
    .bind("EPSG:4326")
    .bind("2026-06-01T12:05:00Z")
    .execute(&ctx.pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO orthomosaic_reconstructions
            (recon_id, frame_set_id, params_json, status, failure_reason, created_at, updated_at)
        VALUES (?1, ?2, ?3, 'completed', NULL, ?4, ?4)
        "#,
    )
    .bind(recon_id)
    .bind(frame_set_id)
    .bind(json!({"feature_detector": "orb"}).to_string())
    .bind("2026-06-01T12:06:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn seed_orthomosaic_publish_product(
    ctx: &TestContext,
    scene_id: &str,
    kind: &str,
) -> Result<()> {
    let scene_dir = ctx.data_root.join("scenes").join(scene_id);
    std::fs::create_dir_all(&scene_dir)?;
    let spatial_ref = orthomosaic_tile_spatial_ref_json();
    insert_scene_with_spatial_ref(ctx, scene_id, &scene_dir, spatial_ref.clone()).await?;
    link_scene_context(ctx, scene_id, "ortho-field-1", "season-2026").await?;
    let product_path = scene_dir
        .join("products")
        .join(kind)
        .join(format!("{kind}.png"));
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    write_gray_png(&product_path, 120)?;
    sqlx::query(
        r#"
        INSERT INTO products (
            product_id, scene_id, field_id, season_id, kind, path,
            width_px, height_px, gsd_m_per_px,
            spatial_ref_json, source_image_ids_json, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(format!("{scene_id}:{kind}"))
    .bind(scene_id)
    .bind("ortho-field-1")
    .bind("season-2026")
    .bind(kind)
    .bind(product_path.to_string_lossy().to_string())
    .bind(2_i64)
    .bind(2_i64)
    .bind(0.05_f64)
    .bind(spatial_ref.to_string())
    .bind(json!(["frame-001", "frame-002"]).to_string())
    .bind("2026-06-01T12:08:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn post_orthomosaic_publish_gate(
    ctx: &TestContext,
    scene_id: &str,
    kind: &str,
    quality_verdict: &str,
) -> Result<serde_json::Value> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/orthomosaic/products/{scene_id}/{kind}/publish-gate"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "scene_id": scene_id,
                        "product_kind": kind,
                        "requested_at": "2026-06-01T12:09:00Z",
                        "qa_report_ref": "qa-report-001",
                        "quality_verdict": quality_verdict,
                        "provenance": {
                            "frames": ["frame-001", "frame-002"],
                            "camera_model": "MicaSense RedEdge",
                            "gcps": ["GCP-1"],
                            "params": {
                                "feature_detector": "orb",
                                "resolution_m_per_px": 0.05
                            },
                            "software_version": "agbot-orthomosaic 0.1.0"
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    serde_json::from_slice(&body).map_err(Into::into)
}

fn orthomosaic_tile_spatial_ref_json() -> serde_json::Value {
    json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    })
}

fn layer_spatial_ref_json() -> serde_json::Value {
    json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.7,
            "min_lat": 41.1,
            "max_lon": -96.6,
            "max_lat": 41.2
        },
        "geo_transform": [-96.7, 0.05, 0.0, 41.2, 0.0, -0.05],
        "resolution": {
            "x": 0.05,
            "y": 0.05
        }
    })
}

fn advisory_spatial_ref() -> serde_json::Value {
    json!({
        "georeferenced": true,
        "crs": "EPSG:4326",
        "bbox": {
            "min_lon": -96.8,
            "min_lat": 41.0,
            "max_lon": -96.2,
            "max_lat": 41.6
        },
        "geo_transform": [-96.8, 0.3, 0.0, 41.6, 0.0, -0.3],
        "resolution": {
            "x": 0.3,
            "y": 0.3
        }
    })
}

async fn insert_advisory_field(ctx: &TestContext, field_id: &str, season: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fields (field_id, owner, name, season, boundary_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(field_id)
    .bind("org-alpha")
    .bind(format!("{field_id} name"))
    .bind(season)
    .bind(
        json!({
            "crs": "EPSG:4326",
            "coordinates": [
                { "longitude": -96.8, "latitude": 41.0 },
                { "longitude": -96.2, "latitude": 41.0 },
                { "longitude": -96.2, "latitude": 41.6 },
                { "longitude": -96.8, "latitude": 41.6 },
                { "longitude": -96.8, "latitude": 41.0 }
            ]
        })
        .to_string(),
    )
    .bind("2026-01-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn insert_advisory_scene(
    ctx: &TestContext,
    scene_id: &str,
    field_id: Option<&str>,
    season_id: Option<&str>,
    acquired_at: &str,
    cloud_cover: Option<f64>,
    scene_dir: &Path,
    spatial_ref: serde_json::Value,
) -> Result<()> {
    std::fs::create_dir_all(scene_dir)?;

    insert_scene_with_spatial_ref(ctx, scene_id, scene_dir, spatial_ref).await?;
    let linked_at = field_id.map(|_| "2026-01-01T00:00:00Z");

    sqlx::query(
        r#"
        UPDATE scenes
        SET acquired_at = ?1, cloud_cover = ?2, field_id = ?3, season_id = ?4, linked_at = ?5
        WHERE scene_id = ?6
        "#,
    )
    .bind(acquired_at)
    .bind(cloud_cover)
    .bind(field_id)
    .bind(season_id)
    .bind(linked_at)
    .bind(scene_id)
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn setup_golden_acceptance_fixture(
    ctx: &TestContext,
    tmp: &TempDir,
) -> Result<AcceptanceFixture> {
    let farm_id = "acceptance-farm".to_string();
    let create_farm = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/farms")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "farm_id": farm_id,
                        "name": "Acceptance Farm"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(create_farm.status(), StatusCode::OK);

    let shapefile_path = tmp.path().join("acceptance_field.shp");
    write_polygon_shapefile(
        &shapefile_path,
        &[vec![
            (-96.7, 41.1),
            (-96.2, 41.1),
            (-96.2, 41.4),
            (-96.7, 41.4),
            (-96.7, 41.1),
        ]],
    )?;
    write_wgs84_prj(&shapefile_path)?;
    let import_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/fields/import/shapefile")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "path": shapefile_path.to_string_lossy().to_string(),
                        "farm_id": farm_id,
                        "name_prefix": "Acceptance Field",
                        "crop": "corn",
                        "season": "2026"
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(import_response.status(), StatusCode::OK);
    let body = to_bytes(import_response.into_body(), 64 * 1024).await?;
    let fields_json: serde_json::Value = serde_json::from_slice(&body)?;
    let field_id = fields_json
        .pointer("/0/field_id")
        .and_then(|value| value.as_str())
        .expect("field_id should exist")
        .to_string();

    let scene_id = "acceptance-scene".to_string();
    let scene_dir = ctx.data_root.join("scenes").join(&scene_id);
    let product_path = scene_dir.join("products").join("ndvi").join("sample.png");
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    std::fs::write(&product_path, TEST_PNG_BYTES)?;
    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&scene_id)
    .bind("landsat8")
    .bind("2025-01-01T00:00:00Z")
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(
        json!({
            "metadata": {
                "timestamp": "2025-01-01T00:00:00Z",
                "gps_position": {
                    "latitude": 41.25,
                    "longitude": -96.45,
                    "altitude": 350.0
                },
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 256,
                "height": 256,
                "spatial_ref": {
                    "georeferenced": true,
                    "crs": "EPSG:4326",
                    "bbox": {
                        "min_lon": -96.7,
                        "min_lat": 41.1,
                        "max_lon": -96.2,
                        "max_lat": 41.4
                    }
                }
            },
            "file_paths": {
                "B4": "B4.png",
                "B5": "B5.png"
            },
            "image_id": Uuid::new_v4()
        })
        .to_string(),
    )
    .bind(None::<f64>)
    .bind("2025-01-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    let link_response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scenes/{scene_id}/field/{field_id}"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(link_response.status(), StatusCode::OK);

    Ok(AcceptanceFixture { field_id, scene_id })
}

async fn create_acceptance_annotation(ctx: &TestContext, scene_id: &str) -> Result<()> {
    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/annotations"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "accept-ann-1",
                        "label": "Acceptance issue",
                        "severity": "medium",
                        "geometry": {
                            "type": "point",
                            "coordinate": {
                                "longitude": -96.45,
                                "latitude": 41.25
                            }
                        }
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn insert_scene_with_spatial_ref(
    ctx: &TestContext,
    scene_id: &str,
    scene_dir: &Path,
    spatial_ref: serde_json::Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(scene_id)
    .bind("org-alpha")
    .bind("landsat8")
    .bind("2026-05-01T00:00:00Z")
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(
        json!({
            "metadata": {
                "timestamp": "2026-05-01T00:00:00Z",
                "gps_position": null,
                "bands": ["B4", "B5"],
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": 2,
                "height": 2,
                "spatial_ref": spatial_ref.clone()
            },
            "file_paths": {
                "B4": "B4.png",
                "B5": "B5.png"
            },
            "image_id": Uuid::new_v4()
        })
        .to_string(),
    )
    .bind(None::<f64>)
    .bind("2026-05-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    upsert_scene_spatial_ref(ctx, scene_id, spatial_ref).await
}

async fn generate_report(
    ctx: &TestContext,
    scene_id: &str,
    title: &str,
    visibility: Option<&str>,
) -> Result<String> {
    let mut payload = json!({
        "title": title
    });
    if let Some(visibility) = visibility {
        payload["visibility"] = serde_json::Value::String(visibility.to_string());
    }

    let response = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scenes/{scene_id}/reports"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request should build"),
        )
        .await
        .expect("router should handle request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 256 * 1024).await?;
    let report_json: serde_json::Value = serde_json::from_slice(&body)?;
    Ok(report_json
        .get("report_id")
        .and_then(|value| value.as_str())
        .expect("report_id should exist")
        .to_string())
}

async fn link_scene_context(
    ctx: &TestContext,
    scene_id: &str,
    field_id: &str,
    season_id: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE scenes SET field_id = ?1, season_id = ?2, linked_at = ?3 WHERE scene_id = ?4",
    )
    .bind(field_id)
    .bind(season_id)
    .bind("2026-05-01T00:00:00Z")
    .bind(scene_id)
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

async fn insert_layer_field(
    ctx: &TestContext,
    field_id: &str,
    boundary_json: serde_json::Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO fields (field_id, owner, name, season, boundary_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(field_id) DO UPDATE SET
            boundary_json = excluded.boundary_json,
            season = excluded.season
        "#,
    )
    .bind(field_id)
    .bind("org-alpha")
    .bind(format!("{field_id} name"))
    .bind("2026")
    .bind(boundary_json.to_string())
    .bind("2026-01-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn insert_layer_product(ctx: &TestContext, scene_id: &str, kind: &str) -> Result<()> {
    let product_path = ctx
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("products")
        .join(kind)
        .join(format!("{kind}.png"));
    std::fs::create_dir_all(product_path.parent().expect("product parent exists"))?;
    std::fs::write(&product_path, TEST_PNG_BYTES)?;

    sqlx::query(
        r#"
        INSERT INTO products (scene_id, kind, path, created_at)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(scene_id, kind) DO UPDATE SET path = excluded.path,
                                                created_at = excluded.created_at
        "#,
    )
    .bind(scene_id)
    .bind(kind)
    .bind(product_path.to_string_lossy().to_string())
    .bind("2026-05-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn insert_ingest_source(ctx: &TestContext, scene_id: &str, source_path: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO scene_ingests (
            scene_id, status, status_reason, ingested_at, acquisition_date,
            coverage_fraction, source_path, updated_at
        )
        VALUES (?1, 'stored', NULL, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(scene_id) DO UPDATE SET
            status = excluded.status,
            ingested_at = excluded.ingested_at,
            acquisition_date = excluded.acquisition_date,
            coverage_fraction = excluded.coverage_fraction,
            source_path = excluded.source_path,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(scene_id)
    .bind("2026-05-01T00:00:00Z")
    .bind("2026-05-01")
    .bind(0.92_f64)
    .bind(source_path)
    .bind("2026-05-01T00:00:00Z")
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn seed_ingest_health_row(
    ctx: &TestContext,
    scene_id: &str,
    status: &str,
    reason: Option<&str>,
    updated_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO scene_ingests (
            scene_id, status, status_reason, ingested_at, acquisition_date,
            coverage_fraction, source_path, updated_at
        )
        VALUES (?1, ?2, ?3, NULL, NULL, NULL, ?4, ?5)
        ON CONFLICT(scene_id) DO UPDATE SET
            status = excluded.status,
            status_reason = excluded.status_reason,
            source_path = excluded.source_path,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(scene_id)
    .bind(status)
    .bind(reason)
    .bind(format!("fixture://{scene_id}"))
    .bind(updated_at)
    .execute(&ctx.pool)
    .await?;

    Ok(())
}

async fn upsert_scene_spatial_ref(
    ctx: &TestContext,
    scene_id: &str,
    spatial_ref: serde_json::Value,
) -> Result<()> {
    let bbox = spatial_ref
        .get("bbox")
        .expect("spatial_ref bbox should exist");
    let resolution = spatial_ref
        .get("resolution")
        .expect("spatial_ref resolution should exist");
    let geo_transform = spatial_ref
        .get("geo_transform")
        .expect("spatial_ref transform should exist");
    sqlx::query(
        r#"
        INSERT INTO scene_spatial_refs (
            scene_id, spatial_ref_json, crs, min_lon, min_lat, max_lon, max_lat,
            resolution_x, resolution_y, geo_transform_json, asserted_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
        ON CONFLICT(scene_id) DO UPDATE SET
            spatial_ref_json = excluded.spatial_ref_json,
            crs = excluded.crs,
            min_lon = excluded.min_lon,
            min_lat = excluded.min_lat,
            max_lon = excluded.max_lon,
            max_lat = excluded.max_lat,
            resolution_x = excluded.resolution_x,
            resolution_y = excluded.resolution_y,
            geo_transform_json = excluded.geo_transform_json,
            asserted_at = datetime('now')
        "#,
    )
    .bind(scene_id)
    .bind(spatial_ref.to_string())
    .bind(
        spatial_ref
            .get("crs")
            .and_then(|value| value.as_str())
            .expect("spatial_ref CRS should exist"),
    )
    .bind(
        bbox.get("min_lon")
            .and_then(|value| value.as_f64())
            .expect("min_lon should exist"),
    )
    .bind(
        bbox.get("min_lat")
            .and_then(|value| value.as_f64())
            .expect("min_lat should exist"),
    )
    .bind(
        bbox.get("max_lon")
            .and_then(|value| value.as_f64())
            .expect("max_lon should exist"),
    )
    .bind(
        bbox.get("max_lat")
            .and_then(|value| value.as_f64())
            .expect("max_lat should exist"),
    )
    .bind(
        resolution
            .get("x")
            .and_then(|value| value.as_f64())
            .expect("resolution x should exist"),
    )
    .bind(
        resolution
            .get("y")
            .and_then(|value| value.as_f64())
            .expect("resolution y should exist"),
    )
    .bind(geo_transform.to_string())
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

async fn test_app(tmp: &TempDir) -> Result<TestContext> {
    test_app_with_paths(tmp.path().join("data"), tmp.path().join("geo_hub_test.db")).await
}

async fn test_app_with_paths(data_root: PathBuf, db_path: PathBuf) -> Result<TestContext> {
    let config = HubConfig {
        bind_address: "127.0.0.1:0".to_string(),
        database_url: sqlite_url(&db_path),
        data_root: data_root.clone(),
        ..HubConfig::default()
    };

    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;

    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
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

fn write_polygon_shapefile(path: &Path, rings: &[Vec<(f64, f64)>]) -> Result<()> {
    write_shapefile(path, 5, rings)
}

fn write_point_shapefile(path: &Path, points: &[(f64, f64)]) -> Result<()> {
    let records = points
        .iter()
        .map(|(x, y)| vec![(*x, *y)])
        .collect::<Vec<_>>();
    write_shapefile(path, 1, &records)
}

fn write_wgs84_prj(shapefile_path: &Path) -> Result<()> {
    std::fs::write(
        shapefile_path.with_extension("prj"),
        "GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433],AUTHORITY[\"EPSG\",\"4326\"]]",
    )?;
    Ok(())
}

fn write_shapefile(path: &Path, shape_type: i32, records: &[Vec<(f64, f64)>]) -> Result<()> {
    let mut bytes = vec![0u8; 100];
    bytes[0..4].copy_from_slice(&9994i32.to_be_bytes());
    bytes[28..32].copy_from_slice(&1000i32.to_le_bytes());
    bytes[32..36].copy_from_slice(&shape_type.to_le_bytes());

    let mut file_x_min = f64::INFINITY;
    let mut file_x_max = f64::NEG_INFINITY;
    let mut file_y_min = f64::INFINITY;
    let mut file_y_max = f64::NEG_INFINITY;

    for (index, record) in records.iter().enumerate() {
        let record_content = shapefile_record_content(shape_type, record)?;
        let content_len_words = (record_content.len() / 2) as i32;
        bytes.extend_from_slice(&((index as i32) + 1).to_be_bytes());
        bytes.extend_from_slice(&content_len_words.to_be_bytes());
        bytes.extend_from_slice(&record_content);

        for (x, y) in record {
            file_x_min = file_x_min.min(*x);
            file_x_max = file_x_max.max(*x);
            file_y_min = file_y_min.min(*y);
            file_y_max = file_y_max.max(*y);
        }
    }

    if file_x_min.is_finite() {
        bytes[36..44].copy_from_slice(&file_x_min.to_le_bytes());
        bytes[44..52].copy_from_slice(&file_y_min.to_le_bytes());
        bytes[52..60].copy_from_slice(&file_x_max.to_le_bytes());
        bytes[60..68].copy_from_slice(&file_y_max.to_le_bytes());
    }

    let file_len_words = (bytes.len() / 2) as i32;
    bytes[24..28].copy_from_slice(&file_len_words.to_be_bytes());
    std::fs::write(path, bytes)?;
    Ok(())
}

fn shapefile_record_content(shape_type: i32, points: &[(f64, f64)]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&shape_type.to_le_bytes());

    match shape_type {
        1 => {
            let (x, y) = points
                .first()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("point shapefile record requires one point"))?;
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
        5 => {
            let x_min = points
                .iter()
                .map(|point| point.0)
                .fold(f64::INFINITY, f64::min);
            let x_max = points
                .iter()
                .map(|point| point.0)
                .fold(f64::NEG_INFINITY, f64::max);
            let y_min = points
                .iter()
                .map(|point| point.1)
                .fold(f64::INFINITY, f64::min);
            let y_max = points
                .iter()
                .map(|point| point.1)
                .fold(f64::NEG_INFINITY, f64::max);
            bytes.extend_from_slice(&x_min.to_le_bytes());
            bytes.extend_from_slice(&y_min.to_le_bytes());
            bytes.extend_from_slice(&x_max.to_le_bytes());
            bytes.extend_from_slice(&y_max.to_le_bytes());
            bytes.extend_from_slice(&1i32.to_le_bytes());
            bytes.extend_from_slice(&(points.len() as i32).to_le_bytes());
            bytes.extend_from_slice(&0i32.to_le_bytes());
            for (x, y) in points {
                bytes.extend_from_slice(&x.to_le_bytes());
                bytes.extend_from_slice(&y.to_le_bytes());
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "unsupported test shapefile type {}",
                shape_type
            ))
        }
    }

    Ok(bytes)
}
