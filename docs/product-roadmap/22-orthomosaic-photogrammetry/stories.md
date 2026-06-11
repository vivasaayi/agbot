# Orthomosaic and Photogrammetry: Detailed Stories

> Greenfield pipeline domain (M0 named): no stitching code exists yet, but the **inputs are real** — frames with EXIF/GPS/IMU pose come from `04`, and `07` already serves tiled rasters. Every story below is **built from scratch** in a new `orthomosaic` crate and sits in the empty seam between `04`→`05`→`07`. The **geospatial-correctness pillar dominates every phase**: a mosaic with the wrong CRS, extent, or georeferencing is worse than none, because every index and finding built on it inherits the error. Deterministic quality products (reprojection error, overlap, GSD, coverage, GCP residuals) must be inspectable before any downstream index, 3D, or AI use. The single M5 story stays advisory and approval-gated.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 22-01 · M1 · S · P0 — Frame-set ingest with camera pose
- **Story**: As `DSP`, I want to ingest a set of frames with EXIF/GPS/IMU camera pose linked to a scene, field, and season, so that a reconstruction has a known, owned, traceable input.
- **Deterministic / evidence**: persist `{frame_set_id, scene_id, field_id, season_id, frames[{frame_id, gps, imu, exif, capture_ts}], crs_hint, created_at}`; read pose from EXIF/GPS/IMU produced by `04`; reject a set with no positional metadata.
- **Acceptance**:
  - Given frames from `04` with GPS/IMU pose, when ingested, then a frame-set record is created with per-frame pose and `04`/`10` linkage.
  - Given frames missing all positional metadata, when ingested, then the set is rejected (4xx) with a "no camera pose" reason code (no silent default to 0,0).
- **Tests**: unit (EXIF/GPS/IMU parse), API contract (ingest/list), failure path (no-pose set rejected).
- **Depends on**: `04` (frame capture), `10` (scene/field/season model).

### STORY 22-02 · M1 · S · P0 — Reconstruction job identity and lifecycle
- **Story**: As `DSP`, I want each reconstruction to have a stable ID and lifecycle linked to its frame set, so that mosaics are traceable and reproducible.
- **Deterministic / evidence**: persist `{recon_id, frame_set_id, params, status, created_at}`; lifecycle `Queued→Reconstructing→Orthorectifying→Completed|Failed`; a failure records a reason code and is retrievable.
- **Acceptance**:
  - Given a valid frame set, when a reconstruction is submitted, then a job is created with `Queued` status and parameter record.
  - Given a job that errors, when it fails, then status is `Failed` with a reason code and the error is retrievable.
- **Tests**: unit (state transitions), API contract (submit/status/result), failure path (submit with unknown frame_set_id → 4xx).
- **Depends on**: 22-01.

---

## M2 — Captured / Observable

### STORY 22-03 · M2 · M · P0 — GSD, overlap, and coverage QA on the frame set
- **Story**: As `DSP`, I want ground sample distance, forward/side overlap, and coverage fraction computed from the frame set before reconstruction, so that I know up front whether the data can stitch.
- **Deterministic / evidence**: from camera pose and intrinsics, compute GSD per frame, pairwise overlap %, and the coverage fraction of the field boundary; flag frames/regions below an overlap threshold with reason codes.
- **Acceptance**:
  - Given a frame set with adequate overlap, when QA runs, then GSD, overlap %, and coverage fraction are returned and tied to the field extent.
  - Given a frame set with a gap (insufficient overlap), when QA runs, then the gap region is flagged with a reason code (not hidden) and coverage reflects the missing area.
- **Tests**: unit (GSD/overlap/coverage math), fixture (frame set with a known gap), failure path (gap flagged, not masked).
- **Depends on**: 22-01, `10` (field boundary).

### STORY 22-04 · M2 · S · P1 — Reconstruction progress and coverage observability
- **Story**: As `OPS`, I want live reconstruction progress with per-stage coverage and freshness, so that a long stitch run is observable rather than opaque.
- **Deterministic / evidence**: emit per-stage progress `{matched_frames, registered_cameras, dense_points, stage}` with timestamps; record stalls/gaps as events; analog of `04`/`01` capture observability.
- **Acceptance**:
  - Given a running reconstruction, when stages progress, then per-stage counts and coverage stream with timestamps.
  - Given a stalled stage, when no progress arrives within a window, then a stall event is recorded and flagged (not silently retried forever).
