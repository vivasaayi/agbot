use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::info;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde_json::json;

use mission_planner::{
    Mission, MissionPlannerService,
    weather_integration::WeatherIntegration,
    mission_optimizer::MissionOptimizer,
    mavlink_integration::MAVLinkConverter,
    websocket_handler::WebSocketHandler,
};

#[derive(Parser)]
#[command(name = "mission_planner")]
#[command(about = "Agricultural drone mission planning system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the mission planning web server
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },
    /// Plan a mission from a GeoJSON file
    Plan {
        #[arg(short, long)]
        input: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Validate weather conditions for a location
    Weather {
        #[arg(long)]
        lat: f64,
        #[arg(long)]
        lon: f64,
    },
}

type AppState = Arc<Mutex<MissionPlannerService>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { port, host } => {
            start_server(host, port).await?;
        }
        Commands::Plan { input, output } => {
            println!("Mission planning from file: {} -> {:?}", input, output);
            // TODO: Implement file-based mission planning
        }
        Commands::Weather { lat, lon } => {
            let weather_integration = WeatherIntegration::new(None);
            match weather_integration.get_current_weather(lat, lon).await {
                Ok(weather) => {
                    println!("Weather at {}, {}: {:?}", lat, lon, weather);
                }
                Err(e) => {
                    eprintln!("Failed to get weather: {}", e);
                }
            }
        }
    }

    Ok(())
}

async fn start_server(host: String, port: u16) -> Result<()> {
    info!("Starting mission planner server on {}:{}", host, port);

    let service = MissionPlannerService::new();
    let state = Arc::new(Mutex::new(service));
    let websocket_handler = Arc::new(WebSocketHandler::new(state.clone()));

    let app = Router::new()
        // REST API routes
        .route("/api/missions", get(list_missions))
        .route("/api/missions", post(create_mission))
        .route("/api/missions/:id", get(get_mission))
        .route("/api/missions/:id", put(update_mission))
        .route("/api/missions/:id", delete(delete_mission))
        .route("/api/missions/:id/optimize", post(optimize_mission))
        .route("/api/missions/:id/mavlink", get(export_mavlink))
        .route("/api/weather", get(get_weather))
        .route("/health", get(health_check))
        
        // WebSocket route with proper state
        .route("/ws", get({
            let handler = websocket_handler.clone();
            move |ws| WebSocketHandler::handle_upgrade(ws, axum::extract::State(handler))
        }))
        
        // Static file serving for frontend
        .route("/", get(serve_frontend))
        .fallback(serve_frontend)
        
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
    info!("Mission planner server listening on {}", listener.local_addr()?);
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({ "status": "healthy" }))
}

async fn list_missions(State(state): State<AppState>) -> Result<Json<Vec<Mission>>, StatusCode> {
    let service = state.lock().await;
    let missions = service.list_missions().await;
    Ok(Json(missions.into_iter().cloned().collect()))
}

