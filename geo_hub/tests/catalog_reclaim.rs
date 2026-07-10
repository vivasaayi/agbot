//! Disk reclaim on supersession (batch 3.2b): a superseded product's artifact
//! is deleted when it lives under `data_root` and no other product references
//! it, and is left untouched otherwise (outside the data tree, or shared).

use anyhow::Result;
use geo_hub::catalog::{self, reclaim_product_artifact};
use geo_hub::{db, HubConfig};
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const T0: &str = "2026-06-01T00:00:00Z";

struct Ctx {
    pool: db::DbPool,
    data_root: PathBuf,
}

async fn ctx(tmp: &TempDir) -> Result<Ctx> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("catalog.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    Ok(Ctx {
        pool,
        data_root: config.data_root,
    })
}

/// A minimal L2 product draft pointing at `artifact_path`. `param` varies the
/// identity so two drafts can share a path but be distinct products.
fn draft_at(artifact_path: &Path, param: i64) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "index.compute".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({ "window": param }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: None,
            scene_id: Some("scene-1".to_string()),
            temporal_start: T0.to_string(),
            temporal_end: T0.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
        artifact: Some(ProductArtifact {
            path: artifact_path.to_string_lossy().to_string(),
            format: "geotiff".to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: None,
    }
}

async fn register(ctx: &Ctx, draft: &ProductRecordDraft) -> Result<String> {
    Ok(catalog::register_product(&ctx.pool, draft, T0).await?)
}

#[tokio::test]
async fn reclaim_deletes_a_superseded_artifact_under_data_root() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    let artifact = ctx.data_root.join("scenes/scene-1/ndvi.tif");
    std::fs::create_dir_all(artifact.parent().unwrap())?;
    std::fs::write(&artifact, b"raster bytes")?;
    let id = register(&ctx, &draft_at(&artifact, 1)).await?;

    let removed = reclaim_product_artifact(&ctx.pool, &ctx.data_root, &id).await?;
    assert_eq!(removed.as_deref(), Some(artifact.as_path()));
    assert!(!artifact.exists(), "artifact must be deleted");

    // A second reclaim is a no-op (file already gone).
    assert_eq!(
        reclaim_product_artifact(&ctx.pool, &ctx.data_root, &id).await?,
        None
    );
    Ok(())
}

#[tokio::test]
async fn reclaim_leaves_a_file_outside_data_root_untouched() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    // Artifact lives outside data_root (directly under tmp, a sibling of data/).
    let artifact = tmp.path().join("outside.tif");
    std::fs::write(&artifact, b"raster bytes")?;
    let id = register(&ctx, &draft_at(&artifact, 1)).await?;

    let removed = reclaim_product_artifact(&ctx.pool, &ctx.data_root, &id).await?;
    assert_eq!(removed, None, "must not delete outside the data tree");
    assert!(artifact.exists(), "outside-root file must survive");
    Ok(())
}

#[tokio::test]
async fn reclaim_leaves_a_shared_artifact_untouched() -> Result<()> {
    let tmp = TempDir::new()?;
    let ctx = ctx(&tmp).await?;

    let artifact = ctx.data_root.join("scenes/scene-1/shared.tif");
    std::fs::create_dir_all(artifact.parent().unwrap())?;
    std::fs::write(&artifact, b"raster bytes")?;

    // Two distinct products point at the same file.
    let id_a = register(&ctx, &draft_at(&artifact, 1)).await?;
    let id_b = register(&ctx, &draft_at(&artifact, 2)).await?;
    assert_ne!(id_a, id_b);

    let removed = reclaim_product_artifact(&ctx.pool, &ctx.data_root, &id_a).await?;
    assert_eq!(removed, None, "shared artifact must not be deleted");
    assert!(
        artifact.exists(),
        "shared file must survive while B references it"
    );
    Ok(())
}
