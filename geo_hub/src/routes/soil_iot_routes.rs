//! Soil IoT / moisture / drought-index route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn register_soil_iot_device(
    State(state): State<AppState>,
    Json(request): Json<RegisterSoilDeviceRequest>,
) -> AppResult<Json<SoilDeviceRecord>> {
    let record = build_soil_device_record(
        request,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(soil_iot_error)?;

    insert_soil_iot_device(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_soil_iot_devices(
    Query(query): Query<SoilDeviceListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilDeviceRecord>>> {
    let org_id = normalize_optional_text(query.org_id);
    let field_id = normalize_optional_text(query.field_id);
    let zone_id = normalize_optional_text(query.zone_id);
    let status = normalize_optional_text(query.status)
        .map(parse_soil_device_status)
        .transpose()?
        .map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT device_id, org_id, field_id, zone_id, sensor_type, latitude, longitude, crs,
               calibration_profile_ref, status, created_at, updated_at
        FROM soil_iot_devices
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR zone_id = ?3)
          AND (?4 IS NULL OR status = ?4)
        ORDER BY updated_at DESC, device_id ASC
        "#,
    )
    .bind(org_id)
    .bind(field_id)
    .bind(zone_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_iot_device(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn record_soil_iot_config_push(
    Path(device_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<SoilDeviceConfigPushRequest>,
) -> AppResult<Json<SoilDeviceConfigPushRecord>> {
    let device_id = normalize_optional_text(Some(device_id))
        .ok_or_else(|| AppError::BadRequest("device_id cannot be empty".to_string()))?;
    if let Some(body_device_id) = normalize_optional_text(Some(request.device_id.clone())) {
        if body_device_id != device_id {
            return Err(AppError::BadRequest(format!(
                "request device_id {} does not match path device_id {}",
                body_device_id, device_id
            )));
        }
    }
    load_soil_iot_device(&state, &device_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("device {device_id} is not registered")))?;

    request.device_id = device_id;
    let record = build_soil_config_push_record(request, Uuid::new_v4().to_string())
        .map_err(soil_iot_error)?;
    insert_soil_iot_config_push(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_soil_iot_config_pushes(
    Path(device_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilDeviceConfigPushRecord>>> {
    let device_id = normalize_optional_text(Some(device_id))
        .ok_or_else(|| AppError::BadRequest("device_id cannot be empty".to_string()))?;
    load_soil_iot_device(&state, &device_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("device {device_id} is not registered")))?;

    let rows = sqlx::query(
        r#"
        SELECT push_id, device_id, config_version, pushed_at, push_status, failure_reason, updated_at
        FROM soil_iot_config_pushes
        WHERE device_id = ?1
        ORDER BY pushed_at ASC, push_id ASC
        "#,
    )
    .bind(&device_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_iot_config_push(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn update_soil_iot_config_push_status(
    Path((device_id, push_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut request): Json<SoilDeviceConfigPushStatusUpdate>,
) -> AppResult<Json<SoilDeviceConfigPushRecord>> {
    let device_id = normalize_optional_text(Some(device_id))
        .ok_or_else(|| AppError::BadRequest("device_id cannot be empty".to_string()))?;
    let push_id = normalize_optional_text(Some(push_id))
        .ok_or_else(|| AppError::BadRequest("push_id cannot be empty".to_string()))?;
    let record = load_soil_iot_config_push(&state, &push_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("config push {push_id} is not registered")))?;
    if record.device_id != device_id {
        return Err(AppError::BadRequest(format!(
            "config push {} belongs to device {}",
            push_id, record.device_id
        )));
    }

    if normalize_optional_text(Some(request.updated_at.clone())).is_none() {
        request.updated_at = current_record_timestamp();
    }
    let updated = transition_soil_config_push_status(&record, request).map_err(soil_iot_error)?;
    update_soil_iot_config_push(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn ingest_soil_iot_reading(
    State(state): State<AppState>,
    Json(request): Json<GatewayReadingRecord>,
) -> AppResult<Json<GeolocatedSoilReading>> {
    let device = load_soil_iot_device(&state, &request.device_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!("device {} is not registered", request.device_id))
        })?;
    let reading = build_geolocated_soil_reading(&device, request).map_err(gateway_ingest_error)?;
    if !reading.excluded_from_geospatial_products {
        let metadata = soil_reading_time_series_metadata(&reading)?;
        insert_time_series_point_record(&state, &reading.to_series_point(), Some(metadata)).await?;
    }

    Ok(Json(reading))
}

pub async fn ingest_soil_moisture_reading(
    State(state): State<AppState>,
    Json(request): Json<SoilMoistureReadingRequest>,
) -> AppResult<Response> {
    let ingested_at = current_record_timestamp();
    let field_id = match normalize_optional_text(request.field_id.clone()) {
        Some(field_id) => field_id,
        None => {
            return reject_soil_moisture_reading(
                &state,
                &request,
                SoilMoistureRejectionReason::MissingFieldLinkage,
                ingested_at,
            )
            .await;
        }
    };

    let Some(field) = load_field(&state, &field_id).await? else {
        return reject_soil_moisture_reading(
            &state,
            &request,
            SoilMoistureRejectionReason::FieldNotFound,
            ingested_at,
        )
        .await;
    };

    let record = match build_soil_moisture_reading(
        request.clone(),
        &field,
        format!("water-moisture-{}", Uuid::new_v4()),
        ingested_at.clone(),
    ) {
        Ok(record) => record,
        Err(error) => {
            return reject_soil_moisture_reading(
                &state,
                &request,
                soil_moisture_rejection_reason_for_error(&error),
                ingested_at,
            )
            .await;
        }
    };

    insert_soil_moisture_reading(&state, &record).await?;
    insert_soil_moisture_time_series_point(&state, &record).await?;

    Ok(Json(record).into_response())
}

pub async fn list_soil_moisture_readings(
    Query(query): Query<SoilMoistureReadingListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilMoistureReadingRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let zone_ref = normalize_optional_text(query.zone_ref);
    let source = normalize_optional_text(query.source);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT reading_id, field_id, zone_ref, value, source, captured_at, qa_flag, ingested_at
        FROM water_moisture_readings
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR zone_ref = ?2)
          AND (?3 IS NULL OR source = ?3)
          AND (?4 IS NULL OR captured_at >= ?4)
          AND (?5 IS NULL OR captured_at <= ?5)
        ORDER BY captured_at ASC, reading_id ASC
        "#,
    )
    .bind(field_id)
    .bind(zone_ref)
    .bind(source)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_moisture_reading(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn list_soil_moisture_rejections(
    Query(query): Query<SoilMoistureRejectionListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SoilMoistureRejectionRecord>>> {
    let field_id = normalize_optional_text(query.field_id);
    let reason = query.reason.map(|reason| reason.as_str().to_string());
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT rejection_id, reading_id, field_id, zone_ref, source, captured_at, reason, rejected_at
        FROM water_moisture_reading_rejections
        WHERE (?1 IS NULL OR field_id = ?1)
          AND (?2 IS NULL OR reason = ?2)
          AND (?3 IS NULL OR rejected_at >= ?3)
          AND (?4 IS NULL OR rejected_at <= ?4)
        ORDER BY rejected_at ASC, rejection_id ASC
        "#,
    )
    .bind(field_id)
    .bind(reason)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_soil_moisture_rejection(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

async fn reject_soil_moisture_reading(
    state: &AppState,
    request: &SoilMoistureReadingRequest,
    reason: SoilMoistureRejectionReason,
    rejected_at: String,
) -> AppResult<Response> {
    let rejection = soil_moisture_rejection_record(
        format!("water-moisture-rejection-{}", Uuid::new_v4()),
        request,
        reason,
        rejected_at,
    )
    .map_err(soil_moisture_error)?;
    insert_soil_moisture_rejection(state, &rejection).await?;
    Ok((StatusCode::BAD_REQUEST, Json(rejection)).into_response())
}

pub async fn compute_drought_index_route(
    State(state): State<AppState>,
    Json(request): Json<DroughtIndexComputeRequest>,
) -> AppResult<Json<DroughtIndexRecord>> {
    validate_drought_scope_ref(&state, &request.field_or_region_ref).await?;
    let record = compute_drought_index(
        request,
        format!("drought-index-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(drought_index_error)?;

    insert_drought_index_record(&state, &record, current_record_timestamp()).await?;
    insert_drought_index_time_series_point(&state, &record).await?;

    Ok(Json(record))
}

pub async fn list_drought_indices(
    Query(query): Query<DroughtIndexListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<DroughtIndexRecord>>> {
    let field_or_region_ref = normalize_optional_text(query.field_or_region_ref);
    let index_type = query
        .index_type
        .map(|index_type| index_type.as_str().to_string());
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT index_id, field_or_region_ref, index_type, value, period_start, period_end,
               accumulation_days, input_refs_json, method, computed_at
        FROM drought_indices
        WHERE (?1 IS NULL OR field_or_region_ref = ?1)
          AND (?2 IS NULL OR index_type = ?2)
          AND (?3 IS NULL OR period_end >= ?3)
          AND (?4 IS NULL OR period_start <= ?4)
        ORDER BY period_end ASC, index_id ASC
        "#,
    )
    .bind(field_or_region_ref)
    .bind(index_type)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_drought_index_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}



