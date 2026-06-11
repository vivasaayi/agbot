# Simulation and Digital Twin: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: simulator engine call or headless command, deterministic seed, and replayable output.
- Safety: the twin enforces the same geofence/altitude/battery limits the real path will, so sim-first testing is meaningful.
- Deterministic: physics, sensor, and terrain math that runs reproducibly without AI, with golden fixtures.
- Telemetry: position/sensor/battery/status event stream matching the `shared` schemas used by `01`.
- UI: globe/3D viewer, HUD, and mission preview (Bevy in-app, OpenGL standalone).
- Tests: unit (physics/sensor/terrain math), fixture (mission JSON, golden telemetry), and one failure path (out-of-bounds / low battery).
- Operations: runtime mode (`Simulation`), reproducible seeds, and a runbook for the canonical twin.

## Category Epics

### EPIC-01: Deterministic Physics and Sensor Twin
- Goal: a reproducible per-drone twin whose telemetry can regression-test flight (`01`) and coordination (`03`).
- First release: golden-telemetry fixtures for takeoff/land/goto/orbit and a noise-injecting sensor suite.
- Expansion: wind and aerodynamic disturbance integrated into the physics loop.
- Hardening: thermal/battery refinement, seed control, and CI gating.

### EPIC-02: Georeferenced Terrain and Sensor Simulation
- Goal: a twin world that matches the real field with capture-shaped sensor outputs.
- First release: real DEM terrain with CRS/extent assertions, replacing placeholder textures and flat tiles.
- Expansion: LiDAR raycast point-cloud sim and camera/multispectral sim feeding domain `04`.
- Hardening: tile streaming/LOD performance and large-area coverage on edge hardware.

### EPIC-03: Canonical Twin and Mission Preview
- Goal: one canonical simulator backing the in-app and standalone surfaces with a shared contract.
- First release: resolve Rust/Bevy vs C++ roles and a shared mission/telemetry format.
- Expansion: mission preview overlay tied to a field boundary, driving `01`/`03` simulation mode.
- Hardening: replay, audit, and export of simulated sessions for after-action review.
