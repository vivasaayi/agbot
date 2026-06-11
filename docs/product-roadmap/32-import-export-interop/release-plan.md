# Import/Export and Interop Adapters: Release Plan

## Shipment Strategy

Ship in maturity order, consolidating scattered export first. The deterministic format-validation + CRS-reprojection pipeline and vector round-trip come first (M3) because every later capability depends on a provably correct round-trip — these are P0. Field-boundary import into `10` follows (M3), then the prescription / VRA export that bridges finding to action (M4, the agronomic payoff, consumed by `14`), then platform connectors, bulk, and provenance-on-import (M4/M5). Geospatial correctness leads every phase: a round-trip that loses CRS, or a prescription that misaligns with the field, is treated as worse than none and must be a tested rejection.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 8 |
| M2 captured | 6 |
| M3 explainable | 24 |
| M4 interactive | 24 |
| M5 autonomous-assist | 7 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 26 |
| P1 | 27 |
| P2 | 16 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 11 |
| M | 36 |
| S | 22 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M3 explainable | M | Format-validation + CRS-reprojection pipeline | geospatial correctness | evaluator |
| M3 explainable | M | Vector import/export (Shapefile/KML/GeoJSON/GeoPackage) | geospatial correctness | export |
| M3 explainable | S | Raster import/export (GeoTIFF) | geospatial correctness | export |
| M3 explainable | M | Field-boundary import (-> `10`) | geospatial correctness | import |
| M4 interactive | M | Prescription / VRA export (Shapefile) | agronomic value | export |
| M4 interactive | L | Prescription / VRA export (ISO-XML / ISOBUS TaskData) | agronomic value | export |
| M4 interactive | M | John Deere Operations Center connector | operability | connector |
| M4 interactive | S | Provenance-on-import (via `30`) | explainability | evaluator |

## Execution Rules

- Every import is validated and reprojected by the deterministic pipeline before use; a malformed file or an unsupported/oblique CRS is rejected with a reason code — never imported best-effort.
- Every export preserves CRS and extent; a round-trip test must prove no coordinate drift before the format is considered shippable. A wrong overlay is worse than none.
- A prescription / VRA export must align with the target field's CRS and extent; zones that do not align are refused (no misapplied rates downstream in `14`).
- Machine-executable prescription formats (ISO-XML / ISOBUS TaskData) must validate against the format spec before they leave the platform.
- Platform connectors (John Deere, FieldView, Trimble) are external boundaries: they are mockable, simulation-first, and have failure/retry paths tested.
- Every import records its source, format, and CRS as provenance via `30` so an imported boundary or prescription stays traceable.
