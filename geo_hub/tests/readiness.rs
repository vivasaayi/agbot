//! Liveness/readiness probes: `/health` is dependency-free; `/ready` (and its
//! `/readyz` alias) confirm the SQLite pool answers a query. Batch 3.3a.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

async fn test_router() -> Result<(Router, TempDir)> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("geo_hub_test.db");
    let config = HubConfig {
        bind_address: "127.0.0.1:0".to_string(),
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let state = AppState {
        pool,
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };
    Ok((server::build_router(state), tmp))
}

async fn get(router: &Router, uri: &str) -> Result<(StatusCode, String)> {
    let response = router
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty())?)
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

#[tokio::test]
async fn health_is_ok_and_dependency_free() -> Result<()> {
    let (router, _tmp) = test_router().await?;
    let (status, body) = get(&router, "/health").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
    Ok(())
}

#[tokio::test]
async fn ready_and_readyz_report_database_backed_readiness() -> Result<()> {
    let (router, _tmp) = test_router().await?;
    for uri in ["/ready", "/readyz"] {
        let (status, body) = get(&router, uri).await?;
        assert_eq!(status, StatusCode::OK, "{uri} should be ready");
        assert_eq!(body, "ready", "{uri} body");
    }
    Ok(())
}

#[tokio::test]
async fn metrics_exposes_liveness_and_pipeline_queue_gauges() -> Result<()> {
    let (router, _tmp) = test_router().await?;
    let (status, body) = get(&router, "/metrics").await?;
    assert_eq!(status, StatusCode::OK);
    // Prometheus text exposition: process liveness plus a gauge per job status
    // (all present at 0 on a fresh queue).
    assert!(body.contains("geo_hub_up 1"), "up gauge: {body}");
    assert!(
        body.contains("geo_hub_pipeline_jobs{status=\"queued\"} 0"),
        "queued gauge: {body}"
    );
    assert!(
        body.contains("geo_hub_pipeline_jobs{status=\"dead\"} 0"),
        "dead gauge: {body}"
    );
    Ok(())
}
