use crate::{config::HubConfig, routes, state::AppState};
use anyhow::Result;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::TcpListener;
use tracing::{info, warn};

async fn health_handler() -> &'static str {
    "ok"
}

async fn ready_handler() -> &'static str {
    "ready"
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(routes::mobile_app))
        .route("/app", get(routes::mobile_app))
        .route(
            "/api/mobile/scenes/search",
            post(routes::mobile_search_scenes),
        )
        .route("/api/mobile/analyze", post(routes::mobile_analyze))
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .route("/api/ingest/health", get(routes::get_ingest_health))
        .route(
            "/api/farms",
            get(routes::list_farms).post(routes::create_farm),
        )
        .route(
            "/api/farms/:farm_id",
            get(routes::get_farm)
                .put(routes::update_farm)
                .delete(routes::delete_farm),
        )
        .route("/api/farms/:farm_id/fields", get(routes::list_farm_fields))
        .route(
            "/api/farms/:farm_id/fields/history",
            get(routes::list_farm_field_history),
        )
        .route(
            "/api/fields",
            get(routes::list_fields).post(routes::create_field),
        )
        .route(
            "/api/fleet/nodes",
            get(routes::list_fleet_nodes).post(routes::enroll_fleet_node),
        )
        .route("/api/fleet/nodes/enroll", post(routes::enroll_fleet_node))
        .route("/api/fleet/nodes/:node_id", get(routes::get_fleet_node))
        .route(
            "/api/tractors",
            get(routes::list_tractors).post(routes::register_tractor),
        )
        .route("/api/tractors/:tractor_id", get(routes::get_tractor))
        .route(
            "/api/tractors/:tractor_id/motion-commands/validate",
            post(routes::validate_tractor_motion_command),
        )
        .route(
            "/api/weather/forecasts",
            get(routes::list_weather_forecasts),
        )
        .route(
            "/api/weather/forecasts/pull",
            post(routes::pull_weather_forecast),
        )
        .route(
            "/api/weather/fetch-failures",
            get(routes::list_weather_fetch_failures),
        )
        .route(
            "/api/orthomosaic/frame-sets",
            get(routes::list_orthomosaic_frame_sets).post(routes::ingest_orthomosaic_frame_set),
        )
        .route(
            "/api/orthomosaic/reconstructions",
            post(routes::submit_orthomosaic_reconstruction),
        )
        .route(
            "/api/orthomosaic/reconstructions/:recon_id",
            get(routes::get_orthomosaic_reconstruction),
        )
        .route(
            "/api/orthomosaic/reconstructions/:recon_id/status",
            put(routes::update_orthomosaic_reconstruction_status),
        )
        .route(
            "/api/orthomosaic/reconstructions/:recon_id/handoff",
            post(routes::handoff_orthomosaic_tiles),
        )
        .route(
            "/api/orthomosaic/products/:scene_id/:kind/publish-gate",
            post(routes::apply_orthomosaic_publish_gate),
        )
        .route(
            "/api/copilot/conversations",
            get(routes::list_copilot_conversations)
                .post(routes::start_copilot_conversation_handler),
        )
        .route(
            "/api/copilot/conversations/:conversation_id/turns",
            get(routes::list_copilot_turns).post(routes::create_copilot_turn_handler),
        )
        .route(
            "/api/crop-intelligence/models",
            get(routes::list_crop_models).post(routes::register_crop_model),
        )
        .route(
            "/api/crop-intelligence/inference-runs",
            post(routes::submit_crop_inference_run),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id",
            get(routes::get_crop_inference_run),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id/status",
            put(routes::update_crop_inference_run_status),
        )
        .route(
            "/api/crop-intelligence/inference-runs/:run_id/result",
            get(routes::get_crop_inference_run_result),
        )
        .route(
            "/api/crop-intelligence/detections/:detection_id/verification",
            post(routes::verify_crop_detection),
        )
        .route(
            "/api/crop-intelligence/detections/:detection_id/finding-promotion/validate",
            post(routes::validate_crop_detection_finding_promotion),
        )
        .route(
            "/api/scenes/:scene_id/crop-intelligence/detections/:detection_id/findings",
            post(routes::emit_crop_detection_finding),
        )
        .route(
            "/api/crop-intelligence/inference-requests/validate",
            post(routes::validate_crop_model_for_inference),
        )
        .route(
            "/api/compliance/records",
            get(routes::list_compliance_records).post(routes::create_compliance_record),
        )
        .route(
            "/api/compliance/records/:record_id",
            delete(routes::refuse_delete_compliance_record),
        )
        .route(
            "/api/compliance/reports/export",
            post(routes::export_compliance_audit_report),
        )
        .route(
            "/api/compliance/records/:record_id/versions",
            post(routes::append_compliance_record_version_route),
        )
        .route(
            "/api/compliance/airspace-zones",
            get(routes::list_airspace_zones).post(routes::ingest_airspace_zone),
        )
        .route(
            "/api/compliance/airspace-zones/query",
            get(routes::query_airspace_zones_for_point),
        )
        .route(
            "/api/fleet-health/components",
            get(routes::list_fleet_components).post(routes::register_fleet_component),
        )
        .route(
            "/api/fleet-health/components/:component_id/history",
            get(routes::get_fleet_component_history),
        )
        .route(
            "/api/fleet-health/components/:component_id/install",
            post(routes::install_fleet_component_route),
        )
        .route(
            "/api/fleet-health/duty-accruals",
            post(routes::accrue_fleet_component_duty),
        )
        .route(
            "/api/fleet-health/health-indicators",
            get(routes::list_fleet_health_indicators)
                .post(routes::derive_fleet_health_indicators_route),
        )
        .route(
            "/api/fleet-health/ota-rollouts/evaluate",
            post(routes::evaluate_ota_rollout_route),
        )
        .route(
            "/api/fleet-health/ota-rollouts/control",
            post(routes::apply_rollout_control_route),
        )
        .route(
            "/api/soil-iot/devices",
            get(routes::list_soil_iot_devices).post(routes::register_soil_iot_device),
        )
        .route(
            "/api/soil-iot/devices/:device_id/config-pushes",
            get(routes::list_soil_iot_config_pushes).post(routes::record_soil_iot_config_push),
        )
        .route(
            "/api/soil-iot/devices/:device_id/config-pushes/:push_id/status",
            put(routes::update_soil_iot_config_push_status),
        )
        .route(
            "/api/soil-iot/readings",
            post(routes::ingest_soil_iot_reading),
        )
        .route(
            "/api/water-management/moisture-readings",
            get(routes::list_soil_moisture_readings).post(routes::ingest_soil_moisture_reading),
        )
        .route(
            "/api/water-management/moisture-reading-rejections",
            get(routes::list_soil_moisture_rejections),
        )
        .route(
            "/api/drought-management/indices",
            get(routes::list_drought_indices),
        )
        .route(
            "/api/drought-management/indices/compute",
            post(routes::compute_drought_index_route),
        )
        .route(
            "/api/marketplace/accounts",
            get(routes::list_marketplace_accounts).post(routes::create_marketplace_account),
        )
        .route(
            "/api/marketplace/accounts/:account_id",
            get(routes::get_marketplace_account),
        )
        .route(
            "/api/marketplace/accounts/:account_id/status",
            post(routes::update_marketplace_account_status),
        )
        .route(
            "/api/marketplace/catalog/items",
            get(routes::list_marketplace_catalog_items)
                .post(routes::create_marketplace_catalog_item),
        )
        .route(
            "/api/marketplace/catalog/items/:item_id",
            get(routes::get_marketplace_catalog_item),
        )
        .route(
            "/api/portal/marketplace-entry",
            get(routes::get_marketplace_portal_entry),
        )
        .route(
            "/api/sustainability/records",
            get(routes::list_sustainability_records).post(routes::create_sustainability_record),
        )
        .route(
            "/api/sustainability/records/:record_id",
            get(routes::get_sustainability_record),
        )
        .route(
            "/api/content/items",
            get(routes::list_content_items).post(routes::create_content_item),
        )
        .route(
            "/api/content/items/:content_id",
            get(routes::get_content_item),
        )
        .route(
            "/api/content/items/:content_id/versions",
            post(routes::append_content_item_version),
        )
        .route(
            "/api/collaboration/channels",
            get(routes::list_collaboration_channels).post(routes::create_collaboration_channel),
        )
        .route(
            "/api/collaboration/channels/:channel_id",
            get(routes::get_collaboration_channel),
        )
        .route(
            "/api/collaboration/channels/:channel_id/messages",
            post(routes::post_collaboration_message),
        )
        .route(
            "/api/time-series/points",
            get(routes::list_time_series_points),
        )
        .route(
            "/api/provenance/lineage",
            get(routes::list_provenance_lineage_records),
        )
        .route(
            "/api/provenance/lineage/:artifact_id",
            get(routes::get_provenance_lineage_record),
        )
        .route(
            "/api/provenance/audit",
            get(routes::list_provenance_audit_entries),
        )
        .route(
            "/api/provenance/audit/:entry_hash",
            get(routes::get_provenance_audit_entry),
        )
        .route(
            "/api/plugins",
            get(routes::list_plugins).post(routes::register_plugin),
        )
        .route(
            "/api/plugins/:plugin_id/status",
            put(routes::update_plugin_status),
        )
        .route(
            "/api/plugins/:plugin_id/execute",
            post(routes::execute_plugin),
        )
        .route(
            "/api/alerting/fired-alerts",
            get(routes::list_fired_alerts).post(routes::store_fired_alert),
        )
        .route(
            "/api/alerting/fired-alerts/:alert_id",
            get(routes::get_fired_alert),
        )
        .route(
            "/api/alerting/rules",
            get(routes::list_alert_rules).post(routes::create_alert_rule),
        )
        .route(
            "/api/alerting/rules/:rule_id",
            get(routes::get_alert_rule_versions).put(routes::update_alert_rule),
        )
        .route(
            "/api/alerting/rules/:rule_id/status",
            put(routes::update_alert_rule_status),
        )
        .route(
            "/api/alerting/rules/:rule_id/subscriptions",
            get(routes::list_alert_rule_subscriptions).post(routes::create_alert_rule_subscription),
        )
        .route(
            "/api/fields/export/geojson",
            get(routes::export_fields_geojson),
        )
        .route(
            "/api/fields/import/geojson",
            post(routes::import_fields_geojson),
        )
        .route(
            "/api/fields/import/shapefile",
            post(routes::import_fields_shapefile),
        )
        .route("/api/fields/boundaries", get(routes::list_field_boundaries))
        .route("/api/fields/:field_id", get(routes::get_field))
        .route(
            "/api/fields/:field_id/farm/:farm_id",
            put(routes::link_field_to_farm),
        )
        .route(
            "/api/fields/:field_id/scenes",
            get(routes::list_field_scenes),
        )
        .route(
            "/api/fields/:field_id/scene-refresh-advisories",
            get(routes::list_field_scene_refresh_advisories),
        )
        .route(
            "/api/fields/:field_id/scene-change-advisories",
            get(routes::list_field_scene_change_advisories),
        )
        .route("/api/scenes", get(routes::list_scenes))
        .route("/api/layers", get(routes::list_layers))
        .route("/api/open-data/layers", get(routes::list_open_data_layers))
        .route(
            "/api/layers/:scene_id/:kind",
            get(routes::get_layer_metadata),
        )
        .route(
            "/api/layers/:scene_id/:kind/open-data",
            post(routes::publish_open_data_layer),
        )
        .route(
            "/api/layers/:scene_id/:kind/export/geotiff",
            get(routes::export_layer_geotiff),
        )
        .route("/api/scenes/:scene_id", get(routes::get_scene))
        .route("/api/scenes/:scene_id/audit", get(routes::get_scene_audit))
        .route(
            "/api/scenes/:scene_id/annotations",
            get(routes::list_scene_annotations).post(routes::create_scene_annotation),
        )
        .route(
            "/api/scenes/:scene_id/annotations/:annotation_id",
            put(routes::update_scene_annotation).delete(routes::delete_scene_annotation),
        )
        .route(
            "/api/scenes/:scene_id/recommendations",
            get(routes::list_scene_recommendations).post(routes::create_scene_recommendation),
        )
        .route(
            "/api/scenes/:scene_id/recommendations/:recommendation_id",
            get(routes::get_scene_recommendation)
                .put(routes::update_scene_recommendation)
                .delete(routes::delete_scene_recommendation),
        )
        .route(
            "/api/scenes/:scene_id/reports",
            get(routes::list_scene_reports).post(routes::generate_scene_report),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id/lineage",
            get(routes::get_scene_report_lineage),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id",
            get(routes::download_scene_report),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id/shares",
            post(routes::create_report_share),
        )
        .route(
            "/api/scenes/:scene_id/reports/:report_id/shares/:share_token",
            delete(routes::revoke_report_share),
        )
        .route(
            "/api/report-shares/:share_token",
            get(routes::download_shared_report),
        )
        .route(
            "/api/fields/:field_id/exports/records.csv",
            get(routes::export_field_records_csv),
        )
        .route(
            "/api/fields/:field_id/exports/records.geojson",
            get(routes::export_field_records_geojson),
        )
        .route(
            "/api/scenes/:scene_id/exports/annotations.csv",
            get(routes::export_scene_annotations_csv),
        )
        .route(
            "/api/scenes/:scene_id/exports/recommendations.csv",
            get(routes::export_scene_recommendations_csv),
        )
        .route(
            "/api/scenes/:scene_id/exports/annotations.geojson",
            get(routes::export_scene_annotations_geojson),
        )
        .route(
            "/api/scenes/:scene_id/exports/recommendations.geojson",
            get(routes::export_scene_recommendations_geojson),
        )
        .route(
            "/api/scenes/:scene_id/field/:field_id",
            put(routes::link_scene_to_field),
        )
        .route(
            "/api/scenes/:scene_id/products/:kind",
            get(routes::stream_product),
        )
        .route(
            "/api/scenes/:scene_id/products/:kind/tiles/:z/:x/:y.png",
            get(routes::stream_product_tile),
        )
        .with_state(state)
}

/// Start the geo_hub HTTP server using configuration and resources.
pub async fn serve(config: HubConfig, pool: crate::db::DbPool) -> Result<()> {
    let addr: SocketAddr = config.bind_address.parse()?;
    let shared_config = Arc::new(config);
    let state = AppState {
        pool: pool.clone(),
        config: Arc::clone(&shared_config),
        scene_search_cache: Default::default(),
    };

    let router = build_router(state);

    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "geo_hub listening");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            warn!(%err, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};

        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    // Give upstream tasks a short window to finish
    tokio::time::sleep(Duration::from_millis(100)).await;
    info!("shutdown signal received");
}
