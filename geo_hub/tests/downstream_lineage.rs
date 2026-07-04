//! Downstream lineage closure + source stubs (Track A batch 10).
//!
//! - weather/soil-IoT/equipment feeds register as catalog sources (the unified
//!   source registry), even before their ingest pipelines exist.
//! - a Report traces all the way back to the satellite L0: L0 -> L1 -> L2 -> L3
//!   -> Finding -> Recommendation -> Report, with zero gaps.

use anyhow::Result;
use geo_hub::ingest_contract;
use geo_hub::{catalog, db, provenance_store, HubConfig};
use provenance::{ActorIdentity, ArtifactKind, LineageRecord, ProvenanceParameters};
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};
use sqlx::Row;
use tempfile::TempDir;

const T0: &str = "2026-06-01T00:00:00Z";

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("dl.db").display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    db::connect_pool(&config).await
}

fn product(level: ProductLevel, kind: &str, inputs: Vec<ProductInputRef>) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.compute"),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({ "scene_id": "scene-1" }),
        inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some("scene-1".to_string()),
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

fn downstream(artifact_id: &str, kind: ArtifactKind, input: &str) -> LineageRecord {
    LineageRecord {
        artifact_id: artifact_id.to_string(),
        kind,
        inputs: vec![input.to_string()],
        method: "derive".to_string(),
        parameters: ProvenanceParameters::from_json(serde_json::json!({})),
        operator: "geo_hub:advisor".to_string(),
        actor: ActorIdentity::system("geo_hub:advisor"),
        created_at: T0.to_string(),
    }
}

#[tokio::test]
async fn weather_iot_equipment_register_as_sources() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    ingest_contract::register_source_stub(&pool, "noaa-gfs", "weather", None, None).await?;
    ingest_contract::register_source_stub(
        &pool,
        "soil-net-1",
        "iot",
        None,
        Some("moisture".into()),
    )
    .await?;
    ingest_contract::register_source_stub(
        &pool,
        "tractor-7",
        "equipment",
        Some("john-deere".into()),
        None,
    )
    .await?;

    let kinds: Vec<String> =
        sqlx::query("SELECT source_kind FROM catalog_sources ORDER BY source_kind")
            .fetch_all(&pool)
            .await?
            .iter()
            .map(|r| r.get::<String, _>("source_kind"))
            .collect();
    assert_eq!(kinds, vec!["equipment", "iot", "weather"]);
    Ok(())
}

#[tokio::test]
async fn report_traces_back_to_the_satellite_l0() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    // Product chain L0 -> L1 -> L2 -> L3 (each register writes Product lineage).
    let l0 = product(ProductLevel::L0, "raw_capture", vec![]);
    let l0_id = l0.product_id();
    catalog::register_product(&pool, &l0, T0).await?;
    let l1 = product(
        ProductLevel::L1,
        "band_nir",
        vec![ProductInputRef {
            product_id: l0_id.clone(),
            role: "raw".into(),
        }],
    );
    let l1_id = catalog::register_product(&pool, &l1, T0).await?;
    let l2 = product(
        ProductLevel::L2,
        "ndvi",
        vec![ProductInputRef {
            product_id: l1_id,
            role: "band:nir".into(),
        }],
    );
    let l2_id = catalog::register_product(&pool, &l2, T0).await?;
    let l3 = product(
        ProductLevel::L3,
        "ndvi_trend",
        vec![ProductInputRef {
            product_id: l2_id,
            role: "l2_input".into(),
        }],
    );
    let l3_id = catalog::register_product(&pool, &l3, T0).await?;

    // Downstream advisor artifacts: Finding -> Recommendation -> Report.
    provenance_store::append_lineage(
        &pool,
        &downstream("finding-1", ArtifactKind::Finding, &l3_id),
    )
    .await?;
    provenance_store::append_lineage(
        &pool,
        &downstream("rec-1", ArtifactKind::Recommendation, "finding-1"),
    )
    .await?;
    provenance_store::append_lineage(
        &pool,
        &downstream("report-1", ArtifactKind::Report, "rec-1"),
    )
    .await?;

    let trace = provenance_store::trace_backward(&pool, "report-1").await?;
    assert!(
        trace.gaps.is_empty(),
        "report->L0 gap-free: {:?}",
        trace.gaps
    );
    assert_eq!(trace.records.len(), 7, "report,rec,finding,L3,L2,L1,L0");
    assert!(
        trace
            .records
            .iter()
            .any(|r| r.artifact_id == l0_id && r.inputs.is_empty()),
        "trace reaches the input-less satellite L0"
    );
    assert!(trace.records.iter().any(|r| r.kind == ArtifactKind::Report));
    Ok(())
}
