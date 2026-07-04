//! Normalized source-ingestion contract (Track A batch 5).
//!
//! `commit_ingest` is the single path into the catalog for source data: it
//! registers the source (`catalog_sources`), upserts the scene (`scenes`), and
//! registers the L0/L1 catalog products (with lineage, transactionally, via the
//! catalog registry). Producers construct a [`NormalizedIngest`]; geo_hub owns
//! the persistence. Satellite (landsat/Sentinel) and drone ingestion both
//! normalize into this shape.

use crate::catalog::{self, CatalogError};
use crate::db::DbPool;
use provenance::ActorIdentity;
use serde::{Deserialize, Serialize};
use shared::drone_ingest::{DroneCapture, DroneIngestManifest, DroneIngestManifestError};
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::path::Path;
use thiserror::Error;

/// Scene metadata upserted into the `scenes` table as part of an ingest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestScene {
    pub scene_id: String,
    #[serde(default)]
    pub owner: Option<String>,
    pub sensor: String,
    pub acquired_at: String,
    pub data_path: String,
    pub metadata_json: String,
    #[serde(default)]
    pub cloud_cover: Option<f64>,
}

/// A normalized ingest: one source, an optional scene, and the L0/L1 products
/// the source produced. L1 products list their L0 inputs by `product_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedIngest {
    pub source_id: String,
    /// One of `satellite|drone|field_survey|iot|weather|equipment`.
    pub source_kind: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sensor: Option<String>,
    #[serde(default)]
    pub source_config: Option<serde_json::Value>,
    #[serde(default)]
    pub scene: Option<IngestScene>,
    #[serde(default)]
    pub l0_products: Vec<ProductRecordDraft>,
    #[serde(default)]
    pub l1_products: Vec<ProductRecordDraft>,
    #[serde(default)]
    pub quality: Option<serde_json::Value>,
}

/// Outcome of [`commit_ingest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestReceipt {
    pub source_id: String,
    pub scene_id: Option<String>,
    pub product_ids: Vec<String>,
}

#[derive(Debug, Error)]
pub enum IngestError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("failed to serialize {what}: {source}")]
    Serialize {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error("source_kind must be satellite|drone|field_survey|iot|weather|equipment, got {0}")]
    InvalidSourceKind(String),
    #[error("invalid drone manifest: {0}")]
    InvalidManifest(#[from] DroneIngestManifestError),
    #[error("drone session {0} already ingested")]
    DuplicateSession(String),
}

const VALID_SOURCE_KINDS: [&str; 6] = [
    "satellite",
    "drone",
    "field_survey",
    "iot",
    "weather",
    "equipment",
];

