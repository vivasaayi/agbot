//! Integration tests for the product catalog registry (Track A batch 2).
//!
//! Covers the batch acceptance criteria: mask-first ordering, unknown-input
//! rejection, deterministic dedupe, scope/level/kind filters, bbox intersection
//! filter, input-edge tracing, and supersession.

use anyhow::Result;
use geo_hub::catalog::{self, CatalogError, ProductFilter};
use geo_hub::{db, HubConfig};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::schemas::{GeoBounds, RasterSpatialRef};
use tempfile::TempDir;

const T0: &str = "2026-06-01T00:00:00Z";

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let db_path = tmp.path().join("catalog_test.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    db::connect_pool(&config).await
}

fn spatial_ref(bbox: [f64; 4]) -> RasterSpatialRef {
    RasterSpatialRef {
        georeferenced: true,
        crs: Some("EPSG:4326".to_string()),
        bbox: Some(GeoBounds {
            min_lon: bbox[0],
            min_lat: bbox[1],
            max_lon: bbox[2],
            max_lat: bbox[3],
        }),
        geo_transform: None,
        resolution: None,
    }
}

/// A minimal L2 index draft over `scene`/`field` with the given parameters.
fn index_draft(kind: &str, scene: &str, field: &str, param: i64) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: kind.to_string(),
        algorithm_id: "index.compute".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({ "window": param }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: Some("farm-1".to_string()),
            field_id: Some(field.to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some(scene.to_string()),
            temporal_start: T0.to_string(),
            temporal_end: T0.to_string(),
        },
        spatial_ref: Some(spatial_ref([0.0, 0.0, 1.0, 1.0])),
        gsd_m_per_px: Some(10.0),
        artifact: Some(ProductArtifact {
            path: format!("scenes/{scene}/{kind}.tif"),
            format: "geotiff".to_string(),
            checksum_sha256: Some("abc123".to_string()),
        }),
        quality_mask: None,
        confidence: Some(0.9),
        confidence_method: Some("qa_pixel_fraction".to_string()),
        quality_summary: Some(serde_json::json!({ "cloud_fraction": 0.02 })),
        evidence_digests: Vec::new(),
        source_id: Some("landsat-9".to_string()),
    }
}

#[tokio::test]
async fn register_then_get_round_trips_core_fields() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let draft = index_draft("ndvi", "scene-1", "field-1", 3);
    let expected_id = draft.product_id();

    let id = catalog::register_product(&pool, &draft, T0).await?;
    assert_eq!(id, expected_id);

    let got = catalog::get_product(&pool, &id).await?.expect("registered");
    assert_eq!(got.product_id, expected_id);
    assert_eq!(got.level, ProductLevel::L2);
    assert_eq!(got.kind, "ndvi");
    assert_eq!(got.field_id.as_deref(), Some("field-1"));
    assert_eq!(got.scene_id.as_deref(), Some("scene-1"));
    assert_eq!(got.source_id.as_deref(), Some("landsat-9"));
    assert_eq!(got.confidence, Some(0.9));
    assert_eq!(got.status, "registered");
    assert_eq!(got.parameters_hash, draft.parameters_hash());
    assert_eq!(got.bbox, Some([0.0, 0.0, 1.0, 1.0]));
    Ok(())
}

#[tokio::test]
async fn registering_with_unknown_input_is_rejected() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let mut draft = index_draft("ndvi_delta", "scene-2", "field-1", 1);
    draft.inputs.push(ProductInputRef {
        product_id: "scene-1:ndvi:deadbeef0000".to_string(),
        role: "prior_epoch".to_string(),
    });

    let err = catalog::register_product(&pool, &draft, T0)
        .await
        .expect_err("unknown input must be rejected");
    match err {
        CatalogError::InputNotFound { product_id, role } => {
            assert_eq!(product_id, "scene-1:ndvi:deadbeef0000");
            assert_eq!(role, "prior_epoch");
        }
        other => panic!("expected InputNotFound, got {other:?}"),
    }

    // Nothing should have been persisted.
    assert!(catalog::get_product(&pool, &draft.product_id())
        .await?
        .is_none());
    Ok(())
}

