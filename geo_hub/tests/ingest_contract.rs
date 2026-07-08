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

/// A verified USGS scene commits through the same contract: source + scene + an
/// L0 raw-scene product, with lineage on the ledger (Track A phase 5b).
#[tokio::test]
async fn commit_usgs_landsat_ingest_registers_scene_and_l0_with_lineage() -> Result<()> {
    use geo_hub::landsat::{
        commit_usgs_landsat_ingest, UsgsIngestVerificationRecord, UsgsSceneSummary,
    };

    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");

    let record = UsgsIngestVerificationRecord {
        scene: UsgsSceneSummary {
            scene_id: "LC80420342026152".to_string(),
            display_id: Some("LC08_L2SP_042034".to_string()),
            dataset_name: "landsat_ot_c2_l2".to_string(),
            provider: "USGS".to_string(),
            acquired_at: Some(T0.to_string()),
            cloud_cover: Some(9.0),
            bbox: None,
            browse_url: Some("https://example.test/browse.png".to_string()),
        },
        metadata_path: tmp.path().join("metadata.json"),
        downloaded_browse_path: tmp.path().join("browse.png"),
        stored_at: T0.to_string(),
    };

    let receipt = commit_usgs_landsat_ingest(&pool, &record, &[], &actor, T0).await?;
    assert_eq!(receipt.source_id, "usgs:landsat_ot_c2_l2");
    assert_eq!(receipt.scene_id.as_deref(), Some("LC80420342026152"));
    assert_eq!(receipt.product_ids.len(), 1, "one L0 raw-scene product");

    // Source registered as satellite.
    let source_kind: String =
        sqlx::query("SELECT source_kind FROM catalog_sources WHERE source_id = ?")
            .bind("usgs:landsat_ot_c2_l2")
            .fetch_one(&pool)
            .await?
            .get("source_kind");
    assert_eq!(source_kind, "satellite");

    // The L0 product carries lineage on the ledger (registered via commit_ingest).
    let l0_id = &receipt.product_ids[0];
    let lineage = provenance_store::get_lineage(&pool, l0_id).await?;
    assert!(lineage.is_some(), "L0 raw-scene has a lineage record");

    // Re-ingesting the same scene is idempotent (one product, not two).
    let again = commit_usgs_landsat_ingest(&pool, &record, &[], &actor, T0).await?;
    assert_eq!(again.product_ids, receipt.product_ids);
    let raw = catalog::list_products(
        &pool,
        &ProductFilter {
            kind: Some("raw_scene".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(raw.len(), 1, "idempotent: still one raw-scene product");
    Ok(())
}

/// A full-band USGS pull registers the L0 raw-scene plus one L1 band per
/// downloaded file, and each L1 traces back to the L0 (Track A phase 5b polish).
#[tokio::test]
async fn commit_usgs_full_band_ingest_registers_l0_and_l1_with_trace() -> Result<()> {
    use geo_hub::landsat::{
        commit_usgs_landsat_ingest, UsgsBandFile, UsgsIngestVerificationRecord, UsgsSceneSummary,
    };

    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");

    let record = UsgsIngestVerificationRecord {
        scene: UsgsSceneSummary {
            scene_id: "LC80420342026152".to_string(),
            display_id: None,
            dataset_name: "landsat_ot_c2_l2".to_string(),
            provider: "USGS".to_string(),
            acquired_at: Some(T0.to_string()),
            cloud_cover: Some(4.0),
            bbox: None,
            browse_url: None,
        },
        metadata_path: tmp.path().join("metadata.json"),
        downloaded_browse_path: tmp.path().join("browse.png"),
        stored_at: T0.to_string(),
    };
    let bands = vec![
        UsgsBandFile {
            band: "nir".to_string(),
            path: tmp.path().join("B05.tif"),
            checksum_sha256: Some("nir-hash".to_string()),
        },
        UsgsBandFile {
            band: "red".to_string(),
            path: tmp.path().join("B04.tif"),
            checksum_sha256: Some("red-hash".to_string()),
        },
    ];

    let receipt = commit_usgs_landsat_ingest(&pool, &record, &bands, &actor, T0).await?;
    assert_eq!(receipt.product_ids.len(), 3, "1 L0 + 2 L1 bands");

    // One L0 and two L1 band products.
    let products = catalog::list_products(&pool, &ProductFilter::default()).await?;
    let l0: Vec<_> = products
        .iter()
        .filter(|p| p.level == ProductLevel::L0)
        .collect();
    let l1: Vec<_> = products
        .iter()
        .filter(|p| p.level == ProductLevel::L1)
        .collect();
    assert_eq!(l0.len(), 1);
    assert_eq!(l1.len(), 2);

    // Each L1 band traces back to the L0 raw-scene, gap-free.
    let l0_id = &l0[0].product_id;
    for band in &l1 {
        let trace = provenance_store::trace_backward(&pool, &band.product_id).await?;
        assert!(
            trace.gaps.is_empty(),
            "L1 {} -> L0 gap-free",
            band.product_id
        );
        assert!(
            trace.records.iter().any(|r| &r.artifact_id == l0_id),
            "L1 band lists the L0 raw-scene as input"
        );
    }
    Ok(())
}