/// Register (or update) the source in `catalog_sources`. Idempotent on
/// `source_id`.
pub async fn register_source(pool: &DbPool, ingest: &NormalizedIngest) -> Result<(), IngestError> {
    if !VALID_SOURCE_KINDS.contains(&ingest.source_kind.as_str()) {
        return Err(IngestError::InvalidSourceKind(ingest.source_kind.clone()));
    }
    let config_json = match &ingest.source_config {
        Some(value) => {
            Some(
                serde_json::to_string(value).map_err(|source| IngestError::Serialize {
                    what: "source_config",
                    source,
                })?,
            )
        }
        None => None,
    };
    let registered_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query(
        r#"
        INSERT INTO catalog_sources
            (source_id, source_kind, platform, sensor, config_json, status, registered_at)
        VALUES (?, ?, ?, ?, ?, 'active', ?)
        ON CONFLICT(source_id) DO UPDATE SET
            source_kind = excluded.source_kind,
            platform = excluded.platform,
            sensor = excluded.sensor,
            config_json = excluded.config_json
        "#,
    )
    .bind(&ingest.source_id)
    .bind(&ingest.source_kind)
    .bind(&ingest.platform)
    .bind(&ingest.sensor)
    .bind(config_json)
    .bind(registered_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert the scene into the `scenes` table (the durable scene record read by
/// the legacy scene/tile routes).
async fn upsert_scene(pool: &DbPool, scene: &IngestScene) -> Result<(), IngestError> {
    let owner = scene
        .owner
        .clone()
        .unwrap_or_else(|| "unassigned".to_string());
    sqlx::query(
        r#"
        INSERT INTO scenes
            (scene_id, owner, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(scene_id) DO UPDATE SET
            owner = excluded.owner,
            sensor = excluded.sensor,
            acquired_at = excluded.acquired_at,
            data_path = excluded.data_path,
            metadata_json = excluded.metadata_json,
            cloud_cover = excluded.cloud_cover
        "#,
    )
    .bind(&scene.scene_id)
    .bind(owner)
    .bind(&scene.sensor)
    .bind(&scene.acquired_at)
    .bind(&scene.data_path)
    .bind(&scene.metadata_json)
    .bind(scene.cloud_cover)
    .execute(pool)
    .await?;
    Ok(())
}

/// The single normalized path into the catalog for source data. Registers the
/// source, upserts the scene, then registers every L0 product followed by every
/// L1 product (L0 first so L1 inputs resolve). Each product registration writes
/// lineage transactionally under `actor`. Idempotent given identical inputs.
pub async fn commit_ingest(
    pool: &DbPool,
    ingest: &NormalizedIngest,
    actor: &ActorIdentity,
    created_at: &str,
) -> Result<IngestReceipt, IngestError> {
    register_source(pool, ingest).await?;
    if let Some(scene) = &ingest.scene {
        upsert_scene(pool, scene).await?;
    }

    let mut product_ids = Vec::with_capacity(ingest.l0_products.len() + ingest.l1_products.len());
    for draft in ingest.l0_products.iter().chain(ingest.l1_products.iter()) {
        let product_id =
            catalog::register_product_with_actor(pool, draft, actor, created_at).await?;
        product_ids.push(product_id);
    }

    Ok(IngestReceipt {
        source_id: ingest.source_id.clone(),
        scene_id: ingest.scene.as_ref().map(|scene| scene.scene_id.clone()),
        product_ids,
    })
}

// --- Drone-session ingest (Track A batch 6) --------------------------------

fn artifact_format(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Map one drone capture into an L0 catalog draft. Drone captures carry no
/// input graph, so `session_id` + `capture_id` + `checksum` go into the
/// parameters to give each capture a distinct identity under
/// `UNIQUE(kind, parameters_hash)`.
fn capture_draft(manifest: &DroneIngestManifest, capture: &DroneCapture) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L0,
        kind: capture.kind.clone(),
        algorithm_id: "drone.capture".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "session_id": manifest.session_id,
            "capture_id": capture.capture_id,
            "checksum": capture.checksum_sha256,
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(manifest.scene.scene_id.clone()),
            temporal_start: capture.captured_at.clone(),
            temporal_end: capture.captured_at.clone(),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: Some(ProductArtifact {
            format: artifact_format(&capture.file_path),
            path: capture.file_path.clone(),
            checksum_sha256: Some(capture.checksum_sha256.clone()),
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(manifest.source_id.clone()),
    }
}

async fn product_exists(pool: &DbPool, product_id: &str) -> Result<bool, IngestError> {
    let row = sqlx::query("SELECT 1 FROM catalog_products WHERE product_id = ? LIMIT 1")
        .bind(product_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// Commit a drone-session manifest into the catalog: validate integrity
/// (checksums present), reject a session whose captures are already registered
/// (duplicate), then register the scene + one L0 product per capture via
/// [`commit_ingest`]. Calibration/derived products stay in imagery_processor.
pub async fn commit_drone_ingest(
    pool: &DbPool,
    manifest: &DroneIngestManifest,
    actor: &ActorIdentity,
    created_at: &str,
) -> Result<IngestReceipt, IngestError> {
    manifest.validate()?;

    let l0_products: Vec<ProductRecordDraft> = manifest
        .captures
        .iter()
        .map(|capture| capture_draft(manifest, capture))
        .collect();

    // Duplicate rejection: if any capture product is already registered, this
    // session was ingested before. (commit_ingest itself would silently dedupe;
    // here we surface it as an explicit error.)
    for draft in &l0_products {
        if product_exists(pool, &draft.product_id()).await? {
            return Err(IngestError::DuplicateSession(manifest.session_id.clone()));
        }
    }

    let ingest = NormalizedIngest {
        source_id: manifest.source_id.clone(),
        source_kind: "drone".to_string(),
        platform: manifest.platform.clone(),
        sensor: manifest.sensor.clone(),
        source_config: None,
        scene: Some(IngestScene {
            scene_id: manifest.scene.scene_id.clone(),
            owner: None,
            sensor: manifest.scene.sensor.clone(),
            acquired_at: manifest.scene.acquired_at.clone(),
            data_path: manifest.scene.data_path.clone(),
            metadata_json: manifest.scene.metadata_json.clone(),
            cloud_cover: None,
        }),
        l0_products,
        l1_products: Vec::new(),
        quality: manifest.quality.clone(),
    };
    commit_ingest(pool, &ingest, actor, created_at).await
}
