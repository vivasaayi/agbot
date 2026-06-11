# Pest, Disease, and Weed Intelligence: Release Plan

## Shipment Strategy

Ship in maturity order with an uncompromising evidence-before-advice discipline. The deterministic crop CV products — stand/plant count and canopy cover fraction — come first (M1 identity, M3 deterministic), because they are countable, inspectable, grower-valued on their own, and must precede any ML claim. The inference platform (model registry, tiled inference) and the CV detectors (pest, disease, weed) follow (M3/M4), each carrying confidence, bounded zones, and evidence-tile citation. Human-in-the-loop verification, findings into `09`, and VRA export to `32` round out interactivity (M4). The model-assisted closed loop (detection → propose targeted re-fly/treatment) is the only M5 item and is bounded, approval-gated, and never auto-dispatching. The explainability-and-trust pillar leads every phase: no detection precedes or replaces the deterministic layer.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 14 |
| M2 captured | 12 |
| M3 explainable | 33 |
| M4 interactive | 22 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 36 |
| P1 | 35 |
| P2 | 16 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 17 |
| M | 46 |
| S | 24 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Model registry and versioning | operability | identity |
| M3 explainable | M | Stand count / plant count (deterministic) | explainability and trust | evaluator |
| M3 explainable | M | Canopy cover fraction (deterministic) | explainability and trust | evaluator |
| M3 explainable | M | Tiled inference pipeline | geospatial correctness | pipeline |
| M3 explainable | M | Disease lesion detection | explainability and trust | evaluator |
| M3 explainable | M | Weed mapping (→ VRA via `32`) | agronomic value | evaluator |
| M4 interactive | M | Human-in-the-loop verification | explainability and trust | interaction |
| M4 interactive | S | Evidence-cited findings into `09` | agronomic value | export |

## Execution Rules

- Evidence before advice is enforced at the gate: the deterministic, countable products (stand count, canopy cover fraction) must run and be inspectable before any pest/disease/weed ML detection is enabled — detections never precede or replace them.
- Every ML detection must carry a confidence score, cite its evidence tile/zone, and be human-verifiable before it drives a field action.
- Geospatial correctness holds end-to-end: tiles, detections, and zones preserve CRS/extent and round-trip; a misplaced zone is a defect, not a cosmetic issue.
- Every model must be versioned and provenance-tracked via `30`; a model that fails evaluation or shows drift is flagged and not silently trusted.
- Every output must tie to a field action: a finding into `09`, a weed map → `32` VRA spot-spray (executed by `14`), or an approval-gated re-fly via `01`.
- The single M5 closed-loop slice stays advisory and approval-gated; it cites its evidence and never auto-dispatches a flight or sprayer.
