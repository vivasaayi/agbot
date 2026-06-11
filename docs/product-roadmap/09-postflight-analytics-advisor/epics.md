# Post-Flight Analytics and Advisor: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: job submit/status/result routes or commands with pagination and audit IDs.
- Deterministic: statistics, anomaly flags, and zone delineation computed without AI, with reason codes.
- Geospatial: findings carry CRS, extent, and per-zone area; outputs round-trip as GeoJSON.
- Explainability: every finding cites its evidence layer; AI/health/yield claims flag uncertainty.
- Agronomic: each finding ties to a recommendation and a field action.
- Tests: unit (stats/anomaly math), fixture (sample product grids), API contract, and one failure path (degenerate/empty raster).
- Operations: job health, retry/backoff, and a runbook.

## Category Epics

### EPIC-01: Deterministic Products and Anomalies
- Goal: real, inspectable statistics and anomaly flags from georeferenced products.
- First release: zonal statistics on a real `05`/`06` product plus threshold/outlier anomaly flagging with reason codes.
- Expansion: zone delineation with extent and area, and thermal hotspot detection.
- Hardening: reproducibility, evidence retention, and large-raster performance.

### EPIC-02: Findings to Recommendations
- Goal: turn anomalous zones into priority-ranked, action-categorized recommendations.
- First release: recommendation generation from a delineated zone, persisted into the `10` model.
- Expansion: linkage to annotations from `08` and status tracking (open/reviewed/completed/dismissed).
- Hardening: recommendation templates and approval-gated health/yield advisories.

### EPIC-03: Grower-Ready Reports and Export
- Goal: a shareable deliverable a grower can read without system access.
- First release: PDF report encoder with field metadata, map views, findings, and recommendations.
- Expansion: CSV and GeoJSON findings export and layer source details.
- Hardening: page-count/quality scoring, branded output, and delivery/storage with negative-path tests.
