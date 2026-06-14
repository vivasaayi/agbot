# Geo Viewer and Visualization: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (geospatial correctness, agronomic value, explainability, data quality, operability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Geo Viewer and Visualization Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Field/scene catalog and selection | partial | 7 | Pick a field and scene from the `10` catalog |
| Tile layer rendering | partial | 7 | Render a scene product with presence/missing states |
| Georeferenced layer placement | early partial | 8 | Assert CRS/extent and place layer on correct ground |
| Field boundary overlay | missing | 6 | Draw the field boundary on the map |
| Layer toggle and product switching | missing | 6 | Toggle NDVI/thermal/source on the same field |
| CRS/extent/resolution readout | missing | 4 | Show CRS, extent, resolution, dimensions on screen |
| Point/polygon annotation workflow | partial | 9 | Create/edit point + polygon with severity and note |
| Recommendation create-from-annotation | partial | 6 | Build a recommendation from selected annotations |
| Report result overlay | early partial | 5 | Render report findings/zones on the field |
| Compare mode (season/product) | missing | 6 | Side-by-side or swipe across two scenes |
| Saved views and export | missing | 4 | Persist a named view and export a snapshot |
| Map navigation and cursor geolocation | partial | 5 | Zoom/pan with live lon/lat cursor readout |
