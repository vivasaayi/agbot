# Regulatory and Compliance: Detailed Stories

> Greenfield domain (M0 named): no code exists yet. Every story below is **built from scratch** in a new `compliance` crate and is gated behind flight authorization (`01`), capture records (`04`), field/org context (`10`), CRS-correct geometry (`07`), and the append-only provenance ledger (`30`). This is a legal and safety surface, so the **safety, explainability/trust, and geospatial pillars dominate every phase**: a flight or application that violates a deterministic rule is hard-blocked with a reason code, and every record is append-only and defensible. The authorization gate's hard-block is the earliest P0 once airspace and certs are real.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Safety / deterministic**: the guardrail or inspectable logic that must hold without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `COMPLIANCE-OFFICER` compliance officer, `DSP` drone service provider, `OPS` operator, `AG` agronomist, `GR` grower, `PA` platform admin.

---

## M1 — Foundation

### STORY 24-01 · M1 · S · P0 — Compliance record identity and linkage
- **Story**: As `COMPLIANCE-OFFICER`, I want every compliance record to have a stable ID linked to org, field, and (where relevant) flight, written append-only to `30`, so that records are traceable and defensible.
- **Safety / deterministic**: persist `{record_id, record_type, org_id, field_id, flight_id?, created_at, actor, provenance_ref}`; records are append-only — updates create a new versioned row, never mutate in place; every write emits a `30` provenance entry.
- **Acceptance**:
  - Given an org and field, when a compliance record is created, then a record row exists with all linkage IDs and a `30` provenance ref.
  - Given an existing record, when a "change" is requested, then a new version is appended and the prior version is retained (no in-place mutation).
- **Tests**: unit (append-only versioning), API contract (create/list/version), failure path (attempt to delete/mutate is refused and audited).
- **Depends on**: `10` (org/field model), `30` (provenance ledger).

### STORY 24-02 · M1 · M · P0 — Airspace / no-fly-zone database
- **Story**: As `PA`, I want airspace and no-fly-zone geometries stored with CRS, extent, and effective-time windows, so that the authorization gate has authoritative zones to check against.
- **Safety / deterministic**: store zones as CRS-asserted geometries (via `07`) with `{zone_id, class, effective_from, effective_to, source}`; reject a zone whose CRS/extent cannot be asserted; queries are point/area-in-zone tests in the correct CRS.
- **Acceptance**:
  - Given a zone with valid geometry and CRS, when ingested, then it is stored and a point-in-zone query returns the correct membership in that CRS.
  - Given a zone geometry with missing/invalid CRS, when ingested, then it is rejected (not stored as ambiguous geometry).
- **Tests**: unit (point/area-in-zone), geospatial (CRS round-trip), failure path (invalid-CRS zone rejected).
- **Depends on**: 24-01, `07` (geometry storage).

---

## M2 — Captured / Observable

### STORY 24-03 · M2 · M · P0 — Remote ID and regulatory flight logging
- **Story**: As `COMPLIANCE-OFFICER`, I want each flight logged as an append-only Remote ID / regulatory record authorities can be handed, so that flights are accountable after the fact.
- **Safety / deterministic**: capture `{flight_id, operator_id, aircraft_id, track[], started_at, ended_at}` from the `04`/`01` session; record telemetry gaps explicitly; the log is append-only against `30`.
- **Acceptance**:
  - Given a completed flight, when logging runs, then a flight record persists with operator, aircraft, and track, linked to the flight and provenance.
  - Given a telemetry dropout during the flight, when logged, then the gap is recorded and flagged (not interpolated into a fabricated track).
- **Tests**: fixture (flight session), unit (gap flagging), API contract (retrieve log), failure path (gap surfaced, not hidden).
- **Depends on**: 24-01, `01`, `04`.

### STORY 24-04 · M2 · M · P0 — Chemical / pesticide application records
- **Story**: As `AG`, I want each chemical application recorded with what, where, when, rate, and operator, so that the regulatory-mandated application history exists and is defensible.
- **Safety / deterministic**: persist `{application_id, product, epa_or_label_ref, field_id, geometry, applied_at, rate, units, operator_id}`; geometry is CRS-asserted; the record is append-only and required before any REI/PHI computation.
- **Acceptance**:
  - Given a field and product, when an application is recorded, then all mandated fields persist with CRS-correct geometry and a provenance ref.
  - Given an application missing rate or product identity, when submitted, then it is rejected as incomplete (no partial regulatory record).
- **Tests**: unit (required-field validation), geospatial (geometry CRS), API contract, failure path (incomplete record rejected).
- **Depends on**: 24-01, `10`, `07`.

---

## M3 — Explainable (the deterministic compliance core)

