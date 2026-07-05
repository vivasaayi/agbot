//! Weather forecasts route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn pull_weather_forecast(
    State(state): State<AppState>,
    Json(request): Json<PullWeatherForecastRequest>,
) -> AppResult<Response> {
    validate_lat_lon(request.latitude, request.longitude)?;
    let field_id = normalize_optional_text(Some(request.field_id))
        .ok_or_else(|| AppError::BadRequest("weather field_id is required".to_string()))?;
    load_field(&state, &field_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("field {field_id} does not exist")))?;
    let field_ref = canonical_weather_field_ref(&field_id);
    let provider = normalize_optional_text(Some(request.provider))
        .ok_or_else(|| AppError::BadRequest("weather provider is required".to_string()))?;
    let fetched_at =
        normalize_optional_text(request.fetched_at).unwrap_or_else(current_record_timestamp);

    if provider.eq_ignore_ascii_case("unreachable") {
        let failure = weather_fetch_failure_record(
            format!("weather-fetch-failure-{}", Uuid::new_v4()),
            field_ref,
            provider,
            fetched_at,
            "provider unreachable".to_string(),
        )
        .map_err(weather_ingest_error)?;
        insert_weather_fetch_failure(
            &state,
            &field_id,
            &failure,
            request.latitude,
            request.longitude,
            current_record_timestamp(),
        )
        .await?;
        return Ok((StatusCode::BAD_GATEWAY, Json(failure)).into_response());
    }

    let provider_response =
        sample_weather_provider_response(&provider, fetched_at, request.valid_time)?;
    let records = normalize_weather_provider_forecast(field_ref, provider_response)
        .map_err(weather_ingest_error)?;
    for record in &records {
        let created_at = current_record_timestamp();
        insert_weather_forecast_record(
            &state,
            &field_id,
            record,
            request.latitude,
            request.longitude,
            created_at.clone(),
        )
        .await?;
        insert_weather_time_series_points(&state, &field_id, record, created_at).await?;
    }

    Ok(Json(records).into_response())
}

pub async fn list_weather_forecasts(
    Query(query): Query<WeatherForecastListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<WeatherForecastRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let source = normalize_optional_text(query.source);
    let rows = sqlx::query(
        r#"
        SELECT forecast_id, field_id, field_ref, valid_time, vars_json, source, fetched_at
        FROM weather_forecasts
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR source = ?2)
        ORDER BY valid_time ASC, forecast_id ASC
        "#,
    )
    .bind(field_id)
    .bind(source)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_weather_forecast_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn list_weather_fetch_failures(
    Query(query): Query<WeatherFetchFailureListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<WeatherFetchFailureRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let source = normalize_optional_text(query.source);
    let rows = sqlx::query(
        r#"
        SELECT failure_id, field_id, field_ref, source, fetched_at, reason
        FROM weather_fetch_failures
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR source = ?2)
        ORDER BY fetched_at DESC, failure_id ASC
        "#,
    )
    .bind(field_id)
    .bind(source)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| WeatherFetchFailureRecord {
                failure_id: row.get("failure_id"),
                field_ref: row.get("field_ref"),
                source: row.get("source"),
                fetched_at: row.get("fetched_at"),
                reason: row.get("reason"),
            })
            .collect(),
    ))
}

