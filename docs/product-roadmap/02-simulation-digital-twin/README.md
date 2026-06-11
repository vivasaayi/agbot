# Simulation and Digital Twin

Simulate drone physics, sensors, and terrain so flight, coordination, and capture can be planned, previewed, and regression-tested before touching hardware.

## Where We Are

- `flight_sim_cpp` (C++20) is the single canonical simulator for both the interactive viewer and headless CI regression: a real per-drone physics loop (gravity/drag/thrust), fixed-timestep waypoint follower, mission JSON loader/editor, JSONL telemetry recorder/replay, OSM/Terrarium terrain, globe picker, a macOS OpenGL viewer, and a deterministic headless runner (`DeterministicRunner`) that emits byte-identical traces plus a per-run manifest.
- The Rust/Bevy `simulator` crate and, more recently, the Rust `drone_simulator` crate were both retired in favor of `flight_sim_cpp`; there is now one runner, not two.
- Wind/aerodynamics, the LiDAR sim, scene synthesis, and the ray-traced camera remain unimplemented or thinly scaffolded on the canonical C++ path.

## Where We Should Be

**Reliability backbone (must land before the synthetic-perception stack is meaningful):**

- A versioned `TwinContractV1` that defines the wire format for commands, telemetry, trace files, scenario manifests, errors, and declared simulator capabilities — so every consumer knows exactly what the twin promises and when it breaks the contract.
- A deterministic runner mode: fixed timestep, seeded PRNG, deterministic timestamps and IDs, and repeatable byte-identical output across runs and platforms.
- A safety parity harness that proves the twin enforces the same geofence, altitude, battery, no-fly-zone, and abort rules as the real flight path — failing loudly when a gap exists.
- A terrain no-data model with explicit states (`available`, `missing`, `stale`, `synthetic`, `flat_fallback`) so missing DEM tiles never silently become flat zero elevation.
- A scenario manifest emitted by every run: simulator version, seed, mission, terrain tiles used, weather config, sensor configs, safety config, source data hashes, and output hashes.
- A trace diff CLI that compares two simulation traces and reports exact divergent steps and fields — without it, golden regression is painfully opaque.
- A fault injection library: seeded wind gusts, GPS drift, IMU noise, sensor dropout, comm loss, low battery, stale terrain, bad tile, and actuator lag — all reproducible by seed.
- Simulation health and operability: headless API/CLI health checks, run status reporting, deterministic seed logging, trace retention policy, tile cache controls, and a runbook.

**Capture and sensor fidelity (should land before scene synthesis is wired end-to-end):**

- A capture replay adapter routing simulated LiDAR/camera/multispectral output through the exact domain `04` capture ingestion path.
- Named sensor calibration profiles (cheap GPS, RTK GPS, noisy IMU, LiDAR A3, multispectral camera) so tests are reproducibly keyed to a sensor model, not a magic noise constant.
- A mission validation report produced before each simulated run: expected coverage, flight duration, battery margin, terrain data gaps, safety risks, and blocked waypoints.
- Single-runner deterministic regression: same-seed byte-identical golden traces plus scenario-manifest hash reproducibility across builds and platforms on `flight_sim_cpp`.

**Synthetic-perception stack (builds on the reliability backbone above):**

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

1. Define and version `TwinContractV1`: commands, telemetry, trace files, scenario manifests, error codes, and declared simulator capabilities. This gates everything else.
2. Lock deterministic runner mode: fixed timestep, seeded PRNG, deterministic timestamps/IDs, byte-identical output. Without this, golden regression is unreliable.
3. Add physics golden-telemetry regression fixtures (takeoff/land/goto/orbit) and flight-controller golden traces — now on a deterministic foundation.
4. Land the safety parity harness: geofence, altitude, battery, no-fly-zone, and abort rules enforced identically in the twin and the real path, with a CI test that fails on any gap.
5. Implement the terrain no-data model (available/missing/stale/synthetic/flat_fallback) and real georeferenced DEM tiles with CRS/extent assertions. Missing tiles must never silently become flat zero.
6. Wire scenario manifest emission for every run and add the trace diff CLI.
7. Add the fault injection library (seeded wind gusts, GPS drift, IMU noise, sensor dropout, comm loss, low battery, bad tile, actuator lag) and simulation health/operability checks.
8. Add capture replay adapter (sim sensor output → domain `04` ingestion path), sensor calibration profiles, mission validation report, and single-runner deterministic regression (golden + manifest hash reproducibility across builds/platforms).
9. Add wind/aerodynamic disturbance and sensor noise to the physics/sensor loop.
10. Replace procedural fallback Earth textures with georeferenced DEM + map-tile terrain.
11. Implement the LiDAR sim (raycast point cloud) and camera sim so capture (`04`) has inputs.
12. Synthesize georeferenced scene objects (buildings, farm vegetation) and the ray-traced drone camera; stream telemetry + video to an external collector.
13. Wire the twin as the simulation-mode backend for flight (`01`) and coordination (`03`).
14. Export labeled synthetic datasets to train vegetation/crop perception (`05`/`23`).

## Primary Crates

`flight_sim_cpp` (the single canonical simulator: deterministic headless runner + OpenGL viewer), with `shared` for schemas. Consumed by domains `01`, `03`, and `04` in simulation-first mode.
