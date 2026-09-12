# Landsat production readiness review

Reviewed and remediated 2026-09-12. Verdict: **the Landsat provider-to-L2 code
path is ready for a controlled production pilot; decision-grade release still
requires field-reference and regional science validation**.

## Remediation status

The implementation now routes Earth Search Landsat S3 assets through the
signed Planetary Computer mirror, uses immutable crop-specific artifacts,
includes field and season in product identity, follows STAC pagination, and
resamples overlapping scene grids for temporal compositing. It applies
QA_PIXEL plus sensor-specific SR QA, and evaluates only the QA_RADSAT bits for
the bands used by the requested product. ETM+ dropped pixels and OLI terrain
occlusion remain unconditional rejects. Surface-temperature QA preserves its
independent nodata mask and reports the fraction of valid pixels at or below
the AGBot 5 K uncertainty ceiling. Missing required QA or confidence below the
configured gate quarantines the product before field time-series publication.
The legacy USGS verifier uses `login-token` and
`AGBOT_USGS_APPLICATION_TOKEN`.

Both local and remote Landsat derivation expose the sensor-supported index
family:
NDVI, NDWI, MNDWI, NDMI, NBR, EVI, EVI2, SAVI, MSAVI, OSAVI, GNDVI, VARI,
AWEI-nsh, AWEI-sh, and LST. Snow, cloud, shadow, fill, and saturation are
masked; Landsat 4-7 cloud adjacency and Landsat 8-9 high aerosol are also
masked, while water remains usable for water indices. Scaled surface
reflectance is preserved rather than clipped to `[0,1]`. Remote LST consumes
the provider-produced Collection-2 ST band, applies the documented scale and
offset, and carries ST_QA confidence. Automated subscriptions accept Landsat
LST and reject unknown indices, Landsat NDRE (no red-edge band), Sentinel LST,
and HLS (no automated LP DAAC discovery adapter) before jobs can dead-letter.

The production access seam was exercised live against Earth Search, Planetary
Computer SAS, Azure COG range reads, Landsat 7 scene
`LE07_L2SP_028031_20230721_02_T1`, and Landsat 9 scene
`LC09_L2SP_029031_20230918_02_T1`. Live NDVI and LST both produced valid
pixels and five polygon-scoped statistics; the NDVI product also rendered as
an opaque PNG map tile. The numbered findings below retain the original
review evidence. Their production-path software corrections are implemented
in the current worktree.

The remaining release evidence is external: independent reference-pixel and
regional agronomic validation, an accepted crop-specific training/validation
set, and weather reference ET for converting the implemented triangle ET
fraction to mm/day. Those inputs cannot be manufactured from unit fixtures.

Scope: scene discovery, remote access, USGS verification, local/remote Landsat
derivation, catalog identity, field statistics, queue execution, QA gating,
and downstream fan-out. This review includes the remediation in the current
uncommitted worktree.

## Algorithm coverage

| Stage | Production behavior | Status |
| --- | --- | --- |
| Discovery and access | Earth Search pagination; Landsat S3 href translation to Planetary Computer; renewable SAS range reads | Implemented and live-tested |
| Radiometry | Collection-2 Level-2 SR scale/offset and Collection-2 ST scale/offset; no destructive reflectance clipping | Implemented |
| Optical indices | NDVI, NDWI, MNDWI, NDMI, NBR, EVI, EVI2, SAVI, MSAVI, OSAVI, GNDVI, VARI, AWEI-nsh, AWEI-sh | Implemented; NDRE explicitly unavailable for Landsat |
| Surface temperature | Provider-produced Collection-2 ST in Kelvin with ST_QA uncertainty evidence | Implemented and live-tested |
| Pixel QA | QA_PIXEL; band-specific QA_RADSAT; SR_CLOUD_QA for Landsat 4-7; SR_QA_AEROSOL for Landsat 8-9; ST_QA for LST | Implemented; missing evidence fails closed |
| Spatial reduction | True Polygon/MultiPolygon membership including holes; mean, median, p10, p90, valid fraction | Implemented and live-tested |
| Temporal processing | Overlap-grid resampling, monthly composites, same-calendar-month climatology with a five-year minimum | Implemented and fixture-tested |
| Phenology | Configurable season-end months, lookback, and minimum observations; NDVI-only execution guard | Implemented and fixture-tested |
| Drought | NDVI→VCI, LST→TCI, VCI+TCI→VHI, plus SPI/CHIRPS paths and evidence-linked alert fan-out | Implemented and fixture-tested; operational output needs multi-year archive/weather |
| Evapotranspiration | NDVI/LST triangle ET fraction | Implemented; ET in mm/day awaits reference-ET weather input |
| Crop/vegetation classification | Deterministic phenology features and a learned-classifier seam | Engine exists; production labels/model acceptance remain open |

