# Flight and Mission Control: Current State and Target State

## Mission

Plan, dispatch, and supervise drone missions: turn a field objective into a validated waypoint mission, fly it safely via MAVLink, and stream live telemetry the operator and advisor workflow can trust.

## Current Maturity

strong partial (planning) + early partial (control): `mission_planner` has real mission CRUD, waypoints, optimization, and a PostgreSQL backend; `mission_control` has the async MAVLink/WebSocket/API skeleton but command handling and telemetry are thin and untested.

## What Exists Now

- Mission CRUD with PostgreSQL persistence and migrations (`mission_planner/src/database.rs`, ~300+ lines).
- Waypoint model with action types (takeoff, fly, land, loiter) and a flight-path optimizer (`mission_planner/src/mission_optimizer.rs`, `waypoint.rs`).
- REST API and WebSocket server for mission updates (`mission_planner/src/api.rs`, websocket handler).
- Dual-mode MAVLink client (flight vs. simulation) and a broadcast telemetry channel (`mission_control/src/mavlink_client.rs`, `websocket_server.rs`, `lib.rs`).
- Configuration-driven runtime modes via `shared` (`RuntimeMode::Flight | Simulation`).

## Gaps to Close

- MAVLink message handling, parameter validation, and ack/timeout logic are incomplete and untested in the websocket handler.
- No persisted live telemetry history, freshness tracking, or link-health/failsafe state.
- Weather and airspace constraint integration is scaffolded but minimal.
- No arming/pre-flight checklist, geofence enforcement at dispatch, or return-to-home/failsafe workflow.
- No survey-pattern mission templates (lawnmower, grid, perimeter) despite being the core ag capture pattern.
- No test coverage on the control path.

## Source Modules Reviewed

- `mission_planner/src/database.rs`, `mission_optimizer.rs`, `api.rs`, `waypoint.rs`, websocket handler
- `mission_control/src/lib.rs`, `mavlink_client.rs`, `websocket_server.rs`, `api_server.rs`
- `shared/src/config.rs`, `schemas.rs` (Telemetry, WebSocketMessage)

## Target Operating Model

- One mission identity linked to field, season, and capture session, with full lifecycle state.
- Pre-flight validation: geofence, altitude ceiling, no-fly zones, battery budget, and waypoint sanity, all before arming.
- MAVLink command path with acks, timeouts, retries, and link-health monitoring; deterministic before any autonomy.
- Live telemetry persisted with freshness, gaps, and failsafe transitions, exported to the ground station and capture provenance.
- Survey-pattern templates that generate coverage missions from a field boundary.
- Simulation-first: every control path is exercised against the `02` digital twin before flight hardware.
</content>
