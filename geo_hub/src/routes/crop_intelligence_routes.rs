//! Crop-intelligence models / inference / detections route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers and domain `*_error` mappers are reached from the parent via
//! `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn register_crop_model(
    State(state): State<AppState>,
    Json(request): Json<ModelVersionRegistrationRequest>,
) -> AppResult<Json<ModelVersionRecord>> {
    let record = build_model_version_record(request, current_record_timestamp())
        .map_err(crop_model_registry_error)?;
    let metrics_json =
        serde_json::to_string(&record.metrics).map_err(|err| AppError::Anyhow(err.into()))?;

    sqlx::query(
        r#"
        INSERT INTO crop_models
            (model_id, version, task, training_set_ref, metrics_json, provenance_ref, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&record.model_id)
    .bind(&record.version)
    .bind(record.task.as_str())
    .bind(&record.training_set_ref)
    .bind(metrics_json)
    .bind(&record.provenance_ref)
    .bind(&record.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(record))
}

pub async fn list_crop_models(
    Query(query): Query<CropModelListQuery>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ModelVersionRecord>>> {
    let task = normalize_optional_text(query.task);
    let rows = if let Some(task) = task {
        let task = parse_crop_model_task(task)?;
        sqlx::query(
            r#"
            SELECT model_id, version, task, training_set_ref, metrics_json, provenance_ref, created_at
            FROM crop_models
            WHERE task = ?1
            ORDER BY created_at DESC, model_id ASC, version ASC
            "#,
        )
        .bind(task.as_str())
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    } else {
        sqlx::query(
            r#"
            SELECT model_id, version, task, training_set_ref, metrics_json, provenance_ref, created_at
            FROM crop_models
            ORDER BY created_at DESC, model_id ASC, version ASC
            "#,
        )
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?
    };

    rows.into_iter()
        .map(|row| decode_crop_model_record(&row))
        .collect::<AppResult<Vec<_>>>()
        .map(Json)
}

pub async fn validate_crop_model_for_inference(
    State(state): State<AppState>,
    Json(reference): Json<InferenceModelReference>,
) -> AppResult<Json<ModelGateResponse>> {
    let model_id = reference.model_id.trim().to_string();
    let version = reference.version.trim().to_string();
    let registered = crop_model_exists(&state, &model_id, &version).await?;
    match validate_model_reference(reference, registered) {
        Ok(response) => Ok(Json(response)),
        Err(CropModelRegistryError::UnregisteredModel { model_id, version }) => {
            audit_crop_model_event(
                &state,
                &model_id,
                &version,
                "unregistered_model_rejected",
                Some("inference request rejected because model version is not registered"),
            )
            .await?;
            Err(AppError::BadRequest(format!(
                "unregistered model {model_id}@{version}"
            )))
        }
        Err(error) => Err(crop_model_registry_error(error)),
    }
}

pub async fn submit_crop_inference_run(
    State(state): State<AppState>,
    Json(request): Json<InferenceRunSubmissionRequest>,
) -> AppResult<Json<InferenceRunRecord>> {
    let model_registered = if let Some(model) = request.model.as_ref() {
        Some(crop_model_exists(&state, model.model_id.trim(), model.version.trim()).await?)
    } else {
        None
    };
    let record = match build_inference_run_record(
        request,
        format!("crop-inference-run-{}", Uuid::new_v4()),
        current_record_timestamp(),
        model_registered,
    ) {
        Ok(record) => record,
        Err(InferenceRunError::ModelGate {
            source: CropModelRegistryError::UnregisteredModel { model_id, version },
        }) => {
            audit_crop_model_event(
                &state,
                &model_id,
                &version,
                "unregistered_model_rejected",
                Some("inference run rejected because model version is not registered"),
            )
            .await?;
            return Err(AppError::BadRequest(format!(
                "unregistered model {model_id}@{version}"
            )));
        }
        Err(error) => return Err(crop_inference_run_error(error)),
    };
    if !crop_inference_mosaic_is_published(
        &state,
        &record.mosaic_ref,
        &record.field_id,
        &record.season_id,
    )
    .await?
    {
        return Err(AppError::BadRequest(format!(
            "mosaic {} is not published and provenance-gated for field {} season {}",
            record.mosaic_ref, record.field_id, record.season_id
        )));
    }
    insert_crop_inference_run(&state, &record).await?;

    Ok(Json(record))
}

pub async fn get_crop_inference_run(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<InferenceRunRecord>> {
    load_crop_inference_run(&state, &run_id)
        .await?
        .ok_or(AppError::NotFound)
        .map(Json)
}

pub async fn get_crop_inference_run_result(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<InferenceRunRecord>> {
    let record = load_crop_inference_run(&state, &run_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if record.status != InferenceRunStatus::Completed {
        return Err(AppError::BadRequest(format!(
            "inference run {run_id} has not completed"
        )));
    }

    Ok(Json(record))
}

pub async fn update_crop_inference_run_status(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateCropInferenceRunStatusRequest>,
) -> AppResult<Json<InferenceRunRecord>> {
    let record = load_crop_inference_run(&state, &run_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let updated = transition_inference_run_status(
        record,
        request.status,
        request.failure_reason_code,
        current_record_timestamp(),
    )
    .map_err(crop_inference_run_error)?;
    update_crop_inference_run(&state, &updated).await?;

    Ok(Json(updated))
}

pub async fn record_crop_inference_run_progress(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
    Json(mut request): Json<InferenceRunProgressInput>,
) -> AppResult<Json<InferenceRunProgressRecord>> {
    if load_crop_inference_run(&state, &run_id).await?.is_none() {
        return Err(AppError::NotFound);
    }
    request.run_id = run_id;
    let progress =
        build_inference_run_progress_record(request, format!("crop-progress-{}", Uuid::new_v4()))
            .map_err(crop_inference_run_error)?;
    insert_crop_inference_progress(&state, &progress).await?;

    Ok(Json(progress))
}

pub async fn list_crop_inference_run_progress(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<InferenceRunProgressStream>> {
    if load_crop_inference_run(&state, &run_id).await?.is_none() {
        return Err(AppError::NotFound);
    }
    let events = load_crop_inference_progress(&state, &run_id).await?;
    let stream = inference_run_progress_stream(run_id, events).map_err(crop_inference_run_error)?;

    Ok(Json(stream))
}

pub async fn check_crop_inference_run_stall(
    Path(run_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CropInferenceStallCheckRequest>,
) -> AppResult<Json<Option<InferenceRunStallEvent>>> {
    if load_crop_inference_run(&state, &run_id).await?.is_none() {
        return Err(AppError::NotFound);
    }
    let events = load_crop_inference_progress(&state, &run_id).await?;
    let stream =
        inference_run_progress_stream(run_id.clone(), events).map_err(crop_inference_run_error)?;
    let stall = detect_inference_run_stall(
        stream.latest.as_ref(),
        run_id,
        format!("crop-stall-{}", Uuid::new_v4()),
        request.detected_at,
        request.stall_window_seconds,
    )
    .map_err(crop_inference_run_error)?;
    if let Some(stall) = stall.as_ref() {
        insert_crop_inference_stall_event(&state, stall).await?;
    }

    Ok(Json(stall))
}

pub async fn verify_crop_detection(
    Path(detection_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<VerifyCropDetectionRequest>,
) -> AppResult<Json<CropDetectionVerificationRecord>> {
    let record = apply_detection_verification(CropDetectionVerificationRequest {
        detection_id,
        task: request.task,
        label: request.label,
        confidence: request.confidence,
        evidence_tile_refs: request.evidence_tile_refs,
        zone_geometry: request.zone_geometry,
        action: request.action,
        actor: request.actor,
        verified_at: request.verified_at,
        corrected_label: request.corrected_label,
        corrected_geometry: request.corrected_geometry,
    })
    .map_err(crop_detection_verification_error)?;

    persist_crop_detection_verification(&state, &record).await?;

    Ok(Json(record))
}

pub async fn validate_crop_detection_finding_promotion(
    Path(detection_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CropFindingPromotionValidationRequest>,
) -> AppResult<Json<FindingPromotionDecision>> {
    let verification_state = load_crop_detection_verification_state(&state, &detection_id)
        .await?
        .unwrap_or_default();
    let decision = validate_detection_finding_promotion(FindingPromotionRequest {
        detection_id,
        verification_state,
        allow_unverified: request.allow_unverified,
    })
    .map_err(finding_promotion_error)?;

    Ok(Json(decision))
}

pub async fn emit_crop_detection_finding(
    Path((scene_id, detection_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<EmitCropDetectionFindingRequest>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }
    let field_id = load_scene_field_id(&state, &scene_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "scene {scene_id} must be linked to a field before emitting crop findings"
            ))
        })?;
    let detection = load_crop_detection_verification_record(&state, &detection_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "verified detection {detection_id} was not found for finding emission"
            ))
        })?;
    let finding = assemble_detection_finding(CropDetectionFindingRequest {
        finding_id: request.finding_id,
        field_id,
        zone_id: request.zone_id,
        detection,
        model: InferenceModelReference {
            model_id: request.model_id,
            version: request.version,
        },
        emitted_at: request.emitted_at,
    })
    .map_err(crop_detection_finding_error)?;

    let annotation = annotation_from_crop_detection_finding(&scene_id, &finding)?;
    let recommendation =
        recommendation_from_crop_detection_finding(&scene_id, &finding, &annotation);
    persist_crop_detection_finding_recommendation(&state, &annotation, &recommendation).await?;

    Ok(Json(recommendation))
}

pub async fn create_crop_closed_loop_proposal(
    State(state): State<AppState>,
    Json(request): Json<CropClosedLoopProposalRequest>,
) -> AppResult<Json<CropClosedLoopProposal>> {
    let proposal =
        build_crop_closed_loop_proposal(request).map_err(crop_closed_loop_proposal_error)?;
    persist_crop_closed_loop_proposal(&state, &proposal).await?;

    Ok(Json(proposal))
}

pub async fn get_crop_closed_loop_proposal(
    Path(proposal_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<CropClosedLoopProposal>> {
    let proposal = load_crop_closed_loop_proposal(&state, &proposal_id)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(proposal))
}
