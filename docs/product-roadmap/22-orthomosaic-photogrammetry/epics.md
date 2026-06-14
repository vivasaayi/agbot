# Orthomosaic and Photogrammetry: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: reconstruction submit/status/result routes or commands with pagination and audit IDs; frame-set and mosaic identity linked to scene/field via `04`/`10`.
- Deterministic: feature matching, bundle adjustment, orthorectification, DSM/DTM, and QA metrics computed without AI, with reason codes and raw evidence (reprojection residuals, GCP residuals, overlap) retained.
- Geospatial: every output asserts CRS, extent, and resolution; georeferenced products round-trip; nothing publishes whose georeferencing cannot be proven.
- Performance: holds up over hundreds of frames; large rasters and point clouds are tiled and stream-friendly.
- Explainability: the mosaic quality/coverage report cites GSD, overlap, reprojection error, and GCP residuals; uncertainty is flagged.
- Agronomic: outputs tie to a downstream product — feed `05` indices, `06` 3D — or to a field action (re-fly when coverage/quality fails).
- Tests: unit (matching/bundle-adjustment/QA math), fixture (sample frame set with known pose), API contract, geospatial round-trip, and one failure path (insufficient overlap / degenerate set).
- Operations: job health, retry/backoff, provenance via `30`, and a runbook.

## Category Epics

### EPIC-01: Frames to Reconstruction
- Goal: turn a frame set with camera pose into a sparse then dense reconstruction with inspectable reprojection error.
- First release: frame ingest with EXIF/GPS/IMU pose, feature detection/matching, and sparse SfM with per-camera reprojection error.
- Expansion: dense reconstruction and the photogrammetric point cloud.
- Hardening: large-frame-count performance, reprojection-error thresholds, and a degenerate/insufficient-overlap failure path.

### EPIC-02: Orthomosaic, DSM, and DTM
- Goal: a single georeferenced field map plus surface and terrain models, with correct CRS/extent.
- First release: orthorectification and mosaicking into one georeferenced raster, with asserted CRS/extent.
- Expansion: seamline blending, color/exposure balancing, DSM generation, and DTM (bare-earth) derivation.
- Hardening: tiled output handed to `07`, geospatial round-trip tests, and overlay-correctness assertions.

### EPIC-03: Accuracy, QA, and Provenance
- Goal: prove the mosaic is trustworthy before any downstream index, 3D, or AI step uses it.
- First release: GSD, overlap %, and coverage-fraction QA plus a mosaic quality report citing them.
- Expansion: Ground Control Point (GCP) registration with geolocation-accuracy residuals.
- Hardening: provenance via `30` (frames, camera model, GCPs, parameters, version), re-derivability, and a re-fly recommendation when QA fails.
