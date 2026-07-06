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
    // Field time series panel (batch S-13): collapsible panel with metric
    // selector (defaulting to sat.ndvi.mean), date range, source toggles,
    // merged overlay toggle, and the chart/harmonization/summary containers.
    for marker in [
        "id=\"field-timeseries-panel\"",
        "id=\"ts-metric\"",
        "sat.ndvi.mean",
        "id=\"ts-start\"",
        "id=\"ts-end\"",
        "id=\"ts-sources\"",
        "id=\"ts-merged\"",
        "id=\"ts-load\"",
        "id=\"ts-chart\"",
        "id=\"ts-harmonization\"",
        "id=\"ts-summary\"",
    ] {
        assert!(body.contains(marker), "index.html missing {marker}");
    }
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
        "/api/ingest/sen2cor/index/derive",
        "/api/composites/derive",
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
        "band_b",
        "index_product_id",
        "composite_product_id",
        "COMPOSITABLE_KINDS",
    ] {
        assert!(app_js.contains(marker), "app.js missing {marker}");
    }

    // Field time series panel (batch S-13): the module hits the timeseries
    // trio (query, metric discovery, per-year summary — the base route via
    // template literal, so assert the shared /timeseries segment plus the
    // subpaths) and renders the inline SVG chart with the merged-overlay,
    // harmonization-report, and anomaly-badge affordances.
    for marker in [
        "/timeseries?",
        "/timeseries/metrics",
        "/timeseries/summary",
        "sat.ndvi.mean",
        "renderTsChart",
        "tsTimeTicks",
        "harmonization",
        "per_year",
        "is_anomalous",
        "stroke-dasharray",
    ] {
        assert!(app_js.contains(marker), "app.js missing {marker}");
    }

    let (status, content_type, body) = get(&app, "/browse/style.css").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/css"), "{content_type}");
    assert!(body.contains("#map"));
    assert!(body.contains(".derive-form"), "derive styles missing");
    for marker in [".ts-svg", ".ts-legend", ".ts-anomaly-badge", ".ts-year-table"] {
        assert!(body.contains(marker), "style.css missing {marker}");
    }
    Ok(())
}
