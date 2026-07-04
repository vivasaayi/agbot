//! Fleet-health components / duty / rollout route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn register_fleet_component(
    State(state): State<AppState>,
    Json(request): Json<RegisterComponentRequest>,
) -> AppResult<Json<FleetComponentRecord>> {
    let record = build_component_record(
        request,
        format!("fleet-component-{}", Uuid::new_v4()),
        current_record_timestamp(),
    )
    .map_err(fleet_health_error)?;

    if let Some(airframe_id) = &record.airframe_id {
        validate_enrolled_airframe(&state, airframe_id).await?;
    }

    insert_fleet_component(&state, &record).await?;
    append_fleet_component_event(
        &state,
        &component_event(
            &record.component_id,
            "registered",
            record.airframe_id.clone(),
            record.created_at.clone(),
            None,
            Some(format!("serial {}", record.serial)),
        )
        .map_err(fleet_health_error)?,
    )
    .await?;
    if let (Some(airframe_id), Some(installed_at)) = (&record.airframe_id, &record.installed_at) {
        append_fleet_component_event(
            &state,
            &component_event(
                &record.component_id,
                "installed",
                Some(airframe_id.clone()),
                installed_at.clone(),
                None,
                Some("initial install".to_string()),
            )
            .map_err(fleet_health_error)?,
        )
        .await?;
    }
    for service in &record.service_history {
        append_fleet_component_event(
            &state,
            &component_event(
                &record.component_id,
                "service_recorded",
                record.airframe_id.clone(),
                service.performed_at.clone(),
                Some(service.technician.clone()),
                Some(service.action.clone()),
            )
            .map_err(fleet_health_error)?,
        )
        .await?;
    }

    Ok(Json(record))
}

