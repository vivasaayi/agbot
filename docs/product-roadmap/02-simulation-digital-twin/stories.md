# Simulation and Digital Twin: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI (here, usually a seeded golden-telemetry fixture).
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `OPS` operator/pilot, `DSP` drone service provider, `AG` agronomist, `PA` platform admin.

The twin is the regression and planning surface for flight (`01`) and coordination (`03`) and the capture source for `04`. Every physics/controller P0 ships with a deterministic, seeded golden-telemetry fixture run in CI; the twin must enforce the same geofence/altitude/battery limits as the real path.

---

## M1 — Foundation

### STORY 02-01 · M1 · M · P0 — Golden-telemetry regression for the physics loop
- **Story**: As `DSP`, I want the per-drone physics loop pinned by a seeded golden-telemetry fixture, so that any change to gravity/drag/thrust/battery is caught before it reaches flight.
- **Deterministic / evidence**: run `SimulationEngine`/`DronePhysics` with a fixed seed and timestep; record a golden telemetry trace; CI fails on any deviation beyond **TELEM** tolerance.
- **Acceptance**:
  - Given a fixed seed and mission, when the physics loop runs, then the trace matches the committed golden fixture within **TELEM** tolerance, with byte identity required for deterministic-runner fields.
  - Given an unintended physics change, when CI runs, then the golden test fails and names the diverging field, not just "mismatch."
- **Tests**: golden-file (seeded trace), unit (physics integration step), failure path (perturbed constant fails golden).
- **Depends on**: `flight_sim_cpp/src/DroneSimulation.cpp`, `DeterministicRunner.cpp`.
- **Status**: initial implementation landed in `flight_sim_cpp` (DeterministicRunner + golden fixture `tests/golden/unit_mission.jsonl`, `agbot_flight_sim_headless --seed` required).

### STORY 02-02 · M1 · M · P0 — Deterministic flight-controller golden traces
- **Story**: As `OPS`, I want takeoff/land/goto/orbit/hover/RTH command modes pinned by golden traces, so that the PID controller behaves identically across builds.
- **Deterministic / evidence**: seed the `FlightController`; run each command mode to completion; record golden traces; assert the `DroneStatus` state machine transitions.
- **Acceptance**:
  - Given each command mode, when run seeded, then its trace and status transitions match the golden fixture.
  - Given a controller-gain change, when CI runs, then the affected mode's golden test fails with the diverging step identified.
- **Tests**: golden-file (per mode), unit (state machine transitions), failure path (gain change fails golden).
- **Depends on**: 02-01, `flight_sim_cpp/src/DroneSimulation.cpp` (autopilot/command modes).
- **Status**: initial implementation landed in `flight_sim_cpp` (DeterministicRunner + golden fixture `tests/golden/unit_mission.jsonl`, `agbot_flight_sim_headless --seed` required); per-command-mode golden coverage still pending.

### STORY 02-03 · M1 · S · P1 — Status state machine and event-broadcast assertions
- **Story**: As `DSP`, I want the status state machine and `SimulationEvent` broadcast pinned, so that downstream consumers can rely on event ordering and emergency signals.
- **Deterministic / evidence**: assert lifecycle transitions and that position/sensor/battery/status/emergency events fire in the expected order with correct payloads.
- **Acceptance**:
  - Given a full mission, when events are captured, then position/sensor/battery/status events fire in the documented order.
  - Given an emergency command, when triggered, then an emergency event is broadcast and no further normal events follow without a recovery transition.
- **Tests**: unit (transition + event ordering), failure path (emergency suppresses normal events), fixture.
- **Depends on**: 02-02.

### STORY 02-04 · M1 · S · P1 — Shared command/telemetry contract with `01`/`03`
- **Story**: As `PA`, I want the twin to speak the same command/telemetry schemas as the real flight path, so that simulation-first testing is actually representative.
- **Deterministic / evidence**: bind the twin's command/telemetry types to the `shared` schemas; a contract test asserts the twin accepts the same commands and emits the same `Telemetry` shape as `01`.
- **Acceptance**:
  - Given a `shared` command, when sent to the twin, then it is accepted and produces a `shared`-shaped telemetry sample.
  - Given a schema drift between twin and `shared`, when the contract test runs, then it fails rather than silently diverging.
- **Tests**: contract test (twin vs `shared`), failure path (schema drift detected).
- **Depends on**: 02-02, `shared/src/schemas.rs`, `01`, `03`.

---

## M2 — Captured / Observable

### STORY 02-05 · M3 · L · P0 — LiDAR raycast point-cloud simulation
- **Phase moved from M2 to M3**: raycast requires terrain geometry (02-09, M3). Moved here to resolve the phase-order contradiction where an M2 story depended on an M3 prerequisite.
- **Story**: As `AG`, I want the simulator to emit a real raycast point cloud, so that capture (`04`) and LiDAR mapping (`06`) can be developed and regression-tested without hardware.
- **Deterministic / evidence**: implement deterministic raycasting against terrain/obstacles in the canonical simulator (`flight_sim_cpp`, bridged capture-shaped to Rust; the Bevy-era `lidar_simulator.rs` stub was removed with the `simulator` crate), emitting `LidarScan`/`LidarPoint` consumable by `04`; seeded so output is reproducible.
- **Acceptance**:
  - Given a scene with known geometry and a seed, when the LiDAR sim runs, then it emits a reproducible point cloud whose ranges match the geometry within **CLOUD** tolerance.
  - Given a degenerate empty scene, when the sim runs, then it emits an empty-but-valid scan, not a panic or garbage points.
- **Tests**: unit (raycast ranges), golden-file (seeded cloud), failure path (empty scene).
- **Depends on**: 02-09 (terrain geometry), `04` (capture shape), `shared` LiDAR schema.

