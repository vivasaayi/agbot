//! Orthomosaic frame-sets / reconstruction / handoff route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn ingest_orthomosaic_frame_set(
    State(state): State<AppState>,
    Json(request): Json<FrameSetIngestRequest>,
) -> AppResult<Json<FrameSetRecord>> {
    validate_orthomosaic_linkage(
        &state,
        &request.scene_id,
        &request.field_id,
        &request.season_id,
    )
    .await?;
    let record = build_frame_set_record(
        request,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(orthomosaic_ingest_error)?;
    let frames_json =
        serde_json::to_string(&record.frames).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO orthomosaic_frame_sets
            (frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.frame_set_id)
    .bind(&record.scene_id)
    .bind(&record.field_id)
    .bind(&record.season_id)
    .bind(frames_json)
    .bind(&record.crs_hint)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(record))
}

pub async fn list_orthomosaic_frame_sets(
    Query(query): Query<OrthomosaicFrameSetListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FrameSetRecord>>> {
    let scene_id = normalize_optional_text(query.scene_id);
    let field_id = normalize_optional_text(query.field_id);
    let rows = match (scene_id, field_id) {
        (Some(scene_id), Some(field_id)) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            WHERE scene_id = ?1 AND field_id = ?2
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .bind(scene_id)
        .bind(field_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
        (Some(scene_id), None) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            WHERE scene_id = ?1
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .bind(scene_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
        (None, Some(field_id)) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            WHERE field_id = ?1
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .bind(field_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
        (None, None) => sqlx::query(
            r#"
            SELECT frame_set_id, scene_id, field_id, season_id, frames_json, crs_hint, created_at
            FROM orthomosaic_frame_sets
            ORDER BY created_at DESC, frame_set_id ASC
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?,
    };

    rows.into_iter()
        .map(|row| decode_orthomosaic_frame_set_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn submit_orthomosaic_reconstruction(
    State(state): State<AppState>,
    Json(request): Json<ReconstructionJobRequest>,
) -> AppResult<Json<ReconstructionJobRecord>> {
    let record = build_reconstruction_job(
        request,
        Uuid::new_v4().to_string(),
        current_record_timestamp(),
    )
    .map_err(reconstruction_job_error)?;
    if !orthomosaic_frame_set_exists(&state, &record.frame_set_id).await? {
        return Err(AppError::BadRequest(format!(
            "frame_set_id {} does not exist",
            record.frame_set_id
        )));
    }
    let params_json =
        serde_json::to_string(&record.params).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO orthomosaic_reconstructions
            (recon_id, frame_set_id, params_json, status, failure_reason, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.recon_id)
    .bind(&record.frame_set_id)
    .bind(params_json)
    .bind(record.status.as_str())
    .bind(&record.failure_reason)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(record))
}

pub async fn get_orthomosaic_reconstruction(
    Path(recon_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<ReconstructionJobRecord>> {
    let record = load_orthomosaic_reconstruction(&state, &recon_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(record))
}

pub async fn update_orthomosaic_reconstruction_status(
    Path(recon_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateReconstructionStatusRequest>,
) -> AppResult<Json<ReconstructionJobRecord>> {
    let record = load_orthomosaic_reconstruction(&state, &recon_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let updated = transition_reconstruction_status(
        record,
        request.status,
        request.failure_reason,
        current_record_timestamp(),
    )
    .map_err(reconstruction_job_error)?;

    sqlx::query(
        r#"
        UPDATE orthomosaic_reconstructions
        SET status = ?2, failure_reason = ?3, updated_at = ?4
        WHERE recon_id = ?1
        "#,
    )
    .bind(&updated.recon_id)
    .bind(updated.status.as_str())
    .bind(&updated.failure_reason)
    .bind(&updated.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(updated))
}

pub async fn handoff_orthomosaic_tiles(
    Path(recon_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<TiledOutputHandoffRequest>,
) -> AppResult<Json<TiledOutputHandoff>> {
    let record = load_orthomosaic_reconstruction(&state, &recon_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if record.status != ReconstructionStatus::Completed {
        return Err(AppError::BadRequest(format!(
            "reconstruction {recon_id} must be completed before tiled handoff"
        )));
    }
    let frame_set = load_orthomosaic_frame_set(&state, &record.frame_set_id)
        .await?
        .ok_or(AppError::NotFound)?;
    request.recon_id = recon_id;
    if normalize_optional_text(Some(request.scene_id.clone())).as_deref()
        != Some(frame_set.scene_id.as_str())
    {
        return Err(AppError::BadRequest(format!(
            "handoff scene_id must match frame set scene_id {}",
            frame_set.scene_id
        )));
    }

    let handoff = build_tiled_output_handoff(request).map_err(tiled_output_handoff_error)?;
    if handoff.recon_id != record.recon_id {
        return Err(AppError::BadRequest(format!(
            "handoff recon_id must match reconstruction {}",
            record.recon_id
        )));
    }

    for layer in &handoff.layers {
        let product_path = PathBuf::from(&layer.uri);
        let exists = fs::try_exists(&product_path)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        if !exists {
            return Err(AppError::BadRequest(format!(
                "product {} output path does not exist: {}",
                layer.product_kind,
                product_path.display()
            )));
        }
        publish_georeferenced_product(
            &state.pool,
            &handoff.scene_id,
            &frame_set.field_id,
            &frame_set.season_id,
            &layer.product_kind,
            &product_path,
            &layer.spatial_ref,
            layer.width_px,
            layer.height_px,
            layer.gsd_m_per_px,
            handoff.source_image_ids.clone(),
            handoff.source_image_ids.clone(),
        )
        .await
        .map_err(|err| {
            if is_product_publish_error(&err) {
                AppError::BadRequest(err.to_string())
            } else {
                AppError::Anyhow(err)
            }
        })?;
    }

    Ok(Json(handoff))
}

pub async fn apply_orthomosaic_publish_gate(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<MosaicPublishGateRequest>,
) -> AppResult<Json<MosaicPublishGateDecision>> {
    let scene_id = normalize_optional_text(Some(scene_id))
        .ok_or_else(|| AppError::BadRequest("scene_id is required".to_string()))?;
    let kind = normalize_optional_text(Some(kind))
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| AppError::BadRequest("product kind is required".to_string()))?;
    let product_exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM products WHERE scene_id = ?1 AND lower(kind) = lower(?2)",
    )
    .bind(&scene_id)
    .bind(&kind)
    .fetch_one(&state.pool)
    .await
    .map_err(Error::from)?;
    if product_exists == 0 {
        return Err(AppError::NotFound);
    }

    let decision = evaluate_mosaic_publish_gate(request).map_err(mosaic_publish_gate_error)?;
    if decision.scene_id != scene_id || decision.product_kind != kind {
        return Err(AppError::BadRequest(format!(
            "publish gate request must target product {scene_id}:{kind}"
        )));
    }
    let downstream_consumers_json = serde_json::to_string(&decision.downstream_consumers)
        .map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        UPDATE products
        SET publish_status = ?3,
            qa_report_ref = ?4,
            provenance_hash = ?5,
            downstream_consumers_json = ?6
        WHERE scene_id = ?1 AND lower(kind) = lower(?2)
        "#,
    )
    .bind(&scene_id)
    .bind(&kind)
    .bind(decision.status.as_str())
    .bind(&decision.qa_report_ref)
    .bind(&decision.provenance_hash)
    .bind(downstream_consumers_json)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(decision))
}

