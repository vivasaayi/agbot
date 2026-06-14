# Weather Advisory System: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: ingestion/forecast/advisory route or command, persistence, auth scoped to org/field (via `10`), pagination, and audit events.
- Data quality: every weather value carries source, freshness, and provenance; stale or missing data is flagged, not silently used — this is the lead pillar.
- Deterministic: window and risk-threshold logic that runs without AI, with the inputs, thresholds, and freshness retained as evidence.
- Explainability: every alert and window cites the inputs and thresholds behind it and flags uncertainty/staleness.
- Consumers: operational windows feed `01`/`14`; GDD/ET feed `16`/`17`; alerts route to `11`/`13`.
- Tests: unit (window/threshold/GDD/ET math), fixture (forecast + sensor payloads), API contract, and one failure path (stale/missing data).
- Operations: feature flag, ingestion health and freshness monitoring, retry/backoff, and a runbook.

## Category Epics

### EPIC-01: Ingestion and Per-Field Forecast
- Goal: trustworthy weather data becomes a hyper-local per-field forecast.
- First release: forecast-API ingestion with provenance/freshness and a forecast keyed on a `10` field boundary.
- Expansion: on-field sensor ingestion and a historical weather store per field.
- Hardening: freshness/stale-data tests and ingestion health monitoring.

### EPIC-02: Window Advisor and Risk Alerts
- Goal: forecasts drive operational windows and crop-aware risk alerts.
- First release: a deterministic spray/flight window advisor feeding `01`/`14`, plus frost/heat/wind/precip threshold alerts.
- Expansion: crop-stage-aware thresholds and alert routing to `11`/`13`.
- Hardening: explainability (cited inputs/thresholds) and negative-path tests.

### EPIC-03: Agronomic Inputs and Verification
- Goal: weather feeds the downstream water and drought domains and proves itself over time.
- First release: growing-degree-day and reference-ET inputs feeding `16`/`17`.
- Expansion: forecast accuracy/verification against observed values.
- Hardening: provenance audit, export, and freshness SLAs.
