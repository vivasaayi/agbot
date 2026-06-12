use crate::{config::HubConfig, db::DbPool, product_catalog};
use anyhow::{anyhow, Result};
use clap::Args;
use imagery_processor::{IndexKind, IndicesArgs, OutputFormat, Processor, SensorPreset};
use serde::{Deserialize, Serialize};
use shared::schemas::{
    assert_raster_spatial_ref, MultispectralImage, RasterSpatialRef, DEFAULT_RECORD_OWNER,
};
use sqlx::Row;
use std::{
    collections::HashMap,
    fmt,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneIngestStatus {
    Queued,
    Downloading,
    Processing,
    Stored,
    Failed,
}

impl SceneIngestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Downloading => "Downloading",
            Self::Processing => "Processing",
            Self::Stored => "Stored",
            Self::Failed => "Failed",
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Downloading)
                | (Self::Downloading, Self::Processing)
                | (Self::Downloading, Self::Failed)
                | (Self::Processing, Self::Stored)
                | (Self::Processing, Self::Failed)
                | (Self::Stored, Self::Queued)
                | (Self::Failed, Self::Queued)
        )
    }
}

impl TryFrom<&str> for SceneIngestStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "Queued" => Ok(Self::Queued),
            "Downloading" => Ok(Self::Downloading),
            "Processing" => Ok(Self::Processing),
            "Stored" => Ok(Self::Stored),
            "Failed" => Ok(Self::Failed),
            other => Err(anyhow!("unknown scene ingest status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneIngestRecord {
    pub scene_id: String,
    pub status: SceneIngestStatus,
    pub status_reason: Option<String>,
    pub ingested_at: Option<String>,
    pub acquisition_date: Option<String>,
    pub coverage_fraction: Option<f64>,
}

#[derive(Debug)]
struct IngestStepError {
    reason_code: &'static str,
    error: anyhow::Error,
}

impl fmt::Display for IngestStepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.reason_code, self.error)
    }
}

