# Weather Advisory System: Release Plan

## Shipment Strategy

Ship in maturity order, weighted to the M1 foundation because this is a greenfield domain. Weather ingestion with provenance and a per-field forecast (M1) come first, then observable freshness/coverage and historical capture (M2), then the deterministic, explainable window advisor and risk alerts (M3), then interactive alert routing and crop-stage tuning (M4). The data-quality and explainability pillars lead every phase: advice gates real field actions, so every value carries source and freshness and every alert cites its inputs. There is little M5 work yet.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 22 |
| M2 captured | 16 |
| M3 explainable | 16 |
| M4 interactive | 10 |
| M5 autonomous-assist | 4 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 0 |
| P1 | 8 |
| P2 | 60 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 6 |
| M | 36 |
| S | 26 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | P1 | Weather data ingestion (forecast APIs) | data quality | ingest |
| M1 foundation | M | P2 | Hyper-local per-field forecast | geospatial correctness | identity |
| M1 foundation | S | P2 | Data provenance and freshness | data quality | ingest |
| M2 captured | S | P2 | On-field sensor ingestion | data quality | capture |
| M2 captured | S | P2 | Historical weather per field | data quality | capture |
| M3 explainable | M | P2 | Spray/flight window advisor (feeds `01`/`14`) | agronomic value | evaluator |
| M3 explainable | M | P2 | Frost / heat / wind / precip risk alerts | explainability | evaluator |
| M4 interactive | S | P2 | Alert routing (-> `11`/`13`) | operability | notify |

## Execution Rules

- This domain is sequenced AFTER the core drone platform (domains `01`-`12`) and is gated by the advisor MVP: it keys forecasts on `10` field identity and feeds operational windows back into `01`/`14`.
- The foundational P1 slice is weather ingestion with provenance; every other row is P2 (post-MVP).
- Every weather value must carry source, freshness, and provenance; stale or missing data is flagged, never silently used.
- Every window and alert must be deterministic and cite its inputs, thresholds, and freshness; AI summaries (if any) flag uncertainty.
- Operational windows that gate `01` flight or `14` ground ops must be explainable and auditable before any consumer enforces them.
