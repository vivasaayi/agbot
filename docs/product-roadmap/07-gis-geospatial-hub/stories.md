# GIS and Geospatial Hub: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI. Geospatial correctness is the dominant pillar — every ingested scene and served layer asserts CRS/extent/resolution and round-trips the transform.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

This domain is the **core advisor MVP geospatial spine**: it ingests scenes, guarantees raster metadata correctness, links scene → field → season, and serves the layers the viewer (`08`) renders and the advisor (`09`) cites. Real crate: `geo_hub` (`config.rs`/`HubConfig`, `server.rs`/`routes.rs` axum REST, `landsat.rs` USGS client, `ingest.rs`/`IngestLandsatArgs`, `db.rs`/`shapefile.rs` spatial DB, `state.rs` query cache, `mobile_app.html` SPA, `error.rs`/`AgroError`), `shared/src/schemas.rs` (`FarmRecord`, `FieldRecord`, `RasterSpatialRef`, scene/annotation/recommendation/report records), and `geo_hub.db` (SQLite).

---

## M1 — Foundation

### STORY 07-01 · M1 · S · P0 — Hub configuration and runtime mode
- **Story**: As `PA`, I want `HubConfig` to drive server/DB/Landsat settings with a runtime mode, so that the hub boots predictably across environments.
- **Deterministic / evidence**: `HubConfig` loads server, database, and Landsat config; a runtime mode (e.g. local/sim/live) selects the storage and credential source; invalid config fails fast.
- **Acceptance**:
  - Given a valid config, when the hub starts, then server/DB/Landsat settings load and the runtime mode is recorded in startup logs.
  - Given a config missing a required field, when the hub starts, then it fails fast with a named-field error rather than booting half-configured.
- **Tests**: unit (config load + mode selection), failure path (missing required field).
- **Depends on**: `geo_hub` config module.

### STORY 07-02 · M1 · M · P0 — Landsat / USGS scene search
- **Story**: As `AG`, I want best-scene search against the USGS API per source with metadata, so that I can find the right scene for a field and date.
- **Deterministic / evidence**: `landsat.rs` queries USGS, selects the dataset per source, and returns ranked scenes with `{scene_id, acquisition_date, cloud_cover, bbox, dataset}`; results cached in `state.rs`.
- **Acceptance**:
  - Given an AOI and date window, when search runs, then ranked scenes with acquisition date, cloud cover, and bbox are returned and cached.
  - Given an AOI with no matching scenes, when search runs, then an explicit empty result is returned, not an error or stale cache hit.
- **Tests**: unit (ranking + dataset selection), API contract (search), failure path (no matches → empty result).
- **Depends on**: 07-01.

### STORY 07-03 · M1 · S · P0 — Scene/farm/field record identity
- **Story**: As `OPS`, I want scenes, farms, and fields to have stable IDs and ownership linkage, so that every layer and finding is traceable.
- **Deterministic / evidence**: persist `FarmRecord`/`FieldRecord`/scene records with `{id, owner, created_at}`; farm/field CRUD via `routes.rs`.
- **Acceptance**:
  - Given a farm creation request, when it is stored, then a `FarmRecord` with a stable ID and owner is returned and retrievable after restart.
  - Given a field referencing a nonexistent farm, when it is created, then it is rejected with a referential-integrity error.
- **Tests**: API contract (farm/field CRUD), unit (ID + owner), failure path (orphan field rejected).
- **Depends on**: 07-01, `shared` records.

### STORY 07-04 · M1 · S · P1 — Shapefile and GeoJSON boundary import
- **Story**: As `DSP`, I want to import field boundaries from shapefile or GeoJSON, so that fields have real geometry to anchor layers and overlays.
- **Deterministic / evidence**: `shapefile.rs` and the GeoJSON import route parse geometry into a `FieldRecord` boundary; assert the source CRS is read and recorded (no assumed CRS).
- **Acceptance**:
  - Given a shapefile with a `.prj`, when import runs, then the field boundary is stored with its CRS recorded.
  - Given a shapefile with no CRS and none supplied, when import runs, then it is rejected with a missing-CRS error rather than assuming WGS84.
- **Tests**: unit (shapefile/GeoJSON parse), geospatial (CRS recorded), failure path (missing CRS).
- **Depends on**: 07-03.

---

## M2 — Captured / Observable

