//! Route tests for the MapLibre browse UI (`/browse`, satellite pipeline
//! batch 5): the page and its embedded assets are served with the right
//! content types and reference the same-origin APIs they consume.

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

async fn ctx(tmp: &TempDir) -> Result<Router> {
    let db_path = tmp.path().join("browse_ui.db");
    let config = HubConfig {
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
    Ok(server::build_router(state))
}

async fn get(app: &Router, uri: &str) -> Result<(StatusCode, String, String)> {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty())?)
        .await?;
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = String::from_utf8(
        to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await?
            .to_vec(),
    )?;
    Ok((status, content_type, body))
}

#[tokio::test]
async fn browse_serves_html_page() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    let (status, content_type, body) = get(&app, "/browse").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    // The page pins MapLibre with SRI and loads the local module + stylesheet.
    assert!(
        body.contains("maplibre-gl@4.7.1"),
        "MapLibre CDN pin missing"
    );
    assert!(body.contains("integrity=\"sha384-"), "SRI pin missing");
    assert!(body.contains("/browse/app.js"));
    assert!(body.contains("/browse/style.css"));
    Ok(())
}

#[tokio::test]
async fn browse_assets_have_correct_content_types() -> Result<()> {
    let tmp = TempDir::new()?;
    let app = ctx(&tmp).await?;

    let (status, content_type, body) = get(&app, "/browse/app.js").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type.starts_with("text/javascript"),
        "{content_type}"
    );
    // The module consumes the same-origin geo_hub APIs (no CORS required),
    // including the derive affordances (batch 26): VCI/TCI, SPI, and water
    // extent are triggered per item from the browser.
    for endpoint in [
        "/api/stac/collections",
        "/api/stac/search",
        "/api/fields/export/geojson",
        "/api/drought-management/rasters/derive",
        "/api/drought-management/spi/derive",
        "/api/water-management/extent/derive",
    ] {
        assert!(
            body.contains(endpoint),
            "app.js does not reference {endpoint}"
        );
    }

    // The derive kinds are wired: drought from ndvi/lst, SPI from
    // precipitation, water extent from the optical + SAR water kinds
    // (with the optional JRC prior).
    let (_, _, app_js) = get(&app, "/browse/app.js").await?;
    for marker in [
        "deriveActionsFor",
        "prior_product_id",
        "window_months",
        "sar_vv",
        "thermal_lst",
    ] {
        assert!(app_js.contains(marker), "app.js missing {marker}");
    }

    let (status, content_type, body) = get(&app, "/browse/style.css").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/css"), "{content_type}");
    assert!(body.contains("#map"));
    assert!(body.contains(".derive-form"), "derive styles missing");
    Ok(())
}
