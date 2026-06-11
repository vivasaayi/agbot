# Simulation and Digital Twin: Current State and Target State

## Mission

Provide a trustworthy digital twin of the drone, its sensors, and the terrain so that flight (`01`), multi-drone coordination (`03`), and capture (`04`) can be planned, previewed, and regression-tested deterministically before any hardware flight.

## Current Maturity

medium partial: `drone_simulator` has a working per-drone physics and sensor loop with a PID controller; `simulator` (Bevy) has an interactive globe, flight UI, cameras, HUD, and OSM/elevation terrain; `flight_sim_cpp` (C++20) has a headless fixed-timestep follower with mission load, telemetry record/replay, and terrain. Wind/aerodynamics, full 3D terrain, the LiDAR sim, and `simulator` test coverage are missing.

## What Exists Now

- Physics-driven `SimulationEngine` with per-drone `DronePhysics` applying gravity, drag, thrust, and angular damping, plus a battery drain model (`drone_simulator/src/lib.rs`, `physics.rs`).
- PID `FlightController` with takeoff/land/goto/hover/orbit/emergency/return-to-home commands and a `DroneStatus` state machine; `SimulationEvent` broadcast for position, sensor, battery, status, and emergency events (`drone_simulator/src/flight_controller.rs`, `lib.rs`).
- Sensor suite: GPS, IMU, barometer, magnetometer, with optional camera/LiDAR/multispectral sensors emitting `SensorReading`s (`drone_simulator/src/sensors.rs`).
- Bevy globe view with mouse picking, zoom, search-driven animation, a flight UI state machine, chase/orbit cameras, and a HUD (compass/speed/altitude/battery) (`simulator/src/globe_view.rs`, `globe_ui.rs`, `hud.rs`).
- OSM Overpass map-tile loader and terrain tile grid with elevation hooks, plus city search and a LiDAR control panel (`simulator/src/map_loader.rs`, `terrain.rs`, `osm.rs`, `lidar_controls.rs`, `city_search/`).
- C++20 `DroneSimulation` with fixed-timestep `step()`, autopilot/manual/replay control modes, `MissionLoader` (local + lat/lon JSON), `TelemetryRecorder`/`TelemetryReplay` (JSONL), `GeoTerrain` (OSM/Terrarium), and a headless CLI (`flight_sim_cpp/src/DroneSimulation.cpp`, `MissionLoader.cpp`, `TelemetryRecorder.cpp`, `headless_main.cpp`, `GeoTerrain.cpp`).

## Gaps to Close

- No wind or aerodynamic disturbance modeling; `set_wind` exists in C++ but the force integration is thin.
- Earth textures are procedural sine-based placeholders; full 3D terrain elevation is in progress and largely flat in the Bevy path.
- The Bevy LiDAR sim is effectively a one-line TODO (`simulator/src/lidar_simulator.rs` has an empty `build()`); no raycast point-cloud generation.
- No automated test coverage in `simulator`; `drone_simulator` and `flight_sim_cpp` have only a few smoke tests.
- Two simulators (Rust/Bevy vs C++) with an unclear canonical role — open question carried from the rigor model.
- No deterministic golden-telemetry regression fixtures to guard flight/coordination behavior.

## Source Modules Reviewed

- `drone_simulator/src/lib.rs`, `physics.rs`, `flight_controller.rs`, `sensors.rs`, `communication.rs`, `environment.rs`
- `simulator/src/main.rs`, `globe_view.rs`, `globe_ui.rs`, `hud.rs`, `map_loader.rs`, `terrain.rs`, `osm.rs`, `lidar_simulator.rs`, `lidar_controls.rs`, `earth_textures.rs`, `procedural_textures.rs`, `autopilot/waypoint.rs`, `city_search/`
- `flight_sim_cpp/src/DroneSimulation.cpp`, `MissionLoader.cpp`, `TelemetryRecorder.cpp`, `TelemetryReplay.cpp`, `GeoTerrain.cpp`, `headless_main.cpp`, `flight_sim_cpp/tests/simulation_tests.cpp`

## Target Operating Model

- One canonical twin with a stable interface contract, mirroring the flight (`01`) and coordination (`03`) command/telemetry schemas from `shared`.
- Physically plausible physics: wind and aerodynamic disturbance, sensor noise injection, and battery/thermal models good enough for planning.
- Real georeferenced terrain (DEM tiles + textures) so mission preview matches the field; CRS/extent preserved.
- A working LiDAR sim (raycast point cloud) and camera sim that emit capture-shaped data into domain `04`.
- Deterministic golden-telemetry regression fixtures run in CI so flight/coordination changes are caught before hardware.
- Resolved Rust-vs-C++ roles: in-app Bevy viewer vs standalone headless/OpenGL, with shared mission and telemetry formats.
