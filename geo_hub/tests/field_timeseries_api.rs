//! Per-field multi-source time-series query API (batch S-4).
//!
//! Covers `GET /api/fields/:field_id/timeseries` and
//! `GET /api/fields/:field_id/timeseries/metrics`: grouping seeded
//! `time_series_points` rows per source family (from the `metadata_json`
//! `"source"` tag the S-3 extraction writes), the cross-sensor
//! harmonization ladder (least_squares >= 8 pairs, offset_only 3-7, none
//! < 3), the merged series ordering, and the metric/source/date filters.
//!
//! Points are seeded with direct INSERTs using exactly the columns the
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
const CAVEAT: &str =
    "bandpass/BRDF differences are not corrected; merged view is for visual continuity and coarse trends";

async fn ctx(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let db_path = tmp.path().join("field_timeseries_api.db");
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

fn series_values(series: &serde_json::Value) -> Vec<f64> {
    series
        .as_array()
        .expect("series is an array")
        .iter()
        .map(|point| point["value"].as_f64().expect("point value"))
        .collect()
}

fn series_times(series: &serde_json::Value) -> Vec<String> {
    series
        .as_array()
        .expect("series is an array")
        .iter()
        .map(|point| point["t"].as_str().expect("point t").to_string())
        .collect()
}

#[tokio::test]
async fn timeseries_route_returns_per_source_series() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let metric = "sat.ndvi.mean";
    seed(
        &pool,
        metric,
        "2026-01-03T10:00:00Z",
        0.41,
        "landsat",
        "p-ls-1",
    )
    .await?;
    seed(
        &pool,
        metric,
        "2026-01-19T10:00:00Z",
        0.44,
        "landsat",
        "p-ls-2",
    )
    .await?;
    seed(
        &pool,
        metric,
        "2026-01-05T10:00:00Z",
        0.52,
        "sentinel2",
        "p-s2-1",
    )
    .await?;
    seed(
        &pool,
        metric,
        "2026-01-06T10:00:00Z",
        0.48,
        "hls",
        "p-hls-1",
    )
    .await?;

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries?metric={metric}"),
    )
    .await?;
    assert_eq!(body["field_id"], FIELD);
    assert_eq!(body["metric"], metric);

    let per_source = body["per_source"].as_object().expect("per_source object");
    let sources: Vec<&String> = per_source.keys().collect();
    assert_eq!(sources, ["hls", "landsat", "sentinel2"]);
    assert_eq!(series_values(&per_source["landsat"]), vec![0.41, 0.44]);
    assert_eq!(series_values(&per_source["sentinel2"]), vec![0.52]);
    assert_eq!(series_values(&per_source["hls"]), vec![0.48]);

    let first_landsat = &per_source["landsat"][0];
    assert_eq!(first_landsat["t"], "2026-01-03T10:00:00Z");
    assert_eq!(first_landsat["source"], "landsat");
    assert_eq!(first_landsat["product_ref"], "product:p-ls-1");

    // Merged carries every observation once, ordered by time, each keeping
    // its origin source tag.
    let merged = body["merged"].as_array().expect("merged array");
    assert_eq!(merged.len(), 4);
    let merged_times = series_times(&body["merged"]);
    let mut sorted = merged_times.clone();
    sorted.sort();
    assert_eq!(merged_times, sorted, "merged series must be sorted by t");
    let merged_sources: Vec<&str> = merged
        .iter()
        .map(|point| point["source"].as_str().unwrap())
        .collect();
    assert_eq!(merged_sources, ["landsat", "sentinel2", "hls", "landsat"]);
    Ok(())
}

