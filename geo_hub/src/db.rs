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
            owner TEXT NOT NULL DEFAULT 'unassigned',
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
            owner TEXT NOT NULL DEFAULT 'unassigned',
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
            owner TEXT NOT NULL DEFAULT 'unassigned',
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
        "farms",
        "owner",
        "ALTER TABLE farms ADD COLUMN owner TEXT NOT NULL DEFAULT 'unassigned'",
    )
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
        "fields",
        "owner",
        "ALTER TABLE fields ADD COLUMN owner TEXT NOT NULL DEFAULT 'unassigned'",
    )
    .await?;

    ensure_column(
        pool,
        "scenes",
        "owner",
        "ALTER TABLE scenes ADD COLUMN owner TEXT NOT NULL DEFAULT 'unassigned'",
    )
    .await?;

    ensure_column(
        pool,
        "scenes",
        "field_id",
        "ALTER TABLE scenes ADD COLUMN field_id TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "scenes",
        "season_id",
        "ALTER TABLE scenes ADD COLUMN season_id TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "scenes",
        "linked_at",
        "ALTER TABLE scenes ADD COLUMN linked_at TEXT",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS scene_ingests (
            scene_id TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            status_reason TEXT,
            ingested_at TEXT,
            acquisition_date TEXT,
            coverage_fraction REAL,
            source_path TEXT,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS scene_spatial_refs (
            scene_id TEXT PRIMARY KEY,
            spatial_ref_json TEXT NOT NULL,
            crs TEXT NOT NULL,
            min_lon REAL NOT NULL,
            min_lat REAL NOT NULL,
            max_lon REAL NOT NULL,
            max_lat REAL NOT NULL,
            resolution_x REAL NOT NULL,
            resolution_y REAL NOT NULL,
            geo_transform_json TEXT NOT NULL,
            asserted_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            product_id TEXT,
            scene_id TEXT NOT NULL,
            field_id TEXT,
            season_id TEXT,
            kind TEXT NOT NULL,
            path TEXT NOT NULL,
            spatial_ref_json TEXT,
            source_image_ids_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY(scene_id) REFERENCES scenes(scene_id) ON DELETE CASCADE,
            UNIQUE(scene_id, kind)
        );
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "products",
        "product_id",
        "ALTER TABLE products ADD COLUMN product_id TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "field_id",
        "ALTER TABLE products ADD COLUMN field_id TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "season_id",
        "ALTER TABLE products ADD COLUMN season_id TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "spatial_ref_json",
        "ALTER TABLE products ADD COLUMN spatial_ref_json TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "source_image_ids_json",
        "ALTER TABLE products ADD COLUMN source_image_ids_json TEXT",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS annotations (
            annotation_id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            field_id TEXT,
            author TEXT,
            crs TEXT,
            audit_id TEXT,
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

    ensure_column(
        pool,
        "annotations",
        "author",
        "ALTER TABLE annotations ADD COLUMN author TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "annotations",
        "crs",
        "ALTER TABLE annotations ADD COLUMN crs TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "annotations",
        "audit_id",
        "ALTER TABLE annotations ADD COLUMN audit_id TEXT",
    )
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
            visibility TEXT NOT NULL DEFAULT 'org',
            annotation_count INTEGER NOT NULL,
            recommendation_count INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "reports",
        "visibility",
        "ALTER TABLE reports ADD COLUMN visibility TEXT NOT NULL DEFAULT 'org'",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS report_shares (
            share_token TEXT PRIMARY KEY,
            report_id TEXT NOT NULL,
            scene_id TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            revoked_at TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY(report_id) REFERENCES reports(report_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS report_share_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            share_token TEXT NOT NULL,
            report_id TEXT NOT NULL,
            scene_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            created_at TEXT NOT NULL,
            details TEXT
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
        CREATE TABLE IF NOT EXISTS fleet_nodes (
            node_id TEXT PRIMARY KEY,
            hardware_id TEXT NOT NULL UNIQUE,
            kind TEXT NOT NULL,
            capabilities_json TEXT NOT NULL,
            owner_org_id TEXT NOT NULL,
            runtime_mode TEXT NOT NULL,
            enrolled_at TEXT NOT NULL,
            status TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS orthomosaic_frame_sets (
            frame_set_id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            field_id TEXT NOT NULL,
            season_id TEXT NOT NULL,
            frames_json TEXT NOT NULL,
            crs_hint TEXT,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS orthomosaic_reconstructions (
            recon_id TEXT PRIMARY KEY,
            frame_set_id TEXT NOT NULL,
            params_json TEXT NOT NULL,
            status TEXT NOT NULL,
            failure_reason TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(frame_set_id) REFERENCES orthomosaic_frame_sets(frame_set_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS crop_models (
            model_id TEXT NOT NULL,
            version TEXT NOT NULL,
            task TEXT NOT NULL,
            training_set_ref TEXT NOT NULL,
            metrics_json TEXT NOT NULL,
            provenance_ref TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (model_id, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS crop_model_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            model_id TEXT NOT NULL,
            version TEXT NOT NULL,
            event_type TEXT NOT NULL,
            created_at TEXT NOT NULL,
            details TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS compliance_records (
            record_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            record_type TEXT NOT NULL,
            org_id TEXT NOT NULL,
            field_id TEXT NOT NULL,
            flight_id TEXT,
            created_at TEXT NOT NULL,
            actor TEXT NOT NULL,
            provenance_ref TEXT NOT NULL,
            prior_version INTEGER,
            change_reason TEXT,
            PRIMARY KEY (record_id, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS compliance_record_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            record_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            actor TEXT,
            created_at TEXT NOT NULL,
            details TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS compliance_airspace_zones (
            zone_id TEXT PRIMARY KEY,
            zone_class TEXT NOT NULL,
            crs TEXT NOT NULL,
            geometry_json TEXT NOT NULL,
            min_lon REAL NOT NULL,
            min_lat REAL NOT NULL,
            max_lon REAL NOT NULL,
            max_lat REAL NOT NULL,
            effective_from TEXT NOT NULL,
            effective_to TEXT,
            source TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
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
        CREATE INDEX IF NOT EXISTS idx_scenes_season_id ON scenes(season_id);
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

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_report_shares_report_id ON report_shares(report_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_fleet_nodes_owner_org_id
        ON fleet_nodes(owner_org_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_orthomosaic_frame_sets_scene_id
        ON orthomosaic_frame_sets(scene_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_orthomosaic_frame_sets_field_id
        ON orthomosaic_frame_sets(field_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_orthomosaic_reconstructions_frame_set_id
        ON orthomosaic_reconstructions(frame_set_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_crop_models_task
        ON crop_models(task);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_crop_model_events_model_version
        ON crop_model_events(model_id, version);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_compliance_records_org_field
        ON compliance_records(org_id, field_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_compliance_records_record_type
        ON compliance_records(record_type);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_compliance_record_events_record_id
        ON compliance_record_events(record_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_compliance_airspace_zones_extent
        ON compliance_airspace_zones(min_lon, min_lat, max_lon, max_lat);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_compliance_airspace_zones_class
        ON compliance_airspace_zones(zone_class);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_products_product_id ON products(product_id);
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
