# Imagery and Remote Sensing: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI. Geospatial correctness is the dominant pillar here — CRS/extent/resolution must be asserted and georeferenced products must round-trip.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

This domain is the **core advisor MVP** alongside `07`/`08`/`09`: deterministic, georeferenced index/thermal/mask products that the viewer renders and the advisor cites. Real crates: `imagery_processor` (clap CLI, `pipeline/{indices,thermal,classify,masks}.rs`, `io/`), `sensor_overlay_engine` (`ndvi.rs`, `thermal.rs`, `composite.rs`, `OverlayProcessor`), `shared/src/schemas.rs` (`ImageMetadata`, `MultispectralImage`, `NdviResult`, `RasterSpatialRef`).

---

## M1 — Foundation

### STORY 05-01 · M1 · M · P0 — Band ingest and metadata mapping
- **Story**: As `DSP`, I want bands loaded from `metadata_*.json` with band names resolved per sensor preset, so that downstream index math reads the right band every time.
- **Deterministic / evidence**: load bands via `imagery_processor/src/io/` into `MultispectralImage`; resolve red/NIR/red-edge/etc. from the sensor preset; persist `{image_id, sensor, band_index→band_name map, width, height}` from `ImageMetadata`.
- **Acceptance**:
  - Given a `metadata_*.json` for a Sentinel-2 capture, when ingest runs, then every required band resolves to a concrete raster of matching dimensions.
  - Given a metadata file missing a band the preset requires, when ingest runs, then it fails with an explicit missing-band error naming the band, not a silent zero-fill.
- **Tests**: unit (band-name resolution per preset), fixture (`04` capture metadata), failure path (missing band).
- **Depends on**: `04` (captured imagery fixtures), `shared` schemas.

### STORY 05-02 · M1 · S · P0 — Band geometry and dimension assertion
- **Story**: As `OPS`, I want every ingested band's width/height/pixel grid asserted to agree, so that an index is never computed across mismatched rasters.
- **Deterministic / evidence**: assert all bands of an image share dimensions and pixel grid; record per-band `{width, height, dtype, nodata}` as inspectable evidence.
- **Acceptance**:
  - Given an image whose bands share a grid, when assertion runs, then ingest proceeds and the grid is recorded.
  - Given two bands of differing dimensions, when assertion runs, then ingest is rejected with a dimension-mismatch error.
- **Tests**: unit (grid agreement), failure path (mismatched band dimensions).
- **Depends on**: 05-01.

### STORY 05-03 · M1 · M · P0 — Spectral index computation (NDVI/NDRE first)
- **Story**: As `AG`, I want NDVI and NDRE computed per pixel with valid-pixel statistics and reason codes, so that I get defensible vegetation numbers, not just a colored picture.
- **Deterministic / evidence**: compute the index from `IndexKind` over the resolved bands; emit `IndexResultMeta` with min/max/mean and valid-pixel count; nodata and divide-by-zero pixels excluded with a reason code.
- **Acceptance**:
  - Given a valid red/NIR image, when NDVI runs, then a per-pixel index raster plus min/max/mean and valid coverage are produced and clamped to `[-1, 1]`.
  - Given pixels where `red+NIR=0`, when NDVI runs, then those pixels are marked nodata with a reason code, not emitted as NaN.
- **Tests**: unit (NDVI/NDRE math incl. divide-by-zero), fixture (known sample grid → known mean), failure path (degenerate zero-sum pixels).
- **Depends on**: 05-01, 05-02.

### STORY 05-04 · M1 · M · P1 — Full 12-index catalog
- **Story**: As `AG`, I want the remaining indices (EVI, SAVI, VARI, GNDVI, NDWI, MNDWI, MSAVI, NBR, NDMI, EVI2) computable, so that I can pick the right index for crop, water, or burn analysis.
- **Deterministic / evidence**: each `IndexKind` variant computes from its required bands with the same stats/nodata contract as 05-03; per-index required-band list validated before compute.
- **Acceptance**:
  - Given the bands an index requires, when that index runs, then it produces a raster with valid stats and the correct value range for that index.
  - Given an index whose required band is absent, when it runs, then it is cleanly unavailable with a named-band error, not silently substituted.
- **Tests**: unit (per-index math, ≥1 known vector each), failure path (missing required band per index).
- **Depends on**: 05-03.

