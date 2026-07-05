//! Integration tests for the provenance ledger write path and trace API
//! (Track A batch 3): registering a catalog product writes lineage in the same
//! transaction; backward trace walks an L3 product down to its L0 sources with
//! gap detection; the `/api/provenance/trace/:id` route serves the trace.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, provenance_store, server, HubConfig};
use provenance::{ActorIdentity, ArtifactKind, LineageRecord, ProvenanceParameters};
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const T0: &str = "2026-06-01T00:00:00Z";

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let db_path = tmp.path().join("prov_test.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    db::connect_pool(&config).await
}

async fn router(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let db_path = tmp.path().join("prov_route.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    };
    Ok((server::build_router(state), pool))
}

fn draft(
    level: ProductLevel,
    kind: &str,
    scene: Option<&str>,
    inputs: Vec<ProductInputRef>,
) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.compute"),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({ "kind": kind }),
        inputs,
        scope: ProductScope {
            farm_id: Some("farm-1".to_string()),
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: scene.map(|s| s.to_string()),
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

/// Register a full L0 → L1 → L2 → L3 chain and return the L3 product id.
async fn register_chain(pool: &db::DbPool) -> Result<String> {
    let l0 = catalog::register_product(
        pool,
        &draft(ProductLevel::L0, "raw_capture", Some("scene-1"), vec![]),
        T0,
    )
    .await?;
    let l1 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L1,
            "band_nir",
            Some("scene-1"),
            vec![ProductInputRef {
                product_id: l0,
                role: "raw".to_string(),
            }],
        ),
        T0,
    )
    .await?;
    let l2 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L2,
            "ndvi",
            Some("scene-1"),
            vec![ProductInputRef {
                product_id: l1,
                role: "band:nir".to_string(),
            }],
        ),
        T0,
    )
    .await?;
    let l3 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L3,
            "ndvi_trend",
            None,
            vec![ProductInputRef {
                product_id: l2,
                role: "epoch".to_string(),
            }],
        ),
        T0,
    )
    .await?;
    Ok(l3)
}

#[tokio::test]
async fn registering_products_writes_lineage_reaching_l0_with_no_gaps() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let l3 = register_chain(&pool).await?;

    let trace = provenance_store::trace_backward(&pool, &l3).await?;
    assert!(
        trace.gaps.is_empty(),
        "chain should be gap-free: {:?}",
        trace.gaps
    );
    assert_eq!(trace.records.len(), 4, "L3 -> L2 -> L1 -> L0");

    // The trace reaches an L0 raw capture (a product with no further inputs).
    let l0_reached = trace
        .records
        .iter()
        .any(|r| r.kind == ArtifactKind::Product && r.inputs.is_empty());
    assert!(
        l0_reached,
        "backward trace must reach an input-less L0 record"
    );

    // Every registered product carries SystemService lineage.
    assert!(trace
        .records
        .iter()
        .all(|r| r.actor.actor_kind == provenance::ActorKind::SystemService));
    Ok(())
}

#[tokio::test]
async fn backward_trace_reports_gap_for_missing_input() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    // A lineage record whose declared input has no record of its own.
    let record = LineageRecord {
        artifact_id: "derived-1".to_string(),
        kind: ArtifactKind::Product,
        inputs: vec!["missing-source".to_string()],
        method: "derive".to_string(),
        parameters: ProvenanceParameters::from_json(serde_json::json!({})),
        operator: "geo_hub:catalog".to_string(),
        actor: ActorIdentity::system("geo_hub:catalog"),
        created_at: T0.to_string(),
    };
    provenance_store::append_lineage(&pool, &record).await?;

    let trace = provenance_store::trace_backward(&pool, "derived-1").await?;
    assert_eq!(trace.records.len(), 1);
    assert_eq!(trace.gaps.len(), 1);
    assert_eq!(trace.gaps[0].missing_artifact_id, "missing-source");
    assert_eq!(trace.gaps[0].referenced_by.as_deref(), Some("derived-1"));
    Ok(())
}

#[tokio::test]
async fn append_lineage_is_idempotent_on_artifact_id() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let record = LineageRecord {
        artifact_id: "art-1".to_string(),
        kind: ArtifactKind::Product,
        inputs: vec![],
        method: "m".to_string(),
        parameters: ProvenanceParameters::from_json(serde_json::json!({})),
        operator: "op".to_string(),
        actor: ActorIdentity::system("svc"),
        created_at: T0.to_string(),
    };
    provenance_store::append_lineage(&pool, &record).await?;
    provenance_store::append_lineage(&pool, &record).await?; // no duplicate/error

    let all = provenance_store::load_all_lineage(&pool).await?;
    assert_eq!(all.iter().filter(|r| r.artifact_id == "art-1").count(), 1);
    Ok(())
}

#[tokio::test]
async fn provenance_trace_route_serves_the_chain() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = router(&tmp).await?;
    let l3 = register_chain(&pool).await?;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/provenance/trace/{l3}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    let trace: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(trace["target_artifact_id"], serde_json::json!(l3));
    assert_eq!(trace["records"].as_array().unwrap().len(), 4);
    assert!(trace["gaps"].as_array().unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn unknown_artifact_trace_is_not_found() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, _pool) = router(&tmp).await?;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/provenance/trace/does-not-exist")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}
