# Simulation and Digital Twin: Current State and Target State

## Mission

Provide a trustworthy digital twin of the drone, its sensors, and the terrain so that flight (`01`), multi-drone coordination (`03`), and capture (`04`) can be planned, previewed, and regression-tested deterministically before any hardware flight.

## Current Maturity

medium partial: `drone_simulator` has a working per-drone physics and sensor loop with a PID controller; `flight_sim_cpp` (C++20, the canonical interactive simulator) has a fixed-timestep follower with mission load/edit, telemetry record/replay, OSM/Terrarium terrain, a globe picker, and a macOS OpenGL viewer. The Rust/Bevy `simulator` crate was removed after the C++ path won the canonical-simulator decision. Wind/aerodynamics, the LiDAR sim, scene synthesis, and the ray-traced camera are missing.

## What Exists Now

- Physics-driven `SimulationEngine` with per-drone `DronePhysics` applying gravity, drag, thrust, and angular damping, plus a battery drain model (`drone_simulator/src/lib.rs`, `physics.rs`).
- PID `FlightController` with takeoff/land/goto/hover/orbit/emergency/return-to-home commands and a `DroneStatus` state machine; `SimulationEvent` broadcast for position, sensor, battery, status, and emergency events (`drone_simulator/src/flight_controller.rs`, `lib.rs`).
- Sensor suite: GPS, IMU, barometer, magnetometer, with optional camera/LiDAR/multispectral sensors emitting `SensorReading`s (`drone_simulator/src/sensors.rs`).
- C++20 `DroneSimulation` with fixed-timestep `step()`, autopilot/manual/replay control modes, `MissionLoader` (local + lat/lon JSON), `TelemetryRecorder`/`TelemetryReplay` (JSONL), `GeoTerrain` (OSM/Terrarium), and a headless CLI (`flight_sim_cpp/src/DroneSimulation.cpp`, `MissionLoader.cpp`, `TelemetryRecorder.cpp`, `headless_main.cpp`, `GeoTerrain.cpp`).
- macOS OpenGL viewer with globe picking (Mercator, up to z17), chase/orbit cameras, 2D/3D terrain views, a telemetry side panel, mission editing, and replay scrubbing (`flight_sim_cpp/src/macos_opengl_viewer.mm`).

## Gaps to Close

- No wind or aerodynamic disturbance modeling; `set_wind` exists in C++ but the force integration is thin.
- No LiDAR raycast point-cloud generation (the Bevy-era stub was removed with the `simulator` crate).
- No 3D scene objects: terrain renders bare — no buildings from OSM footprints and no farm vegetation (trees, bushes, crop rows) from land-cover classes.
- No simulated drone camera with FOV/intrinsics, no ray-traced frame capture, and no video encoding/streaming to an external telemetry collector.
- `drone_simulator` and `flight_sim_cpp` have only a few smoke tests.
- No deterministic golden-telemetry regression fixtures to guard flight/coordination behavior.

## Source Modules Reviewed

- `drone_simulator/src/lib.rs`, `physics.rs`, `flight_controller.rs`, `sensors.rs`, `communication.rs`, `environment.rs`
- `flight_sim_cpp/src/DroneSimulation.cpp`, `MissionLoader.cpp`, `TelemetryRecorder.cpp`, `TelemetryReplay.cpp`, `GeoTerrain.cpp`, `headless_main.cpp`, `macos_opengl_viewer.mm`, `flight_sim_cpp/tests/simulation_tests.cpp`
- (removed) the Rust/Bevy `simulator` crate — globe view, HUD, map loader, and LiDAR stub were superseded by `flight_sim_cpp`

## Target Operating Model

- One canonical twin with a stable interface contract, mirroring the flight (`01`) and coordination (`03`) command/telemetry schemas from `shared`.
- Physically plausible physics: wind and aerodynamic disturbance, sensor noise injection, and battery/thermal models good enough for planning.
- Real georeferenced terrain (DEM tiles + textures) so mission preview matches the field; CRS/extent preserved.
- A working LiDAR sim (raycast point cloud) and camera sim that emit capture-shaped data into domain `04`.
- A synthetic world for sim-first perception: georeferenced scene objects (OSM buildings, land-cover-driven farm vegetation), a ray-traced drone camera with configurable FOV, telemetry + encoded video streamed to an external collector, and labeled dataset export for `05`/`23` model training.
- Deterministic golden-telemetry regression fixtures run in CI so flight/coordination changes are caught before hardware.
- Resolved canonical roles: `flight_sim_cpp` standalone headless/OpenGL is the canonical interactive simulator; `drone_simulator` is the headless Rust twin; both share mission and telemetry formats.
