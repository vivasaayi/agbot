# Orthomosaic and Photogrammetry: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (geospatial correctness first, then performance and scale, data quality, explainability and trust, agronomic value) and the workstreams in `release-plan.md`. Because this is a greenfield pipeline domain (M0 named) built on real upstream capture, every capability's current source status is "missing (greenfield)" while the inputs (frames from `04`) and the serving surface (`07`) already exist; the Primary First Slice describes the M1 foundation step. The geospatial-correctness and performance pillars dominate: a wrong overlay is worse than none, and reconstruction runs over hundreds of frames. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Orthomosaic and Photogrammetry Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Frame ingest with EXIF/GPS/IMU camera pose | missing (greenfield) | 8 | Ingest a frame set with pose linked to scene/field via `04`/`10` |
| Feature detection and matching | missing (greenfield) | 8 | Detect/match features across overlapping frames |
| Bundle adjustment / SfM (sparse + dense) | missing (greenfield) | 9 | Sparse reconstruction with per-camera reprojection error |
| Orthorectification and mosaicking | missing (greenfield) | 9 | Orthorectify and mosaic frames into one georeferenced raster |
| Seamlines and color/exposure balancing | missing (greenfield) | 7 | Blend seamlines and balance exposure across frames |
| DSM generation | missing (greenfield) | 7 | Build a digital surface model from the dense reconstruction |
| DTM generation | missing (greenfield) | 6 | Derive a bare-earth digital terrain model from the DSM |
| GCP registration and geolocation accuracy | missing (greenfield) | 8 | Register Ground Control Points and report residuals |
| GSD and coverage/overlap QA | missing (greenfield) | 7 | Compute GSD, overlap %, and coverage fraction |
| Reprojection-error reporting | missing (greenfield) | 6 | Per-point/per-camera reprojection error with thresholds |
| Tiled output handed to `07` | missing (greenfield) | 6 | Emit tiled raster + DSM with CRS/extent for `07` serving |
| Mosaic quality and coverage report | missing (greenfield) | 6 | A defensible quality report citing all QA metrics |
