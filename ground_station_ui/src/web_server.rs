use crate::{
    CaptureEvent, LinkStateSnapshot, SharedLinkState, SharedMessageDispatchState,
    TelemetryFreshnessSnapshot, TelemetryTileSnapshot,
};
use axum::{extract::State, response::Html, Json};
use serde::Serialize;
use shared::{config::AgroConfig, AgroResult};
use std::sync::Arc;
use tracing::info;

pub struct WebServer {
    #[allow(dead_code)]
    config: Arc<AgroConfig>,
    link_state: SharedLinkState,
    dispatch_state: SharedMessageDispatchState,
}

#[derive(Clone)]
struct WebServerState {
    link_state: SharedLinkState,
    dispatch_state: SharedMessageDispatchState,
}

#[derive(Debug, Clone, Serialize)]
struct DispatchStateResponse {
    malformed_frames: u64,
    telemetry_tile: Option<TelemetryTileSnapshot>,
    telemetry_freshness: TelemetryFreshnessSnapshot,
    capture_events: Vec<CaptureEvent>,
}

impl WebServer {
    pub async fn new(
        config: Arc<AgroConfig>,
        link_state: SharedLinkState,
        dispatch_state: SharedMessageDispatchState,
    ) -> AgroResult<Self> {
        Ok(Self {
            config,
            link_state,
            dispatch_state,
        })
    }

    pub async fn run(&self) -> AgroResult<()> {
        use axum::{routing::get, Router};
        use tower_http::services::ServeDir;

        let app = Router::new()
            .route("/", get(dashboard_page))
            .route("/api/link-state", get(link_state))
            .route("/api/dispatch-state", get(dispatch_state))
            .route("/telemetry", get(telemetry_page))
            .route("/maps", get(maps_page))
            .nest_service("/static", ServeDir::new("static"))
            .with_state(WebServerState {
                link_state: self.link_state.clone(),
                dispatch_state: self.dispatch_state.clone(),
            });

        let bind_addr = "0.0.0.0:8081"; // Different port from mission control
        let listener = tokio::net::TcpListener::bind(bind_addr).await?;
        info!("Ground Station Web UI listening on http://{}", bind_addr);

        axum::serve(listener, app).await?;

        Ok(())
    }
}

async fn link_state(State(state): State<WebServerState>) -> Json<LinkStateSnapshot> {
    Json(state.link_state.read().await.snapshot())
}

async fn dispatch_state(State(state): State<WebServerState>) -> Json<DispatchStateResponse> {
    let dispatch = state.dispatch_state.read().await.clone();
    Json(DispatchStateResponse {
        malformed_frames: dispatch.malformed_frames,
        telemetry_tile: dispatch.telemetry_tile_snapshot(),
        telemetry_freshness: dispatch.telemetry_freshness(),
        capture_events: dispatch.capture_events(None),
    })
}

