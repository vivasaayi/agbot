# Simulation and Digital Twin: Current State and Target State

## Mission

Provide a trustworthy digital twin of the drone, its sensors, and the terrain so that flight (`01`), multi-drone coordination (`03`), and capture (`04`) can be planned, previewed, and regression-tested deterministically before any hardware flight.

## Current Maturity

medium partial: `flight_sim_cpp` (C++20) is the single canonical simulator for both interactive and headless surfaces. It has a fixed-timestep follower with mission load/edit, telemetry record/replay, OSM/Terrarium terrain, a globe picker, a macOS OpenGL viewer, a deterministic headless runner (`DeterministicRunner`) that emits byte-identical traces plus a per-run manifest with a committed golden fixture, an initial safety parity harness, terrain tile state tagging, and a first trace diff CLI. The Rust/Bevy `simulator` crate and the Rust `drone_simulator` crate were both retired after the C++ path won the canonical-simulator decision; there is one runner, not two. Wind/aerodynamics, the LiDAR sim, scene synthesis, and the ray-traced camera are missing.

## What Exists Now

- C++20 `DroneSimulation` with fixed-timestep `step()`, autopilot/manual/replay control modes, a per-drone physics loop (gravity/drag/thrust/angular damping) and battery drain, `MissionLoader` (local + lat/lon JSON), `TelemetryRecorder`/`TelemetryReplay` (JSONL), `GeoTerrain` (OSM/Terrarium), and a headless CLI (`flight_sim_cpp/src/DroneSimulation.cpp`, `MissionLoader.cpp`, `TelemetryRecorder.cpp`, `headless_main.cpp`, `GeoTerrain.cpp`).
- Deterministic headless runner: `run_deterministic(mission, RunConfig{seed, timestep_s, record_interval_s, max_time_s})` returns a byte-identical JSONL trace plus a `RunManifest` (simulator_version, contract_version `1.0.0` seeding TwinContractV1, seed, timestep, mission_hash, output_hash via FNV-1a, prng_nonce, completed). The `agbot_flight_sim_headless` CLI now requires `--seed`, accepts `--timestep-ms`, and writes `<output>.manifest.json` next to the trace (`flight_sim_cpp/include/agbot_flight_sim/DeterministicRunner.hpp`, `src/DeterministicRunner.cpp`).
- Safety parity first slice: `SafetyRules` defines geofence, altitude ceiling, no-fly-zone, low-battery, and emergency-abort violation codes; `DroneSimulation` records the last violation and fails safe when the envelope is breached. The test harness verifies required rule coverage (`flight_sim_cpp/include/agbot_flight_sim/SafetyRules.hpp`, `src/SafetyRules.cpp`).
- Terrain no-data first slice: `TerrainTileState`/`TerrainTileStatus` tags available and `flat_fallback` elevation composites so a missing DEM tile is no longer represented as silent zero elevation (`flight_sim_cpp/include/agbot_flight_sim/GeoTerrain.hpp`, `src/GeoTerrain.cpp`).
- Trace diff first slice: `agbot-sim diff <trace-a.jsonl> <trace-b.jsonl>` reports the first divergent telemetry step and field path (`flight_sim_cpp/include/agbot_flight_sim/TraceDiff.hpp`, `src/TraceDiff.cpp`, `src/agbot_sim_main.cpp`).
- Tests cover byte-identity, seed-drives-PRNG, manifest contents, and a committed golden fixture `tests/golden/unit_mission.jsonl` (`flight_sim_cpp/tests/simulation_tests.cpp`).
- macOS OpenGL viewer with globe picking (Mercator, up to z17), chase/orbit cameras, 2D/3D terrain views, a telemetry side panel, mission editing, and replay scrubbing (`flight_sim_cpp/src/macos_opengl_viewer.mm`).

## Gaps to Close

### Reliability backbone (partial — must land before synthetic-perception work is meaningful)

- `TwinContractV1` PARTIALLY SEEDED: the deterministic runner emits a `contract_version` of `1.0.0` as the seed of TwinContractV1, but there is not yet a full versioned wire contract for commands, telemetry, trace files, scenario manifests, errors, or declared simulator capabilities. Broader contract drift is still invisible.
- Deterministic runner mode PARTIALLY CLOSED: `flight_sim_cpp` now has a `--seed` / `--timestep-ms` headless runner that produces byte-identical output across runs (verified by tests), refuses to start without `--seed`, and logs seed/timestep/version in the run header. Golden fixtures are now reproducible on this foundation; cross-platform byte-identity verification is still pending.
- Safety parity harness PARTIALLY CLOSED: the C++ twin now has shared safety violation codes, a coverage harness for geofence/altitude/battery/no-fly-zone/emergency-abort rules, and simulation failsafe integration. Still pending: parity against the authoritative `01`/`03` safety rule source and mission-dispatch wiring.
- Terrain no-data model PARTIALLY CLOSED: elevation composites can now emit explicit tile states and mark missing tiles as `flat_fallback`. Still pending: stale/synthetic/missing state propagation from actual tile fetch/cache outcomes and manifest serialization.
- Scenario manifest PARTIALLY CLOSED: the headless runner now emits a per-run `<output>.manifest.json` (`RunManifest`: simulator version, contract version, seed, timestep, mission_hash, output_hash, prng_nonce, completed). Still missing: terrain tile states/hashes, weather config, sensor configs, safety config, and a full versioned manifest schema with consumer round-trip.
- Trace diff CLI PARTIALLY CLOSED: `agbot-sim diff` now reports the first divergent telemetry step and field. Still pending: tolerance flags, multi-diff JSON output, and incompatible contract-version handling.
- No fault injection library: there is no seeded, reproducible mechanism to inject wind gusts, GPS drift, IMU noise, sensor dropout, comm loss, low battery, stale terrain, bad tile, or actuator lag as named, reproducible fault classes.
- No simulation health/operability: no health-check endpoint or CLI, no per-run seed/version log header, no trace retention policy, no tile cache controls, and no runbook.