### STORY 05-05 · M1 · S · P1 — Sensor presets and per-band overrides
- **Story**: As `DSP`, I want Sentinel-2 / Landsat-8 / DJI Multispectral presets with per-band CLI overrides, so that I can map bands correctly for any of my sensors.
- **Deterministic / evidence**: preset supplies default red/NIR/red-edge mapping; CLI `--band` overrides take precedence and are recorded in product provenance.
- **Acceptance**:
  - Given a DJI capture, when the DJI preset is selected, then default band mapping applies and indices compute correctly.
  - Given an override for the NIR band, when an index runs, then the override is used and recorded; an override naming a nonexistent band is rejected.
- **Tests**: unit (preset defaults + override precedence), failure path (override to unknown band).
- **Depends on**: 05-01.

---

## M2 — Captured / Observable

### STORY 05-06 · M2 · M · P0 — Radiometric calibration (DN → reflectance)
- **Story**: As `AG`, I want digital numbers calibrated to reflectance per sensor preset before index math, so that NDVI/NDRE values are comparable across captures and seasons.
- **Deterministic / evidence**: apply per-preset gain/offset (and TOA correction where defined) to convert DN → reflectance; record the calibration coefficients used as product evidence.
- **Acceptance**:
  - Given a DN image and a preset with calibration coefficients, when calibration runs, then output reflectance is in `[0,1]` and coefficients are recorded.
  - Given a preset with no calibration coefficients, when calibration is requested, then the product is marked "uncalibrated DN" rather than silently treated as reflectance.
- **Tests**: unit (gain/offset math, range bound), fixture (DN → reflectance vector), failure path (missing coefficients → uncalibrated flag).
- **Depends on**: 05-01.

### STORY 05-07 · M2 · S · P0 — QA mask generation (cloud/shadow/snow/water/clear)
- **Story**: As `AG`, I want a clear-sky QA mask generated from QA bands, so that clouds and shadows do not corrupt my index statistics.
- **Deterministic / evidence**: `MasksArgs` over QA bands produces per-pixel class (cloud/shadow/snow/water/clear); persist the mask raster and class counts as evidence.
- **Acceptance**:
  - Given an image with QA bands, when masking runs, then a clear-sky mask is produced with per-class pixel counts.
  - Given an image with no QA band, when masking is requested, then the capability is cleanly unavailable, not a faked all-clear mask.
- **Tests**: unit (class assignment), fixture (synthetic QA band), failure path (no QA band).
- **Depends on**: 05-01.

### STORY 05-08 · M2 · S · P1 — Apply QA mask before statistics
- **Story**: As `AG`, I want masked pixels excluded before index min/max/mean are computed, so that a cloudy patch never skews the field's vigor number.
- **Deterministic / evidence**: combine the 05-07 mask with the index nodata mask; recompute stats and valid-pixel coverage over the clear-sky pixels only.
- **Acceptance**:
  - Given an index raster and a clear-sky mask, when stats run, then masked pixels are excluded and coverage reflects the clear fraction.
  - Given a fully clouded scene, when stats run, then the result is an explicit "no clear pixels" outcome, not a misleading mean.
- **Tests**: unit (masked stats + coverage), failure path (fully masked scene).
- **Depends on**: 05-03, 05-07.

### STORY 05-09 · M2 · S · P1 — Ingest freshness and coverage tracking
- **Story**: As `OPS`, I want each ingested image to record capture timestamp and spatial coverage, so that stale or partial captures are visible before analysis.
- **Deterministic / evidence**: persist `{capture_time, ingested_at, valid_pixel_fraction}` per image; flag coverage below a configurable floor.
- **Acceptance**:
  - Given a fresh full-coverage image, when ingest runs, then freshness and coverage are recorded and pass the floor.
  - Given an image below the coverage floor, when ingest runs, then it is flagged low-coverage and surfaced, not silently analyzed.
- **Tests**: unit (coverage fraction, freshness), failure path (below-floor coverage flagged).
- **Depends on**: 05-01.

---

## M3 — Explainable (geospatial correctness — the trust foundation)

### STORY 05-10 · M3 · L · P0 — Georeferencing and CRS/extent/resolution assertion
- **Story**: As `OPS`, I want every product to carry an asserted CRS, extent, resolution, and transform, so that an overlay is provably on the right ground.
- **Deterministic / evidence**: populate `RasterSpatialRef` (CRS, extent, resolution, transform) from source metadata; assert non-empty CRS, positive resolution, and extent consistent with width×height×resolution before any product is written.
- **Acceptance**:
  - Given a georeferenced source, when a product is built, then its `RasterSpatialRef` is asserted and `extent == origin + dims·resolution` within tolerance.
  - Given a source with a missing/zero CRS or non-positive resolution, when assertion runs, then product write is rejected with a georeferencing error rather than emitting an unreferenced raster.
