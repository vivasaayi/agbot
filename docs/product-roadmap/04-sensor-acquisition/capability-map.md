# Sensor Acquisition and Data Capture: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety, geospatial correctness, data quality, performance and scale, operability, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Sensor Acquisition and Data Capture Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| LiDAR capture (RPLIDAR A3 serial) | medium partial (mock parse) | 7 | Validate real serial path against the sim with QA |
| Multispectral camera capture | medium partial (mock) | 7 | Capture georeferenced bands with calibration metadata |
| Simulated sensor readers | medium partial (thin) | 6 | Georeference simulated scans to a flight path |
| Capture session lifecycle | strong partial | 7 | Link session to flight (`01`) and field/scene (`10`) |
| Data record model and provenance | strong partial | 8 | Persist provenance (sensor/GPS/time/calibration) |
| File-based storage and retention | medium partial (load stubbed) | 8 | Make load/list/cleanup/stats query-complete |
| Spatial/temporal/type indexing | medium partial (in-memory only) | 7 | Index persisted records and answer `SearchQuery` |
| Session aggregates (distance/area/battery) | missing (0.0 placeholders) | 6 | Compute aggregates from telemetry track |
| Data export (JSON/CSV) | medium partial | 6 | Load session records before export (`export_session` TODO) |
| Geospatial export (GeoTIFF/KML/Shapefile) | missing | 6 | One geospatial export with CRS/extent preserved |
| Freshness, coverage, and failure handling | early partial | 6 | Track capture freshness and collection failures |
| Integrity and QA masking | missing | 5 | Checksum records and flag low-quality scans |
