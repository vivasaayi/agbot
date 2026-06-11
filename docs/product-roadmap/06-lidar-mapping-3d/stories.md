# LiDAR Mapping and 3D Reconstruction: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI. Occupancy and elevation grids must assert CRS/extent/resolution and round-trip for the viewer; every product retains thresholds and observation counts.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

Real crates: `lidar_mapper` (`lib.rs`, `main.rs` — RPLIDAR JSON ingest, `GridCell` occupancy, PCD/heatmap export, occupancy unit tests), `sensor_overlay_engine/src/lidar_overlay.rs` (`LidarOverlayProcessor`, height coloring, `LidarElevation`/`LidarIntensity`), `shared/src/schemas.rs` (`LidarScan`, `LidarPoint`, `RasterSpatialRef`). Occupancy/PCD/heatmap are real; cleaning, normals, segmentation, DSM/DTM, and meshing are the gaps.

---

## M1 — Foundation

### STORY 06-01 · M1 · S · P0 — Scan ingest and parallel loading
- **Story**: As `DSP`, I want RPLIDAR JSON scans ingested with freshness and coverage tracking, so that every scan is identified and its completeness is visible before products are derived.
- **Deterministic / evidence**: parallel ingest (futures stream, 8 workers) into `LidarScan`/`LidarPoint`; persist `{scan_id, captured_at, ingested_at, point_count, angular_coverage}`.
- **Acceptance**:
  - Given a directory of RPLIDAR JSON scans, when ingest runs, then each scan is loaded into `LidarScan` with point count and angular coverage recorded.
  - Given a malformed scan file, when ingest runs, then that file is rejected with a parse error and the remaining scans still load (no whole-batch failure).
- **Tests**: unit (scan parse), fixture (`04` scans), failure path (malformed scan skipped with error).
- **Depends on**: `04` (captured scans), `shared` schemas.

### STORY 06-02 · M1 · M · P0 — Occupancy grid construction with asserted extent
- **Story**: As `OPS`, I want a configurable-resolution occupancy grid whose extent is asserted, so that obstacle maps are provably placed on the right ground.
- **Deterministic / evidence**: `GridCell` tracks obstacle and total-observation counts against an occupancy threshold; populate `RasterSpatialRef` (extent, resolution) and assert `extent == origin + dims·resolution`.
- **Acceptance**:
  - Given scans and a resolution, when the grid is built, then each cell carries obstacle/total counts and the grid extent is asserted consistent with dims·resolution.
  - Given a zero or negative resolution, when grid build runs, then it is rejected with a resolution error rather than producing a degenerate grid.
- **Tests**: unit (occupancy threshold, cell counts — existing coverage), geospatial (extent↔dims↔resolution), failure path (non-positive resolution).
- **Depends on**: 06-01, `shared` `RasterSpatialRef`.

### STORY 06-03 · M1 · S · P1 — Occupancy threshold and Y-flip controls
- **Story**: As `OPS`, I want overridable distance/quality/occupancy thresholds and Y-flip from the CLI, so that I can tune the grid to the sensor and frame convention.
- **Deterministic / evidence**: CLI thresholds applied during grid build and recorded in product evidence; Y-flip applied consistently to points and extent.
- **Acceptance**:
  - Given a raised occupancy threshold, when the grid is built, then fewer cells are marked occupied and the threshold is recorded.
  - Given Y-flip enabled, when the grid is built, then points and extent are flipped consistently so the overlay is not mirrored.
- **Tests**: unit (threshold effect, Y-flip consistency), failure path (out-of-range threshold rejected).
- **Depends on**: 06-02.

### STORY 06-04 · M1 · S · P1 — PCD v0.7 export with provenance
- **Story**: As `DSP`, I want point clouds exported as PCD v0.7 ASCII with provenance metadata, so that clients can load clouds in standard tools with traceability.
- **Deterministic / evidence**: write PCD v0.7 ASCII from `LidarPoint`s; embed `{scan_ids[], captured_at, point_count, frame/CRS note}` in a sidecar or header comment.
- **Acceptance**:
  - Given an ingested scan set, when PCD export runs, then a valid PCD v0.7 file with the correct point count and provenance is written.
  - Given an empty scan set, when export runs, then a valid empty PCD (header only) is produced, not a corrupt file.