async fn create_mission(
    State(state): State<AppState>,
    Json(mission): Json<Mission>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut service = state.lock().await;
    match service.create_mission(mission).await {
        Ok(id) => Ok(Json(json!({ "id": id }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Mission>, StatusCode> {
    let service = state.lock().await;
    match service.get_mission(&id).await {
        Some(mission) => Ok(Json(mission.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(mission): Json<Mission>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut service = state.lock().await;
    match service.update_mission(&id, mission).await {
        Ok(_) => Ok(Json(json!({ "success": true }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn delete_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut service = state.lock().await;
    match service.delete_mission(&id).await {
        Ok(_) => Ok(Json(json!({ "success": true }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn optimize_mission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Mission>, StatusCode> {
    let mut service = state.lock().await;
    
    if let Some(mission) = service.get_mission(&id).await {
        let optimizer = MissionOptimizer::new();
        match optimizer.optimize_mission(mission) {
            Ok(optimized) => {
                let _ = service.update_mission(&id, optimized.clone()).await;
                Ok(Json(optimized))
            }
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn get_weather() -> Result<Json<serde_json::Value>, StatusCode> {
    let weather_integration = WeatherIntegration::new(None);
    
    // Mock coordinates for demonstration
    match weather_integration.get_current_weather(40.7128, -74.0060).await {
        Ok(weather) => Ok(Json(serde_json::to_value(weather).unwrap())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn export_mavlink(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let service = state.lock().await;
    let mission_id = id.parse::<Uuid>().map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match service.get_mission(&mission_id).await {
        Some(mission) => {
            match MAVLinkConverter::mission_to_mavlink(&mission) {
                Ok(mavlink_mission) => {
                    let waypoint_file = MAVLinkConverter::to_waypoint_file(&mavlink_mission);
                    let flight_time = MAVLinkConverter::estimate_flight_time(&mavlink_mission, 10.0);
                    
                    Ok(Json(json!({
                        "mavlink_mission": mavlink_mission,
                        "waypoint_file": waypoint_file,
                        "estimated_flight_time_seconds": flight_time,
                        "item_count": mavlink_mission.count
                    })))
                }
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
        None => Err(StatusCode::NOT_FOUND)
    }
}

async fn serve_frontend() -> Result<axum::response::Html<&'static str>, StatusCode> {
    Ok(axum::response::Html(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>AgBot Mission Planner</title>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <style>
                body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
                .container { max-width: 800px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
                h1 { color: #2c5530; margin-bottom: 30px; }
                .info-box { background: #e8f5e8; padding: 20px; border-radius: 6px; margin: 20px 0; border-left: 4px solid #4CAF50; }
                .api-endpoint { background: #f8f8f8; padding: 15px; border-radius: 4px; margin: 10px 0; font-family: monospace; }
                .button { background: #4CAF50; color: white; padding: 12px 24px; border: none; border-radius: 4px; cursor: pointer; margin: 5px; text-decoration: none; display: inline-block; }
                .button:hover { background: #45a049; }
                .status { padding: 10px; border-radius: 4px; margin: 10px 0; }
                .status.success { background: #d4edda; color: #155724; border: 1px solid #c3e6cb; }
                .status.info { background: #d1ecf1; color: #0c5460; border: 1px solid #bee5eb; }
            </style>
        </head>
        <body>
            <div class="container">
                <h1>🚁 AgBot Mission Planner</h1>
                
                <div class="status success">
                    ✅ Mission Planner API Server is running successfully!
                </div>
                
                <div class="info-box">
                    <h3>🌟 Features Available:</h3>
                    <ul>
                        <li>✅ <strong>REST API</strong> - Full CRUD operations for missions</li>
                        <li>✅ <strong>MAVLink Integration</strong> - Convert missions to MAVLink format</li>
                        <li>✅ <strong>WebSocket Support</strong> - Real-time mission deployment</li>
                        <li>✅ <strong>Weather Integration</strong> - Flight condition checking</li>
                        <li>✅ <strong>Mission Optimization</strong> - Automatic route optimization</li>
                        <li>🚧 <strong>Frontend UI</strong> - React frontend (in development)</li>
                    </ul>
                </div>

                <h3>🔗 API Endpoints:</h3>
                <div class="api-endpoint">GET /api/missions - List all missions</div>
                <div class="api-endpoint">POST /api/missions - Create new mission</div>
                <div class="api-endpoint">GET /api/missions/{id} - Get mission details</div>
                <div class="api-endpoint">PUT /api/missions/{id} - Update mission</div>
                <div class="api-endpoint">DELETE /api/missions/{id} - Delete mission</div>
                <div class="api-endpoint">POST /api/missions/{id}/optimize - Optimize mission</div>
                <div class="api-endpoint">GET /api/missions/{id}/mavlink - Export as MAVLink</div>
                <div class="api-endpoint">GET /api/weather - Check weather conditions</div>
                <div class="api-endpoint">WS /ws - WebSocket connection for real-time updates</div>

                <div class="status info">
                    <strong>📍 Next Step:</strong> The React frontend is prepared in the <code>frontend/</code> directory. 
                    Run <code>npm install && npm start</code> in the frontend folder to launch the interactive map interface.
                </div>

                <h3>🧪 Quick Test:</h3>
                <button class="button" onclick="testApi()">Test API Connection</button>
                <button class="button" onclick="testWebSocket()">Test WebSocket</button>
                
                <div id="test-results"></div>

                <script>
                    async function testApi() {
                        const results = document.getElementById('test-results');
                        try {
                            const response = await fetch('/api/missions');
                            const data = await response.json();
                            results.innerHTML = '<div class="status success">✅ API Test Successful! Found ' + data.length + ' missions.</div>';
                        } catch (error) {
                            results.innerHTML = '<div class="status error">❌ API Test Failed: ' + error.message + '</div>';
                        }
                    }

                    function testWebSocket() {
                        const results = document.getElementById('test-results');
                        try {
                            const ws = new WebSocket('ws://localhost:3000/ws');
                            ws.onopen = () => {
                                results.innerHTML = '<div class="status success">✅ WebSocket Connected!</div>';
                                ws.send(JSON.stringify({type: 'SubscribeToUpdates'}));
                                setTimeout(() => ws.close(), 2000);
                            };
                            ws.onerror = () => {
                                results.innerHTML = '<div class="status error">❌ WebSocket Connection Failed</div>';
                            };
                        } catch (error) {
                            results.innerHTML = '<div class="status error">❌ WebSocket Test Failed: ' + error.message + '</div>';
                        }
                    }
                </script>
            </div>
        </body>
        </html>
        "#
    ))
}
