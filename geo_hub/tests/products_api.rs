use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
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
                .uri("/api/layers?field_id=field-alpha&season_id=2026&product_kind=ndvi&page=1&page_size=1")
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
        SELECT product_id, field_id, season_id, spatial_ref_json, source_image_ids_json, path
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
