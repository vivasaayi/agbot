# Weather Advisory System: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (data quality and explainability and trust lead here, then agronomic value, safety, geospatial correctness, operability) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Weather Advisory Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Weather data ingestion (forecast APIs) | missing (greenfield) | 8 | Pull a forecast for a field point, normalized + timestamped |
| On-field sensor ingestion | missing (greenfield) | 6 | Ingest one sensor stream with freshness + provenance |
| Data provenance and freshness | missing (greenfield) | 6 | Assert source/freshness on every weather value |
| Hyper-local per-field forecast | missing (greenfield) | 8 | Resolve a forecast keyed on a `10` field boundary |
| Spray/flight window advisor (feeds `01`/`14`) | missing (greenfield) | 8 | Deterministic wind/precip window for a field |
| Frost / heat / wind / precip risk alerts | missing (greenfield) | 8 | Raise a threshold-breach alert with cited inputs |
| Crop-stage-aware recommendations | missing (greenfield) | 6 | Adjust a risk threshold by crop stage from `10` |
| Growing-degree-day inputs (feeds `16`/`17`) | missing (greenfield) | 5 | Compute daily GDD per field from temperature |
| Evapotranspiration inputs (feeds `16`/`17`) | missing (greenfield) | 5 | Compute reference ET per field for irrigation |
| Historical weather per field | missing (greenfield) | 5 | Store and query a field's weather history |
| Alert routing (-> `11`/`13`) | missing (greenfield) | 5 | Route one alert type to operator/portal |
| Forecast accuracy / verification | missing (greenfield) | 4 | Compare a past forecast to observed values |
