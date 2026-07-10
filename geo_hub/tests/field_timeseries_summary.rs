//! Per-field time-series summary API (batch S-5).
//!
//! Covers `GET /api/fields/:field_id/timeseries/summary`: calendar-year
//! season stats (count/mean/max/peak date per year), the rolling-baseline
//! anomaly block over the trailing observations, the same-DOY prior-years
//! comparison, the single-source filter, and the missing-metric client
//! error.
//!
//! Points are seeded with direct INSERTs using exactly the columns the S-3
//! extraction writes, so the API is tested against the real persisted shape.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{db, server, HubConfig};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const FIELD: &str = "field-42";
const METRIC: &str = "sat.ndvi.mean";

async fn ctx(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let db_path = tmp.path().join("field_timeseries_summary.db");
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

/// Seed one observation with the same columns the S-3 extraction writes.
async fn seed(
    pool: &db::DbPool,
    metric: &str,
    t: &str,
    value: f64,
    source: &str,
    product_id: &str,
) -> Result<()> {
    let metadata = json!({
        "source": source,
        "scene_id": format!("scene-{product_id}"),
        "level": "l2",
    })
    .to_string();
    sqlx::query(
        r#"
        INSERT INTO time_series_points (
            entity_ref, metric, t, value_kind, scalar_value, source_ref,
            created_at, metadata_json
        )
        VALUES (?1, ?2, ?3, 'scalar', ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(format!("field:{FIELD}"))
    .bind(metric)
    .bind(t)
    .bind(value)
    .bind(format!("product:{product_id}"))
    .bind("2026-02-20T00:00:00Z")
    .bind(metadata)
    .execute(pool)
    .await?;
    Ok(())
}

async fn get_response(app: &Router, uri: &str) -> Result<(StatusCode, Vec<u8>)> {
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024).await?;
    Ok((status, bytes.to_vec()))
}

async fn get_json(app: &Router, uri: &str) -> Result<serde_json::Value> {
    let (status, bytes) = get_response(app, uri).await?;
    let body = String::from_utf8_lossy(&bytes).to_string();
    assert_eq!(status, StatusCode::OK, "GET {uri}: {body}");
    Ok(serde_json::from_slice(&bytes)?)
}

fn assert_close(value: &serde_json::Value, expected: f64, what: &str) {
    let value = value
        .as_f64()
        .unwrap_or_else(|| panic!("{what} is a number, got {value}"));
    assert!(
        (value - expected).abs() < 1e-9,
        "{what}: {value} != {expected}"
    );
}

#[tokio::test]
async fn summary_reports_seasonal_anomaly_against_baseline() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;

    // Three years of monthly observations (15th of each month, one source).
    // 2024/2025 have a June peak; 2026 holds flat at 0.60 and collapses to
    // 0.10 in December — the value the anomaly block must flag.
    for month in 1..=12u32 {
        let value_2024 = if month == 6 { 0.70 } else { 0.40 };
        let value_2025 = if month == 6 { 0.80 } else { 0.50 };
        let value_2026 = if month == 12 { 0.10 } else { 0.60 };
        for (year, value) in [(2024, value_2024), (2025, value_2025), (2026, value_2026)] {
            seed(
                &pool,
                METRIC,
                &format!("{year}-{month:02}-15T00:00:00Z"),
                value,
                "landsat",
                &format!("p-{year}-{month:02}"),
            )
            .await?;
        }
    }

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries/summary?metric={METRIC}"),
    )
    .await?;
    assert_eq!(body["field_id"], FIELD);
    assert_eq!(body["metric"], METRIC);
    assert_eq!(body["series_basis"], "merged");
    assert_eq!(body["observation_count"], 36);
    assert_eq!(body["first_t"], "2024-01-15T00:00:00Z");
    assert_eq!(body["last_t"], "2026-12-15T00:00:00Z");

    // Calendar-year season stats: count, mean, max and the peak date.
    let per_year = body["per_year"].as_array().expect("per_year array");
    assert_eq!(per_year.len(), 3);
    assert_eq!(per_year[0]["year"], 2024);
    assert_eq!(per_year[0]["count"], 12);
    assert_close(
        &per_year[0]["mean"],
        (11.0 * 0.40 + 0.70) / 12.0,
        "2024 mean",
    );
    assert_close(&per_year[0]["max"], 0.70, "2024 max");
    assert_eq!(per_year[0]["max_t"], "2024-06-15T00:00:00Z");
    assert_eq!(per_year[1]["year"], 2025);
    assert_close(
        &per_year[1]["mean"],
        (11.0 * 0.50 + 0.80) / 12.0,
        "2025 mean",
    );
    assert_eq!(per_year[1]["max_t"], "2025-06-15T00:00:00Z");
    assert_eq!(per_year[2]["year"], 2026);
    assert_eq!(per_year[2]["count"], 12);
    assert_close(
        &per_year[2]["mean"],
        (11.0 * 0.60 + 0.10) / 12.0,
        "2026 mean",
    );
    assert_close(&per_year[2]["max"], 0.60, "2026 max");
    assert_eq!(
        per_year[2]["max_t"], "2026-01-15T00:00:00Z",
        "flat year: first occurrence of the max is the peak date"
    );

    // Rolling baseline: trailing window (Jul-Nov 2026, all 0.60) vs the
    // depressed December value.
    let anomaly = &body["anomaly"];
    assert!(anomaly.is_object(), "anomaly block present: {body}");
    assert_eq!(anomaly["latest_t"], "2026-12-15T00:00:00Z");
    assert_close(&anomaly["latest_value"], 0.10, "latest_value");
    assert_close(&anomaly["baseline_mean"], 0.60, "baseline_mean");
    assert_close(&anomaly["deviation"], -0.50, "deviation");
    assert_eq!(anomaly["is_anomalous"], true);

    // Prior-years same-DOY comparison: Dec 15 of 2024 (0.40) and 2025 (0.50).
    let vs_prior = &body["vs_prior_years"];
    assert!(vs_prior.is_object(), "vs_prior_years block present: {body}");
    assert_eq!(vs_prior["current_t"], "2026-12-15T00:00:00Z");
    assert_close(&vs_prior["current_value"], 0.10, "current_value");
    assert_eq!(vs_prior["prior_point_count"], 2);
    assert_close(&vs_prior["seasonal_mean"], 0.45, "seasonal_mean");
    assert_close(
        &vs_prior["delta_from_seasonal_mean"],
        -0.35,
        "delta_from_seasonal_mean",
    );
    Ok(())
}

