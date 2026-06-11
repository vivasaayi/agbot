use crate::{config::HubConfig, routes, state::AppState};
use anyhow::Result;
use axum::{
    routing::{get, post, put},
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
