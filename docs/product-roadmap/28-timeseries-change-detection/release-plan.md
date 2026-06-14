# Time-Series and Change Detection: Release Plan

## Shipment Strategy

Ship in maturity order with geospatial correctness and explainability leading every phase, because the value of this subsystem collapses if a comparison is made across scenes that are not actually aligned. The generic time-series store and reusable API come first (M1), then series capture and freshness across dates (M2), then the deterministic, co-registration-gated change-detection core — alignment QA, per-pixel delta, threshold masks, zonal trend, baseline/seasonality, and ranked change events (M3). Interactive compare/export and the consumer integrations land in M4. Forecast/gap-fill and the closed-loop re-fly hook (M5) stay uncertainty-flagged and approval-gated, and are never enabled before deterministic change is trustworthy. Reusability is sequenced deliberately: the first consumer (`09` vegetation trend) ports onto the shared engine in M3 to prove the API, and the remaining consumers (`15`/`16`/`17`/`19`/`25`/`27`) follow.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 13 |
| M3 explainable | 38 |
| M4 interactive | 22 |
| M5 autonomous-assist | 8 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 41 |
| P1 | 38 |
| P2 | 18 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 19 |
| M | 50 |
| S | 28 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Generic time-series store (scalar + raster) | performance and scale | identity |
| M1 foundation | M | Reusable time-series API | operability | interaction |
| M3 explainable | L | Temporal alignment / co-registration | geospatial correctness | evaluator |
| M3 explainable | M | Alignment QA guard (refuse uncoregistered) | geospatial correctness | evaluator |
| M3 explainable | M | Raster change detection (delta / mask) | explainability | evaluator |
| M3 explainable | M | Zonal trend analysis (slope / trajectory) | agronomic value | evaluator |
| M3 explainable | M | Change events (detect + rank) | agronomic value | evaluator |
| M4 interactive | S | Export (CSV / GeoTIFF / GeoJSON) | geospatial correctness | export |

## Execution Rules

- No two-date comparison and no change map without a proven co-registration; the alignment QA guard must run and refuse non-co-registerable pairs before any delta is computed.
- Every change output must assert CRS, extent, and resolution, cite its evidence layer (the two source series/scenes plus the alignment proof), and carry a reason code.
- The store and API must be generic across `(entity, metric, time)`; a consumer integration may not fork the engine. The first consumer (`09`) ports onto it in M3 to prove reusability before others follow.
- Deterministic change (delta, masks, zonal trend, baseline, change events) must run and be inspectable before any forecast/gap-fill is enabled; forecast/gap-fill outputs always carry an uncertainty band.
- The closed-loop hook only proposes an approval-gated mission; it never executes a re-fly/treatment without human approval and is gated behind reliable deterministic change with tested refusal paths.