### STORY 02-06 · M3 · M · P1 — Camera / multispectral simulation emitting georeferenced bands
- **Phase moved from M2 to M3**: georeferenced band emission requires terrain tile extent (02-09, M3). Moved here to resolve the phase-order contradiction.
- **Story**: As `AG`, I want simulated multispectral band images tagged with georeference, so that imagery (`05`) processing can be tested against known inputs.
- **Deterministic / evidence**: emit RGB/NIR/Green/Blue band images with vegetation patterns and a georeference (CRS/extent/transform) tied to the camera pose; capture-shaped for `04`/`05`.
- **Acceptance**:
  - Given a camera pose over a field, when the sim captures, then band images carry a CRS/extent that round-trips a known ground coordinate.
  - Given a pose outside any terrain tile, when capture runs, then it reports "no coverage" rather than emitting an ungeoreferenced image.
- **Tests**: geospatial round-trip (CRS/extent), unit (band emission), failure path (no coverage).
- **Depends on**: 02-09, `04`, `05`.

### STORY 02-07 · M2 · S · P1 — Capture-shaped sensor stream into `04`
- **Story**: As `DSP`, I want simulated sensor output to flow into the capture session pipeline, so that the whole capture path is exercised end to end without flying.
- **Deterministic / evidence**: route 02-05/02-06 output through the same `04` `FlightDataRecord` ingestion with provenance (sensor/GPS/time) as the real readers.
- **Acceptance**:
  - Given a simulated flight, when sensors emit, then `04` persists provenance-complete records linked to the sim mission.
  - Given a sensor that fails mid-flight in sim, when capture runs, then `04` records a collection-failure, exercising the failure path.
- **Tests**: integration (sim → `04` records), failure path (simulated sensor failure recorded).
- **Depends on**: 02-05, 02-06, `04`.

---

## M3 — Explainable

### STORY 02-08 · M3 · S · P0 — Configurable sensor noise and calibration
- **Story**: As `DSP`, I want GPS/IMU/baro/mag readings to carry configurable, seeded noise and calibration, so that the twin tests downstream code against realistic, reproducible imperfection.
- **Deterministic / evidence**: inject seeded noise per sensor with documented distributions; emit calibrated `SensorReading`s; noise config is inspectable and reproducible.
- **Acceptance**:
  - Given a noise config and seed, when sensors run, then readings reproduce exactly and statistics match the configured distribution.
  - Given a zero-noise config, when sensors run, then readings are exactly the ideal values (no hidden noise).
- **Tests**: unit (noise distribution stats), golden-file (seeded readings), failure path (zero-noise is exact).
- **Depends on**: 02-01; sensor simulation is currently a `flight_sim_cpp` gap (no dedicated sensor-noise module yet).

### STORY 02-09 · M3 · M · P0 — Real DEM terrain with CRS/extent assertions
- **Story**: As `OPS`, I want real georeferenced DEM terrain loaded with asserted CRS/extent/resolution, so that mission preview matches the actual field.
- **Deterministic / evidence**: load DEM elevation tiles (OSM/Terrarium) into the terrain grid; assert CRS, extent, and resolution and round-trip a known coordinate; replace unannotated flat fallback elevation.
- **Acceptance**:
  - Given a field's DEM tiles, when terrain loads, then a known lat/lon round-trips to the correct elevation within **GEO** tolerance and CRS/extent are asserted.
  - Given a missing tile, when terrain loads, then the gap is reported with a `TerrainTileState` such as `flat_fallback` or `missing` and recorded in the scenario manifest, not silently flattened to zero.
- **Tests**: geospatial round-trip (coordinate → elevation), unit (CRS/extent assertions), failure path (missing tile reported).
- **Depends on**: `flight_sim_cpp/src/GeoTerrain.cpp` (OSM/Terrarium tile fetch, elevation sampling, terrain mesh).

### STORY 02-10 · M3 · M · P0 — Wind field and aerodynamic disturbance
- **Story**: As `OPS`, I want a configurable wind field integrated into the physics, so that the twin can show whether a plan holds under realistic disturbance.
- **Deterministic / evidence**: add a wind field and integrate the force into the `flight_sim_cpp` physics loop via the `set_wind` path; deterministic given seed/config and reproducible through the deterministic runner.
- **Acceptance**:
  - Given a steady crosswind and seed, when a mission flies, then the deterministic drift matches the golden trace.
  - Given zero wind, when a mission flies, then the trace is identical to the no-wind golden fixture (no spurious force).
- **Tests**: golden-file (seeded wind trace), unit (force integration), failure path (zero wind unchanged).
- **Depends on**: 02-01, `flight_sim_cpp` `set_wind`, `flight_sim_cpp/src/DroneSimulation.cpp`.

### STORY 02-11 · M3 · S · P1 — Twin enforces real geofence/altitude/battery limits
- **Story**: As `PA`, I want the twin to enforce the same geofence, altitude, and battery limits as the real path, so that sim-first testing actually validates safety.
- **Deterministic / evidence**: wire the same constraint checks used by `01`/`03` into the twin so violations are raised in simulation identically.
- **Acceptance**:
  - Given a mission that violates the geofence, when run in the twin, then the twin raises the same violation the real path would.
  - Given a constraint that the twin does not enforce, when the parity test runs, then it fails, flagging the gap.
- **Tests**: parity test (twin vs `01`/`03` constraints), failure path (unenforced constraint flagged).
- **Depends on**: 02-04, `01`, `03`.

