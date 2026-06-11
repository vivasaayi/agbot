# GIS and Geospatial Hub: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: axum route or ingest command, persistence, pagination, and audit events.
- Geospatial: CRS, extent, resolution, and transform asserted on ingest and round-tripped on serve.
- Deterministic: scene selection, ingest, and metadata validation that runs without AI.
- Data quality: scene freshness, coverage, and ingest-failure handling.
- UI: layers and metadata served to the viewer (`08`) and the SPA.
- Tests: unit (metadata validation), fixture (Landsat scene), API contract, and one failure path (ingest/credential failure).
- Operations: runtime mode, storage backend choice, processing health, and a runbook.

## Category Epics

### EPIC-01: Scene Ingest and Catalog
- Goal: a Landsat/USGS scene becomes a stored, cataloged, queryable layer.
- First release: best-scene search per source and download → process → store with freshness.
- Expansion: real-credential ingestion verified end to end with coverage and failure handling.
- Hardening: ingest retries, audit, and the scene catalog API.

### EPIC-02: Raster Metadata Correctness
- Goal: every scene and served layer carries provably correct geospatial metadata.
- First release: assert and persist CRS, extent, and resolution per ingested scene.
- Expansion: round-trip the transform through the served layer and export.
- Hardening: settle the authoritative storage backend (PostGIS vs SQLite) and migrate.

### EPIC-03: Field Spine and Layer Serving
- Goal: scenes link to fields and seasons and serve trustworthy layers to the viewer.
- First release: farm/field CRUD with boundary import and a scene → field → season link.
- Expansion: a paginated layer/metadata API and annotations/recommendations/reports per scene.
- Hardening: GeoJSON/CSV/GeoTIFF export, the SPA surface, and contract tests.
