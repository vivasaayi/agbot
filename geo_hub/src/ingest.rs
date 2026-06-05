use crate::{config::HubConfig, db::DbPool};
use anyhow::{anyhow, Result};
use clap::Args;
use imagery_processor::{IndexKind, IndicesArgs, OutputFormat, Processor, SensorPreset};
use serde::Serialize;
use shared::schemas::MultispectralImage;
use sqlx::Row;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Args, Debug)]
pub struct IngestLandsatArgs {
    /// Scene identifier (e.g., LC08_L1TP_044034_20210101_20210115_01_T1)
    #[arg(long)]
    pub scene_id: String,
    /// Path to directory containing metadata_*.json and band files
    #[arg(long)]
    pub source_dir: PathBuf,
}

#[derive(Debug, Serialize)]
struct SceneMetadataSummary {
    scene_id: String,
    bands: Vec<String>,
    width: u32,
    height: u32,
    timestamp: chrono::DateTime<chrono::Utc>,
    image_id: Uuid,
}

pub async fn ingest_landsat(
    args: IngestLandsatArgs,
    config: &HubConfig,
    pool: &DbPool,
) -> Result<()> {
    let scenes_root = config.data_root.join("scenes");
    fs::create_dir_all(&scenes_root).await?;

    let metadata_path = discover_metadata(&args.source_dir).await?;
    let metadata_json_original = fs::read_to_string(&metadata_path).await?;
    let mut image: MultispectralImage = serde_json::from_str(&metadata_json_original)?;

    let scene_dir = scenes_root.join(&args.scene_id);
    if scene_dir.exists() {
        warn!(scene = %args.scene_id, "scene already ingested, overwriting metadata only");
    }
    fs::create_dir_all(&scene_dir).await?;

    let mut rewritten_paths = HashMap::new();
    for (band, path) in &image.file_paths {
        let src = resolve_band_source(&args.source_dir, path);
        let file_name = src
            .file_name()
            .map(|f| f.to_owned())
            .unwrap_or_else(|| std::ffi::OsString::from(format!("{}_band", band)));
        let dest = scene_dir.join(&file_name);
        if src.exists() {
            fs::copy(&src, &dest).await?;
            rewritten_paths.insert(band.clone(), dest.to_string_lossy().to_string());
        } else {
            warn!(
                band,
                path, "band file missing, keeping original path reference"
            );
            rewritten_paths.insert(band.clone(), path.clone());
        }
    }
    image.file_paths = rewritten_paths;

    let metadata_filename = metadata_path
        .file_name()
        .map(|f| f.to_owned())
        .unwrap_or_else(|| std::ffi::OsString::from("metadata_ingested.json"));
    let metadata_json = serde_json::to_string_pretty(&image)?;
    fs::write(scene_dir.join(&metadata_filename), &metadata_json).await?;

    let summary = SceneMetadataSummary {
        scene_id: args.scene_id.clone(),
        bands: image.metadata.bands.clone(),
        width: image.metadata.width,
        height: image.metadata.height,
        timestamp: image.metadata.timestamp,
        image_id: image.image_id,
    };

    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at)
        VALUES (?1, 'landsat8', ?2, ?3, ?4, NULL, datetime('now'))
        ON CONFLICT(scene_id) DO UPDATE SET metadata_json = excluded.metadata_json,
                                          data_path = excluded.data_path,
                                          acquired_at = excluded.acquired_at
        "#,
    )
    .bind(&args.scene_id)
    .bind(summary.timestamp.to_rfc3339())
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(&metadata_json)
    .execute(pool)
    .await?;

    info!(scene = %args.scene_id, "scene ingested");

    Ok(())
}

async fn discover_metadata(source_dir: &Path) -> Result<PathBuf> {
    let mut metadata_path = None;
    let mut entries = fs::read_dir(source_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|os| os.to_str())
            .map_or(false, |name| name.starts_with("metadata_"))
            && path.extension().map_or(false, |ext| ext == "json")
        {
            metadata_path = Some(path);
            break;
        }
    }

    metadata_path.ok_or_else(|| anyhow!("metadata file not found in {}", source_dir.display()))
}

