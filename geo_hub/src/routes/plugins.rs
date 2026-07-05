//! Plugin registry / execution route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn register_plugin(
    State(state): State<AppState>,
    Json(manifest): Json<RawPluginManifest>,
) -> AppResult<Json<PluginRegistrationRecord>> {
    let mut host = PluginHost::default();
    let record = host
        .register_plugin(manifest)
        .map_err(plugin_registration_error)?;
    insert_plugin_registration(&state, &record, current_record_timestamp()).await?;
    Ok(Json(record))
}

pub async fn list_plugins(
    Query(query): Query<PluginListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<PluginRegistrationPage>> {
    let kind = query.kind.map(|kind| kind.as_str().to_string());
    let status = query.status.map(|status| status.as_str().to_string());
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM plugin_registrations
        WHERE (?1 IS NULL OR kind = ?1)
          AND (?2 IS NULL OR status = ?2)
        "#,
    )
    .bind(&kind)
    .bind(&status)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT plugin_id, name, version, kind, host_api_version, capabilities_json, entrypoint,
               status
        FROM plugin_registrations
        WHERE (?1 IS NULL OR kind = ?1)
          AND (?2 IS NULL OR status = ?2)
        ORDER BY updated_at DESC, plugin_id ASC
        LIMIT ?3 OFFSET ?4
        "#,
    )
    .bind(kind)
    .bind(status)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let plugins = rows
        .into_iter()
        .map(|row| decode_plugin_registration(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(PluginRegistrationPage {
        page,
        page_size,
        total: total as usize,
        plugins,
    }))
}

pub async fn update_plugin_status(
    Path(plugin_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<PluginStatusUpdateRequest>,
) -> AppResult<Json<PluginRegistrationRecord>> {
    let plugin_id = normalize_optional_text(Some(plugin_id))
        .ok_or_else(|| AppError::BadRequest("plugin_id is required".to_string()))?;
    let current = load_plugin_registration(&state, &plugin_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let occurred_at =
        normalize_optional_text(request.occurred_at).unwrap_or_else(current_record_timestamp);
    let actor_kind = request.actor_kind.unwrap_or(ActorKind::PlatformAdmin);
    let mut host =
        PluginHost::with_registration_records(vec![current]).map_err(plugin_registration_error)?;
    let (updated, audit) = host
        .transition_plugin_status(
            &plugin_id,
            PluginLifecycleTransitionRequest {
                status: request.status,
                actor_id: request.actor_id,
                occurred_at,
            },
            format!("plugin-lifecycle-audit-{}", Uuid::new_v4()),
        )
        .map_err(plugin_lifecycle_error)?;

    update_plugin_registration_status(&state, &updated, &audit.occurred_at).await?;
    insert_plugin_lifecycle_audit(&state, &audit).await?;
    append_plugin_lifecycle_provenance_audit(&state, &audit, actor_kind).await?;

    Ok(Json(updated))
}

pub async fn execute_plugin(
    Path(plugin_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<PluginExecutionRequest>,
) -> AppResult<Json<SandboxExecutionOutcome>> {
    let plugin_id = normalize_optional_text(Some(plugin_id))
        .ok_or_else(|| AppError::BadRequest("plugin_id is required".to_string()))?;
    let current = load_plugin_registration(&state, &plugin_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut host =
        PluginHost::with_registration_records(vec![current]).map_err(plugin_registration_error)?;
    let limits = request.limits.unwrap_or(PluginExecutionLimits {
        max_runtime_ms: 1_000,
        max_memory_mb: 512,
    });
    let attempted_at =
        normalize_optional_text(request.attempted_at).unwrap_or_else(current_record_timestamp);
    let outcome = host.execute_sandboxed(
        PluginExecutionPlan {
            plugin_id: plugin_id.clone(),
            required_capabilities: request.required_capabilities,
            estimated_runtime_ms: request.estimated_runtime_ms,
            estimated_memory_mb: request.estimated_memory_mb,
            result: request
                .result
                .unwrap_or_else(|| "plugin execution complete".to_string()),
        },
        limits,
        &attempted_at,
    );
    if outcome.status == SandboxExecutionStatus::Terminated
        && outcome.termination_reason == Some(SandboxTerminationReason::PluginNotEnabled)
    {
        return Err(AppError::Forbidden(format!(
            "plugin {plugin_id} is not enabled"
        )));
    }

    Ok(Json(outcome))
}