### STORY 02-12 · M3 · S · P1 — Georeferenced terrain textures
- **Story**: As `OPS`, I want procedural fallback textures replaced with georeferenced map tiles, so that the preview visually matches the real field.
- **Deterministic / evidence**: load OSM map-tile textures aligned to the DEM extent; assert tile alignment to terrain CRS/extent.
- **Acceptance**:
  - Given a field extent, when textures load, then tiles align to the terrain grid within **GEO** tolerance.
  - Given a tile fetch failure, when textures load, then a clearly marked fallback tile is shown with a "tile unavailable" marker, not a silent procedural fill claiming to be real.
- **Tests**: unit (tile alignment), failure path (tile fetch failure marked), fixture.
- **Depends on**: 02-09, `flight_sim_cpp` map-tile fetching/caching (`GeoTerrain.cpp`, viewer tile compositor).

### STORY 02-13 · M3 · S · P2 — Single mission/telemetry contract on the canonical runner
- **Superseded by the deterministic runner**: there is now one canonical simulator (`flight_sim_cpp`), so the original "two interchangeable runners" framing no longer applies. This story folds into enforcing one mission/telemetry format (TwinContractV1) on the single runner; the cross-build/cross-platform parity role moves to STORY 02-35.
- **Story**: As `DSP`, I want the `flight_sim_cpp` headless runner to read one mission format and emit one telemetry format bound to the shared contract, so that interactive and headless runs are interchangeable and regression-safe.
- **Deterministic / evidence**: align `flight_sim_cpp` `MissionLoader`/`TelemetryRecorder` JSONL with the shared mission/telemetry contract (TwinContractV1); a contract test asserts the loader and recorder round-trip the contract types.
- **Acceptance**:
  - Given one mission, when run headless and (conceptually) interactively, then the telemetry format is identical and bound to the same contract.
  - Given a format drift from the contract, when the contract test runs, then it fails identifying the diverging field.
- **Tests**: contract round-trip (mission/telemetry format), failure path (format drift named).
- **Depends on**: 02-04, `flight_sim_cpp/src/MissionLoader.cpp`, `TelemetryRecorder.cpp`, `DeterministicRunner.cpp`.

### STORY 02-19 · M3 · L · P1 — Georeferenced 3D scene synthesis (buildings and farm vegetation)
- **Story**: As `DSP`, I want the world around a chosen location populated with 3D scene objects — building footprints from OSM and farm vegetation (forest trees, bushes, crop rows by crop type) from land-cover classes — so that a simulated flight over a real farm encounters the obstacles and canopy a real flight would.
- **Deterministic / evidence**: instantiate scene objects on the 02-09 DEM from OSM building footprints (`07`) and land-cover/vegetation classes (`05` classification products); placement is seeded and procedural per class (tree spacing, bush density, crop-row geometry); the run emits a reproducible scene manifest listing every object's class, georeferenced footprint, and height.
- **Acceptance**:
  - Given a location and a seed, when scene synthesis runs, then the scene manifest is byte-identical across runs and each object's footprint georeference matches its source feature within **GEO** tolerance.
  - Given an area with no land-cover or OSM coverage, when synthesis runs, then the area is marked "unpopulated" in the manifest, not filled with invented vegetation presented as real.
- **Tests**: golden-file (seeded scene manifest), unit (footprint → placement geometry per class), failure path (missing land cover marked unpopulated).
- **Depends on**: 02-09, `flight_sim_cpp/src/GeoTerrain.cpp`, `05` (vegetation classes), `07` (OSM features).

### STORY 02-20 · M3 · L · P1 — Ray-traced drone camera with configurable field of view
- **Story**: As `DSP`, I want a simulated drone camera that ray-traces the synthesized scene from the drone's pose through configurable FOV/intrinsics, so that perception and capture software can be developed against realistic frames before any real-world flight — the autonomous-vehicle-style sim-first approach.
- **Deterministic / evidence**: cast rays from the drone pose through a pinhole intrinsics model (FOV, resolution, distortion optional) against terrain plus 02-19 scene objects; emit each frame with per-pixel depth, the camera pose, and a timestamp; seeded and reproducible; frame georeference derives from pose + intrinsics.
- **Acceptance**:
  - Given a known scene, pose, and seed, when a frame is captured, then it is reproducible and known objects appear at projectively correct pixel positions within **IMAGE** tolerance.
  - Given a pose outside the loaded scene extent, when capture runs, then the frame is marked "no scene coverage" rather than emitting an empty frame presented as real imagery.
- **Tests**: golden-file (seeded frame hash), unit (ray–object intersection, projection math), failure path (no coverage).
- **Depends on**: 02-19, 02-06 (georeferenced band emission), 02-08 (optional pose noise).

---

## M4 — Interactive

### STORY 02-14 · M4 · M · P0 — Mission preview overlay tied to a field boundary
- **Story**: As `OPS`, I want to preview a planned mission as an overlay on the globe tied to the field boundary, so that I can sanity-check coverage before flying.
- **Deterministic / evidence**: render the `01` waypoint mission and survey pattern over the field boundary in the canonical globe viewer (`flight_sim_cpp`) with correct georeference; coverage fraction shown.
- **Acceptance**:
  - Given a mission and field boundary, when previewed, then waypoints and the boundary render aligned in the globe with reported coverage.
  - Given a mission referencing a field with no boundary, when previewed, then it shows "no boundary" rather than rendering an unanchored path.
- **Tests**: unit (overlay georeference), integration (mission → overlay), failure path (no boundary).
- **Depends on**: 02-09, `01` (mission + survey templates), `10` (field boundary), `flight_sim_cpp` globe picker and mission editor (`macos_opengl_viewer.mm`).

