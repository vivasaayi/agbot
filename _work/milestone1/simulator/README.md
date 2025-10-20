# Simulator Milestone 1 — Core foundations

This document defines the detailed job breakdown, checkpoints, acceptance criteria, and runbook to deliver Milestone 1 for the Bevy-based simulator.

## Goal
Ship a usable simulator baseline with:
- World geodesy (WGS84 <-> ECEF <-> ENU) utilities and tests
- Terrain tile streaming scaffold with LOD rings (stubbed data source)
- NDVI overlay renderer hook (consumes rasters later)
- Basic waypoint autopilot for a single drone
- Telemetry schema alignment with `shared` crate
- Build scripts/tests docs so others can run it reliably

## Scope of work
- Code additions live under `simulator/src/` in new small modules and lightweight integration wiring.
- Add unit tests where math-heavy (geodesy, autopilot).
- No external web services; terrain source can be a stubbed heightmap file.

## Jobs and checkpoints

1) Geodesy utilities
- Jobs:
  - Add `simulator/src/geodesy.rs` with conversions: WGS84 (lat, lon, h) <-> ECEF (x, y, z) and ECEF <-> ENU given an origin.
  - Provide helper `LocalFrame` that precomputes rotation from ECEF to ENU for an origin.
  - Expose constants for WGS84 (a, f, b, e2).
- Checkpoints:
  - Unit tests cover round-trip error < 1e-3 m.
  - Example usage from a system prints camera position ENU.
- Acceptance:
  - Functions are pure, documented, and tested.

2) Terrain tile streaming + LOD
- Jobs:
  - Create `simulator/src/terrain/streamer.rs` with a `TerrainStreamer` resource.
  - Ring-based LOD around camera (R0..R2) that decides which tiles to load/evict.
  - Stubbed loader that generates a flat or sine-wave heightmap per tile.
  - Systems that spawn/despawn tile entities.
- Checkpoints:
  - Logging shows load/evict as the camera moves.
  - Max in-memory tiles bounded by config.
- Acceptance:
  - No panics; stepping camera position triggers tile churn deterministically.

3) NDVI overlay renderer hook
- Jobs:
  - Add `simulator/src/overlays/ndvi.rs` with a `NdviOverlay` component and system that tints tile material from an NDVI texture (placeholder texture for now).
  - Resource/interface to plug an NDVI provider later (file, websocket, etc.).
- Checkpoints:
  - Tiles are tinted with a green-red gradient texture.
- Acceptance:
  - Cleanly disabled if feature flag `ndvi_overlay` is off.

4) Waypoint autopilot (basic)
- Jobs:
  - Add `simulator/src/autopilot/waypoint.rs` implementing a simple P controller tracking waypoints with speed/altitude caps.
  - Integrate with existing drone component/physics.
  - Expose minimal CLI/config for waypoints.
- Checkpoints:
  - Drone traverses 3-4 waypoints in a square at constant speed.
- Acceptance:
  - No oscillations or NaNs; stops at final waypoint within tolerance.

5) Telemetry schema alignment
- Jobs:
  - Use `shared::schemas` telemetry structures for outbound messages.
  - Update `simulator/src/communication.rs` to serialize these for UI/ground-station.
- Checkpoints:
  - JSON frames match schema; round-trip serde tests pass.
- Acceptance:
  - Consumers can parse without custom mapping.

6) Build, lint, and tests
- Jobs:
  - Add unit tests to `simulator` for geodesy and waypoint controller.
  - Ensure `cargo check -p simulator && cargo test -p simulator` passes.
- Acceptance:
  - CI-ready commands documented in Runbook.

## Deliverables
- New source files:
  - `simulator/src/geodesy.rs`
  - `simulator/src/terrain/streamer.rs`
  - `simulator/src/overlays/ndvi.rs`
  - `simulator/src/autopilot/waypoint.rs`
- Updated integration in `simulator/src/app.rs` or appropriate plugin setup.
- Unit tests in `simulator/src/...` or `simulator/tests/` as appropriate.
- This README as the jobs plan with checkpoints.

## Runbook
- Build everything:
  - `cargo check`
- Run simulator only:
  - `cargo run -p simulator`
- Run tests for simulator:
  - `cargo test -p simulator`

## Notes / Future milestones
- Replace stub heightmap with real terrain (SRTM, Cesium, local DEMs).
- Feed NDVI textures from `imagery_processor` outputs.
- Expand autopilot to loiter, RTL, and follow modes.
