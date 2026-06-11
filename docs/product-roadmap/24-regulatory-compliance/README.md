# Regulatory and Compliance

Flight law and agronomic compliance: airspace authorization, Remote ID and flight logging for authorities, operator certification, chemical-application records, REI/PHI tracking, drift/buffer-zone compliance, data residency, and audit-ready exports.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#19 Regulatory & Compliance); no compliance crate exists.
- The surfaces it would build on are partially real: pre-flight authorization (`01`), capture records (`04`), field/org context (`10`), and the provenance/audit ledger (`30`) for audit-grade, append-only records.
- Compliance is a legal and safety surface, so the safety, explainability/trust, and geospatial-correctness pillars dominate: every authorization and record must be deterministic, defensible, and inspectable.

## Where We Should Be

- An airspace/no-fly-zone database with pre-flight authorization checks that deterministically block a flight on violation, with reason codes — no flight proceeds against a hard rule.
- Remote ID and regulatory flight logging that authorities can be handed, plus an operator certification/license registry with expiry tracking.
- Chemical-application records (what/where/when/rate/operator) with REI and pre-harvest-interval (PHI) tracking, and geospatial drift/buffer-zone compliance around sensitive areas and water.
- Data-residency/retention controls and audit-ready compliance reports/exports, every record append-only and tied to the `30` provenance ledger.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0/P1 slices.

## Build Order

1. Compliance record identity/registry linked to org/field/flight via `10`/`01`, append-only against `30`.
2. Airspace/no-fly-zone database with CRS-asserted geometries (consumes `07`).
3. Deterministic pre-flight authorization check that blocks on airspace/cert violation with reason codes (gates `01`).
4. Operator certification/license registry with expiry tracking.
5. Chemical-application records plus REI/PHI tracking and Remote ID / regulatory flight logging.
6. Geospatial drift/buffer-zone compliance, data residency/retention controls, and audit-ready reports/exports.

## Primary Crates

New crate `compliance` (airspace DB, authorization evaluator, application/cert registries, report exporters). Builds on `01` (flight authorization), `04` (capture records), `10` (field/org), `07` (boundaries/airspace geometry), and `30` (provenance/audit ledger). Feeds `29` (alerting) for expiry/deadline alerts.
