# Simulation and Digital Twin: Release Plan

## Shipment Strategy

Ship in maturity order. A deterministic physics/sensor twin with golden fixtures (M1/M3) comes first because it is what makes simulation-first testing trustworthy for `01` and `03`. Then capture-shaped sensor simulation and georeferenced terrain (M2/M3) to feed `04`/`06`. Interactive mission preview and a resolved canonical twin (M4) follow. Closed-loop autonomy preview against `03` (M5) is last.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 18 |
| M3 explainable | 20 |
| M4 interactive | 16 |
| M5 autonomous-assist | 8 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 32 |
| P1 | 30 |
| P2 | 16 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 14 |
| M | 38 |
| S | 26 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Per-drone physics (gravity/drag/thrust/battery) | explainability and trust | regression |
| M1 foundation | M | PID flight controller and command modes | safety | regression |
| M3 explainable | S | Sensor suite (GPS/IMU/baro/mag) | data quality | evaluator |
| M3 explainable | M | Wind and aerodynamic disturbance | performance and scale | physics |
| M2 captured | L | LiDAR sensor simulation | data quality | capture |
| M3 explainable | M | OSM map-tile and terrain loading | geospatial correctness | terrain |
| M4 interactive | M | Bevy globe and flight UI | agronomic value | preview |
| M4 interactive | M | Twin-as-backend for flight/coordination | safety | operations |

## Execution Rules

- The twin must enforce the same geofence/altitude/battery limits as the real path, or sim-first testing is not meaningful.
- Every physics/controller P0 ships with a deterministic, seeded golden-telemetry fixture run in CI.
- Every terrain P0 must assert CRS, extent, and resolution, and round-trip a known coordinate.
- Resolve the canonical-simulator question (Rust/Bevy vs C++) before investing in a second mission-preview surface.
- Do not ship the LiDAR/camera sim as "done" until it emits capture-shaped output consumable by domain `04`.
