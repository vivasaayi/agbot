//! Recommendation lineage persisted at create time (Track A phase 10b polish).
//!
//! A recommendation created via the live route appends its own lineage record to
//! the ledger, so a *direct* backward trace of `recommendation:<id>` reaches its
//! evidence — not only the on-demand report-lineage reconstruction.

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, ingest_contract, server, HubConfig};
use provenance::ActorIdentity;
use serde_json::json;
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

const T0: &str = "2026-06-01T00:00:00Z";

async fn ctx(tmp: &TempDir) -> Result<(Router, db::DbPool)> {
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", tmp.path().join("rec.db").display()),
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

fn product(level: ProductLevel, kind: &str, inputs: Vec<ProductInputRef>) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.compute"),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "scene_id": "scene-1" }),
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

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> Result<(StatusCode, serde_json::Value)> {
    let req = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&b)?))?,
        None => req.body(Body::empty())?,
    };
    let response = app.clone().oneshot(req).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into())),
    ))
}

#[tokio::test]
async fn created_recommendation_is_directly_traceable_to_its_evidence() -> Result<()> {
    let tmp = TempDir::new()?;
    let (app, pool) = ctx(&tmp).await?;
    let actor = ActorIdentity::system("geo_hub:ingest");

    // A scene (so the recommendation route accepts it) with an L0->L1->L2 chain;
    // the L2 product is the recommendation's evidence.
    ingest_contract::commit_ingest(
        &pool,
        &ingest_contract::NormalizedIngest {
            source_id: "landsat-9".to_string(),
            source_kind: "satellite".to_string(),
            platform: None,
            sensor: None,
            source_config: None,
            scene: Some(ingest_contract::IngestScene {
                scene_id: "scene-1".to_string(),
                owner: None,
                sensor: "landsat9".to_string(),
                acquired_at: T0.to_string(),
                data_path: "data/scene-1".to_string(),
                metadata_json: "{}".to_string(),
                cloud_cover: None,
            }),
            l0_products: Vec::new(),
            l1_products: Vec::new(),
            quality: None,
        },
        &actor,
        T0,
    )
    .await?;

    let l0 = catalog::register_product(&pool, &product(ProductLevel::L0, "raw_capture", vec![]), T0)
        .await?;
    let l1 = catalog::register_product(
        &pool,
        &product(
            ProductLevel::L1,
            "band_nir",
            vec![ProductInputRef {
                product_id: l0.clone(),
                role: "raw".into(),
            }],
        ),
        T0,
    )
    .await?;
    let l2 = catalog::register_product(
        &pool,
        &product(
            ProductLevel::L2,
            "ndvi",
            vec![ProductInputRef {
                product_id: l1,
                role: "band:nir".into(),
            }],
        ),
        T0,
    )
    .await?;

    // An annotation on the scene (the route requires a recommendation to cite at
    // least one).
    let (status, annotation) = send(
        &app,
        "POST",
        "/api/scenes/scene-1/annotations",
        Some(json!({
            "label": "declining zone",
            "geometry": { "type": "point", "coordinate": { "longitude": -96.4, "latitude": 41.2 } },
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{annotation}");
    let annotation_id = annotation["annotation_id"].as_str().unwrap().to_string();

    // Create a recommendation citing the annotation, with the L2 product as
    // additional evidence.
    let (status, recommendation) = send(
        &app,
        "POST",
        "/api/scenes/scene-1/recommendations",
        Some(json!({
            "title": "Irrigate declining zone",
            "author_user_id": "agronomist-1",
            "annotation_ids": [annotation_id],
            "evidence_refs": [l2],
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{recommendation}");
    let recommendation_id = recommendation["recommendation_id"].as_str().unwrap();

    // Before this change a direct trace of the recommendation found nothing (it
    // was never on the ledger). Now it resolves the recommendation node and
    // follows its L2 evidence down to the L0 — that evidence sub-path is gap-free.
    let artifact = format!("recommendation:{recommendation_id}");
    let (status, trace) = send(
        &app,
        "GET",
        &format!("/api/provenance/trace/{}", artifact.replace(':', "%3A")),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{trace}");
    let records = trace["records"].as_array().expect("records array");
    assert!(
        records.iter().any(|r| r["artifact_id"] == json!(artifact)),
        "the recommendation itself is on the ledger and in the trace"
    );
    assert!(
        records.iter().any(|r| r["artifact_id"] == json!(l2)),
        "recommendation traces to its L2 evidence"
    );
    assert!(
        records.iter().any(|r| r["artifact_id"] == json!(l0)),
        "and through the L2 down to the L0 root"
    );
    Ok(())
}
