# Field, Farm, and Data Management: Release Plan

## Shipment Strategy

Ship in maturity order, front-loaded on M1 because this is the Phase 0 product spine that gates the whole advisor workflow. Tenant-safe identity and the field/boundary spine come first (M1), then season/crop-plan history and scene/layer ownership make field context observable (M2), then audited annotation/recommendation/report persistence makes the workflow explainable and accountable (M3/M4). This domain maps to milestone M1 (platform foundation) and M3 (collaboration: orgs, roles, work orders, field history).

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 28 |
| M2 captured | 16 |
| M3 explainable | 16 |
| M4 interactive | 14 |
| M5 autonomous-assist | 4 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 38 |
| P1 | 26 |
| P2 | 14 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 10 |
| M | 40 |
| S | 28 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Organization and user model | operability | identity |
| M1 foundation | M | Roles and access control | operability | identity |
| M1 foundation | M | Tenant isolation | explainability | safety |
| M1 foundation | M | Farm and field entities | agronomic value | identity |
| M1 foundation | M | GeoJSON boundary import | geospatial correctness | ingest |
| M2 captured | M | Season and crop-plan history | agronomic value | identity |
| M2 captured | M | Scene and layer registry | data quality | identity |
| M3 explainable | M | Annotation persistence and audit | explainability | audit |

## Execution Rules

- Every entity belongs to an organization; no read or write may cross a tenant boundary.
- Every boundary import must assert CRS and extent and round-trip as GeoJSON.
- Every annotation, recommendation, and work order must carry author and change history.
- This spine ships before `07`/`08`/`09` depend on it for field context; identity and isolation are non-negotiable P0.
