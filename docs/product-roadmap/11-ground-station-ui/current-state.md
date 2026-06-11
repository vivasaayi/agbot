# Ground Station UI: Current State and Target State

## Mission

Be the operator's real-time window into a live flight: stream trustworthy telemetry and capture events from `mission_control`, render position and coverage on a map, and let the operator act on the aircraft safely. This is the operations console, distinct from the post-flight GIS advisor viewer in domain `08`.

## Current Maturity

early partial: the WebSocket transport and message dispatch are solid and typed, but both the web and CLI surfaces are frameworks with minimal interactivity. There is no real map rendering, no live data binding on the web pages, no operator actions back to flight control, and no auth.

## What Exists Now

- Dual-mode entry point: `Args::web` switches between an HTML web server and an async CLI console (`ground_station_ui/src/lib.rs`).
- WebSocket client that connects to `mission_control`, parses `WebSocketMessage`, and dispatches all six variants: telemetry, mission status, LiDAR update, image captured, NDVI processed, and system status.
- CLI telemetry display (position, battery, mode/armed, ground/air speed, heading, relative altitude) printed per update (`lib.rs::display_telemetry`).
- Web server with three routes (dashboard, telemetry, maps) on port 8081, each serving static HTML with an embedded browser-side WebSocket client (`web_server.rs`).
- CLI command loop with `help`, `status`, and `quit` (`cli_interface.rs`).
- LiDAR scan-count display and NDVI result notifications (mean NDVI, vegetation percentage) on event receipt.

## Gaps to Close

- Web telemetry/maps pages are static scaffolds: the maps page is three `.map-placeholder` divs with no rendering engine.
- No live data binding on the web surface beyond the inline demo script; no shared client state, reconnect, or buffering.
- No operator actions: the UI is receive-only and cannot arm, dispatch, pause, RTH, or abort a mission.
- No authentication, session, or role model; no audit of operator intent.
- No connection-health, freshness, or telemetry-gap indicators surfaced to the operator.
- CLI `status` is hardcoded ("Connected"/"Receiving") rather than reflecting real link state.
- No tests on message dispatch, rendering, or the action path.

## Source Modules Reviewed

- `ground_station_ui/src/lib.rs` (Args, GroundStationUI, WebSocket client, message dispatch, telemetry display)
- `ground_station_ui/src/web_server.rs` (axum routes, dashboard/telemetry/maps HTML)
- `ground_station_ui/src/cli_interface.rs` (async terminal command loop)
- `shared/src/schemas.rs` (`WebSocketMessage`, `Telemetry`), `shared/src/config.rs` (`ServerConfig::ws_bind_address`)

## Target Operating Model

- One operations console bound to a live mission, showing position, flight path, geofence, and no-fly zones on a real basemap.
- Telemetry and status driven by live data with explicit freshness, gap, and link-health indicators.
- Capture events (LiDAR, image, NDVI) and system alerts collected into an inspectable event timeline.
- Operator actions routed back to `mission_control` (domain `01`) only through its guardrails, behind auth, every action audited.
- Simulation-first: the full operator loop is exercised against the domain `02` digital twin before it touches flight hardware.
