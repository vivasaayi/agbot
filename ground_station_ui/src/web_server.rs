use crate::{
    operator_actions::{
        shared_operator_action_audit_log, shared_operator_action_state, ActionAckStatus,
        MissionControlActionAck, MissionControlActionRequest, OperatorActionAuditLog,
        OperatorActionAuditRecord, OperatorActionError, OperatorActionKind, OperatorActionState,
        RejectingMissionControlActionClient, SharedMissionControlActionClient,
        SharedOperatorActionAuditLog, SharedOperatorActionState,
    },
    operator_session::{
        shared_operator_session_registry, AuthorizedOperatorAction, OperatorLoginRequest,
        OperatorSession, OperatorSessionError, OperatorSessionRegistry,
        SharedOperatorSessionRegistry,
    },
    CaptureEvent, LinkStateSnapshot, MapRenderState, SharedLinkState, SharedMessageDispatchState,
    TelemetryFreshnessSnapshot, TelemetryTileSnapshot,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use shared::control_plane::MembershipRole;
use shared::{config::AgroConfig, AgroResult};
use std::sync::Arc;
use tracing::info;

pub struct WebServer {
    #[allow(dead_code)]
    config: Arc<AgroConfig>,
    link_state: SharedLinkState,
    dispatch_state: SharedMessageDispatchState,
    operator_sessions: SharedOperatorSessionRegistry,
    operator_action_state: SharedOperatorActionState,
    mission_control_actions: SharedMissionControlActionClient,
    operator_action_audit_log: SharedOperatorActionAuditLog,
}

#[derive(Clone)]
struct WebServerState {
    link_state: SharedLinkState,
    dispatch_state: SharedMessageDispatchState,
    operator_sessions: SharedOperatorSessionRegistry,
    operator_action_state: SharedOperatorActionState,
    mission_control_actions: SharedMissionControlActionClient,
    operator_action_audit_log: SharedOperatorActionAuditLog,
}

#[derive(Debug, Clone, Serialize)]
struct DispatchStateResponse {
    malformed_frames: u64,
    telemetry_tile: Option<TelemetryTileSnapshot>,
    telemetry_freshness: TelemetryFreshnessSnapshot,
    capture_events: Vec<CaptureEvent>,
}

#[derive(Debug, Clone, Serialize)]
struct OperatorActionGateResponse {
    authorized: bool,
    operator_id: uuid::Uuid,
    org_id: uuid::Uuid,
    role: MembershipRole,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize)]
struct OperatorActionRequestBody {
    mission_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize)]
struct OperatorActionResponse {
    success: bool,
    status: &'static str,
    message: String,
    ack: Option<MissionControlActionAck>,
}

#[derive(Debug, Clone, Serialize)]
struct OperatorActionAuditResponse {
    records: Vec<OperatorActionAuditRecord>,
}

impl WebServer {
    pub async fn new(
        config: Arc<AgroConfig>,
        link_state: SharedLinkState,
        dispatch_state: SharedMessageDispatchState,
    ) -> AgroResult<Self> {
        Self::new_with_operator_sessions(
            config,
            link_state,
            dispatch_state,
            shared_operator_session_registry(OperatorSessionRegistry::default()),
        )
        .await
    }

    pub async fn new_with_operator_sessions(
        config: Arc<AgroConfig>,
        link_state: SharedLinkState,
        dispatch_state: SharedMessageDispatchState,
        operator_sessions: SharedOperatorSessionRegistry,
    ) -> AgroResult<Self> {
        Self::new_with_operator_controls(
            config,
            link_state,
            dispatch_state,
            operator_sessions,
            shared_operator_action_state(OperatorActionState::default()),
            Arc::new(RejectingMissionControlActionClient),
        )
        .await
    }

    pub async fn new_with_operator_controls(
        config: Arc<AgroConfig>,
        link_state: SharedLinkState,
        dispatch_state: SharedMessageDispatchState,
        operator_sessions: SharedOperatorSessionRegistry,
        operator_action_state: SharedOperatorActionState,
        mission_control_actions: SharedMissionControlActionClient,
    ) -> AgroResult<Self> {
        Self::new_with_operator_controls_and_audit(
            config,
            link_state,
            dispatch_state,
            operator_sessions,
            operator_action_state,
            mission_control_actions,
            shared_operator_action_audit_log(OperatorActionAuditLog::default()),
        )
        .await
    }

