# GIS and Geospatial Hub

The geospatial spine: ingest scenes, guarantee raster metadata correctness, and serve the geospatial layers the viewer trusts and the advisor workflow depends on.

## Where We Are

- `geo_hub` has an axum REST server and routes, a spatial DB layer with shapefile storage, a Landsat (USGS) client for scene queries/metadata, an ingest pipeline, state management, and an HTML5 SPA mobile app.
- `HubConfig` covers server/DB/Landsat; routes cover farms, fields, scene annotations/recommendations/reports, GeoJSON/shapefile import, and CSV/GeoJSON export; test coverage is present.
- A `geo_hub.db` (SQLite) exists at the repo root; the authoritative storage backend (PostGIS vs SQLite) is an open question.

## Where We Should Be

- CRS, extent, and resolution treated as first-class, asserted contracts on every ingested scene and served layer.
- A scene → field → season linkage that forms the spine the advisor workflow walks.
- Landsat ingest verified against real USGS credentials, with a settled, authoritative storage backend.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Make CRS/extent/resolution first-class on ingest and assert them on every scene.
2. Verify the Landsat/USGS client against real credentials end to end.
3. Establish the scene → field → season linkage the advisor needs.
4. Settle the authoritative storage backend (PostGIS vs SQLite) and migrate.
5. Harden the layer-serving API (pagination, metadata correctness) the viewer (`08`) trusts.
6. Add scene/layer export (GeoJSON/GeoTIFF) and audit.

## Primary Crates

`geo_hub`, with `shared` for schemas (`FarmRecord`, `FieldRecord`, `RasterSpatialRef`, scene/annotation/recommendation/report records). Serves the viewer (`08`); gates the advisor workflow alongside domain `10`. This is a core Advisor-MVP domain.