This covers the Landsat algorithms required by the current product and
roadmap contracts. It does not establish regional decision accuracy: the 5 K
ST_QA ceiling, spectral thresholds, phenology settings, drought alert
thresholds, and any crop model must be validated and versioned for each
deployed crop and region.

## Findings

### 1. P1 — Production Landsat remote access is disconnected

`geo_hub/src/server.rs:1251` constructs the worker with `UrlCogResolver` and `EarthSearchItemFetcher`. `geo_hub/src/routes/satellite.rs:126` and `:147` do the equivalent for direct requests. The captured Earth Search Landsat item contains requester-pays `s3://usgs-landsat` assets, but `raster_io/Cargo.toml` enables HTTP object storage only. `PcSignedCogResolver` exists without a production caller; simply supplying a Planetary Computer item does not wire signing.

A temporary probe using the captured item and the actual production resolver returned: `Generic parse_url error: feature for AmazonS3 not enabled`.

Required correction: select a supported Landsat provider and wire item resolution and authenticated/range-readable asset resolution together for both the API and worker. Adding S3 support alone does not resolve requester-pays authorization.

### 2. P1 — A second field crop overwrites the first product's raster

`geo_hub/src/satellite_derivation.rs:886` always writes `scenes/{scene}/products/{index}.tif`. AOI changes produce separate product identities, but share that file. Catalog records and checksums for the earlier product then describe bytes that no longer exist at their registered path. This affects serving, composites, and every consumer that rereads the artifact.

A temporary two-field/AOI test confirmed different product IDs, identical artifact paths, and changed bytes at the first product's path after the second derivation.

Required correction: immutable artifact paths keyed by derivation identity, with temporary-file write and atomic publication. Test sequential and concurrent requests for different AOIs and verify earlier products remain unchanged.

### 3. P1 — Field and season ownership can disappear through deduplication

`shared/src/product_graph.rs:177` prefers scene ID over field ID when constructing identity. Landsat local parameters (`geo_hub/src/landsat_derive.rs:506`) and remote parameters (`satellite_derivation.rs:1031`) omit field and season. `geo_hub/src/catalog.rs:162` returns an existing product on the same kind/parameter hash without updating scope.

Consequently, the same scene/index/crop requested for another field or season reuses the first record. Field-stat extraction reads that record's original field. A temporary remote test confirmed that the same crop requested for two different fields returns the same product ID.

Required correction: explicit product-to-field associations or scope-aware identity/deduplication. Include re-scoping an initially unscoped product and separate seasons in regression coverage.

### 4. P1 — Field statistics measure the bounding rectangle, not the field

`geo_hub/src/pipeline_worker.rs:677` reduces the field boundary to a bounding box; remote derivation accepts only that box. `geo_hub/src/field_timeseries.rs:192` reads the full raster band and reduces all non-nodata pixels, without applying the actual field polygon.

For irregular fields, holes, or multipart boundaries, neighboring crops, roads, and water can affect the reported field mean, percentiles, and valid fraction, and therefore advisor findings.

Required correction: rasterize the field geometry on the output grid and use the in-field mask for statistics and coverage denominators. Test a polygon whose bbox contains contrasting outside-field pixels and a polygon with a hole.

### 5. P1 — The configured remote-product QA gate is ineffective

`geo_hub/src/satellite_derivation.rs:1095` always sets L2 confidence to `None`. The worker gate (`pipeline_worker.rs:850`) evaluates confidence, and `catalog.rs:497` passes unknown confidence. Raising `qa_min_confidence` therefore cannot quarantine any remote Landsat product, including a raster with zero valid pixels. Field-stat writes also happen before this gate and are best-effort.

Required correction: define and persist an evidence-backed quality/coverage measure, explicitly handle unknown quality, and apply acceptance before downstream stats/fan-out. Test all-cloud, missing QA, low coverage, and fully valid scenes.

### 6. P1 for the USGS alternative — Authentication and verification do not establish a scientific ingest

`geo_hub/src/landsat.rs:323` calls `/login` with username/password. USGS documents this endpoint as deprecated February 26, 2025 and directs clients to `/login-token` with an application token: https://www.usgs.gov/media/files/m2m-application-token-documentation . No authenticated USGS request was made in this review, so current live failure behavior was not independently exercised.

