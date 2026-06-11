# Carbon and Sustainability Tracking: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (explainability and trust, geospatial correctness, data quality, agronomic value, operability) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Carbon and Sustainability Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Sustainability record identity (via `10`) | missing (greenfield) | 6 | Create a carbon/sustainability record owned by field/season/operation |
| Carbon-footprint model (per operation/field) | missing (greenfield) | 8 | Deterministic footprint from logged operation inputs, evidence-cited |
| Biomass / canopy estimation (consumes `06`/`05`) | missing (greenfield) | 8 | Estimate biomass from `06` canopy height + `05` indices, georeferenced |
| Soil-carbon proxies | missing (greenfield) | 6 | Proxy soil-carbon model with explicit uncertainty bounds |
| Biodiversity assessment from imagery | missing (greenfield) | 7 | Habitat/heterogeneity proxy from `05` imagery over a field |
| Sustainability KPI tracking | missing (greenfield) | 6 | KPI catalog tracked against a per-field target |
| Baseline and time-series comparison | missing (greenfield) | 7 | Compare a metric to a stored season baseline |
| MRV evidence trail | missing (greenfield) | 7 | Record inputs/method/version/georef/audit per output |
| Certification evidence packs (via `09`) | missing (greenfield) | 6 | Export a verifiable evidence pack for one claim |
| Export and reporting | missing (greenfield) | 5 | GeoJSON/CSV/PDF export of a field sustainability summary |
