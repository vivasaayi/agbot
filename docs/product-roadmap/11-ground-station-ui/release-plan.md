# Ground Station UI: Release Plan

## Shipment Strategy

Ship in maturity order. A trustworthy receive-only console comes first: live telemetry binding and capture events (M2), then deterministic map rendering and mission overlays with correct georeferencing (M3), then interactive operator actions behind auth and `mission_control` guardrails (M4). This domain maps to milestone M3 (operations) and depends on domain `01` for the telemetry/status feed and the action path, and on domain `02` for simulation-first validation. Autonomy-assist controls (M5) are gated behind reliable single-drone control in `01`/`03`.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 12 |
| M2 captured | 16 |
| M3 explainable | 18 |
| M4 interactive | 18 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 30 |
| P1 | 26 |
| P2 | 14 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 10 |
| M | 34 |
| S | 26 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | WebSocket client and message dispatch | operability | transport |
| M2 captured | M | Live telemetry display and binding | data quality | capture |
| M2 captured | S | Connection and link-health indicators | operability | capture |
| M2 captured | M | Capture event timeline (LiDAR/image/NDVI) | data quality | capture |
| M3 explainable | L | Map rendering (basemap, position, path) | geospatial correctness | overlay |
| M3 explainable | M | Mission overlay (waypoints, geofence, no-fly) | geospatial correctness | overlay |
| M4 interactive | S | Operator auth and session | operability | operations |
| M4 interactive | M | Operator actions (dispatch, pause, RTH, abort) | safety | operations |

## Execution Rules

- The UI never commands the vehicle directly: every operator action routes through `mission_control` (domain `01`) guardrails and returns an ack.
- Do not enable any action control until the full loop passes in `Simulation` mode against domain `02`.
- Every telemetry P0 must surface freshness, gaps, and link state to the operator, never a hardcoded "Connected".
- Every map P0 must assert correct CRS and extent; a wrong overlay is worse than no overlay.
- Operator actions require auth and are audited before M4 ships.
