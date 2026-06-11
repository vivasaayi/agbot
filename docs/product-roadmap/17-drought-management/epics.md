# Drought Management: Epic Breakdown

These epics are greenfield (M0): no code exists yet. Each is intended to ship as a vertical slice once the core drone platform (`01`–`12`) and the advisor MVP are in place. The defining discipline is evidence before advice: deterministic indices and stress evidence ship and are inspectable before any AI forecast.

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: route or command, persistence, auth/tenant scope (via `10`), pagination, and audit events.
- Deterministic: drought-index, baseline, and risk-scoring math that runs without AI, with reason codes and retained inputs.
- Geospatial: every index, scene, and score asserts CRS/extent and resolves through `07`/`10`.
- Data quality: fused satellite/weather inputs carry source, freshness, and coverage.
- Explainability: any AI forecast cites its deterministic evidence layer and flags uncertainty.
- UI: drought index map, trend vs. baseline, risk score, and alert feed (consumed by `13`/`11`).
- Tests: unit (index/baseline/score math), fixture (Landsat + weather inputs), API contract, and one failure path (stale/missing satellite or weather input).
- Operations: feature flag or runtime mode, ingestion health, retry/backoff, and a runbook.

## Category Epics

### EPIC-01: Drought Evidence Foundation
- Goal: a field and region have deterministic drought indices and stress evidence, traceable to inputs.
- First release: persist an SPI/SPEI-style index linked to field/region with inputs cited; ingest one stress index from `05`.
- Expansion: satellite + weather data fusion into one georeferenced store; historical baselines and seasonal trends.
- Hardening: coverage/freshness tracking, input QA, and full provenance.

### EPIC-02: Risk Scoring and Early Warning
- Goal: a defensible per-field/region risk score and timely warnings, evidence shown before any AI call.
- First release: deterministic risk score from index + stress evidence; threshold-crossing alerts to `13`/`11`.
- Expansion: an evidence-gated AI drought forecast that cites its evidence and flags uncertainty.
- Hardening: alert deduplication, escalation, and audit of every warning.

### EPIC-03: Mitigation and Reporting
- Goal: turn drought risk into a real, recommended field action with reporting.
- First release: mitigation recommendations tied to irrigation (`16`) and advisor (`09`).
- Expansion: per-field/region drought report with evidence and trend; drought history.
- Hardening: recommendation audit, outcome tracking, and rollback/disable controls.
