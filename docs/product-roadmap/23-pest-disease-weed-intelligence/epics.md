# Pest, Disease, and Weed Intelligence: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: run submit/status/result routes or commands with pagination and audit IDs; detection and count runs linked to scene/field/season via `05`/`22`/`10`.
- Deterministic: stand/plant count, canopy cover fraction, and tile/zone geometry computed without AI, with reason codes and raw evidence (counts, masks, per-tile stats) retained — and run before any ML output.
- Explainability: every ML detection carries a confidence score and cites the evidence tile/zone; a drifting or unevaluated model is flagged; nothing is presented as fact without verification.
- Geospatial: tiles, detections, and zones preserve CRS/extent; georeferenced footprints round-trip; a misplaced zone is a defect.
- Agronomic: outputs tie to a field action — finding into `09`, weed map → `32` VRA spot-spray executed by `14`, or an approval-gated re-fly via `01`.
- Human-in-the-loop: detections are verifiable/correctable with audit before they drive action.
- Tests: unit (count/cover math, confidence/threshold), fixture (labeled tiles, sample mosaic), API contract, geospatial round-trip, and one failure path (degenerate tile / low-confidence / drift).
- Operations: model registry, run health, retry/backoff, provenance via `30`, and a runbook.

## Category Epics

### EPIC-01: Deterministic Crop CV (the trust foundation)
- Goal: countable, inspectable products that stand on their own and precede every ML claim.
- First release: stand/plant count on a real mosaic from `05`/`22`, with retained counts and a QA mask.
- Expansion: canopy cover fraction and growth-stage / phenology estimation grounded in index + cover evidence.
- Hardening: reproducibility, large-mosaic performance, and a degenerate-tile failure path.

### EPIC-02: Inference Platform and Detection
- Goal: defensible CV detection of pests, disease lesions, and weeds with confidence and bounded zones.
- First release: a model registry/versioning and a tiled inference pipeline that preserves CRS/extent per tile.
- Expansion: pest detection, disease lesion detection, and weed mapping — each with confidence, bounded zones, and evidence-tile citation.
- Hardening: training-data/label management, model evaluation, and drift monitoring with negative-path tests.

### EPIC-03: Verification, Findings, and Closed Loop
- Goal: human-verified detections that drive a real field action.
- First release: human-in-the-loop verification/correction with audit, and evidence-cited findings emitted into `09`.
- Expansion: weed maps exported as `32` VRA spot-spray prescriptions (executed by `14`).
- Hardening: a model-assisted, approval-gated closed loop (detection → propose targeted re-fly via `01` / treatment via `14`) that cites evidence and never auto-dispatches.
