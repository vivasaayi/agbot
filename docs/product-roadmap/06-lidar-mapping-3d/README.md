# LiDAR Mapping and 3D Reconstruction

Turn LiDAR captures into occupancy grids, elevation and obstacle products, and 3D reconstructions for the viewer, terrain, and agronomy workflows.

## Where We Are

- `lidar_mapper` loads RPLIDAR JSON scans, builds configurable-resolution occupancy grids, exports PCD v0.7 point clouds and obstacle heatmaps, and has test coverage for the occupancy logic.
- Parallel scan loading (futures stream, 8 workers), an overridable threshold CLI, and occupancy-grid/heatmap PNG output exist.
- `sensor_overlay_engine` has a `LidarOverlayProcessor` (height-based coloring, occupancy grid, elevation/intensity overlays) for the viewer.

## Where We Should Be

- Cleaned point clouds (outlier/noise removal, normal estimation) before any product is derived.
- Clustering and segmentation that separate ground, canopy, and obstacles, feeding canopy-height to agronomy (`09`).
- True 3D reconstruction (DSM/DTM, meshing) with asserted georeferencing, rendered as terrain in the viewer (`08`).

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Harden scan ingest and the occupancy-grid contract with asserted resolution and extent.
2. Add outlier/noise removal so downstream products are not corrupted.
3. Add normal estimation and ground/non-ground segmentation.
4. Derive elevation (DSM/DTM) and canopy-height products with georeferencing.
5. Add clustering/segmentation for obstacles and canopy objects.
6. Publish elevation/obstacle overlays to the viewer (`08`) and canopy-height to the advisor (`09`).

## Primary Crates

`lidar_mapper`, `sensor_overlay_engine` (LiDAR overlay portions), with `shared` for config and `LidarScan`/`LidarPoint` schemas. Consumes captures from domain `04`; the `02` simulator LiDAR feed is not implemented yet. Feeds the viewer (`08`) and agronomy (`09`).