#[tokio::test]
async fn summary_with_insufficient_history_omits_anomaly() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    seed(
        &pool,
        METRIC,
        "2026-01-15T00:00:00Z",
        0.50,
        "landsat",
        "p-1",
    )
    .await?;
    seed(
        &pool,
        METRIC,
        "2026-02-15T00:00:00Z",
        0.60,
        "landsat",
        "p-2",
    )
    .await?;

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries/summary?metric={METRIC}"),
    )
    .await?;
    assert_eq!(body["observation_count"], 2);
    // Two points cannot fill the rolling window: no anomaly verdict rather
    // than a fabricated one.
    assert!(body["anomaly"].is_null(), "anomaly omitted: {body}");
    // No prior-year observation near the same day-of-year either.
    assert!(
        body["vs_prior_years"].is_null(),
        "vs_prior_years omitted: {body}"
    );

    let per_year = body["per_year"].as_array().expect("per_year array");
    assert_eq!(per_year.len(), 1);
    assert_eq!(per_year[0]["year"], 2026);
    assert_eq!(per_year[0]["count"], 2);
    assert_close(&per_year[0]["mean"], 0.55, "2026 mean");
    assert_close(&per_year[0]["max"], 0.60, "2026 max");
    assert_eq!(per_year[0]["max_t"], "2026-02-15T00:00:00Z");

    // Empty series: an honest zero summary, not an error.
    let body = get_json(
        &app,
        &format!("/api/fields/no-such-field/timeseries/summary?metric={METRIC}"),
    )
    .await?;
    assert_eq!(body["observation_count"], 0);
    assert!(body["first_t"].is_null());
    assert!(body["last_t"].is_null());
    assert_eq!(body["per_year"], json!([]));
    assert!(body["anomaly"].is_null());
    assert!(body["vs_prior_years"].is_null());
    Ok(())
}

#[tokio::test]
async fn summary_respects_source_filter() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    seed(
        &pool,
        METRIC,
        "2026-01-01T00:00:00Z",
        0.40,
        "landsat",
        "p-1",
    )
    .await?;
    seed(
        &pool,
        METRIC,
        "2026-02-01T00:00:00Z",
        0.50,
        "landsat",
        "p-2",
    )
    .await?;
    // Far from any landsat point: zero overlap pairs, identity harmonization,
    // so the merged mean is exact.
    seed(
        &pool,
        METRIC,
        "2026-03-01T00:00:00Z",
        0.90,
        "sentinel2",
        "p-3",
    )
    .await?;

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries/summary?metric={METRIC}"),
    )
    .await?;
    assert_eq!(body["series_basis"], "merged");
    assert_eq!(body["observation_count"], 3);
    assert_close(&body["per_year"][0]["mean"], 0.60, "merged mean");
    assert_close(&body["per_year"][0]["max"], 0.90, "merged max");

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries/summary?metric={METRIC}&source=landsat"),
    )
    .await?;
    assert_eq!(body["series_basis"], "single_source");
    assert_eq!(body["observation_count"], 2);
    assert_eq!(body["last_t"], "2026-02-01T00:00:00Z");
    assert_close(&body["per_year"][0]["mean"], 0.45, "landsat mean");
    assert_close(&body["per_year"][0]["max"], 0.50, "landsat max");
    Ok(())
}

#[tokio::test]
async fn summary_missing_metric_is_bad_request() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = ctx(&tmp).await?;

    let (status, bytes) =
        get_response(&app, &format!("/api/fields/{FIELD}/timeseries/summary")).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&bytes)
    );

    let (status, _) = get_response(
        &app,
        &format!("/api/fields/{FIELD}/timeseries/summary?metric=%20%20"),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "blank metric refused");
    Ok(())
}
