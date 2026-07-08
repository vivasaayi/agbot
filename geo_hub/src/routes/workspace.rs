//! Annotations / recommendations / reports + exports route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers (decode_*, load_*, scene_exists, build_*_record) and domain
//! `*_error` mappers stay in the parent module and are reached via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn list_scene_annotations(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AnnotationRecord>>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity, geometry_json, created_at, updated_at
        FROM annotations
        WHERE scene_id = ?1
        ORDER BY created_at ASC
        "#,
    )
    .bind(&scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut annotations = Vec::with_capacity(rows.len());
    for row in rows {
        annotations.push(decode_annotation_record(&row)?);
    }

    Ok(Json(annotations))
}

pub async fn create_scene_annotation(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CreateAnnotationRequest>,
) -> AppResult<Json<AnnotationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let annotation = build_annotation_record(&state, &scene_id, request).await?;
    sqlx::query(
        r#"
        INSERT INTO annotations (
            annotation_id, scene_id, field_id, author, crs, audit_id, label, note, severity, geometry_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&annotation.annotation_id)
    .bind(&annotation.scene_id)
    .bind(&annotation.field_id)
    .bind(&annotation.author)
    .bind(&annotation.crs)
    .bind(&annotation.audit_id)
    .bind(&annotation.label)
    .bind(&annotation.note)
    .bind(&annotation.severity)
    .bind(
        serde_json::to_string(&annotation.geometry).map_err(|err| AppError::Anyhow(err.into()))?,
    )
    .bind(&annotation.created_at)
    .bind(&annotation.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(annotation))
}