- **Tests**: unit (extent↔dims↔resolution consistency), geospatial assertion (CRS/resolution validity), failure path (missing CRS / zero resolution).
- **Depends on**: 05-01, `shared` `RasterSpatialRef`.

### STORY 05-11 · M3 · L · P0 — GeoTIFF write with lossless transform round-trip
- **Story**: As `OPS`, I want products written as GeoTIFF whose CRS/extent/resolution round-trip without drift, so that the viewer and advisor read back exactly what was computed.
- **Deterministic / evidence**: harden the `gdal-io` GeoTIFF path with sidecar transform; on write-then-read the CRS, extent, resolution, transform, and pixel values must match within tolerance.
- **Acceptance**:
  - Given a product with an asserted `RasterSpatialRef`, when it is written to GeoTIFF and re-read, then CRS/extent/resolution/transform and pixel values round-trip within tolerance.
  - Given the `gdal-io` feature is unavailable, when GeoTIFF write is requested, then it fails with a clear "feature not enabled" error rather than writing a PNG mislabeled as georeferenced.
- **Tests**: geospatial round-trip (write→read equality), unit (sidecar transform), failure path (`gdal-io` disabled).
- **Depends on**: 05-10.

### STORY 05-12 · M3 · S · P1 — PNG product output with sidecar georeference
- **Story**: As `DSP`, I want PNG products to carry a sidecar world-file/metadata, so that a non-GeoTIFF viewer can still place them correctly.
- **Deterministic / evidence**: `OutputFormat::Png` writes the raster plus a sidecar carrying extent/resolution/CRS; sidecar values match the product's `RasterSpatialRef`.
- **Acceptance**:
  - Given a georeferenced product, when PNG output runs, then a sidecar is written whose extent/resolution/CRS match the product metadata.
  - Given a product lacking georeferencing, when PNG output runs, then it is emitted as a plain non-georeferenced image clearly marked as such.
- **Tests**: unit (sidecar values match metadata), failure path (ungeoreferenced product marked).
- **Depends on**: 05-10.

### STORY 05-13 · M3 · M · P0 — Thermal LST pipeline (radiance → BT → LST)
- **Story**: As `AG`, I want land-surface temperature computed through radiance → brightness-temperature → LST with constant emissivity, so that thermal stress findings rest on real temperatures.
- **Deterministic / evidence**: `ThermalArgs` chain produces radiance, brightness temperature, then LST in Kelvin/Celsius; retain intermediate stats and emissivity used as evidence.
- **Acceptance**:
  - Given a TIR band and thermal coefficients, when LST runs, then a georeferenced LST raster with min/max/mean is produced in the requested units.
  - Given a missing TIR band or thermal coefficients, when LST runs, then it fails with a named-input error rather than emitting a fabricated temperature field.
- **Tests**: unit (radiance/BT/LST math against known vector), fixture (TIR band), failure path (missing TIR/coefficients).
- **Depends on**: 05-01, 05-10.

### STORY 05-14 · M3 · S · P1 — NDVI-based emissivity and split-window
- **Story**: As `AG`, I want emissivity derived from an NDVI image and split-window applied when two TIR bands exist, so that LST is more accurate over vegetated fields.
- **Deterministic / evidence**: compute per-pixel emissivity from NDVI thresholds; when two TIR bands are present, apply split-window; record method and parameters as evidence.
- **Acceptance**:
  - Given an NDVI raster and one TIR band, when LST runs, then NDVI-derived emissivity is applied and recorded.
  - Given two TIR bands, when LST runs, then split-window is used; given only one, it falls back to single-channel and records the fallback.
- **Tests**: unit (emissivity from NDVI, split-window vs single), failure path (single-TIR fallback recorded).
- **Depends on**: 05-13, 05-03.

