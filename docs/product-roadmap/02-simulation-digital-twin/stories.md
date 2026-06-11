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
- **Deterministic / evidence**: run `SimulationEngine`/`DronePhysics` with a fixed seed and timestep; record a golden telemetry trace; CI fails on any deviation beyond tolerance.
- **Acceptance**:
  - Given a fixed seed and mission, when the physics loop runs, then the trace matches the committed golden fixture within tolerance.
  - Given an unintended physics change, when CI runs, then the golden test fails and names the diverging field, not just "mismatch."
- **Tests**: golden-file (seeded trace), unit (physics integration step), failure path (perturbed constant fails golden).
- **Depends on**: `drone_simulator/src/physics.rs`, `lib.rs`.

### STORY 02-02 · M1 · M · P0 — Deterministic flight-controller golden traces
- **Story**: As `OPS`, I want takeoff/land/goto/orbit/hover/RTH command modes pinned by golden traces, so that the PID controller behaves identically across builds.
- **Deterministic / evidence**: seed the `FlightController`; run each command mode to completion; record golden traces; assert the `DroneStatus` state machine transitions.
- **Acceptance**:
  - Given each command mode, when run seeded, then its trace and status transitions match the golden fixture.
  - Given a controller-gain change, when CI runs, then the affected mode's golden test fails with the diverging step identified.
- **Tests**: golden-file (per mode), unit (state machine transitions), failure path (gain change fails golden).
- **Depends on**: 02-01, `drone_simulator/src/flight_controller.rs`.

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

### STORY 02-05 · M2 · L · P0 — LiDAR raycast point-cloud simulation
- **Story**: As `AG`, I want the simulator to emit a real raycast point cloud, so that capture (`04`) and LiDAR mapping (`06`) can be developed and regression-tested without hardware.
- **Deterministic / evidence**: implement deterministic raycasting against terrain/obstacles in the canonical simulator (`flight_sim_cpp`, bridged capture-shaped to Rust; the Bevy-era `lidar_simulator.rs` stub was removed with the `simulator` crate), emitting `LidarScan`/`LidarPoint` consumable by `04`; seeded so output is reproducible.
- **Acceptance**:
  - Given a scene with known geometry and a seed, when the LiDAR sim runs, then it emits a reproducible point cloud whose ranges match the geometry within tolerance.
  - Given a degenerate empty scene, when the sim runs, then it emits an empty-but-valid scan, not a panic or garbage points.
- **Tests**: unit (raycast ranges), golden-file (seeded cloud), failure path (empty scene).
- **Depends on**: 02-09 (terrain geometry), `04` (capture shape), `shared` LiDAR schema.

### STORY 02-06 · M2 · M · P1 — Camera / multispectral simulation emitting georeferenced bands
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
- **Depends on**: 02-01, `drone_simulator/src/sensors.rs`.

### STORY 02-09 · M3 · M · P0 — Real DEM terrain with CRS/extent assertions
- **Story**: As `OPS`, I want real georeferenced DEM terrain loaded with asserted CRS/extent/resolution, so that mission preview matches the actual field.
- **Deterministic / evidence**: load DEM elevation tiles (OSM/Terrarium) into the terrain grid; assert CRS, extent, and resolution and round-trip a known coordinate; replace flat/placeholder elevation.
- **Acceptance**:
  - Given a field's DEM tiles, when terrain loads, then a known lat/lon round-trips to the correct elevation within tolerance and CRS/extent are asserted.
  - Given a missing tile, when terrain loads, then the gap is reported and the area marked "no elevation," not silently flattened to zero.
- **Tests**: geospatial round-trip (coordinate → elevation), unit (CRS/extent assertions), failure path (missing tile reported).
- **Depends on**: `flight_sim_cpp/src/GeoTerrain.cpp` (OSM/Terrarium tile fetch, elevation sampling, terrain mesh).

### STORY 02-10 · M3 · M · P0 — Wind field and aerodynamic disturbance
- **Story**: As `OPS`, I want a configurable wind field integrated into the physics, so that the twin can show whether a plan holds under realistic disturbance.
- **Deterministic / evidence**: add a wind field and integrate the force into `DronePhysics`; deterministic given seed/config; consistent between the Rust twin and the C++ `set_wind` path.
- **Acceptance**:
  - Given a steady crosswind and seed, when a mission flies, then the deterministic drift matches the golden trace.
  - Given zero wind, when a mission flies, then the trace is identical to the no-wind golden fixture (no spurious force).
- **Tests**: golden-file (seeded wind trace), unit (force integration), failure path (zero wind unchanged).
- **Depends on**: 02-01, `flight_sim_cpp` `set_wind`, `drone_simulator/src/physics.rs`.

### STORY 02-11 · M3 · S · P1 — Twin enforces real geofence/altitude/battery limits
- **Story**: As `PA`, I want the twin to enforce the same geofence, altitude, and battery limits as the real path, so that sim-first testing actually validates safety.
- **Deterministic / evidence**: wire the same constraint checks used by `01`/`03` into the twin so violations are raised in simulation identically.
- **Acceptance**:
  - Given a mission that violates the geofence, when run in the twin, then the twin raises the same violation the real path would.
  - Given a constraint that the twin does not enforce, when the parity test runs, then it fails, flagging the gap.
- **Tests**: parity test (twin vs `01`/`03` constraints), failure path (unenforced constraint flagged).
- **Depends on**: 02-04, `01`, `03`.

