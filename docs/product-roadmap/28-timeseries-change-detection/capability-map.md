# Time-Series and Change Detection: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (geospatial correctness and explainability first, then agronomic value, data quality, performance and scale) and the workstreams in `release-plan.md`. This is a flagship reusable subsystem: today only thin slivers exist (a "compare" capability in `08`, a "trend vs last flight" line in `09`), so the Primary First Slice promotes each capability into the shared `timeseries` engine rather than a per-domain feature. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Time-Series and Change Detection Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Generic time-series store (scalar + raster) | missing (promote) | 10 | Store/read series keyed by `(entity, metric, time)` |
| Reusable time-series API | missing (promote) | 8 | Append/query API any domain plugs into |
| Temporal alignment / co-registration | missing | 10 | Resample two scenes onto a common grid/CRS/resolution |
| Alignment QA guard (refuse uncoregistered) | missing | 7 | Reject comparisons without co-registration proof |
| Raster change detection (delta / mask) | thin (08 compare) | 9 | Per-pixel delta + threshold change mask with CRS assert |
| Zonal trend analysis (slope / trajectory) | thin (09 trend line) | 8 | Metric trajectory + slope per field/zone |
| Baseline and seasonality | missing | 7 | Rolling baseline + season-over-season delta |
| Change events (detect + rank) | missing | 8 | Rank significant changes with reason codes + evidence |
| Reusable consumer integrations (09/15/16/17/19/25/27) | missing | 9 | Vegetation trend (`09`) on the shared engine |
| Compare view feed to `08` | thin (08 compare) | 5 | Serve aligned two-date pair + change mask to `08` |
| Export (CSV / GeoTIFF / GeoJSON) | missing | 7 | Series CSV, change-mask GeoTIFF, change-zone GeoJSON |
| Forecast / gap-fill (uncertainty-flagged) | missing | 5 | Trend projection + interpolation with uncertainty band |
| Closed-loop change hook (auto-propose re-fly) | missing | 4 | Significant change drafts an approval-gated re-fly mission |
