# GIS and Geospatial Hub: Current State and Target State

## Mission

Be the geospatial spine of the platform: ingest scenes, guarantee raster metadata correctness, link scenes to fields and seasons, and serve the geospatial layers the viewer renders and the advisor workflow trusts.

## Current Maturity

strong partial: `geo_hub` has a real axum server, routes, spatial DB layer, Landsat client, ingest pipeline, and an SPA mobile app with test coverage; CRS/extent contracts, real-credential ingestion, and the scene → field → season spine still need hardening, and the authoritative storage backend is unsettled.

## What Exists Now

- `HubConfig` covering server, database, and Landsat configuration (`geo_hub/src/config.rs`, `lib.rs`).
- Landsat module: a USGS API client for scene queries and metadata, dataset selection per source, best-scene search, and product PNG/statistics rendering (`geo_hub/src/landsat.rs`).
- Spatial DB layer with shapefile storage (`geo_hub/src/{db.rs,shapefile.rs}`).
- Ingest pipeline: download → process → store Landsat scenes (`geo_hub/src/ingest.rs`, `IngestLandsatArgs`).
- axum REST server and routes: scene search, farms/fields CRUD, scene annotations/recommendations/reports, GeoJSON/shapefile import, and CSV/GeoJSON export (`geo_hub/src/{server.rs,routes.rs}`).
- State management for active queries and a results cache (`geo_hub/src/state.rs`).
- An HTML5 SPA mobile app (`geo_hub/src/mobile_app.html`) with search/analyze endpoints.
- `AgroError` types and present test coverage (`geo_hub/src/error.rs`).
- A `geo_hub.db` (SQLite) at the repo root.

## Gaps to Close

- Landsat integration is API-correct but untested against real USGS credentials end to end.
- CRS, extent, and resolution are not yet first-class, asserted contracts on ingested scenes or served layers.
- Georeferencing correctness of served layers is not provably asserted or round-tripped.
- The scene → field → season linkage — the spine the advisor workflow needs — is incomplete.
- The authoritative storage backend is an open question: PostGIS vs the existing SQLite `geo_hub.db`.
- Layer-serving API pagination, freshness, and metadata-correctness guarantees need hardening.

## Source Modules Reviewed

- `geo_hub/src/lib.rs`, `geo_hub/src/config.rs`
- `geo_hub/src/server.rs`, `geo_hub/src/routes.rs`
- `geo_hub/src/landsat.rs`, `geo_hub/src/ingest.rs`
- `geo_hub/src/db.rs`, `geo_hub/src/shapefile.rs`, `geo_hub/src/state.rs`
- `geo_hub/src/mobile_app.html`, `geo_hub/src/error.rs`
- `shared/src/schemas.rs` (`FarmRecord`, `FieldRecord`, `RasterSpatialRef`, scene/annotation/recommendation/report records)

## Target Operating Model

- Every ingested scene carries an asserted CRS, extent, resolution, and transform that round-trip through the served layer.
- Landsat ingest verified against real USGS credentials with freshness, coverage, and failure handling.
- A scene → field → season spine that the advisor workflow walks from raster to finding to report.
- One settled, authoritative storage backend (PostGIS or SQLite) with migrations, chosen against the open confirmation question.
- A layer-serving API with pagination, freshness, and metadata-correctness guarantees the viewer (`08`) can trust.
- Scene/layer export (GeoJSON/GeoTIFF) and audit, with at least the happy path and one ingest-failure path tested.
