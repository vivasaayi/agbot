//! Fleet nodes / tractors route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn enroll_fleet_node(
    State(state): State<AppState>,
    Json(request): Json<FleetNodeEnrollmentRequest>,
) -> AppResult<Json<FleetNodeRecord>> {
    let binding = bind_fleet_node_identity(
        request.clone(),
        None,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(fleet_enrollment_error)?;
    let record = binding.record;
    let capabilities_json =
        serde_json::to_string(&record.capabilities).map_err(|err| AppError::Anyhow(err.into()))?;

    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO fleet_nodes
            (node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&record.node_id)
    .bind(&record.hardware_id)
    .bind(record.kind.as_str())
    .bind(capabilities_json)
    .bind(&record.owner_org_id)
    .bind(record.runtime_mode.as_str())
    .bind(&record.enrolled_at)
    .bind(record.status.as_str())
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    if result.rows_affected() == 0 {
        let existing = load_fleet_node_by_hardware_id(&state, &record.hardware_id)
            .await?
            .ok_or_else(|| AppError::Anyhow(anyhow::anyhow!("fleet node conflict not found")))?;
        let binding =
            bind_fleet_node_identity(request, Some(existing), record.node_id, record.enrolled_at)
                .map_err(fleet_enrollment_error)?;
        return Ok(Json(binding.record));
    }

    Ok(Json(record))
}

pub async fn list_fleet_nodes(
    Query(query): Query<FleetNodeListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FleetNodeRecord>>> {
    let owner_org_id = normalize_optional_text(query.owner_org_id);
    let rows = if let Some(owner_org_id) = owner_org_id {
        sqlx::query(
            r#"
            SELECT node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status
            FROM fleet_nodes
            WHERE owner_org_id = ?1
            ORDER BY enrolled_at DESC, node_id ASC
            "#,
        )
        .bind(owner_org_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    } else {
        sqlx::query(
            r#"
            SELECT node_id, hardware_id, kind, capabilities_json, owner_org_id, runtime_mode, enrolled_at, status
            FROM fleet_nodes
            ORDER BY enrolled_at DESC, node_id ASC
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    };

    rows.into_iter()
        .map(|row| decode_fleet_node_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_fleet_node(
    Path(node_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FleetNodeRecord>> {
    let node = load_fleet_node(&state, &node_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(node))
}

pub async fn register_tractor(
    State(state): State<AppState>,
    Json(mut request): Json<TractorRegistrationRequest>,
) -> AppResult<Json<TractorRecord>> {
    if request
        .tractor_id
        .as_ref()
        .is_none_or(|tractor_id| tractor_id.trim().is_empty())
    {
        request.tractor_id = Some(Uuid::new_v4().to_string());
    }

    let field_id = normalize_optional_text(Some(request.field_id.clone()))
        .ok_or_else(|| AppError::BadRequest("tractor field_id is required".to_string()))?;
    let field = load_field(&state, &field_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("field {field_id} does not exist")))?;
    let record = build_tractor_record(request, &field, current_record_timestamp())
        .map_err(tractor_registry_error)?;
    if load_tractor(&state, &record.tractor_id).await?.is_some() {
        return Err(AppError::BadRequest(format!(
            "tractor {} is already registered",
            record.tractor_id
        )));
    }
    insert_tractor_record(&state, &record).await?;
    Ok(Json(record))
}

pub async fn list_tractors(
    Query(query): Query<TractorListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<TractorRecord>>> {
    let org_id = normalize_optional_text(query.org_id);
    let field_id = normalize_optional_text(query.field_id);
    let status = query.status.map(|status| status.as_str().to_string());
    let rows = sqlx::query(
        r#"
        SELECT tractor_id, org_id, field_id, capabilities_json, implement_ref_json, status,
               registered_at, updated_at
        FROM tractor_vehicles
        WHERE (?1 IS NULL OR org_id = ?1)
          AND (?2 IS NULL OR field_id = ?2)
          AND (?3 IS NULL OR status = ?3)
        ORDER BY tractor_id ASC
        "#,
    )
    .bind(org_id)
    .bind(field_id)
    .bind(status)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_tractor_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn get_tractor(
    Path(tractor_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<TractorRecord>> {
    let tractor = load_tractor(&state, &tractor_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(tractor))
}

pub async fn validate_tractor_motion_command(
    Path(tractor_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<TractorMotionCommandValidationRequest>,
) -> AppResult<Response> {
    let command_type = normalize_optional_text(Some(request.command_type))
        .ok_or_else(|| AppError::BadRequest("tractor command_type is required".to_string()))?;
    let command = TractorMotionCommandRequest {
        command_id: normalize_optional_text(request.command_id),
        tractor_id: tractor_id.clone(),
        command_type,
        requested_by: normalize_optional_text(request.requested_by),
    };

    let Some(tractor) = load_tractor(&state, &tractor_id).await? else {
        let audit = build_tractor_command_audit(
            &command,
            None,
            TractorCommandRejectionReason::UnknownTractor,
        );
        insert_tractor_command_audit(&state, &audit).await?;
        let rejection = TractorCommandRejection {
            tractor_id,
            reason: TractorCommandRejectionReason::UnknownTractor,
            status: None,
            audit,
        };
        return Ok((tractor_rejection_status(&rejection), Json(rejection)).into_response());
    };

    if tractor.status == TractorLifecycleStatus::OutOfService {
        let audit = build_tractor_command_audit(
            &command,
            Some(&tractor),
            TractorCommandRejectionReason::TractorOutOfService,
        );
        insert_tractor_command_audit(&state, &audit).await?;
        let rejection = TractorCommandRejection {
            tractor_id,
            reason: TractorCommandRejectionReason::TractorOutOfService,
            status: Some(tractor.status),
            audit,
        };
        return Ok((tractor_rejection_status(&rejection), Json(rejection)).into_response());
    }

    Ok(Json(tractor).into_response())
}
