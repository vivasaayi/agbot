# Import/Export and Interop Adapters: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: import/export and connector routes or commands with pagination, per-row reports, and audit IDs.
- Deterministic: format validation, CRS reprojection, geometry checks, and prescription assembly computed without AI, with reason codes — geospatial correctness is the dominant pillar.
- Geospatial: every import asserts/reprojects CRS and extent; every export preserves them; a round-trip test proves no coordinate drift.
- Explainability/trust: every import records its source, format, and CRS as provenance via `30`; interop fidelity is provable.
- Agronomic: prescription / VRA export ties a finding to a real machine action consumed by `14`.
- Tests: unit (validation/reprojection/assembly), fixture (sample files incl. oblique-CRS and malformed), round-trip, API contract, and one failure path (malformed/oblique-CRS rejected).
- Operations: connector mocks, simulation-first runtime, import/export health, and a runbook.

## Category Epics

### EPIC-01: Geospatial Interchange with CRS Fidelity
- Goal: import and export the core geospatial formats without losing CRS or extent.
- First release: a consolidated `interop` crate with a deterministic format-validation + CRS-reprojection pipeline, and Shapefile/GeoJSON round-trip with CRS preserved.
- Expansion: KML/KMZ, GeoPackage, and GeoTIFF import/export, plus field-boundary import into `10` with validation.
- Hardening: round-trip tests across all formats, oblique-CRS and malformed-file rejection, and report-generator consolidation from `09`.

### EPIC-02: Prescription / VRA Export (Finding to Action)
- Goal: turn management zones and per-zone rates into a file a machine executes.
- First release: prescription Shapefile export from `05`/`09` management zones with per-zone rates.
- Expansion: machine-executable ISO-XML / ISOBUS TaskData export consumed by `14`.
- Hardening: prescription round-trip and CRS-alignment tests, and a refusal path when zones do not align with the field CRS/extent.

### EPIC-03: Platform Connectors, Bulk, and Provenance
- Goal: exchange boundaries and prescriptions with external platforms and at scale, traceably.
- First release: a John Deere Operations Center connector for boundary/prescription exchange behind a mockable boundary.
- Expansion: Climate FieldView and Trimble connectors, bulk import/export and migration, and provenance-on-import via `30`.
- Hardening: connector failure/retry tests, bulk per-row reporting, and migration round-trip verification.