### STORY 02-15 · M4 · M · P0 — Twin-as-backend for `01`/`03` simulation mode
- **Story**: As `OPS`, I want `01` and `03` to drive their `Simulation` runtime mode through one twin API, so that there is a single canonical backend for sim-first testing.
- **Deterministic / evidence**: expose a twin API consuming `shared` commands and producing telemetry; `01`/`03` in `Simulation` mode route through it. The canonical role is resolved: `flight_sim_cpp` is the single canonical simulator for both the interactive viewer and headless CI regression (the Bevy `simulator` crate and the Rust `drone_simulator` crate were both retired).
- **Acceptance**:
  - Given `01` in `Simulation` mode, when it dispatches commands, then they execute against the twin API and stream telemetry back.
  - Given the twin API is unavailable, when `01` enters `Simulation` mode, then it fails closed (refuses to "simulate" with no backend), not silently no-op.
- **Tests**: integration (`01`/`03` sim mode through twin), failure path (twin unavailable fails closed), contract.
- **Depends on**: 02-04, 02-11, `01`, `03`.

### STORY 02-16 · M4 · S · P1 — HUD and flight-UI fidelity for review
- **Story**: As `OPS`, I want the HUD (compass/speed/altitude/battery) and flight-UI state machine validated against telemetry, so that what the operator sees in preview matches the twin state.
- **Deterministic / evidence**: assert HUD values and UI state transitions track the underlying telemetry/status deterministically.
- **Acceptance**:
  - Given a running sim, when the HUD renders, then displayed altitude/speed/battery match telemetry within **TELEM** tolerance.
  - Given a battery-critical state, when the UI updates, then it reflects the emergency state and does not stay green.
- **Tests**: unit (HUD value mapping), failure path (critical state reflected), fixture.
- **Depends on**: 02-03, 02-14, `flight_sim_cpp` viewer telemetry panel (`macos_opengl_viewer.mm`).

### STORY 02-21 · M4 · M · P1 — Telemetry and encoded video streaming to an external collector
- **Story**: As `OPS`, I want a simulated flight's telemetry and camera frames streamed as an encoded video feed to an external telemetry collector, so that the full drone-to-ground data path is exercised end to end with zero hardware.
- **Deterministic / evidence**: encode the 02-20 frame sequence into a video stream; transmit telemetry (in the `shared` schema) and video to a configurable collector endpoint; record sent/acked counts; a local collector fixture asserts frame↔telemetry timestamp alignment and that the video decodes.
- **Acceptance**:
  - Given a simulated mission and a running collector, when streaming runs, then the collector receives decodable video and telemetry whose timestamps align, with sent/received counts matching.
  - Given an unreachable collector, when streaming runs, then frames/telemetry are buffered or dropped with an explicit delivery-failure reason code, never silently lost.
- **Tests**: integration (sim → local collector fixture), unit (timestamp alignment, encode/decode round-trip), failure path (unreachable collector).
- **Depends on**: 02-20, 02-04 (shared telemetry contract), `04` (capture-shaped ingestion).

### STORY 02-22 · M4 · M · P2 — Location-anchored scenario loading from globe navigation
- **Story**: As `OPS`, I want to navigate the globe to any location and have terrain, scene objects, and a mission template load there automatically, so that any field in the world becomes a flyable scenario in minutes — the flight-simulator experience.
- **Deterministic / evidence**: a globe pick triggers DEM/map-tile fetch (02-09/02-12), scene synthesis (02-19), and generation of a default survey mission anchored to the picked coordinates; the scenario manifest (location, tiles, scene seed, mission) is persisted and reloadable.
- **Acceptance**:
  - Given a picked location, when scenario loading completes, then the drone can fly a mission there and a known ground coordinate round-trips through the loaded terrain.
  - Given unreachable tile or feature sources, when loading runs, then a partial scenario is produced with the gaps explicitly listed, not a silently flat or empty world.
- **Tests**: integration (pick → flyable scenario), unit (coordinate anchoring), failure path (fetch failure → explicit gaps).
- **Depends on**: 02-09, 02-12, 02-19, `flight_sim_cpp` globe picker.

---

## M5 — Autonomous-Assist

### STORY 02-23 · M5 · L · P2 — Labeled synthetic perception dataset export
- **Story**: As `AG`, I want every ray-traced frame exportable with perfect ground truth — per-pixel object-class masks, depth, object poses, and camera pose — so that vegetation and crop perception models can be trained sim-first, the way autonomous-vehicle stacks are.
- **Deterministic / evidence**: dataset export derives label rasters directly from the 02-19 scene manifest and 02-20 ray hits (the renderer is the labeler — no model in the loop); seeded and reproducible; a dataset manifest links every frame to its scenario, seed, and pose.
- **Acceptance**:
  - Given an exported dataset, when masks are checked against the scene manifest, then every labeled pixel agrees with the geometry that produced it.
  - Given a frame missing pose or scene linkage, when export runs, then it is excluded with a reason code, not emitted unlabeled.
- **Tests**: unit (mask ↔ scene geometry agreement), golden-file (seeded dataset manifest), failure path (unlinked frame excluded).
- **Depends on**: 02-19, 02-20, consumed by `05`/`23` (classifier fixtures and training).

### STORY 02-17 · M5 · M · P1 — Closed-loop coordination preview against `03`
- **Story**: As `OPS`, I want to preview a coordinated multi-drone maneuver in the twin before any real swarm flight, so that formations and separation are validated safely.
- **Deterministic / evidence**: run multiple twin instances under `03` coordination; verify minimum separation and formation geometry hold across the seeded run; approval-gated and disabled by default.
- **Acceptance**:
  - Given a coordinated survey, when previewed in the twin, then minimum separation holds throughout and the run is reproducible.
  - Given a maneuver that would breach separation, when previewed, then the twin surfaces the breach so it never reaches real flight.
