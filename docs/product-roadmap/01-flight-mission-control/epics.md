# Flight and Mission Control: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: route or command, persistence, auth, pagination, and audit events.
- Safety: geofence, altitude, battery, and abort checks before any mutation.
- Deterministic: validation and path logic that runs without AI.
- Telemetry: live stream with freshness, gaps, and failsafe state.
- UI: mission list, map, dispatch, and live status (consumed by domain `11`).
- Tests: unit (validation/path math), fixture (MAVLink frames), API contract, and one failure path (link loss).
- Operations: runtime mode (`Simulation` first), collection health, and a runbook.

## Category Epics

### EPIC-01: Mission Lifecycle and Templates
- Goal: a field objective becomes a validated, persisted mission linked to field/season.
- First release: mission CRUD, waypoint validation, and one survey-pattern template from a boundary.
- Expansion: optimization with battery/time budget, multi-pass and overlap control.
- Hardening: versioning, audit, and replay.

### EPIC-02: Safe Dispatch and MAVLink Control
- Goal: dispatch a mission to a (sim or real) vehicle with acks, timeouts, and guardrails.
- First release: pre-flight checklist, geofence/altitude enforcement, and command ack/retry.
- Expansion: failsafe and return-to-home on link loss or low battery.
- Hardening: link-health monitoring, parameter validation, and full negative-path tests.

### EPIC-03: Live Telemetry and After-Action
- Goal: trustworthy live telemetry that feeds the ground station and capture provenance.
- First release: persisted telemetry stream with freshness and gap detection.
- Expansion: failsafe-state timeline and mission audit log.
- Hardening: replay, export, and link to the capture session in domain `04`.
</content>
