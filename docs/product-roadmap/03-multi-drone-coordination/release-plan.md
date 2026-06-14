# Multi-Drone Coordination: Release Plan

## Shipment Strategy

Ship in maturity order, but gated on prerequisites: this domain depends on reliable single-drone flight (`01`) and safety being solid first. Swarm safety and constraints (M1/M3) come first, then collision avoidance and formations (M3), then coordinated coverage and assignment (M4). All coordinated execution is approval-gated; bounded swarm autonomy (M5) is last and only after `01`/`02` are trustworthy. This is Phase 4 work in the overall roadmap.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 12 |
| M3 explainable | 20 |
| M4 interactive | 16 |
| M5 autonomous-assist | 10 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 30 |
| P1 | 28 |
| P2 | 16 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 14 |
| M | 36 |
| S | 24 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Swarm registry and lifecycle | operability | identity |
| M3 explainable | M | Global constraints (geofence/altitude/no-fly) | safety | evaluator |
| M3 explainable | S | Safety violation detection and audit | safety | evaluator |
| M3 explainable | L | Collision-avoidance maneuvers | safety | evaluator |
| M3 explainable | M | Formation definition (Line/Grid/Circle/V) | geospatial correctness | geometry |
| M4 interactive | L | Coordinated actions (survey/coverage) | agronomic value | coverage |
| M4 interactive | M | Mission assignment strategies | performance and scale | assignment |
| M4 interactive | M | Approval-gated coordinated execution | safety | operations |

## Execution Rules

- Do not start coordinated execution until single-drone flight (`01`) and its safety path are reliable.
- Every coordinated maneuver must verify minimum separation before and after, with a working abort.
- Every swarm action must enforce global geofence, altitude, no-fly, and battery limits or raise an audited `SafetyViolation`.
- Validate all formations and maneuvers in the `02` twin before any real swarm flight.
- No autonomous swarm maneuver executes without an operator approval gate in v1.
