# Orthomosaic and Photogrammetry: Release Plan

## Shipment Strategy

Ship in maturity order with a geospatial-correctness-before-everything discipline. Frame ingest and reconstruction identity come first (M1), then frame-set capture with coverage/overlap observability (M2), then the deterministic reconstruction, orthomosaic, DSM/DTM, and QA products (M3) — the bulk of the value, since a wrong overlay is worse than none. Interactive QA review, GCP registration, and tiled handoff to `07` follow (M4). A model-assisted re-fly suggestion (detect coverage/quality gaps → propose a targeted re-fly) is the only M5 item and stays advisory and approval-gated. This is a foundational pipeline domain that unblocks `05` and `06`, so reconstruction is sequenced early once capture is trustworthy.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 13 |
| M3 explainable | 32 |
| M4 interactive | 21 |
| M5 autonomous-assist | 5 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 38 |
| P1 | 33 |
| P2 | 16 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 19 |
| M | 44 |
| S | 24 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Frame ingest with EXIF/GPS/IMU camera pose | geospatial correctness | identity |
| M2 captured | M | GSD and coverage/overlap QA | data quality | capture |
| M3 explainable | M | Feature detection and matching | performance and scale | evaluator |
| M3 explainable | L | Bundle adjustment / SfM (sparse + dense) | geospatial correctness | evaluator |
| M3 explainable | L | Orthorectification and mosaicking | geospatial correctness | evaluator |
| M3 explainable | M | Reprojection-error reporting | explainability | evaluator |
| M4 interactive | M | GCP registration and geolocation accuracy | geospatial correctness | interaction |
| M4 interactive | S | Tiled output handed to `07` | performance and scale | export |

## Execution Rules

- Geospatial correctness leads every phase: no orthomosaic, DSM, or DTM is published whose CRS, extent, and resolution cannot be proven correct and round-tripped.
- Deterministic quality products (reprojection error, overlap %, GSD, coverage fraction, GCP residuals) must run and be inspectable before any downstream index (`05`), 3D (`06`), or AI step consumes the mosaic.
- Reconstruction must hold up over hundreds of frames; large rasters and point clouds are tiled before handoff to `07`.
- Every mosaic must record provenance via `30` (frames, camera model, GCPs, parameters, software version) and be re-derivable.
- When QA fails (low overlap/coverage or high reprojection error), the output ties to a field action: recommend a targeted re-fly rather than publishing a misleading map.
- The single M5 re-fly suggestion stays advisory, cites the QA evidence, and is approval-gated; it never auto-dispatches a flight.
