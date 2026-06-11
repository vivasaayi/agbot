# Farmers Portal: Release Plan

## Shipment Strategy

Ship in maturity order, weighted to the M1 foundation because this is a greenfield domain. The grower identity slice and a read-only field/farm overview (M1) come first, then the report inbox and recommendation tracking that consume `09` (M2), then audited recommendation status, notifications, and export (M3/M4). Marketplace (`18`) and community (`20`) entry points are post-MVP. There is little or no M5 work yet: this is a presentation/consumption surface, not an autonomy domain.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 22 |
| M2 captured | 16 |
| M3 explainable | 12 |
| M4 interactive | 12 |
| M5 autonomous-assist | 2 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 0 |
| P1 | 10 |
| P2 | 54 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 8 |
| M | 30 |
| S | 26 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | P1 | Grower account and org/role access (via `10`) | operability | identity |
| M1 foundation | M | P2 | Grower dashboard (home) | agronomic value | overview |
| M1 foundation | M | P2 | Field and farm overview | agronomic value | overview |
| M2 captured | M | P2 | Report inbox (consumes `09`) | explainability | consume |
| M2 captured | M | P2 | Recommendation tracking | explainability | audit |
| M3 explainable | S | P2 | Notifications and alert feed | operability | notify |
| M3 explainable | M | P2 | Field map and layer view (via `07`/`08`) | geospatial correctness | overlay |
| M4 interactive | M | P2 | Data export and sharing | explainability | export |

## Execution Rules

- This domain is sequenced AFTER the core drone platform (domains `01`-`12`) and is gated by the advisor MVP: there is no report inbox until `09` produces reports and `10` provides field/org context.
- The foundational P1 slice is grower identity/access; every other row is P2 (post-MVP).
- Every grower view is tenant-safe and resolved through the `10` org/role model; no read or write may cross a tenant boundary.
- The portal consumes `07`/`09`/`10` read-only; it does not own field, layer, or report data.