### STORY 24-05 · M3 · L · P0 — Pre-flight authorization checks (hard block)
- **Story**: As `OPS`, I want a flight deterministically authorized or blocked before launch against airspace and certification rules, so that no flight proceeds against a hard rule.
- **Safety / deterministic**: the authorization evaluator checks the planned flight area/time against the airspace DB (24-02) and the operator's certs (24-06); a violation returns a hard block with `{reason_code, zone_ref|cert_ref}`; on missing/stale airspace or cert data, deny by default; the gate runs in the `01` pre-flight hook and abort/deny is the default.
- **Acceptance**:
  - Given a flight clear of all no-fly zones with valid certs, when authorized, then it is permitted and the decision is recorded.
  - Given a flight intersecting an active no-fly zone, when authorized, then it is hard-blocked with a zone reason code before launch.
  - Given missing or stale airspace data, when authorization is requested, then it is denied by default (never authorized on uncertainty).
- **Tests**: unit (authorization evaluator incl. deny-on-missing-data), geospatial (zone intersection in CRS), API contract (`01` gate), failure path (no-fly intersection blocks; stale data denies).
- **Depends on**: 24-02, 24-06, `01`, `07`.

### STORY 24-06 · M3 · M · P0 — Operator certification / license registry with expiry
- **Story**: As `COMPLIANCE-OFFICER`, I want operator certifications and licenses registered with expiry tracking, so that an expired or missing cert blocks the associated flight.
- **Safety / deterministic**: persist `{cert_id, operator_id, cert_type, issued_at, expires_at, authority}`; a deterministic check returns `valid|expired|missing` at flight time; an expired/missing cert is a hard block input to 24-05.
- **Acceptance**:
  - Given an operator with a valid, unexpired cert, when checked at flight time, then status is `valid`.
  - Given an operator whose cert expired before flight time, when checked, then status is `expired` and the flight is blocked.
  - Given an operator with no cert of the required type, when checked, then status is `missing` and the flight is blocked.
- **Tests**: unit (expiry/validity logic), API contract (register/list), failure path (expired and missing both block).
- **Depends on**: 24-01, feeds 24-05.

### STORY 24-07 · M3 · M · P0 — REI / pre-harvest interval (PHI) tracking
- **Story**: As `AG`, I want restricted-entry (REI) and pre-harvest (PHI) windows computed from each application, so that re-entry and harvest are gated until it is safe and legal.
- **Safety / deterministic**: from an application record (24-04) and its product label, deterministically compute `{rei_clear_at, phi_clear_at}`; a re-entry or harvest request before clearance is blocked with a reason code; windows are inspectable and cite the source application.
- **Acceptance**:
  - Given a recorded application with label REI/PHI, when windows are computed, then `rei_clear_at` and `phi_clear_at` are returned citing the application.
  - Given a re-entry request before `rei_clear_at`, when evaluated, then it is blocked with an REI reason code.
  - Given an application with a missing label interval, when computing, then the window is marked unknown and re-entry/harvest is blocked (never cleared on missing data).
- **Tests**: unit (REI/PHI math), integration (re-entry gate), failure path (missing label blocks).
- **Depends on**: 24-04.

### STORY 24-08 · M3 · M · P1 — Spray drift and buffer-zone compliance
- **Story**: As `AG`, I want applications checked against buffer zones around sensitive areas and water, so that drift compliance is provable, not assumed.
- **Safety / deterministic**: a deterministic buffer evaluator builds CRS-correct buffers around sensitive features (water, dwellings, organic fields) and asserts the application geometry maintains required separation; a breach blocks the application with `{reason_code, feature_ref, required_buffer, actual}`.
- **Acceptance**:
  - Given an application geometry outside all required buffers, when evaluated, then it is compliant and recorded.
  - Given an application overlapping a required water buffer, when evaluated, then it is blocked with a buffer reason code and the measured separation.
- **Tests**: unit (buffer/separation math), geospatial (buffer CRS round-trip), failure path (buffer breach blocks).
- **Depends on**: 24-04, 24-02, `07`.

### STORY 24-09 · M3 · S · P1 — Append-only evidence and reproducibility
- **Story**: As `COMPLIANCE-OFFICER`, I want every authorization decision and compliance check to retain its raw evidence and reason codes, so that a decision can be re-derived and defended.
- **Safety / deterministic**: re-running an authorization/REI/buffer check on the same inputs yields an identical decision; each decision stores the rule version, inputs, and reason code, append-only against `30`.
- **Acceptance**:
  - Given the same flight/application inputs, when a check re-runs, then it produces an identical decision and reason code.
  - Given a rule version change, when a check re-runs, then the new decision records the new rule version while the prior decision remains retained.
- **Tests**: determinism (same input → same decision hash), fixture, failure path (no in-place overwrite of a prior decision).
- **Depends on**: 24-05, 24-07, 24-08, `30`.

---

## M4 — Interactive (records, residency, and audit-ready export)