pub async fn ensure_product(pool: &DbPool, scene_id: &str, kind: &str) -> Result<PathBuf> {
    if let Some(path) = existing_product(pool, scene_id, kind).await? {
        return Ok(path);
    }

    info!(scene = scene_id, kind, "generating product");

    let row = sqlx::query("SELECT data_path, metadata_json FROM scenes WHERE scene_id = ?1")
        .bind(scene_id)
        .fetch_one(pool)
        .await?;

    let data_path: String = row.get("data_path");
    let metadata_json: String = row.get("metadata_json");
    let scene_dir = PathBuf::from(&data_path);
    let products_root = scene_dir.join("products");
    fs::create_dir_all(&products_root).await?;
    let product_dir = products_root.join(kind);

    if product_dir.exists() {
        let mut entries = fs::read_dir(&product_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().is_file() {
                fs::remove_file(entry.path()).await?;
            }
        }
    }
    fs::create_dir_all(&product_dir).await?;

    // Ensure metadata is present within the scene directory for downstream tools.
    let metadata_path = scene_dir.join("metadata_ingested.json");
    if !metadata_path.exists() {
        fs::write(&metadata_path, &metadata_json).await?;
    }

    let (index_kind, sensor) = match kind.to_lowercase().as_str() {
        "ndvi" => (IndexKind::Ndvi, Some(SensorPreset::Landsat8)),
        "ndre" => (IndexKind::Ndre, Some(SensorPreset::Landsat8)),
        "evi" => (IndexKind::Evi, Some(SensorPreset::Landsat8)),
        "savi" => (IndexKind::Savi, Some(SensorPreset::Landsat8)),
        "vari" => (IndexKind::Vari, Some(SensorPreset::Landsat8)),
        "gndvi" => (IndexKind::Gndvi, Some(SensorPreset::Landsat8)),
        "ndwi" => (IndexKind::Ndwi, Some(SensorPreset::Landsat8)),
        "mndwi" => (IndexKind::Mndwi, Some(SensorPreset::Landsat8)),
        "msavi" => (IndexKind::Msavi, Some(SensorPreset::Landsat8)),
        "nbr" => (IndexKind::Nbr, Some(SensorPreset::Landsat8)),
        "ndmi" => (IndexKind::Ndmi, Some(SensorPreset::Landsat8)),
        "evi2" => (IndexKind::Evi2, Some(SensorPreset::Landsat8)),
        other => return Err(anyhow!("unsupported product kind: {other}")),
    };

    let indices_args = IndicesArgs {
        input_dir: scene_dir.clone(),
        output_dir: product_dir.clone(),
        index: index_kind,
        red: Some("B4".to_string()),
        nir: Some("B5".to_string()),
        red_edge: Some("B6".to_string()),
        green: Some("B3".to_string()),
        blue: Some("B2".to_string()),
        swir1: Some("B6".to_string()),
        swir2: Some("B7".to_string()),
        out_format: OutputFormat::Png,
        sensor,
        mask: None,
    };

    let processor = Processor::new().await?;
    processor.run_indices(&indices_args).await?;

    let product_path = find_latest_file(&product_dir, &["png", "tif", "tiff"]).await?;

    sqlx::query(
        r#"
        INSERT INTO products (scene_id, kind, path, created_at)
        VALUES (?1, ?2, ?3, datetime('now'))
        ON CONFLICT(scene_id, kind) DO UPDATE SET path = excluded.path,
                                                created_at = datetime('now')
        "#,
    )
    .bind(scene_id)
    .bind(kind)
    .bind(product_path.to_string_lossy().to_string())
    .execute(pool)
    .await?;

    Ok(product_path)
}

async fn existing_product(pool: &DbPool, scene_id: &str, kind: &str) -> Result<Option<PathBuf>> {
    let row = sqlx::query("SELECT path FROM products WHERE scene_id = ?1 AND kind = ?2")
        .bind(scene_id)
        .bind(kind)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| PathBuf::from(r.get::<String, _>("path"))))
}

fn resolve_band_source(base: &Path, candidate: &str) -> PathBuf {
    let direct = PathBuf::from(candidate);
    if direct.is_absolute() {
        direct
    } else {
        base.join(direct)
    }
}

async fn find_latest_file(dir: &Path, extensions: &[&str]) -> Result<PathBuf> {
    let mut entries = fs::read_dir(dir).await?;
    let mut latest: Option<(SystemTime, PathBuf)> = None;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        {
            continue;
        }
        let modified = entry
            .metadata()
            .await?
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        match &mut latest {
            Some((current_time, current_path)) => {
                if modified > *current_time {
                    *current_time = modified;
                    *current_path = path;
                }
            }
            None => latest = Some((modified, path)),
        }
    }

    latest
        .map(|(_, path)| path)
        .ok_or_else(|| anyhow!("no product files produced in {}", dir.display()))
}
