# Plugin / Extension SDK and Open Data: Release Plan

## Shipment Strategy

Ship in maturity order, weighted to the M1/M3 foundation because the security boundary must exist before any third-party code runs. The extension-point taxonomy, manifest, and registration come first (M1), then the capability/permission model, sandboxed execution, and version gating (M3) — these are P0 because an extension host without an enforced capability boundary is a liability, not a feature. The concrete extension points and the SDK land next (M4), and open-data publishing plus the plugin registry/marketplace follow (M4/M5). Operability and explainability/trust lead, with a security dimension running through every phase: a plugin can never exceed its declared capabilities.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 14 |
| M2 captured | 6 |
| M3 explainable | 22 |
| M4 interactive | 24 |
| M5 autonomous-assist | 8 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 22 |
| P1 | 30 |
| P2 | 22 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 13 |
| M | 36 |
| S | 25 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Plugin manifest and registration | operability | identity |
| M1 foundation | S | Extension-point taxonomy | operability | contract |
| M3 explainable | M | Capability / permission model | explainability | evaluator |
| M3 explainable | L | Sandboxed execution | operability | runtime |
| M3 explainable | M | Versioning and compatibility contract | operability | evaluator |
| M4 interactive | M | Custom spectral index extension point (`05`) | agronomic value | extension |
| M4 interactive | M | Custom processor / report template (`09`) | agronomic value | extension |
| M4 interactive | M | Open-data catalog and publishing | explainability | export |

## Execution Rules

- No plugin runs outside the capability/permission boundary: a plugin must declare its capabilities in its manifest, and the host must deny anything undeclared. A capability violation is a tested failure path that blocks execution and is audited.
- Every plugin is gated against the host API version before loading; an incompatible plugin is refused, never loaded with degraded behavior.
- Manifest validation is deterministic and inspectable; an invalid or unsigned manifest is rejected with reason codes, not loaded best-effort.
- Custom indices and map layers must preserve CRS/extent; a plugin that would emit an incorrectly georeferenced layer is rejected (a wrong overlay is worse than none).
- Every plugin-produced artifact records its plugin identity and version via `30` so extended outputs remain traceable.
- Published open data must carry license and attribution metadata and pass anonymization checks before it leaves the platform.