### STORY 07-05 · M2 · M · P0 — Scene ingest pipeline with freshness
- **Story**: As `OPS`, I want the download → process → store ingest pipeline to track freshness and coverage, so that stale or partial scenes are visible before analysis.
- **Deterministic / evidence**: `ingest.rs` (`IngestLandsatArgs`) downloads, processes, and stores a scene with `{ingested_at, acquisition_date, coverage_fraction, status}`; status lifecycle `Queued→Downloading→Processing→Stored|Failed`.
- **Acceptance**:
  - Given a selected scene, when ingest runs, then it is downloaded, processed, stored, and its freshness and coverage are recorded with `Stored` status.
  - Given a download that errors, when ingest runs, then status is `Failed` with a reason code and the partial artifacts are cleaned up, not left half-stored.
- **Tests**: unit (state transitions), fixture (sample scene), failure path (download failure → cleanup).
- **Depends on**: 07-02, 07-03.

### STORY 07-06 · M2 · M · P1 — Real-credential USGS ingest verification
- **Story**: As `PA`, I want Landsat ingest verified end-to-end against real USGS credentials, so that the capture path is proven, not just API-correct.
- **Deterministic / evidence**: a credentialed integration path exercises search → download → store; credential failures surface distinct error codes; no secrets logged.
- **Acceptance**:
  - Given valid USGS credentials, when an integration ingest runs, then a real scene is stored with correct metadata.
  - Given invalid/expired credentials, when ingest runs, then it fails with an auth error code distinct from a not-found, and no credential is logged.
- **Tests**: integration (credentialed, gated), failure path (auth error distinct from not-found).
- **Depends on**: 07-05.

### STORY 07-07 · M2 · S · P1 — Ingest health and retry/backoff
- **Story**: As `OPS`, I want ingest to retry transient failures with backoff and expose health, so that flaky downloads don't drop scenes silently.
- **Deterministic / evidence**: bounded retry with backoff on transient errors; an ingest-health endpoint reports `{in_flight, succeeded, failed, last_error}`.
- **Acceptance**:
  - Given a transient download error, when ingest runs, then it retries with backoff and ultimately succeeds, with attempts recorded.
  - Given a permanent error, when retries exhaust, then ingest fails with the terminal reason and health reflects the failure.
- **Tests**: unit (retry/backoff bound), API contract (health), failure path (exhausted retries → terminal).
- **Depends on**: 07-05.

---

## M3 — Explainable (raster metadata correctness — the trust foundation)

### STORY 07-08 · M3 · L · P0 — Raster metadata correctness (CRS/extent/resolution assertion)
- **Story**: As `OPS`, I want every ingested scene to assert and persist its CRS, extent, resolution, and transform, so that a wrong overlay can never reach the viewer.
- **Deterministic / evidence**: on ingest, populate `RasterSpatialRef` from the source and assert non-empty CRS, positive resolution, and `extent == origin + dims·resolution` within tolerance; reject on violation.
- **Acceptance**:
  - Given a georeferenced scene, when ingest runs, then its `RasterSpatialRef` is persisted and the extent↔dims↔resolution relation is asserted.
  - Given a scene with a missing CRS or non-positive resolution, when ingest runs, then it is rejected with a georeferencing error rather than stored as an unreferenced layer.
- **Tests**: unit (extent↔dims↔resolution), geospatial assertion (CRS/resolution validity), failure path (missing CRS / zero resolution).
- **Depends on**: 07-05, `shared` `RasterSpatialRef`.

### STORY 07-09 · M3 · M · P0 — Georeferenced transform round-trip
- **Story**: As `OPS`, I want a scene's transform to round-trip from store to served layer, so that what the viewer fetches is provably the same ground as what was ingested.
- **Deterministic / evidence**: store→serve the CRS/extent/resolution/transform; on a write→serve→read round-trip the values must match within tolerance; a pixel↔world reprojection at the corners must agree.
- **Acceptance**:
  - Given an ingested scene, when it is served and read back, then CRS/extent/resolution/transform round-trip within tolerance and corner pixel↔world coordinates agree.
  - Given a stored transform that fails the round-trip check, when the layer is requested, then it is withheld with a metadata-integrity error rather than served wrong.
- **Tests**: geospatial round-trip (store→serve→read equality, corner reprojection), failure path (transform integrity failure → withheld).
- **Depends on**: 07-08.

