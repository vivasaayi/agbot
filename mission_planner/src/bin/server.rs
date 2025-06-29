use anyhow::Result;
use axum::{
    Router,
    routing::get,
    response::Json,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use mission_planner::{MissionPlannerService, MissionApi};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mission_planner=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/agbot".to_string());

    // Initialize the mission planner service
    let service = Arc::new(MissionPlannerService::new(&database_url).await?);

    // Create the API router
    let api_router = MissionApi::router(service.clone());

    // Create the main app router
    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1", api_router)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
        );

    // Start the server
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()?;

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    
    tracing::info!("Mission Planner API server starting on port {}", port);
    tracing::info!("Health check available at: http://localhost:{}/health", port);
    tracing::info!("API documentation:");
    tracing::info!("  POST   /api/v1/missions          - Create mission");
    tracing::info!("  GET    /api/v1/missions          - List missions");
    tracing::info!("  GET    /api/v1/missions/{{id}}     - Get mission");
    tracing::info!("  PUT    /api/v1/missions/{{id}}     - Update mission");
    tracing::info!("  DELETE /api/v1/missions/{{id}}     - Delete mission");
    tracing::info!("  POST   /api/v1/missions/{{id}}/optimize - Optimize mission");
    tracing::info!("  GET    /api/v1/missions/search   - Search missions");
    tracing::info!("  GET    /api/v1/missions/stats    - Get statistics");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "mission-planner",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
