# Water Management: Release Plan

## Shipment Strategy

Greenfield (M0): nothing ships until the moisture data model exists. Ship in maturity order, weighted to M1/M2: the soil-moisture data model and field/zone linkage come first (M1), then moisture and weather inputs flow with freshness and coverage (M2), then deterministic ET, water-need mapping, and scheduling make the output explainable (M3), then dry-run/execute control with savings reporting makes it interactive (M4). Autonomous closed-loop irrigation (M5) is gated behind reliable deterministic scheduling and a proven valve-control safety path.

This domain is sequenced AFTER the core drone platform (`01`–`12`) and is gated by the advisor MVP (`09`), because its zones and field context come from that workflow. Most rows are P2 (post-MVP); the foundational soil-moisture identity slice is P1.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 20 |
| M2 captured | 16 |
| M3 explainable | 14 |
| M4 interactive | 10 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 0 |
| P1 | 8 |
| P2 | 58 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 8 |
| M | 30 |
| S | 28 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- | --- |
| M1 foundation | M | P1 | Soil-moisture data model | data quality | identity |
| M1 foundation | S | P2 | Remote-sensing moisture proxies (from `05`) | data quality | ingest |
| M2 captured | S | P2 | Weather-input contract (from `15`) | data quality | ingest |
| M3 explainable | M | P2 | Evapotranspiration (ET) calculation | agronomic value | evaluator |
| M3 explainable | M | P2 | Zone-based water-need mapping | agronomic value | evaluator |
| M3 explainable | L | P2 | Irrigation scheduling engine | agronomic value | evaluator |
| M4 interactive | M | P2 | Irrigation hardware/valve control interface | safety | operations |
| M4 interactive | S | P2 | Water-use and savings reporting | agronomic value | reporting |

## Execution Rules

- Sequence this domain AFTER the core drone platform (`01`–`12`); do not start before the advisor MVP (`09`) can supply management zones.
- The foundational soil-moisture identity slice is the single P1; everything else is P2 (post-MVP).
- Deterministic ET and water-need must run and be inspectable before any AI or scheduling recommendation.
- Every moisture reading must carry source, freshness, and a QA flag; stale weather input must degrade gracefully, not silently.
- No valve/hardware action may execute without dry-run, bounds, abort, and an audit record.
- Do not start M5 closed-loop irrigation until deterministic scheduling and valve-control safety are reliable.
