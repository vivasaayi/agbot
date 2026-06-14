# Flight and Mission Control: Release Plan

## Shipment Strategy

Ship in maturity order. Mission identity and validation (M1) come first, then trustworthy telemetry capture (M2), then deterministic safety and path logic (M3), then interactive dispatch with guardrails (M4). Autonomous mission execution (M5) is gated behind reliable single-drone control and the `03` safety work.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 18 |
| M2 captured | 16 |
| M3 explainable | 18 |
| M4 interactive | 18 |
| M5 autonomous-assist | 8 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 34 |
| P1 | 30 |
| P2 | 14 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 12 |
| M | 38 |
| S | 28 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Mission CRUD and persistence | geospatial correctness | identity |
| M1 foundation | M | Waypoint and flight-path model | safety | validation |
| M1 foundation | M | Survey-pattern templates | agronomic value | template |
| M2 captured | M | Live telemetry streaming and history | data quality | capture |
| M3 explainable | M | Geofence and no-fly enforcement | safety | evaluator |
| M3 explainable | S | Arming and pre-flight checklist | safety | evaluator |
| M4 interactive | L | MAVLink command interface | safety | operations |
| M4 interactive | M | Failsafe and return-to-home | safety | operations |

## Execution Rules

- Do not enable real-flight dispatch until the full path passes in `Simulation` mode against domain `02`.
- Every dispatch P0 must enforce geofence, altitude ceiling, battery budget, and a working abort.
- Every telemetry P0 must record freshness, gaps, and failsafe transitions.
- Do not start M5 autonomous execution until pre-flight checks, failsafe, and `03` collision safety are reliable.
</content>
