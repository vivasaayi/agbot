# Pest, Disease, and Weed Intelligence: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (explainability and trust first, then agronomic value, data quality, geospatial correctness, performance and scale) and the workstreams in `release-plan.md`. Because this is a greenfield CV/ML domain (M0 named), every capability's current source status is "missing (greenfield)"; the Primary First Slice describes the M1/M3 foundation step. The explainability-and-trust pillar dominates: the **deterministic, countable products run first**, and no ML detection precedes or replaces them — every detection carries confidence, cites its evidence tile/zone, and is human-verifiable. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Pest, Disease, and Weed Intelligence Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Stand count / plant count (deterministic) | missing (greenfield) | 8 | Detect and count plants on a real mosaic, inspectable |
| Canopy cover fraction (deterministic) | missing (greenfield) | 7 | Compute canopy cover fraction with a QA mask |
| Growth-stage / phenology estimation | missing (greenfield) | 6 | Estimate growth stage from index + cover evidence |
| Model registry and versioning | missing (greenfield) | 7 | Register a model version with metadata and evidence |
| Tiled inference pipeline | missing (greenfield) | 8 | Tile a mosaic and run inference, CRS/extent preserved |
| Pest detection | missing (greenfield) | 8 | Detect pests with confidence and bounded zones |
| Disease lesion detection | missing (greenfield) | 8 | Detect lesions with confidence and bounded zones |
| Weed mapping (→ VRA via `32`) | missing (greenfield) | 8 | Map weeds and export a spot-spray prescription |
| Training-data and label management | missing (greenfield) | 7 | Manage labeled tiles with provenance and splits |
| Model evaluation and drift monitoring | missing (greenfield) | 7 | Evaluate a model on a held-out set; watch drift |
| Human-in-the-loop verification | missing (greenfield) | 7 | Verify/correct detections with audit |
| Evidence-cited findings into `09` | missing (greenfield) | 6 | Emit findings citing tile/zone evidence to `09` |
