# GIS and Geospatial Hub: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (geospatial correctness, data quality, operability, explainability, performance/scale) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated. Geospatial correctness is the dominant pillar for this domain.

## GIS and Geospatial Hub Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Hub configuration and runtime | strong partial | 5 | `HubConfig` for server/DB/Landsat with runtime mode |
| Landsat / USGS scene search | strong partial | 8 | Best-scene search per source with metadata |
| Scene ingest pipeline | strong partial | 8 | Download → process → store with freshness |
| Raster metadata correctness (CRS/extent/resolution) | partial | 9 | Assert and persist CRS/extent/resolution per scene |
| Spatial DB and storage backend | strong partial | 7 | Settle PostGIS vs SQLite and migrate (open question) |
| Shapefile and GeoJSON import | strong partial | 6 | Import field boundaries from shapefile/GeoJSON |
| Farm/field record management | strong partial | 7 | Farm/field CRUD with boundary linkage |
| Scene → field → season linkage | partial | 8 | Link an ingested scene to a field and season |
| Layer-serving REST API | strong partial | 8 | Paginated layer/metadata API the viewer trusts |
| Annotations, recommendations, reports | strong partial | 7 | Scene annotation/recommendation/report records |
| Export (GeoJSON / CSV / GeoTIFF) | strong partial | 6 | Export annotations/recommendations as GeoJSON/CSV |
| Mobile SPA surface | partial | 5 | Search/analyze scenes from the SPA |
