# Simulation and Digital Twin: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: simulator engine call or headless command, deterministic seed, and replayable output.
- Safety: the twin enforces the same geofence/altitude/battery limits the real path will, so sim-first testing is meaningful.
- Deterministic: physics, sensor, and terrain math that runs reproducibly without AI, with golden fixtures.
- Telemetry: position/sensor/battery/status event stream matching the `shared` schemas used by `01`.
- UI: globe/3D viewer, HUD, and mission preview (`flight_sim_cpp` OpenGL standalone viewer).
- Tests: unit (physics/sensor/terrain math), fixture (mission JSON, golden telemetry), and one failure path (out-of-bounds / low battery).
- Operations: runtime mode (`Simulation`), reproducible seeds, and a runbook for the canonical twin.

## Category Epics

### EPIC-00: Twin Reliability Backbone
- Goal: establish the non-negotiable reliability infrastructure that makes simulation-first testing trustworthy. Without this backbone, golden fixtures, safety parity claims, and regression results are not meaningful.
- First release: `TwinContractV1` (versioned wire format for commands, telemetry, trace files, scenario manifests, errors, and simulator capabilities), deterministic runner mode (fixed timestep, seeded PRNG, byte-identical output), and the safety parity harness (CI test proving geofence/altitude/battery/no-fly-zone/abort enforcement matches the real path).
- Expansion: terrain no-data model (`available`/`missing`/`stale`/`synthetic`/`flat_fallback`), scenario manifest per run, trace diff CLI, fault injection library (seeded wind gusts/GPS drift/IMU noise/sensor dropout/comm loss/low battery/bad tile/actuator lag), and simulation health/operability (health checks, seed logging, trace retention, cache controls, runbook).
- Hardening: single-runner deterministic regression (same-seed byte-identity plus scenario-manifest hash reproducibility across builds/platforms on `flight_sim_cpp`), sensor calibration profiles, capture replay adapter, and mission validation reports.

### EPIC-01: Deterministic Physics and Sensor Twin
- Goal: a reproducible per-drone twin whose telemetry can regression-test flight (`01`) and coordination (`03`).
- First release: golden-telemetry fixtures for takeoff/land/goto/orbit (an initial deterministic golden fixture `tests/golden/unit_mission.jsonl` already exists on `flight_sim_cpp`) and a noise-injecting sensor suite.
- Expansion: wind and aerodynamic disturbance integrated into the physics loop.
- Hardening: thermal/battery refinement, seed control, and CI gating.

### EPIC-02: Georeferenced Terrain and Sensor Simulation
- Goal: a twin world that matches the real field with capture-shaped sensor outputs.
- First release: real DEM terrain with CRS/extent assertions, replacing procedural fallback textures and unannotated flat tiles.
- Expansion: LiDAR raycast point-cloud sim and camera/multispectral sim feeding domain `04`.
- Hardening: tile streaming/LOD performance and large-area coverage on edge hardware.

### EPIC-03: Canonical Twin and Mission Preview
- Goal: one canonical simulator backing both the interactive and headless surfaces with a shared contract. Resolved: `flight_sim_cpp` is the single canonical simulator for both surfaces (the Bevy `simulator` crate and the Rust `drone_simulator` crate were both retired in its favor).
- First release: one mission/telemetry format (TwinContractV1) on the single `flight_sim_cpp` runner, exercised in both interactive and headless modes.
- Expansion: mission preview overlay tied to a field boundary, driving `01`/`03` simulation mode; location-anchored scenario loading from globe navigation.
- Hardening: replay, audit, and export of simulated sessions for after-action review.

### EPIC-04: Synthetic Perception and World Rendering
- Goal: a flight-simulator-grade synthetic world — real terrain populated with georeferenced buildings and farm vegetation — observed through a ray-traced drone camera, so most perception/integration issues are found in software before real-world flight.
- First release: seeded scene synthesis from OSM footprints and land-cover classes on DEM terrain, with a reproducible scene manifest.
- Expansion: ray-traced camera with configurable FOV/intrinsics; encoded video plus telemetry streamed to an external collector.
- Hardening: labeled synthetic dataset export (class masks, depth, poses) feeding vegetation-type classification in `05`/`23`; per-crop scene generators (cotton, rice, palm, forest, bush).
