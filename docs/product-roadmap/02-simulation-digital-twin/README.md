# Simulation and Digital Twin

Simulate drone physics, sensors, and terrain so flight, coordination, and capture can be planned, previewed, and regression-tested before touching hardware.

## Where We Are

- `drone_simulator` has a real per-drone physics loop (gravity/drag/thrust/battery), a sensor suite, a PID flight controller, and a status state machine with event broadcast.
- `simulator` (Bevy) has an interactive globe with picking/search, a flight UI state machine, chase/orbit cameras, HUD telemetry, and OSM map-tile plus elevation terrain loading.
- `flight_sim_cpp` (C++20) has a fixed-timestep waypoint follower, mission JSON loader, JSONL telemetry recorder/replay, OSM/Terrarium terrain, and a headless runner plus a macOS OpenGL viewer.
- Wind/aerodynamics, full 3D terrain, and the LiDAR sim are missing or stubbed, and there is no test coverage in `simulator`.

## Where We Should Be

- One canonical digital twin that mirrors flight (`01`), coordination (`03`), and capture (`04`) deterministically and is exercised in CI.
- Physically plausible physics with wind/aerodynamic disturbance, sensor noise models, and a working LiDAR/camera sim that feeds `04`/`06`.
- Real georeferenced terrain (DEM + textures) so mission preview matches the field, with a clear Rust-vs-C++ canonical role decided.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Pick the canonical simulator (Rust/Bevy vs C++) and define the twin's interface contract.
2. Add deterministic physics regression fixtures (takeoff/land/goto/orbit) with golden telemetry.
3. Add wind/aerodynamic disturbance and sensor noise to the physics/sensor loop.
4. Replace placeholder Earth textures and flat terrain with real georeferenced DEM tiles.
5. Implement the LiDAR sim (raycast point cloud) and camera sim so capture (`04`) has inputs.
6. Wire the twin as the simulation-mode backend for flight (`01`) and coordination (`03`).

## Primary Crates

`drone_simulator` (physics/sensor twin), `simulator` (Bevy in-app viewer), `flight_sim_cpp` (standalone headless/OpenGL), with `shared` for schemas. Consumed by domains `01`, `03`, and `04` in simulation-first mode.
