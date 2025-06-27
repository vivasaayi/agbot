Create a Rust monorepo (Cargo workspace) called `agrodrone`.

The repo must contain the following workspace members:

1. `mission_control` – A core async service that:
   - Connects to a Pixhawk/Cube Orange flight controller using the MAVLink protocol over serial.
   - Uses the `mavlink` crate to send/receive telemetry, heartbeat, and mission waypoints.
   - Provides a JSON API or gRPC server for uploading missions and real-time monitoring.
   - Emits events over WebSockets to a ground control UI (e.g., mission status, battery, coordinates).

2. `sensor_collector` – A sensor ingestion module that:
   - Reads LiDAR stream from RPLIDAR A3 over serial.
   - Streams parsed distance data to mission control via channels or publish/subscribe.
   - Reads raw frames from a multispectral camera (simulate if hardware not connected).
   - Saves all data to timestamped local folders.

3. `ndvi_processor` – A post-flight image processor that:
   - Accepts folder of captured multispectral bands (NIR + Red).
   - Generates NDVI images using `(NIR - Red) / (NIR + Red)`.
   - Saves resulting images as GeoTIFF or PNG with GPS overlay metadata if available.
   - Uses the `image` crate and optionally FFI-bindings to `opencv`.

4. `lidar_mapper` – A LiDAR post-processing module that:
   - Accepts scan data (timestamp, angle, distance).
   - Constructs a 2D occupancy grid or point cloud in `.pcd` or `.ply`.
   - Optionally runs a basic obstacle heatmap for visualization.
   - Uses `nalgebra`, `kiss3d`, or export to WebGL viewer.

5. `ground_station_ui` – A CLI or Web dashboard (optional) that:
   - Connects to WebSocket server in `mission_control`.
   - Displays live telemetry (altitude, battery, position).
   - Shows NDVI map overlay or LIDAR scan overlay.
   - Uses `egui`, `tui`, or serve a frontend via `warp` or `axum`.

6. `shared` – Common crates used across all modules:
   - Configuration loader
   - Sensor schemas (structs)
   - Logging setup
   - Runtime configuration profiles (SIMULATION vs FLIGHT)

Workspace-level tools:
- `.env` support using `dotenvy`
- Logging with `tracing`
- Async runtime using `tokio`
- Use `cross` for ARM builds (Jetson or Pi)
- Use `anyhow`, `thiserror` for error handling

Create a `README.md` in the root that explains:
- Setup instructions for Jetson/Linux
- Build and test commands
- How to run each module in dev or flight mode