### Physics, sensor, and terrain gaps

- No wind or aerodynamic disturbance modeling; `set_wind` exists in C++ but the force integration is thin.
- No LiDAR raycast point-cloud generation (the Bevy-era stub was removed with the `simulator` crate).
- No 3D scene objects: terrain renders bare — no buildings from OSM footprints and no farm vegetation (trees, bushes, crop rows) from land-cover classes.
- No simulated drone camera with FOV/intrinsics, no ray-traced frame capture, and no video encoding/streaming to an external telemetry collector.
- `flight_sim_cpp` has deterministic-runner, byte-identity, and manifest tests plus one golden fixture, but broader physics/sensor/terrain test coverage is still thin.
- Golden-telemetry regression PARTIALLY CLOSED: a deterministic golden fixture (`tests/golden/unit_mission.jsonl`) now exists; per-command-mode and per-sensor golden coverage is still pending.
- No capture replay adapter routing simulated sensor output through the exact domain `04` ingestion path; the path currently bypasses real ingestion handlers.
- No named sensor calibration profiles; noise constants are buried in ad-hoc config.
- No mission validation report produced before a simulated run.
- Single-runner deterministic regression PARTIALLY CLOSED: same-seed byte-identity, per-run manifest hashing, and a first trace diff CLI now exist on `flight_sim_cpp`; cross-build/cross-platform determinism verification and CI gating are still pending.

## Source Modules Reviewed

- `flight_sim_cpp/src/DroneSimulation.cpp`, `MissionLoader.cpp`, `TelemetryRecorder.cpp`, `TelemetryReplay.cpp`, `GeoTerrain.cpp`, `headless_main.cpp`, `macos_opengl_viewer.mm`, `flight_sim_cpp/tests/simulation_tests.cpp`
- `flight_sim_cpp/include/agbot_flight_sim/DeterministicRunner.hpp`, `SafetyRules.hpp`, `TraceDiff.hpp`, `flight_sim_cpp/src/DeterministicRunner.cpp`, `SafetyRules.cpp`, `TraceDiff.cpp`, `flight_sim_cpp/tests/golden/unit_mission.jsonl`
- (retired) the Rust `drone_simulator` crate — its physics/sensor/controller loop was superseded by `flight_sim_cpp` and the crate was removed from the workspace
- (retired) the Rust/Bevy `simulator` crate — globe view, HUD, map loader, and LiDAR stub were superseded by `flight_sim_cpp`

## Target Operating Model

**Reliability backbone (must come first):**
- `TwinContractV1`: a versioned wire contract (commands, telemetry, trace files, scenario manifests, errors, capabilities) that every consumer validates at the boundary.
- Deterministic runner mode: fixed timestep, seeded PRNG, byte-identical output across runs and platforms.
- Safety parity harness: CI-enforced proof that the twin and real path enforce identical geofence, altitude, battery, no-fly-zone, and abort rules.
- Terrain no-data model: explicit `available`/`missing`/`stale`/`synthetic`/`flat_fallback` state tags on every elevation sample; missing tiles never silently become flat zero.
- Scenario manifest: every run emits a manifest recording simulator version, seed, mission, terrain tile states and hashes, weather config, sensor configs, safety config, and output hashes.
- Trace diff CLI: `agbot-sim diff` names the first divergent step index and field, making golden failures actionable.
- Fault injection library: seeded, reproducible fault classes (wind gusts, GPS drift, IMU noise, sensor dropout, comm loss, low battery, stale terrain, bad tile, actuator lag) with CI regression outcomes.
- Simulation health/operability: structured health checks, per-run seed/version logging, trace retention policy, tile cache controls, and a published runbook.

**Physics, capture, and sensor fidelity:**
- One canonical twin with a stable interface contract, mirroring the flight (`01`) and coordination (`03`) command/telemetry schemas from `shared`.
- Physically plausible physics: wind and aerodynamic disturbance, sensor noise injection, and battery/thermal models good enough for planning.
- Named sensor calibration profiles (cheap GPS, RTK GPS, noisy IMU, LiDAR A3, multispectral camera) so tests are keyed to a sensor model, not a magic constant.
- A capture replay adapter routing simulated sensor output through the exact domain `04` ingestion path.
- A mission validation report produced before each run: coverage, duration, battery margin, terrain gaps, safety risks, blocked waypoints.
- Single-runner deterministic regression: same-seed byte-identity plus scenario-manifest hash reproducibility on `flight_sim_cpp`, verified across builds and platforms in CI.

**Terrain and perception:**
- Real georeferenced terrain (DEM tiles + textures) so mission preview matches the field; CRS/extent preserved.
- A working LiDAR sim (raycast point cloud) and camera sim that emit capture-shaped data into domain `04`.
- A synthetic world for sim-first perception: georeferenced scene objects (OSM buildings, land-cover-driven farm vegetation), a ray-traced drone camera with configurable FOV, telemetry + encoded video streamed to an external collector, and labeled dataset export for `05`/`23` model training.
- Deterministic golden-telemetry regression fixtures run in CI so flight/coordination changes are caught before hardware.
- Resolved canonical role: `flight_sim_cpp` is the single canonical simulator for both the interactive (OpenGL viewer) and headless (deterministic CI regression) surfaces — one runner, one mission/telemetry format. The Rust/Bevy `simulator` and Rust `drone_simulator` crates were both retired.
