//! Time-series + provenance lineage/trace/audit reads route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn list_time_series_points(
    Query(query): Query<TimeSeriesPointListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<TimeSeriesPointResponse>>> {
    let entity_ref = normalize_optional_text(query.entity_ref)
        .ok_or_else(|| AppError::BadRequest("entity_ref is required".to_string()))?;
    let metric = normalize_optional_text(query.metric);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let rows = sqlx::query(
        r#"
        SELECT entity_ref, metric, t, value_kind, scalar_value, source_ref, created_at, metadata_json
        FROM time_series_points
        WHERE entity_ref = ?1
          AND (?2 IS NULL OR metric = ?2)
          AND (?3 IS NULL OR t >= ?3)
          AND (?4 IS NULL OR t <= ?4)
        ORDER BY t ASC, metric ASC, source_ref ASC
        "#,
    )
    .bind(entity_ref)
    .bind(metric)
    .bind(start)
    .bind(end)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    rows.into_iter()
        .map(|row| decode_time_series_point_response(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn list_provenance_lineage_records(
    Query(query): Query<ProvenanceLineageListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ProvenanceLineagePage>> {
    let artifact_id = normalize_optional_text(query.artifact_id);
    let actor_id = normalize_optional_text(query.actor_id);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM provenance_lineage_records
        WHERE (?1 IS NULL OR artifact_id = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR created_at >= ?3)
          AND (?4 IS NULL OR created_at <= ?4)
        "#,
    )
    .bind(&artifact_id)
    .bind(&actor_id)
    .bind(&start)
    .bind(&end)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT artifact_id, kind, inputs_json, method, parameters_json, operator, actor_id,
               actor_kind, created_at
        FROM provenance_lineage_records
        WHERE (?1 IS NULL OR artifact_id = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR created_at >= ?3)
          AND (?4 IS NULL OR created_at <= ?4)
        ORDER BY created_at DESC, artifact_id ASC
        LIMIT ?5 OFFSET ?6
        "#,
    )
    .bind(artifact_id)
    .bind(actor_id)
    .bind(start)
    .bind(end)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let records = rows
        .into_iter()
        .map(|row| decode_lineage_record(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(ProvenanceLineagePage {
        page,
        page_size,
        total: total as usize,
        records,
    }))
}

pub async fn get_provenance_lineage_record(
    Path(artifact_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<LineageRecord>> {
    let artifact_id = normalize_optional_text(Some(artifact_id))
        .ok_or_else(|| AppError::BadRequest("artifact_id is required".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT artifact_id, kind, inputs_json, method, parameters_json, operator, actor_id,
               actor_kind, created_at
        FROM provenance_lineage_records
        WHERE artifact_id = ?1
        "#,
    )
    .bind(artifact_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_lineage_record(&row))
        .transpose()?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// Backward provenance trace for any artifact id (product, finding, report,
/// …): the chain of lineage records down to its L0 sources, plus any gaps.
/// This is the read API behind the workspace provenance inspector.
pub async fn get_provenance_trace(
    Path(artifact_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<BackwardProvenanceTrace>> {
    let artifact_id = normalize_optional_text(Some(artifact_id))
        .ok_or_else(|| AppError::BadRequest("artifact_id is required".to_string()))?;
    let trace = crate::provenance_store::trace_backward(&state.pool, &artifact_id)
        .await
        .map_err(|err| AppError::Anyhow(Error::new(err)))?;
    // An unknown target has no lineage record of its own (it surfaces only as a
    // self-referential gap); treat that as not found.
    if trace.records.is_empty() {
        return Err(AppError::NotFound);
    }
    Ok(Json(trace))
}

pub async fn list_provenance_audit_entries(
    Query(query): Query<ProvenanceAuditListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<ProvenanceAuditPage>> {
    let artifact_id = normalize_optional_text(query.artifact_id);
    let actor_id = normalize_optional_text(query.actor_id);
    let start = normalize_optional_text(query.start);
    let end = normalize_optional_text(query.end);
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM provenance_audit_entries
        WHERE (?1 IS NULL OR artifact_ref = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR ts >= ?3)
          AND (?4 IS NULL OR ts <= ?4)
        "#,
    )
    .bind(&artifact_id)
    .bind(&actor_id)
    .bind(&start)
    .bind(&end)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;

    let rows = sqlx::query(
        r#"
        SELECT entry_hash, seq, prev_hash, payload_hash, actor_id, actor_kind, ts, action_ref,
               action_kind, artifact_ref, payload_json, occurred_at, outcome, refusal_reason
        FROM provenance_audit_entries
        WHERE (?1 IS NULL OR artifact_ref = ?1)
          AND (?2 IS NULL OR actor_id = ?2)
          AND (?3 IS NULL OR ts >= ?3)
          AND (?4 IS NULL OR ts <= ?4)
        ORDER BY ts DESC, seq DESC
        LIMIT ?5 OFFSET ?6
        "#,
    )
    .bind(artifact_id)
    .bind(actor_id)
    .bind(start)
    .bind(end)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let entries = rows
        .into_iter()
        .map(|row| decode_audit_entry(&row))
        .collect::<AppResult<Vec<_>>>()?;

    Ok(Json(ProvenanceAuditPage {
        page,
        page_size,
        total: total as usize,
        entries,
    }))
}

pub async fn get_provenance_audit_entry(
    Path(entry_hash): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<AuditEntry>> {
    let entry_hash = normalize_optional_text(Some(entry_hash))
        .ok_or_else(|| AppError::BadRequest("entry_hash is required".to_string()))?;
    let row = sqlx::query(
        r#"
        SELECT entry_hash, seq, prev_hash, payload_hash, actor_id, actor_kind, ts, action_ref,
               action_kind, artifact_ref, payload_json, occurred_at, outcome, refusal_reason
        FROM provenance_audit_entries
        WHERE entry_hash = ?1
        "#,
    )
    .bind(entry_hash)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_audit_entry(&row))
        .transpose()?
        .map(Json)
        .ok_or(AppError::NotFound)
}