#[tokio::test]
async fn merged_series_applies_offset_from_overlap_pairs() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let metric = "sat.ndvi.mean";
    // Reference: hls. Four sentinel2 observations sit within +/-3 days of an
    // hls observation and read +0.05 high; one trailing sentinel2 point has
    // no reference partner and must still be adjusted.
    let hls = [
        ("2026-01-01T00:00:00Z", 0.50),
        ("2026-01-11T00:00:00Z", 0.60),
        ("2026-01-21T00:00:00Z", 0.70),
        ("2026-01-31T00:00:00Z", 0.55),
    ];
    for (index, (t, value)) in hls.iter().enumerate() {
        seed(&pool, metric, t, *value, "hls", &format!("p-hls-{index}")).await?;
    }
    let sentinel = [
        ("2026-01-02T00:00:00Z", 0.55),
        ("2026-01-12T00:00:00Z", 0.65),
        ("2026-01-22T00:00:00Z", 0.75),
        ("2026-02-01T00:00:00Z", 0.60),
        ("2026-02-15T00:00:00Z", 0.62),
    ];
    for (index, (t, value)) in sentinel.iter().enumerate() {
        seed(
            &pool,
            metric,
            t,
            *value,
            "sentinel2",
            &format!("p-s2-{index}"),
        )
        .await?;
    }

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries?metric={metric}"),
    )
    .await?;

    let harmonization = body["harmonization"].as_array().expect("harmonization");
    assert_eq!(harmonization.len(), 1, "one non-reference source");
    let entry = &harmonization[0];
    assert_eq!(entry["source"], "sentinel2");
    assert_eq!(entry["method"], "offset_only");
    assert_eq!(entry["pair_count"], 4);
    assert!((entry["gain"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    assert!(
        (entry["offset"].as_f64().unwrap() + 0.05).abs() < 1e-9,
        "offset: {}",
        entry["offset"]
    );
    assert_eq!(entry["caveat"], CAVEAT);

    // per_source stays raw; merged carries the adjusted sentinel2 values.
    assert_eq!(
        series_values(&body["per_source"]["sentinel2"]),
        vec![0.55, 0.65, 0.75, 0.60, 0.62]
    );
    let merged = body["merged"].as_array().expect("merged");
    assert_eq!(merged.len(), 9);
    let merged_times = series_times(&body["merged"]);
    let mut sorted = merged_times.clone();
    sorted.sort();
    assert_eq!(merged_times, sorted, "merged series must be sorted by t");
    for point in merged {
        let value = point["value"].as_f64().unwrap();
        let expected = match (
            point["source"].as_str().unwrap(),
            point["t"].as_str().unwrap(),
        ) {
            ("hls", t) => hls.iter().find(|(ht, _)| *ht == t).unwrap().1,
            ("sentinel2", t) => sentinel.iter().find(|(st, _)| *st == t).unwrap().1 - 0.05,
            other => panic!("unexpected merged point {other:?}"),
        };
        assert!(
            (value - expected).abs() < 1e-9,
            "merged {point}: {value} != {expected}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn harmonization_reports_no_adjustment_under_three_pairs() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let metric = "sat.ndvi.mean";
    seed(
        &pool,
        metric,
        "2026-01-01T00:00:00Z",
        0.50,
        "hls",
        "p-hls-0",
    )
    .await?;
    seed(
        &pool,
        metric,
        "2026-01-11T00:00:00Z",
        0.60,
        "hls",
        "p-hls-1",
    )
    .await?;
    seed(
        &pool,
        metric,
        "2026-01-01T00:00:00Z",
        0.60,
        "sentinel2",
        "p-s2-0",
    )
    .await?;
    seed(
        &pool,
        metric,
        "2026-01-11T00:00:00Z",
        0.70,
        "sentinel2",
        "p-s2-1",
    )
    .await?;

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries?metric={metric}"),
    )
    .await?;
    let entry = &body["harmonization"][0];
    assert_eq!(entry["source"], "sentinel2");
    assert_eq!(entry["method"], "none");
    assert_eq!(entry["pair_count"], 2);
    assert_eq!(entry["gain"].as_f64().unwrap(), 1.0);
    assert_eq!(entry["offset"].as_f64().unwrap(), 0.0);

    // Under three pairs nothing is adjusted: merged sentinel2 values are raw.
    let merged = body["merged"].as_array().expect("merged");
    let sentinel_values: Vec<f64> = merged
        .iter()
        .filter(|point| point["source"] == "sentinel2")
        .map(|point| point["value"].as_f64().unwrap())
        .collect();
    assert_eq!(sentinel_values, vec![0.60, 0.70]);
    Ok(())
}

#[tokio::test]
async fn least_squares_gain_offset_with_eight_pairs() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let metric = "sat.ndvi.mean";
    // Eight same-day pairs on the exact linear map y = 1.2 x + 0.03: least
    // squares must recover gain/offset to numerical precision.
    const GAIN: f64 = 1.2;
    const OFFSET: f64 = 0.03;
    for index in 0..8u32 {
        let day = 1 + 4 * index; // 4-day spacing keeps nearest-match unambiguous
        let t = format!("2026-01-{day:02}T00:00:00Z");
        let x = 0.1 + 0.1 * f64::from(index);
        let y = GAIN * x + OFFSET;
        seed(&pool, metric, &t, y, "hls", &format!("p-hls-{index}")).await?;
        seed(&pool, metric, &t, x, "sentinel2", &format!("p-s2-{index}")).await?;
    }

    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries?metric={metric}"),
    )
    .await?;
    let entry = &body["harmonization"][0];
    assert_eq!(entry["source"], "sentinel2");
    assert_eq!(entry["method"], "least_squares");
    assert_eq!(entry["pair_count"], 8);
    assert!(
        (entry["gain"].as_f64().unwrap() - GAIN).abs() < 1e-6,
        "gain: {}",
        entry["gain"]
    );
    assert!(
        (entry["offset"].as_f64().unwrap() - OFFSET).abs() < 1e-6,
        "offset: {}",
        entry["offset"]
    );

    // Adjusted sentinel2 points land on the reference values.
    for point in body["merged"].as_array().expect("merged") {
        if point["source"] != "sentinel2" {
            continue;
        }
        let t = point["t"].as_str().unwrap();
        let value = point["value"].as_f64().unwrap();
        let hls_value = body["per_source"]["hls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["t"] == t)
            .expect("same-day hls point")["value"]
            .as_f64()
            .unwrap();
        assert!(
            (value - hls_value).abs() < 1e-6,
            "adjusted sentinel2 at {t}: {value} != {hls_value}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn metric_filter_and_source_filter_work() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    seed(
        &pool,
        "sat.ndvi.mean",
        "2026-01-01T00:00:00Z",
        0.4,
        "landsat",
        "p-1",
    )
    .await?;
    seed(
        &pool,
        "sat.ndvi.mean",
        "2026-01-02T00:00:00Z",
        0.5,
        "sentinel2",
        "p-2",
    )
    .await?;
    seed(
        &pool,
        "sat.evi.mean",
        "2026-01-01T00:00:00Z",
        0.3,
        "landsat",
        "p-3",
    )
    .await?;

    // Metric filter: only ndvi rows, evi stays out.
    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries?metric=sat.ndvi.mean"),
    )
    .await?;
    assert_eq!(body["merged"].as_array().unwrap().len(), 2);

    // Source filter: single source, merged == that raw series, no harmonization.
    let body = get_json(
        &app,
        &format!("/api/fields/{FIELD}/timeseries?metric=sat.ndvi.mean&source=landsat"),
    )
    .await?;
    let per_source = body["per_source"].as_object().unwrap();
    assert_eq!(per_source.keys().collect::<Vec<_>>(), ["landsat"]);
    assert_eq!(series_values(&body["merged"]), vec![0.4]);
    assert_eq!(body["merged"][0]["source"], "landsat");
    assert_eq!(body["harmonization"].as_array().unwrap().len(), 0);

    // Date range filter.
    let body = get_json(
        &app,
        &format!(
            "/api/fields/{FIELD}/timeseries?metric=sat.ndvi.mean&start=2026-01-02T00:00:00Z&end=2026-01-03T00:00:00Z"
        ),
    )
    .await?;
    assert_eq!(series_values(&body["merged"]), vec![0.5]);
    assert_eq!(body["merged"][0]["source"], "sentinel2");

    // Missing metric is a client error.
    let (status, bytes) = get_response(&app, &format!("/api/fields/{FIELD}/timeseries")).await?;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&bytes)
    );

    // Unknown field: empty series, not an error.
    let body = get_json(
        &app,
        "/api/fields/no-such-field/timeseries?metric=sat.ndvi.mean",
    )
    .await?;
    assert_eq!(body["merged"].as_array().unwrap().len(), 0);
    assert_eq!(body["per_source"].as_object().unwrap().len(), 0);
    Ok(())
}

#[tokio::test]
async fn metrics_endpoint_lists_distinct_metrics() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    seed(
        &pool,
        "sat.ndvi.mean",
        "2026-01-01T00:00:00Z",
        0.4,
        "landsat",
        "p-1",
    )
    .await?;
    seed(
        &pool,
        "sat.ndvi.mean",
        "2026-01-02T00:00:00Z",
        0.5,
        "sentinel2",
        "p-2",
    )
    .await?;
    seed(
        &pool,
        "sat.evi.mean",
        "2026-01-01T00:00:00Z",
        0.3,
        "landsat",
        "p-3",
    )
    .await?;

    let body = get_json(&app, &format!("/api/fields/{FIELD}/timeseries/metrics")).await?;
    assert_eq!(body["field_id"], FIELD);
    assert_eq!(
        body["metrics"],
        json!(["sat.evi.mean", "sat.ndvi.mean"]),
        "distinct metrics, sorted"
    );

    let body = get_json(&app, "/api/fields/no-such-field/timeseries/metrics").await?;
    assert_eq!(body["metrics"], json!([]));
    Ok(())
}