- **Tests**: fixture (staged progress stream), unit (stall detection), failure path (stall flagged).
- **Depends on**: 22-02.

---

## M3 — Explainable (the deterministic reconstruction and QA core)

### STORY 22-05 · M3 · M · P0 — Feature detection and matching
- **Story**: As `DSP`, I want features detected and matched across overlapping frames, so that the reconstruction has the tie points it needs.
- **Deterministic / evidence**: detect keypoints per frame and match across overlapping pairs; retain match counts and inlier ratios per pair; deterministic given the same frames and parameters.
- **Acceptance**:
  - Given overlapping frames, when matching runs, then matched pairs carry inlier counts and the match graph connects the set.
  - Given two frames with no real overlap, when matching runs, then few/zero inlier matches are reported (no fabricated matches) and the pair is left unconnected.
- **Tests**: unit (match + inlier ratio), fixture (overlapping pair), failure path (non-overlapping pair → no false matches).
- **Depends on**: 22-01, 22-02.

### STORY 22-06 · M3 · L · P0 — Bundle adjustment and sparse SfM
- **Story**: As `DSP`, I want a sparse Structure-from-Motion reconstruction with per-camera reprojection error, so that camera poses and tie points are solved and inspectable.
- **Deterministic / evidence**: run bundle adjustment over the match graph to estimate camera poses and a sparse point cloud; emit per-camera and per-point reprojection error; converge or fail explicitly (no partial garbage published).
- **Acceptance**:
  - Given a connected match graph, when bundle adjustment runs, then camera poses and a sparse cloud are produced with per-camera reprojection error within a configured threshold.
  - Given a disconnected/under-constrained graph (insufficient overlap), when bundle adjustment runs, then it fails with a "could not solve" reason code instead of emitting an unconstrained solution.
- **Tests**: unit (reprojection-error math), fixture (synthetic scene with known poses), failure path (under-constrained graph fails cleanly).
- **Depends on**: 22-05.

### STORY 22-07 · M3 · L · P1 — Dense reconstruction and point cloud
- **Story**: As `AG`, I want a dense photogrammetric point cloud from the sparse reconstruction, so that the DSM and 3D overlap with `06` have real geometry.
- **Deterministic / evidence**: densify from the solved poses into a point cloud in the reconstruction CRS; retain point count and density; assert CRS/extent.
- **Acceptance**:
  - Given a converged sparse reconstruction, when densification runs, then a point cloud is produced in the correct CRS with reported density and extent.
  - Given a sparse reconstruction that did not converge, when densification is requested, then it is refused (no densifying an invalid pose set).
- **Tests**: unit (density/extent), geospatial (CRS round-trip), failure path (densify refused on unconverged input).
- **Depends on**: 22-06.

### STORY 22-08 · M3 · L · P0 — Orthorectification and mosaicking
- **Story**: As `AG`, I want frames orthorectified and mosaicked into one georeferenced raster, so that the whole field is a single correct map instead of disconnected photos.
- **Deterministic / evidence**: orthorectify each frame using solved poses and the surface model, then mosaic into one raster; assert and persist CRS, extent, and resolution; the mosaic round-trips to its georeferenced extent.
- **Acceptance**:
  - Given solved poses and a surface, when orthorectification runs, then a single georeferenced raster is produced with asserted CRS, extent, and resolution that round-trip.
  - Given an inconsistent/unsolved pose set, when mosaicking is requested, then it is refused with a georeferencing-error reason (a wrong overlay is never published).
- **Tests**: unit (orthorectification transform), geospatial (CRS/extent round-trip), failure path (unsolved poses refused).
- **Depends on**: 22-06, 22-07.

### STORY 22-09 · M3 · M · P1 — Seamlines and color/exposure balancing
- **Story**: As `AG`, I want seamlines blended and exposure balanced across frames, so that the mosaic reads as one image without visible seams or brightness jumps.
- **Deterministic / evidence**: compute seamlines that minimize visible transitions and apply per-frame color/exposure correction; the balancing is geometry-preserving (pixels stay georeferenced, only radiometry changes) and recorded.
- **Acceptance**:
  - Given an orthomosaic with overlapping frames, when balancing runs, then seamlines and exposure correction are applied and the per-frame georeferencing is unchanged.
  - Given a frame with a corrupt/extreme exposure, when balancing runs, then it is flagged rather than allowed to distort the whole mosaic.
