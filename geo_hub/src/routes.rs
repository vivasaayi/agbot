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
use std::path::{Path as FsPath, PathBuf};
use tokio::fs::File;
use tokio::fs::{self, DirEntry};
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
    let product_path =
        if let Some(path) = find_product_file_on_disk(&state, &scene_id, &kind).await? {
            path
        } else {
            match ingest::ensure_product(&state.pool, &scene_id, &kind).await {
                Ok(path) => path,
                Err(err) if is_missing_scene_error(&err) => return Err(AppError::NotFound),
                Err(err) => return Err(AppError::Anyhow(err)),
            }
        };

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
                // Prefer PNG when available because geo_viewer expects image bytes for direct display.
                if is_png(&path) && !is_png(current) {
                    selected = Some(path);
                }
            }
        }
    }

    Ok(selected)
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

#[cfg(test)]
mod tests {
    use super::{content_type_for_path, is_missing_scene_error, is_png};
    use std::path::Path;

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
}