fn ingest_step_error<E>(reason_code: &'static str, error: E) -> IngestStepError
where
    E: Into<anyhow::Error>,
{
    IngestStepError {
        reason_code,
        error: error.into(),
    }
}

type IngestStepResult<T> = std::result::Result<T, IngestStepError>;

pub async fn ingest_landsat(
    args: IngestLandsatArgs,
    config: &HubConfig,
    pool: &DbPool,
) -> Result<SceneIngestRecord> {
    let scenes_root = config.data_root.join("scenes");
    fs::create_dir_all(&scenes_root).await?;
    let scene_dir = scenes_root.join(&args.scene_id);
    let source_path = args.source_dir.to_string_lossy().to_string();

    record_ingest_status(
        pool,
        &args.scene_id,
        SceneIngestStatus::Queued,
        None,
        None,
        None,
        None,
        &source_path,
    )
    .await?;
    record_ingest_status(
        pool,
        &args.scene_id,
        SceneIngestStatus::Downloading,
        None,
        None,
        None,
        None,
        &source_path,
    )
    .await?;

    match ingest_landsat_inner(&args, pool, &scene_dir, &source_path).await {
        Ok(record) => Ok(record),
        Err(err) => {
            if let Err(cleanup_err) = cleanup_failed_ingest(pool, &args.scene_id, &scene_dir).await
            {
                warn!(
                    scene = %args.scene_id,
                    error = %cleanup_err,
                    "failed to clean up partial scene ingest"
                );
            }
            let _ = record_ingest_status(
                pool,
                &args.scene_id,
                SceneIngestStatus::Failed,
                Some(err.reason_code),
                None,
                None,
                None,
                &source_path,
            )
            .await?;
            Err(anyhow!(err.to_string()))
        }
    }
}

async fn ingest_landsat_inner(
    args: &IngestLandsatArgs,
    pool: &DbPool,
    scene_dir: &Path,
    source_path: &str,
) -> IngestStepResult<SceneIngestRecord> {
    let metadata_path = discover_metadata(&args.source_dir)
        .await
        .map_err(|err| ingest_step_error("download_error", err))?;
    let metadata_json_original = fs::read_to_string(&metadata_path)
        .await
        .map_err(|err| ingest_step_error("download_error", err))?;
    let mut image: MultispectralImage = serde_json::from_str(&metadata_json_original)
        .map_err(|err| ingest_step_error("metadata_error", err))?;
    let spatial_ref = assert_raster_spatial_ref(
        image.metadata.spatial_ref.as_ref(),
        image.metadata.width,
        image.metadata.height,
    )
    .map_err(|err| ingest_step_error("georeferencing_error", err))?;
    image.metadata.spatial_ref = Some(spatial_ref.clone());

    if scene_dir.exists() {
        warn!(scene = %args.scene_id, "scene already ingested, overwriting metadata only");
    }
    fs::create_dir_all(&scene_dir)
        .await
        .map_err(|err| ingest_step_error("store_error", err))?;
    let coverage_fraction = copy_scene_assets(&args.source_dir, scene_dir, &mut image).await?;

    record_ingest_status(
        pool,
        &args.scene_id,
        SceneIngestStatus::Processing,
        None,
        None,
        None,
        Some(coverage_fraction),
        source_path,
    )
    .await
    .map_err(|err| ingest_step_error("store_error", err))?;

    let metadata_filename = metadata_path
        .file_name()
        .map(|f| f.to_owned())
        .unwrap_or_else(|| std::ffi::OsString::from("metadata_ingested.json"));
    let metadata_json = serde_json::to_string_pretty(&image)
        .map_err(|err| ingest_step_error("metadata_error", err))?;
    fs::write(scene_dir.join(&metadata_filename), &metadata_json)
        .await
        .map_err(|err| ingest_step_error("processing_error", err))?;

    let summary = SceneMetadataSummary {
        scene_id: args.scene_id.clone(),
        bands: image.metadata.bands.clone(),
        width: image.metadata.width,
        height: image.metadata.height,
        timestamp: image.metadata.timestamp,
        image_id: image.image_id,
    };
    let acquisition_date = summary.timestamp.date_naive().to_string();
    let ingested_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    sqlx::query(
        r#"
        INSERT INTO scenes (scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at)
        VALUES (?1, ?2, 'landsat8', ?3, ?4, ?5, NULL, datetime('now'))
        ON CONFLICT(scene_id) DO UPDATE SET owner = excluded.owner,
                                          metadata_json = excluded.metadata_json,
                                          data_path = excluded.data_path,
                                          acquired_at = excluded.acquired_at
        "#,
    )
    .bind(&args.scene_id)
    .bind(DEFAULT_RECORD_OWNER)
    .bind(summary.timestamp.to_rfc3339())
    .bind(scene_dir.to_string_lossy().to_string())
    .bind(&metadata_json)
    .execute(pool)
    .await
    .map_err(|err| ingest_step_error("store_error", err))?;

    store_scene_spatial_ref(pool, &args.scene_id, &spatial_ref)
        .await
        .map_err(|err| ingest_step_error("store_error", err))?;

    let record = record_ingest_status(
        pool,
        &args.scene_id,
        SceneIngestStatus::Stored,
        None,
        Some(&ingested_at),
        Some(&acquisition_date),
        Some(coverage_fraction),
        source_path,
    )
    .await
    .map_err(|err| ingest_step_error("store_error", err))?;

    info!(scene = %args.scene_id, "scene ingested");

    Ok(record)
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

async fn copy_scene_assets(
    source_dir: &Path,
    scene_dir: &Path,
    image: &mut MultispectralImage,
) -> IngestStepResult<f64> {
    let total_assets = image.file_paths.len();
    let mut copied_assets = 0usize;
    let mut rewritten_paths = HashMap::new();

    for (band, path) in &image.file_paths {
        let src = resolve_band_source(source_dir, path);
        let file_name = src
            .file_name()
            .map(|f| f.to_owned())
            .unwrap_or_else(|| std::ffi::OsString::from(format!("{}_band", band)));
        let dest = scene_dir.join(&file_name);
        if !src.exists() {
            warn!(
                band,
                path, "band file missing, keeping original path reference"
            );
            rewritten_paths.insert(band.clone(), path.clone());
            continue;
        }
        let metadata = fs::metadata(&src)
            .await
            .map_err(|err| ingest_step_error("download_error", err))?;
        if !metadata.is_file() {
            return Err(ingest_step_error(
                "download_error",
                anyhow!("source asset {} is not a file", src.display()),
            ));
        }
        fs::copy(&src, &dest)
            .await
            .map_err(|err| ingest_step_error("download_error", err))?;
        copied_assets += 1;
        rewritten_paths.insert(band.clone(), dest.to_string_lossy().to_string());
    }

    image.file_paths = rewritten_paths;
    if total_assets == 0 {
        Ok(0.0)
    } else {
        Ok(copied_assets as f64 / total_assets as f64)
    }
}

pub async fn load_ingest_record(
    pool: &DbPool,
    scene_id: &str,
) -> Result<Option<SceneIngestRecord>> {
    let row = sqlx::query(
        r#"
        SELECT scene_id, status, status_reason, ingested_at, acquisition_date, coverage_fraction
        FROM scene_ingests
        WHERE scene_id = ?1
        "#,
    )
    .bind(scene_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let status_text: String = row.get("status");
        Ok(SceneIngestRecord {
            scene_id: row.get("scene_id"),
            status: SceneIngestStatus::try_from(status_text.as_str())?,
            status_reason: row.get("status_reason"),
            ingested_at: row.get("ingested_at"),
            acquisition_date: row.get("acquisition_date"),
            coverage_fraction: row.get("coverage_fraction"),
        })
    })
    .transpose()
}

pub async fn load_scene_spatial_ref(
    pool: &DbPool,
    scene_id: &str,
) -> Result<Option<RasterSpatialRef>> {
    let row = sqlx::query(
        r#"
        SELECT spatial_ref_json
        FROM scene_spatial_refs
        WHERE scene_id = ?1
        "#,
    )
    .bind(scene_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let spatial_ref_json: String = row.get("spatial_ref_json");
        serde_json::from_str(&spatial_ref_json).map_err(anyhow::Error::from)
    })
    .transpose()
}