    pub async fn new_with_operator_controls_and_audit(
        config: Arc<AgroConfig>,
        link_state: SharedLinkState,
        dispatch_state: SharedMessageDispatchState,
        operator_sessions: SharedOperatorSessionRegistry,
        operator_action_state: SharedOperatorActionState,
        mission_control_actions: SharedMissionControlActionClient,
        operator_action_audit_log: SharedOperatorActionAuditLog,
    ) -> AgroResult<Self> {
        Ok(Self {
            config,
            link_state,
            dispatch_state,
            operator_sessions,
            operator_action_state,
            mission_control_actions,
            operator_action_audit_log,
        })
    }

    pub async fn run(&self) -> AgroResult<()> {
        let app = build_router_with_state(WebServerState {
            link_state: self.link_state.clone(),
            dispatch_state: self.dispatch_state.clone(),
            operator_sessions: self.operator_sessions.clone(),
            operator_action_state: self.operator_action_state.clone(),
            mission_control_actions: self.mission_control_actions.clone(),
            operator_action_audit_log: self.operator_action_audit_log.clone(),
        });

        let bind_addr = "0.0.0.0:8081"; // Different port from mission control
        let listener = tokio::net::TcpListener::bind(bind_addr).await?;
        info!("Ground Station Web UI listening on http://{}", bind_addr);

        axum::serve(listener, app).await?;

        Ok(())
    }
}