### STORY 05-15 · M3 · M · P1 — Classification (threshold and k-means)
- **Story**: As `AG`, I want an index raster classified into vegetation classes by threshold or k-means, so that I can map healthy/stressed/bare zones deterministically.
- **Deterministic / evidence**: `ClassifyArgs` threshold mode assigns classes by cut points; k-means mode is seeded for determinism; persist class boundaries and per-class pixel counts.
- **Acceptance**:
  - Given an NDVI raster and thresholds, when threshold classification runs, then each pixel gets a class and per-class counts are returned, in the product CRS.
  - Given a fixed seed, when k-means runs twice on the same input, then the class assignment is identical (deterministic).
- **Tests**: unit (threshold cuts, k-means determinism with seed), failure path (single-value raster → one class, no crash).
- **Depends on**: 05-03, 05-10.

### STORY 05-16 · M3 · S · P1 — Evidence retention and reproducibility
- **Story**: As `DSP`, I want every product to persist its inputs, method, parameters, and stats, so that a result can be re-derived and defended.
- **Deterministic / evidence**: each product stores `{source_image_ids, index/method, params, calibration, mask_ref, min/max/mean, coverage}`; re-running on identical inputs yields an identical output hash.
- **Acceptance**:
  - Given a product, when it is inspected, then it cites its source images, method, parameters, and stats.
  - Given identical inputs, when the pipeline re-runs, then the output is byte-identical (deterministic hash match).
- **Tests**: determinism test (same input → same hash), fixture (provenance fields present).
- **Depends on**: 05-03, 05-06, 05-07, 05-10.

---

## M4 — Interactive

### STORY 05-17 · M4 · M · P0 — Overlay rendering and colormaps
- **Story**: As `AG`, I want index and thermal products rendered with vegetation/thermal colormaps via `OverlayProcessor`, so that the viewer shows a meaningful, legible layer.
- **Deterministic / evidence**: `NdviProcessor`/`ThermalProcessor` map values to viridis/jet/hot/grayscale with a fixed value→color domain; the colormap, value range, and legend stops are emitted as overlay metadata.
- **Acceptance**:
  - Given an NDVI product, when overlay rendering runs, then a colored layer plus a legend (value→color stops) is produced over the same georeferenced extent.
  - Given an out-of-range or all-nodata product, when rendering runs, then nodata renders transparent and the legend reflects the actual data range, not a default.
- **Tests**: unit (value→color mapping, legend stops), failure path (all-nodata → transparent).
- **Depends on**: 05-03, 05-10, `sensor_overlay_engine`.

### STORY 05-18 · M4 · S · P1 — Composite and heatmap overlays
- **Story**: As `AG`, I want composite and IDW-interpolated heatmap overlays, so that sparse or multi-product views are still readable on the field.
- **Deterministic / evidence**: `composite.rs` blends layers and the IDW grid interpolation fills sparse samples; record blend/interpolation parameters and assert output shares the product extent.
- **Acceptance**:
  - Given two georeferenced products on the same extent, when a composite is built, then the output preserves CRS/extent and records the blend parameters.
  - Given products on mismatched extents, when a composite is requested, then it is rejected with an extent-mismatch error.
- **Tests**: unit (blend + IDW), geospatial (shared-extent assertion), failure path (extent mismatch).
- **Depends on**: 05-17.

### STORY 05-19 · M4 · S · P0 — Product provenance and scene/field/season linkage
- **Story**: As `AG`, I want every product linked to its scene, field, and season, so that the advisor (`09`) and viewer (`08`) can trace a finding back to the right field and date.
- **Deterministic / evidence**: persist `{product_id, scene_id, field_id, season_id, product_kind, spatial_ref, source_image_ids[]}`; publish the product to `07` so the viewer can fetch it.
- **Acceptance**:
  - Given an ingested scene linked to a field/season, when a product is built, then it carries all linkage IDs and is retrievable by the viewer.
  - Given a product whose scene has no field linkage, when publish runs, then it is rejected with an unlinked-scene error rather than orphaning the product.
- **Tests**: API/contract (publish + fetch), unit (linkage fields present), failure path (unlinked scene).
- **Depends on**: 05-10, `07` (scene/field/season spine), `09` (consumes products).

### STORY 05-20 · M4 · S · P1 — Product export (GeoTIFF + stats CSV)
- **Story**: As `DSP`, I want products exported as GeoTIFF plus a stats CSV, so that clients can use index/thermal rasters in other tools.
- **Deterministic / evidence**: export the georeferenced GeoTIFF (round-trip from 05-11) and a CSV of `{product, min, max, mean, coverage, units}`; both validate against a schema.
- **Acceptance**:
  - Given a completed product, when export runs, then the GeoTIFF round-trips its CRS/extent and the CSV stats match the product evidence.
  - Given a product with no valid pixels, when export runs, then a valid empty/zero-coverage export is produced, not a corrupt file.
