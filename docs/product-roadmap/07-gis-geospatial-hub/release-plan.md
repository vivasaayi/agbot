# GIS and Geospatial Hub: Release Plan

## Shipment Strategy

Ship in maturity order. Hub config, scene search, and ingest identity (M1) come first, then real-credential ingest with freshness and coverage (M2), then raster metadata correctness with asserted CRS/extent/resolution and the field spine (M3), then interactive layer serving, annotations/recommendations/reports, and export (M4). Automated scene-refresh and change-detection advisories (M5) are gated behind metadata correctness and the `09`/`10` advisor spine.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 18 |
| M2 captured | 15 |
| M3 explainable | 21 |
| M4 interactive | 17 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 34 |
| P1 | 29 |
| P2 | 14 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 13 |
| M | 40 |
| S | 24 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Hub configuration and runtime | operability | identity |
| M1 foundation | M | Landsat / USGS scene search | data quality | capture |
| M2 captured | M | Scene ingest pipeline | data quality | capture |
| M3 explainable | L | Raster metadata correctness (CRS/extent/resolution) | geospatial correctness | evaluator |
| M3 explainable | M | Scene → field → season linkage | agronomic value | identity |
| M3 explainable | M | Spatial DB and storage backend | operability | operations |
| M4 interactive | M | Layer-serving REST API | geospatial correctness | operations |
| M4 interactive | S | Export (GeoJSON / CSV / GeoTIFF) | explainability and trust | reporting |

## Execution Rules

- Every ingested scene and served layer P0 must assert CRS, extent, and resolution and round-trip the transform — a wrong overlay is worse than no overlay.
- Verify Landsat ingest against real USGS credentials before declaring the capture path done.
- Settle the authoritative storage backend (PostGIS vs SQLite `geo_hub.db`) before scaling the catalog — this is an open confirmation question.
- The scene → field → season spine gates the advisor workflow; build it alongside domain `10`.
- Do not start M5 scene-refresh or change-detection advisories until metadata correctness and the field spine are reliable.
