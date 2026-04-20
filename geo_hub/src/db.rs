use crate::config::HubConfig;
use anyhow::Result;
use sqlx::{Pool, Row, Sqlite, SqlitePool};
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
        CREATE TABLE IF NOT EXISTS farms (
            farm_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            notes TEXT,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fields (
            field_id TEXT PRIMARY KEY,
            farm_id TEXT,
            name TEXT NOT NULL,
            crop TEXT,
            season TEXT,
            notes TEXT,
            boundary_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
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

    ensure_column(
        pool,
        "fields",
        "farm_id",
        "ALTER TABLE fields ADD COLUMN farm_id TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "scenes",
        "field_id",
        "ALTER TABLE scenes ADD COLUMN field_id TEXT",
    )
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

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS annotations (
            annotation_id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            field_id TEXT,
            label TEXT NOT NULL,
            note TEXT,
            severity TEXT,
            geometry_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS recommendations (
            recommendation_id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            field_id TEXT,
            title TEXT NOT NULL,
            note TEXT,
            category TEXT,
            priority TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS recommendation_annotations (
            recommendation_id TEXT NOT NULL,
            annotation_id TEXT NOT NULL,
            PRIMARY KEY (recommendation_id, annotation_id),
            FOREIGN KEY(recommendation_id) REFERENCES recommendations(recommendation_id) ON DELETE CASCADE,
            FOREIGN KEY(annotation_id) REFERENCES annotations(annotation_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS reports (
            report_id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            field_id TEXT,
            title TEXT NOT NULL,
            format TEXT NOT NULL,
            path TEXT NOT NULL,
            annotation_count INTEGER NOT NULL,
            recommendation_count INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_fields_farm_id ON fields(farm_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_scenes_field_id ON scenes(field_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_annotations_scene_id ON annotations(scene_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_annotations_field_id ON annotations(field_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_recommendations_scene_id ON recommendations(scene_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_recommendations_field_id ON recommendations(field_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_recommendation_annotations_annotation_id
        ON recommendation_annotations(annotation_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_reports_scene_id ON reports(scene_id);
        "#,
    )
    .execute(pool)
    .await?;

    info!("database ready");
    Ok(())
}

async fn ensure_column(
    pool: &Pool<Sqlite>,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<()> {
    let pragma = format!("PRAGMA table_info({table});");
    let rows = sqlx::query(&pragma).fetch_all(pool).await?;
    let column_exists = rows
        .iter()
        .any(|row| row.get::<String, _>("name") == column);

    if !column_exists {
        sqlx::query(alter_sql).execute(pool).await?;
    }

    Ok(())
}
