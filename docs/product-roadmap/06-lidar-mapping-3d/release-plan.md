# LiDAR Mapping and 3D Reconstruction: Release Plan

## Shipment Strategy

Ship in maturity order. Scan ingest and the occupancy/point-cloud products (M1) come first — these already exist and need hardening — then cleaned, coverage-tracked clouds (M2), then deterministic geometry and elevation products with correct georeferencing (M3), then interactive elevation/obstacle overlays and canopy-height for the advisor (M4). Real-time obstacle avoidance and autonomous terrain mapping (M5) are gated behind the `02` LiDAR sim and reliable segmentation.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 15 |
| M2 captured | 13 |
| M3 explainable | 21 |
| M4 interactive | 14 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 29 |
| P1 | 27 |
| P2 | 13 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 12 |
| M | 35 |
| S | 22 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Scan ingest and parallel loading | data quality | identity |
| M1 foundation | M | Occupancy grid construction | geospatial correctness | evaluator |
| M2 captured | M | Outlier and noise removal | data quality | capture |
| M3 explainable | M | Normal estimation | performance and scale | evaluator |
| M3 explainable | M | Ground/non-ground segmentation | agronomic value | evaluator |
| M3 explainable | L | DSM/DTM elevation products | geospatial correctness | evaluator |
| M4 interactive | M | Canopy-height model | agronomic value | evaluator |
| M4 interactive | S | Elevation/obstacle overlays | explainability and trust | overlay |

## Execution Rules

- Every occupancy and elevation P0 must assert CRS, extent, and resolution and round-trip for the viewer.
- Clean the point cloud (outlier/noise removal) before deriving any product — corrupt geometry produces wrong findings.
- Every deterministic product must retain its thresholds and observation counts as inspectable evidence.
- Run on captured `04` fixtures now; move to simulation-first once the `02` LiDAR sim lands.
- Do not start M5 obstacle avoidance or autonomous mapping until segmentation and the `03` safety work are reliable.
