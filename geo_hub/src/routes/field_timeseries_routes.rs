//! Field time-series query routes (`/api/fields/:field_id/timeseries...`).
//!
//! Thin wrapper over `crate::field_timeseries`: the S-3 extraction fills
//! `time_series_points`; these handlers serve the per-source series, the
//! harmonized merged view, and the metric discovery list (batch S-4).

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::field_timeseries::{
    list_field_metrics, query_field_series, summarize_field_series, FieldSeriesResponse,
    FieldSeriesSummary, FieldTimeseriesError,
};
use crate::state::AppState;

impl From<FieldTimeseriesError> for AppError {
    fn from(err: FieldTimeseriesError) -> Self {
        match err {
            FieldTimeseriesError::ProductNotFound(_) => AppError::NotFound,
            FieldTimeseriesError::NotLevel2 { .. }
            | FieldTimeseriesError::NoFieldScope(_)
            | FieldTimeseriesError::NoArtifact(_)
            | FieldTimeseriesError::NoTemporalStart(_)
            | FieldTimeseriesError::NoValidPixels(_) => AppError::BadRequest(err.to_string()),
            other => AppError::Anyhow(other.into()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FieldTimeseriesQuery {
    pub metric: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub source: Option<String>,
}

/// `GET /api/fields/:field_id/timeseries?metric=sat.ndvi.mean&start=&end=&source=`
///
/// `metric` is required; `start`/`end` bound `t` inclusively; `source`
/// narrows to one source family and skips harmonization.
pub async fn get_field_timeseries(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
    Query(query): Query<FieldTimeseriesQuery>,
) -> AppResult<Json<FieldSeriesResponse>> {
    let metric = query
        .metric
        .as_deref()
        .map(str::trim)
        .filter(|metric| !metric.is_empty())
        .ok_or_else(|| {
            AppError::BadRequest(
                "metric query parameter is required (e.g. metric=sat.ndvi.mean)".to_string(),
            )
        })?;
    let response = query_field_series(
        &state.pool,
        &field_id,
        metric,
        query.start.as_deref(),
        query.end.as_deref(),
        query.source.as_deref(),
    )
    .await?;
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
pub struct FieldTimeseriesSummaryQuery {
    pub metric: Option<String>,
    pub source: Option<String>,
}

/// `GET /api/fields/:field_id/timeseries/summary?metric=sat.ndvi.mean&source=`
///
/// `metric` is required. Without `source` the summary runs over the
/// harmonized merged series; with it, over that source's raw series.
pub async fn get_field_timeseries_summary(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
    Query(query): Query<FieldTimeseriesSummaryQuery>,
) -> AppResult<Json<FieldSeriesSummary>> {
    let metric = query
        .metric
        .as_deref()
        .map(str::trim)
        .filter(|metric| !metric.is_empty())
        .ok_or_else(|| {
            AppError::BadRequest(
                "metric query parameter is required (e.g. metric=sat.ndvi.mean)".to_string(),
            )
        })?;
    let summary =
        summarize_field_series(&state.pool, &field_id, metric, query.source.as_deref()).await?;
    Ok(Json(summary))
}

#[derive(Debug, Serialize)]
pub struct FieldTimeseriesMetricsResponse {
    pub field_id: String,
    pub metrics: Vec<String>,
}

/// `GET /api/fields/:field_id/timeseries/metrics` — distinct metrics
/// recorded for the field, sorted.
pub async fn get_field_timeseries_metrics(
    State(state): State<AppState>,
    Path(field_id): Path<String>,
) -> AppResult<Json<FieldTimeseriesMetricsResponse>> {
    let metrics = list_field_metrics(&state.pool, &field_id).await?;
    Ok(Json(FieldTimeseriesMetricsResponse { field_id, metrics }))
}
