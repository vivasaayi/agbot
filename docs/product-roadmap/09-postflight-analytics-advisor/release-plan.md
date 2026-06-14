# Post-Flight Analytics and Advisor: Release Plan

## Shipment Strategy

Ship in maturity order with an evidence-before-advice discipline. Deterministic statistics and anomaly flags on real georeferenced products come first (M3), then findings-to-recommendations and grower-ready reports (M4). Crop-health and yield outputs (M5 advisory-assist) stay gated behind explicit uncertainty and are never enabled before the deterministic products and anomaly flags are trustworthy. This is the first revenue domain, so the report-delivered outcome is sequenced early once anomalies are real.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 9 |
| M2 captured | 8 |
| M3 explainable | 24 |
| M4 interactive | 20 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 31 |
| P1 | 24 |
| P2 | 12 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 12 |
| M | 35 |
| S | 20 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Processing job queue and lifecycle | operability | identity |
| M3 explainable | M | Deterministic zonal statistics | explainability | evaluator |
| M3 explainable | M | Anomaly flagging | agronomic value | evaluator |
| M3 explainable | M | Zone delineation | geospatial correctness | evaluator |
| M4 interactive | M | Recommendation generation | agronomic value | interaction |
| M4 interactive | L | Report generation (PDF) | agronomic value | export |
| M4 interactive | S | Findings export (CSV/GeoJSON) | geospatial correctness | export |
| M5 autonomous-assist | M | Crop health assessment | explainability | evaluator |

## Execution Rules

- Deterministic statistics and anomaly flags must run and be inspectable before any AI, health, or yield output is enabled.
- Every finding must cite its evidence layer, carry a reason code, and tie to a georeferenced zone.
- Every recommendation P0 must persist into the `10` domain model with priority, action category, and status.
- Health and yield slices must flag uncertainty and are approval-gated; they never precede the deterministic products.
- Reports must assert correct field metadata and layer source details before delivery.