- **Tests**: integration (multi-twin separation), golden-file (seeded coordinated run), failure path (separation breach surfaced).
- **Depends on**: 02-15, 02-10, `03` (collision avoidance + formations).

### STORY 02-18 · M5 · S · P2 — Disturbance scenario library for regression
- **Story**: As `DSP`, I want a library of seeded disturbance scenarios (wind gusts, sensor dropouts, comm loss), so that autonomy and failsafe behavior are regression-tested against adverse conditions.
- **Deterministic / evidence**: each scenario is seeded and committed; a CI suite runs `01`/`03` autonomy/failsafe against them and pins outcomes.
- **Acceptance**:
  - Given the scenario library, when CI runs, then each scenario produces its pinned failsafe/recovery outcome.
  - Given a regression that changes a recovery outcome, when CI runs, then the affected scenario fails, naming the scenario.
- **Tests**: scenario suite (seeded), golden outcomes, failure path (recovery regression flagged).
- **Depends on**: 02-10, 02-17, `01` (failsafe), `03`.

---

## Reliability Backbone — M1/M2 P0 (prerequisites for all regression and safety work)

> These stories are the missing backbone identified in the product review. They are listed as new M1/M2 P0 requirements and must land before the synthetic-perception stack is accepted as meaningful for regression or safety validation.

### STORY 02-24 · M1 · M · P0 — TwinContractV1: versioned interface contract
- **Story**: As `PA`, I want a versioned `TwinContractV1` that defines the wire format for commands, telemetry, trace files, scenario manifests, errors, and declared simulator capabilities, so that every consumer knows exactly what the twin promises and a breaking change is impossible to ship silently.
- **Deterministic / evidence**: a schema file (e.g. `shared/src/twin_contract_v1.rs`) versioned by semver; a contract test asserts each consumer crate (`01`, `03`, `04`) compiles and round-trips all defined types; a version bump is required before any breaking format change.
- **Acceptance**:
  - Given `TwinContractV1` is defined, when any consumer sends a command or reads telemetry, then it is validated against the versioned schema at the boundary, not by convention.
  - Given a breaking format change without a version bump, when the contract test runs, then it fails naming the type and field that broke, not just "compile error."
  - Given a consumer built against `TwinContractV1`, when the twin version changes in a compatible way, then existing consumers continue working without recompilation.
- **Tests**: contract round-trip (all defined types), contract version check (incompatible bump fails), failure path (schema drift without bump detected).
- **Depends on**: `shared/src/`, `flight_sim_cpp` telemetry contract, `flight_sim_cpp/src/DeterministicRunner.cpp`.
- **Status**: seeded — `contract_version` 1.0.0 is now emitted by the C++ runner's `RunManifest` as the seed of TwinContractV1; the full versioned schema (commands, telemetry, trace, manifest, errors, capabilities) plus consumer round-trip is still pending.
- **Note**: this story precedes STORY 02-04 (shared command/telemetry contract); 02-04 becomes an implementation step within the TwinContractV1 envelope.

### STORY 02-25 · M1 · M · P0 — Deterministic runner mode (fixed timestep, seeded PRNG, byte-identical output)
- **Story**: As `DSP`, I want the headless twin runner to operate in a deterministic mode — fixed timestep, seeded PRNG, deterministic timestamps and IDs — so that running the same mission twice with the same seed produces byte-identical telemetry traces.
- **Deterministic / evidence**: a `--seed N --timestep-ms N` CLI flag sets the runner; a two-run test asserts the resulting JSONL traces are byte-identical; the seed and timestep are logged in the run header.
- **Acceptance**:
  - Given the same seed and mission, when the runner executes twice, then the two traces are byte-identical (no clock jitter, no floating-point nondeterminism, no random IDs).
  - Given two different seeds, when the runner executes, then the two traces differ, confirming the seed is actually driving behavior.
  - Given no seed flag, when the runner executes, then it refuses to start with an error: "deterministic mode requires --seed".
- **Tests**: two-run byte-identity assertion, different-seed divergence assertion, failure path (missing seed rejected).
- **Depends on**: `flight_sim_cpp/src/headless_main.cpp`, `flight_sim_cpp/src/DeterministicRunner.cpp`.
- **Status**: initial implementation landed in `flight_sim_cpp` (DeterministicRunner + golden fixture `tests/golden/unit_mission.jsonl`, `agbot_flight_sim_headless --seed` required; byte-identity, seed-drives-PRNG, and missing-seed-rejected paths are tested). Cross-platform byte-identity verification is still pending.
- **Note**: this story is a prerequisite for all golden-fixture stories (02-01, 02-02, 02-05, etc.). A golden trace produced by a non-deterministic runner is not a golden trace.

### STORY 02-26 · M1 · S · P0 — Safety parity harness (geofence/altitude/battery/no-fly-zone/abort rules)
- **Story**: As `PA`, I want a CI test harness that proves the twin enforces the same geofence, altitude ceiling, battery abort, no-fly-zone exclusion, and emergency-abort rules as the real flight path, so that sim-first safety testing produces meaningful evidence.
- **Deterministic / evidence**: for each safety rule, a parameterized harness runs both the twin and a real-path stub with an identical boundary-crossing scenario; asserts the same violation event fires with the same rule code; if the twin does not enforce a rule the real path enforces, the test fails and names the gap.
- **Acceptance**:
  - Given a mission that crosses the geofence, when run in the twin, then the twin raises the same `GeofenceViolation` the real path raises at the same waypoint.
  - Given a mission that enters a no-fly zone, when run in the twin, then the twin raises the same exclusion event as the real path.
  - Given a low-battery abort scenario, when run in the twin, then the twin triggers RTH at the same battery threshold as the real path.
  - Given a safety rule that exists in the real path but is absent from the harness, when the coverage check runs, then it fails listing the uncovered rule.