#[tokio::test]
async fn quality_mask_must_be_registered_before_the_product() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    // A product that references a mask which does not exist yet.
    let mut mask = index_draft("qa_mask", "scene-1", "field-1", 0);
    mask.level = ProductLevel::L2;
    let mask_id = mask.product_id();

    let mut ndvi = index_draft("ndvi", "scene-1", "field-1", 3);
    ndvi.quality_mask = Some(ProductInputRef {
        product_id: mask_id.clone(),
        role: "mask".to_string(),
    });

    let err = catalog::register_product(&pool, &ndvi, T0)
        .await
        .expect_err("mask-first ordering must be enforced");
    assert!(matches!(err, CatalogError::MaskNotRegistered { .. }));

    // Register the mask, then the product succeeds and links the mask.
    catalog::register_product(&pool, &mask, T0).await?;
    let ndvi_id = catalog::register_product(&pool, &ndvi, T0).await?;

    let got = catalog::get_product(&pool, &ndvi_id)
        .await?
        .expect("stored");
    assert_eq!(
        got.quality_mask_product_id.as_deref(),
        Some(mask_id.as_str())
    );

    let edges = catalog::trace_inputs(&pool, &ndvi_id).await?;
    assert!(edges
        .iter()
        .any(|e| e.input_product_id == mask_id && e.role == "mask"));
    Ok(())
}

#[tokio::test]
async fn registering_the_same_draft_twice_is_idempotent() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let draft = index_draft("ndvi", "scene-1", "field-1", 3);
    let first = catalog::register_product(&pool, &draft, T0).await?;
    let second = catalog::register_product(&pool, &draft, T0).await?;
    assert_eq!(first, second);

    let all = catalog::list_products(&pool, &ProductFilter::default()).await?;
    assert_eq!(all.len(), 1, "dedupe on (kind, parameters_hash)");
    Ok(())
}

#[tokio::test]
async fn identity_ignores_scope_two_scenes_same_computation_collapse() -> Result<()> {
    // Identity is (kind, parameters_hash) over algorithm + parameters + sorted
    // inputs. Scope (scene/field/season/time) is descriptive metadata, not
    // identity. Two drafts that differ only in scene_id but share the same
    // computation therefore resolve to the same product. In the real pipeline
    // distinct scenes carry distinct L0/L1 input products, which makes the
    // hashes differ; this test pins the underlying rule.
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let a = index_draft("ndvi", "scene-1", "field-1", 7);
    let mut b = index_draft("ndvi", "scene-2", "field-2", 7);
    b.scope.farm_id = Some("farm-9".to_string());

    let id_a = catalog::register_product(&pool, &a, T0).await?;
    let id_b = catalog::register_product(&pool, &b, T0).await?;
    assert_eq!(id_a, id_b);
    assert_eq!(
        catalog::list_products(&pool, &ProductFilter::default())
            .await?
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn trace_inputs_returns_registered_edges() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let nir = index_draft("band_nir", "scene-1", "field-1", 0);
    let red = index_draft("band_red", "scene-1", "field-1", 0);
    let nir_id = catalog::register_product(&pool, &nir, T0).await?;
    let red_id = catalog::register_product(&pool, &red, T0).await?;

    let mut ndvi = index_draft("ndvi", "scene-1", "field-1", 3);
    ndvi.inputs = vec![
        ProductInputRef {
            product_id: nir_id.clone(),
            role: "band:nir".to_string(),
        },
        ProductInputRef {
            product_id: red_id.clone(),
            role: "band:red".to_string(),
        },
    ];
    let ndvi_id = catalog::register_product(&pool, &ndvi, T0).await?;

    let mut edges = catalog::trace_inputs(&pool, &ndvi_id).await?;
    edges.sort_by(|a, b| a.role.cmp(&b.role));
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].role, "band:nir");
    assert_eq!(edges[0].input_product_id, nir_id);
    assert_eq!(edges[1].role, "band:red");
    assert_eq!(edges[1].input_product_id, red_id);
    Ok(())
}

