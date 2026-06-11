# Sensor Acquisition and Data Capture: Release Plan

## Shipment Strategy

Ship in maturity order. Capture identity and provenance (M1) come first so every record is traceable to a flight (`01`) and field/scene (`10`). Then observable capture with freshness/coverage/failure handling (M2), then query-complete storage, indexing, and real aggregates (M3), then export and inspect workflows (M4). Capture is the input layer for `05`/`06`/`09`, so correctness and provenance outrank breadth of formats.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 20 |
| M3 explainable | 18 |
| M4 interactive | 14 |
| M5 autonomous-assist | 4 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 30 |
| P1 | 28 |
| P2 | 14 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 10 |
| M | 36 |
| S | 26 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Capture session lifecycle | operability | identity |
| M1 foundation | M | Data record model and provenance | geospatial correctness | identity |
| M2 captured | M | LiDAR capture (RPLIDAR A3 serial) | data quality | capture |
| M2 captured | M | Multispectral camera capture | data quality | capture |
| M2 captured | S | Freshness, coverage, and failure handling | data quality | capture |
| M3 explainable | M | File-based storage and retention | operability | storage |
| M3 explainable | S | Session aggregates (distance/area/battery) | explainability and trust | evaluator |
| M4 interactive | M | Data export (JSON/CSV) | agronomic value | export |

## Execution Rules

- Every captured record must carry provenance (sensor, GPS, timestamp, calibration) and link to a flight (`01`) and field/scene (`10`).
- No session is "captured" without freshness, coverage, and a collection-failure path.
- Storage and indexing P0s must operate over persisted records, not just in-memory state.
- Replace 0.0 aggregate placeholders with telemetry-derived values before claiming session metrics.
- Do not ship export breadth before `export_session` correctly loads records and CSV/JSON pass contract tests; feature-gate `unimplemented!` formats.