### STORY 02-12 · M3 · S · P1 — Georeferenced terrain textures
- **Story**: As `OPS`, I want procedural placeholder textures replaced with georeferenced map tiles, so that the preview visually matches the real field.
- **Deterministic / evidence**: load OSM map-tile textures aligned to the DEM extent; assert tile alignment to terrain CRS/extent.
- **Acceptance**:
  - Given a field extent, when textures load, then tiles align to the terrain grid within pixel tolerance.
  - Given a tile fetch failure, when textures load, then the placeholder is shown with a "tile unavailable" marker, not a silent procedural fill claiming to be real.
- **Tests**: unit (tile alignment), failure path (tile fetch failure marked), fixture.
- **Depends on**: 02-09, `flight_sim_cpp` map-tile fetching/caching (`GeoTerrain.cpp`, viewer tile compositor).

### STORY 02-13 · M3 · S · P2 — C++ headless runner shares mission/telemetry contract
- **Story**: As `DSP`, I want the C++ headless runner to read the same mission format and emit the same telemetry as the Rust twin, so that the two simulators stay interchangeable for regression.
- **Deterministic / evidence**: align `flight_sim_cpp` `MissionLoader`/`TelemetryRecorder` JSONL with the shared mission/telemetry contract; a cross-runner test compares traces for the same mission.
- **Acceptance**:
  - Given one mission, when run on both runners, then their telemetry traces agree within tolerance.
  - Given a format mismatch, when the cross-runner test runs, then it fails identifying the diverging field.
- **Tests**: cross-runner trace comparison, contract test (mission/telemetry format), failure path (format mismatch).
- **Depends on**: 02-04, `flight_sim_cpp/src/MissionLoader.cpp`, `TelemetryRecorder.cpp`.

### STORY 02-19 · M3 · L · P1 — Georeferenced 3D scene synthesis (buildings and farm vegetation)
- **Story**: As `DSP`, I want the world around a chosen location populated with 3D scene objects — building footprints from OSM and farm vegetation (forest trees, bushes, crop rows by crop type) from land-cover classes — so that a simulated flight over a real farm encounters the obstacles and canopy a real flight would.
- **Deterministic / evidence**: instantiate scene objects on the 02-09 DEM from OSM building footprints (`07`) and land-cover/vegetation classes (`05` classification products); placement is seeded and procedural per class (tree spacing, bush density, crop-row geometry); the run emits a reproducible scene manifest listing every object's class, georeferenced footprint, and height.
- **Acceptance**:
  - Given a location and a seed, when scene synthesis runs, then the scene manifest is byte-identical across runs and each object's footprint georeference matches its source feature within tolerance.
  - Given an area with no land-cover or OSM coverage, when synthesis runs, then the area is marked "unpopulated" in the manifest, not filled with invented vegetation presented as real.
- **Tests**: golden-file (seeded scene manifest), unit (footprint → placement geometry per class), failure path (missing land cover marked unpopulated).
- **Depends on**: 02-09, `flight_sim_cpp/src/GeoTerrain.cpp`, `05` (vegetation classes), `07` (OSM features).

### STORY 02-20 · M3 · L · P1 — Ray-traced drone camera with configurable field of view
- **Story**: As `DSP`, I want a simulated drone camera that ray-traces the synthesized scene from the drone's pose through configurable FOV/intrinsics, so that perception and capture software can be developed against realistic frames before any real-world flight — the autonomous-vehicle-style sim-first approach.
- **Deterministic / evidence**: cast rays from the drone pose through a pinhole intrinsics model (FOV, resolution, distortion optional) against terrain plus 02-19 scene objects; emit each frame with per-pixel depth, the camera pose, and a timestamp; seeded and reproducible; frame georeference derives from pose + intrinsics.
- **Acceptance**:
  - Given a known scene, pose, and seed, when a frame is captured, then it is reproducible and known objects appear at projectively correct pixel positions within tolerance.
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
- **Deterministic / evidence**: expose a twin API consuming `shared` commands and producing telemetry; `01`/`03` in `Simulation` mode route through it. Canonical roles are resolved: `flight_sim_cpp` is the canonical interactive simulator/viewer, `drone_simulator` is the headless Rust twin for CI regression (the Bevy `simulator` crate was removed).
- **Acceptance**:
  - Given `01` in `Simulation` mode, when it dispatches commands, then they execute against the twin API and stream telemetry back.
  - Given the twin API is unavailable, when `01` enters `Simulation` mode, then it fails closed (refuses to "simulate" with no backend), not silently no-op.
- **Tests**: integration (`01`/`03` sim mode through twin), failure path (twin unavailable fails closed), contract.
- **Depends on**: 02-04, 02-11, `01`, `03`.

### STORY 02-16 · M4 · S · P1 — HUD and flight-UI fidelity for review
- **Story**: As `OPS`, I want the HUD (compass/speed/altitude/battery) and flight-UI state machine validated against telemetry, so that what the operator sees in preview matches the twin state.
- **Deterministic / evidence**: assert HUD values and UI state transitions track the underlying telemetry/status deterministically.
- **Acceptance**:
  - Given a running sim, when the HUD renders, then displayed altitude/speed/battery match telemetry within tolerance.
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

## Coverage note

~23 stories cover the 12 capabilities in `capability-map.md` plus the synthetic-perception stack (scene synthesis → ray-traced camera → video/telemetry streaming → labeled dataset export, stories 02-19 to 02-23), ordered by phase and weighted toward M3 (golden fixtures, terrain, physics) per `release-plan.md`. The curated counts in `release-plan.md` (≈91 rows) expand several of these — per-sensor noise models, per-command-mode golden traces, per-formation preview cases, per-vegetation-class scene generators, and additional terrain-tile sources — into sibling stories when implemented. Every physics/controller story carries a seeded golden fixture, and the twin enforces the same geofence/altitude/battery limits as the real flight path so sim-first testing for `01`/`03` is meaningful.