#[tokio::test]
async fn list_products_filters_by_scope_level_and_kind() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    // The two ndvi products are distinct computations (different `window`
    // parameter), so they have distinct identities; the field-2 one uses
    // param 2 so it does not dedupe against the field-1 one.
    catalog::register_product(&pool, &index_draft("ndvi", "scene-1", "field-1", 1), T0).await?;
    catalog::register_product(&pool, &index_draft("evi", "scene-1", "field-1", 1), T0).await?;
    catalog::register_product(&pool, &index_draft("ndvi", "scene-9", "field-2", 2), T0).await?;

    let by_field = catalog::list_products(
        &pool,
        &ProductFilter {
            field_id: Some("field-1".to_string()),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(by_field.len(), 2);

    let by_kind = catalog::list_products(
        &pool,
        &ProductFilter {
            kind: Some("ndvi".to_string()),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(by_kind.len(), 2);

    let by_level = catalog::list_products(
        &pool,
        &ProductFilter {
            level: Some(ProductLevel::L2),
            kind: Some("evi".to_string()),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(by_level.len(), 1);
    Ok(())
}

#[tokio::test]
async fn list_products_filters_by_bbox_intersection() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let mut west = index_draft("ndvi", "scene-w", "field-1", 1);
    west.spatial_ref = Some(spatial_ref([0.0, 0.0, 1.0, 1.0]));
    let mut east = index_draft("ndvi", "scene-e", "field-1", 2);
    east.spatial_ref = Some(spatial_ref([10.0, 10.0, 11.0, 11.0]));
    catalog::register_product(&pool, &west, T0).await?;
    catalog::register_product(&pool, &east, T0).await?;

    let hits = catalog::list_products(
        &pool,
        &ProductFilter {
            bbox: Some([0.5, 0.5, 5.0, 5.0]),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].scene_id.as_deref(), Some("scene-w"));
    Ok(())
}

#[tokio::test]
async fn supersede_marks_old_product_and_links_successor() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let old_id =
        catalog::register_product(&pool, &index_draft("ndvi", "scene-1", "field-1", 1), T0).await?;
    let new_id =
        catalog::register_product(&pool, &index_draft("ndvi", "scene-1", "field-1", 2), T0).await?;

    catalog::supersede_product(&pool, &old_id, &new_id).await?;

    let old = catalog::get_product(&pool, &old_id).await?.expect("old");
    assert_eq!(old.status, "superseded");
    assert_eq!(old.superseded_by.as_deref(), Some(new_id.as_str()));
    Ok(())
}

#[tokio::test]
async fn quarantined_product_is_excluded_from_registered_queries() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;

    let id =
        catalog::register_product(&pool, &index_draft("ndvi", "scene-1", "field-1", 1), T0).await?;

    let registered_filter = || ProductFilter {
        status: Some("registered".to_string()),
        ..ProductFilter::default()
    };

    // Present as registered before the gate acts.
    let before = catalog::list_products(&pool, &registered_filter()).await?;
    assert!(before.iter().any(|p| p.product_id == id));

    catalog::quarantine_product(&pool, &id).await?;
    let got = catalog::get_product(&pool, &id).await?.expect("product");
    assert_eq!(got.status, "failed_qa");

    // Now excluded from `registered` queries (serving + downstream fan-out).
    let after = catalog::list_products(&pool, &registered_filter()).await?;
    assert!(
        !after.iter().any(|p| p.product_id == id),
        "quarantined product must not appear as registered"
    );
    Ok(())
}
