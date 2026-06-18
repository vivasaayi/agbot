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
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT ''
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
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT ''
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
        "farms",
        "status",
        "ALTER TABLE farms ADD COLUMN status TEXT NOT NULL DEFAULT 'active'",
    )
    .await?;

    ensure_column(
        pool,
        "farms",
        "updated_at",
        "ALTER TABLE farms ADD COLUMN updated_at TEXT NOT NULL DEFAULT ''",
    )
    .await?;

    ensure_column(
        pool,
        "fields",
        "status",
        "ALTER TABLE fields ADD COLUMN status TEXT NOT NULL DEFAULT 'active'",
    )
    .await?;

    ensure_column(
        pool,
        "fields",
        "updated_at",
        "ALTER TABLE fields ADD COLUMN updated_at TEXT NOT NULL DEFAULT ''",
    )
    .await?;

    sqlx::query("UPDATE farms SET updated_at = created_at WHERE trim(updated_at) = ''")
        .execute(pool)
        .await?;

    sqlx::query("UPDATE fields SET updated_at = created_at WHERE trim(updated_at) = ''")
        .execute(pool)
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
        CREATE TABLE IF NOT EXISTS scene_ingest_attempts (
            attempt_id INTEGER PRIMARY KEY AUTOINCREMENT,
            scene_id TEXT NOT NULL,
            attempt_number INTEGER NOT NULL,
            status TEXT NOT NULL,
            reason_code TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            UNIQUE(scene_id, attempt_number)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS scene_link_audits (
            audit_id TEXT PRIMARY KEY,
            scene_id TEXT NOT NULL,
            mutation TEXT NOT NULL,
            previous_field_id TEXT,
            previous_season_id TEXT,
            new_field_id TEXT NOT NULL,
            new_season_id TEXT NOT NULL,
            occurred_at TEXT NOT NULL
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
            width_px INTEGER,
            height_px INTEGER,
            gsd_m_per_px REAL,
            spatial_ref_json TEXT,
            source_image_ids_json TEXT,
            source_scan_ids_json TEXT,
            publish_status TEXT,
            qa_report_ref TEXT,
            provenance_hash TEXT,
            downstream_consumers_json TEXT,
            open_data_license TEXT,
            open_data_attribution TEXT,
            open_data_anonymized INTEGER,
            open_data_refusal_reason TEXT,
            open_data_published_at TEXT,
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
        "width_px",
        "ALTER TABLE products ADD COLUMN width_px INTEGER",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "height_px",
        "ALTER TABLE products ADD COLUMN height_px INTEGER",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "gsd_m_per_px",
        "ALTER TABLE products ADD COLUMN gsd_m_per_px REAL",
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

    ensure_column(
        pool,
        "products",
        "source_scan_ids_json",
        "ALTER TABLE products ADD COLUMN source_scan_ids_json TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "publish_status",
        "ALTER TABLE products ADD COLUMN publish_status TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "qa_report_ref",
        "ALTER TABLE products ADD COLUMN qa_report_ref TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "provenance_hash",
        "ALTER TABLE products ADD COLUMN provenance_hash TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "downstream_consumers_json",
        "ALTER TABLE products ADD COLUMN downstream_consumers_json TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "open_data_license",
        "ALTER TABLE products ADD COLUMN open_data_license TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "open_data_attribution",
        "ALTER TABLE products ADD COLUMN open_data_attribution TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "open_data_anonymized",
        "ALTER TABLE products ADD COLUMN open_data_anonymized INTEGER",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "open_data_refusal_reason",
        "ALTER TABLE products ADD COLUMN open_data_refusal_reason TEXT",
    )
    .await?;

    ensure_column(
        pool,
        "products",
        "open_data_published_at",
        "ALTER TABLE products ADD COLUMN open_data_published_at TEXT",
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
            evidence_refs_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "recommendations",
        "evidence_refs_json",
        "ALTER TABLE recommendations ADD COLUMN evidence_refs_json TEXT NOT NULL DEFAULT '[]'",
    )
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
        CREATE TABLE IF NOT EXISTS tractor_vehicles (
            tractor_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            field_id TEXT NOT NULL,
            capabilities_json TEXT NOT NULL,
            implement_ref_json TEXT NOT NULL,
            status TEXT NOT NULL,
            registered_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tractor_command_audits (
            audit_id TEXT PRIMARY KEY,
            command_id TEXT,
            tractor_id TEXT NOT NULL,
            org_id TEXT,
            field_id TEXT,
            command_type TEXT NOT NULL,
            requested_by TEXT,
            decision TEXT NOT NULL,
            reason_code TEXT NOT NULL,
            at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS weather_forecasts (
            forecast_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            field_ref TEXT NOT NULL,
            valid_time TEXT NOT NULL,
            vars_json TEXT NOT NULL,
            source TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            latitude REAL NOT NULL,
            longitude REAL NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS weather_fetch_failures (
            failure_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            field_ref TEXT NOT NULL,
            source TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            reason TEXT NOT NULL,
            latitude REAL NOT NULL,
            longitude REAL NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "weather_forecasts",
        "field_ref",
        "ALTER TABLE weather_forecasts ADD COLUMN field_ref TEXT NOT NULL DEFAULT ''",
    )
    .await?;

    ensure_column(
        pool,
        "weather_fetch_failures",
        "field_ref",
        "ALTER TABLE weather_fetch_failures ADD COLUMN field_ref TEXT NOT NULL DEFAULT ''",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fleet_components (
            component_id TEXT PRIMARY KEY,
            component_type TEXT NOT NULL,
            serial TEXT NOT NULL UNIQUE,
            airframe_id TEXT,
            installed_at TEXT,
            removed_at TEXT,
            service_history_json TEXT NOT NULL,
            flight_hours REAL NOT NULL DEFAULT 0,
            cycles INTEGER NOT NULL DEFAULT 0,
            duty_score REAL NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fleet_component_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            component_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            airframe_id TEXT,
            event_at TEXT NOT NULL,
            actor TEXT,
            details TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "fleet_components",
        "flight_hours",
        "ALTER TABLE fleet_components ADD COLUMN flight_hours REAL NOT NULL DEFAULT 0",
    )
    .await?;
    ensure_column(
        pool,
        "fleet_components",
        "cycles",
        "ALTER TABLE fleet_components ADD COLUMN cycles INTEGER NOT NULL DEFAULT 0",
    )
    .await?;
    ensure_column(
        pool,
        "fleet_components",
        "duty_score",
        "ALTER TABLE fleet_components ADD COLUMN duty_score REAL NOT NULL DEFAULT 0",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fleet_component_duty_accruals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            component_id TEXT NOT NULL,
            airframe_id TEXT NOT NULL,
            flight_hours REAL NOT NULL,
            cycles INTEGER NOT NULL,
            duty_score REAL NOT NULL,
            accrued_at TEXT NOT NULL,
            UNIQUE(session_id, component_id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS time_series_points (
            entity_ref TEXT NOT NULL,
            metric TEXT NOT NULL,
            t TEXT NOT NULL,
            value_kind TEXT NOT NULL,
            scalar_value REAL,
            source_ref TEXT NOT NULL,
            created_at TEXT NOT NULL,
            metadata_json TEXT,
            PRIMARY KEY (entity_ref, metric, t, source_ref)
        );
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "time_series_points",
        "metadata_json",
        "ALTER TABLE time_series_points ADD COLUMN metadata_json TEXT",
    )
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fleet_health_indicator_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            component_id TEXT NOT NULL,
            airframe_id TEXT,
            indicator TEXT NOT NULL,
            value REAL NOT NULL,
            ts TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            freshness TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(component_id, indicator, ts, source_ref)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fleet_health_telemetry_gaps (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            component_id TEXT NOT NULL,
            airframe_id TEXT,
            started_at TEXT NOT NULL,
            ended_at TEXT NOT NULL,
            reason TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(component_id, started_at, ended_at, source_ref)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS soil_iot_devices (
            device_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            field_id TEXT NOT NULL,
            zone_id TEXT,
            sensor_type TEXT NOT NULL,
            latitude REAL NOT NULL,
            longitude REAL NOT NULL,
            crs TEXT NOT NULL,
            calibration_profile_ref TEXT NOT NULL,
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
        CREATE TABLE IF NOT EXISTS soil_iot_reading_rejections (
            rejection_id TEXT PRIMARY KEY,
            device_id TEXT NOT NULL,
            reason TEXT NOT NULL,
            rejected_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS soil_iot_config_pushes (
            push_id TEXT PRIMARY KEY,
            device_id TEXT NOT NULL,
            config_version TEXT NOT NULL,
            pushed_at TEXT NOT NULL,
            push_status TEXT NOT NULL,
            failure_reason TEXT,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS water_moisture_readings (
            reading_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            zone_ref TEXT NOT NULL,
            value REAL NOT NULL,
            source TEXT NOT NULL,
            captured_at TEXT NOT NULL,
            qa_flag TEXT NOT NULL,
            ingested_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS water_moisture_reading_rejections (
            rejection_id TEXT PRIMARY KEY,
            reading_id TEXT,
            field_id TEXT,
            zone_ref TEXT,
            source TEXT,
            captured_at TEXT,
            reason TEXT NOT NULL,
            rejected_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS drought_indices (
            index_id TEXT PRIMARY KEY,
            field_or_region_ref TEXT NOT NULL,
            index_type TEXT NOT NULL,
            value REAL NOT NULL,
            period_start TEXT NOT NULL,
            period_end TEXT NOT NULL,
            accumulation_days INTEGER,
            input_refs_json TEXT NOT NULL,
            method TEXT NOT NULL,
            computed_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS marketplace_accounts (
            account_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            party_type TEXT NOT NULL,
            role_refs_json TEXT NOT NULL,
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
        CREATE TABLE IF NOT EXISTS marketplace_catalog_items (
            item_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            category TEXT NOT NULL,
            name TEXT NOT NULL,
            unit_of_measure TEXT NOT NULL,
            owner_account_id TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS marketplace_listings (
            listing_id TEXT PRIMARY KEY,
            item_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            price REAL NOT NULL,
            currency TEXT NOT NULL,
            available_qty REAL NOT NULL,
            window_from TEXT NOT NULL,
            window_to TEXT NOT NULL,
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
        CREATE TABLE IF NOT EXISTS marketplace_inventory (
            inventory_id TEXT PRIMARY KEY,
            item_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            on_hand REAL NOT NULL,
            reserved REAL NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS marketplace_orders (
            order_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            listing_ref TEXT NOT NULL,
            buyer_account_id TEXT NOT NULL,
            qty REAL NOT NULL,
            line_total REAL NOT NULL,
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
        CREATE TABLE IF NOT EXISTS marketplace_order_audits (
            audit_id TEXT PRIMARY KEY,
            order_id TEXT NOT NULL,
            from_status TEXT,
            to_status TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            occurred_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS marketplace_demand_forecasts (
            forecast_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            field_id TEXT NOT NULL,
            item_kind TEXT NOT NULL,
            horizon TEXT NOT NULL,
            value REAL,
            evidence_refs_json TEXT NOT NULL,
            status TEXT NOT NULL,
            uncertainty_low REAL,
            uncertainty_high REAL,
            method TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS marketplace_fulfillments (
            fulfillment_id TEXT PRIMARY KEY,
            order_ref TEXT NOT NULL,
            org_id TEXT NOT NULL,
            carrier_ref TEXT NOT NULL,
            tracking_ref TEXT NOT NULL,
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
        CREATE TABLE IF NOT EXISTS marketplace_fulfillment_audits (
            audit_id TEXT PRIMARY KEY,
            fulfillment_id TEXT NOT NULL,
            from_status TEXT,
            to_status TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            occurred_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS marketplace_ratings (
            rating_id TEXT PRIMARY KEY,
            order_ref TEXT NOT NULL,
            rater_account_id TEXT NOT NULL,
            ratee_account_id TEXT NOT NULL,
            score REAL NOT NULL,
            comment TEXT,
            org_scope TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(order_ref, rater_account_id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sustainability_records (
            record_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            season_id TEXT NOT NULL,
            operation_id TEXT NOT NULL,
            metric_type TEXT NOT NULL,
            method_version TEXT NOT NULL,
            created_at TEXT NOT NULL,
            audit_id TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS carbon_footprints (
            footprint_id TEXT PRIMARY KEY,
            record_id TEXT NOT NULL,
            operation_id TEXT NOT NULL,
            value_co2e REAL,
            inputs_json TEXT NOT NULL,
            factor_set_version TEXT NOT NULL,
            factors_json TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL,
            status TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            computed_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS biomass_estimates (
            estimate_id TEXT PRIMARY KEY,
            record_id TEXT NOT NULL,
            biomass_value REAL NOT NULL,
            area REAL NOT NULL,
            crs TEXT NOT NULL,
            extent_json TEXT NOT NULL,
            resolution_json TEXT NOT NULL,
            source_layer_refs_json TEXT NOT NULL,
            method_version TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            computed_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sustainability_baselines (
            baseline_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            season_id TEXT NOT NULL,
            metric_type TEXT NOT NULL,
            metric_value REAL NOT NULL,
            source_record_id TEXT NOT NULL,
            method_version TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sustainability_comparisons (
            comparison_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            baseline_season_id TEXT NOT NULL,
            current_season_id TEXT NOT NULL,
            metric_type TEXT NOT NULL,
            baseline_value REAL,
            current_value REAL NOT NULL,
            delta REAL,
            trend TEXT NOT NULL,
            status TEXT NOT NULL,
            baseline_source_record_id TEXT,
            current_source_record_id TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL,
            method_version TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            compared_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sustainability_mrv_trails (
            trail_id TEXT PRIMARY KEY,
            output_ref TEXT NOT NULL,
            output_kind TEXT NOT NULL,
            input_layer_refs_json TEXT NOT NULL,
            method TEXT NOT NULL,
            method_version TEXT NOT NULL,
            crs TEXT NOT NULL,
            extent_json TEXT NOT NULL,
            parameters_json TEXT NOT NULL,
            audit_id TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            rederived_result_hash TEXT NOT NULL,
            certification_ready INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS biodiversity_proxies (
            proxy_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            heterogeneity_score REAL,
            cover_fraction REAL,
            uncertainty REAL NOT NULL,
            status TEXT NOT NULL,
            crs TEXT NOT NULL,
            extent_json TEXT NOT NULL,
            source_layer_refs_json TEXT NOT NULL,
            method_version TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            computed_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS soil_carbon_proxies (
            proxy_id TEXT PRIMARY KEY,
            record_id TEXT NOT NULL,
            field_id TEXT NOT NULL,
            proxy_value REAL,
            uncertainty_low REAL,
            uncertainty_high REAL,
            status TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL,
            method_version TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            computed_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sustainability_kpis (
            kpi_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            season_id TEXT NOT NULL,
            metric_ref TEXT NOT NULL,
            current_value REAL,
            target_value REAL NOT NULL,
            direction TEXT NOT NULL,
            at_risk_fraction REAL NOT NULL,
            status TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL,
            method_version TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            computed_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sustainability_certification_packs (
            pack_id TEXT PRIMARY KEY,
            claim_id TEXT NOT NULL,
            claim_type TEXT NOT NULL,
            field_id TEXT NOT NULL,
            season_id TEXT NOT NULL,
            claimed_output_refs_json TEXT NOT NULL,
            outputs_json TEXT NOT NULL,
            evidence_layer_refs_json TEXT NOT NULL,
            mrv_trails_json TEXT NOT NULL,
            audit_ids_json TEXT NOT NULL,
            result_hash TEXT NOT NULL,
            pack_hash TEXT NOT NULL,
            method_version TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_contents (
            content_id TEXT PRIMARY KEY,
            content_type TEXT NOT NULL,
            author_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            status TEXT NOT NULL,
            current_version TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_content_versions (
            version_id TEXT PRIMARY KEY,
            content_id TEXT NOT NULL,
            body TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_content_workflow_audits (
            audit_id TEXT PRIMARY KEY,
            content_id TEXT NOT NULL,
            action TEXT NOT NULL,
            from_status TEXT NOT NULL,
            to_status TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            actor_role TEXT NOT NULL,
            occurred_at TEXT NOT NULL,
            scheduled_effective_at TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_content_tags (
            content_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            value TEXT NOT NULL,
            source TEXT NOT NULL,
            applied_at TEXT NOT NULL,
            PRIMARY KEY (content_id, kind, value)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_success_stories (
            content_id TEXT PRIMARY KEY,
            grower TEXT NOT NULL,
            crop TEXT NOT NULL,
            region TEXT NOT NULL,
            outcome_summary TEXT NOT NULL,
            metrics_json TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_content_locale_variants (
            content_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            version_id TEXT NOT NULL,
            body TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (content_id, locale)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_community_contributions (
            contribution_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            contributor_id TEXT NOT NULL,
            content_type TEXT NOT NULL,
            body TEXT NOT NULL,
            status TEXT NOT NULL,
            content_id TEXT,
            submitted_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_community_contribution_audits (
            audit_id TEXT PRIMARY KEY,
            contribution_id TEXT NOT NULL,
            action TEXT NOT NULL,
            from_status TEXT NOT NULL,
            to_status TEXT NOT NULL,
            moderator_id TEXT NOT NULL,
            occurred_at TEXT NOT NULL,
            reason TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_content_engagement_events (
            event_id TEXT PRIMARY KEY,
            content_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            period TEXT NOT NULL,
            occurred_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cms_content_engagement_summaries (
            content_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            period TEXT NOT NULL,
            views INTEGER NOT NULL,
            reads INTEGER NOT NULL,
            helpful_votes INTEGER NOT NULL,
            event_count INTEGER NOT NULL,
            evidence_refs_json TEXT NOT NULL,
            computed_at TEXT NOT NULL,
            PRIMARY KEY (content_id, org_id, period)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_channels (
            channel_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            field_ref TEXT NOT NULL,
            member_account_ids_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_messages (
            message_id TEXT PRIMARY KEY,
            channel_id TEXT NOT NULL,
            author_id TEXT NOT NULL,
            body TEXT NOT NULL,
            sent_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_message_audits (
            audit_id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            occurred_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_permission_audits (
            audit_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            actor_org_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            action TEXT NOT NULL,
            permission TEXT NOT NULL,
            allowed INTEGER NOT NULL,
            reason_code TEXT NOT NULL,
            channel_id TEXT,
            occurred_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_presence (
            org_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            state TEXT NOT NULL,
            last_seen TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (channel_id, account_id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_notifications (
            notification_id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            recipient_account_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            body TEXT NOT NULL,
            delivery_state TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_streams (
            stream_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            mission_ref TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            state TEXT NOT NULL,
            latency_budget_ms INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            evidence_refs_json TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_stream_frames (
            frame_id TEXT PRIMARY KEY,
            stream_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            captured_at TEXT NOT NULL,
            relayed_at TEXT NOT NULL,
            latency_ms INTEGER NOT NULL,
            payload_ref TEXT NOT NULL,
            encoded_ref TEXT NOT NULL,
            relay_ref TEXT NOT NULL,
            view_ref TEXT NOT NULL,
            dropped INTEGER NOT NULL,
            FOREIGN KEY(stream_id) REFERENCES collab_streams(stream_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_emergency_alerts (
            alert_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            source TEXT NOT NULL,
            severity TEXT NOT NULL,
            trigger_ref TEXT NOT NULL,
            body TEXT NOT NULL,
            state TEXT NOT NULL,
            raised_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_alert_deliveries (
            delivery_id TEXT PRIMARY KEY,
            alert_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            channel_id TEXT NOT NULL,
            recipient_account_id TEXT NOT NULL,
            delivery_state TEXT NOT NULL,
            retry_count INTEGER NOT NULL,
            last_attempt_at TEXT NOT NULL,
            FOREIGN KEY(alert_id) REFERENCES collab_emergency_alerts(alert_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_alert_audits (
            audit_id TEXT PRIMARY KEY,
            alert_id TEXT NOT NULL,
            action TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            from_state TEXT NOT NULL,
            to_state TEXT NOT NULL,
            occurred_at TEXT NOT NULL,
            FOREIGN KEY(alert_id) REFERENCES collab_emergency_alerts(alert_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_sessions (
            session_id TEXT PRIMARY KEY,
            org_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            event_count INTEGER NOT NULL,
            has_explicit_gap INTEGER NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS collab_session_events (
            event_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            org_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            occurred_at TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            subject_ref TEXT NOT NULL,
            note TEXT NOT NULL,
            FOREIGN KEY(session_id) REFERENCES collab_sessions(session_id) ON DELETE CASCADE
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
        CREATE TABLE IF NOT EXISTS crop_inference_runs (
            run_id TEXT PRIMARY KEY,
            mosaic_ref TEXT NOT NULL,
            field_id TEXT NOT NULL,
            season_id TEXT NOT NULL,
            model_id TEXT,
            model_version TEXT NOT NULL,
            status TEXT NOT NULL,
            failure_reason_code TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS crop_detection_verifications (
            detection_id TEXT PRIMARY KEY,
            task TEXT NOT NULL,
            label TEXT NOT NULL,
            confidence REAL NOT NULL,
            evidence_tile_refs_json TEXT NOT NULL,
            zone_geometry_json TEXT NOT NULL,
            verification_state TEXT NOT NULL,
            actor TEXT NOT NULL,
            verified_at TEXT NOT NULL,
            corrected_label TEXT,
            corrected_geometry_json TEXT,
            correction_label_json TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS crop_detection_correction_labels (
            label_id TEXT PRIMARY KEY,
            source_detection_id TEXT NOT NULL,
            task TEXT NOT NULL,
            label TEXT NOT NULL,
            geometry_json TEXT NOT NULL,
            actor TEXT NOT NULL,
            created_at TEXT NOT NULL,
            evidence_tile_refs_json TEXT NOT NULL,
            FOREIGN KEY(source_detection_id) REFERENCES crop_detection_verifications(detection_id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS copilot_conversations (
            conversation_id TEXT PRIMARY KEY,
            field_id TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS copilot_turns (
            turn_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            field_id TEXT NOT NULL,
            role TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS alert_fired_alerts (
            alert_id TEXT PRIMARY KEY,
            matched_rule_id TEXT NOT NULL,
            source_event_ref TEXT NOT NULL,
            source_domain TEXT NOT NULL,
            event_type TEXT NOT NULL,
            subject_ref TEXT NOT NULL,
            field_id TEXT,
            evidence_refs_json TEXT NOT NULL,
            severity TEXT NOT NULL,
            channels_json TEXT NOT NULL,
            fired_at TEXT NOT NULL,
            explanation TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS alert_rules (
            rule_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            event_type TEXT NOT NULL,
            subject_ref TEXT,
            severity TEXT NOT NULL,
            channels_json TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(rule_id, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS alert_rule_subscriptions (
            subscription_id TEXT PRIMARY KEY,
            rule_id TEXT NOT NULL,
            recipient_id TEXT NOT NULL,
            recipient_role TEXT NOT NULL,
            channels_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS alert_rule_audits (
            audit_id TEXT PRIMARY KEY,
            rule_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            previous_status TEXT NOT NULL,
            new_status TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            occurred_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS provenance_lineage_records (
            artifact_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            inputs_json TEXT NOT NULL,
            method TEXT NOT NULL,
            parameters_json TEXT NOT NULL,
            operator TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            actor_kind TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS provenance_audit_entries (
            entry_hash TEXT PRIMARY KEY,
            seq INTEGER NOT NULL,
            prev_hash TEXT,
            payload_hash TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            actor_kind TEXT NOT NULL,
            ts TEXT NOT NULL,
            action_ref TEXT NOT NULL,
            action_kind TEXT NOT NULL,
            artifact_ref TEXT,
            payload_json TEXT NOT NULL,
            occurred_at TEXT NOT NULL,
            outcome TEXT NOT NULL,
            refusal_reason TEXT
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS plugin_registrations (
            plugin_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            version TEXT NOT NULL,
            kind TEXT NOT NULL,
            host_api_version TEXT NOT NULL,
            capabilities_json TEXT NOT NULL,
            entrypoint TEXT NOT NULL,
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
        CREATE TABLE IF NOT EXISTS plugin_lifecycle_audits (
            audit_id TEXT PRIMARY KEY,
            plugin_id TEXT NOT NULL,
            previous_status TEXT NOT NULL,
            new_status TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            occurred_at TEXT NOT NULL
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
            payload_json TEXT,
            PRIMARY KEY (record_id, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(
        pool,
        "compliance_records",
        "payload_json",
        "ALTER TABLE compliance_records ADD COLUMN payload_json TEXT",
    )
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
        CREATE INDEX IF NOT EXISTS idx_tractor_vehicles_org_field_status
        ON tractor_vehicles(org_id, field_id, status);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_tractor_command_audits_tractor_id
        ON tractor_command_audits(tractor_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_soil_iot_config_pushes_device
        ON soil_iot_config_pushes(device_id, pushed_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_weather_forecasts_field_valid
        ON weather_forecasts(field_id, valid_time);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_weather_forecasts_field_ref_source_fetch
        ON weather_forecasts(field_ref, source, fetched_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_weather_fetch_failures_field
        ON weather_fetch_failures(field_id, fetched_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_weather_fetch_failures_field_ref_source_fetch
        ON weather_fetch_failures(field_ref, source, fetched_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_fleet_components_airframe_id
        ON fleet_components(airframe_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_fleet_component_events_component_id
        ON fleet_component_events(component_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_fleet_component_duty_accruals_airframe
        ON fleet_component_duty_accruals(airframe_id, session_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_time_series_points_entity_metric
        ON time_series_points(entity_ref, metric, t);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_fleet_health_indicator_samples_component
        ON fleet_health_indicator_samples(component_id, indicator, ts);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_fleet_health_telemetry_gaps_component
        ON fleet_health_telemetry_gaps(component_id, started_at, ended_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_soil_iot_devices_field_id
        ON soil_iot_devices(field_id, zone_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_soil_iot_devices_org_id
        ON soil_iot_devices(org_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_water_moisture_readings_field_zone
        ON water_moisture_readings(field_id, zone_ref, captured_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_water_moisture_readings_source
        ON water_moisture_readings(source, captured_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_water_moisture_rejections_field
        ON water_moisture_reading_rejections(field_id, rejected_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_drought_indices_scope_type_period
        ON drought_indices(field_or_region_ref, index_type, period_start, period_end);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_drought_indices_computed_at
        ON drought_indices(computed_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_accounts_org_status
        ON marketplace_accounts(org_id, status);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_accounts_party_type
        ON marketplace_accounts(party_type, org_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_catalog_items_org_kind
        ON marketplace_catalog_items(org_id, kind, category);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_catalog_items_owner
        ON marketplace_catalog_items(owner_account_id, org_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_listings_org_status
        ON marketplace_listings(org_id, status, item_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_inventory_org_item
        ON marketplace_inventory(org_id, item_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_orders_org_status
        ON marketplace_orders(org_id, status, listing_ref);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_order_audits_order
        ON marketplace_order_audits(order_id, occurred_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_demand_forecasts_org_field
        ON marketplace_demand_forecasts(org_id, field_id, horizon);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_fulfillments_org_order
        ON marketplace_fulfillments(org_id, order_ref, status);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_fulfillment_audits_fulfillment
        ON marketplace_fulfillment_audits(fulfillment_id, occurred_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_marketplace_ratings_ratee_org
        ON marketplace_ratings(ratee_account_id, org_scope, order_ref);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_sustainability_records_field_season
        ON sustainability_records(field_id, season_id, metric_type);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_carbon_footprints_record_operation
        ON carbon_footprints(record_id, operation_id, status);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_biomass_estimates_record
        ON biomass_estimates(record_id, computed_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_sustainability_baselines_field_metric
        ON sustainability_baselines(field_id, season_id, metric_type);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_sustainability_comparisons_field_status
        ON sustainability_comparisons(field_id, status, compared_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_sustainability_mrv_trails_output
        ON sustainability_mrv_trails(output_ref, output_kind, created_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_biodiversity_proxies_field_status
        ON biodiversity_proxies(field_id, status, computed_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_soil_carbon_proxies_field_status
        ON soil_carbon_proxies(field_id, status, computed_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_sustainability_kpis_field_status
        ON sustainability_kpis(field_id, season_id, status, computed_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_sustainability_certification_packs_claim
        ON sustainability_certification_packs(claim_id, claim_type, created_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_sustainability_records_audit
        ON sustainability_records(audit_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_cms_contents_org_status
        ON cms_contents(org_id, status, content_type);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_cms_content_versions_content
        ON cms_content_versions(content_id, created_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_cms_community_contributions_org_status
        ON cms_community_contributions(org_id, status);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_cms_content_engagement_events_scope
        ON cms_content_engagement_events(content_id, org_id, period);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_collab_channels_org_field
        ON collab_channels(org_id, field_ref);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_collab_messages_channel
        ON collab_messages(channel_id, sent_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_collab_message_audits_message
        ON collab_message_audits(message_id, occurred_at);
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
        CREATE INDEX IF NOT EXISTS idx_crop_inference_runs_field_season
        ON crop_inference_runs(field_id, season_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_crop_inference_runs_mosaic_ref
        ON crop_inference_runs(mosaic_ref);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_crop_detection_verifications_state
        ON crop_detection_verifications(verification_state);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_crop_detection_correction_labels_source
        ON crop_detection_correction_labels(source_detection_id);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_copilot_conversations_field
        ON copilot_conversations(field_id, created_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_copilot_turns_conversation
        ON copilot_turns(conversation_id, created_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_alert_fired_alerts_source_field
        ON alert_fired_alerts(source_domain, field_id, fired_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_alert_fired_alerts_severity_time
        ON alert_fired_alerts(severity, fired_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_alert_rules_status_event
        ON alert_rules(status, event_type);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_alert_rule_subscriptions_rule
        ON alert_rule_subscriptions(rule_id, created_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_alert_rule_audits_rule
        ON alert_rule_audits(rule_id, occurred_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_provenance_lineage_actor_date
        ON provenance_lineage_records(actor_id, created_at);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_provenance_audit_artifact_date
        ON provenance_audit_entries(artifact_ref, ts);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_provenance_audit_actor_date
        ON provenance_audit_entries(actor_id, ts);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_plugin_registrations_kind_status
        ON plugin_registrations(kind, status);
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_plugin_lifecycle_audits_plugin
        ON plugin_lifecycle_audits(plugin_id, occurred_at);
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
