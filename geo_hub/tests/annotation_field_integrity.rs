use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

#[tokio::test]
async fn annotation_cannot_override_the_scenes_linked_field() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("hub.db").display()),
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

    sqlx::query(
        "INSERT INTO scenes \
         (scene_id, owner, sensor, acquired_at, data_path, metadata_json, created_at, field_id) \
         VALUES ('scene-1', 'org-1', 'test', '2026-06-01T00:00:00Z', 'scene-1', '{}', \
                 '2026-06-01T00:00:00Z', 'field-1')",
    )
    .execute(&pool)
    .await?;

    let response = server::build_router(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scenes/scene-1/annotations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "annotation_id": "annotation-1",
                        "field_id": "field-2",
                        "label": "stress zone",
                        "geometry": {
                            "type": "point",
                            "coordinate": { "longitude": -96.4, "latitude": 41.2 }
                        }
                    })
                    .to_string(),
                ))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 64 * 1024).await?;
    assert!(
        String::from_utf8_lossy(&body).contains("linked to field field-1"),
        "response should explain the field mismatch: {}",
        String::from_utf8_lossy(&body)
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM annotations")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0, "a rejected annotation must not be persisted");

    Ok(())
}
