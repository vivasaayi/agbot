# Post-Flight Analytics and Advisor

Turn deterministic remote-sensing products into anomalies, findings, recommendations, and grower-ready reports: the "report delivered" outcome.

## Where We Are

- `post_processor` has a real job queue (Queued/Processing/Completed/Failed) with NDVI, LiDAR, thermal, multispectral, composite, health, and yield job types.
- `AnalysisResult` carries rich statistics (min/max/mean/std/percentiles/coverage) and grid/point/zonal/time-series result data with priority-ranked recommendations.
- The hard analysis (multispectral, health, yield) returns synthetic sample data, anomaly detection and zone delineation are not real, and the report generator has scaffolded encoder paths that need real implementations.

## Where We Should Be

- The first revenue domain: deterministic stats and anomaly flags before any AI or yield claim, every finding tied to a field zone.
- Recommendations with priority, action category, and evidence linkage, written into the `10` domain model.
- A real PDF/CSV/GeoJSON report generator that produces a shareable, farmer-friendly deliverable.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Consume a real georeferenced product from `05`/`06` and compute deterministic zonal statistics.
2. Add evidence-cited anomaly flagging and zone delineation on the index grid.
3. Generate priority-ranked recommendations linked to anomalous zones.
4. Implement the PDF report encoder with field metadata, map views, and findings.
5. Add CSV and GeoJSON export of findings and zones.
6. Wire health/yield outputs behind explicit uncertainty and an evidence gate.

## Primary Crates

`post_processor`, with `shared` for schemas and `sensor_overlay_engine` for visualization. Consumes products from `05`/`06`, field context from `10`, and is presented through `08`.
