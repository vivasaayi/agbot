//! End-to-end L0 -> L3 lineage acceptance (Track A batch 9).
//!
//! Register a satellite chain L0 (raw) -> L1 (band) -> L2 (ndvi) via the ingest
//! contract + catalog, then an L3 aggregate (ndvi_trend) whose inputs are the L2
//! product, and prove `trace_backward` from the L3 reaches the L0 with no gaps.

use anyhow::Result;
use geo_hub::catalog;
use geo_hub::ingest_contract::{self, IngestScene, NormalizedIngest};
use geo_hub::{db, provenance_store, HubConfig};
use provenance::{ActorIdentity, ArtifactKind};
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};
use tempfile::TempDir;

const T0: &str = "2026-06-01T00:00:00Z";

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("l3.db").display()),
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
    inputs: Vec<ProductInputRef>,
) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.compute"),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({ "scene_id": scene }),
        inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some(scene.to_string()),
            temporal_start: T0.to_string(),
            temporal_end: T0.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some("landsat-9".to_string()),
    }
}

#[tokio::test]
async fn l3_trend_traces_back_to_the_satellite_l0() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");

    // L0 raw + L1 band via the ingest contract.
    let l0 = draft(ProductLevel::L0, "raw_capture", "scene-1", vec![]);
    let l0_id = l0.product_id();
    let l1 = draft(
        ProductLevel::L1,
        "band_nir",
        "scene-1",
        vec![ProductInputRef {
            product_id: l0_id.clone(),
            role: "raw".to_string(),
        }],
    );
    let l1_id = l1.product_id();
    ingest_contract::commit_ingest(
        &pool,
        &NormalizedIngest {
            source_id: "landsat-9".to_string(),
            source_kind: "satellite".to_string(),
            platform: None,
            sensor: None,
            source_config: None,
            scene: Some(IngestScene {
                scene_id: "scene-1".to_string(),
                owner: None,
                sensor: "landsat9".to_string(),
                acquired_at: T0.to_string(),
                data_path: "data/s1".to_string(),
                metadata_json: "{}".to_string(),
                cloud_cover: Some(5.0),
            }),
            l0_products: vec![l0],
            l1_products: vec![l1],
            quality: None,
        },
        &actor,
        T0,
    )
    .await?;

    // L2 ndvi consuming the L1 band.
    let l2 = draft(
        ProductLevel::L2,
        "ndvi",
        "scene-1",
        vec![ProductInputRef {
            product_id: l1_id,
            role: "band:nir".to_string(),
        }],
    );
    let l2_id = catalog::register_product(&pool, &l2, T0).await?;

    // L3 ndvi_trend consuming the L2 ndvi (the post_processor productization
    // shape: inputs are L2 catalog ids).
    let l3 = draft(
        ProductLevel::L3,
        "ndvi_trend",
        "scene-1",
        vec![ProductInputRef {
            product_id: l2_id,
            role: "l2_input".to_string(),
        }],
    );
    let l3_id = catalog::register_product(&pool, &l3, T0).await?;

    // Backward trace from L3 reaches the L0 with no gaps.
    let trace = provenance_store::trace_backward(&pool, &l3_id).await?;
    assert!(
        trace.gaps.is_empty(),
        "L3->L0 chain gap-free: {:?}",
        trace.gaps
    );
    assert_eq!(trace.records.len(), 4, "L3 -> L2 -> L1 -> L0");
    assert!(
        trace
            .records
            .iter()
            .any(|r| r.kind == ArtifactKind::Product && r.inputs.is_empty()),
        "trace reaches the input-less L0 capture"
    );
    // The L0 root is the raw satellite capture.
    assert!(trace.records.iter().any(|r| r.artifact_id == l0_id));
    Ok(())
}
