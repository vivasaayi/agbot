# Water Management: Epic Breakdown

These epics are greenfield (M0): no code exists yet. Each is intended to ship as a vertical slice once the core drone platform (`01`–`12`) and the advisor MVP are in place.

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: route or command, persistence, auth/tenant scope (via `10`), pagination, and audit events.
- Deterministic: ET math, water-balance, and scheduling logic that runs without AI, with reason codes and retained inputs.
- Data quality: moisture readings carry source, freshness, coverage, and QA flags.
- Geospatial: every reading and zone asserts CRS/extent and resolves through `07`/`10`.
- Safety: valve/hardware actions require dry-run, bounds, and abort before execute.
- UI: zone moisture map, ET trend, schedule view, and savings report (consumed by `13`/`11`).
- Tests: unit (ET/water-balance math), fixture (sensor + RS readings), API contract, and one failure path (stale/missing weather input).
- Operations: feature flag or runtime mode, ingestion health, retry/backoff, and a runbook.

## Category Epics

### EPIC-01: Moisture Evidence Foundation
- Goal: a field has a trustworthy soil-moisture picture from sensors and remote-sensing proxies.
- First release: persist moisture readings linked to field/zone with source, freshness, and QA flag; ingest one NDWI/NDMI proxy from `05`.
- Expansion: multi-sensor fusion, coverage and gap reporting, per-zone aggregation.
- Hardening: calibration metadata, outlier handling, and full provenance.

### EPIC-02: Evapotranspiration and Water Need
- Goal: deterministic ET and a defensible per-zone water need before any recommendation.
- First release: reference ET from `15` weather inputs, cited; water-need mapping onto `09`/`05` zones.
- Expansion: crop ET (crop coefficients), water-balance over time, seasonal water budget.
- Hardening: method selection, sensitivity to missing inputs, and evidence audit.

### EPIC-03: Scheduling, Control, and Reporting
- Goal: turn water need into a scheduled, optionally executed irrigation plan with savings reporting.
- First release: per-zone water plan from moisture + ET evidence; dry-run against a valve adapter.
- Expansion: hardware/valve execute with audit, alerts to `13`/`11`, water-use vs. baseline reporting.
- Hardening: per-field irrigation history, savings analytics, and rollback/disable controls.