- **Tests**: geospatial round-trip, schema validation, failure path (empty product → valid export).
- **Depends on**: 05-11, 05-16.

---

## M5 — Autonomous-Assist (gated, uncertainty-flagged)

### STORY 05-21 · M5 · M · P1 — Index-trend advisory across seasons
- **Story**: As `AG`, I want index trends compared across comparable scenes with an uncertainty flag, so that I can see vigor change over time without over-trusting it.
- **Deterministic / evidence**: align two georeferenced products of the same field over a common extent; compute per-zone delta with coverage; flag uncertainty when CRS/extent/resolution or calibration differ; feature-flagged.
- **Acceptance**:
  - Given two comparable calibrated scenes of one field, when trend runs, then a delta layer plus stats and an uncertainty band are produced over the common extent.
  - Given two scenes with mismatched CRS/extent or one uncalibrated, when trend runs, then it is marked low-confidence or unavailable, never silently differenced.
- **Tests**: unit (delta + alignment), gating test (mismatch → low-confidence), failure path (uncalibrated input).
- **Depends on**: 05-06, 05-10, 05-19, `09` advisor spine.

### STORY 05-22 · M5 · M · P2 — Index anomaly detection (gated)
- **Story**: As `AG`, I want anomalous index regions auto-flagged with their evidence, so that I can find stress without scanning the whole field — once georeferencing is trustworthy.
- **Deterministic / evidence**: statistical outlier flagging over the masked, calibrated index raster; each flag carries threshold, value, and the georeferenced cell; disabled until 05-10/05-11 are reliable.
- **Acceptance**:
  - Given a trustworthy georeferenced index product, when anomaly detection runs, then flagged regions carry threshold/value and correct georeferencing.
  - Given a product without asserted georeferencing, when detection is requested, then it is unavailable (gated), not run on unreferenced data.
- **Tests**: unit (outlier logic), gating test (ungeoreferenced → disabled), failure path (uniform raster → 0 flags).
- **Depends on**: 05-10, 05-11, 05-08, `09`.

### STORY 05-23 · M5 · L · P2 — Vegetation-type classification (crop/forest/bush) with confidence
- **Story**: As `AG`, I want satellite and drone scenes classified into vegetation types — e.g., cotton, rice, palm, forest, bush/shrub — with per-class confidence, so that I know what is growing where; early accuracy may be modest and should improve season over season rather than block shipping.
- **Deterministic / evidence**: the baseline is deterministic, no opaque model: match per-pixel spectral signatures (calibrated indices from 05-04/05-06) and temporal/phenological profiles (aligned via `28`) against a versioned signature library; every classification emits per-class confidence, the matched signature as citable evidence, and an uncertainty flag. ML models may later replace the matcher behind the same gated, flagged contract.
- **Acceptance**:
  - Given calibrated multi-date scenes of a field with a known crop, when classification runs, then a vegetation-type map is produced with per-class confidence and the matched signature evidence for each zone.
  - Given a single uncalibrated scene, when classification runs, then the result is "insufficient evidence" or explicitly low-confidence — never an unflagged confident label.
- **Tests**: fixture (known-crop scenes, including synthetic labeled scenes from the `02` dataset export where ground truth is perfect), unit (signature matching and confidence math), failure path (uncalibrated single scene gated).
- **Depends on**: 05-04, 05-06, 05-15, `28` (temporal alignment), `02` STORY 02-23 (synthetic labeled fixtures), `07` (Landsat/Sentinel scene ingestion).

---

## Coverage note

These 23 stories cover all 12 capabilities in `capability-map.md` plus vegetation-type classification, ordered by phase and consistent with `release-plan.md` (M1 16 / M2 15 / M3 22 / M4 16 / M5 10; P0 33 / P1 29 / P2 17). Geospatial correctness is the gating concern: every product asserts CRS/extent/resolution (05-10) and round-trips through GeoTIFF (05-11) before it is trusted by the viewer (`08`) or advisor (`09`). The curated ~76 backlog rows expand several stories into siblings — per-index variants of 05-04, additional QA classes in 05-07, and per-sensor calibration tables in 05-06 — when implemented.