### STORY 24-10 · M4 · S · P1 — Compliance deadline and expiry alerting
- **Story**: As `COMPLIANCE-OFFICER`, I want alerts ahead of cert expiry, REI/PHI clearance, and filing deadlines, so that nothing lapses silently.
- **Safety / deterministic**: a deterministic scheduler emits alert events via `29` at configured lead times before `expires_at`/clearance/deadline; alerts cite the underlying record; a missing record source surfaces a "no data" alert rather than silence.
- **Acceptance**:
  - Given a cert expiring within the lead window, when the scheduler runs, then a `29` alert fires citing the cert.
  - Given a record source that is unreachable, when the scheduler runs, then a "source unavailable" alert is raised (not silent).
- **Tests**: unit (lead-time logic), integration (`29` delivery), failure path (source unavailable surfaced).
- **Depends on**: 24-06, 24-07, `29`.

### STORY 24-11 · M4 · S · P1 — Data residency and retention policy controls
- **Story**: As `PA`, I want compliance records subject to residency and retention policy, so that records live in the right jurisdiction for the required duration.
- **Safety / deterministic**: each record carries a residency tag and retention class; a deterministic policy enforces storage location and minimum retention before any expiry/redaction; retention/residency violations are blocked and audited.
- **Acceptance**:
  - Given a record with a residency tag, when stored, then it is placed per policy and a retention clock starts.
  - Given a deletion request before the minimum retention period, when evaluated, then it is refused with a retention reason code.
- **Tests**: unit (retention/residency policy), API contract, failure path (early deletion refused).
- **Depends on**: 24-01, `30`.

### STORY 24-12 · M4 · L · P0 — Audit-ready compliance report and export
- **Story**: As `COMPLIANCE-OFFICER`, I want a complete, defensible compliance report/export tied to `30` provenance, so that an authority can be handed the full record on demand.
- **Safety / deterministic**: the report assembler asserts field/org metadata and pulls flight logs, applications, certs, REI/PHI windows, and buffer checks, each with its provenance ref; the export validates against a schema; missing mandatory records fail the export rather than emitting a partial report.
- **Acceptance**:
  - Given a field and period, when a report is generated, then it contains flight logs, applications, certs, and clearance windows, each citing its `30` provenance ref.
  - Given a missing mandatory record, when generation runs, then it fails with a clear error (no partial/placeholder audit report).
- **Tests**: unit (section assembly + provenance linkage), schema validation, failure path (missing mandatory record fails export).
- **Depends on**: 24-03..24-09, 24-11, `30`.

### STORY 24-13 · M4 · S · P2 — Per-authority export formats and bounded sharing
- **Story**: As `DSP`, I want compliance exports in per-authority formats with bounded, revocable sharing, so that I can submit to the right body without granting system access.
- **Safety / deterministic**: format adapters emit authority-specific layouts from the same validated record set; a share artifact respects residency/retention and is revocable; revocation is audited.
- **Acceptance**:
  - Given a generated report, when exported for a specific authority, then the layout matches that authority's format and validates.
  - Given a shared report, when access is revoked, then the link no longer resolves and the revocation is audited.
- **Tests**: schema validation per format, API contract (share/revoke), failure path (revoked link denied).
- **Depends on**: 24-12, 24-11.

---

## M5 — Autonomous-Assist (gated, cites the record, never authorizes)

### STORY 24-14 · M5 · M · P2 — Regulation-summary assist (evidence-gated)
- **Story**: As `COMPLIANCE-OFFICER`, I want an AI assist that summarizes the applicable rules and drafts filing text, so that I prepare submissions faster — without it ever granting an authorization.
- **Safety / deterministic**: the assist is composed only from the deterministic records and rule set; every output cites the underlying record/rule and carries an uncertainty flag; it is feature-flagged and can never approve a flight, clear a violation, or alter a record.
- **Acceptance**:
  - Given trustworthy compliance records, when the assist runs, then it returns a summary/draft citing each source record and flagging uncertainty.
  - Given a request to "authorize" or "clear" a blocked item, when made to the assist, then it refuses and points to the deterministic gate (never overrides a hard block).
- **Tests**: unit (citation + uncertainty), gating test (cannot authorize/clear), failure path (override attempt refused).
- **Depends on**: 24-05, 24-09, 24-12.

---

## Coverage note

These 14 stories cover all 11 capabilities in `capability-map.md` (~1+ stories each, with the authorization core split across 24-05/24-06). The breakdown is safety- and explainability-led, with a heavy M3 deterministic core (authorization gate, certs, REI/PHI, buffer zones) reflecting that **a hard block on violation leads** in `release-plan.md`. The authorization gate, airspace DB, flight/application records, and audit export are P0; certifications and REI/PHI are P0 inputs to the gate. The single M5 story (regulation-summary assist) stays evidence-gated and can never authorize. The curated counts in `release-plan.md` (~80 rows) expand several of these (per-zone-class checks, per-authority formats, additional record types) into sibling stories when implemented.