async fn dashboard_page() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>AgroDrone Ground Station</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f0f0f0; }
        .container { max-width: 1200px; margin: 0 auto; }
        .header { background: #2c3e50; color: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }
        .panel { background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; box-shadow: 0 2px 5px rgba(0,0,0,0.1); }
        .telemetry-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 10px; }
        .telemetry-item { background: #ecf0f1; padding: 10px; border-radius: 3px; }
        .telemetry-item.stale { background: #fdecea; border-left: 4px solid #e74c3c; }
        .telemetry-age { color: #555; font-size: 0.9em; margin-top: 8px; }
        .status-indicator { width: 20px; height: 20px; border-radius: 50%; display: inline-block; margin-right: 10px; }
        .status-connected { background: #27ae60; }
        .status-disconnected { background: #e74c3c; }
        .status-connecting { background: #f39c12; }
        .nav { margin-bottom: 20px; }
        .nav a { margin-right: 20px; text-decoration: none; color: #3498db; }
        .timeline-controls { margin-bottom: 10px; }
        .timeline-event { border-bottom: 1px solid #ddd; padding: 8px 0; }
        .timeline-event:last-child { border-bottom: 0; }
        .timeline-event small { color: #555; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚁 AgroDrone Ground Station</h1>
            <p>Real-time monitoring and control dashboard</p>
        </div>
        
        <div class="nav">
            <a href="/">Dashboard</a>
            <a href="/telemetry">Telemetry</a>
            <a href="/maps">Maps</a>
        </div>

        <div class="panel">
            <h2>System Status</h2>
            <p><span id="mission-control-indicator" class="status-indicator status-connecting"></span>Mission Control: <span id="mission-control-state">Connecting</span></p>
            <p><span class="status-indicator status-disconnected"></span>Flight Controller: Simulation Mode</p>
            <p><span class="status-indicator status-connected"></span>Sensors: Active</p>
            <p>Malformed frames: <span id="malformed-frames">0</span></p>
        </div>

        <div class="panel">
            <h2>Live Telemetry</h2>
            <p>Freshness: <span id="telemetry-freshness">No data</span></p>
            <div id="telemetry" class="telemetry-grid">
                <div class="telemetry-item">
                    <strong>Position</strong><br>
                    <span id="position">Loading...</span>
                </div>
                <div class="telemetry-item">
                    <strong>Battery</strong><br>
                    <span id="battery">Loading...</span>
                </div>
                <div class="telemetry-item">
                    <strong>Mode</strong><br>
                    <span id="mode">Loading...</span>
                </div>
                <div class="telemetry-item">
                    <strong>Speed</strong><br>
                    <span id="speed">Loading...</span>
                </div>
            </div>
            <div class="telemetry-age" id="telemetry-age">No telemetry received</div>
        </div>

        <div class="panel">
            <h2>Recent Activity</h2>
            <div id="activity">
                <p>Connecting to data stream...</p>
            </div>
        </div>

        <div class="panel">
            <h2>Capture Timeline</h2>
            <div class="timeline-controls">
                <label for="capture-filter">Type</label>
                <select id="capture-filter">
                    <option value="">All</option>
                    <option value="lidar">LiDAR</option>
                    <option value="image_captured">Image</option>
                    <option value="ndvi_processed">NDVI</option>
                </select>
            </div>
            <div id="capture-events">
                <p>No capture events received</p>
            </div>
        </div>
    </div>

    <script>
        // WebSocket connection to mission control
        const ws = new WebSocket('ws://localhost:8080/ws');
        
        ws.onopen = function() {
            console.log('Connected to mission control');
            updateActivity('Connected to mission control');
        };
        
        ws.onmessage = function(event) {
            const data = JSON.parse(event.data);
            handleWebSocketMessage(data);
        };
        
        ws.onerror = function(error) {
            console.error('WebSocket error:', error);
            updateActivity('Connection error: ' + error);
        };
        
        function handleWebSocketMessage(msg) {
            switch(msg.type) {
                case 'Telemetry':
                    updateTelemetry(msg.data);
                    break;
                case 'MissionStatus':
                    updateActivity(`Mission ${msg.mission_id}: ${msg.status}`);
                    break;
                case 'LidarUpdate':
                    updateActivity(`LiDAR scan: ${msg.scan.points.length} points`);
                    break;
                case 'ImageCaptured':
                    updateActivity(`Image captured: ${msg.image.image_id}`);
                    break;
                case 'NdviProcessed':
                    updateActivity(`NDVI processed: ${msg.result.mean_ndvi.toFixed(3)} mean`);
                    break;
                default:
                    updateActivity(`System: ${msg.status || 'Unknown event'}`);
            }
        }
        
        function updateTelemetry(telemetry) {
            document.getElementById('position').textContent = 
                `${telemetry.position.latitude.toFixed(6)}, ${telemetry.position.longitude.toFixed(6)}`;
            document.getElementById('battery').textContent = 
                `${telemetry.battery_percentage}% (${telemetry.battery_voltage.toFixed(1)}V)`;
            document.getElementById('mode').textContent = 
                `${telemetry.mode} ${telemetry.armed ? '(ARMED)' : '(DISARMED)'}`;
            document.getElementById('speed').textContent = 
                `${telemetry.ground_speed.toFixed(1)} m/s`;
        }

        function renderTelemetryTile(tile, freshness) {
            const state = freshness.state || 'no_data';
            document.getElementById('telemetry-freshness').textContent =
                state.replace('_', ' ') + (freshness.last_update_age_seconds !== null
                    ? ` (${freshness.last_update_age_seconds}s)`
                    : '');
            const tileElements = document.querySelectorAll('.telemetry-item');
            tileElements.forEach((element) => element.classList.toggle('stale', state === 'stale'));

            if (!tile) {
                document.getElementById('telemetry-age').textContent = 'No telemetry received';
                return;
            }

            document.getElementById('position').textContent =
                `${tile.latitude.toFixed(6)}, ${tile.longitude.toFixed(6)} @ ${tile.altitude_m.toFixed(1)}m`;
            document.getElementById('battery').textContent =
                `${tile.battery_percentage}% (${tile.battery_voltage.toFixed(1)}V)`;
            document.getElementById('mode').textContent =
                `${tile.mode} ${tile.armed ? '(ARMED)' : '(DISARMED)'}`;
            document.getElementById('speed').textContent =
                `${tile.ground_speed.toFixed(1)} m/s ground, ${tile.air_speed.toFixed(1)} m/s air`;
            document.getElementById('telemetry-age').textContent =
                `Last update: ${tile.last_update_age_seconds}s ago${tile.stale ? ' (stale)' : ''}`;
        }

        function renderCaptureEvents(events) {
            const container = document.getElementById('capture-events');
            const selectedType = document.getElementById('capture-filter').value;
            const filtered = selectedType
                ? events.filter((event) => event.event_type === selectedType)
                : events;

            if (filtered.length === 0) {
                container.innerHTML = '<p>No capture events received</p>';
                return;
            }

            container.innerHTML = filtered
                .map((event) => {
                    const timestamp = new Date(event.timestamp).toLocaleTimeString();
                    return `<div class="timeline-event"><strong>${event.event_type.replaceAll('_', ' ')}</strong><br><small>${timestamp}</small><br>${event.summary}</div>`;
                })
                .join('');
        }
        
        function updateActivity(message) {
            const activity = document.getElementById('activity');
            const timestamp = new Date().toLocaleTimeString();
            activity.innerHTML = `<p>[${timestamp}] ${message}</p>` + activity.innerHTML;
            
            // Keep only last 10 messages
            const messages = activity.getElementsByTagName('p');
            while (messages.length > 10) {
                activity.removeChild(messages[messages.length - 1]);
            }
        }

        async function refreshLinkState() {
            try {
                const response = await fetch('/api/link-state');
                const snapshot = await response.json();
                const state = snapshot.state || 'lost';
                const stateLabel = state.charAt(0).toUpperCase() + state.slice(1);
                const detail = snapshot.last_error ? ` (${snapshot.last_error})` : '';
                document.getElementById('mission-control-state').textContent = stateLabel + detail;
                const indicator = document.getElementById('mission-control-indicator');
                indicator.className = 'status-indicator ' + (
                    state === 'connected'
                        ? 'status-connected'
                        : (state === 'connecting' || state === 'reconnecting')
                            ? 'status-connecting'
                            : 'status-disconnected'
                );
            } catch (error) {
                document.getElementById('mission-control-state').textContent = 'Lost';
                document.getElementById('mission-control-indicator').className = 'status-indicator status-disconnected';
            }
        }

        async function refreshDispatchState() {
            try {
                const response = await fetch('/api/dispatch-state');
                const snapshot = await response.json();
                document.getElementById('malformed-frames').textContent = snapshot.malformed_frames || 0;
                renderTelemetryTile(snapshot.telemetry_tile, snapshot.telemetry_freshness || { state: 'no_data', last_update_age_seconds: null });
                renderCaptureEvents(snapshot.capture_events || []);
            } catch (error) {
                document.getElementById('malformed-frames').textContent = 'unknown';
            }
        }

        document.getElementById('capture-filter').addEventListener('change', refreshDispatchState);
        refreshLinkState();
        refreshDispatchState();
        setInterval(refreshLinkState, 1000);
        setInterval(refreshDispatchState, 1000);
    </script>
</body>
</html>
    "#,
    )
}

async fn telemetry_page() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Telemetry - AgroDrone</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f0f0f0; }
        .container { max-width: 1200px; margin: 0 auto; }
        .header { background: #2c3e50; color: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }
        .panel { background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; box-shadow: 0 2px 5px rgba(0,0,0,0.1); }
        .nav { margin-bottom: 20px; }
        .nav a { margin-right: 20px; text-decoration: none; color: #3498db; }
        table { width: 100%; border-collapse: collapse; }
        th, td { padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }
        th { background: #f8f9fa; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📊 Telemetry Data</h1>
        </div>
        
        <div class="nav">
            <a href="/">Dashboard</a>
            <a href="/telemetry">Telemetry</a>
            <a href="/maps">Maps</a>
        </div>

        <div class="panel">
            <h2>Detailed Telemetry</h2>
            <table>
                <thead>
                    <tr>
                        <th>Parameter</th>
                        <th>Value</th>
                        <th>Last Updated</th>
                    </tr>
                </thead>
                <tbody id="telemetry-table">
                    <tr><td colspan="3">Loading telemetry data...</td></tr>
                </tbody>
            </table>
        </div>
    </div>

    <script>
        // Similar WebSocket code as dashboard but focused on detailed telemetry
        const ws = new WebSocket('ws://localhost:8080/ws');
        
        ws.onmessage = function(event) {
            const data = JSON.parse(event.data);
            if (data.type === 'Telemetry') {
                updateTelemetryTable(data.data);
            }
        };
        
        function updateTelemetryTable(telemetry) {
            const tbody = document.getElementById('telemetry-table');
            const timestamp = new Date(telemetry.timestamp).toLocaleString();
            
            tbody.innerHTML = `
                <tr><td>Latitude</td><td>${telemetry.position.latitude.toFixed(6)}°</td><td>${timestamp}</td></tr>
                <tr><td>Longitude</td><td>${telemetry.position.longitude.toFixed(6)}°</td><td>${timestamp}</td></tr>
                <tr><td>Altitude</td><td>${telemetry.position.altitude.toFixed(1)} m</td><td>${timestamp}</td></tr>
                <tr><td>Battery Voltage</td><td>${telemetry.battery_voltage.toFixed(2)} V</td><td>${timestamp}</td></tr>
                <tr><td>Battery Percentage</td><td>${telemetry.battery_percentage}%</td><td>${timestamp}</td></tr>
                <tr><td>Flight Mode</td><td>${telemetry.mode}</td><td>${timestamp}</td></tr>
                <tr><td>Armed Status</td><td>${telemetry.armed ? 'ARMED' : 'DISARMED'}</td><td>${timestamp}</td></tr>
                <tr><td>Ground Speed</td><td>${telemetry.ground_speed.toFixed(1)} m/s</td><td>${timestamp}</td></tr>
                <tr><td>Air Speed</td><td>${telemetry.air_speed.toFixed(1)} m/s</td><td>${timestamp}</td></tr>
                <tr><td>Heading</td><td>${telemetry.heading.toFixed(1)}°</td><td>${timestamp}</td></tr>
                <tr><td>Relative Altitude</td><td>${telemetry.altitude_relative.toFixed(1)} m</td><td>${timestamp}</td></tr>
            `;
        }
    </script>
</body>
</html>
    "#,
    )
}

async fn maps_page() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Maps - AgroDrone</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; background: #f0f0f0; }
        .container { max-width: 1200px; margin: 0 auto; }
        .header { background: #2c3e50; color: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }
        .panel { background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; box-shadow: 0 2px 5px rgba(0,0,0,0.1); }
        .nav { margin-bottom: 20px; }
        .nav a { margin-right: 20px; text-decoration: none; color: #3498db; }
        .map-placeholder { height: 400px; background: #ecf0f1; border: 2px dashed #bdc3c7; display: flex; align-items: center; justify-content: center; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🗺️ Maps & Visualization</h1>
        </div>
        
        <div class="nav">
            <a href="/">Dashboard</a>
            <a href="/telemetry">Telemetry</a>
            <a href="/maps">Maps</a>
        </div>

        <div class="panel">
            <h2>NDVI Map</h2>
            <div class="map-placeholder">
                <p>NDVI visualization will appear here when images are processed</p>
            </div>
        </div>

        <div class="panel">
            <h2>LiDAR Point Cloud</h2>
            <div class="map-placeholder">
                <p>LiDAR scan visualization will appear here</p>
            </div>
        </div>

        <div class="panel">
            <h2>Flight Path</h2>
            <div class="map-placeholder">
                <p>Flight path and telemetry overlay will appear here</p>
            </div>
        </div>
    </div>
</body>
</html>
    "#,
    )
}