- **Tests**: unit (seamline + exposure math), fixture (uneven-exposure set), failure path (outlier frame flagged).
- **Depends on**: 22-08.

### STORY 22-10 · M3 · M · P0 — DSM generation
- **Story**: As `AG`, I want a digital surface model from the dense reconstruction, so that elevation and canopy height are available and overlap with `06`.
- **Deterministic / evidence**: rasterize the dense point cloud into a DSM grid in the reconstruction CRS; assert CRS/extent/resolution; retain per-cell point support and nodata mask.
- **Acceptance**:
  - Given a dense point cloud, when DSM generation runs, then a georeferenced DSM is produced with correct CRS/extent and a nodata mask where support is absent.
  - Given a region with no point support, when DSM generation runs, then those cells are nodata (not interpolated across a gap silently).
- **Tests**: unit (rasterization + nodata), geospatial (CRS round-trip), failure path (unsupported region → nodata).
- **Depends on**: 22-07.

### STORY 22-11 · M3 · M · P1 — DTM (bare-earth) generation
- **Story**: As `AG`, I want a bare-earth digital terrain model derived from the DSM, so that drainage, slope, and ground elevation are available without canopy.
- **Deterministic / evidence**: filter the DSM to ground returns and interpolate a DTM; retain the ground/non-ground classification and assert CRS/extent; cross-checkable against `06` terrain.
- **Acceptance**:
  - Given a DSM, when DTM derivation runs, then a bare-earth model is produced in the correct CRS with a recorded ground classification.
  - Given a fully canopy-covered region with no ground returns, when DTM derivation runs, then those cells are flagged low-confidence/nodata rather than fabricated.
- **Tests**: unit (ground filter + interpolation), geospatial (CRS round-trip), failure path (no-ground region flagged).
- **Depends on**: 22-10.

### STORY 22-12 · M3 · M · P0 — Reprojection-error reporting
- **Story**: As `DSP`, I want a per-camera and per-point reprojection-error report with thresholds, so that I can prove (or reject) the reconstruction's geometric accuracy.
- **Deterministic / evidence**: aggregate reprojection residuals from bundle adjustment into per-camera, per-point, and overall RMS; compare against a configured threshold; flag cameras/points that exceed it.
- **Acceptance**:
  - Given a converged reconstruction, when the report runs, then per-camera and overall RMS reprojection error are returned with pass/fail against the threshold.
  - Given a reconstruction whose RMS exceeds the threshold, when the report runs, then it is flagged failing (and downstream publish is blocked, see 22-16).
- **Tests**: unit (RMS aggregation + thresholding), fixture (known-residual scene), failure path (over-threshold flagged).
- **Depends on**: 22-06.

---

## M4 — Interactive (accuracy, handoff, and reporting)

### STORY 22-13 · M4 · M · P0 — GCP registration and geolocation accuracy
- **Story**: As `DSP`, I want to register Ground Control Points and get geolocation-accuracy residuals, so that the mosaic is anchored to surveyed truth and its absolute accuracy is known.
- **Deterministic / evidence**: ingest GCPs `{id, marked_image_points[], surveyed_coord, crs}`; constrain bundle adjustment to them; emit per-GCP horizontal/vertical residuals and an overall accuracy figure.
- **Acceptance**:
  - Given GCPs marked in images, when registration runs, then the mosaic is constrained to them and per-GCP residuals plus overall accuracy are reported in the correct CRS.
  - Given a GCP whose surveyed CRS does not match the project CRS, when registered, then it is refused with a CRS-mismatch error (no silent reprojection of a control point).
- **Tests**: unit (GCP residual math), geospatial (CRS consistency), failure path (CRS-mismatch GCP refused).
- **Depends on**: 22-06, 22-08.

