use crate::{
    error::{AppError, AppResult},
    ingest, shapefile,
    state::AppState,
};
use anyhow::Error;
use axum::response::{IntoResponse, Response};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    Json,
};
use geojson::{
    feature::Id as GeoJsonId, Feature, FeatureCollection, GeoJson, Geometry, Value as GeoJsonValue,
};
use image::{imageops::FilterType, DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use shared::schemas::{
    bounds_from_points, AnnotationGeometry, AnnotationRecord, FarmRecord, FieldBoundary,
    FieldRecord, GeoPoint, GpsCoords, MultispectralImage, RecommendationPriority,
    RecommendationRecord, RecommendationStatus, ReportFormat, ReportRecord,
};
use sqlx::Row;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::io::ErrorKind;
use std::path::{Path as FsPath, PathBuf};
use std::time::SystemTime;
use tokio::fs::File;
use tokio::fs::{self, DirEntry};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

const TILE_SIZE: u32 = 256;

#[derive(Debug, Serialize)]
pub struct SceneSummary {
    pub scene_id: String,
    pub sensor: String,
    pub acquired_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneDetail {
    pub scene_id: String,
    pub sensor: Option<String>,
    pub acquired_at: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bands: Vec<String>,
    pub gps_position: Option<GpsCoords>,
    pub data_path: Option<String>,
    pub field: Option<FieldRecord>,
    pub geospatial: SceneGeospatialMetadata,
    pub available_products: Vec<ProductSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductSummary {
    pub kind: String,
    pub filename: String,
    pub content_type: String,
    pub url_path: String,
    pub tile_url_template: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SceneGeospatialMetadata {
    pub georeferenced: bool,
    pub crs: Option<String>,
    pub center: Option<GpsCoords>,
    pub extent: Option<SceneExtent>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SceneExtent {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateFieldRequest {
    pub farm_id: Option<String>,
    pub field_id: Option<String>,
    pub name: String,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub notes: Option<String>,
    pub boundary: FieldBoundary,
}

#[derive(Debug, Deserialize)]
pub struct CreateFarmRequest {
    pub farm_id: Option<String>,
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFarmRequest {
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnotationRequest {
    pub annotation_id: Option<String>,
    pub label: String,
    pub note: Option<String>,
    pub severity: Option<String>,
    pub geometry: AnnotationGeometry,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnnotationRequest {
    pub label: String,
    pub note: Option<String>,
    pub severity: Option<String>,
    pub geometry: AnnotationGeometry,
}

#[derive(Debug, Deserialize)]
pub struct CreateRecommendationRequest {
    pub recommendation_id: Option<String>,
    pub title: String,
    pub note: Option<String>,
    pub category: Option<String>,
    pub priority: Option<RecommendationPriority>,
    pub status: Option<RecommendationStatus>,
    #[serde(default)]
    pub annotation_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRecommendationRequest {
    pub title: String,
    pub note: Option<String>,
    pub category: Option<String>,
    pub priority: RecommendationPriority,
    pub status: RecommendationStatus,
    #[serde(default)]
    pub annotation_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReportRequest {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportShapefileRequest {
    pub path: String,
    pub name_prefix: Option<String>,
    pub farm_id: Option<String>,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldSeasonGroup {
    pub season: Option<String>,
    pub fields: Vec<FieldRecord>,
}

pub async fn import_fields_geojson(
    State(state): State<AppState>,
    Json(payload): Json<GeoJson>,
) -> AppResult<Json<Vec<FieldRecord>>> {
    let fields = fields_from_geojson(payload)?;

    upsert_fields(&state, &fields).await?;

    Ok(Json(fields))
}

pub async fn import_fields_shapefile(
    State(state): State<AppState>,
    Json(payload): Json<ImportShapefileRequest>,
) -> AppResult<Json<Vec<FieldRecord>>> {
    let fields = fields_from_shapefile(payload).await?;

    upsert_fields(&state, &fields).await?;

    Ok(Json(fields))
}

async fn upsert_fields(state: &AppState, fields: &[FieldRecord]) -> AppResult<()> {
    for field in fields {
        ensure_field_farm_exists(state, field.farm_id.as_deref()).await?;
        sqlx::query(
            r#"
            INSERT INTO fields (field_id, farm_id, name, crop, season, notes, boundary_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
            ON CONFLICT(field_id) DO UPDATE SET
                farm_id = excluded.farm_id,
                name = excluded.name,
                crop = excluded.crop,
                season = excluded.season,
                notes = excluded.notes,
                boundary_json = excluded.boundary_json
            "#,
        )
        .bind(&field.field_id)
        .bind(&field.farm_id)
        .bind(&field.name)
        .bind(&field.crop)
        .bind(&field.season)
        .bind(&field.notes)
        .bind(serde_json::to_string(&field.boundary).map_err(|err| AppError::Anyhow(err.into()))?)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;
    }

    Ok(())
}

pub async fn list_farms(State(state): State<AppState>) -> AppResult<Json<Vec<FarmRecord>>> {
    let rows = sqlx::query("SELECT farm_id, name, notes FROM farms ORDER BY name ASC")
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?;

    let mut farms = Vec::with_capacity(rows.len());
    for row in rows {
        farms.push(decode_farm_record(&row));
    }

    Ok(Json(farms))
}

pub async fn create_farm(
    State(state): State<AppState>,
    Json(request): Json<CreateFarmRequest>,
) -> AppResult<Json<FarmRecord>> {
    let farm = build_farm_record(request)?;

    sqlx::query(
        r#"
        INSERT INTO farms (farm_id, name, notes, created_at)
        VALUES (?1, ?2, ?3, datetime('now'))
        "#,
    )
    .bind(&farm.farm_id)
    .bind(&farm.name)
    .bind(&farm.notes)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(farm))
}

pub async fn get_farm(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<FarmRecord>> {
    let farm = load_farm(&state, &farm_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(farm))
}

pub async fn update_farm(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateFarmRequest>,
) -> AppResult<Json<FarmRecord>> {
    let mut farm = load_farm(&state, &farm_id)
        .await?
        .ok_or(AppError::NotFound)?;
    farm.name = normalize_farm_name(request.name)?;
    farm.notes = normalize_optional_text(request.notes);

    sqlx::query(
        r#"
        UPDATE farms
        SET name = ?2, notes = ?3
        WHERE farm_id = ?1
        "#,
    )
    .bind(&farm.farm_id)
    .bind(&farm.name)
    .bind(&farm.notes)
    .execute(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(Json(farm))
}

pub async fn delete_farm(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    if load_farm(&state, &farm_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query("UPDATE fields SET farm_id = NULL WHERE farm_id = ?1")
        .bind(&farm_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;
    sqlx::query("DELETE FROM farms WHERE farm_id = ?1")
        .bind(&farm_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_farm_fields(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FieldRecord>>> {
    if load_farm(&state, &farm_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        "SELECT field_id, farm_id, name, crop, season, notes, boundary_json FROM fields WHERE farm_id = ?1 ORDER BY COALESCE(season, '') DESC, name ASC",
    )
    .bind(&farm_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        fields.push(decode_field_record(&row)?);
    }

    Ok(Json(fields))
}

pub async fn list_farm_field_history(
    Path(farm_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<FieldSeasonGroup>>> {
    let fields = list_farm_fields(Path(farm_id), State(state)).await?.0;
    Ok(Json(group_fields_by_season(fields)))
}

pub async fn list_fields(State(state): State<AppState>) -> AppResult<Json<Vec<FieldRecord>>> {
    let rows = sqlx::query(
        "SELECT field_id, farm_id, name, crop, season, notes, boundary_json FROM fields ORDER BY name ASC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        fields.push(decode_field_record(&row)?);
    }

    Ok(Json(fields))
}

pub async fn export_fields_geojson(State(state): State<AppState>) -> AppResult<Json<GeoJson>> {
    let rows = sqlx::query(
        "SELECT field_id, farm_id, name, crop, season, notes, boundary_json FROM fields ORDER BY name ASC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut fields = Vec::with_capacity(rows.len());
    for row in rows {
        fields.push(decode_field_record(&row)?);
    }

    Ok(Json(geojson_from_fields(fields)))
}

pub async fn list_scene_annotations(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<AnnotationRecord>>> {
    if !scene_exists(&state, &scene_id).await? {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, label, note, severity, geometry_json, created_at, updated_at
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
            annotation_id, scene_id, field_id, label, note, severity, geometry_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&annotation.annotation_id)
    .bind(&annotation.scene_id)
    .bind(&annotation.field_id)
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
    let updated = AnnotationRecord {
        annotation_id: annotation_id.clone(),
        scene_id: scene_id.clone(),
        field_id: load_scene_field_id(&state, &scene_id).await?,
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
        SET field_id = ?1, label = ?2, note = ?3, severity = ?4, geometry_json = ?5, updated_at = ?6
        WHERE annotation_id = ?7 AND scene_id = ?8
        "#,
    )
    .bind(&updated.field_id)
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
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, created_at, updated_at
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
            recommendation_id, scene_id, field_id, title, note, category, priority, status, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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

    let updated = RecommendationRecord {
        recommendation_id: recommendation_id.clone(),
        scene_id: scene_id.clone(),
        field_id: load_scene_field_id(&state, &scene_id).await?,
        title: normalize_recommendation_title(request.title)?,
        note: normalize_optional_text(request.note),
        category: normalize_optional_text(request.category),
        priority: request.priority,
        status: request.status,
        annotation_ids: request.annotation_ids,
        created_at: existing.created_at,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let result = sqlx::query(
        r#"
        UPDATE recommendations
        SET field_id = ?1, title = ?2, note = ?3, category = ?4, priority = ?5, status = ?6, updated_at = ?7
        WHERE recommendation_id = ?8 AND scene_id = ?9
        "#,
    )
    .bind(&updated.field_id)
    .bind(&updated.title)
    .bind(&updated.note)
    .bind(&updated.category)
    .bind(recommendation_priority_str(updated.priority))
    .bind(recommendation_status_str(updated.status))
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
        SELECT report_id, scene_id, field_id, title, format, path, annotation_count, recommendation_count, created_at
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

    let report = build_scene_report(&state, &scene_id, request.title).await?;
    sqlx::query(
        r#"
        INSERT INTO reports (
            report_id, scene_id, field_id, title, format, path, annotation_count, recommendation_count, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&report.report_id)
    .bind(&report.scene_id)
    .bind(&report.field_id)
    .bind(&report.title)
    .bind(report_format_str(report.format))
    .bind(&report.artifact_path)
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
    let report_path = PathBuf::from(&report.artifact_path);

    let file = File::open(&report_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    if let Some(filename) = report_path.file_name().and_then(|name| name.to_str()) {
        if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{}\"", filename)) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }

    Ok((headers, body).into_response())
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
            "label",
            "severity",
            "note",
            "geometry_type",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;
    for annotation in annotations {
        writer
            .write_record([
                annotation.annotation_id,
                annotation.label,
                annotation.severity.unwrap_or_default(),
                annotation.note.unwrap_or_default(),
                match annotation.geometry {
                    AnnotationGeometry::Point { .. } => "point".to_string(),
                    AnnotationGeometry::Polygon { .. } => "polygon".to_string(),
                },
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
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "recommendation_id",
            "title",
            "category",
            "priority",
            "status",
            "annotation_ids",
            "note",
        ])
        .map_err(|err| AppError::Anyhow(err.into()))?;
    for recommendation in recommendations {
        writer
            .write_record([
                recommendation.recommendation_id,
                recommendation.title,
                recommendation.category.unwrap_or_default(),
                recommendation_priority_str(recommendation.priority).to_string(),
                recommendation_status_str(recommendation.status).to_string(),
                recommendation.annotation_ids.join("|"),
                recommendation.note.unwrap_or_default(),
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
    let geojson = GeoJson::FeatureCollection(FeatureCollection {
        bbox: None,
        foreign_members: None,
        features: annotations
            .iter()
            .map(feature_from_annotation)
            .collect::<AppResult<Vec<_>>>()?,
    });

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
    let mut features = Vec::new();
    for recommendation in &recommendations {
        features.extend(recommendation_features(recommendation, &annotations)?);
    }

    let geojson = GeoJson::FeatureCollection(FeatureCollection {
        bbox: None,
        foreign_members: None,
        features,
    });

    response_with_bytes(
        serde_json::to_vec(&geojson).map_err(|err| AppError::Anyhow(err.into()))?,
        "application/geo+json",
        "recommendations.geojson",
    )
}

pub async fn create_field(
    State(state): State<AppState>,
    Json(request): Json<CreateFieldRequest>,
) -> AppResult<Json<FieldRecord>> {
    let field = build_field_record(request)?;
    ensure_field_farm_exists(&state, field.farm_id.as_deref()).await?;

    sqlx::query(
        r#"
        INSERT INTO fields (field_id, farm_id, name, crop, season, notes, boundary_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
        "#,
    )
    .bind(&field.field_id)
    .bind(&field.farm_id)
    .bind(&field.name)
    .bind(&field.crop)
    .bind(&field.season)
    .bind(&field.notes)
    .bind(serde_json::to_string(&field.boundary).map_err(|err| AppError::Anyhow(err.into()))?)
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
    if load_farm(&state, &farm_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query("UPDATE fields SET farm_id = ?2 WHERE field_id = ?1")
        .bind(&field_id)
        .bind(&farm_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    field.farm_id = Some(farm_id);
    Ok(Json(field))
}

pub async fn list_field_scenes(
    Path(field_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SceneSummary>>> {
    if load_field(&state, &field_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let rows = sqlx::query(
        "SELECT scene_id, sensor, acquired_at FROM scenes WHERE field_id = ?1 ORDER BY acquired_at DESC",
    )
    .bind(&field_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let scenes = rows
        .into_iter()
        .map(|row| SceneSummary {
            scene_id: row.get("scene_id"),
            sensor: row.get("sensor"),
            acquired_at: row.get("acquired_at"),
        })
        .collect();

    Ok(Json(scenes))
}

pub async fn link_scene_to_field(
    Path((scene_id, field_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Json<SceneDetail>> {
    if load_field(&state, &field_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let updated = sqlx::query("UPDATE scenes SET field_id = ?1 WHERE scene_id = ?2")
        .bind(&field_id)
        .bind(&scene_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    get_scene(Path(scene_id), State(state)).await
}

pub async fn list_scenes(State(state): State<AppState>) -> AppResult<Json<Vec<SceneSummary>>> {
    let rows =
        sqlx::query("SELECT scene_id, sensor, acquired_at FROM scenes ORDER BY acquired_at DESC")
            .fetch_all(&state.pool)
            .await
            .map_err(Error::from)?;

    let scenes = rows
        .into_iter()
        .map(|row| SceneSummary {
            scene_id: row.get("scene_id"),
            sensor: row.get("sensor"),
            acquired_at: row.get("acquired_at"),
        })
        .collect();

    Ok(Json(scenes))
}

pub async fn get_scene(
    Path(scene_id): Path<String>,
    State(state): State<AppState>,
) -> AppResult<Json<SceneDetail>> {
    let scene_row = sqlx::query(
        "SELECT scene_id, sensor, acquired_at, data_path, metadata_json, field_id FROM scenes WHERE scene_id = ?1",
    )
            .bind(&scene_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(Error::from)?;

    let scene_dir = state.config.data_root.join("scenes").join(&scene_id);
    let has_scene_dir = fs::try_exists(&scene_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    if scene_row.is_none() && !has_scene_dir {
        return Err(AppError::NotFound);
    }

    let metadata = load_scene_metadata(scene_row.as_ref(), &scene_dir).await?;
    let field = load_scene_field(&state, scene_row.as_ref()).await?;
    let available_products = collect_scene_products(&state, &scene_id).await?;

    Ok(Json(SceneDetail {
        scene_id,
        sensor: scene_row.as_ref().map(|row| row.get("sensor")),
        acquired_at: scene_row.as_ref().map(|row| row.get("acquired_at")),
        width: metadata.as_ref().map(|image| image.metadata.width),
        height: metadata.as_ref().map(|image| image.metadata.height),
        bands: metadata
            .as_ref()
            .map(|image| image.metadata.bands.clone())
            .unwrap_or_default(),
        gps_position: metadata
            .as_ref()
            .and_then(|image| image.metadata.gps_position.clone()),
        data_path: scene_row.as_ref().map(|row| row.get("data_path")),
        field,
        geospatial: build_geospatial_metadata(metadata.as_ref()),
        available_products,
    }))
}

pub async fn stream_product(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let product_path = resolve_product_path(&state, &scene_id, &kind).await?;

    let file = File::open(&product_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = content_type_for_path(&product_path);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));

    if let Some(filename) = product_path.file_name().and_then(|name| name.to_str()) {
        if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{}\"", filename)) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }

    Ok((headers, body).into_response())
}

pub async fn stream_product_tile(
    Path((scene_id, kind, z, x, y_segment)): Path<(String, String, u8, u32, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let y = y_segment
        .strip_suffix(".png")
        .ok_or_else(|| AppError::BadRequest("tile requests must end with .png".to_string()))?
        .parse::<u32>()
        .map_err(|_| AppError::BadRequest("invalid tile y coordinate".to_string()))?;
    let product_path = resolve_product_path(&state, &scene_id, &kind).await?;
    let tile_path = tile_cache_path(&state, &scene_id, &kind, &product_path, z, x, y).await?;

    if !fs::try_exists(&tile_path)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let source_path = product_path.clone();
        let tile_bytes =
            tokio::task::spawn_blocking(move || generate_tile_bytes(&source_path, z, x, y))
                .await
                .map_err(|err| AppError::Anyhow(err.into()))??;

        if let Some(parent) = tile_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;
        }
        fs::write(&tile_path, tile_bytes)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
    }

    let file = File::open(&tile_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );

    Ok((headers, body).into_response())
}

async fn resolve_product_path(state: &AppState, scene_id: &str, kind: &str) -> AppResult<PathBuf> {
    if let Some(path) = find_product_file_on_disk(state, scene_id, kind).await? {
        return Ok(path);
    }

    match ingest::ensure_product(&state.pool, scene_id, kind).await {
        Ok(path) => Ok(path),
        Err(err) if is_missing_scene_error(&err) => Err(AppError::NotFound),
        Err(err) => Err(AppError::Anyhow(err)),
    }
}

async fn find_product_file_on_disk(
    state: &AppState,
    scene_id: &str,
    kind: &str,
) -> AppResult<Option<PathBuf>> {
    let product_dir = state
        .config
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("products")
        .join(kind);

    if !fs::try_exists(&product_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        return Ok(None);
    }

    let mut entries = fs::read_dir(&product_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    select_preferred_product_path(&mut entries).await
}

async fn tile_cache_path(
    state: &AppState,
    scene_id: &str,
    kind: &str,
    product_path: &FsPath,
    z: u8,
    x: u32,
    y: u32,
) -> AppResult<PathBuf> {
    // On-demand tiles are cached under a source fingerprint so regenerated products
    // naturally miss the old cache path without needing synchronous cleanup work.
    let fingerprint = product_cache_fingerprint(product_path).await?;
    Ok(state
        .config
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("tile_cache")
        .join(kind)
        .join(fingerprint)
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.png")))
}

async fn product_cache_fingerprint(path: &FsPath) -> AppResult<String> {
    let metadata = fs::metadata(path)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let modified_epoch = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|value| value.as_secs())
        .unwrap_or_default();

    Ok(format!("{}-{}", metadata.len(), modified_epoch))
}

fn generate_tile_bytes(product_path: &FsPath, z: u8, x: u32, y: u32) -> AppResult<Vec<u8>> {
    let tiles_per_axis = 1_u32
        .checked_shl(z as u32)
        .ok_or_else(|| AppError::BadRequest("unsupported zoom level".to_string()))?;
    if x >= tiles_per_axis || y >= tiles_per_axis {
        return Err(AppError::NotFound);
    }

    let image = image::open(product_path).map_err(|err| AppError::Anyhow(err.into()))?;
    let rgba = image.to_rgba8();
    let source_width = rgba.width().max(1);
    let source_height = rgba.height().max(1);

    let x0 = (((x as f64) / (tiles_per_axis as f64)) * source_width as f64).floor() as u32;
    let y0 = (((y as f64) / (tiles_per_axis as f64)) * source_height as f64).floor() as u32;
    let x1 = ((((x + 1) as f64) / (tiles_per_axis as f64)) * source_width as f64).ceil() as u32;
    let y1 = ((((y + 1) as f64) / (tiles_per_axis as f64)) * source_height as f64).ceil() as u32;

    let crop_width = x1
        .saturating_sub(x0)
        .clamp(1, source_width.saturating_sub(x0).max(1));
    let crop_height = y1
        .saturating_sub(y0)
        .clamp(1, source_height.saturating_sub(y0).max(1));

    let cropped = image::imageops::crop_imm(&rgba, x0, y0, crop_width, crop_height).to_image();
    let resized = image::imageops::resize(&cropped, TILE_SIZE, TILE_SIZE, FilterType::Triangle);
    let tile = DynamicImage::ImageRgba8(resized);

    let mut cursor = Cursor::new(Vec::new());
    tile.write_to(&mut cursor, ImageFormat::Png)
        .map_err(|err| AppError::Anyhow(err.into()))?;
    Ok(cursor.into_inner())
}

async fn collect_scene_products(
    state: &AppState,
    scene_id: &str,
) -> AppResult<Vec<ProductSummary>> {
    let mut products = BTreeMap::new();
    let scene_products_dir = state
        .config
        .data_root
        .join("scenes")
        .join(scene_id)
        .join("products");

    if fs::try_exists(&scene_products_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let mut kind_dirs = fs::read_dir(&scene_products_dir)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;

        while let Some(entry) = kind_dirs
            .next_entry()
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;
            if !file_type.is_dir() {
                continue;
            }

            let kind = entry.file_name().to_string_lossy().to_string();
            let mut entries = fs::read_dir(entry.path())
                .await
                .map_err(|err| AppError::Anyhow(err.into()))?;

            if let Some(path) = select_preferred_product_path(&mut entries).await? {
                products.insert(kind.clone(), build_product_summary(scene_id, &kind, &path));
            }
        }
    }

    let rows = sqlx::query("SELECT kind, path FROM products WHERE scene_id = ?1")
        .bind(scene_id)
        .fetch_all(&state.pool)
        .await
        .map_err(Error::from)?;

    for row in rows {
        let kind: String = row.get("kind");
        let path = PathBuf::from(row.get::<String, _>("path"));
        let exists = fs::try_exists(&path)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        if !exists {
            continue;
        }
        products
            .entry(kind.clone())
            .or_insert_with(|| build_product_summary(scene_id, &kind, &path));
    }

    Ok(products.into_values().collect())
}

async fn is_supported_product_file(entry: &DirEntry) -> AppResult<bool> {
    let file_type = entry
        .file_type()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    if !file_type.is_file() {
        return Ok(false);
    }

    let extension = entry
        .path()
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    Ok(matches!(
        extension.as_deref(),
        Some("png") | Some("jpg") | Some("jpeg") | Some("tif") | Some("tiff")
    ))
}

fn is_missing_scene_error(err: &anyhow::Error) -> bool {
    err.chain().any(|source| {
        source
            .downcast_ref::<sqlx::Error>()
            .is_some_and(|sqlx_err| matches!(sqlx_err, sqlx::Error::RowNotFound))
    })
}

fn content_type_for_path(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("tif") | Some("tiff") => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn is_png(path: &FsPath) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}

async fn select_preferred_product_path(entries: &mut fs::ReadDir) -> AppResult<Option<PathBuf>> {
    let mut selected: Option<PathBuf> = None;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        if !is_supported_product_file(&entry).await? {
            continue;
        }

        let path = entry.path();
        match &selected {
            None => selected = Some(path),
            Some(current) => {
                if is_png(&path) && !is_png(current) {
                    selected = Some(path);
                }
            }
        }
    }

    Ok(selected)
}

fn build_product_summary(scene_id: &str, kind: &str, path: &FsPath) -> ProductSummary {
    ProductSummary {
        kind: kind.to_string(),
        filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string(),
        content_type: content_type_for_path(path).to_string(),
        url_path: format!("/api/scenes/{scene_id}/products/{kind}"),
        tile_url_template: format!(
            "/api/scenes/{scene_id}/products/{kind}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
    }
}

fn build_geospatial_metadata(metadata: Option<&MultispectralImage>) -> SceneGeospatialMetadata {
    let spatial_ref = metadata.and_then(|image| image.metadata.spatial_ref.as_ref());
    let extent = spatial_ref.and_then(|spatial| {
        spatial.bbox.as_ref().map(|bbox| SceneExtent {
            min_lon: bbox.min_lon,
            min_lat: bbox.min_lat,
            max_lon: bbox.max_lon,
            max_lat: bbox.max_lat,
        })
    });
    let center = extent.as_ref().map(|bbox| GpsCoords {
        latitude: (bbox.min_lat + bbox.max_lat) / 2.0,
        longitude: (bbox.min_lon + bbox.max_lon) / 2.0,
        altitude: metadata
            .and_then(|image| image.metadata.gps_position.as_ref())
            .map(|gps| gps.altitude)
            .unwrap_or(0.0),
    });

    SceneGeospatialMetadata {
        georeferenced: spatial_ref.is_some_and(|spatial| spatial.georeferenced),
        crs: spatial_ref.and_then(|spatial| spatial.crs.clone()),
        center: center.or_else(|| metadata.and_then(|image| image.metadata.gps_position.clone())),
        extent,
    }
}

fn build_field_record(request: CreateFieldRequest) -> AppResult<FieldRecord> {
    let field_id = request
        .field_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("field name is required".to_string()));
    }
    if request.boundary.coordinates.len() < 3 {
        return Err(AppError::BadRequest(
            "field boundary must contain at least three coordinates".to_string(),
        ));
    }
    if request.boundary.coordinates.iter().any(|point| {
        !point.longitude.is_finite()
            || !point.latitude.is_finite()
            || point.longitude < -180.0
            || point.longitude > 180.0
            || point.latitude < -90.0
            || point.latitude > 90.0
    }) {
        return Err(AppError::BadRequest(
            "field boundary contains invalid geographic coordinates".to_string(),
        ));
    }

    let extent = bounds_from_points(&request.boundary.coordinates).ok_or_else(|| {
        AppError::BadRequest("field boundary must contain valid coordinates".to_string())
    })?;

    Ok(FieldRecord {
        farm_id: request.farm_id,
        field_id,
        name,
        crop: request.crop,
        season: request.season,
        notes: request.notes,
        boundary: request.boundary,
        extent,
    })
}

fn build_farm_record(request: CreateFarmRequest) -> AppResult<FarmRecord> {
    let farm_id = request
        .farm_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    Ok(FarmRecord {
        farm_id,
        name: normalize_farm_name(request.name)?,
        notes: normalize_optional_text(request.notes),
    })
}

async fn build_annotation_record(
    state: &AppState,
    scene_id: &str,
    request: CreateAnnotationRequest,
) -> AppResult<AnnotationRecord> {
    let annotation_id = request
        .annotation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let label = normalize_annotation_label(request.label)?;
    validate_annotation_geometry(&request.geometry)?;
    let field_id = load_scene_field_id(state, scene_id).await?;

    let timestamp = chrono::Utc::now().to_rfc3339();
    Ok(AnnotationRecord {
        annotation_id,
        scene_id: scene_id.to_string(),
        field_id,
        label,
        note: normalize_optional_text(request.note),
        severity: normalize_optional_text(request.severity),
        geometry: request.geometry,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

async fn build_recommendation_record(
    state: &AppState,
    scene_id: &str,
    request: CreateRecommendationRequest,
) -> AppResult<RecommendationRecord> {
    validate_recommendation_annotation_ids(state, scene_id, &request.annotation_ids).await?;

    let recommendation_id = request
        .recommendation_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = chrono::Utc::now().to_rfc3339();

    Ok(RecommendationRecord {
        recommendation_id,
        scene_id: scene_id.to_string(),
        field_id: load_scene_field_id(state, scene_id).await?,
        title: normalize_recommendation_title(request.title)?,
        note: normalize_optional_text(request.note),
        category: normalize_optional_text(request.category),
        priority: request.priority.unwrap_or_default(),
        status: request.status.unwrap_or_default(),
        annotation_ids: request.annotation_ids,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

async fn build_scene_report(
    state: &AppState,
    scene_id: &str,
    title: Option<String>,
) -> AppResult<ReportRecord> {
    let scene_row = sqlx::query(
        "SELECT scene_id, sensor, acquired_at, data_path, metadata_json, field_id FROM scenes WHERE scene_id = ?1",
    )
    .bind(scene_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;
    let scene_dir = state.config.data_root.join("scenes").join(scene_id);
    let metadata = load_scene_metadata(scene_row.as_ref(), &scene_dir).await?;
    let field = load_scene_field(state, scene_row.as_ref()).await?;
    let geospatial = build_geospatial_metadata(metadata.as_ref());
    let annotations = load_scene_annotation_records(state, scene_id).await?;
    let recommendations = load_scene_recommendation_records(state, scene_id).await?;
    let report_id = Uuid::new_v4().to_string();
    let report_title = title
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .unwrap_or_else(|| format!("Scene {} field intelligence report", scene_id));
    let report_dir = state.config.data_root.join("reports").join(scene_id);
    fs::create_dir_all(&report_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;
    let artifact_path = report_dir.join(format!("{report_id}.html"));
    let html = render_scene_report_html(
        scene_id,
        scene_row.as_ref().map(|row| row.get("sensor")),
        scene_row.as_ref().map(|row| row.get("acquired_at")),
        metadata.as_ref(),
        field.as_ref(),
        &geospatial,
        &annotations,
        &recommendations,
        &report_title,
    );
    fs::write(&artifact_path, html)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?;

    Ok(ReportRecord {
        report_id: report_id.clone(),
        scene_id: scene_id.to_string(),
        field_id: field.as_ref().map(|field| field.field_id.clone()),
        title: report_title,
        format: ReportFormat::Html,
        artifact_path: artifact_path.to_string_lossy().to_string(),
        download_url: format!("/api/scenes/{scene_id}/reports/{report_id}"),
        annotation_count: annotations.len(),
        recommendation_count: recommendations.len(),
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn normalize_annotation_label(label: String) -> AppResult<String> {
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err(AppError::BadRequest(
            "annotation label is required".to_string(),
        ));
    }
    Ok(label)
}

fn normalize_farm_name(name: String) -> AppResult<String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("farm name is required".to_string()));
    }
    Ok(name)
}

fn normalize_recommendation_title(title: String) -> AppResult<String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::BadRequest(
            "recommendation title is required".to_string(),
        ));
    }
    Ok(title)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn fields_from_geojson(geojson: GeoJson) -> AppResult<Vec<FieldRecord>> {
    match geojson {
        GeoJson::FeatureCollection(collection) => collection
            .features
            .into_iter()
            .enumerate()
            .map(|(index, feature)| build_field_from_feature(feature, index))
            .collect(),
        GeoJson::Feature(feature) => Ok(vec![build_field_from_feature(feature, 0)?]),
        GeoJson::Geometry(geometry) => Ok(vec![build_field_from_geometry(geometry, None, 0)?]),
    }
}

async fn fields_from_shapefile(request: ImportShapefileRequest) -> AppResult<Vec<FieldRecord>> {
    let path = PathBuf::from(request.path.trim());
    if path.as_os_str().is_empty() {
        return Err(AppError::BadRequest(
            "shapefile path is required".to_string(),
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|ext| !ext.eq_ignore_ascii_case("shp"))
        .unwrap_or(true)
    {
        return Err(AppError::BadRequest(
            "shapefile import currently requires a .shp path".to_string(),
        ));
    }

    let bytes = fs::read(&path).await.map_err(|err| {
        AppError::BadRequest(format!(
            "failed to read shapefile {}: {err}",
            path.display()
        ))
    })?;
    let shapes = shapefile::parse_polygon_records(&path, &bytes)?;
    let base_name = request
        .name_prefix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "Imported Field".to_string());
    let single_shape = shapes.len() == 1;

    shapes
        .into_iter()
        .map(|shape| {
            let shape_name = if single_shape {
                base_name.clone()
            } else {
                format!("{} {}", base_name, shape.record_index + 1)
            };
            build_field_record(CreateFieldRequest {
                farm_id: request.farm_id.clone(),
                field_id: None,
                name: shape_name,
                crop: request.crop.clone(),
                season: request.season.clone(),
                notes: request.notes.clone(),
                boundary: FieldBoundary {
                    coordinates: shape.coordinates,
                },
            })
        })
        .collect()
}

fn group_fields_by_season(fields: Vec<FieldRecord>) -> Vec<FieldSeasonGroup> {
    let mut grouped: BTreeMap<Option<String>, Vec<FieldRecord>> = BTreeMap::new();
    for field in fields {
        grouped.entry(field.season.clone()).or_default().push(field);
    }

    grouped
        .into_iter()
        .rev()
        .map(|(season, fields)| FieldSeasonGroup { season, fields })
        .collect()
}

fn geojson_from_fields(fields: Vec<FieldRecord>) -> GeoJson {
    GeoJson::FeatureCollection(FeatureCollection {
        bbox: None,
        foreign_members: None,
        features: fields.into_iter().map(feature_from_field).collect(),
    })
}

fn response_with_bytes(bytes: Vec<u8>, content_type: &str, filename: &str) -> AppResult<Response> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).map_err(|err| AppError::Anyhow(err.into()))?,
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|err| AppError::Anyhow(err.into()))?,
    );

    Ok((headers, Body::from(bytes)).into_response())
}

fn feature_from_field(field: FieldRecord) -> Feature {
    let mut ring: Vec<Vec<f64>> = field
        .boundary
        .coordinates
        .iter()
        .map(|point| vec![point.longitude, point.latitude])
        .collect();
    if let Some(first) = ring.first().cloned() {
        ring.push(first);
    }

    let mut properties = serde_json::Map::new();
    properties.insert(
        "field_id".to_string(),
        serde_json::Value::String(field.field_id.clone()),
    );
    if let Some(farm_id) = field.farm_id {
        properties.insert("farm_id".to_string(), serde_json::Value::String(farm_id));
    }
    properties.insert("name".to_string(), serde_json::Value::String(field.name));
    if let Some(crop) = field.crop {
        properties.insert("crop".to_string(), serde_json::Value::String(crop));
    }
    if let Some(season) = field.season {
        properties.insert("season".to_string(), serde_json::Value::String(season));
    }
    if let Some(notes) = field.notes {
        properties.insert("notes".to_string(), serde_json::Value::String(notes));
    }

    Feature {
        bbox: None,
        geometry: Some(Geometry::new(GeoJsonValue::Polygon(vec![ring]))),
        id: Some(GeoJsonId::String(field.field_id)),
        properties: Some(properties),
        foreign_members: None,
    }
}

fn feature_from_annotation(annotation: &AnnotationRecord) -> AppResult<Feature> {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "annotation_id".to_string(),
        serde_json::Value::String(annotation.annotation_id.clone()),
    );
    properties.insert(
        "label".to_string(),
        serde_json::Value::String(annotation.label.clone()),
    );
    if let Some(severity) = annotation.severity.as_ref() {
        properties.insert(
            "severity".to_string(),
            serde_json::Value::String(severity.clone()),
        );
    }
    if let Some(note) = annotation.note.as_ref() {
        properties.insert("note".to_string(), serde_json::Value::String(note.clone()));
    }

    Ok(Feature {
        bbox: None,
        geometry: Some(geometry_from_annotation(&annotation.geometry)?),
        id: Some(GeoJsonId::String(annotation.annotation_id.clone())),
        properties: Some(properties),
        foreign_members: None,
    })
}

fn recommendation_features(
    recommendation: &RecommendationRecord,
    annotations: &[AnnotationRecord],
) -> AppResult<Vec<Feature>> {
    if recommendation.annotation_ids.is_empty() {
        let mut properties = serde_json::Map::new();
        populate_recommendation_properties(&mut properties, recommendation);
        return Ok(vec![Feature {
            bbox: None,
            geometry: None,
            id: Some(GeoJsonId::String(recommendation.recommendation_id.clone())),
            properties: Some(properties),
            foreign_members: None,
        }]);
    }

    let mut features = Vec::new();
    for annotation_id in &recommendation.annotation_ids {
        if let Some(annotation) = annotations
            .iter()
            .find(|annotation| annotation.annotation_id == *annotation_id)
        {
            let mut properties = serde_json::Map::new();
            populate_recommendation_properties(&mut properties, recommendation);
            properties.insert(
                "annotation_id".to_string(),
                serde_json::Value::String(annotation.annotation_id.clone()),
            );
            features.push(Feature {
                bbox: None,
                geometry: Some(geometry_from_annotation(&annotation.geometry)?),
                id: Some(GeoJsonId::String(format!(
                    "{}:{}",
                    recommendation.recommendation_id, annotation.annotation_id
                ))),
                properties: Some(properties),
                foreign_members: None,
            });
        }
    }

    Ok(features)
}

fn populate_recommendation_properties(
    properties: &mut serde_json::Map<String, serde_json::Value>,
    recommendation: &RecommendationRecord,
) {
    properties.insert(
        "recommendation_id".to_string(),
        serde_json::Value::String(recommendation.recommendation_id.clone()),
    );
    properties.insert(
        "title".to_string(),
        serde_json::Value::String(recommendation.title.clone()),
    );
    properties.insert(
        "priority".to_string(),
        serde_json::Value::String(recommendation_priority_str(recommendation.priority).to_string()),
    );
    properties.insert(
        "status".to_string(),
        serde_json::Value::String(recommendation_status_str(recommendation.status).to_string()),
    );
    if let Some(category) = recommendation.category.as_ref() {
        properties.insert(
            "category".to_string(),
            serde_json::Value::String(category.clone()),
        );
    }
    if let Some(note) = recommendation.note.as_ref() {
        properties.insert("note".to_string(), serde_json::Value::String(note.clone()));
    }
}

fn geometry_from_annotation(geometry: &AnnotationGeometry) -> AppResult<Geometry> {
    Ok(match geometry {
        AnnotationGeometry::Point { coordinate } => Geometry::new(GeoJsonValue::Point(vec![
            coordinate.longitude,
            coordinate.latitude,
        ])),
        AnnotationGeometry::Polygon { coordinates } => {
            let mut ring = coordinates
                .iter()
                .map(|coordinate| vec![coordinate.longitude, coordinate.latitude])
                .collect::<Vec<_>>();
            if let Some(first) = ring.first().cloned() {
                ring.push(first);
            }
            Geometry::new(GeoJsonValue::Polygon(vec![ring]))
        }
    })
}

fn validate_annotation_geometry(geometry: &AnnotationGeometry) -> AppResult<()> {
    match geometry {
        AnnotationGeometry::Point { coordinate } => {
            validate_geo_point(coordinate)?;
        }
        AnnotationGeometry::Polygon { coordinates } => {
            if coordinates.len() < 3 {
                return Err(AppError::BadRequest(
                    "polygon annotation must contain at least three coordinates".to_string(),
                ));
            }
            for coordinate in coordinates {
                validate_geo_point(coordinate)?;
            }
        }
    }
    Ok(())
}

fn validate_geo_point(point: &GeoPoint) -> AppResult<()> {
    if !point.longitude.is_finite()
        || !point.latitude.is_finite()
        || point.longitude < -180.0
        || point.longitude > 180.0
        || point.latitude < -90.0
        || point.latitude > 90.0
    {
        return Err(AppError::BadRequest(
            "annotation geometry contains invalid geographic coordinates".to_string(),
        ));
    }

    Ok(())
}

fn build_field_from_feature(feature: geojson::Feature, index: usize) -> AppResult<FieldRecord> {
    let geojson::Feature {
        geometry,
        id,
        properties,
        ..
    } = feature;
    let geometry = geometry
        .ok_or_else(|| AppError::BadRequest("GeoJSON feature is missing geometry".to_string()))?;
    let properties = properties.unwrap_or_default();

    let field_id = property_string(&properties, "field_id")
        .or_else(|| property_string(&properties, "id"))
        .or_else(|| id.as_ref().and_then(geojson_id_to_string));
    let name = property_string(&properties, "name")
        .or_else(|| property_string(&properties, "field_name"))
        .unwrap_or_else(|| format!("Imported Field {}", index + 1));

    build_field_from_geometry(
        geometry,
        Some(CreateFieldRequest {
            farm_id: None,
            field_id,
            name,
            crop: property_string(&properties, "crop"),
            season: property_string(&properties, "season"),
            notes: property_string(&properties, "notes"),
            boundary: FieldBoundary {
                coordinates: Vec::new(),
            },
        }),
        index,
    )
}

fn build_field_from_geometry(
    geometry: Geometry,
    template: Option<CreateFieldRequest>,
    index: usize,
) -> AppResult<FieldRecord> {
    let boundary = boundary_from_geometry(geometry)?;
    let template = template.unwrap_or(CreateFieldRequest {
        farm_id: None,
        field_id: None,
        name: format!("Imported Field {}", index + 1),
        crop: None,
        season: None,
        notes: None,
        boundary: FieldBoundary {
            coordinates: Vec::new(),
        },
    });

    build_field_record(CreateFieldRequest {
        farm_id: template.farm_id,
        field_id: template.field_id,
        name: template.name,
        crop: template.crop,
        season: template.season,
        notes: template.notes,
        boundary,
    })
}

fn boundary_from_geometry(geometry: Geometry) -> AppResult<FieldBoundary> {
    match geometry.value {
        GeoJsonValue::Polygon(rings) => {
            let exterior = rings.into_iter().next().ok_or_else(|| {
                AppError::BadRequest(
                    "GeoJSON polygon does not contain an exterior ring".to_string(),
                )
            })?;
            boundary_from_ring(exterior)
        }
        GeoJsonValue::MultiPolygon(polygons) => {
            let exterior = polygons
                .into_iter()
                .max_by_key(|polygon| polygon.first().map_or(0, Vec::len))
                .and_then(|polygon| polygon.into_iter().next())
                .ok_or_else(|| {
                    AppError::BadRequest(
                        "GeoJSON multipolygon does not contain a usable exterior ring".to_string(),
                    )
                })?;
            boundary_from_ring(exterior)
        }
        _ => Err(AppError::BadRequest(
            "only Polygon and MultiPolygon GeoJSON geometries are supported".to_string(),
        )),
    }
}

fn boundary_from_ring(ring: Vec<Vec<f64>>) -> AppResult<FieldBoundary> {
    let mut coordinates = Vec::with_capacity(ring.len());
    for position in ring {
        if position.len() < 2 {
            return Err(AppError::BadRequest(
                "GeoJSON polygon coordinates must contain longitude and latitude".to_string(),
            ));
        }
        coordinates.push(GeoPoint {
            longitude: position[0],
            latitude: position[1],
        });
    }

    if coordinates.len() >= 2 && coordinates.first() == coordinates.last() {
        coordinates.pop();
    }

    Ok(FieldBoundary { coordinates })
}

fn property_string(
    properties: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    properties.get(key).and_then(|value| match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    })
}

fn geojson_id_to_string(id: &GeoJsonId) -> Option<String> {
    match id {
        GeoJsonId::String(text) => Some(text.clone()),
        GeoJsonId::Number(number) => Some(number.to_string()),
    }
}

fn decode_field_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<FieldRecord> {
    let boundary_json: String = row.get("boundary_json");
    let boundary = serde_json::from_str::<FieldBoundary>(&boundary_json).map_err(|err| {
        AppError::Anyhow(anyhow::Error::new(err).context("failed to decode field boundary_json"))
    })?;
    let extent = bounds_from_points(&boundary.coordinates).ok_or_else(|| {
        AppError::Anyhow(anyhow::anyhow!(
            "field boundary does not contain any coordinates"
        ))
    })?;

    Ok(FieldRecord {
        farm_id: row.get("farm_id"),
        field_id: row.get("field_id"),
        name: row.get("name"),
        crop: row.get("crop"),
        season: row.get("season"),
        notes: row.get("notes"),
        boundary,
        extent,
    })
}

fn decode_farm_record(row: &sqlx::sqlite::SqliteRow) -> FarmRecord {
    FarmRecord {
        farm_id: row.get("farm_id"),
        name: row.get("name"),
        notes: row.get("notes"),
    }
}

fn decode_annotation_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<AnnotationRecord> {
    let geometry_json: String = row.get("geometry_json");
    let geometry = serde_json::from_str::<AnnotationGeometry>(&geometry_json).map_err(|err| {
        AppError::Anyhow(anyhow::Error::new(err).context("failed to decode annotation geometry"))
    })?;

    Ok(AnnotationRecord {
        annotation_id: row.get("annotation_id"),
        scene_id: row.get("scene_id"),
        field_id: row.get("field_id"),
        label: row.get("label"),
        note: row.get("note"),
        severity: row.get("severity"),
        geometry,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn decode_recommendation_record(
    state: &AppState,
    row: &sqlx::sqlite::SqliteRow,
) -> AppResult<RecommendationRecord> {
    let recommendation_id: String = row.get("recommendation_id");
    Ok(RecommendationRecord {
        recommendation_id: recommendation_id.clone(),
        scene_id: row.get("scene_id"),
        field_id: row.get("field_id"),
        title: row.get("title"),
        note: row.get("note"),
        category: row.get("category"),
        priority: parse_recommendation_priority(row.get("priority"))?,
        status: parse_recommendation_status(row.get("status"))?,
        annotation_ids: load_recommendation_annotation_ids(state, &recommendation_id).await?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn decode_report_record(row: &sqlx::sqlite::SqliteRow) -> AppResult<ReportRecord> {
    let scene_id: String = row.get("scene_id");
    let report_id: String = row.get("report_id");
    Ok(ReportRecord {
        report_id: report_id.clone(),
        scene_id: scene_id.clone(),
        field_id: row.get("field_id"),
        title: row.get("title"),
        format: parse_report_format(row.get("format"))?,
        artifact_path: row.get("path"),
        download_url: format!("/api/scenes/{scene_id}/reports/{report_id}"),
        annotation_count: row.get::<i64, _>("annotation_count") as usize,
        recommendation_count: row.get::<i64, _>("recommendation_count") as usize,
        created_at: row.get("created_at"),
    })
}

async fn load_field(state: &AppState, field_id: &str) -> AppResult<Option<FieldRecord>> {
    let row = sqlx::query(
        "SELECT field_id, farm_id, name, crop, season, notes, boundary_json FROM fields WHERE field_id = ?1",
    )
    .bind(field_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_field_record(&row)).transpose()
}

async fn load_farm(state: &AppState, farm_id: &str) -> AppResult<Option<FarmRecord>> {
    let row = sqlx::query("SELECT farm_id, name, notes FROM farms WHERE farm_id = ?1")
        .bind(farm_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(Error::from)?;

    Ok(row.map(|row| decode_farm_record(&row)))
}

async fn ensure_field_farm_exists(state: &AppState, farm_id: Option<&str>) -> AppResult<()> {
    if let Some(farm_id) = farm_id {
        if load_farm(state, farm_id).await?.is_none() {
            return Err(AppError::BadRequest(format!(
                "farm {} does not exist",
                farm_id
            )));
        }
    }
    Ok(())
}

async fn load_scene_field(
    state: &AppState,
    scene_row: Option<&sqlx::sqlite::SqliteRow>,
) -> AppResult<Option<FieldRecord>> {
    let Some(field_id) = scene_row.and_then(|row| row.get::<Option<String>, _>("field_id")) else {
        return Ok(None);
    };

    load_field(state, &field_id).await
}

async fn load_annotation(
    state: &AppState,
    scene_id: &str,
    annotation_id: &str,
) -> AppResult<Option<AnnotationRecord>> {
    let row = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, label, note, severity, geometry_json, created_at, updated_at
        FROM annotations
        WHERE scene_id = ?1 AND annotation_id = ?2
        "#,
    )
    .bind(scene_id)
    .bind(annotation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_annotation_record(&row)).transpose()
}

async fn load_recommendation(
    state: &AppState,
    scene_id: &str,
    recommendation_id: &str,
) -> AppResult<Option<RecommendationRecord>> {
    let row = sqlx::query(
        r#"
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, created_at, updated_at
        FROM recommendations
        WHERE scene_id = ?1 AND recommendation_id = ?2
        "#,
    )
    .bind(scene_id)
    .bind(recommendation_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    match row {
        Some(row) => Ok(Some(decode_recommendation_record(state, &row).await?)),
        None => Ok(None),
    }
}

async fn load_report(
    state: &AppState,
    scene_id: &str,
    report_id: &str,
) -> AppResult<Option<ReportRecord>> {
    let row = sqlx::query(
        r#"
        SELECT report_id, scene_id, field_id, title, format, path, annotation_count, recommendation_count, created_at
        FROM reports
        WHERE scene_id = ?1 AND report_id = ?2
        "#,
    )
    .bind(scene_id)
    .bind(report_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(Error::from)?;

    row.map(|row| decode_report_record(&row)).transpose()
}

async fn load_recommendation_annotation_ids(
    state: &AppState,
    recommendation_id: &str,
) -> AppResult<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT annotation_id
        FROM recommendation_annotations
        WHERE recommendation_id = ?1
        ORDER BY annotation_id ASC
        "#,
    )
    .bind(recommendation_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("annotation_id"))
        .collect())
}

async fn load_scene_annotation_records(
    state: &AppState,
    scene_id: &str,
) -> AppResult<Vec<AnnotationRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT annotation_id, scene_id, field_id, label, note, severity, geometry_json, created_at, updated_at
        FROM annotations
        WHERE scene_id = ?1
        ORDER BY created_at ASC
        "#,
    )
    .bind(scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut annotations = Vec::with_capacity(rows.len());
    for row in rows {
        annotations.push(decode_annotation_record(&row)?);
    }

    Ok(annotations)
}

async fn load_scene_recommendation_records(
    state: &AppState,
    scene_id: &str,
) -> AppResult<Vec<RecommendationRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT recommendation_id, scene_id, field_id, title, note, category, priority, status, created_at, updated_at
        FROM recommendations
        WHERE scene_id = ?1
        ORDER BY created_at DESC
        "#,
    )
    .bind(scene_id)
    .fetch_all(&state.pool)
    .await
    .map_err(Error::from)?;

    let mut recommendations = Vec::with_capacity(rows.len());
    for row in rows {
        recommendations.push(decode_recommendation_record(state, &row).await?);
    }

    Ok(recommendations)
}

async fn load_scene_field_id(state: &AppState, scene_id: &str) -> AppResult<Option<String>> {
    Ok(
        sqlx::query("SELECT field_id FROM scenes WHERE scene_id = ?1")
            .bind(scene_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(Error::from)?
            .and_then(|row| row.get::<Option<String>, _>("field_id")),
    )
}

async fn validate_recommendation_annotation_ids(
    state: &AppState,
    scene_id: &str,
    annotation_ids: &[String],
) -> AppResult<()> {
    for annotation_id in annotation_ids {
        let annotation_id = annotation_id.trim();
        if annotation_id.is_empty() {
            return Err(AppError::BadRequest(
                "recommendation annotation links cannot be empty".to_string(),
            ));
        }
        if load_annotation(state, scene_id, annotation_id)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest(format!(
                "annotation {} does not exist on this scene",
                annotation_id
            )));
        }
    }

    Ok(())
}

async fn persist_recommendation_annotations(
    state: &AppState,
    recommendation_id: &str,
    annotation_ids: &[String],
) -> AppResult<()> {
    sqlx::query("DELETE FROM recommendation_annotations WHERE recommendation_id = ?1")
        .bind(recommendation_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;

    for annotation_id in annotation_ids {
        sqlx::query(
            r#"
            INSERT INTO recommendation_annotations (recommendation_id, annotation_id)
            VALUES (?1, ?2)
            "#,
        )
        .bind(recommendation_id)
        .bind(annotation_id)
        .execute(&state.pool)
        .await
        .map_err(Error::from)?;
    }

    Ok(())
}

fn recommendation_priority_str(priority: RecommendationPriority) -> &'static str {
    match priority {
        RecommendationPriority::Low => "low",
        RecommendationPriority::Medium => "medium",
        RecommendationPriority::High => "high",
        RecommendationPriority::Critical => "critical",
    }
}

fn recommendation_status_str(status: RecommendationStatus) -> &'static str {
    match status {
        RecommendationStatus::Open => "open",
        RecommendationStatus::Reviewed => "reviewed",
        RecommendationStatus::Closed => "closed",
    }
}

fn parse_recommendation_priority(value: String) -> AppResult<RecommendationPriority> {
    match value.as_str() {
        "low" => Ok(RecommendationPriority::Low),
        "medium" => Ok(RecommendationPriority::Medium),
        "high" => Ok(RecommendationPriority::High),
        "critical" => Ok(RecommendationPriority::Critical),
        _ => Err(AppError::Anyhow(anyhow::anyhow!(
            "invalid recommendation priority {}",
            value
        ))),
    }
}

fn parse_recommendation_status(value: String) -> AppResult<RecommendationStatus> {
    match value.as_str() {
        "open" => Ok(RecommendationStatus::Open),
        "reviewed" => Ok(RecommendationStatus::Reviewed),
        "closed" => Ok(RecommendationStatus::Closed),
        _ => Err(AppError::Anyhow(anyhow::anyhow!(
            "invalid recommendation status {}",
            value
        ))),
    }
}

fn report_format_str(format: ReportFormat) -> &'static str {
    match format {
        ReportFormat::Html => "html",
    }
}

fn parse_report_format(value: String) -> AppResult<ReportFormat> {
    match value.as_str() {
        "html" => Ok(ReportFormat::Html),
        _ => Err(AppError::Anyhow(anyhow::anyhow!(
            "invalid report format {}",
            value
        ))),
    }
}

fn render_scene_report_html(
    scene_id: &str,
    sensor: Option<String>,
    acquired_at: Option<String>,
    metadata: Option<&MultispectralImage>,
    field: Option<&FieldRecord>,
    geospatial: &SceneGeospatialMetadata,
    annotations: &[AnnotationRecord],
    recommendations: &[RecommendationRecord],
    report_title: &str,
) -> String {
    let field_name = field
        .map(|field| field.name.clone())
        .unwrap_or_else(|| "Unlinked field".to_string());
    let map_svg = render_report_map_svg(field, geospatial, annotations, recommendations);
    let recommendations_html = recommendations
        .iter()
        .map(|recommendation| {
            format!(
                "<li><strong>{}</strong> [{} / {}]{}{} </li>",
                escape_html(&recommendation.title),
                recommendation_status_str(recommendation.status),
                recommendation_priority_str(recommendation.priority),
                recommendation
                    .category
                    .as_ref()
                    .map(|category| format!(" Category: {}.", escape_html(category)))
                    .unwrap_or_default(),
                recommendation
                    .note
                    .as_ref()
                    .map(|note| format!(" {}", escape_html(note)))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let annotations_html = annotations
        .iter()
        .map(|annotation| {
            format!(
                "<li><strong>{}</strong>{}{} </li>",
                escape_html(&annotation.label),
                annotation
                    .severity
                    .as_ref()
                    .map(|severity| format!(" [{}]", escape_html(severity)))
                    .unwrap_or_default(),
                annotation
                    .note
                    .as_ref()
                    .map(|note| format!(" {}", escape_html(note)))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{title}</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 32px; color: #1a1f26; background: #f7f4ee; }}
    h1, h2 {{ margin-bottom: 8px; }}
    .meta {{ display: grid; grid-template-columns: repeat(2, minmax(240px, 1fr)); gap: 12px; margin-bottom: 24px; }}
    .card {{ background: #ffffff; border: 1px solid #d8d0c4; border-radius: 10px; padding: 16px; }}
    .map {{ margin: 24px 0; background: #ffffff; border: 1px solid #d8d0c4; border-radius: 10px; padding: 16px; }}
    ul {{ padding-left: 20px; }}
    .muted {{ color: #5b6572; }}
  </style>
</head>
<body>
  <h1>{title}</h1>
  <p class="muted">Scene {scene_id} • Field {field_name}</p>
  <div class="meta">
    <div class="card"><strong>Sensor</strong><div>{sensor}</div></div>
    <div class="card"><strong>Acquired</strong><div>{acquired_at}</div></div>
    <div class="card"><strong>Raster</strong><div>{width} × {height} px</div></div>
    <div class="card"><strong>Products</strong><div>{bands}</div></div>
    <div class="card"><strong>Annotations</strong><div>{annotation_count}</div></div>
    <div class="card"><strong>Recommendations</strong><div>{recommendation_count}</div></div>
  </div>
  <div class="map">
    <h2>Field Snapshot</h2>
    {map_svg}
  </div>
  <div class="card">
    <h2>Findings</h2>
    <ul>{annotations_html}</ul>
  </div>
  <div class="card" style="margin-top: 16px;">
    <h2>Recommendations</h2>
    <ul>{recommendations_html}</ul>
  </div>
</body>
</html>"#,
        title = escape_html(report_title),
        scene_id = escape_html(scene_id),
        field_name = escape_html(&field_name),
        sensor = escape_html(sensor.as_deref().unwrap_or("unknown")),
        acquired_at = escape_html(acquired_at.as_deref().unwrap_or("n/a")),
        width = metadata
            .map(|image| image.metadata.width)
            .unwrap_or_default(),
        height = metadata
            .map(|image| image.metadata.height)
            .unwrap_or_default(),
        bands = escape_html(
            &metadata
                .map(|image| image.metadata.bands.join(", "))
                .unwrap_or_else(|| "n/a".to_string())
        ),
        annotation_count = annotations.len(),
        recommendation_count = recommendations.len(),
        annotations_html = annotations_html,
        recommendations_html = recommendations_html,
        map_svg = map_svg,
    )
}

fn render_report_map_svg(
    field: Option<&FieldRecord>,
    geospatial: &SceneGeospatialMetadata,
    annotations: &[AnnotationRecord],
    recommendations: &[RecommendationRecord],
) -> String {
    let width = 820.0;
    let height = 360.0;
    let extent = geospatial.extent.clone().or_else(|| {
        field.map(|field| SceneExtent {
            min_lon: field.extent.min_lon,
            min_lat: field.extent.min_lat,
            max_lon: field.extent.max_lon,
            max_lat: field.extent.max_lat,
        })
    });

    let Some(extent) = extent else {
        return "<div class=\"muted\">No geospatial extent available for map preview.</div>"
            .to_string();
    };

    let mut svg = format!(
        "<svg viewBox=\"0 0 {width} {height}\" width=\"100%\" height=\"{height}\" xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"100%\" height=\"100%\" fill=\"#f4efe5\"/>"
    );

    if let Some(field) = field {
        let points = field
            .boundary
            .coordinates
            .iter()
            .map(|point| svg_project(point.longitude, point.latitude, &extent, width, height))
            .map(|(x, y)| format!("{x:.1},{y:.1}"))
            .collect::<Vec<_>>()
            .join(" ");
        svg.push_str(&format!(
            "<polygon points=\"{}\" fill=\"#e4d7b5\" stroke=\"#967433\" stroke-width=\"2\"/>",
            points
        ));
    }

    for annotation in annotations {
        match &annotation.geometry {
            AnnotationGeometry::Point { coordinate } => {
                let (x, y) = svg_project(
                    coordinate.longitude,
                    coordinate.latitude,
                    &extent,
                    width,
                    height,
                );
                svg.push_str(&format!(
                    "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"6\" fill=\"#c64242\" stroke=\"#ffffff\" stroke-width=\"2\"/>"
                ));
            }
            AnnotationGeometry::Polygon { coordinates } => {
                let points = coordinates
                    .iter()
                    .map(|point| {
                        svg_project(point.longitude, point.latitude, &extent, width, height)
                    })
                    .map(|(x, y)| format!("{x:.1},{y:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                svg.push_str(&format!(
                    "<polygon points=\"{}\" fill=\"rgba(198,66,66,0.2)\" stroke=\"#c64242\" stroke-width=\"2\"/>",
                    points
                ));
            }
        }
    }

    for recommendation in recommendations {
        if recommendation.annotation_ids.is_empty() {
            continue;
        }
        svg.push_str(&format!(
            "<text x=\"16\" y=\"{}\" font-size=\"12\" fill=\"#1a1f26\">{} [{} / {}]</text>",
            22 + (recommendations
                .iter()
                .position(
                    |candidate| candidate.recommendation_id == recommendation.recommendation_id
                )
                .unwrap_or(0) as i32
                * 18),
            escape_html(&recommendation.title),
            recommendation_status_str(recommendation.status),
            recommendation_priority_str(recommendation.priority),
        ));
    }

    svg.push_str("</svg>");
    svg
}

fn svg_project(
    longitude: f64,
    latitude: f64,
    extent: &SceneExtent,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let x = if (extent.max_lon - extent.min_lon).abs() <= f64::EPSILON {
        width / 2.0
    } else {
        ((longitude - extent.min_lon) / (extent.max_lon - extent.min_lon)) * width
    };
    let y = if (extent.max_lat - extent.min_lat).abs() <= f64::EPSILON {
        height / 2.0
    } else {
        (1.0 - ((latitude - extent.min_lat) / (extent.max_lat - extent.min_lat))) * height
    };
    (x.clamp(0.0, width), y.clamp(0.0, height))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn scene_exists(state: &AppState, scene_id: &str) -> AppResult<bool> {
    let scene_in_db = sqlx::query("SELECT 1 FROM scenes WHERE scene_id = ?1 LIMIT 1")
        .bind(scene_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(Error::from)?
        .is_some();
    if scene_in_db {
        return Ok(true);
    }

    let scene_dir = state.config.data_root.join("scenes").join(scene_id);
    fs::try_exists(scene_dir)
        .await
        .map_err(|err| AppError::Anyhow(err.into()))
}

async fn load_scene_metadata(
    scene_row: Option<&sqlx::sqlite::SqliteRow>,
    scene_dir: &FsPath,
) -> AppResult<Option<MultispectralImage>> {
    if let Some(row) = scene_row {
        let metadata_json: String = row.get("metadata_json");
        let image = serde_json::from_str::<MultispectralImage>(&metadata_json).map_err(|err| {
            AppError::Anyhow(
                anyhow::Error::new(err)
                    .context("failed to decode scene metadata_json from database"),
            )
        })?;
        return Ok(Some(image));
    }

    let mut entries = match fs::read_dir(scene_dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(AppError::Anyhow(err.into())),
    };

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| AppError::Anyhow(err.into()))?
    {
        let path = entry.path();
        let is_metadata = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "metadata_ingested.json" || name.starts_with("metadata_"))
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if !is_metadata {
            continue;
        }

        let metadata_json = fs::read_to_string(&path)
            .await
            .map_err(|err| AppError::Anyhow(err.into()))?;
        let image = serde_json::from_str::<MultispectralImage>(&metadata_json).map_err(|err| {
            AppError::Anyhow(anyhow::Error::new(err).context(format!(
                "failed to decode scene metadata at {}",
                path.display()
            )))
        })?;
        return Ok(Some(image));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{
        build_field_record, build_geospatial_metadata, build_product_summary,
        content_type_for_path, is_missing_scene_error, is_png, AppError, CreateFieldRequest,
    };
    use shared::schemas::{
        FieldBoundary, GeoBounds, GeoPoint, GpsCoords, ImageMetadata, MultispectralImage,
        RasterSpatialRef,
    };
    use std::path::Path;
    use uuid::Uuid;

    #[test]
    fn content_type_detection_works() {
        assert_eq!(content_type_for_path(Path::new("tile.png")), "image/png");
        assert_eq!(content_type_for_path(Path::new("tile.JPG")), "image/jpeg");
        assert_eq!(content_type_for_path(Path::new("tile.tiff")), "image/tiff");
        assert_eq!(
            content_type_for_path(Path::new("tile.unknown")),
            "application/octet-stream"
        );
    }

    #[test]
    fn png_extension_detection_is_case_insensitive() {
        assert!(is_png(Path::new("x.png")));
        assert!(is_png(Path::new("x.PNG")));
        assert!(!is_png(Path::new("x.jpeg")));
    }

    #[test]
    fn row_not_found_errors_are_detected() {
        let err = anyhow::Error::new(sqlx::Error::RowNotFound);
        assert!(is_missing_scene_error(&err));
    }

    #[test]
    fn product_summary_contains_expected_url_and_filename() {
        let summary = build_product_summary("scene-1", "ndvi", Path::new("/tmp/output.png"));
        assert_eq!(summary.filename, "output.png");
        assert_eq!(summary.content_type, "image/png");
        assert_eq!(summary.url_path, "/api/scenes/scene-1/products/ndvi");
    }

    #[test]
    fn geospatial_metadata_uses_available_center_but_not_fake_extent() {
        let image = MultispectralImage {
            image_id: Uuid::nil(),
            metadata: ImageMetadata {
                timestamp: "2025-01-01T00:00:00Z"
                    .parse()
                    .expect("timestamp should parse"),
                gps_position: Some(GpsCoords {
                    latitude: 40.7128,
                    longitude: -74.0060,
                    altitude: 12.0,
                }),
                bands: vec!["B4".to_string(), "B5".to_string()],
                exposure_time: 1.0,
                gain: 1.0,
                width: 512,
                height: 256,
                spatial_ref: None,
            },
            file_paths: Default::default(),
        };

        let geospatial = build_geospatial_metadata(Some(&image));

        assert!(!geospatial.georeferenced);
        assert_eq!(geospatial.crs, None);
        assert_eq!(
            geospatial.center.as_ref().map(|gps| gps.latitude),
            Some(40.7128)
        );
        assert_eq!(geospatial.extent, None);
    }

    #[test]
    fn geospatial_metadata_defaults_when_no_metadata_exists() {
        let geospatial = build_geospatial_metadata(None);

        assert!(!geospatial.georeferenced);
        assert_eq!(geospatial.crs, None);
        assert!(geospatial.center.is_none());
        assert_eq!(geospatial.extent, None);
    }

    #[test]
    fn geospatial_metadata_prefers_bbox_when_available() {
        let image = MultispectralImage {
            image_id: Uuid::nil(),
            metadata: ImageMetadata {
                timestamp: "2025-01-01T00:00:00Z"
                    .parse()
                    .expect("timestamp should parse"),
                gps_position: Some(GpsCoords {
                    latitude: 1.0,
                    longitude: 2.0,
                    altitude: 3.0,
                }),
                bands: vec!["B4".to_string(), "B5".to_string()],
                exposure_time: 1.0,
                gain: 1.0,
                width: 512,
                height: 256,
                spatial_ref: Some(RasterSpatialRef {
                    georeferenced: true,
                    crs: Some("EPSG:4326".to_string()),
                    bbox: Some(GeoBounds {
                        min_lon: -74.1,
                        min_lat: 40.6,
                        max_lon: -73.9,
                        max_lat: 40.8,
                    }),
                    geo_transform: Some([-74.1, 0.0001, 0.0, 40.8, 0.0, -0.0001]),
                }),
            },
            file_paths: Default::default(),
        };

        let geospatial = build_geospatial_metadata(Some(&image));

        assert!(geospatial.georeferenced);
        assert_eq!(geospatial.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(
            geospatial.center.as_ref().map(|gps| gps.latitude),
            Some(40.7)
        );
        assert_eq!(
            geospatial.center.as_ref().map(|gps| gps.longitude),
            Some(-74.0)
        );
        assert_eq!(
            geospatial.extent,
            Some(super::SceneExtent {
                min_lon: -74.1,
                min_lat: 40.6,
                max_lon: -73.9,
                max_lat: 40.8,
            })
        );
    }

    #[test]
    fn build_field_record_computes_extent_from_boundary() {
        let field = build_field_record(CreateFieldRequest {
            farm_id: None,
            field_id: Some("north-80".to_string()),
            name: "North 80".to_string(),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: Some("test field".to_string()),
            boundary: FieldBoundary {
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.4,
                    },
                ],
            },
        })
        .expect("field should build");

        assert_eq!(field.field_id, "north-80");
        assert_eq!(field.name, "North 80");
        assert_eq!(
            field.extent,
            GeoBounds {
                min_lon: -96.7,
                min_lat: 41.1,
                max_lon: -96.2,
                max_lat: 41.4,
            }
        );
    }

    #[test]
    fn build_field_record_rejects_short_boundary() {
        let err = build_field_record(CreateFieldRequest {
            farm_id: None,
            field_id: None,
            name: "Short boundary".to_string(),
            crop: None,
            season: None,
            notes: None,
            boundary: FieldBoundary {
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.1,
                    },
                ],
            },
        })
        .expect_err("boundary should be rejected");

        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn build_field_record_rejects_invalid_coordinate_ranges() {
        let err = build_field_record(CreateFieldRequest {
            farm_id: None,
            field_id: None,
            name: "Bad coordinates".to_string(),
            crop: None,
            season: None,
            notes: None,
            boundary: FieldBoundary {
                coordinates: vec![
                    GeoPoint {
                        longitude: -96.7,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: 200.0,
                        latitude: 41.1,
                    },
                    GeoPoint {
                        longitude: -96.2,
                        latitude: 41.4,
                    },
                ],
            },
        })
        .expect_err("invalid coordinates should be rejected");

        assert!(matches!(err, AppError::BadRequest(_)));
    }
}
