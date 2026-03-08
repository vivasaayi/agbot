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
use shared::schemas::{GpsCoords, MultispectralImage};
use sqlx::Row;
use std::collections::BTreeMap;
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
    pub available_products: Vec<ProductSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductSummary {
    pub kind: String,
    pub filename: String,
    pub content_type: String,
    pub url_path: String,
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
        "SELECT scene_id, sensor, acquired_at, data_path, metadata_json FROM scenes WHERE scene_id = ?1",
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
        available_products,
    }))
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

    select_preferred_product_path(&mut entries).await
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
    }
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
    use super::{build_product_summary, content_type_for_path, is_missing_scene_error, is_png};
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

    #[test]
    fn product_summary_contains_expected_url_and_filename() {
        let summary = build_product_summary("scene-1", "ndvi", Path::new("/tmp/output.png"));
        assert_eq!(summary.filename, "output.png");
        assert_eq!(summary.content_type, "image/png");
        assert_eq!(summary.url_path, "/api/scenes/scene-1/products/ndvi");
    }
}
