# Regulatory and Compliance: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: compliance record route or command, append-only persistence, auth scoped to org/field (via `10`), pagination, and audit events written to `30`.
- Safety: the authorization gate blocks a flight on an airspace or certification violation with a reason code — no flight mutation proceeds against a hard rule; abort/deny is always the default on uncertainty.
- Deterministic: airspace checks, REI/PHI windows, buffer-zone separation, and expiry logic computed without AI, with reason codes and raw evidence retained.
- Geospatial: airspace zones and buffer zones assert CRS/extent and round-trip as GeoJSON; separation from sensitive areas/water is provable.
- Explainability: every record is inspectable and append-only; any AI assist cites the deterministic record and flags uncertainty, and never grants authorization.
- Tests: unit (window/separation/expiry math), fixture (airspace zones, application records, certs), API contract, and one failure path (violation blocks).
- Operations: retention/residency policy, alert delivery via `29`, and a runbook.

## Category Epics

### EPIC-01: Authorization and Airspace
- Goal: a flight is deterministically authorized or blocked before launch against airspace and certification rules.
- First release: airspace/no-fly-zone database (CRS-asserted) and a pre-flight authorization gate that blocks on violation with reason codes, wired into the `01` pre-flight hook.
- Expansion: operator certification/license registry with expiry that blocks flight on expired/missing certs.
- Hardening: effective-time windows, predicted-breach handling, and full negative-path tests (violation always blocks).

### EPIC-02: Records and Agronomic Compliance
- Goal: every flight and application produces a defensible, append-only regulatory record.
- First release: Remote ID / regulatory flight logging and chemical-application records (what/where/when/rate/operator), all append-only against `30`.
- Expansion: REI/PHI window computation and tracking, and spray-drift / buffer-zone compliance around sensitive areas and water.
- Hardening: data-residency/retention controls and expiry/deadline alerting via `29`.

### EPIC-03: Audit-Ready Reporting and Export
- Goal: hand an authority a complete, defensible compliance record on demand.
- First release: audit-ready compliance report/export tied to `30` provenance, with field/org metadata and record linkage.
- Expansion: per-authority report formats and bounded, revocable sharing.
- Hardening: export schema validation, retention-aware redaction, and reproducibility tests (same inputs → same report).
