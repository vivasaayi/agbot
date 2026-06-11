# Autonomous Tractor: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: vehicle/registry route or command, persistence, auth scoped to org/field (via `10`), pagination, and audit events.
- Safety: geofence, e-stop, obstacle stop, and operator approval before any motion — this is the dominant pillar; no motion mutation exists without a working abort.
- Deterministic: path-following, coverage planning, and per-zone rate logic that runs without AI, with reason codes and raw evidence retained.
- Telemetry: live ground-vehicle stream with freshness, gaps, and safety-state transitions (e-stop, geofence breach, obstacle halt).
- UI: vehicle list, field map, path plan, dispatch, and live status (operator console patterns from `11`).
- Tests: unit (path/rate math), fixture (telemetry frames, prescription maps), API contract, and one failure path (e-stop / obstacle halt).
- Operations: simulation-first runtime mode, session health, and a runbook. No real ground motion until the full path passes in simulation.

## Category Epics

### EPIC-01: Vehicle Identity and Guidance Foundation
- Goal: a tractor is a registered vehicle that can follow a planned path in simulation.
- First release: tractor/implement registry (via `10`), GPS/RTK guidance with bounded cross-track error, and a field-ops session/telemetry log.
- Expansion: coverage/path planning from a field boundary (ground analog of `01` survey patterns).
- Hardening: guidance tests in simulation and after-action replay.

### EPIC-02: Ground Safety Core
- Goal: nothing moves without geofence, e-stop, obstacle detection, and operator approval.
- First release: field geofence enforcement, soft/hard e-stop, and operator approval gate.
- Expansion: obstacle detection that halts motion in the path.
- Hardening: full negative-path tests (geofence breach, e-stop, obstacle halt) and safety-event audit.

### EPIC-03: Implement Control and Prescription Execution
- Goal: the tractor executes an agronomic prescription with per-zone rate control.
- First release: implement abstraction (planter/sprayer/tiller) and per-zone rate from a management-zone map (`09`/`05`).
- Expansion: multi-vehicle coordination over a shared field (parallels `03`) and weather-window gating (via `15`).
- Hardening: prescription round-trip tests, coverage verification, and session export.