- **Tests**: unit (PCD header/fields), fixture (round-trip read of point count), failure path (empty cloud → valid empty PCD).
- **Depends on**: 06-01.

### STORY 06-05 · M1 · S · P1 — Obstacle heatmap
- **Story**: As `AG`, I want a blue→red obstacle heatmap with explicit thresholds, so that I can see where obstacles concentrate at a glance.
- **Deterministic / evidence**: map occupancy density to a blue→red ramp with recorded thresholds; the heatmap shares the occupancy grid extent.
- **Acceptance**:
  - Given an occupancy grid, when the heatmap renders, then density maps to the blue→red ramp over the same extent with the thresholds recorded.
  - Given an all-empty grid, when the heatmap renders, then it is uniformly low (no false hotspots).
- **Tests**: unit (density→color, threshold record), failure path (empty grid → no hotspots).
- **Depends on**: 06-02.

---

## M2 — Captured / Observable

### STORY 06-06 · M2 · M · P0 — Outlier and noise removal
- **Story**: As `AG`, I want statistical outlier removal applied before any product is derived, so that corrupt geometry does not produce wrong findings.
- **Deterministic / evidence**: statistical outlier removal (e.g. k-nearest mean-distance filter) over the raw cloud; record `{points_in, points_removed, params}` as evidence; cleaning runs before occupancy/elevation derivation.
- **Acceptance**:
  - Given a cloud with injected far-outlier points, when cleaning runs, then those points are removed and the removed count is recorded.
  - Given a clean cloud, when cleaning runs, then zero points are removed (no false trimming of valid geometry).
- **Tests**: unit (outlier filter on synthetic cloud), fixture (clean cloud → 0 removed), failure path (degenerate <k points handled, no crash).
- **Depends on**: 06-01.

### STORY 06-07 · M2 · S · P1 — Coverage and density tracking
- **Story**: As `OPS`, I want per-scan point density and spatial coverage tracked, so that sparse or partial captures are flagged before mapping.
- **Deterministic / evidence**: compute points-per-cell density and covered-cell fraction over the grid extent; flag below a configurable coverage floor.
- **Acceptance**:
  - Given a dense full-coverage scan set, when coverage runs, then density and coverage fraction are recorded and pass the floor.
  - Given a sparse scan set, when coverage runs, then it is flagged low-coverage and surfaced, not silently mapped.
- **Tests**: unit (density + coverage fraction), failure path (below-floor coverage flagged).
- **Depends on**: 06-02, 06-06.

---

## M3 — Explainable (deterministic geometry — georeferencing the trust foundation)

### STORY 06-08 · M3 · M · P0 — Normal estimation
- **Story**: As `OPS`, I want per-point surface normals estimated, so that surface orientation and downstream ground/non-ground separation are possible.
- **Deterministic / evidence**: estimate per-point normals from a local neighborhood (e.g. PCA over k-nearest neighbors); record neighborhood size; normals are deterministic for a fixed neighborhood.
- **Acceptance**:
  - Given a planar patch, when normal estimation runs, then estimated normals are consistent and orthogonal to the plane within tolerance.
  - Given a point with fewer than k neighbors, when estimation runs, then that point is marked undefined-normal rather than producing a garbage vector.
- **Tests**: unit (normals on synthetic plane), failure path (insufficient neighbors → undefined).
- **Depends on**: 06-06.

### STORY 06-09 · M3 · M · P0 — Ground/non-ground segmentation
- **Story**: As `AG`, I want the ground plane separated from canopy and obstacles, so that elevation and canopy products rest on a real terrain surface.
- **Deterministic / evidence**: classify points as ground vs non-ground (e.g. progressive morphological / slope filter) using normals from 06-08; record parameters and per-class counts.
- **Acceptance**:
  - Given a cloud over sloped terrain with canopy, when segmentation runs, then ground and non-ground classes are assigned with per-class counts recorded.
  - Given an all-canopy cloud with no ground returns, when segmentation runs, then it reports "no ground surface" rather than misclassifying canopy as ground.
- **Tests**: unit (segmentation on synthetic terrain+canopy), failure path (no ground returns).
- **Depends on**: 06-08.

