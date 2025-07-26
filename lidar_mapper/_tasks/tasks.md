# LiDAR Mapper Enhancement Tasks

This document tracks the tasks needed to implement the planned enhancements for the `lidar_mapper` crate.

## 1. Wire CLI Overrides into `main.rs`
- **Completed**: Wired CLI flags into `main.rs` by calling `LidarMapper::new(&args)`.

## 2. Parallel Scan Loading
- **Completed**: Implemented parallel loading via `futures::stream::buffer_unordered`, with success/failure tracking.

## 3. Progress Reporting
- **Completed**: Integrated progress bar via `indicatif` crate during scan loading operations.

## 4. Unit & Integration Tests
- Add unit tests for `create_occupancy_grid()` with small synthetic scans.
- Add integration tests that run `process_directory()` over sample data and validate outputs.

## 5. Output Format Options
- Extend CLI to allow choosing output formats (PNG vs GeoTIFF for grids, CSV/LAS for point clouds).

## 6. Error Summary File
- Write a summary JSON or text file listing failed scan filenames and error messages in `output_dir`.

## 7. Grid Bounds & Origin Control
- Add CLI/config options for explicit grid origin and extents instead of auto-fitting to data.

## 8. 3D LiDAR Support
- Update point cloud and grid logic to handle non-zero Z coordinates when input includes elevation.

## 9. Clean Up Warnings
- Remove unused imports (`warn`, commented-out futures import) and address any new dead code warnings.

## 10. Documentation & Examples
- Update `README.md` with usage examples, CLI flag descriptions, and sample outputs.

---
All tasks are currently _Pending_.
