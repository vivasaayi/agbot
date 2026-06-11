# Pest, Disease, and Weed Intelligence: Current State and Target State

## Mission

Turn field imagery into defensible crop-CV outputs: run deterministic, countable products first (stand/plant count, canopy cover fraction), then layer computer-vision detection of pests, disease lesions, and weeds on top — every ML detection carrying confidence, citing its evidence tile/zone, and human-verifiable — and route the results to a real field action: scout, spot-spray (via a `32` VRA prescription executed by `14`), re-fly, or report (into `09`).

## Current Maturity

greenfield pending (M0 named): no detection or crop-CV code exists. This domain is a product-vision module for the highest-willingness-to-pay agronomy output (pest/disease/weed) and the deterministic crop products growers already ask for. Nothing in the repository implements a model registry, tiled inference, plant counting, canopy cover, or detection.

## What Exists Now

- Nothing is built for this domain. There is no `crop_intelligence`/`pest_detection` crate, no model registry, tiled inference pipeline, deterministic counter, or detector.
- Adjacent surfaces it builds on and feeds (real or in progress):
  - Domains `05`/`22` (imagery / orthomosaic): the field mosaic and indices that are this domain's image inputs — detection and counting run on the stitched, georeferenced mosaic, not single frames.
  - Domain `09` (post-flight advisor): the findings/recommendations workflow that consumes evidence-cited detections and turns them into grower-ready output.
  - Domain `10` (field/farm data): field, crop, season, and boundary context that scopes tiles and zones.
  - Domain `32` (import/export interop): the VRA/prescription export path — a weed map becomes a spot-spray prescription.
  - Domain `14` (autonomous tractor): the executor of a spot-spray prescription; domain `01` (mission control): the executor of a model-assisted re-fly.
  - Domain `30` (provenance/audit ledger): the provenance record for model versions, training data, and each detection run.

## Gaps to Close

- No deterministic crop CV: no stand/plant count and no canopy cover fraction — the countable products that must exist and be inspectable before any ML claim.
- No growth-stage / phenology estimation grounded in index and cover evidence.
- No model registry or versioning (model metadata, training set, metrics, evidence).
- No tiled inference pipeline that preserves CRS/extent per tile across a large mosaic.
- No pest detection, disease lesion detection, or weed mapping with confidence and bounded zones.
- No training-data/label management (labeled tiles, provenance, train/val/test splits).
- No model evaluation or drift monitoring on held-out data.
- No human-in-the-loop verification/correction workflow with audit.
- No evidence-cited findings emitted into `09`, and no VRA prescription export to `32`.

## Related Existing Surfaces

- Domains `05`/`22` (imagery / orthomosaic): the georeferenced mosaic and indices this domain runs on.
- Domain `09` (advisor): findings/recommendation workflow that consumes detections.
- Domain `10` (field/farm data): field/crop/season/boundary context.
- Domain `32` (import/export interop): VRA/prescription export path for weed maps.
- Domains `01`/`14` (mission control / tractor): executors of the approval-gated closed loop (re-fly / spot-spray).
- Domain `30` (provenance/audit ledger): provenance for models, training data, and detection runs.
- `docs/reference/product-summary.md`: the source description for pest/disease/weed intelligence and the deterministic crop products.

## Target Operating Model

- Evidence before advice is the dominant discipline: deterministic, countable products (stand count, canopy cover fraction) run first, are inspectable on their own, and ML detections never precede or replace them.
- Every ML detection carries a confidence score, cites the exact evidence tile/zone it came from, and is presented for human-in-the-loop verification/correction before it drives action.
- Geospatial correctness holds end-to-end: tiles, detections, and zones preserve CRS and extent; a detection's georeferenced footprint round-trips, and a misplaced zone is treated as a defect.
- Models are versioned and provenance-tracked via `30`: training data, metrics, and the evaluation/drift state are recorded, and a drifting model is flagged rather than trusted silently.
- Every output ties to a real field action: a finding routes into `09`; a weed map becomes a `32` VRA spot-spray prescription executed by `14`; a coverage/quality gap or fresh detection can propose an approval-gated re-fly via `01`.
- The closed loop (detection → propose targeted re-fly/treatment) is bounded and approval-gated: it suggests, cites its evidence, and never auto-dispatches a flight or sprayer.
