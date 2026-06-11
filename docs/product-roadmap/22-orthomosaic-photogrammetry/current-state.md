# Orthomosaic and Photogrammetry: Current State and Target State

## Mission

Turn hundreds of georeferenced drone frames into one trustworthy field map: ingest frames with camera pose, reconstruct geometry via Structure-from-Motion, orthorectify and mosaic into a single georeferenced raster, derive DSM/DTM and a point cloud, and prove the result with deterministic quality products (reprojection error, overlap, GSD, coverage, GCP residuals) before any index, 3D, or AI step consumes it.

## Current Maturity

greenfield pending (M0 named): no stitching pipeline exists. This is the core drone-ag primitive that currently has no home, implied in the seam between `04` (capture) → `05` (indices) → `07` (serving). Capture upstream is real, and `07` can serve tiled rasters, but nothing reconstructs a field mosaic, DSM/DTM, or photogrammetric point cloud.

## What Exists Now

- No reconstruction is built for this domain. There is no `orthomosaic`/`photogrammetry` crate, no feature-matching, bundle-adjustment, orthorectification, or DSM/DTM code.
- Adjacent surfaces it builds on and feeds (already partially real):
  - Domain `04` (sensor acquisition): `sensor_collector`/`data_collector` produce the frames — image data with EXIF/GPS/IMU camera pose — that are this domain's only input.
  - Domain `05` (imagery / remote sensing): the index workbench that should compute NDVI and other indices on the **field mosaic**, not on disconnected single frames.
  - Domain `06` (LiDAR mapping / 3D): the DSM and photogrammetric point cloud overlap and cross-check the LiDAR surface and 3D map.
  - Domain `07` (GIS hub): the tile-serving surface that publishes the orthomosaic and DSM as georeferenced layers.
  - Domain `10` (field/farm data) and `30` (provenance/audit ledger): scene/field/season linkage and the provenance record (frames, camera model, GCPs, parameters) for each mosaic.

## Gaps to Close

- No frame-ingest contract that reads EXIF/GPS/IMU pose and links a frame set to scene/field/season via `04`/`10`.
- No feature detection and matching across overlapping frames.
- No bundle adjustment / SfM producing a sparse then dense reconstruction with per-camera reprojection error.
- No orthorectification or mosaicking into a single georeferenced raster, and no seamline blending or color/exposure balancing.
- No DSM generation from the dense reconstruction, and no DTM (bare-earth) derivation.
- No Ground Control Point (GCP) registration or geolocation-accuracy residual reporting.
- No deterministic QA: ground sample distance (GSD), overlap percentage, coverage fraction, and reprojection-error thresholds.
- No tiled output handoff to `07` with asserted CRS/extent, and no mosaic quality/coverage report or provenance record via `30`.

## Source Modules Reviewed

- `sensor_collector` / `data_collector` (`04`): frame capture, EXIF/GPS/IMU metadata, storage — the input boundary.
- `geo_hub` (`07`): tile serving and raster layer model — the output boundary for the mosaic/DSM.
- `shared/src/schemas.rs`: scene/field/season records this domain links a frame set and mosaic to.
- `docs/reference/product-requirements.md`: the field-to-decision pipeline that assumes a stitched mosaic between capture and indices.

## Target Operating Model

- A reconstruction job is a first-class entity with identity, linked to a frame set, scene, field, and season via `04`/`10`, with a clear lifecycle (`Queued→Reconstructing→Orthorectifying→Completed|Failed`).
- Geospatial correctness is the dominant pillar: every output asserts CRS, extent, and resolution; georeferenced products round-trip; a mosaic whose CRS/extent cannot be proven correct is never published.
- Deterministic quality products run and are inspectable before any downstream use: reprojection error, overlap %, GSD, coverage fraction, and GCP residuals, each with thresholds and reason codes.
- Performance and scale are explicit: reconstruction holds up over hundreds of frames; large rasters and dense point clouds are tiled and stream-friendly for `07`.
- Tiled orthomosaic and DSM are handed to `07` for serving; provenance (frames, camera model, GCPs, parameters, software version) is recorded via `30` so a mosaic can be defended and re-derived.
- Every output ties to a downstream product: the mosaic feeds `05` indices, the DSM/point cloud feeds `06` 3D, and the quality report tells an operator whether the field needs a re-fly.
