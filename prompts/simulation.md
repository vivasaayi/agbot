Create a Rust monorepo named `agrodrone` using Cargo workspaces. This repo will power autonomous agricultural drones.

The project should simulate and manage multiple drones, run missions over real GPS maps, collect synthetic or real sensor data (NDVI, LiDAR, multispectral), and support post-processing and visualization.

### Workspace Structure
Define the following crates:

1. `mission_planner`:
   - Web frontend (optionally React+TypeScript or egui) with a map interface using Google Maps or custom satellite tiles.
   - User can draw flight paths over a field and save as waypoints (lat/lng/alt).
   - Convert drawn paths into MAVLink-compatible mission format (e.g., TAKEOFF, WAYPOINT, LAND).
   - Send missions to the backend API over WebSocket or gRPC.

2. `drone_simulator`:
   - Accept a mission plan (JSON or protobuf) and simulate real-world GPS movement with time and altitude interpolation.
   - Generate synthetic telemetry (battery %, speed, GPS fix, etc.).
   - Optionally add artificial noise, wind, and drift for realism.
   - Simulate multiple drones concurrently with unique IDs.

3. `sensor_overlay_engine`:
   - Overlay synthetic NDVI, multispectral, or LiDAR data onto the mission path.
   - Simulate real-world imaging (e.g., NIR + Red channel for NDVI).
   - For LiDAR, simulate 2D/3D point cloud slices based on altitude and terrain model.
   - Export sensor layers as heatmaps or point cloud files.

4. `multi_drone_control`:
   - Runtime orchestrator that tracks all active drones, missions, and sensor streams.
   - Each drone connects via WebSocket or gRPC.
   - Provides a unified telemetry feed to the frontend.
   - Handles drone status (idle, running, completed, failed) and supports mission restarts.

5. `data_collector`:
   - Stores simulated or real sensor data and logs in a structured format.
   - Organize by flight ID and timestamp.
   - Exports CSV, GeoTIFF (NDVI), and PCD (LiDAR) for analysis.

6. `post_processor`:
   - For real or simulated flights, process sensor files to generate NDVI maps and LiDAR heatmaps.
   - Uses the `image`, `ndarray`, or `opencv` crate.
   - Includes color mapping and statistics extraction (e.g., healthy/unhealthy regions).

7. `shared`:
   - Shared types: Waypoints, telemetry structs, drone state, sensor packets.
   - Logging (`tracing`), CLI arg parsing (`clap`), serialization (`serde`, `bincode`).
   - Build profiles for simulation, development, and production modes.

### Global Tech Requirements:
- All crates should be in `agrodrone/` with Cargo workspace.
- Use `tokio` as the async runtime.
- Use `warp`, `axum`, or `tonic` for backend APIs.
- Use `tokio-tungstenite` for WebSocket telemetry.
- Use `image`, `ndarray`, or `opencv` FFI for NDVI and visual processing.
- Use `kiss3d` or export `.ply`/`.pcd` for LiDAR visualization.
- Support simulated GPS movement and sensor overlays on maps.
- Include `.env` support and `dotenvy`.

### Optional Web UI:
- React or `egui`-based UI to:
  - Draw waypoints
  - View live drone positions
  - View NDVI/LiDAR overlays
  - Simulate or replay missions

### README.md should explain:
- How to run a full simulated mission
- How to ingest custom waypoints
- How to visualize telemetry and overlays
- Folder structure and build steps

The goal is to simulate and visualize drone missions, overlay crop data, and support full flight and post-flight analysis using real or synthetic data.
