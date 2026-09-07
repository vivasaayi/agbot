//! Schema versioning (batch 3.3e): connect_pool stamps PRAGMA user_version, a
//! reconnect preserves it (baseline not destructively re-run), and the recorded
//! version is exposed via db::schema_version.

use anyhow::Result;
use geo_hub::{db, HubConfig};
use tempfile::TempDir;

fn config_at(db_path: &std::path::Path, data_root: &std::path::Path) -> HubConfig {
    HubConfig {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        data_root: data_root.to_path_buf(),
        ..HubConfig::default()
    }
}

#[tokio::test]
async fn connect_pool_stamps_and_preserves_schema_version() -> Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("versioned.db");
    let config = config_at(&db_path, &tmp.path().join("data"));
    config.ensure_data_dirs()?;

    // Fresh connect: schema is stamped to a positive version.
    let pool = db::connect_pool(&config).await?;
    let version = db::schema_version(&pool).await?;
    assert!(
        version >= 1,
        "fresh database must be stamped, got {version}"
    );

    // Prove the schema is usable and holds data across the reconnect.
    sqlx::query(
        "INSERT INTO farms (farm_id, owner, name, created_at) \
         VALUES ('farm-1', 'org-1', 'North', '2026-07-01T00:00:00Z')",
    )
    .execute(&pool)
    .await?;
    pool.close().await;

    // Reconnect to the same file: version unchanged, data intact — the baseline
    // was skipped (fast path), not destructively re-run.
    let pool2 = db::connect_pool(&config).await?;
    assert_eq!(db::schema_version(&pool2).await?, version);
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM farms WHERE farm_id = 'farm-1'")
        .fetch_one(&pool2)
        .await?;
    assert_eq!(count, 1, "data must survive the versioned reconnect");
    Ok(())
}

#[tokio::test]
async fn foreign_keys_pragma_is_enabled_after_reconnect() -> Result<()> {
    // The FK pragma is per-connection and must be set on every startup, even
    // when the versioned baseline is skipped.
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("fk.db");
    let config = config_at(&db_path, &tmp.path().join("data"));
    config.ensure_data_dirs()?;

    db::connect_pool(&config).await?.close().await;

    // Second startup skips the baseline; foreign_keys must still be ON.
    let pool = db::connect_pool(&config).await?;
    let (fk,): (i64,) = sqlx::query_as("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await?;
    assert_eq!(fk, 1, "foreign_keys must be enabled on reconnect");
    Ok(())
}