### STORY 07-10 · M3 · M · P0 — Scene → field → season linkage spine
- **Story**: As `AG`, I want an ingested scene linked to a field and season, so that the advisor workflow can walk from raster to field to report.
- **Deterministic / evidence**: persist `{scene_id, field_id, season_id, linked_at}`; a scene's geometry must intersect the field boundary to link; linkage is queryable both directions.
- **Acceptance**:
  - Given a scene overlapping a field, when linkage runs, then the scene is linked to the field and season and both directions are queryable.
  - Given a scene whose extent does not intersect the field boundary, when linkage is attempted, then it is rejected with a no-overlap error.
- **Tests**: unit (intersection check), API contract (link + bidirectional query), failure path (no-overlap rejected).
- **Depends on**: 07-04, 07-08, `10` (season model).

### STORY 07-11 · M3 · M · P0 — Spatial DB and storage backend settlement
- **Story**: As `PA`, I want the authoritative storage backend settled (PostGIS vs SQLite `geo_hub.db`) with migrations, so that the catalog scales on one chosen backend.
- **Deterministic / evidence**: a settled backend choice with versioned migrations; spatial queries (intersection, bbox) run on the chosen backend; `db.rs` abstracts the store.
- **Acceptance**:
  - Given the settled backend, when migrations run on an empty store, then the schema is created and a scene/field round-trips through spatial queries.
  - Given a failed/partial migration, when the hub starts, then it refuses to serve and reports the migration version mismatch rather than running on a half-migrated schema.
- **Tests**: integration (migration + spatial query), failure path (migration mismatch → refuse to serve).
- **Depends on**: 07-03, requirements-rigor open confirmation question (PostGIS vs SQLite).

### STORY 07-12 · M3 · S · P1 — Scene/layer evidence and audit
- **Story**: As `DSP`, I want every scene and link to retain provenance and an audit trail, so that a served layer can be defended and traced.
- **Deterministic / evidence**: persist `{source, dataset, ingested_at, spatial_ref, link history}` and an audit ID per mutation; identical ingest of the same source is idempotent.
- **Acceptance**:
  - Given a served layer, when inspected, then it cites source, dataset, ingest time, and spatial ref.
  - Given the same scene ingested twice, when the second runs, then it is idempotent (no duplicate), with the audit trail recording both attempts.
- **Tests**: unit (provenance fields, idempotency), API contract (audit query), failure path (duplicate ingest deduped).
- **Depends on**: 07-05, 07-08.

---

## M4 — Interactive

### STORY 07-13 · M4 · M · P0 — Layer-serving REST API (paginated, metadata-correct)
- **Story**: As `AG`, I want a paginated layer/metadata API the viewer can trust, so that the geo viewer always renders provably georeferenced layers.
- **Deterministic / evidence**: a layer-list and layer-metadata endpoint returns `{layer_id, product_kind, spatial_ref, freshness, source}` with pagination and filters by field/season/date; the served `spatial_ref` matches the stored one exactly.
- **Acceptance**:
  - Given layers for a field, when the viewer lists them, then results are paginated, filterable, and each carries an asserted `spatial_ref`.
  - Given a layer that fails its metadata-integrity check, when it is requested, then it is excluded from the list (or returned with an explicit invalid flag), never served as valid.
- **Tests**: API contract (pagination + filters), geospatial (served spatial_ref matches stored), failure path (integrity-failed layer excluded).
- **Depends on**: 07-09, 07-10, `08` (viewer consumes).

### STORY 07-14 · M4 · M · P1 — Annotations, recommendations, reports records
- **Story**: As `AG`, I want scene annotation/recommendation/report records served with write-back audit, so that the viewer's annotate→recommend→report loop persists through the hub.
- **Deterministic / evidence**: CRUD for annotation/recommendation/report records via `routes.rs` with `{author, severity, timestamp, audit_id, links}`; geometry stored in the field CRS.
- **Acceptance**:
  - Given an annotation drawn in the viewer, when it is written back, then it is stored with author, severity, timestamp, and an audit ID, in the field CRS.
  - Given a recommendation referencing a deleted annotation, when it is created, then it is rejected with a dangling-reference error.
