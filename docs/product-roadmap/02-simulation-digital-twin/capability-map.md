# Simulation and Digital Twin: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety, geospatial correctness, data quality, performance and scale, operability, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Simulation and Digital Twin Domain

### Reliability Backbone (must land before synthetic-perception stack is meaningful)

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| TwinContractV1: versioned contract (commands, telemetry, trace files, scenario manifests, errors, capabilities) | early-implemented (partial: `contract_version` 1.0.0 emitted by the C++ runner) | 6 | Define and version the wire contract; contract test fails on any schema drift |
| Deterministic runner mode (fixed timestep, seeded PRNG, deterministic timestamps/IDs, byte-identical output) | early-implemented (`DeterministicRunner` in `flight_sim_cpp`; `--seed` required, byte-identity tested) | 5 | Fixed-timestep runner with seeded PRNG producing byte-identical output across runs |
| Safety parity harness (geofence, altitude, battery, no-fly-zone, abort rules identical to real path) | early-implemented (C++ safety rule codes, coverage harness, and twin failsafe integration; authoritative `01`/`03` parity pending) | 6 | CI test that proves each safety rule fires identically in twin and real path; fails on any gap |
| Terrain no-data model (available / missing / stale / synthetic / flat_fallback states) | early-implemented (elevation composites tag `available` and missing-tile `flat_fallback`; cache/fetch state propagation pending) | 4 | Load DEM tiles with explicit state tags; missing tile never silently becomes flat zero |
| Scenario manifest (simulator version, seed, mission, terrain tiles, weather, sensor configs, safety config, source data, output hashes) | early-implemented (partial: per-run `RunManifest` with version/seed/timestep/mission_hash/output_hash emitted by the C++ runner) | 5 | Every headless run emits a manifest; run without manifest is rejected |
| Trace diff CLI (compares two simulation traces, reports exact divergent fields/steps) | early-implemented (`agbot-sim diff` reports first divergent step/field; tolerance and contract-version checks pending) | 4 | `agbot-sim diff <trace-a> <trace-b>` reports first divergent step and field name |
| Fault injection library (seeded wind gusts, GPS drift, IMU noise, sensor dropout, comm loss, low battery, stale terrain, bad tile, actuator lag) | missing | 8 | Inject each fault type by seed; CI regression suite covers each class |
| Simulation health/operability (headless health checks, run status, seed logging, trace retention, cache controls, runbook) | missing | 5 | `agbot-sim health` returns structured pass/fail; seed and version logged per run |

### Sensor and Capture Fidelity (should land before scene synthesis is wired end-to-end)

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Capture replay adapter (simulated sensor output routed through domain `04` ingestion path) | early partial (02-07) | 5 | Simulated LiDAR/camera/multispectral flows through the same `04` `FlightDataRecord` path as real hardware |
| Sensor calibration profiles (named: cheap GPS, RTK GPS, noisy IMU, LiDAR A3, multispectral camera) | missing | 4 | Load a named profile by key; profile is reproducible and locked to seed |
| Mission validation report (coverage, duration, battery margin, terrain gaps, safety risks, blocked waypoints) | missing | 5 | Report produced before each simulated run; run blocked when terrain gaps exceed threshold |
| Deterministic golden + manifest regression (single runner: same-seed byte-identity and manifest hash reproducibility across builds/platforms) | early-implemented (byte-identity and manifest hashing in `flight_sim_cpp`; cross-platform gate pending) | 4 | Same mission and seed reproduce a byte-identical trace and matching manifest hash across builds; divergence names the field |

### Physics, Sensors, and Terrain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Per-drone physics (gravity/drag/thrust/battery) | strong partial | 8 | Golden-telemetry regression fixtures for the physics loop |
| Sensor suite (GPS/IMU/baro/mag) | medium partial | 7 | Inject configurable noise and emit calibrated readings |
| PID flight controller and command modes | strong partial | 7 | Deterministic takeoff/land/goto/orbit golden traces |
| Status state machine and event broadcast | strong partial | 6 | Assert lifecycle transitions and emergency events |
| Wind and aerodynamic disturbance | missing | 6 | Add a wind field and integrate force into physics |
| LiDAR sensor simulation | missing (not implemented on the canonical C++ path) | 8 | Raycast point cloud into capture-shaped output for `04` |
| Camera / multispectral simulation | early partial | 6 | Emit georeferenced band images for `04`/`05` |
| Globe navigation and flight UI (`flight_sim_cpp`) | strong partial | 7 | Mission preview overlay tied to a field boundary |
| OSM map-tile and terrain loading | medium partial | 8 | Real DEM elevation with CRS/extent assertions |
| Earth textures and 3D terrain rendering | early partial (procedural fallback textures) | 6 | Replace procedural textures with georeferenced tiles |
| C++ headless runner and telemetry replay | medium partial (deterministic runner landed) | 6 | One mission/telemetry contract (TwinContractV1) on the single canonical runner |
| Twin-as-backend for flight/coordination | early partial | 7 | Drive `01`/`03` simulation mode through one twin API |
| Georeferenced 3D scene synthesis (buildings, farm vegetation) | missing | 6 | Seeded scene manifest from OSM footprints + land-cover classes |
| Ray-traced drone camera (FOV/intrinsics) | missing | 5 | Reproducible frame + depth from drone pose over a known scene |
| Telemetry + video streaming to external collector | missing | 4 | Encoded video and `shared` telemetry into a local collector fixture |
| Labeled synthetic dataset export | missing | 3 | Frames + class masks/depth/poses derived from the scene manifest |