### STORY 06-10 · M3 · M · P1 — Clustering and segmentation of objects
- **Story**: As `AG`, I want non-ground points clustered into obstacle and canopy objects, so that I can isolate individual obstacles and canopy features for agronomy and safety.
- **Deterministic / evidence**: Euclidean/region clustering over non-ground points; each cluster gets `{id, point_count, bbox, centroid}`; min-cluster-size recorded.
- **Acceptance**:
  - Given two spatially separated object groups, when clustering runs, then exactly two clusters with bounding boxes and centroids are produced.
  - Given noise below the min-cluster size, when clustering runs, then it is dropped, not emitted as spurious objects.
- **Tests**: unit (cluster count + bbox on synthetic groups), failure path (sub-threshold noise dropped).
- **Depends on**: 06-09.

### STORY 06-11 · M3 · L · P0 — DSM/DTM elevation products
- **Story**: As `OPS`, I want the cleaned cloud rasterized into georeferenced DSM and DTM grids, so that elevation surfaces round-trip correctly for the viewer.
- **Deterministic / evidence**: rasterize all returns → DSM and ground returns → DTM at a set resolution; populate and assert `RasterSpatialRef` (CRS/extent/resolution/transform); empty cells carry nodata.
- **Acceptance**:
  - Given a segmented cloud and a resolution, when DSM/DTM build, then both rasters are produced with asserted CRS/extent/resolution and round-trip on write→read within tolerance.
  - Given a region with no ground returns, when the DTM builds, then those cells are nodata, not interpolated as zero elevation.
- **Tests**: unit (rasterization), geospatial round-trip (write→read CRS/extent equality), failure path (no-ground cells → nodata).
- **Depends on**: 06-09, `shared` `RasterSpatialRef`.

### STORY 06-12 · M3 · M · P1 — 3D reconstruction and meshing
- **Story**: As `GR`, I want the cleaned cloud meshed into a terrain surface, so that I can view the field in 3D in the viewer.
- **Deterministic / evidence**: build a deterministic surface mesh (e.g. 2.5D Delaunay over the DSM) with vertex/face counts; the mesh shares the DSM's georeferenced frame.
- **Acceptance**:
  - Given a DSM, when meshing runs, then a watertight-enough terrain mesh with recorded vertex/face counts is produced in the DSM frame.
  - Given a DSM that is entirely nodata, when meshing runs, then it produces an empty mesh with a clear "no surface" result, not a crash.
- **Tests**: unit (mesh vertex/face counts on synthetic DSM), failure path (all-nodata DSM → empty mesh).
- **Depends on**: 06-11.

### STORY 06-13 · M3 · S · P1 — Product evidence and reproducibility
- **Story**: As `DSP`, I want every LiDAR product to persist its thresholds, observation counts, and parameters, so that an obstacle or elevation finding can be re-derived and defended.
- **Deterministic / evidence**: each product stores `{scan_ids[], cleaning_params, thresholds, occupancy/observation counts, spatial_ref}`; identical inputs yield an identical output hash.
- **Acceptance**:
  - Given a product, when inspected, then it cites its source scans, thresholds, and observation counts.
  - Given identical inputs, when the pipeline re-runs, then the product is byte-identical (deterministic hash match).
- **Tests**: determinism test (same input → same hash), fixture (evidence fields present).
- **Depends on**: 06-02, 06-06, 06-11.

---

## M4 — Interactive

### STORY 06-14 · M4 · M · P1 — Canopy-height model for the advisor
- **Story**: As `AG`, I want a canopy-height model derived as DSM − DTM, so that the advisor (`09`) can flag tall/short canopy zones.
- **Deterministic / evidence**: compute CHM = DSM − DTM per cell on the shared georeferenced grid; clamp negatives to nodata; retain min/max/mean canopy height and coverage.
- **Acceptance**:
  - Given aligned DSM and DTM, when CHM runs, then a georeferenced canopy-height raster with stats is produced and shares their CRS/extent.
  - Given DSM and DTM on mismatched extents, when CHM runs, then it is rejected with an extent-mismatch error rather than differencing misaligned grids.
- **Tests**: unit (CHM math, negative→nodata), geospatial (shared-extent assertion), failure path (extent mismatch).
- **Depends on**: 06-11, `09` (consumes canopy product).