- **Tests**: API contract (CRUD + audit), unit (geometry CRS), failure path (dangling annotation reference).
- **Depends on**: 07-10, `08` (annotation UX), `09` (recommendations/reports).

### STORY 07-15 · M4 · S · P0 — Export (GeoJSON / CSV / GeoTIFF)
- **Story**: As `DSP`, I want annotations/recommendations exported as GeoJSON/CSV and scenes/layers as GeoTIFF, so that clients can use the geospatial data in other tools.
- **Deterministic / evidence**: GeoJSON carries correct CRS and properties; CSV rows match the records; GeoTIFF export round-trips the layer's CRS/extent (07-09); all validate against a schema.
- **Acceptance**:
  - Given annotations for a field, when export runs, then GeoJSON validates with correct CRS/properties and CSV rows match the records.
  - Given an empty selection, when export runs, then a valid empty export is produced, not a malformed file.
- **Tests**: geospatial round-trip (GeoTIFF, GeoJSON CRS), schema validation, failure path (empty → valid empty export).
- **Depends on**: 07-09, 07-14.

### STORY 07-16 · M4 · S · P2 — Mobile SPA search/analyze surface
- **Story**: As `GR`, I want to search and analyze scenes from the mobile SPA, so that I can browse field imagery without a desktop tool.
- **Deterministic / evidence**: `mobile_app.html` calls the search/analyze endpoints and renders results with their metadata; errors surface to the UI rather than failing silently.
- **Acceptance**:
  - Given a field, when the SPA searches, then scenes render with acquisition date and cloud cover from the API.
  - Given an API error, when the SPA loads, then the error is shown to the user rather than a blank screen.
- **Tests**: SPA integration (search/analyze render), failure path (API error surfaced).
- **Depends on**: 07-02, 07-13.

---

## M5 — Autonomous-Assist (gated)

### STORY 07-17 · M5 · M · P1 — Automated scene-refresh advisory
- **Story**: As `AG`, I want the hub to auto-detect a newer scene for a field and advise a refresh with an uncertainty flag, so that analyses stay current — once metadata correctness is reliable.
- **Deterministic / evidence**: scheduled best-scene search per active field; when a fresher, lower-cloud scene exists, emit a refresh advisory citing the candidate scene; gated behind 07-08/07-09 and the `09`/`10` advisor spine.
- **Acceptance**:
  - Given a field with a stored scene, when a fresher comparable scene appears, then a refresh advisory citing the candidate is emitted.
  - Given metadata correctness checks not yet reliable for the field, when refresh runs, then it is disabled (gated), not auto-ingesting unverified scenes.
- **Tests**: unit (freshness comparison), gating test (correctness unreliable → disabled), failure path (no fresher scene → no advisory).
- **Depends on**: 07-08, 07-09, 07-10, `09`, `10`.

### STORY 07-18 · M5 · M · P2 — Change-detection advisory across scenes
- **Story**: As `AG`, I want change between two linked scenes of a field flagged with evidence, so that I can spot field-level change without manual comparison.
- **Deterministic / evidence**: align two georeferenced scenes over a common extent; compute coarse change with coverage; flag uncertainty on CRS/extent/resolution mismatch; feature-flagged.
- **Acceptance**:
  - Given two comparable linked scenes, when change detection runs, then a change summary with an uncertainty band is produced over the common extent.
  - Given mismatched CRS/extent, when it runs, then it is marked low-confidence or unavailable, never silently differenced.
- **Tests**: unit (change + alignment), gating test (mismatch → low-confidence), failure path (single scene → no comparison).
- **Depends on**: 07-09, 07-10, `09`.

---

## Coverage note

These 18 stories cover all 12 capabilities in `capability-map.md`, ordered by phase and consistent with `release-plan.md` (M1 18 / M2 15 / M3 21 / M4 17 / M5 6; P0 34 / P1 29 / P2 14). Raster metadata correctness gates the whole advisor workflow: every ingested scene asserts CRS/extent/resolution (07-08) and round-trips its transform to the served layer (07-09) before the viewer (`08`) renders or the advisor (`09`) cites it; the scene → field → season spine (07-10) is built alongside `10`. The storage-backend question (07-11) is settled before catalog scaling. The curated ~77 backlog rows expand several stories into siblings — per-source dataset handling in 07-02, additional import formats in 07-04, and per-record-type export variants in 07-15 — when implemented.
