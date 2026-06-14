# Pest, Disease, and Weed Intelligence

Computer-vision on imagery — pest, disease, and weed detection — the highest-willingness-to-pay agronomy output, built on top of deterministic, countable crop CV products (stand count, canopy cover) that growers already ask for and that must run first.

## Where We Are

- Not started. This is a greenfield CV/ML domain (M0 named) sourced from the product vision; no detection code exists. A new `crop_intelligence` (a.k.a. `pest_detection`) crate would own it.
- Its inputs are real or in progress: the orthomosaic and indices come from `05`/`22`; field context comes from `10`; findings flow to the advisor `09`; weed maps feed prescription/VRA export (`32`) for spot-spray executed by `14`; provenance via `30`.
- Evidence-before-advice is non-negotiable here. Growers ask for countable, verifiable products (stand count, canopy cover fraction) — those deterministic layers must exist and be inspectable, and ML detections must carry confidence, cite the evidence tile/zone, support human verification, and NEVER precede or replace the deterministic layer.

## Where We Should Be

- Deterministic crop CV products — stand/plant count and canopy cover fraction — run first, are countable and inspectable, and stand on their own as grower-valued outputs.
- A model registry, tiled inference pipeline, and evaluation/drift monitoring make ML detections defensible: every pest, disease, or weed finding carries confidence, cites the evidence tile/zone, and is human-verifiable.
- Findings flow as evidence-cited records into `09`; weed maps become VRA spot-spray prescriptions via `32`; a model-assisted closed loop can propose a targeted re-fly or treatment mission — always approval-gated.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0/P1 slices.

## Build Order

1. Deterministic crop CV first: stand/plant count and canopy cover fraction on a real mosaic from `05`/`22`.
2. Model registry and versioning, and a tiled inference pipeline with CRS/extent preserved per tile.
3. Pest and disease detection with confidence and bounded zones, citing the evidence tile.
4. Weed mapping that exports a VRA spot-spray prescription via `32`.
5. Training-data/label management, model evaluation, and drift monitoring; human-in-the-loop verification.
6. Findings as evidence-cited records into `09`, and a model-assisted closed-loop re-fly/treatment proposal (approval-gated, ties to `01`/`14`).

## Primary Crates

New `crop_intelligence` (a.k.a. `pest_detection`) crate, with `shared` for schemas. Consumes the orthomosaic and indices from `05`/`22`, field context from `10`, emits findings to `09`, prescription inputs to `32` (executed by `14`), and records provenance via `30`.
