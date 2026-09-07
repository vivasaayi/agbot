//! Field record exports + field CRUD/link route handlers (extracted verbatim from routes.rs).
//!
//! Shared helpers (decode_*, load_*, scene_exists, build_*_record) and domain
//! `*_error` mappers stay in the parent module and are reached via `use super::*`.

#![allow(clippy::too_many_arguments)]
use super::*;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;

pub async fn export_field_records_csv(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let annotations = load_field_annotation_records(&state, &field_id).await?;
    let recommendations = load_field_recommendation_records(&state, &field_id).await?;
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "record_type",
            "record_id",
            "scene_id",
            "field_id",
            "crs",
            "title",
            "label",
            "status",
            "priority",
            "evidence_refs",
            "annotation_ids",
            "geometry_type",
            "geometry_json",
            "created_at",
            "updated_at",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;

    for annotation in &annotations {
        let geometry_type = annotation_geometry_type(&annotation.geometry).to_string();
        let geometry_json = serde_json::to_string(&annotation.geometry)
            .map_err(|err| AppError::Anyhow(err.into()))?;
        writer
            .write_record(vec![
                "annotation".to_string(),
                annotation.annotation_id.clone(),
                annotation.scene_id.clone(),
                annotation
                    .field_id
                    .clone()
                    .unwrap_or_else(|| field.field_id.clone()),
                annotation
                    .crs
                    .clone()
                    .unwrap_or_else(|| field_record_crs(&field)),
                String::new(),
                annotation.label.clone(),
                String::new(),
                annotation.severity.clone().unwrap_or_default(),
                String::new(),
                String::new(),
                geometry_type,
                geometry_json,
                annotation.created_at.clone(),
                annotation.updated_at.clone(),
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }

    for recommendation in recommendations {
        writer
            .write_record(vec![
                "recommendation".to_string(),
                recommendation.recommendation_id,
                recommendation.scene_id,
                recommendation
                    .field_id
                    .unwrap_or_else(|| field.field_id.clone()),
                field_record_crs(&field),
                recommendation.title,
                String::new(),
                recommendation_status_str(recommendation.status).to_string(),
                recommendation_priority_str(recommendation.priority).to_string(),
                recommendation.evidence_refs.join("|"),
                recommendation.annotation_ids.join("|"),
                String::new(),
                String::new(),
                recommendation.created_at,
                recommendation.updated_at,
            ])
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }

    let csv_bytes = writer
        .into_inner()
        .map_err(|err| AppError::Anyhow(err.into_error().into()))?;

    response_with_bytes(csv_bytes, "text/csv; charset=utf-8", "field-records.csv")
}

pub async fn export_field_records_geojson(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let field_crs = field_record_crs(&field);
    let annotations = load_field_annotation_records(&state, &field_id).await?;
    assert_field_bundle_annotation_crs(&annotations, &field_crs)?;
    let recommendations = load_field_recommendation_records(&state, &field_id).await?;

    let mut features = vec![feature_from_field(field.clone())];
    features.extend(
        annotations
            .iter()
            .map(feature_from_annotation)
            .collect::<AppResult<Vec<_>>>()?,
    );
    for recommendation in &recommendations {
        features.extend(recommendation_features(recommendation, &annotations)?);
    }

    let geojson = feature_collection_with_crs(features, &field_crs);

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "field-records.geojson",
    )
}

pub async fn create_field(
    State(state): State<AppState>,
    Json(request): Json<CreateFieldRequest>,
) -> AppResult<Json<FieldRecord>> {
    let mut field = build_field_record(request)?;
    field.owner = field_owner_for_farm(&state, field.farm_id.as_deref(), &field.owner).await?;
    field.org_id = field.owner.clone();

    sqlx::query(
        r#"
        INSERT INTO fields (field_id, farm_id, owner, name, crop, season, notes, boundary_json, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&field.field_id)
    .bind(&field.farm_id)
    .bind(&field.owner)
    .bind(&field.name)
    .bind(&field.crop)
    .bind(&field.season)
    .bind(&field.notes)
    .bind(serde_json::to_string(&field.boundary).map_err(|err| AppError::Anyhow(err.into()))?)
    .bind(field.status.as_str())
    .bind(&field.created_at)
    .bind(&field.updated_at)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(field))
}

pub async fn get_field(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FieldRecord>> {
    let field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(field))
}

pub async fn link_field_to_farm(
    Path((field_id, farm_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<FieldRecord>> {
    let mut field = load_field(&state, &field_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let farm = load_farm(&state, &farm_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let updated_at = current_record_timestamp();
    sqlx::query("UPDATE fields SET farm_id = ?2, owner = ?3, updated_at = ?4 WHERE field_id = ?1")
        .bind(&field_id)
        .bind(&farm_id)
        .bind(&farm.owner)
        .bind(&updated_at)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    field.farm_id = Some(farm_id);
    field.owner = farm.owner.clone();
    field.org_id = farm.owner;
    field.updated_at = updated_at;
    Ok(Json(field))
}
