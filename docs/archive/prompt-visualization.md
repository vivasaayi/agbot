Add a new crate in the `agrodrone` Rust monorepo called `visualizer`.

Use the Bevy game engine (latest version) to:

- Visualize a 3D world with a flat or hilly terrain (optional DEM loader).
- Spawn multiple drones as animated meshes using telemetry updates.
- Convert GPS (lat/lon/alt) to local X/Y/Z using a simple projection.
- Connect to the `multi_drone_control` crate over WebSocket or gRPC.
- Each drone publishes position, heading, battery, and sensor metadata.
- Load NDVI or multispectral data as textures and overlay them on the terrain mesh.
- Simulate LiDAR as dynamic 3D point clouds emitted from drone body.
- Add a HUD with altitude, heading, speed, and sensor data overlays.
- Allow time-based playback of missions or live stream.

Bonus:
- Use `bevy_egui` to show UI controls and charts.
- Include a "Replay Mission" mode that loads a saved telemetry log and replays the flight.

Requirements:
- Add this crate to the `Cargo.toml` workspace.
- Use `tokio` for async tasks and channel-based event system.
- Use `serde` + `bincode` for efficient telemetry data.
- Use `bevy_asset`, `bevy_render`, and `bevy_egui` for interactivity and visualization.
