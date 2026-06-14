# Orthomosaic and Photogrammetry

Stitch hundreds of georeferenced drone frames into one field map: orthomosaic, DSM/DTM, and a photogrammetric point cloud via Structure-from-Motion, with reprojection error, overlap, and GSD inspectable before any downstream use.

## Where We Are

- Not started as a pipeline. This is the core drone-ag primitive with no home today: capture exists **upstream** (frames with EXIF/GPS/IMU pose come from `04` `sensor_collector`/`data_collector`), and the GIS hub `07` can serve tiled rasters — but nothing stitches frames into a mosaic.
- Downstream domains already assume a mosaic exists: `05` should compute indices on the field mosaic (not single frames), `06` LiDAR/3D overlaps the DSM/point cloud, and `07` serves the orthomosaic as a layer. The seam between `04`→`05`→`07` is empty.
- Photogrammetry is geospatially unforgiving: a mosaic with the wrong CRS, extent, or georeferencing is worse than none, because every index and finding built on it inherits the error.

## Where We Should Be

- A reconstruction job ingests a set of frames with camera pose, runs feature detection/matching and bundle adjustment, and produces an orthomosaic, DSM, DTM, and point cloud with a known CRS and extent.
- Deterministic quality products — reprojection error, overlap percentage, ground sample distance (GSD), coverage fraction, and Ground Control Point (GCP) residuals — run and are inspectable before any downstream index or AI use consumes the mosaic.
- Tiled outputs are handed to `07` for serving, with provenance (frames, camera model, GCPs, parameters) recorded via `30` so a mosaic can be defended and re-derived.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0/P1 slices.

## Build Order

1. Frame ingest with EXIF/GPS/IMU camera pose, linked to scene/field/season via `10`.
2. Feature detection and matching across overlapping frames.
3. Bundle adjustment / SfM sparse then dense reconstruction.
4. Orthorectification and mosaicking with seamlines and color/exposure balancing.
5. DSM and DTM generation from the dense reconstruction.
6. Deterministic QA products (reprojection error, overlap %, GSD, coverage) and GCP registration, then tiled output handed to `07` and provenance via `30`.

## Primary Crates

New `orthomosaic` (a.k.a. `photogrammetry`) crate, with `shared` for schemas. Consumes frames from `04`, feeds the index workbench `05` and 3D mapping `06`, is served as a layer by `07`, draws field context from `10`, and records provenance via `30`.
