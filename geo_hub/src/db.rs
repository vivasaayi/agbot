use crate::config::HubConfig;
use anyhow::Result;
use sqlx::{Pool, Sqlite, SqlitePool};
use tracing::info;

pub type DbPool = SqlitePool;

pub async fn connect_pool(config: &HubConfig) -> Result<DbPool> {
    let pool = SqlitePool::connect(&config.database_url).await?;
    apply_migrations(&pool).await?;
    Ok(pool)
}

async fn apply_migrations(pool: &Pool<Sqlite>) -> Result<()> {
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS scenes (
            scene_id TEXT PRIMARY KEY,
            sensor TEXT NOT NULL,
            acquired_at TEXT NOT NULL,
            data_path TEXT NOT NULL,
            metadata_json TEXT NOT NULL,
            cloud_cover REAL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scene_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(scene_id) REFERENCES scenes(scene_id) ON DELETE CASCADE,
            UNIQUE(scene_id, kind)
        );
        "#,
    )
    .execute(pool)
    .await?;

    info!("database ready");
    Ok(())
}
