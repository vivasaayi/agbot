# Drought Management: Release Plan

## Shipment Strategy

Greenfield (M0): nothing ships until the drought-index data model exists. Ship in maturity order, weighted to M1/M2: the drought-index and stress-evidence model and field/region linkage come first (M1), then satellite + weather fusion flows with freshness and coverage (M2), then deterministic baselines, trends, and risk scoring make the output explainable (M3), then risk-gated AI forecasts, early warnings, and mitigation recommendations make it interactive (M4). Any closed-loop or autonomous advisory (M5) is gated behind reliable deterministic scoring and a proven evidence-before-advice path.

This domain is sequenced AFTER the core drone platform (`01`–`12`) and is gated by the advisor MVP (`09`), because its stress evidence and recommendations run through that workflow. Most rows are P2 (post-MVP); the foundational drought-index identity slice is P1.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 20 |
| M2 captured | 16 |
| M3 explainable | 16 |
| M4 interactive | 10 |
| M5 autonomous-assist | 3 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 0 |
| P1 | 7 |
| P2 | 58 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 8 |
| M | 31 |
| S | 26 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- | --- |
| M1 foundation | M | P1 | Drought-index data model (SPI/SPEI-style) | explainability | identity |
| M1 foundation | S | P2 | Vegetation-stress evidence (from `05`) | data quality | ingest |
| M2 captured | M | P2 | Satellite + weather data fusion | data quality | ingest |
| M3 explainable | S | P2 | Historical baselines and seasonal trends | explainability | evaluator |
| M3 explainable | M | P2 | Per-field/region drought risk scoring | explainability | evaluator |
| M4 interactive | M | P2 | AI drought forecast (evidence-gated) | explainability | evaluator |
| M4 interactive | S | P2 | Early-warning and alerting | agronomic value | operations |
| M4 interactive | M | P2 | Mitigation strategy recommendations (to `16`/`09`) | agronomic value | reporting |

## Execution Rules

- Sequence this domain AFTER the core drone platform (`01`–`12`); do not start before the advisor MVP (`09`) can supply stress evidence and carry recommendations.
- The foundational drought-index identity slice is the single P1; everything else is P2 (post-MVP).
- Evidence before advice is non-negotiable: a deterministic index and risk score must run and be inspectable before any AI forecast is shown.
- Every AI forecast must cite its evidence layer and flag uncertainty; a wrong drought call is treated as a high-cost error.
- Every fused input must carry source, freshness, and coverage; stale satellite or weather input must degrade gracefully, not silently.
- Do not start M5 autonomous advisory until deterministic scoring and the evidence-before-advice gate are reliable.
