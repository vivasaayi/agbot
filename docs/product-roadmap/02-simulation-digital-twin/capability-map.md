# Simulation and Digital Twin: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety, geospatial correctness, data quality, performance and scale, operability, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Simulation and Digital Twin Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Per-drone physics (gravity/drag/thrust/battery) | strong partial | 8 | Golden-telemetry regression fixtures for the physics loop |
| Sensor suite (GPS/IMU/baro/mag) | medium partial | 7 | Inject configurable noise and emit calibrated readings |
| PID flight controller and command modes | strong partial | 7 | Deterministic takeoff/land/goto/orbit golden traces |
| Status state machine and event broadcast | strong partial | 6 | Assert lifecycle transitions and emergency events |
| Wind and aerodynamic disturbance | missing | 6 | Add a wind field and integrate force into physics |
| LiDAR sensor simulation | missing (1-line TODO) | 8 | Raycast point cloud into capture-shaped output for `04` |
| Camera / multispectral simulation | early partial | 6 | Emit georeferenced band images for `04`/`05` |
| Globe navigation and flight UI (`flight_sim_cpp`) | strong partial | 7 | Mission preview overlay tied to a field boundary |
| OSM map-tile and terrain loading | medium partial | 8 | Real DEM elevation with CRS/extent assertions |
| Earth textures and 3D terrain rendering | early partial (placeholder) | 6 | Replace procedural textures with georeferenced tiles |
| C++ headless runner and telemetry replay | medium partial | 6 | Shared mission/telemetry contract with the Rust twin |
| Twin-as-backend for flight/coordination | early partial | 7 | Drive `01`/`03` simulation mode through one twin API |
| Georeferenced 3D scene synthesis (buildings, farm vegetation) | missing | 6 | Seeded scene manifest from OSM footprints + land-cover classes |
| Ray-traced drone camera (FOV/intrinsics) | missing | 5 | Reproducible frame + depth from drone pose over a known scene |
| Telemetry + video streaming to external collector | missing | 4 | Encoded video and `shared` telemetry into a local collector fixture |
| Labeled synthetic dataset export | missing | 3 | Frames + class masks/depth/poses derived from the scene manifest |
