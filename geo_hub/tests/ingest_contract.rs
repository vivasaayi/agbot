//! Integration tests for the normalized source-ingestion contract
//! (Track A batch 5): `commit_ingest` is the single path into the catalog for
//! source data. It registers the source, upserts the scene, registers L0/L1
//! catalog products with lineage, and its L1 products trace back to their L0
//! roots.

use anyhow::Result;
use geo_hub::catalog::{self, ProductFilter};
use geo_hub::ingest_contract::{self, IngestScene, NormalizedIngest};
use geo_hub::{db, provenance_store, HubConfig};
use provenance::ActorIdentity;
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};
use sqlx::Row;
use tempfile::TempDir;

const T0: &str = "2026-06-01T00:00:00Z";

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let db_path = tmp.path().join("ingest_test.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    db::connect_pool(&config).await
}

fn draft(
    level: ProductLevel,
    kind: &str,
    scene: &str,
    source_id: &str,
    inputs: Vec<ProductInputRef>,
) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.ingest"),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({ "scene_id": scene }),
        inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(scene.to_string()),
            temporal_start: T0.to_string(),
            temporal_end: T0.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(30.0),
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(source_id.to_string()),
    }
}

/// A satellite ingest: source + scene + one L0 raw capture + one L1 surface
/// reflectance product listing the L0 as its input.
fn satellite_ingest(source_id: &str, scene: &str, cloud_cover: f64) -> NormalizedIngest {
    let l0 = draft(ProductLevel::L0, "raw_capture", scene, source_id, vec![]);
    let l0_id = l0.product_id();
    let l1 = draft(
        ProductLevel::L1,
        "surface_reflectance",
        scene,
        source_id,
        vec![ProductInputRef {
            product_id: l0_id,
            role: "raw".to_string(),
        }],
    );
    NormalizedIngest {
        source_id: source_id.to_string(),
        source_kind: "satellite".to_string(),
        platform: Some("Landsat 9".to_string()),
        sensor: Some("OLI/TIRS".to_string()),
        source_config: None,
        scene: Some(IngestScene {
            scene_id: scene.to_string(),
            owner: None,
            sensor: "landsat9".to_string(),
            acquired_at: T0.to_string(),
            data_path: format!("data/scenes/{scene}"),
            metadata_json: "{}".to_string(),
            cloud_cover: Some(cloud_cover),
        }),
        l0_products: vec![l0],
        l1_products: vec![l1],
        quality: Some(serde_json::json!({ "cloud_cover": cloud_cover })),
    }
}

#[tokio::test]
async fn commit_ingest_registers_source_scene_and_products() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");

    let receipt = ingest_contract::commit_ingest(
        &pool,
        &satellite_ingest("landsat-9", "scene-1", 12.0),
        &actor,
        T0,
    )
    .await?;
    assert_eq!(receipt.source_id, "landsat-9");
    assert_eq!(receipt.scene_id.as_deref(), Some("scene-1"));
    assert_eq!(receipt.product_ids.len(), 2, "L0 + L1 registered");

    // Source is registered.
    let source_kind: String =
        sqlx::query("SELECT source_kind FROM catalog_sources WHERE source_id = ?")
            .bind("landsat-9")
            .fetch_one(&pool)
            .await?
            .get("source_kind");
    assert_eq!(source_kind, "satellite");

    // Scene is upserted with its cloud cover.
    let cloud: Option<f64> = sqlx::query("SELECT cloud_cover FROM scenes WHERE scene_id = ?")
        .bind("scene-1")
        .fetch_one(&pool)
        .await?
        .get("cloud_cover");
    assert_eq!(cloud, Some(12.0));

    // Two catalog products, one L0 and one L1.
    let products = catalog::list_products(&pool, &ProductFilter::default()).await?;
    assert_eq!(products.len(), 2);
    assert!(products.iter().any(|p| p.level == ProductLevel::L0));
    assert!(products.iter().any(|p| p.level == ProductLevel::L1));
    Ok(())
}

#[tokio::test]
async fn l1_product_traces_back_to_its_l0_root() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");

    ingest_contract::commit_ingest(
        &pool,
        &satellite_ingest("landsat-9", "scene-1", 5.0),
        &actor,
        T0,
    )
    .await?;

    let l1 = catalog::list_products(
        &pool,
        &ProductFilter {
            level: Some(ProductLevel::L1),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(l1.len(), 1);
    let trace = provenance_store::trace_backward(&pool, &l1[0].product_id).await?;
    assert!(
        trace.gaps.is_empty(),
        "L1 -> L0 chain is gap-free: {:?}",
        trace.gaps
    );
    assert_eq!(trace.records.len(), 2, "L1 and its L0 input");
    assert!(
        trace.records.iter().any(|r| r.inputs.is_empty()),
        "trace reaches an input-less L0 capture"
    );
    Ok(())
}

#[tokio::test]
async fn commit_ingest_is_idempotent() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");
    let ingest = satellite_ingest("landsat-9", "scene-1", 8.0);

    ingest_contract::commit_ingest(&pool, &ingest, &actor, T0).await?;
    ingest_contract::commit_ingest(&pool, &ingest, &actor, T0).await?;

    let products = catalog::list_products(&pool, &ProductFilter::default()).await?;
    assert_eq!(
        products.len(),
        2,
        "re-committing the same ingest adds no duplicates"
    );
    let source_count: i64 = sqlx::query("SELECT COUNT(*) AS n FROM catalog_sources")
        .fetch_one(&pool)
        .await?
        .get("n");
    assert_eq!(source_count, 1, "source registered once");
    Ok(())
}

#[tokio::test]
async fn two_scenes_from_one_source_yield_distinct_products() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");

    ingest_contract::commit_ingest(
        &pool,
        &satellite_ingest("landsat-9", "scene-a", 3.0),
        &actor,
        T0,
    )
    .await?;
    ingest_contract::commit_ingest(
        &pool,
        &satellite_ingest("landsat-9", "scene-b", 7.0),
        &actor,
        T0,
    )
    .await?;

    let l1 = catalog::list_products(
        &pool,
        &ProductFilter {
            kind: Some("surface_reflectance".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(l1.len(), 2, "two scenes -> two distinct L1 products");
    Ok(())
}