### STORY 06-15 · M4 · S · P0 — Elevation/obstacle overlays for the viewer
- **Story**: As `AG`, I want elevation and obstacle overlays published to the viewer with a legend, so that I can see canopy and obstacles on the correct field.
- **Deterministic / evidence**: `LidarOverlayProcessor` renders `LidarElevation`/`LidarIntensity` and occupancy with height-based coloring and a legend; overlay carries the product `RasterSpatialRef`.
- **Acceptance**:
  - Given a georeferenced DSM/occupancy product, when overlay rendering runs, then a colored layer plus a height/density legend is produced over the asserted extent.
  - Given a product lacking georeferencing, when overlay is requested, then it is refused rather than drawn on the wrong ground.
- **Tests**: unit (height→color, legend), geospatial (extent carried), failure path (ungeoreferenced → refused).
- **Depends on**: 06-11, 06-02, `08` (viewer consumes overlays).

### STORY 06-16 · M4 · S · P1 — Product provenance and scene/field linkage
- **Story**: As `AG`, I want LiDAR products linked to scene/field/season, so that the advisor and viewer can trace a canopy or obstacle finding to the right field and flight.
- **Deterministic / evidence**: persist `{product_id, scene_id, field_id, season_id, product_kind, spatial_ref, scan_ids[]}`; publish to `07` for viewer fetch.
- **Acceptance**:
  - Given a scan set linked to a field/season, when a product is built, then it carries all linkage IDs and is retrievable by the viewer.
  - Given a scan set with no field linkage, when publish runs, then it is rejected with an unlinked error rather than orphaning the product.
- **Tests**: API/contract (publish + fetch), unit (linkage fields), failure path (unlinked scan set).
- **Depends on**: 06-11, `07`, `09`.

### STORY 06-17 · M4 · S · P1 — Elevation/CHM export (GeoTIFF + PCD)
- **Story**: As `DSP`, I want DSM/DTM/CHM exported as GeoTIFF and cleaned clouds as PCD, so that clients can use elevation products in other tools.
- **Deterministic / evidence**: export georeferenced GeoTIFF (round-trip from 06-11) plus PCD (06-04); both validate against a schema and carry provenance.
- **Acceptance**:
  - Given a DSM/CHM, when export runs, then the GeoTIFF round-trips its CRS/extent and validates against the schema.
  - Given an empty product, when export runs, then a valid empty export is produced, not a corrupt file.
- **Tests**: geospatial round-trip, schema validation, failure path (empty product → valid export).
- **Depends on**: 06-11, 06-13.

---

## M5 — Autonomous-Assist (gated)

### STORY 06-18 · M5 · M · P1 — Obstacle-change advisory across flights
- **Story**: As `AG`, I want obstacle/canopy changes flagged across two flights with an uncertainty band, so that I can spot new hazards or growth without manual comparison — once segmentation is reliable.
- **Deterministic / evidence**: align two georeferenced occupancy/CHM products over a common extent; compute per-cell change with coverage; flag uncertainty on CRS/extent/resolution mismatch; feature-flagged and gated behind reliable segmentation.
- **Acceptance**:
  - Given two comparable georeferenced flights of one field, when change detection runs, then a change layer plus stats and an uncertainty band are produced over the common extent.
  - Given mismatched CRS/extent or unreliable segmentation, when change detection runs, then it is marked low-confidence or unavailable, never silently differenced.
- **Tests**: unit (change + alignment), gating test (mismatch → low-confidence), failure path (segmentation unavailable → disabled).
- **Depends on**: 06-09, 06-11, 06-14, `09`, `03` (safety work).

---

## Coverage note

These 18 stories cover all 12 capabilities in `capability-map.md`, ordered by phase and consistent with `release-plan.md` (M1 15 / M2 13 / M3 21 / M4 14 / M5 6; P0 29 / P1 27 / P2 13). Cleaning (06-06) runs before any product is derived, and every occupancy/elevation product asserts CRS/extent/resolution and round-trips (06-02, 06-11) before the viewer (`08`) or advisor (`09`) trusts it. M5 obstacle-change advisories (06-18) are gated behind reliable segmentation and the `03` safety work and the `02` LiDAR sim. The curated ~69 backlog rows expand several stories into siblings — additional cleaning filters in 06-06, per-method clustering in 06-10, and mesh formats in 06-12 — when implemented.
