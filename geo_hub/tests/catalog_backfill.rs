//! Integration tests for the legacy `products` -> `catalog_products` bridge
//! (Track A batch 4): backfill and publish-time dual-write.
//!
//! Acceptance criteria:
//! - `backfill_products_to_catalog` registers every legacy `products` row as a
//!   catalog product and is idempotent (re-running adds nothing, no dupes).
//! - Two scenes with the same product kind stay distinct catalog products (the
//!   identity invariant: scene_id folded into parameters, since legacy rows
//!   carry no input graph).
//! - The legacy `products` rows are left intact (tile/serving routes unaffected).

use anyhow::Result;
use geo_hub::catalog::{self, ProductFilter};
use geo_hub::{db, product_catalog, HubConfig};
use shared::product_graph::ProductLevel;
use sqlx::Row;
use tempfile::TempDir;

const T0: &str = "2026-06-01T00:00:00Z";

async fn pool(tmp: &TempDir) -> Result<db::DbPool> {
    let db_path = tmp.path().join("backfill_test.db");
    let config = HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    db::connect_pool(&config).await
}

/// Insert a minimal scene + a legacy `products` row directly (simulating data
/// that predates the catalog), so the backfill has something to migrate.
async fn insert_legacy_product(
    pool: &db::DbPool,
    scene_id: &str,
    kind: &str,
    path: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO scenes
            (scene_id, sensor, acquired_at, data_path, metadata_json, cloud_cover, created_at)
        VALUES (?, 'sentinel-2', ?, ?, '{}', 0.0, ?)
        "#,
    )
    .bind(scene_id)
    .bind(T0)
    .bind(format!("data/scenes/{scene_id}"))
    .bind(T0)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO products
            (product_id, scene_id, field_id, season_id, kind, path, gsd_m_per_px, created_at)
        VALUES (?, ?, 'field-1', '2026', ?, ?, 10.0, ?)
        "#,
    )
    .bind(format!("{scene_id}:{kind}"))
    .bind(scene_id)
    .bind(kind)
    .bind(path)
    .bind(T0)
    .execute(pool)
    .await?;
    Ok(())
}

async fn count_products(pool: &db::DbPool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM products")
        .fetch_one(pool)
        .await?;
    Ok(row.get::<i64, _>("n"))
}

#[tokio::test]
async fn backfill_registers_every_legacy_product() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    insert_legacy_product(&pool, "scene-a", "ndvi", "data/a/ndvi.tif").await?;
    insert_legacy_product(&pool, "scene-a", "ndwi", "data/a/ndwi.tif").await?;
    insert_legacy_product(&pool, "scene-b", "ndvi", "data/b/ndvi.tif").await?;

    let migrated = product_catalog::backfill_products_to_catalog(&pool).await?;
    assert_eq!(migrated, 3, "all three legacy rows migrate");

    let cataloged = catalog::list_products(&pool, &ProductFilter::default()).await?;
    assert_eq!(cataloged.len(), 3, "catalog holds three products");
    // Every backfilled product is L2 (legacy published scene raster).
    assert!(cataloged.iter().all(|p| p.level == ProductLevel::L2));
    Ok(())
}

#[tokio::test]
async fn two_scenes_same_kind_stay_distinct() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    insert_legacy_product(&pool, "scene-a", "ndvi", "data/a/ndvi.tif").await?;
    insert_legacy_product(&pool, "scene-b", "ndvi", "data/b/ndvi.tif").await?;

    product_catalog::backfill_products_to_catalog(&pool).await?;

    let ndvi = catalog::list_products(
        &pool,
        &ProductFilter {
            kind: Some("ndvi".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    assert_eq!(
        ndvi.len(),
        2,
        "same-kind products from two scenes must not collapse"
    );
    Ok(())
}

#[tokio::test]
async fn backfill_is_idempotent_and_preserves_legacy_rows() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool(&tmp).await?;
    insert_legacy_product(&pool, "scene-a", "ndvi", "data/a/ndvi.tif").await?;
    insert_legacy_product(&pool, "scene-b", "ndvi", "data/b/ndvi.tif").await?;

    product_catalog::backfill_products_to_catalog(&pool).await?;
    let first = catalog::list_products(&pool, &ProductFilter::default())
        .await?
        .len();
    // Re-run: no duplicate catalog rows.
    product_catalog::backfill_products_to_catalog(&pool).await?;
    let second = catalog::list_products(&pool, &ProductFilter::default())
        .await?
        .len();
    assert_eq!(first, second, "re-running backfill adds no duplicates");

    // Legacy rows are untouched (tile/serving routes read these).
    assert_eq!(count_products(&pool).await?, 2, "legacy products intact");
    Ok(())
}