### STORY 22-14 · M4 · S · P0 — Tiled output handed to `07`
- **Story**: As `OPS`, I want the orthomosaic and DSM emitted as tiles with asserted CRS/extent for `07` to serve, so that the field map appears as a correct GIS layer.
- **Deterministic / evidence**: tile the mosaic and DSM into a pyramid with metadata `{crs, extent, resolution, gsd}`; `07` ingests by reference; the served layer round-trips to the original extent.
- **Acceptance**:
  - Given a completed mosaic, when tiling and handoff run, then `07` serves a layer whose CRS/extent/resolution match the source and round-trip.
  - Given a mosaic missing CRS/extent metadata, when handoff is requested, then it is refused (no untraceable layer published to `07`).
- **Tests**: geospatial round-trip (served vs source extent), API contract (`07` handoff), failure path (missing-CRS handoff refused).
- **Depends on**: 22-08, `07`.

### STORY 22-15 · M4 · M · P1 — Mosaic quality and coverage report
- **Story**: As `AG`, I want a defensible mosaic quality report citing GSD, overlap, coverage, reprojection error, and GCP residuals, so that I can trust (or reject) the map before building indices on it.
- **Deterministic / evidence**: assemble all QA metrics into one report with pass/fail per metric and an overall verdict; the report cites every evidence source and flags any metric that failed its threshold.
- **Acceptance**:
  - Given a completed reconstruction, when the report runs, then it lists GSD, overlap %, coverage, reprojection RMS, and GCP residuals with pass/fail and an overall verdict.
  - Given any failing metric (e.g. low coverage), when the report runs, then the verdict is "not publishable" with the failing metric cited (not a blanket pass).
- **Tests**: unit (verdict assembly), golden-file (report structure), failure path (failing metric → not-publishable verdict).
- **Depends on**: 22-03, 22-12, 22-13.

### STORY 22-16 · M4 · S · P0 — Provenance and downstream publish gate
- **Story**: As `PA`, I want every mosaic to record provenance and be blocked from downstream use until QA passes, so that no index, 3D, or AI step builds on an unproven map.
- **Deterministic / evidence**: record provenance via `30` `{frames[], camera_model, gcps[], params, software_version, qa_report_ref}`; the mosaic is only marked `Published` (consumable by `05`/`06`) when the quality report verdict is publishable.
- **Acceptance**:
  - Given a mosaic with a passing quality report, when publish is requested, then it is marked `Published` and its provenance record is complete and re-derivable.
  - Given a mosaic with a failing quality report, when publish is requested, then it is blocked and `05`/`06` cannot consume it (evidence-before-advice enforced at the gate).
- **Tests**: API contract (publish gate), determinism (same inputs → same provenance hash), failure path (failing QA blocks publish).
- **Depends on**: 22-15, `30`.

---

## M5 — Autonomous-Assist (advisory, approval-gated)

### STORY 22-17 · M5 · M · P2 — Model-assisted re-fly suggestion
- **Story**: As `AG`, I want a suggested targeted re-fly when coverage or quality fails, so that I can fix the data gap instead of shipping a misleading map — without the system flying on its own.
- **Deterministic / evidence**: the deterministic QA (coverage gaps from 22-03, failing reprojection/GCP residuals) drives the suggestion; the proposal cites the exact failing regions/metrics and proposes a bounded re-fly area; it is advisory only and approval-gated, ties to `01` mission planning, never auto-dispatches.
- **Acceptance**:
  - Given a mosaic with a coverage gap, when a re-fly is suggested, then the proposal cites the gap region and metric and produces a bounded re-fly area for `01`, pending operator approval.
  - Given a fully covered, passing mosaic, when evaluated, then no re-fly is suggested (no spurious flight proposals); and no proposal ever dispatches without approval.
- **Tests**: unit (suggestion from QA evidence), integration (`01` proposal hand-off), failure path (passing mosaic → no suggestion; un-approved proposal never dispatches).
- **Depends on**: 22-03, 22-12, 22-13, 22-15, `01`.

---

## Coverage note

These 17 stories cover all 12 capabilities in `capability-map.md`. The breakdown is M3-heavy by design: the deterministic reconstruction, orthomosaic, DSM/DTM, and QA core is the trust foundation, and **geospatial correctness leads every phase** per `release-plan.md`. The single M5 story (model-assisted re-fly) stays advisory and approval-gated, gated behind the deterministic QA products. The curated counts in `release-plan.md` (≈87 rows) expand several of these — per-stage SfM variants, additional QA-metric and tiling slices, multi-camera/multispectral mosaic variants — into sibling stories when implemented.