pub async fn update_scene_annotation(
    Path((scene_id, annotation_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<UpdateAnnotationRequest>,
) -> AppResult<Json<AnnotationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let existing = load_annotation(&state, &scene_id, &annotation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    validate_annotation_geometry(&request.geometry)?;

    let label = normalize_annotation_label(request.label)?;
    let author = normalize_optional_text(request.author).or(existing.author);
    let crs = normalize_optional_text(request.crs).or(existing.crs);
    let audit_id = normalize_optional_text(request.audit_id).or(existing.audit_id);
    let updated = AnnotationRecord {
        annotation_id: annotation_id.clone(),
        scene_id: scene_id.clone(),
        field_id: load_scene_field_id(&state, &scene_id).await?,
        author,
        crs,
        audit_id,
        label,
        note: normalize_optional_text(request.note),
        severity: normalize_optional_text(request.severity),
        geometry: request.geometry,
        created_at: existing.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let result = sqlx::query(
        r#"
        UPDATE annotations
        SET field_id = ?1, author = ?2, crs = ?3, audit_id = ?4, label = ?5, note = ?6, severity = ?7, geometry_json = ?8, updated_at = ?9
        WHERE annotation_id = ?10 AND scene_id = ?11
        "#,
    )
    .bind(&updated.field_id)
    .bind(&updated.author)
    .bind(&updated.crs)
    .bind(&updated.audit_id)
    .bind(&updated.label)
    .bind(&updated.note)
    .bind(&updated.severity)
    .bind(serde_json::to_string(&updated.geometry).map_err(|err| AppError::Anyhow(err.into()))?)
    .bind(&updated.updated_at)
    .bind(&updated.annotation_id)
    .bind(&updated.scene_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(updated))
}

pub async fn delete_scene_annotation(
    Path((scene_id, annotation_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let result = sqlx::query("DELETE FROM annotations WHERE annotation_id = ?1 AND scene_id = ?2")
        .bind(&annotation_id)
        .bind(&scene_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_scene_recommendations(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<RecommendationRecord>>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, evidence_refs_json, created_at, updated_at
        FROM recommendations
        WHERE scene_id = ?1
        ORDER BY created_at DESC
        "#,
    )
    .bind(&scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut recommendations = Vec::with_capacity(rows.len());
    for row in rows {
        recommendations.push(decode_recommendation_record(&state, &row).await?);
    }

    Ok(Json(recommendations))
}

pub async fn get_scene_recommendation(
    Path((scene_id, recommendation_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendation = load_recommendation(&state, &scene_id, &recommendation_id)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(recommendation))
}

pub async fn create_scene_recommendation(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CreateRecommendationRequest>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendation = build_recommendation_record(&state, &scene_id, request).await?;
    sqlx::query(
        r#"
        INSERT INTO recommendations (
            recommendation_id, scene_id, field_id, title, note, category, priority, status, evidence_refs_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&recommendation.recommendation_id)
    .bind(&recommendation.scene_id)
    .bind(&recommendation.field_id)
    .bind(&recommendation.title)
    .bind(&recommendation.note)
    .bind(&recommendation.category)
    .bind(recommendation_priority_str(recommendation.priority))
    .bind(recommendation_status_str(recommendation.status))
    .bind(
        serde_json::to_string(&recommendation.evidence_refs)
            .map_err(|err| AppError::Anyhow(err.into()))?,
    )
    .bind(&recommendation.created_at)
    .bind(&recommendation.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    persist_recommendation_annotations(
        &state,
        &recommendation.recommendation_id,
        &recommendation.annotation_ids,
    )
    .await?;

    // Persist the recommendation's lineage at create time (Track A phase 10b
    // polish) so a direct trace of `recommendation:<id>` closes to its source
    // annotations/findings — not only via the report-lineage reconstruction. The
    // record mirrors the one build_report_lineage_records derives, so the two
    // agree and push_lineage_record_if_absent stays a no-op there.
    let recommendation_inputs = unique_lineage_inputs(
        recommendation
            .annotation_ids
            .iter()
            .map(|annotation_id| annotation_artifact_ref(annotation_id))
            .chain(recommendation.evidence_refs.iter().cloned())
            .collect::<Vec<_>>(),
    );
    crate::provenance_store::append_lineage(
        &state.pool,
        &LineageRecord {
            artifact_id: recommendation_artifact_ref(&recommendation.recommendation_id),
            kind: ArtifactKind::Recommendation,
            inputs: recommendation_inputs,
            method: "10.recommendation_lifecycle".to_string(),
            parameters: ProvenanceParameters::from_json(serde_json::json!({
                "field_id": &recommendation.field_id,
                "title": &recommendation.title,
                "category": &recommendation.category,
                "priority": recommendation.priority,
                "status": recommendation.status,
            })),
            operator: recommendation.author_user_id.clone(),
            actor: ActorIdentity::system("geo_hub"),
            created_at: recommendation.created_at.clone(),
        },
    )
    .await
    .map_err(|err| AppError::Anyhow(err.into()))?;

    Ok(Json(recommendation))
}

pub async fn update_scene_recommendation(
    Path((scene_id, recommendation_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<UpdateRecommendationRequest>,
) -> AppResult<Json<RecommendationRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let existing = load_recommendation(&state, &scene_id, &recommendation_id)
        .await?
        .ok_or(AppError::NotFound)?;
    validate_recommendation_annotation_ids(&state, &scene_id, &request.annotation_ids).await?;
    let explicit_evidence_refs = if request.evidence_refs.is_empty() {
        existing.evidence_refs.clone()
    } else {
        request.evidence_refs
    };

    let updated = RecommendationRecord {
        recommendation_id: recommendation_id.clone(),
        scene_id: scene_id.clone(),
        field_id: load_scene_field_id(&state, &scene_id).await?,
        org_id: existing.org_id,
        author_user_id: existing.author_user_id,
        title: normalize_recommendation_title(request.title)?,
        note: normalize_optional_text(request.note),
        category: normalize_optional_text(request.category),
        action_category: normalize_optional_text(request.action_category)
            .unwrap_or(existing.action_category),
        priority: request.priority,
        status: request.status,
        evidence_refs: combine_text_values(
            recommendation_evidence_from_annotations(&request.annotation_ids),
            explicit_evidence_refs,
        ),
        annotation_ids: request.annotation_ids,
        created_at: existing.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let result = sqlx::query(
        r#"
        UPDATE recommendations
        SET field_id = ?1, title = ?2, note = ?3, category = ?4, priority = ?5, status = ?6, evidence_refs_json = ?7, updated_at = ?8
        WHERE recommendation_id = ?9 AND scene_id = ?10
        "#,
    )
    .bind(&updated.field_id)
    .bind(&updated.title)
    .bind(&updated.note)
    .bind(&updated.category)
    .bind(recommendation_priority_str(updated.priority))
    .bind(recommendation_status_str(updated.status))
    .bind(
        serde_json::to_string(&updated.evidence_refs)
            .map_err(|err| AppError::Anyhow(err.into()))?,
    )
    .bind(&updated.updated_at)
    .bind(&updated.recommendation_id)
    .bind(&updated.scene_id)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    persist_recommendation_annotations(&state, &updated.recommendation_id, &updated.annotation_ids)
        .await?;

    Ok(Json(updated))
}

pub async fn delete_scene_recommendation(
    Path((scene_id, recommendation_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let result =
        sqlx::query("DELETE FROM recommendations WHERE recommendation_id = ?1 AND scene_id = ?2")
            .bind(&recommendation_id)
            .bind(&scene_id)
            .execute(&state.pool)
            .await
            .map_err(Error::from)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_scene_reports(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ReportRecord>>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT report_id, scene_id, field_id, title, format, path, visibility, annotation_count, recommendation_count, created_at
        FROM reports
        WHERE scene_id = ?1
        ORDER BY created_at DESC
        "#,
    )
    .bind(&scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut reports = Vec::with_capacity(rows.len());
    for row in rows {
        reports.push(decode_report_record(&row)?);
    }

    Ok(Json(reports))
}

pub async fn generate_scene_report(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CreateReportRequest>,
) -> AppResult<Json<ReportRecord>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = build_scene_report(&state, &scene_id, request.title, request.visibility).await?;
    sqlx::query(
        r#"
        INSERT INTO reports (
            report_id, scene_id, field_id, title, format, path, visibility, annotation_count, recommendation_count, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&report.report_id)
    .bind(&report.scene_id)
    .bind(&report.field_id)
    .bind(&report.title)
    .bind(report_format_str(report.format))
    .bind(&report.artifact_path)
    .bind(report_visibility_str(report.visibility))
    .bind(report.annotation_count as i64)
    .bind(report.recommendation_count as i64)
    .bind(&report.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(report))
}

pub async fn download_scene_report(
    Path((scene_id, report_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = load_report(&state, &scene_id, &report_id)
        .await?
        .ok_or(AppError::NotFound)?;
    report_file_response(&report).await
}

pub async fn get_scene_report_lineage(
    Path((scene_id, report_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<BackwardProvenanceTrace>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = load_report(&state, &scene_id, &report_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let records = build_report_lineage_records(&state, &report).await?;
    let ledger = LineageLedger::from_persisted_records(records)
        .map_err(|err| AppError::Anyhow(Error::new(err)))?;
    let trace = ledger
        .trace_backward(&report_artifact_ref(&report.report_id))
        .map_err(|err| AppError::Anyhow(Error::new(err)))?;

    Ok(Json(trace))
}

pub async fn create_report_share(
    Path((scene_id, report_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(request): Json<CreateReportShareRequest>,
) -> AppResult<Json<ReportShareResponse>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let report = load_report(&state, &scene_id, &report_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if report.visibility != ReportVisibility::Shared {
        return Err(AppError::BadRequest(
            "org-only report cannot be shared".to_string(),
        ));
    }

    let now = current_record_timestamp();
    let share = ReportShareRecord {
        share_token: Uuid::new_v4().to_string(),
        report_id,
        scene_id,
        expires_at: normalize_share_expires_at(request.expires_at)?,
        revoked_at: None,
        created_at: now,
    };

    sqlx::query(
        r#"
        INSERT INTO report_shares (share_token, report_id, scene_id, expires_at, revoked_at, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&share.share_token)
    .bind(&share.report_id)
    .bind(&share.scene_id)
    .bind(&share.expires_at)
    .bind(&share.revoked_at)
    .bind(&share.created_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    audit_report_share_event(&state, &share, "share_created", None).await?;

    Ok(Json(report_share_response(&share)))
}

pub async fn revoke_report_share(
    Path((scene_id, report_id, share_token)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    let revoked_at = current_record_timestamp();
    let result = sqlx::query(
        r#"
        UPDATE report_shares
        SET revoked_at = COALESCE(revoked_at, ?1)
        WHERE scene_id = ?2 AND report_id = ?3 AND share_token = ?4
        "#,
    )
    .bind(&revoked_at)
    .bind(&scene_id)
    .bind(&report_id)
    .bind(&share_token)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    let share = load_report_share(&state, &share_token)
        .await?
        .ok_or(AppError::NotFound)?;
    audit_report_share_event(&state, &share, "share_revoked", None).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn download_shared_report(
    Path(share_token): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let share = load_report_share_with_report(&state, &share_token)
        .await?
        .ok_or(AppError::NotFound)?;

    if share.share.revoked_at.is_some() {
        return Err(AppError::Forbidden(
            "report share link has been revoked".to_string(),
        ));
    }
    if share_expired(&share.share.expires_at)? {
        return Err(AppError::Forbidden(
            "report share link has expired".to_string(),
        ));
    }
    if share.report.visibility != ReportVisibility::Shared {
        return Err(AppError::Forbidden(
            "report is not publicly shareable".to_string(),
        ));
    }

    audit_report_share_event(&state, &share.share, "share_accessed", None).await?;
    report_file_response(&share.report).await
}

pub async fn export_scene_annotations_csv(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "annotation_id",
            "scene_id",
            "field_id",
            "author",
            "crs",
            "audit_id",
            "label",
            "severity",
            "note",
            "geometry_type",
            "geometry_json",
            "created_at",
            "updated_at",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;
    for annotation in annotations {
        let geometry_type = annotation_geometry_type(&annotation.geometry).to_string();
        let geometry_json = serde_json::to_string(&annotation.geometry)
            .map_err(|err| AppError::Anyhow(err.into()))?;
        writer
            .write_record(vec![
                annotation.annotation_id,
                annotation.scene_id,
                annotation.field_id.unwrap_or_default(),
                annotation.author.unwrap_or_default(),
                annotation.crs.unwrap_or_default(),
                annotation.audit_id.unwrap_or_default(),
                annotation.label,
                annotation.severity.unwrap_or_default(),
                annotation.note.unwrap_or_default(),
                geometry_type,
                geometry_json,
                annotation.created_at,
                annotation.updated_at,
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }
    let csv_bytes = writer
        .into_inner()
        .map_err(|err| AppError::Anyhow(err.into_error().into()))?;

    response_with_bytes(csv_bytes, "text/csv; charset=utf-8", "annotations.csv")
}

pub async fn export_scene_recommendations_csv(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendations = load_scene_recommendation_records(&state, &scene_id).await?;
    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "recommendation_id",
            "scene_id",
            "field_id",
            "org_id",
            "author_user_id",
            "title",
            "category",
            "action_category",
            "priority",
            "status",
            "evidence_refs",
            "annotation_ids",
            "note",
            "created_at",
            "updated_at",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;
    for recommendation in recommendations {
        let export_field_id = recommendation_export_field_id(&recommendation, &annotations);
        writer
            .write_record(vec![
                recommendation.recommendation_id,
                recommendation.scene_id,
                export_field_id.unwrap_or_default(),
                recommendation.org_id,
                recommendation.author_user_id,
                recommendation.title,
                recommendation.category.unwrap_or_default(),
                recommendation.action_category,
                recommendation_priority_str(recommendation.priority).to_string(),
                recommendation_status_str(recommendation.status).to_string(),
                recommendation.evidence_refs.join("|"),
                recommendation.annotation_ids.join("|"),
                recommendation.note.unwrap_or_default(),
                recommendation.created_at,
                recommendation.updated_at,
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }
    let csv_bytes = writer
        .into_inner()
        .map_err(|err| AppError::Anyhow(err.into_error().into()))?;

    response_with_bytes(csv_bytes, "text/csv; charset=utf-8", "recommendations.csv")
}

pub async fn export_scene_annotations_geojson(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let crs = collection_crs_from_annotations(&annotations)?;
    let geojson = feature_collection_with_crs(
        annotations
            .iter()
            .map(feature_from_annotation)
            .collect::<AppResult<Vec<_>>>()?,
        &crs,
    );

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "annotations.geojson",
    )
}

pub async fn export_scene_recommendations_geojson(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let recommendations = load_scene_recommendation_records(&state, &scene_id).await?;
    let annotations = load_scene_annotation_records(&state, &scene_id).await?;
    let crs = collection_crs_from_annotations(&annotations)?;
    let mut features = Vec::new();
    for recommendation in &recommendations {
        features.extend(recommendation_features(recommendation, &annotations)?);
    }

    let geojson = feature_collection_with_crs(features, &crs);

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "recommendations.geojson",
    )
}
