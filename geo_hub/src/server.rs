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
            "/api/crop-intelligence/models",
            get(routes::list_crop_models).post(routes::register_crop_model),
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
            "/api/soil-iot/readings",
            post(routes::ingest_soil_iot_reading),
        )
        .route(
            "/api/time-series/points",
            get(routes::list_time_series_points),
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
        .route("/api/scenes", get(routes::list_scenes))
        .route("/api/layers", get(routes::list_layers))
        .route(
            "/api/layers/:scene_id/:kind",
            get(routes::get_layer_metadata),
        )
        .route(
            "/api/layers/:scene_id/:kind/export/geotiff",
            get(routes::export_layer_geotiff),
        )
        .route("/api/scenes/:scene_id", get(routes::get_scene))
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
