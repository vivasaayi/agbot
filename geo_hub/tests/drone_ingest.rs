//! Integration tests for the drone-session ingest route (Track A batch 6):
//! `POST /api/ingest/drone-session` commits a scene + L0 capture products via
//! `commit_ingest`, rejects duplicate sessions, and rejects manifests with
//! missing integrity checksums.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::catalog::{self, ProductFilter};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::json;
use shared::product_graph::ProductLevel;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

async fn router(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let db_path = tmp.path().join("drone_route.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
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
    Ok((server::build_router(state), pool))
}

fn manifest(session: &str, checksum: &str) -> serde_json::Value {
    json!({
        "source_id": "drone-1",
        "session_id": session,
        "platform": "quad-x",
        "sensor": "multispectral",
        "scene": {
            "scene_id": format!("scene-{session}"),
            "sensor": "multispectral",
            "acquired_at": "2026-06-01T00:00:00Z",
            "data_path": "data/scenes/s1",
            "metadata_json": "{}"
        },
        "captures": [{
            "capture_id": "cap-1",
            "kind": "multispectral_capture",
            "file_path": "data/scenes/s1/cap-1.tif",
            "checksum_sha256": checksum,
            "size_bytes": 2048,
            "captured_at": "2026-06-01T00:00:00Z"
        }]
    })
}

async fn post(app: &Router, body: serde_json::Value) -> Result<(StatusCode, serde_json::Value)> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ingest/drone-session")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body)?))?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await?;
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    Ok((status, value))
}

#[tokio::test]
async fn drone_session_commits_scene_and_l0_captures() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = router(&tmp).await?;

    let (status, receipt) = post(&app, manifest("sess-1", "chk-abc")).await?;
    assert_eq!(status, StatusCode::OK, "receipt: {receipt}");
    assert_eq!(receipt["scene_id"], "scene-sess-1");
    assert_eq!(receipt["product_ids"].as_array().unwrap().len(), 1);

    // The capture is an L0 catalog product, and the source is registered.
    let products = catalog::list_products(
        &pool,
        &ProductFilter {
            level: Some(ProductLevel::L0),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(products.len(), 1);
    assert_eq!(products[0].kind, "multispectral_capture");

    let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM catalog_sources")
        .fetch_one(&pool)
        .await?;
    assert_eq!(source_count, 1);
    Ok(())
}

#[tokio::test]
async fn duplicate_session_is_rejected() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = router(&tmp).await?;

    let (first, _) = post(&app, manifest("sess-1", "chk-abc")).await?;
    assert_eq!(first, StatusCode::OK);
    let (second, _) = post(&app, manifest("sess-1", "chk-abc")).await?;
    assert_eq!(
        second,
        StatusCode::BAD_REQUEST,
        "re-ingesting a session is rejected"
    );
    Ok(())
}

#[tokio::test]
async fn missing_checksum_is_rejected() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = router(&tmp).await?;

    let (status, _) = post(&app, manifest("sess-1", "")).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a capture without a checksum is rejected"
    );
    Ok(())
}
