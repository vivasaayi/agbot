# Carbon and Sustainability Tracking: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: sustainability service route or command, persistence, auth scoped to org/role (via `10`), pagination, and audit events.
- Identity: every record is attributed to a field, season, and operation through the `10` spine; no metric is orphaned.
- Deterministic: carbon, biomass, and KPI math runs without AI; any AI summary cites the deterministic output and the input layers it came from.
- Geospatial: biomass/biodiversity outputs assert CRS/extent through `07` and round-trip their georeferencing.
- MRV: every output records inputs, method, version, georeference, and audit IDs so a third party can reproduce it.
- Tests: unit (carbon/biomass/KPI math), fixture (sample `05`/`06` layers), API contract, and one failure path (missing/stale input layer rejected).
- Operations: feature flag, evidence-trail health, and a runbook.

## Category Epics

### EPIC-01: Footprint and Identity Foundation
- Goal: a field/operation carries a deterministic, attributed carbon footprint.
- First release: sustainability record identity (via `10`) and a carbon-footprint model from logged operation inputs.
- Expansion: per-field aggregation and a sustainability KPI catalog tracked against targets.
- Hardening: provenance, method versioning, and footprint-math tests.

### EPIC-02: Biomass, Biodiversity, and Baselines
- Goal: defensible, georeferenced environmental assessments that change over time.
- First release: biomass/canopy estimation consuming `06` canopy height and `05` indices, georeferenced through `07`.
- Expansion: biodiversity assessment from imagery, soil-carbon proxies, and a baseline plus time-series comparison.
- Hardening: CRS/extent round-trip assertions and uncertainty bounds on every proxy.

### EPIC-03: MRV and Certification Evidence
- Goal: every number is verifiable and exportable as a certification evidence pack.
- First release: an MRV evidence trail recording inputs, method, version, georeference, and audit per output.
- Expansion: certification evidence packs (via `09`) and GeoJSON/CSV/PDF export.
- Hardening: third-party reproducibility checks and a tamper-evident audit trail.