fn build_router_with_state(state: WebServerState) -> Router {
    use tower_http::services::ServeDir;

    Router::new()
        .route("/", get(dashboard_page))
        .route("/api/link-state", get(link_state))
        .route("/api/dispatch-state", get(dispatch_state))
        .route("/api/map-state", get(map_state))
        .route("/api/operator/login", post(operator_login))
        .route(
            "/api/operator/actions/session-check",
            post(operator_action_session_check),
        )
        .route("/api/operator/actions/audit", get(operator_action_audit))
        .route(
            "/api/operator/actions/:action",
            post(submit_operator_action),
        )
        .route("/telemetry", get(telemetry_page))
        .route("/maps", get(maps_page))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
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

async fn map_state(State(state): State<WebServerState>) -> Json<MapRenderState> {
    Json(state.dispatch_state.read().await.map_render_state())
}

async fn operator_login(
    State(state): State<WebServerState>,
    Json(request): Json<OperatorLoginRequest>,
) -> Result<Json<OperatorSession>, (StatusCode, String)> {
    let mut sessions = state.operator_sessions.write().await;
    sessions
        .login_at(request, chrono::Utc::now())
        .map(Json)
        .map_err(operator_session_error_response)
}

async fn operator_action_session_check(
    State(state): State<WebServerState>,
    headers: HeaderMap,
) -> Result<Json<OperatorActionGateResponse>, (StatusCode, String)> {
    let authorized = authorize_operator_action(&state, &headers).await?;

    Ok(Json(OperatorActionGateResponse {
        authorized: true,
        operator_id: authorized.principal.user_id,
        org_id: authorized.principal.org_id,
        role: authorized.principal.role,
        expires_at: authorized.expires_at,
    }))
}

async fn operator_action_audit(
    State(state): State<WebServerState>,
    headers: HeaderMap,
) -> Result<Json<OperatorActionAuditResponse>, (StatusCode, String)> {
    let authorized = authorize_operator_action(&state, &headers).await?;
    let records = state
        .operator_action_audit_log
        .read()
        .await
        .records_for_org(authorized.principal.org_id);

    Ok(Json(OperatorActionAuditResponse { records }))
}

async fn submit_operator_action(
    State(state): State<WebServerState>,
    Path(action): Path<String>,
    headers: HeaderMap,
    Json(body): Json<OperatorActionRequestBody>,
) -> Result<(StatusCode, Json<OperatorActionResponse>), (StatusCode, String)> {
    let authorized = authorize_operator_action(&state, &headers).await?;
    let action = action
        .parse::<OperatorActionKind>()
        .map_err(operator_action_error_response)?;

    if let Err(error) = state
        .operator_action_state
        .read()
        .await
        .ensure_simulation_validated()
    {
        return Ok(operator_action_status_response(error, None));
    }

    let request = MissionControlActionRequest::new(
        authorized.principal,
        action,
        body.mission_id,
        chrono::Utc::now(),
    );
    let result = state
        .mission_control_actions
        .submit_operator_action(request.clone());

    let audit_record = match &result {
        Ok(ack) => OperatorActionAuditRecord::from_ack(&request, ack),
        Err(error) => OperatorActionAuditRecord::from_error(&request, error, chrono::Utc::now()),
    };
    if let Err(error) = state
        .operator_action_audit_log
        .write()
        .await
        .record(audit_record)
    {
        return Ok(operator_action_status_response(error, None));
    }

    Ok(match result {
        Ok(ack) => operator_action_ack_response(ack),
        Err(error) => operator_action_status_response(error, None),
    })
}

async fn authorize_operator_action(
    state: &WebServerState,
    headers: &HeaderMap,
) -> Result<AuthorizedOperatorAction, (StatusCode, String)> {
    let token = bearer_token(headers)
        .ok_or(OperatorSessionError::MissingSession)
        .map_err(operator_session_error_response)?;
    let sessions = state.operator_sessions.read().await;
    sessions
        .authorize_action_at(token, chrono::Utc::now())
        .map_err(operator_session_error_response)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .trim()
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn operator_session_error_response(error: OperatorSessionError) -> (StatusCode, String) {
    let status = match error {
        OperatorSessionError::RoleNotAuthorized => StatusCode::FORBIDDEN,
        OperatorSessionError::InvalidCredentials
        | OperatorSessionError::MissingSession
        | OperatorSessionError::SessionNotFound
        | OperatorSessionError::SessionExpired => StatusCode::UNAUTHORIZED,
    };
    (status, error.to_string())
}

fn operator_action_error_response(error: OperatorActionError) -> (StatusCode, String) {
    let status = match &error {
        OperatorActionError::UnsupportedAction { .. } => StatusCode::BAD_REQUEST,
        OperatorActionError::SimulationLoopNotValidated => StatusCode::CONFLICT,
        OperatorActionError::MissionControlRejected { .. } => StatusCode::CONFLICT,
        OperatorActionError::MissionControlNoAck { .. } => StatusCode::GATEWAY_TIMEOUT,
        OperatorActionError::MissionControlUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
        OperatorActionError::AuditWriteFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, error.to_string())
}

fn operator_action_ack_response(
    ack: MissionControlActionAck,
) -> (StatusCode, Json<OperatorActionResponse>) {
    let status = match ack.status {
        ActionAckStatus::Accepted => StatusCode::OK,
        ActionAckStatus::Rejected => StatusCode::CONFLICT,
        ActionAckStatus::TimedOut => StatusCode::GATEWAY_TIMEOUT,
    };
    let response_status = match ack.status {
        ActionAckStatus::Accepted => "accepted",
        ActionAckStatus::Rejected => "rejected",
        ActionAckStatus::TimedOut => "timed_out",
    };
    let success = ack.status == ActionAckStatus::Accepted;
    let message = ack.message.clone();

    (
        status,
        Json(OperatorActionResponse {
            success,
            status: response_status,
            message,
            ack: Some(ack),
        }),
    )
}

fn operator_action_status_response(
    error: OperatorActionError,
    ack: Option<MissionControlActionAck>,
) -> (StatusCode, Json<OperatorActionResponse>) {
    let (status, response_status) = match &error {
        OperatorActionError::UnsupportedAction { .. } => (StatusCode::BAD_REQUEST, "unsupported"),
        OperatorActionError::SimulationLoopNotValidated => (StatusCode::CONFLICT, "disabled"),
        OperatorActionError::MissionControlRejected { .. } => (StatusCode::CONFLICT, "rejected"),
        OperatorActionError::MissionControlNoAck { .. } => {
            (StatusCode::GATEWAY_TIMEOUT, "timed_out")
        }
        OperatorActionError::MissionControlUnavailable { .. } => {
            (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
        }
        OperatorActionError::AuditWriteFailed { .. } => {
            (StatusCode::INTERNAL_SERVER_ERROR, "audit_failed")
        }
    };
    (
        status,
        Json(OperatorActionResponse {
            success: false,
            status: response_status,
            message: error.to_string(),
            ack,
        }),
    )
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
        body { font-family: Arial, sans-serif; margin: 20px; background: #f0f0f0; color: #17202a; }
        .container { max-width: 1200px; margin: 0 auto; }
        .header { background: #2c3e50; color: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; }
        .panel { background: white; padding: 20px; border-radius: 5px; margin-bottom: 20px; box-shadow: 0 2px 5px rgba(0,0,0,0.1); }
        .nav { margin-bottom: 20px; }
        .nav a { margin-right: 20px; text-decoration: none; color: #3498db; }
        .map-toolbar { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; margin-bottom: 14px; }
        .badge { background: #edf2f7; border: 1px solid #d5dde6; border-radius: 4px; padding: 6px 10px; font-size: 14px; }
        .badge.ok { background: #e8f6ef; border-color: #a9dfbf; color: #145a32; }
        .badge.error { background: #fdecea; border-color: #f5b7b1; color: #922b21; }
        .map-frame { border: 1px solid #b8c2cc; background: #dfe7ef; overflow: hidden; }
        #operation-map { width: 100%; height: auto; display: block; background: #eef3f7; }
        .map-readout { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 8px; margin-top: 12px; }
        .readout-item { background: #f8fafc; border: 1px solid #e1e7ef; border-radius: 4px; padding: 8px; }
        .readout-item strong { display: block; font-size: 12px; color: #536271; margin-bottom: 4px; text-transform: uppercase; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Maps & Visualization</h1>
        </div>
        
        <div class="nav">
            <a href="/">Dashboard</a>
            <a href="/telemetry">Telemetry</a>
            <a href="/maps">Maps</a>
        </div>

        <div class="panel">
            <div class="map-toolbar">
                <span id="map-status" class="badge">Loading</span>
                <span id="map-crs" class="badge">CRS</span>
                <span id="map-path-count" class="badge">Path: 0</span>
                <span id="map-overlay-state" class="badge">Overlay</span>
            </div>
            <div class="map-frame">
                <canvas id="operation-map" width="900" height="520"></canvas>
            </div>
            <div class="map-readout">
                <div class="readout-item">
                    <strong>Position</strong>
                    <span id="position-readout">No data</span>
                </div>
                <div class="readout-item">
                    <strong>Altitude</strong>
                    <span id="altitude-readout">No data</span>
                </div>
                <div class="readout-item">
                    <strong>Updated</strong>
                    <span id="timestamp-readout">No data</span>
                </div>
            </div>
        </div>
    </div>
    <script>
        const canvas = document.getElementById('operation-map');
        const ctx = canvas.getContext('2d');

        function drawGrid(state) {
            const width = state.basemap.width_px;
            const height = state.basemap.height_px;
            canvas.width = width;
            canvas.height = height;

            ctx.fillStyle = '#eef3f7';
            ctx.fillRect(0, 0, width, height);

            ctx.strokeStyle = '#c9d6e2';
            ctx.lineWidth = 1;
            const gridCount = 8;
            for (let i = 0; i <= gridCount; i++) {
                const x = (width / gridCount) * i;
                const y = (height / gridCount) * i;
                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, height);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(0, y);
                ctx.lineTo(width, y);
                ctx.stroke();
            }

            ctx.strokeStyle = '#6b7c8f';
            ctx.lineWidth = 2;
            ctx.strokeRect(0, 0, width, height);
        }

        function drawPath(path) {
            if (!path || path.length === 0) {
                return;
            }

            ctx.strokeStyle = '#1f7a8c';
            ctx.lineWidth = 3;
            ctx.lineJoin = 'round';
            ctx.lineCap = 'round';
            ctx.beginPath();
            path.forEach((point, index) => {
                if (index === 0) {
                    ctx.moveTo(point.x_px, point.y_px);
                } else {
                    ctx.lineTo(point.x_px, point.y_px);
                }
            });
            ctx.stroke();
        }

        function drawPolygon(vertices, strokeStyle, fillStyle, lineWidth) {
            if (!vertices || vertices.length < 3) {
                return;
            }

            ctx.beginPath();
            vertices.forEach((vertex, index) => {
                if (index === 0) {
                    ctx.moveTo(vertex.x_px, vertex.y_px);
                } else {
                    ctx.lineTo(vertex.x_px, vertex.y_px);
                }
            });
            ctx.closePath();
            ctx.fillStyle = fillStyle;
            ctx.fill();
            ctx.strokeStyle = strokeStyle;
            ctx.lineWidth = lineWidth;
            ctx.stroke();
        }

        function drawMissionOverlay(overlay) {
            if (!overlay) {
                return;
            }

            if (overlay.geofence) {
                drawPolygon(overlay.geofence.vertices, '#16803c', 'rgba(22, 128, 60, 0.10)', 3);
            }

            (overlay.no_fly_zones || []).forEach((zone) => {
                drawPolygon(zone.vertices, '#b42318', 'rgba(180, 35, 24, 0.18)', 2);
            });

            ctx.font = '12px Arial, sans-serif';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            (overlay.waypoints || []).forEach((waypoint) => {
                ctx.fillStyle = '#1d4ed8';
                ctx.strokeStyle = '#ffffff';
                ctx.lineWidth = 2;
                ctx.beginPath();
                ctx.arc(waypoint.x_px, waypoint.y_px, 6, 0, Math.PI * 2);
                ctx.fill();
                ctx.stroke();
                ctx.fillStyle = '#ffffff';
                ctx.fillText(String(waypoint.sequence), waypoint.x_px, waypoint.y_px);
            });
        }

        function drawMarker(marker) {
            if (!marker) {
                return;
            }

            ctx.fillStyle = '#e74c3c';
            ctx.strokeStyle = '#ffffff';
            ctx.lineWidth = 3;
            ctx.beginPath();
            ctx.arc(marker.x_px, marker.y_px, 8, 0, Math.PI * 2);
            ctx.fill();
            ctx.stroke();

            ctx.strokeStyle = '#2c3e50';
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(marker.x_px, marker.y_px - 16);
            ctx.lineTo(marker.x_px + 7, marker.y_px);
            ctx.lineTo(marker.x_px, marker.y_px + 16);
            ctx.stroke();
        }

        function updateReadout(state) {
            document.getElementById('map-crs').textContent = state.basemap.crs;
            document.getElementById('map-path-count').textContent = `Path: ${state.flight_path.length}`;
            const assertions = state.overlay_assertions || [];
            const allAccepted = assertions.length > 0 && assertions.every((assertion) => assertion.accepted);
            const overlayBadge = document.getElementById('map-overlay-state');
            overlayBadge.textContent = allAccepted ? 'Overlays aligned' : 'Overlay refused';
            overlayBadge.className = allAccepted ? 'badge ok' : 'badge error';

            const marker = state.current_position;
            if (!marker) {
                document.getElementById('map-status').textContent = 'No telemetry';
                document.getElementById('map-status').className = 'badge';
                document.getElementById('position-readout').textContent = 'No data';
                document.getElementById('altitude-readout').textContent = 'No data';
                document.getElementById('timestamp-readout').textContent = 'No data';
                return;
            }

            const breach = state.geofence_breach;
            document.getElementById('map-status').textContent =
                breach && breach.outside ? 'Geofence breach' : 'Live telemetry';
            document.getElementById('map-status').className =
                breach && breach.outside ? 'badge error' : 'badge ok';
            document.getElementById('position-readout').textContent =
                `${marker.latitude.toFixed(6)}, ${marker.longitude.toFixed(6)}`;
            document.getElementById('altitude-readout').textContent =
                `${marker.altitude_m.toFixed(1)} m`;
            document.getElementById('timestamp-readout').textContent =
                new Date(marker.timestamp).toLocaleTimeString();
        }

        function renderMap(state) {
            drawGrid(state);
            drawMissionOverlay(state.mission_overlay);
            drawPath(state.flight_path || []);
            drawMarker(state.current_position);
            updateReadout(state);
        }

        async function refreshMapState() {
            try {
                const response = await fetch('/api/map-state');
                const state = await response.json();
                renderMap(state);
            } catch (error) {
                drawGrid({
                    basemap: { width_px: 900, height_px: 520, crs: 'EPSG:3857' },
                    flight_path: [],
                    current_position: null,
                    overlay_assertions: []
                });
                document.getElementById('map-status').textContent = 'Map unavailable';
                document.getElementById('map-status').className = 'badge error';
            }
        }

        refreshMapState();
        setInterval(refreshMapState, 1000);
    </script>
</body>
</html>
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        operator_actions::MissionControlActionClient,
        operator_session::{
            shared_operator_session_registry, OperatorCredential, OperatorSessionConfig,
            OperatorSessionRegistry,
        },
        shared_link_state, shared_message_dispatch_state, ReconnectPolicy,
    };
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
    };
    use serde_json::json;
    use shared::control_plane::{MembershipRole, TenantPrincipal};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    use uuid::Uuid;

    #[tokio::test]
    async fn login_establishes_session_and_action_gate_accepts_bearer_token() {
        let (state, principal) = test_state(MembershipRole::Operator, "secret", 15);
        let app = build_router_with_state(state);

        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/operator/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "email": "ops@example.com",
                            "credential": "secret"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle login");
        assert_eq!(login_response.status(), StatusCode::OK);
        let body = to_bytes(login_response.into_body(), 64 * 1024)
            .await
            .expect("body should read");
        let login_json: serde_json::Value =
            serde_json::from_slice(&body).expect("login response should decode");
        let token = login_json
            .get("session_token")
            .and_then(|value| value.as_str())
            .expect("login should return a session token");

        let action_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/operator/actions/session-check")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should handle action gate");
        assert_eq!(action_response.status(), StatusCode::OK);
        let body = to_bytes(action_response.into_body(), 64 * 1024)
            .await
            .expect("body should read");
        let action_json: serde_json::Value =
            serde_json::from_slice(&body).expect("action response should decode");
        assert_eq!(
            action_json
                .get("authorized")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            action_json
                .get("operator_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned),
            Some(principal.user_id.to_string())
        );
    }

    #[tokio::test]
    async fn action_gate_rejects_missing_session_without_dispatching() {
        let (state, _) = test_state(MembershipRole::Operator, "secret", 15);
        let dispatch_state = state.dispatch_state.clone();
        let app = build_router_with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/operator/actions/session-check")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should handle action gate");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let dispatch = dispatch_state.read().await;
        assert!(dispatch.mission_statuses.is_empty());
        assert!(dispatch.system_statuses.is_empty());
        assert_eq!(dispatch.malformed_frames, 0);
    }

    #[tokio::test]
    async fn expired_session_is_rejected_by_action_gate() {
        let (state, _) = test_state(MembershipRole::Operator, "secret", -1);
        let app = build_router_with_state(state);
        let login_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/operator/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "email": "ops@example.com",
                            "credential": "secret"
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle login");
        assert_eq!(login_response.status(), StatusCode::OK);
        let body = to_bytes(login_response.into_body(), 64 * 1024)
            .await
            .expect("body should read");
        let login_json: serde_json::Value =
            serde_json::from_slice(&body).expect("login response should decode");
        let token = login_json
            .get("session_token")
            .and_then(|value| value.as_str())
            .expect("login should return a session token");

        let action_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/operator/actions/session-check")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should handle action gate");

        assert_eq!(action_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rth_action_forwards_to_mission_control_and_returns_ack() {
        let mission_id = Uuid::new_v4();
        let client = Arc::new(RecordingMissionControlActionClient::new(vec![Ok(
            MissionControlActionAck::accepted(
                OperatorActionKind::ReturnToHome,
                mission_id,
                "rth guardrails accepted",
                chrono::Utc::now(),
            ),
        )]));
        let (state, principal) = test_state_with_operator_actions(
            MembershipRole::Operator,
            "secret",
            15,
            validated_operator_action_state(),
            client.clone(),
        );
        let app = build_router_with_state(state);
        let token = login_token(app.clone(), "secret").await;

        let (status, body) = submit_action(app, &token, "return-to-home", mission_id).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body.get("success").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            body.pointer("/ack/status").and_then(|value| value.as_str()),
            Some("accepted")
        );
        assert_eq!(
            body.pointer("/ack/action").and_then(|value| value.as_str()),
            Some("return-to-home")
        );

        let requests = client.recorded_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].operator_id, principal.user_id);
        assert_eq!(requests[0].org_id, principal.org_id);
        assert_eq!(requests[0].operator_role, MembershipRole::Operator);
        assert_eq!(requests[0].action, OperatorActionKind::ReturnToHome);
        assert_eq!(requests[0].target_mission_id, mission_id);
    }

    #[tokio::test]
    async fn operator_action_is_disabled_until_simulation_loop_validates() {
        let mission_id = Uuid::new_v4();
        let client = Arc::new(RecordingMissionControlActionClient::new(vec![Ok(
            MissionControlActionAck::accepted(
                OperatorActionKind::Abort,
                mission_id,
                "abort accepted",
                chrono::Utc::now(),
            ),
        )]));
        let (state, _) = test_state_with_operator_actions(
            MembershipRole::Operator,
            "secret",
            15,
            shared_operator_action_state(OperatorActionState::default()),
            client.clone(),
        );
        let app = build_router_with_state(state);
        let token = login_token(app.clone(), "secret").await;

        let (status, body) = submit_action(app.clone(), &token, "abort", mission_id).await;
        let audit_body = fetch_action_audit(app, &token).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("success").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            body.get("status").and_then(|value| value.as_str()),
            Some("disabled")
        );
        assert_eq!(
            audit_body
                .get("records")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(0)
        );
        assert!(client.recorded_requests().is_empty());
    }

    #[tokio::test]
    async fn guardrail_rejection_is_returned_without_reporting_success() {
        let mission_id = Uuid::new_v4();
        let client = Arc::new(RecordingMissionControlActionClient::new(vec![Ok(
            MissionControlActionAck::rejected(
                OperatorActionKind::Abort,
                mission_id,
                "battery guardrail rejected abort mode",
                chrono::Utc::now(),
            ),
        )]));
        let (state, _) = test_state_with_operator_actions(
            MembershipRole::Operator,
            "secret",
            15,
            validated_operator_action_state(),
            client.clone(),
        );
        let app = build_router_with_state(state);
        let token = login_token(app.clone(), "secret").await;

        let (status, body) = submit_action(app.clone(), &token, "abort", mission_id).await;
        let audit_body = fetch_action_audit(app, &token).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body.get("success").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            body.pointer("/ack/status").and_then(|value| value.as_str()),
            Some("rejected")
        );
        assert_eq!(
            audit_body
                .pointer("/records/0/result")
                .and_then(|value| value.as_str()),
            Some("rejected")
        );
        assert_eq!(client.recorded_requests().len(), 1);
    }

    #[tokio::test]
    async fn missing_mission_control_ack_is_returned_as_timeout() {
        let mission_id = Uuid::new_v4();
        let client = Arc::new(RecordingMissionControlActionClient::new(vec![Err(
            OperatorActionError::MissionControlNoAck {
                reason: "ack deadline elapsed".to_string(),
            },
        )]));
        let (state, _) = test_state_with_operator_actions(
            MembershipRole::Operator,
            "secret",
            15,
            validated_operator_action_state(),
            client.clone(),
        );
        let app = build_router_with_state(state);
        let token = login_token(app.clone(), "secret").await;

        let (status, body) = submit_action(app, &token, "pause", mission_id).await;

        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(
            body.get("success").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            body.get("status").and_then(|value| value.as_str()),
            Some("timed_out")
        );
        assert!(body.get("ack").is_some_and(|value| value.is_null()));
        assert_eq!(client.recorded_requests().len(), 1);
    }

    #[tokio::test]
    async fn accepted_operator_action_is_queryable_from_audit_log() {
        let mission_id = Uuid::new_v4();
        let audit_log = shared_operator_action_audit_log(OperatorActionAuditLog::default());
        let client = Arc::new(RecordingMissionControlActionClient::new(vec![Ok(
            MissionControlActionAck::accepted(
                OperatorActionKind::ReturnToHome,
                mission_id,
                "rth guardrails accepted",
                chrono::Utc::now(),
            ),
        )]));
        let (state, principal) = test_state_with_operator_audit(
            MembershipRole::Operator,
            "secret",
            15,
            validated_operator_action_state(),
            client,
            audit_log,
        );
        let app = build_router_with_state(state);
        let token = login_token(app.clone(), "secret").await;

        let (status, action_body) =
            submit_action(app.clone(), &token, "return-to-home", mission_id).await;
        let audit_body = fetch_action_audit(app, &token).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            action_body.get("success").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert_eq!(
            audit_body
                .pointer("/records/0/operator_id")
                .and_then(|value| value.as_str()),
            Some(principal.user_id.to_string().as_str())
        );
        assert_eq!(
            audit_body
                .pointer("/records/0/action")
                .and_then(|value| value.as_str()),
            Some("return-to-home")
        );
        assert_eq!(
            audit_body
                .pointer("/records/0/result")
                .and_then(|value| value.as_str()),
            Some("accepted")
        );
        assert_eq!(
            audit_body
                .pointer("/records/0/target_mission_id")
                .and_then(|value| value.as_str()),
            Some(mission_id.to_string().as_str())
        );
    }

    #[tokio::test]
    async fn audit_write_failure_blocks_success_response() {
        let mission_id = Uuid::new_v4();
        let mut audit = OperatorActionAuditLog::default();
        audit.fail_next_write("audit storage unavailable");
        let audit_log = shared_operator_action_audit_log(audit);
        let client = Arc::new(RecordingMissionControlActionClient::new(vec![Ok(
            MissionControlActionAck::accepted(
                OperatorActionKind::Abort,
                mission_id,
                "abort guardrails accepted",
                chrono::Utc::now(),
            ),
        )]));
        let (state, _) = test_state_with_operator_audit(
            MembershipRole::Operator,
            "secret",
            15,
            validated_operator_action_state(),
            client.clone(),
            audit_log,
        );
        let app = build_router_with_state(state);
        let token = login_token(app.clone(), "secret").await;

        let (status, body) = submit_action(app, &token, "abort", mission_id).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            body.get("success").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            body.get("status").and_then(|value| value.as_str()),
            Some("audit_failed")
        );
        assert!(body.get("ack").is_some_and(|value| value.is_null()));
        assert_eq!(client.recorded_requests().len(), 1);
    }

    fn test_state(
        role: MembershipRole,
        credential: &str,
        session_minutes: i64,
    ) -> (WebServerState, TenantPrincipal) {
        test_state_with_operator_actions(
            role,
            credential,
            session_minutes,
            shared_operator_action_state(OperatorActionState::default()),
            Arc::new(RejectingMissionControlActionClient),
        )
    }

    fn test_state_with_operator_actions(
        role: MembershipRole,
        credential: &str,
        session_minutes: i64,
        operator_action_state: SharedOperatorActionState,
        mission_control_actions: SharedMissionControlActionClient,
    ) -> (WebServerState, TenantPrincipal) {
        test_state_with_operator_audit(
            role,
            credential,
            session_minutes,
            operator_action_state,
            mission_control_actions,
            shared_operator_action_audit_log(OperatorActionAuditLog::default()),
        )
    }

    fn test_state_with_operator_audit(
        role: MembershipRole,
        credential: &str,
        session_minutes: i64,
        operator_action_state: SharedOperatorActionState,
        mission_control_actions: SharedMissionControlActionClient,
        operator_action_audit_log: SharedOperatorActionAuditLog,
    ) -> (WebServerState, TenantPrincipal) {
        let principal = TenantPrincipal {
            user_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            role,
        };
        let sessions = OperatorSessionRegistry::with_credentials(
            OperatorSessionConfig::minutes(session_minutes),
            vec![OperatorCredential::new(
                "ops@example.com",
                credential,
                principal,
            )],
        );
        (
            WebServerState {
                link_state: shared_link_state(ReconnectPolicy::default()),
                dispatch_state: shared_message_dispatch_state(),
                operator_sessions: shared_operator_session_registry(sessions),
                operator_action_state,
                mission_control_actions,
                operator_action_audit_log,
            },
            principal,
        )
    }

    fn validated_operator_action_state() -> SharedOperatorActionState {
        let mut state = OperatorActionState::default();
        state.mark_simulation_validated("flight_sim_cpp:headless-regression");
        shared_operator_action_state(state)
    }

    async fn login_token(app: Router, credential: &str) -> String {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/operator/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "email": "ops@example.com",
                            "credential": credential
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle login");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body should read");
        let login_json: serde_json::Value =
            serde_json::from_slice(&body).expect("login response should decode");

        login_json
            .get("session_token")
            .and_then(|value| value.as_str())
            .expect("login should return session token")
            .to_string()
    }

    async fn submit_action(
        app: Router,
        token: &str,
        action: &str,
        mission_id: Uuid,
    ) -> (StatusCode, serde_json::Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/operator/actions/{action}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "mission_id": mission_id
                        })
                        .to_string(),
                    ))
                    .expect("request should build"),
            )
            .await
            .expect("router should handle operator action");
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body should read");
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("action response should decode");
        (status, json)
    }

    async fn fetch_action_audit(app: Router, token: &str) -> serde_json::Value {
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/operator/actions/audit")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should handle audit query");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body should read");
        serde_json::from_slice(&body).expect("audit response should decode")
    }

    struct RecordingMissionControlActionClient {
        requests: Mutex<Vec<MissionControlActionRequest>>,
        responses: Mutex<Vec<Result<MissionControlActionAck, OperatorActionError>>>,
    }

    impl RecordingMissionControlActionClient {
        fn new(responses: Vec<Result<MissionControlActionAck, OperatorActionError>>) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses),
            }
        }

        fn recorded_requests(&self) -> Vec<MissionControlActionRequest> {
            self.requests
                .lock()
                .expect("request lock should not be poisoned")
                .clone()
        }
    }

    impl MissionControlActionClient for RecordingMissionControlActionClient {
        fn submit_operator_action(
            &self,
            request: MissionControlActionRequest,
        ) -> Result<MissionControlActionAck, OperatorActionError> {
            self.requests
                .lock()
                .expect("request lock should not be poisoned")
                .push(request);
            self.responses
                .lock()
                .expect("response lock should not be poisoned")
                .remove(0)
        }
    }
}
