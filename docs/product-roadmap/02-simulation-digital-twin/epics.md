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
- Goal: one canonical simulator backing the interactive and headless surfaces with a shared contract. Resolved: `flight_sim_cpp` is the canonical interactive simulator, `drone_simulator` the headless Rust twin (the Bevy `simulator` crate was removed).
- First release: shared mission/telemetry format across `flight_sim_cpp` and `drone_simulator`.
- Expansion: mission preview overlay tied to a field boundary, driving `01`/`03` simulation mode; location-anchored scenario loading from globe navigation.
- Hardening: replay, audit, and export of simulated sessions for after-action review.

### EPIC-04: Synthetic Perception and World Rendering
- Goal: a flight-simulator-grade synthetic world — real terrain populated with georeferenced buildings and farm vegetation — observed through a ray-traced drone camera, so most perception/integration issues are found in software before real-world flight.
- First release: seeded scene synthesis from OSM footprints and land-cover classes on DEM terrain, with a reproducible scene manifest.
- Expansion: ray-traced camera with configurable FOV/intrinsics; encoded video plus telemetry streamed to an external collector.
- Hardening: labeled synthetic dataset export (class masks, depth, poses) feeding vegetation-type classification in `05`/`23`; per-crop scene generators (cotton, rice, palm, forest, bush).