- **Tests**: parameterized parity test per safety rule, coverage check (unregistered rule fails), failure path (harness gap named explicitly).
- **Depends on**: 02-25 (deterministic runner), `01` (real-path safety rules), `03` (coordination abort rules).
- **Note**: this story upgrades and supersedes STORY 02-11, which was P1/M3. Safety parity is a P0 prerequisite for sim-first testing, not an M3 polish item.
- **Status**: initial implementation landed in `flight_sim_cpp` (`SafetyRules`, required-rule coverage harness, `DroneSimulation` failsafe integration, and violation code tests). Still pending: direct parity against the authoritative `01`/`03` safety rule source and dispatch-path CI wiring.

---

## Reliability Backbone — M2 P0 (terrain fidelity, observability, and fault injection)

### STORY 02-27 · M2 · S · P0 — Terrain no-data model (available/missing/stale/synthetic/flat_fallback)
- **Story**: As `OPS`, I want every DEM and map tile to carry an explicit data-quality state — `available`, `missing`, `stale`, `synthetic`, or `flat_fallback` — so that a missing tile never silently becomes flat zero elevation.
- **Deterministic / evidence**: a `TerrainTileState` enum is carried alongside every elevation sample; the scenario manifest records per-tile states; a test asserts that a missing tile produces `flat_fallback` state, not a zero-elevation sample with no annotation.
- **Acceptance**:
  - Given a tile that cannot be fetched, when terrain loads, then the elevation is tagged `flat_fallback` and the gap is recorded in the scenario manifest; no silent zero is emitted.
  - Given a tile older than its staleness threshold, when terrain loads, then it is tagged `stale` and the run manifest records the staleness reason.
  - Given a synthetic tile generated in the absence of real data, when terrain loads, then it is tagged `synthetic` and any downstream consumer sees the tag.
  - Given a tile with all valid data, when terrain loads, then it is tagged `available`.
- **Tests**: unit (state per condition), scenario manifest assertion (per-tile states), failure path (missing tile → flat_fallback, not silent zero).
- **Depends on**: 02-09 (DEM terrain), `flight_sim_cpp/src/GeoTerrain.cpp`, `TwinContractV1` (manifest schema).
- **Note**: this requirement extends STORY 02-09. The failure path in 02-09 mentions "gap is reported," but does not define explicit states. This story makes the states a first-class contract item.
- **Status**: initial implementation landed in `flight_sim_cpp` (`TerrainTileState`, `TerrainTileStatus`, and `composite_elevation_with_state` mark missing expected tiles as `flat_fallback`). Still pending: stale/synthetic/missing propagation from real fetch/cache outcomes and scenario-manifest serialization.

### STORY 02-28 · M2 · M · P0 — Scenario manifest (per-run metadata and hash registry)
- **Story**: As `PA`, I want every simulation run to produce a scenario manifest — simulator version, seed, mission, terrain tiles used, weather config, sensor configs, safety config, source data hashes, and output hashes — so that any run can be reproduced and any trace can be audited back to its inputs.
- **Deterministic / evidence**: the headless runner emits a `scenario_manifest.json` (schema from `TwinContractV1`) alongside the telemetry trace; the manifest includes SHA-256 hashes of all inputs and outputs; a run without a manifest is rejected by the CI harness.
- **Acceptance**:
  - Given a completed run, when the manifest is inspected, then it contains simulator version, seed, mission hash, terrain tile states and hashes, weather config, sensor config, safety config, and the output trace hash.
  - Given the same seed and inputs, when the run is replayed, then the manifest hashes match the original, confirming reproducibility.
  - Given a run that terminates without emitting a manifest, when the CI harness checks, then it fails with "missing scenario manifest," not silently passes.
- **Tests**: manifest schema validation (all required fields), hash reproducibility (same seed → same hashes), failure path (missing manifest rejected).
- **Depends on**: 02-25 (deterministic runner), 02-27 (terrain tile states), `TwinContractV1`.
- **Status**: seeded — the C++ headless runner now emits a per-run `<output>.manifest.json` (`RunManifest`: simulator_version, contract_version, seed, timestep, mission_hash, output_hash via FNV-1a, prng_nonce, completed). Still pending: terrain tile states/hashes, weather/sensor/safety config, SHA-256 input/output hashing, and the full versioned schema with consumer round-trip.

### STORY 02-29 · M2 · M · P0 — Trace diff CLI (`agbot-sim diff <a> <b>`)
- **Story**: As `DSP`, I want a CLI that compares two simulation trace files and reports the exact step index, field name, and values that diverge, so that golden regression failures name the problem instead of just saying "mismatch."
- **Deterministic / evidence**: `agbot-sim diff <trace-a.jsonl> <trace-b.jsonl>` exits 0 if traces are identical within **TELEM** tolerance, exits 1 with a structured JSON diff listing the first N divergent steps (step index, field path, value in A, value in B, delta if numeric).
- **Acceptance**:
  - Given two identical traces, when `agbot-sim diff` runs, then it exits 0 with "traces identical."
  - Given two traces that diverge at step 42 on field `position.altitude_m`, when `agbot-sim diff` runs, then it exits 1 and names step 42, `position.altitude_m`, and both values.
  - Given a numeric tolerance flag, when comparing traces whose values differ by less than the tolerance, then it reports them as identical.
  - Given a trace from a different contract version, when diff runs, then it reports "incompatible contract version" and exits 2.
