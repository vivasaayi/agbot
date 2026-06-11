# LiDAR Mapping and 3D Reconstruction: Current State and Target State

## Mission

Turn LiDAR captures into deterministic occupancy, elevation, obstacle, and 3D products — occupancy grids, point clouds, canopy-height and terrain — that the viewer renders and the advisor uses for obstacle and canopy findings.

## Current Maturity

strong partial: `lidar_mapper` has a working scan-ingest → occupancy-grid → export pipeline with real test coverage on the occupancy logic, but lacks point-cloud cleaning, normal estimation, segmentation, and true 3D reconstruction.

## What Exists Now

- RPLIDAR JSON scan loading with parallel ingest (futures stream, 8 workers) and a progress bar (`lidar_mapper/src/lib.rs`, ~406 lines).
- Configurable-resolution occupancy grids: `GridCell` tracks obstacle and total-observation counts with an occupancy threshold.
- CLI with overridable distance/quality/occupancy thresholds and Y-flip (`lidar_mapper/src/{lib.rs,main.rs}`).
- Occupancy-grid PNG and obstacle heatmap (blue → red) output.
- PCD v0.7 ASCII point-cloud export.
- Unit test coverage for the occupancy logic.
- `LidarOverlayProcessor` in `sensor_overlay_engine` with height-based coloring, occupancy-grid rendering, and `LidarElevation`/`LidarIntensity` overlays (`sensor_overlay_engine/src/lidar_overlay.rs`, ~499 lines).

## Gaps to Close

- No outlier/noise removal — raw scans flow straight into products.
- No normal estimation, so surface orientation and ground/non-ground separation are unavailable.
- No clustering or segmentation (obstacles, canopy objects, ground plane).
- No true 3D reconstruction: no DSM/DTM, no meshing, no canopy-height model.
- Georeferencing (CRS/extent/resolution) of occupancy and elevation grids is not asserted or round-tripped.
- The `02` simulator LiDAR feed that would drive this in simulation is not implemented yet, so real captures from `04` are the only input.
- No canopy-height or obstacle product handoff to the advisor (`09`).

## Source Modules Reviewed

- `lidar_mapper/src/lib.rs`, `lidar_mapper/src/main.rs`
- `sensor_overlay_engine/src/lidar_overlay.rs`
- `sensor_overlay_engine/src/{lib.rs,composite.rs}` (overlay trait, IDW grid interpolation)
- `shared/src/schemas.rs` (`LidarScan`, `LidarPoint`, `RasterSpatialRef`)

## Target Operating Model

- Scans ingested with freshness and coverage tracking, then cleaned (outlier/noise removal) before any product is derived.
- Occupancy and elevation grids with asserted CRS, extent, and resolution that round-trip for the viewer.
- Normal estimation and ground/non-ground segmentation feeding a DSM/DTM and a canopy-height model.
- Clustering/segmentation that isolates obstacles and canopy objects for agronomy and safety.
- Elevation/obstacle overlays published to the viewer (`08`); canopy-height products handed to the advisor (`09`).
- Simulation-first once the `02` LiDAR sim lands, with the occupancy and elevation math unit-tested on fixtures.
