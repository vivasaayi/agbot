# Ground Station UI: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: WebSocket subscription or action route, client state, auth, and audit events.
- Safety: every operator action passes through `mission_control` guardrails (geofence, altitude, battery, abort) before it reaches the vehicle.
- Deterministic: rendering and binding logic that runs without AI; map overlays assert correct CRS/extent.
- Telemetry: live stream with freshness, gaps, and link-health surfaced to the operator.
- UI: dashboard, map, event timeline, alerts, and action controls on both web and CLI surfaces.
- Tests: unit (message dispatch, freshness logic), fixture (recorded WebSocket frames), UI/overlay, and one failure path (link loss / stale telemetry).
- Operations: runtime mode (`Simulation` first), reconnect/backoff, and a runbook.

## Category Epics

### EPIC-01: Live Operations Display
- Goal: a trustworthy receive-only console showing live telemetry, capture events, and alerts.
- First release: live-bound dashboard tiles with freshness, plus an event timeline for LiDAR/image/NDVI.
- Expansion: link-health and telemetry-gap indicators, severity-ranked alerts, reconnect/backoff.
- Hardening: shared client state across web and CLI, recorded-frame tests, stale-telemetry failure path.

### EPIC-02: Map and Mission Overlay
- Goal: render the flight on a real map with mission context, replacing the static map scaffolds.
- First release: basemap with drone position and flight path.
- Expansion: waypoints, geofence, and no-fly zones drawn with correct CRS/extent; NDVI/LiDAR result overlays.
- Hardening: coordinate round-trip assertions and overlay-correctness tests.

### EPIC-03: Operator Actions and Trust
- Goal: let an authenticated operator act on a live mission safely.
- First release: operator auth/session, then a guarded abort/RTH routed through `mission_control`.
- Expansion: dispatch, pause, and resume actions; full operator-action audit.
- Hardening: permissions/roles, simulation-only action gating, and negative-path tests (rejected action, lost link mid-action).
