# Geo Viewer and Visualization: Release Plan

## Shipment Strategy

Ship in maturity order, but this domain leans M3/M4 because the plugin scaffolding already exists. First make layers provably georeferenced and rendered with a CRS/extent readout (M3), then make field context and boundary overlays real (M3/M4), then deliver the interactive annotate -> recommend -> report workflow and compare mode (M4). Saved views and shareable export are the last interactive slices.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 8 |
| M2 captured | 9 |
| M3 explainable | 22 |
| M4 interactive | 22 |
| M5 autonomous-assist | 2 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 30 |
| P1 | 23 |
| P2 | 10 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 11 |
| M | 32 |
| S | 20 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Field/scene catalog and selection | agronomic value | identity |
| M3 explainable | M | Georeferenced layer placement | geospatial correctness | evaluator |
| M3 explainable | S | CRS/extent/resolution readout | geospatial correctness | explainability |
| M3 explainable | M | Field boundary overlay | geospatial correctness | overlay |
| M4 interactive | M | Layer toggle and product switching | agronomic value | overlay |
| M4 interactive | M | Point/polygon annotation workflow | explainability | interaction |
| M4 interactive | M | Recommendation create-from-annotation | agronomic value | interaction |
| M4 interactive | M | Compare mode (season/product) | agronomic value | interaction |

## Execution Rules

- Do not draw any overlay whose CRS, extent, and resolution cannot be asserted against the `07` manifest.
- Every layer view must show field context (which field, owner, season) sourced from `10`.
- Every annotation P0 must write back through `07` with author, severity, timestamp, and an audit ID.
- The surface must always offer a path to a recommendation; no dead-end raster views.