Separately, `landsat.rs:267` downloads only the scene browse image. A successful `verify_credentialed_usgs_landsat_ingest` therefore does not prove surface-reflectance, thermal, or QA band acquisition, or local derivation.

Required correction: current authentication, actual required science/QA band acquisition, and an acceptance test that derives a product from those acquired bands.

### 7. P2 — Discovery/backfill silently stops at the first search page

`geo_hub/src/earth_search.rs:148` discards response links; `:335` parses one response only. The production worker requests 50 scenes and advances the backfill cursor after processing that response (`pipeline_worker.rs:562`). A chunk with more than 50 matches can be marked enumerated while missing later pages.

Required correction: follow provider pagination, preserve progress safely, and test a multi-page chunk plus a transient failure on a later page.

### 8. P2 — Invalid thermal uncertainty can produce high confidence

`geo_hub/src/landsat_derive.rs:331` converts invalid/nodata band values to DN zero. At `:472`, ST_QA uses that conversion and zero satisfies the low-uncertainty threshold. A valid thermal pixel with invalid ST_QA is counted as low uncertainty; invalid uncertainty across a scene can yield confidence 1.0.

Required correction: preserve an independent uncertainty-validity mask and distinguish missing uncertainty from measured zero. Add an ST_QA nodata fixture.

### 9. Science acceptance gap — Saturation QA is not applied

Both Landsat paths use QA_PIXEL but do not load QA_RADSAT. Clear-sky pixels with saturation in a contributing band can therefore remain valid for spectral indices. The missing band-specific quality treatment needs a regression and documented policy before agricultural decision use. USGS reference: https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands .

Remediated: both paths now use an instrument- and product-specific reject
mask. Saturation in an unrelated band no longer discards the requested
product, while contributing-band saturation, ETM+ dropped pixels, and OLI
terrain occlusion are rejected and recorded.

## Validation performed after remediation

The focused and crate-level validations completed successfully:

```sh
cargo check -p geo_hub
cargo test -p geo_hub --lib
cargo test -p geo_hub --test landsat_derive
cargo test -p geo_hub --test landsat_remote_derive
cargo test -p geo_hub --test pipeline_discover
cargo test -p geo_hub --test pipeline_l3_suite
cargo test -p imagery_processor
cargo test -p geo_hub --test landsat_remote_derive -- --ignored live_landsat_production_derivation_smoke --nocapture
cargo fmt --all -- --check
git diff --check
```

The `geo_hub` library suite passed 239 tests with one ignored credentialed USGS
test. The local Landsat suite passed 6 tests, the remote Landsat suite passed 7
fixture tests, the pipeline discovery suite passed 5 tests, and the L3 suite
passed 8 tests. The live Landsat smoke passed separately for NDVI and LST. The full
`cargo test -p geo_hub` run passed all Landsat and satellite-related suites but
stopped on the unrelated `terrain_derive` integration test because the local
ONNX Runtime installation references a missing
`libprotobuf-lite.35.1.0.dylib`. Existing compiler warnings remain.

## Release acceptance gates

1. A real Landsat scene for a known field completes discovery, field statistics, tile serving, and applicable advisor processing through production dependencies. Direct provider lookup, readable SR/ST/QA acquisition, calibrated NDVI/LST derivation, field statistics, immutable artifact registration, and tile serving have passed the live smoke. Automated discovery and multi-observation advisor output remain deployment acceptance checks.
2. Independent reference pixels agree with the output for reflectance/index math, QA masking, thermal conversion, and uncertainty handling.
3. Two fields in one scene and repeated processing across seasons retain correct ownership and immutable artifacts.
4. Field geometry, no-data coverage, all-cloud scenes, and saturation/uncertainty cases produce explicit accepted or quarantined outcomes.
5. Pagination, provider failure/retry, process restart, and expired-token recovery preserve completeness and avoid duplicate or corrupt products.

The calibrated math, lineage, local derivation fixtures, queue retry/recovery,
downstream tests, and live provider smoke establish the software baseline.
They do not substitute for the remaining field and regional acceptance gates.

## Scientific sources

- [USGS Landsat Collection-2 Surface Reflectance](https://www.usgs.gov/landsat-missions/landsat-collection-2-surface-reflectance)
- [USGS Landsat Collection-2 Surface Temperature](https://www.usgs.gov/landsat-missions/landsat-collection-2-surface-temperature)
- [USGS Landsat Collection-2 Quality Assessment Bands](https://www.usgs.gov/landsat-missions/landsat-collection-2-quality-assessment-bands)