- **Tests**: unit (identical → 0, divergent → named diff), tolerance assertion, failure path (incompatible version → exit 2).
- **Depends on**: 02-25 (deterministic traces), `TwinContractV1` (trace schema).
- **Status**: initial implementation landed in `flight_sim_cpp` (`TraceDiff` core and `agbot-sim diff` executable report first divergent step and field path; identical traces exit 0). Still pending: tolerance flags, structured multi-diff JSON output, and contract-version incompatibility checks.

### STORY 02-30 · M2 · M · P0 — Fault injection library (seeded, reproducible fault classes)
- **Story**: As `DSP`, I want a seeded fault injection library covering wind gusts, GPS drift, IMU noise, sensor dropout, comm loss, low battery, stale terrain, bad tile, and actuator lag, so that autonomy and failsafe behavior can be regression-tested against adverse conditions with reproducible inputs.
- **Deterministic / evidence**: each fault class is a named, seeded generator that injects its perturbation into the physics/sensor/terrain/communication layer at a configurable schedule; the scenario manifest records which fault classes and seeds were active; a CI regression suite runs each fault class and pins the failsafe outcome.
- **Acceptance**:
  - Given a seeded GPS-drift fault, when the flight runs, then the GPS readings drift by the documented distribution and the trace is reproducible by seed.
  - Given a sensor-dropout fault scheduled at step 100, when the flight runs, then the sensor emits no readings from step 100 forward until recovery, and the manifest records the dropout event.
  - Given a bad-tile fault, when terrain loads, then the affected tile is tagged `flat_fallback` and the scenario manifest records the injection.
  - Given the same fault seed across two runs, when traces are compared with the trace diff CLI, then they are identical.
- **Tests**: per-fault-class seeded regression, manifest records injection, failure path (injected fault without seed → rejected), trace reproducibility (same seed → same fault trace).
- **Depends on**: 02-25 (deterministic runner), 02-27 (terrain no-data), 02-28 (scenario manifest), 02-29 (trace diff).
- **Note**: this story supersedes and expands STORY 02-18 (Disturbance scenario library, M5/P2). The fault injection library is a reliability backbone item, not a late-stage polish item.

### STORY 02-31 · M2 · S · P0 — Simulation health/operability (health checks, seed logging, trace retention, runbook)
- **Story**: As `PA`, I want the headless simulator to expose structured health checks, log its seed and version on every run, enforce a trace retention policy, control its tile cache, and have a runbook — so that CI failures are diagnosable and the simulator is operable without tribal knowledge.
- **Deterministic / evidence**: `agbot-sim health` returns a structured JSON pass/fail over: runner mode, PRNG seeded, terrain cache state, last-run manifest present, trace retention compliant; every run logs a header with simulator version, contract version, seed, timestep, and run ID.
- **Acceptance**:
  - Given a healthy simulator, when `agbot-sim health` runs, then it exits 0 with a structured JSON listing all checked subsystems as `pass`.
  - Given a run with no seed set, when the health check runs, then `prng_seeded` is `fail` and the runner refuses to start.
  - Given a trace retention policy (e.g. keep last N runs), when a new run completes, then old traces beyond N are deleted and the manifest records the deletion.
  - Given a cache-clear command, when it runs, then the tile cache is emptied and the next run fetches fresh tiles.
- **Tests**: health check pass/fail per subsystem, seed-missing → refused, trace retention enforcement, cache-clear confirmation.
- **Depends on**: 02-25 (deterministic runner), 02-28 (scenario manifest).

---

## Reliability Backbone — M2 P1 (capture fidelity and single-runner deterministic regression)

### STORY 02-32 · M2 · M · P1 — Capture replay adapter (sim sensor output → domain `04` ingestion path)
- **Story**: As `AG`, I want simulated LiDAR, camera, and multispectral sensor output to flow through the exact same domain `04` `FlightDataRecord` ingestion path as real hardware, so that the entire capture pipeline is regression-tested without physical sensors.
- **Deterministic / evidence**: the adapter routes 02-05/02-06 sensor output through the same `04` ingestion handler (same provenance fields: sensor ID, GPS tag, timestamp, session ID) rather than a test-only bypass; a CI test compares records produced by the sim adapter to records produced by a real-hardware fixture and asserts structural identity.
- **Acceptance**:
  - Given a simulated flight, when sensors emit, then `04` records carry the same provenance structure as real-hardware records (sensor ID, GPS tag, timestamp, session).
  - Given a simulated sensor that fails mid-flight, when the adapter runs, then `04` records a `collection_failure` with the same structure as a real hardware failure.
  - Given a real-hardware ingestion test fixture and the sim adapter, when both are run with the same data shape, then the resulting `FlightDataRecord` schemas are identical.
- **Tests**: integration (sim → `04` records match schema), provenance completeness, failure path (sim sensor failure → `collection_failure` record).
- **Depends on**: 02-05 (LiDAR sim), 02-06 (camera sim), `04` (capture ingestion), `TwinContractV1`.
- **Note**: this story replaces and strengthens STORY 02-07 (Capture-shaped sensor stream into `04`). The key difference is the adapter must use the exact `04` ingestion path, not a test-only shortcut.

### STORY 02-33 · M2 · S · P1 — Sensor calibration profiles (cheap GPS, RTK GPS, noisy IMU, LiDAR A3, multispectral camera)
- **Story**: As `DSP`, I want named, versioned sensor calibration profiles that configure noise distributions, drift rates, and calibration offsets for each sensor model — so that tests are reproducibly keyed to a named sensor, not to a magic noise constant buried in a config file.
- **Deterministic / evidence**: profiles are stored as named files (e.g. `calibration/rtk_gps_a1.toml`); loading a profile by name configures the sensor suite reproducibly; the scenario manifest records the profile name and version.
- **Acceptance**:
  - Given a named profile `rtk_gps_a1`, when the sensor suite loads it, then GPS readings match the documented RTK-grade noise characteristics, and the scenario manifest records `sensor_profile: rtk_gps_a1`.
  - Given a profile `cheap_gps_b2`, when the sensor suite loads it, then GPS readings match the documented consumer-grade noise characteristics.
  - Given an unknown profile name, when the runner starts, then it rejects the run with "unknown sensor profile," not defaults to a silent fallback.
