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
            publish_status TEXT,
            qa_report_ref TEXT,
            provenance_hash TEXT,
            downstream_consumers_json TEXT,
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
