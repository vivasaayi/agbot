# Regulatory and Compliance: Current State and Target State

## Mission

Make every flight and agronomic action legally defensible: maintain an airspace/no-fly-zone database, deterministically authorize or block flights before they launch, log Remote ID and flight records for authorities, track operator certifications and chemical applications with their REI/PHI windows, enforce drift/buffer-zone and data-residency rules, and produce audit-ready compliance reports — every record append-only and tied to the `30` provenance ledger.

## Current Maturity

greenfield pending (M0 named): no implementation exists; this domain is a product-vision module from `product-summary.md` (#19 Regulatory & Compliance). Nothing in the repository implements an airspace database, authorization evaluator, certification registry, application record, or compliance export.

## What Exists Now

- Nothing is built for this domain. There is no `compliance` crate, airspace store, authorization gate, application-record model, or compliance report encoder.
- Adjacent surfaces it would build on and parallel (already partially real):
  - Domain `01` (flight and mission control): the mission identity, dispatch, and pre-flight check hooks where an authorization gate must run before launch.
  - Domain `04` (sensor acquisition): the capture/session records a flight log and Remote ID stream attach to.
  - Domain `07` (GIS hub): the CRS-correct geometry storage an airspace/no-fly-zone database and buffer-zone evaluator reuse.
  - Domain `10` (field-farm-data): the org/field/season model that scopes ownership and links compliance records.
  - Domain `30` (provenance/audit ledger): the append-only, audit-grade record store every compliance artifact must write to.
  - Domain `29` (alerting/notification): the channel for cert-expiry, REI-clearance, and filing-deadline alerts.

## Gaps to Close

- No airspace / no-fly-zone database with CRS/extent-asserted zone geometries and effective-time windows.
- No deterministic pre-flight authorization evaluator that blocks a flight on an airspace or certification violation with reason codes.
- No Remote ID / regulatory flight logging that produces an authority-handable, append-only record.
- No operator certification/license registry with expiry tracking and a block on expired/missing certs.
- No chemical/pesticide application record (what/where/when/rate/operator) as a regulatory-mandated entity.
- No REI (restricted-entry interval) or pre-harvest-interval (PHI) computation/tracking from application records.
- No spray-drift / buffer-zone compliance evaluator asserting buffers around sensitive areas and water.
- No data-residency or retention-policy controls on compliance records.
- No audit-ready compliance reports/exports tied to the `30` provenance ledger.

## Related Existing Surfaces

- Domain `01` (flight and mission control): pre-flight check hook where the authorization gate runs; flight identity a log attaches to.
- Domain `04` (sensor acquisition): capture/session records that flight and Remote ID logging extend.
- Domain `07` (GIS hub): CRS-correct geometry storage reused for airspace zones and buffer-zone geometry.
- Domain `10` (field-farm-data): org/field/season ownership model scoping compliance records.
- Domain `30` (provenance/audit ledger): append-only, audit-grade store for every compliance record.
- Domain `29` (alerting/notification): delivery channel for expiry and deadline alerts.
- `docs/reference/product-summary.md` (#19 Regulatory & Compliance): the source description for this module.

## Target Operating Model

- Safety is non-negotiable: a flight that violates a deterministic airspace/no-fly-zone or certification rule cannot proceed; the authorization gate returns a hard block with a reason code, never a soft warning.
- Evidence before advice: every authorization and record is rule-checked, append-only, and inspectable; any AI assist (e.g. summarizing a regulation) cites the deterministic record and flags uncertainty, and never grants an authorization.
- Geospatial correctness is mandatory: airspace zones and buffer zones assert CRS/extent; the buffer-zone evaluator round-trips geometry and proves separation from sensitive areas and water.
- Chemical-application records carry what/where/when/rate/operator and drive deterministic REI/PHI windows that gate re-entry and harvest.
- Operator certifications carry expiry; an expired or missing cert blocks the associated flight, and `29` raises an alert ahead of expiry.
- Every record is append-only against `30`, subject to data-residency/retention policy, and exportable as an audit-ready, defensible report for authorities.