- **Tests**: per-profile noise-distribution check, manifest records profile name, failure path (unknown profile → rejected).
- **Depends on**: 02-08 (sensor noise), 02-28 (scenario manifest), `TwinContractV1`.

### STORY 02-34 · M2 · M · P1 — Mission validation report (pre-run deterministic check)
- **Story**: As `OPS`, I want a deterministic mission validation report produced before each simulated run — expected coverage fraction, estimated flight duration, battery margin, terrain data gaps, safety risks, and blocked waypoints — so that a doomed or risky run is caught before it wastes resources.
- **Deterministic / evidence**: `agbot-sim validate <mission.json>` produces a structured JSON report; blocked missions produce non-zero exit code listing the blocking reasons; the report is deterministic given the same mission and terrain state.
- **Acceptance**:
  - Given a valid mission over fully available terrain, when validation runs, then it produces a report with coverage, estimated duration, battery margin, and zero terrain gaps.
  - Given a mission with waypoints over a missing-terrain tile, when validation runs, then it reports the terrain gap and classifies the mission as "runnable with gaps" or "blocked" per policy.
  - Given a mission that would breach the geofence, when validation runs, then it reports the blocked waypoints and exits non-zero.
  - Given the same mission and terrain state, when validation runs twice, then reports are byte-identical.
- **Tests**: report schema validation, terrain-gap detection, geofence-blocked exit code, determinism (same input → byte-identical report).
- **Depends on**: 02-26 (safety parity), 02-27 (terrain no-data), 02-28 (scenario manifest), `01` (mission schema).

### STORY 02-35 · M2 · M · P1 — Single-runner deterministic regression parity (cross-build/cross-platform determinism)
- **Story**: As `PA`, I want mandatory CI regression that runs a reference mission with a fixed seed on `flight_sim_cpp` and asserts the trace is byte-identical and the scenario-manifest hash matches across builds and platforms, so that the one canonical runner cannot silently drift between commits or environments.
- **Deterministic / evidence**: a regression suite runs a set of reference missions (takeoff/land/goto/orbit) with the same seed against committed golden traces and golden manifest hashes; uses the trace diff CLI (02-29) to compare; any field divergence beyond tolerance, or any manifest-hash mismatch, fails the test and names the diverging field. Determinism is verified across debug/release builds and across supported platforms.
- **Acceptance**:
  - Given a reference mission and seed, when re-run against its golden trace, then `agbot-sim diff` reports "traces identical" and the manifest output_hash matches the golden hash.
  - Given a behavior change that perturbs the trace, when the regression runs, then it fails naming the step, field, and both values.
  - Given a build- or platform-induced nondeterminism, when the same seed is run on two environments, then the regression fails identifying the divergent step rather than passing silently.
  - Given a format drift (different telemetry/manifest schema), when the regression runs, then it fails with "incompatible contract version," not a silent parse error.
- **Tests**: golden trace + manifest-hash regression on all reference missions, divergence detection (names field), cross-build/cross-platform determinism check, format-drift detected.
- **Depends on**: 02-25 (deterministic runner), 02-24 (`TwinContractV1`), 02-28 (scenario manifest), 02-29 (trace diff CLI).
- **Status**: partially implemented — same-seed byte-identity and per-run manifest hashing already exist on `flight_sim_cpp` with a committed golden fixture; the cross-build/cross-platform CI gate and a manifest-hash golden are still pending.
- **Note**: this replaces the obsolete two-runner framing (there is now one canonical runner) and folds in the regression role previously assigned to STORY 02-13. Single-runner determinism is a required reliability item, not a P2 nice-to-have.

---

## Coverage note

~35 stories cover three layers: (1) the reliability backbone (02-24 to 02-35: TwinContractV1, deterministic runner, safety parity harness, terrain no-data model, scenario manifest, trace diff CLI, fault injection library, health/operability, capture replay adapter, sensor calibration profiles, mission validation report, and single-runner deterministic regression — all M1/M2); (2) the physics/sensor/terrain foundation (02-01 to 02-13, M1/M2/M3); (3) the synthetic-perception stack (02-19 to 02-23: scene synthesis → ray-traced camera → video/telemetry streaming → labeled dataset export, M3/M4/M5). There is a single canonical runner (`flight_sim_cpp`); the Rust/Bevy `simulator` and Rust `drone_simulator` crates were both retired. "Parity" now means cross-build/cross-platform determinism on that one runner — same-seed byte-identity plus scenario-manifest hash reproducibility — not agreement between two runners. Story 02-26 (safety parity) supersedes 02-11 (upgrading it from P1/M3 to P0/M1), and 02-35 (single-runner deterministic regression) folds in the regression role previously assigned to 02-13; the deterministic runner, run header logging, scenario-manifest seed, and the golden fixture `tests/golden/unit_mission.jsonl` already exist on `flight_sim_cpp`. The curated counts in `release-plan.md` expand several of these — per-sensor noise models, per-command-mode golden traces, per-formation preview cases, per-vegetation-class scene generators, and additional terrain-tile sources — into sibling stories when implemented. Every physics/controller story carries a seeded golden fixture, and the twin enforces the same geofence/altitude/battery/no-fly-zone/abort limits as the real flight path so sim-first testing for `01`/`03` is meaningful.
