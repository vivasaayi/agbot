# Imagery and Remote Sensing: Release Plan

## Shipment Strategy

Ship in maturity order. Band ingest and a real index product (M1) come first, then calibrated, QA-masked products with statistics (M2), then georeferencing correctness with GeoTIFF round-trip and the thermal/classification math (M3), then interactive overlays and provenance for the advisor (M4). Anomaly detection and automated index-trend advisories (M5) are gated behind correct georeferencing and the advisor workflow in domains `09`/`10`.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 15 |
| M3 explainable | 22 |
| M4 interactive | 16 |
| M5 autonomous-assist | 10 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 33 |
| P1 | 29 |
| P2 | 17 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 14 |
| M | 41 |
| S | 24 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Band ingest and metadata mapping | data quality | identity |
| M1 foundation | M | Spectral index computation (12 indices) | agronomic value | evaluator |
| M2 captured | M | Radiometric calibration | data quality | capture |
| M2 captured | S | QA masking (cloud/shadow/snow/water/clear) | data quality | capture |
| M3 explainable | L | Georeferencing and CRS/extent assertion | geospatial correctness | evaluator |
| M3 explainable | M | Thermal LST pipeline | agronomic value | evaluator |
| M4 interactive | M | Overlay rendering and colormaps | explainability and trust | overlay |
| M4 interactive | S | Product provenance and scene linkage | geospatial correctness | identity |

## Execution Rules

- Every index/thermal/mask P0 must assert CRS, extent, and resolution and round-trip through GeoTIFF without drift.
- Apply QA masks and radiometric calibration before computing index statistics — a wrong overlay is worse than no overlay.
- Every deterministic product must retain min/max/mean and valid-pixel coverage as inspectable evidence.
- Run the pipeline on captured `04` fixtures before real-hardware inputs.
- Do not start M5 index-trend or anomaly advisories until georeferencing and the `09`/`10` advisor spine are reliable.
- Vegetation-type classification (M5) starts deterministic — spectral/phenological signature matching with per-class confidence and citable evidence; ML may replace the matcher only behind the same gated, uncertainty-flagged contract, and synthetic labeled scenes from `02` are first-class test fixtures.
