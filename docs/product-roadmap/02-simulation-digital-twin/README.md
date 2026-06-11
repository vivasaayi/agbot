# Simulation and Digital Twin

Simulate drone physics, sensors, and terrain so flight, coordination, and capture can be planned, previewed, and regression-tested before touching hardware.

## Where We Are

- `drone_simulator` has a real per-drone physics loop (gravity/drag/thrust/battery), a sensor suite, a PID flight controller, and a status state machine with event broadcast.
- `flight_sim_cpp` (C++20) is the canonical interactive simulator: fixed-timestep waypoint follower, mission JSON loader/editor, JSONL telemetry recorder/replay, OSM/Terrarium terrain, globe picker, and a headless runner plus a macOS OpenGL viewer. The Rust/Bevy `simulator` crate was removed in its favor.
- Wind/aerodynamics, the LiDAR sim, scene synthesis, and the ray-traced camera are missing or stubbed.

## Where We Should Be

- One canonical digital twin that mirrors flight (`01`), coordination (`03`), and capture (`04`) deterministically and is exercised in CI.
- Physically plausible physics with wind/aerodynamic disturbance, sensor noise models, and a working LiDAR/camera sim that feeds `04`/`06`.
- Real georeferenced terrain (DEM + textures) so mission preview matches the field.
- A flight-simulator-grade synthetic world: navigate the globe to any location, populate it with georeferenced scene objects (buildings from OSM; forest trees, bushes, and crop rows from land-cover classes), fly the drone there, ray-trace its camera view, and stream telemetry plus encoded video to an external collector — so the majority of integration issues are sorted in software before real-world flight.
- Labeled synthetic dataset export (class masks, depth, poses) so vegetation/crop perception models in `05`/`23` can be trained sim-first, autonomous-vehicle-style.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Define the twin's interface contract across `flight_sim_cpp` (canonical interactive) and `drone_simulator` (headless Rust twin).
2. Add deterministic physics regression fixtures (takeoff/land/goto/orbit) with golden telemetry.
3. Add wind/aerodynamic disturbance and sensor noise to the physics/sensor loop.
4. Replace placeholder Earth textures and flat terrain with real georeferenced DEM tiles.
5. Implement the LiDAR sim (raycast point cloud) and camera sim so capture (`04`) has inputs.
6. Synthesize georeferenced scene objects (buildings, farm vegetation) and the ray-traced drone camera; stream telemetry + video to an external collector.
7. Wire the twin as the simulation-mode backend for flight (`01`) and coordination (`03`).
8. Export labeled synthetic datasets to train vegetation/crop perception (`05`/`23`).

## Primary Crates

`drone_simulator` (headless physics/sensor twin), `flight_sim_cpp` (canonical interactive simulator: headless runner + OpenGL viewer), with `shared` for schemas. Consumed by domains `01`, `03`, and `04` in simulation-first mode.
