use crate::{
    error::{AppError, AppResult},
    ingest,
    state::AppState,
};
use anyhow::Error;
use axum::response::{IntoResponse, Response};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    Json,
};
use serde::Serialize;
use sqlx::Row;
use std::io::ErrorKind;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

#[derive(Debug, Serialize)]
pub struct SceneSummary {
    pub scene_id: String,
    pub sensor: String,
    pub acquired_at: String,
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

pub async fn stream_product(
    Path((scene_id, kind)): Path<(String, String)>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let product_path = ingest::ensure_product(&state.pool, &scene_id, &kind)
        .await
        .map_err(AppError::from)?;

    let file = File::open(&product_path)
        .await
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => AppError::NotFound,
            _ => AppError::Anyhow(error.into()),
        })?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = match product_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("tif") | Some("tiff") => "image/tiff",
        _ => "application/octet-stream",
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));

    if let Some(filename) = product_path.file_name().and_then(|name| name.to_str()) {
        if let Ok(value) = HeaderValue::from_str(&format!("inline; filename=\"{}\"", filename)) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }

    Ok((headers, body).into_response())
}