async fn store_scene_spatial_ref(
    pool: &DbPool,
    scene_id: &str,
    spatial_ref: &RasterSpatialRef,
) -> Result<()> {
    let crs = spatial_ref
        .crs
        .as_deref()
        .ok_or_else(|| anyhow!("asserted spatial ref missing CRS"))?;
    let bbox = spatial_ref
        .bbox
        .as_ref()
        .ok_or_else(|| anyhow!("asserted spatial ref missing extent"))?;
    let resolution = spatial_ref
        .resolution
        .ok_or_else(|| anyhow!("asserted spatial ref missing resolution"))?;
    let geo_transform = spatial_ref
        .geo_transform
        .ok_or_else(|| anyhow!("asserted spatial ref missing transform"))?;
    let spatial_ref_json = serde_json::to_string(spatial_ref)?;
    let geo_transform_json = serde_json::to_string(&geo_transform)?;

    sqlx::query(
        r#"
        INSERT INTO scene_spatial_refs (
            scene_id, spatial_ref_json, crs, min_lon, min_lat, max_lon, max_lat,
            resolution_x, resolution_y, geo_transform_json, asserted_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
        ON CONFLICT(scene_id) DO UPDATE SET
            spatial_ref_json = excluded.spatial_ref_json,
            crs = excluded.crs,
            min_lon = excluded.min_lon,
            min_lat = excluded.min_lat,
            max_lon = excluded.max_lon,
            max_lat = excluded.max_lat,
            resolution_x = excluded.resolution_x,
            resolution_y = excluded.resolution_y,
            geo_transform_json = excluded.geo_transform_json,
            asserted_at = datetime('now')
        "#,
    )
    .bind(scene_id)
    .bind(spatial_ref_json)
    .bind(crs)
    .bind(bbox.min_lon)
    .bind(bbox.min_lat)
    .bind(bbox.max_lon)
    .bind(bbox.max_lat)
    .bind(resolution.x)
    .bind(resolution.y)
    .bind(geo_transform_json)
    .execute(pool)
    .await?;

    Ok(())
}

async fn record_ingest_status(
    pool: &DbPool,
    scene_id: &str,
    status: SceneIngestStatus,
    status_reason: Option<&str>,
    ingested_at: Option<&str>,
    acquisition_date: Option<&str>,
    coverage_fraction: Option<f64>,
    source_path: &str,
) -> Result<SceneIngestRecord> {
    sqlx::query(
        r#"
        INSERT INTO scene_ingests (
            scene_id, status, status_reason, ingested_at, acquisition_date,
            coverage_fraction, source_path, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
        ON CONFLICT(scene_id) DO UPDATE SET
            status = excluded.status,
            status_reason = excluded.status_reason,
            ingested_at = excluded.ingested_at,
            acquisition_date = excluded.acquisition_date,
            coverage_fraction = excluded.coverage_fraction,
            source_path = excluded.source_path,
            updated_at = datetime('now')
        "#,
    )
    .bind(scene_id)
    .bind(status.as_str())
    .bind(status_reason)
    .bind(ingested_at)
    .bind(acquisition_date)
    .bind(coverage_fraction)
    .bind(source_path)
    .execute(pool)
    .await?;

    Ok(SceneIngestRecord {
        scene_id: scene_id.to_string(),
        status,
        status_reason: status_reason.map(ToOwned::to_owned),
        ingested_at: ingested_at.map(ToOwned::to_owned),
        acquisition_date: acquisition_date.map(ToOwned::to_owned),
        coverage_fraction,
    })
}

async fn cleanup_failed_ingest(pool: &DbPool, scene_id: &str, scene_dir: &Path) -> Result<()> {
    if fs::try_exists(scene_dir).await.unwrap_or(false) {
        fs::remove_dir_all(scene_dir).await?;
    }
    sqlx::query("DELETE FROM products WHERE scene_id = ?1")
        .bind(scene_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM scene_spatial_refs WHERE scene_id = ?1")
        .bind(scene_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM scenes WHERE scene_id = ?1")
        .bind(scene_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn ensure_product(pool: &DbPool, scene_id: &str, kind: &str) -> Result<PathBuf> {
    if let Some(path) = existing_product(pool, scene_id, kind).await? {
        product_catalog::publish_product(pool, scene_id, kind, &path).await?;
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

    product_catalog::publish_product(pool, scene_id, kind, &product_path).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use shared::schemas::{
        GeoBounds, ImageMetadata, MultispectralImage, RasterResolution, RasterSpatialRef,
    };
    use tempfile::TempDir;

    #[test]
    fn scene_ingest_status_lifecycle_is_ordered() {
        assert!(SceneIngestStatus::Queued.can_transition_to(SceneIngestStatus::Downloading));
        assert!(SceneIngestStatus::Downloading.can_transition_to(SceneIngestStatus::Processing));
        assert!(SceneIngestStatus::Processing.can_transition_to(SceneIngestStatus::Stored));
        assert!(SceneIngestStatus::Processing.can_transition_to(SceneIngestStatus::Failed));
        assert!(!SceneIngestStatus::Queued.can_transition_to(SceneIngestStatus::Stored));
        assert!(!SceneIngestStatus::Stored.can_transition_to(SceneIngestStatus::Processing));
    }

    #[tokio::test]
    async fn ingest_landsat_records_freshness_coverage_and_status() -> Result<()> {
        let tmp = TempDir::new()?;
        let source_dir = tmp.path().join("source");
        std::fs::create_dir_all(&source_dir)?;
        write_scene_fixture(&source_dir, &[("B4", "B4.png"), ("B5", "B5.png")])?;

        let config = test_config(&tmp);
        config.ensure_data_dirs()?;
        let pool = db::connect_pool(&config).await?;

        let record = ingest_landsat(
            IngestLandsatArgs {
                scene_id: "scene-fresh".to_string(),
                source_dir: source_dir.clone(),
            },
            &config,
            &pool,
        )
        .await?;

        assert_eq!(record.status, SceneIngestStatus::Stored);
        assert_eq!(record.acquisition_date.as_deref(), Some("2026-05-01"));
        assert_eq!(record.coverage_fraction, Some(1.0));
        assert!(record.ingested_at.is_some());
        assert!(record.status_reason.is_none());
        assert!(config
            .data_root
            .join("scenes")
            .join("scene-fresh")
            .join("B4.png")
            .exists());

        let persisted = load_ingest_record(&pool, "scene-fresh")
            .await?
            .expect("ingest record should persist");
        assert_eq!(persisted, record);

        Ok(())
    }

    #[tokio::test]
    async fn ingest_landsat_failure_records_reason_and_cleans_partial_scene() -> Result<()> {
        let tmp = TempDir::new()?;
        let source_dir = tmp.path().join("source");
        std::fs::create_dir_all(source_dir.join("bad_band"))?;
        write_scene_fixture(&source_dir, &[("B4", "bad_band")])?;

        let config = test_config(&tmp);
        config.ensure_data_dirs()?;
        let pool = db::connect_pool(&config).await?;

        let err = ingest_landsat(
            IngestLandsatArgs {
                scene_id: "scene-failed".to_string(),
                source_dir: source_dir.clone(),
            },
            &config,
            &pool,
        )
        .await
        .expect_err("directory band copy should fail");
        assert!(err.to_string().contains("download_error"));

        let record = load_ingest_record(&pool, "scene-failed")
            .await?
            .expect("failed ingest record should persist");
        assert_eq!(record.status, SceneIngestStatus::Failed);
        assert_eq!(record.status_reason.as_deref(), Some("download_error"));
        assert!(record.ingested_at.is_none());
        assert!(!config
            .data_root
            .join("scenes")
            .join("scene-failed")
            .exists());

        let scene_row: Option<i64> = sqlx::query_scalar("SELECT 1 FROM scenes WHERE scene_id = ?1")
            .bind("scene-failed")
            .fetch_optional(&pool)
            .await?;
        assert!(scene_row.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn ingest_landsat_asserts_and_persists_spatial_ref() -> Result<()> {
        let tmp = TempDir::new()?;
        let source_dir = tmp.path().join("source");
        std::fs::create_dir_all(&source_dir)?;
        write_scene_fixture_with_spatial_ref(
            &source_dir,
            &[("B4", "B4.png"), ("B5", "B5.png")],
            Some(valid_spatial_ref()),
        )?;

        let config = test_config(&tmp);
        config.ensure_data_dirs()?;
        let pool = db::connect_pool(&config).await?;

        ingest_landsat(
            IngestLandsatArgs {
                scene_id: "scene-georef".to_string(),
                source_dir: source_dir.clone(),
            },
            &config,
            &pool,
        )
        .await?;

        let spatial_ref = load_scene_spatial_ref(&pool, "scene-georef")
            .await?
            .expect("asserted spatial ref should persist");
        assert_eq!(spatial_ref.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(
            spatial_ref.resolution,
            Some(RasterResolution { x: 0.05, y: 0.05 })
        );

        let metadata_json: String =
            sqlx::query_scalar("SELECT metadata_json FROM scenes WHERE scene_id = ?1")
                .bind("scene-georef")
                .fetch_one(&pool)
                .await?;
        let image: MultispectralImage = serde_json::from_str(&metadata_json)?;
        assert_eq!(image.metadata.spatial_ref, Some(spatial_ref));

        Ok(())
    }

    #[tokio::test]
    async fn ingest_landsat_rejects_missing_crs_spatial_ref() -> Result<()> {
        let tmp = TempDir::new()?;
        let source_dir = tmp.path().join("source");
        std::fs::create_dir_all(&source_dir)?;
        let mut spatial_ref = valid_spatial_ref();
        spatial_ref.crs = None;
        write_scene_fixture_with_spatial_ref(&source_dir, &[("B4", "B4.png")], Some(spatial_ref))?;

        let config = test_config(&tmp);
        config.ensure_data_dirs()?;
        let pool = db::connect_pool(&config).await?;

        let err = ingest_landsat(
            IngestLandsatArgs {
                scene_id: "scene-bad-georef".to_string(),
                source_dir: source_dir.clone(),
            },
            &config,
            &pool,
        )
        .await
        .expect_err("missing CRS should reject ingest");
        assert!(err.to_string().contains("georeferencing_error"));

        let record = load_ingest_record(&pool, "scene-bad-georef")
            .await?
            .expect("failed ingest record should persist");
        assert_eq!(record.status, SceneIngestStatus::Failed);
        assert_eq!(
            record.status_reason.as_deref(),
            Some("georeferencing_error")
        );
        assert!(load_scene_spatial_ref(&pool, "scene-bad-georef")
            .await?
            .is_none());

        let scene_row: Option<i64> = sqlx::query_scalar("SELECT 1 FROM scenes WHERE scene_id = ?1")
            .bind("scene-bad-georef")
            .fetch_optional(&pool)
            .await?;
        assert!(scene_row.is_none());

        Ok(())
    }

    fn test_config(tmp: &TempDir) -> HubConfig {
        HubConfig {
            bind_address: "127.0.0.1:0".to_string(),
            database_url: format!(
                "sqlite://{}?mode=rwc",
                tmp.path().join("geo_hub_ingest_test.db").display()
            ),
            data_root: tmp.path().join("data"),
            ..HubConfig::default()
        }
    }

    fn write_scene_fixture(source_dir: &Path, bands: &[(&str, &str)]) -> Result<()> {
        write_scene_fixture_with_spatial_ref(source_dir, bands, Some(valid_spatial_ref()))
    }

    fn write_scene_fixture_with_spatial_ref(
        source_dir: &Path,
        bands: &[(&str, &str)],
        spatial_ref: Option<RasterSpatialRef>,
    ) -> Result<()> {
        let mut file_paths = HashMap::new();
        for (band, file_name) in bands {
            if *file_name != "bad_band" {
                std::fs::write(source_dir.join(file_name), b"band-bytes")?;
            }
            file_paths.insert((*band).to_string(), (*file_name).to_string());
        }
        let image = MultispectralImage {
            metadata: ImageMetadata {
                timestamp: chrono::DateTime::parse_from_rfc3339("2026-05-01T12:34:56Z")
                    .expect("timestamp should parse")
                    .with_timezone(&chrono::Utc),
                gps_position: None,
                bands: bands.iter().map(|(band, _)| (*band).to_string()).collect(),
                exposure_time: 1.0,
                gain: 1.0,
                width: 2,
                height: 2,
                spatial_ref,
            },
            file_paths,
            image_id: Uuid::new_v4(),
        };
        std::fs::write(
            source_dir.join("metadata_scene.json"),
            serde_json::to_string_pretty(&image)?,
        )?;
        Ok(())
    }

    fn valid_spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -96.7,
                min_lat: 41.1,
                max_lon: -96.6,
                max_lat: 41.2,
            }),
            geo_transform: Some([-96.7, 0.05, 0.0, 41.2, 0.0, -0.05]),
            resolution: None,
        }
    }
}
