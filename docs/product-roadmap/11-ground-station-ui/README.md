# Ground Station UI

The real-time operations console: show live telemetry, mission status, and capture events to an operator, and (eventually) let them act on a flight safely.

## Where We Are

- Dual-mode binary: `--web` serves an HTML dashboard; default runs an async CLI console (`ground_station_ui/src/lib.rs`).
- WebSocket client connects to `mission_control`, parses `WebSocketMessage`, and dispatches telemetry, mission status, LiDAR, image, NDVI, and system-status events.
- Web surface (`web_server.rs`) renders dashboard/telemetry/maps pages with static map containers; CLI surface (`cli_interface.rs`) handles `help`/`status`/`quit`.

## Where We Should Be

- A live operations console with real map rendering, the drone's position, flight path, and geofence drawn on it.
- Telemetry and status bound to live data with freshness, gap, and link-health indicators, not static scaffold content.
- Operator actions (arm, dispatch, pause, RTH, abort) routed back to `mission_control` through guardrails, behind auth, with an audit trail.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Bind live telemetry to the dashboard with freshness and connection state.
2. Render a real basemap with the drone position and flight path.
3. Draw mission waypoints, geofence, and no-fly zones on the map.
4. Surface capture events (LiDAR, image, NDVI) and system alerts in an event timeline.
5. Add operator auth and a session model.
6. Wire guarded mission actions (dispatch, pause, RTH, abort) back to `mission_control`.

## Primary Crates

`ground_station_ui`, consuming telemetry and status from domain `01` (`mission_control`) over WebSocket, with `shared` for config and `WebSocketMessage`/`Telemetry` schemas. Distinct from the field GIS advisor viewer in domain `08`: this is the live operations console.
