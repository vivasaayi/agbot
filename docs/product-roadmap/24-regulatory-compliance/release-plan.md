# Regulatory and Compliance: Release Plan

## Shipment Strategy

Ship in maturity order with safety leading every phase. Compliance record identity and the airspace/no-fly-zone database (M1) come first, then Remote ID / regulatory flight logging and application-record capture (M2), then the deterministic authorization gate, REI/PHI windows, and buffer-zone separation (M3), then interactive certification management, data-residency controls, and audit-ready reports/exports (M4). Any AI assist (regulation summarization, filing helpers) is M5 and gated: it cites the deterministic record and never grants an authorization. Because this is a legal/safety surface, the authorization gate's hard-block behavior is sequenced as the first P0 the moment airspace and certs are real.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 18 |
| M2 captured | 16 |
| M3 explainable | 24 |
| M4 interactive | 16 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 28 |
| P1 | 34 |
| P2 | 18 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 14 |
| M | 42 |
| S | 24 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Compliance record identity and linkage | explainability and trust | identity |
| M1 foundation | M | Airspace / no-fly-zone database | geospatial correctness | evaluator |
| M2 captured | M | Remote ID and regulatory flight logging | explainability and trust | capture |
| M2 captured | M | Chemical / pesticide application records | explainability and trust | capture |
| M3 explainable | L | Pre-flight authorization checks | safety | evaluator |
| M3 explainable | M | Operator certification / license registry | safety | evaluator |
| M3 explainable | M | REI / pre-harvest interval (PHI) tracking | agronomic value | evaluator |
| M3 explainable | M | Spray drift and buffer-zone compliance | geospatial correctness | evaluator |
| M4 interactive | L | Audit-ready compliance reports and exports | explainability and trust | export |

## Execution Rules

- A flight that violates a deterministic airspace/no-fly-zone or certification rule must be hard-blocked with a reason code at the `01` pre-flight gate; on missing or stale airspace/cert data, deny by default — never authorize on uncertainty.
- Every compliance record (flight log, application, cert, authorization decision) must be append-only and written to the `30` provenance ledger; no record may be silently edited or deleted.
- Airspace zones and buffer zones must assert CRS/extent and round-trip as GeoJSON; buffer-zone separation from sensitive areas/water must be provable before an application is cleared.
- REI/PHI windows must be deterministically computed from the application record and gate re-entry/harvest; certifications carry expiry and `29` alerts fire ahead of it.
- Any AI assist (regulation summarization, report drafting) is M5, must cite the deterministic record, flag uncertainty, and never grant an authorization or clear a violation.
- Compliance reports/exports must be audit-ready, retention/residency-aware, and reproducible (same inputs → same report).