pub async fn list_fleet_components(
    Query(query): Query<FleetComponentListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetComponentRecord>>> {
    let airframe_id = normalize_optional_text(query.airframe_id);
    let component_type = normalize_optional_text(query.component_type)
        .map(parse_fleet_component_type)
        .transpose()?
        .map(|component_type| component_type.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT component_id, component_type, serial, airframe_id, installed_at, removed_at,
               service_history_json, flight_hours, cycles, duty_score, created_at, updated_at
        FROM fleet_components
        WHERE (?1 IS NULL OR airframe_id = ?1)
          AND (?2 IS NULL OR component_type = ?2)
        ORDER BY updated_at DESC, component_id ASC
        "#,
    )
    .bind(airframe_id)
    .bind(component_type)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_fleet_component_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_fleet_component_history(
    Path(component_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetComponentEventRecord>>> {
    load_fleet_component(&state, &component_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let rows = sqlx::query(
        r#"
        SELECT component_id, event_type, airframe_id, event_at, actor, details
        FROM fleet_component_events
        WHERE component_id = ?1
        ORDER BY event_at ASC, id ASC
        "#,
    )
    .bind(component_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_fleet_component_event(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn install_fleet_component_route(
    Path(component_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<InstallComponentRequest>,
) -> AppResult<Json<FleetComponentRecord>> {
    let component_id = normalize_optional_text(Some(component_id))
        .ok_or_else(|| AppError::BadRequest("component_id is required".to_string()))?;
    let existing = load_fleet_component(&state, &component_id)
        .await?
        .ok_or(AppError::NotFound)?;
    validate_enrolled_airframe(&state, request.airframe_id.trim()).await?;
    let attempted_airframe = request.airframe_id.trim().to_string();
    let attempted_at = normalize_optional_text(Some(request.installed_at.clone()))
        .unwrap_or_else(current_record_timestamp);
    let actor = request.actor.clone();

    let updated = match install_component(&existing, request, current_record_timestamp()) {
        Ok(updated) => updated,
        Err(FleetHealthError::AlreadyInstalled { .. }) => {
            append_fleet_component_event(
                &state,
                &component_event(
                    &component_id,
                    "double_install_rejected",
                    Some(attempted_airframe),
                    attempted_at,
                    actor,
                    Some("component already installed on another airframe".to_string()),
                )
                .map_err(fleet_health_error)?,
            )
            .await?;
            return Err(fleet_health_error(FleetHealthError::AlreadyInstalled {
                component_id: existing.component_id,
                airframe_id: existing.airframe_id.unwrap_or_default(),
            }));
        }
        Err(error) => return Err(fleet_health_error(error)),
    };

    update_fleet_component_install(&state, &updated).await?;
    append_fleet_component_event(
        &state,
        &component_event(
            &updated.component_id,
            "installed",
            updated.airframe_id.clone(),
            updated
                .installed_at
                .clone()
                .unwrap_or_else(current_record_timestamp),
            actor,
            Some("component installed".to_string()),
        )
        .map_err(fleet_health_error)?,
    )
    .await?;

    Ok(Json(updated))
}

pub async fn accrue_fleet_component_duty(
    State(state): State<AppState>,
    Json(request): Json<DutyAccrualRequest>,
) -> AppResult<Json<Vec<ComponentDutyAccrualRecord>>> {
    validate_enrolled_airframe(&state, request.airframe_id.trim()).await?;
    let components =
        load_active_fleet_components_for_airframe(&state, request.airframe_id.trim()).await?;
    let component_ids = components
        .iter()
        .map(|component| component.component_id.clone())
        .collect::<Vec<_>>();
    let accruals =
        build_component_duty_accruals(request, &component_ids).map_err(fleet_health_error)?;

    for accrual in &accruals {
        let inserted = insert_component_duty_accrual(&state, accrual).await?;
        if inserted {
            if let Some(component) = components
                .iter()
                .find(|component| component.component_id == accrual.component_id)
            {
                let updated = accrue_component_duty(component, accrual, current_record_timestamp())
                    .map_err(fleet_health_error)?;
                update_fleet_component_duty_totals(&state, &updated).await?;
                append_fleet_component_event(
                    &state,
                    &component_event(
                        &updated.component_id,
                        "duty_accrued",
                        updated.airframe_id.clone(),
                        accrual.accrued_at.clone(),
                        None,
                        Some(format!("session {}", accrual.session_id)),
                    )
                    .map_err(fleet_health_error)?,
                )
                .await?;
            }
        }
    }

    let session_id = accruals
        .first()
        .map(|accrual| accrual.session_id.clone())
        .unwrap_or_default();
    let airframe_id = accruals
        .first()
        .map(|accrual| accrual.airframe_id.clone())
        .unwrap_or_default();
    let persisted = if session_id.is_empty() {
        Vec::new()
    } else {
        load_component_duty_accruals_for_session(&state, &session_id, &airframe_id).await?
    };

    Ok(Json(persisted))
}

pub async fn derive_fleet_health_indicators_route(
    State(state): State<AppState>,
    Json(mut request): Json<TelemetryHealthIndicatorRequest>,
) -> AppResult<Json<FleetHealthIndicatorDerivation>> {
    let component_ids = request
        .samples
        .iter()
        .filter_map(|sample| normalize_optional_text(Some(sample.component_id.clone())))
        .chain(
            request
                .telemetry_gaps
                .iter()
                .filter_map(|gap| normalize_optional_text(Some(gap.component_id.clone()))),
        )
        .collect::<BTreeSet<_>>();
    let mut components = BTreeMap::new();
    for component_id in component_ids {
        let component = load_fleet_component(&state, &component_id)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("component {component_id} does not exist"))
            })?;
        components.insert(component.component_id.clone(), component);
    }
    for sample in &mut request.samples {
        if let Some(component_id) = normalize_optional_text(Some(sample.component_id.clone())) {
            if let Some(component) = components.get(&component_id) {
                sample.component_id = component.component_id.clone();
                sample.component_type = component.component_type;
            }
        }
    }
    for gap in &mut request.telemetry_gaps {
        if let Some(component_id) = normalize_optional_text(Some(gap.component_id.clone())) {
            if let Some(component) = components.get(&component_id) {
                gap.component_id = component.component_id.clone();
            }
        }
    }

    let derived = derive_health_indicators(request).map_err(fleet_health_error)?;
    for sample in &derived.samples {
        let airframe_id = components
            .get(&sample.component_id)
            .and_then(|component| component.airframe_id.as_deref());
        insert_fleet_health_indicator_sample(&state, sample, airframe_id).await?;
        insert_time_series_point(&state, sample).await?;
    }
    for gap in &derived.gaps {
        let airframe_id = components
            .get(&gap.component_id)
            .and_then(|component| component.airframe_id.as_deref());
        insert_fleet_health_telemetry_gap(&state, gap, airframe_id, &derived).await?;
    }

    Ok(Json(derived))
}

pub async fn ingest_tractor_fleet_health(
    Path(tractor_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<GroundVehicleHealthIngestRequest>,
) -> AppResult<Json<GroundVehicleHealthIntegration>> {
    load_tractor(&state, &tractor_id)
        .await?
        .ok_or(AppError::NotFound)?;
    request.vehicle_id = tractor_id.clone();
    let integration = integrate_ground_vehicle_health(request).map_err(fleet_health_error)?;
    upsert_fleet_component(&state, &integration.component).await?;
    append_fleet_component_event(
        &state,
        &component_event(
            &integration.component.component_id,
            "ground_vehicle_health_ingested",
            Some(tractor_id),
            integration.readiness_decision.checked_at.clone(),
            None,
            Some("tractor health ingested from domain 14".to_string()),
        )
        .map_err(fleet_health_error)?,
    )
    .await?;

    Ok(Json(integration))
}

pub async fn list_fleet_health_indicators(
    Query(query): Query<FleetHealthIndicatorListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetHealthIndicatorSample>>> {
    let component_id = normalize_optional_text(query.component_id);
    let indicator = normalize_optional_text(query.indicator)
        .map(parse_fleet_health_indicator)
        .transpose()?
        .map(|indicator| indicator.as_str().to_string());
    let freshness = normalize_optional_text(query.freshness)
        .map(parse_health_indicator_freshness)
        .transpose()?
        .map(|freshness| freshness.as_str().to_string());

    let rows = sqlx::query(
        r#"
        SELECT component_id, indicator, value, ts, source_ref, freshness, created_at
        FROM fleet_health_indicator_samples
        WHERE (?1 IS NULL OR component_id = ?1)
          AND (?2 IS NULL OR indicator = ?2)
          AND (?3 IS NULL OR freshness = ?3)
        ORDER BY ts ASC, component_id ASC, indicator ASC
        "#,
    )
    .bind(component_id)
    .bind(indicator)
    .bind(freshness)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_fleet_health_indicator_sample(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn evaluate_ota_rollout_route(
    Json(request): Json<OtaRolloutRequest>,
) -> AppResult<Json<OtaRolloutDecision>> {
    evaluate_ota_rollout(request)
        .map(Json)
        .map_err(fleet_health_error)
}

pub async fn apply_rollout_control_route(
    Json(request): Json<RolloutControlRequest>,
) -> AppResult<Json<RolloutControlDecision>> {
    apply_rollout_control(request)
        .map(Json)
        .map_err(fleet_health_error)
}

