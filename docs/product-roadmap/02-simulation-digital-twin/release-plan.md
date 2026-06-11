# Simulation and Digital Twin: Release Plan

## Shipment Strategy

Ship in two layers. The reliability backbone (EPIC-00) is the prerequisite for everything else: `TwinContractV1`, deterministic runner mode, safety parity harness, terrain no-data model, scenario manifest, trace diff CLI, fault injection library, and simulation health/operability must land in M1/M2 before the synthetic-perception stack is meaningful for regression. Without a deterministic runner, golden traces are not reproducible. Without the safety parity harness, sim-first safety testing is theater. Without the trace diff CLI, golden failures are opaque. Without the terrain no-data model, a missing DEM tile silently becomes flat zero.

On top of the reliability backbone, ship capture and sensor fidelity (M2): capture replay adapter, sensor calibration profiles, mission validation report, and single-runner deterministic regression (same-seed byte-identity plus manifest hash reproducibility across builds/platforms).

On top of that foundation, ship a deterministic physics/sensor twin with golden fixtures (M1/M3) and capture-shaped sensor simulation and georeferenced terrain (M2/M3) to feed `04`/`06`. On that terrain foundation sits the synthetic-perception stack (M3–M5): georeferenced scene synthesis (buildings + farm vegetation), a ray-traced drone camera with configurable FOV, telemetry/video streaming to an external collector, and labeled dataset export — the sim-first path intended to surface the majority of integration issues before any real-world flight. Interactive mission preview and location-anchored scenario loading (M4) follow. Closed-loop autonomy preview against `03` (M5) is last.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 18 |
| M3 explainable | 26 |
| M4 interactive | 20 |
| M5 autonomous-assist | 11 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 32 |
| P1 | 39 |
| P2 | 20 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 18 |
| M | 45 |
| S | 28 |

## First P0 Vertical Slices

### Reliability Backbone (M1/M2 — prerequisites for all regression work)

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | TwinContractV1: versioned contract (commands, telemetry, trace, manifest, errors, capabilities) | explainability and trust | contract |
| M1 foundation | M | Deterministic runner mode (fixed timestep, seeded PRNG, byte-identical output) | explainability and trust | regression |
| M1 foundation | S | Safety parity harness (geofence/altitude/battery/no-fly-zone/abort rules identical to real path) | safety | parity |
| M2 captured | S | Terrain no-data model (available/missing/stale/synthetic/flat_fallback; missing ≠ flat zero) | geospatial correctness | terrain |
| M2 captured | M | Scenario manifest (simulator version, seed, mission, tiles, weather, sensor config, hashes) | explainability and trust | manifest |
| M2 captured | M | Trace diff CLI (`agbot-sim diff <a> <b>` → divergent step and field) | explainability and trust | regression |
| M2 captured | M | Fault injection library (seeded: wind gusts, GPS drift, IMU noise, dropout, comm loss, battery, bad tile, actuator lag) | safety | fault-injection |
| M2 captured | S | Simulation health/operability (health checks, seed logging, trace retention, cache controls, runbook) | operability | operations |

### Physics, Capture, and Terrain (M1/M2/M3)

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Per-drone physics (gravity/drag/thrust/battery) | explainability and trust | regression |
| M1 foundation | M | PID flight controller and command modes | safety | regression |
| M3 explainable | S | Sensor suite (GPS/IMU/baro/mag) | data quality | evaluator |
| M3 explainable | M | Wind and aerodynamic disturbance | performance and scale | physics |
| M2 captured | L | LiDAR sensor simulation | data quality | capture |
| M3 explainable | M | OSM map-tile and terrain loading | geospatial correctness | terrain |
| M4 interactive | M | Globe navigation and flight UI (`flight_sim_cpp`) | agronomic value | preview |
| M4 interactive | M | Twin-as-backend for flight/coordination | safety | operations |

## Execution Rules

**Reliability backbone rules (prerequisites — enforced before any synthetic-perception work is accepted):**

- `TwinContractV1` must exist and be versioned before any consumer crate (`01`, `03`, `04`) wires against the twin. A schema drift without a version bump is a blocking defect.
- Deterministic runner mode must be verified by a byte-identical multi-run test before any golden fixture is committed. A golden fixture produced by a non-deterministic runner is not a golden fixture.
- The safety parity harness must pass in CI before the twin can be called "safe for sim-first testing." A gap in the harness (missing rule, untested rule) is a P0 blocking defect, not a backlog item.
- Missing terrain must never silently become flat zero. The terrain no-data model must be in place before any DEM tile loading work is accepted. Every terrain gap must produce an explicit `missing` or `flat_fallback` state tag, never a silent zero.
- Every headless run must emit a scenario manifest. A run without a manifest is a rejected run.
- The trace diff CLI must exist before golden regression failures can be called actionable. A golden test that fails with only "mismatch" and no field names is not a useful signal.
- Fault injection must be seeded and reproducible. An injected fault that cannot be reproduced by seed is not a test fixture — it is noise.
- Simulation health/operability must pass before the twin is used in any CI pipeline. A twin with no health check is an invisible failure mode.

**Physics, terrain, and capture rules:**

- The twin must enforce the same geofence/altitude/battery/no-fly-zone/abort limits as the real path, or sim-first testing is not meaningful.
- Every physics/controller P0 ships with a deterministic, seeded golden-telemetry fixture run in CI.
- Every terrain P0 must assert CRS, extent, and resolution, and round-trip a known coordinate.
- The canonical-simulator question is resolved: `flight_sim_cpp` is the single canonical simulator for both the interactive viewer and headless CI regression (golden fixtures). The Bevy `simulator` crate and the Rust `drone_simulator` crate were both retired; do not reintroduce a second simulator or mission-preview surface.
- Do not ship the LiDAR/camera sim as "done" until it emits capture-shaped output consumable by domain `04`.
- Scene synthesis must be seeded and manifest-backed: every rendered object traces to a source feature (OSM footprint or land-cover class) or is explicitly marked synthetic filler.
- The ray-traced camera and dataset export share one geometry path — the renderer is the labeler — so ground-truth masks can never drift from the rendered frames.
- Single-runner determinism is mandatory: the same mission and seed must reproduce a byte-identical trace and a matching scenario-manifest hash across builds and platforms. A determinism or manifest-hash divergence that the regression gate does not catch is a P1 defect.
