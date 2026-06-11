# Pest, Disease, and Weed Intelligence: Detailed Stories

> Greenfield CV/ML domain (M0 named): no detection or crop-CV code exists yet. Every story below is **built from scratch** in a new `crop_intelligence` crate, running on the orthomosaic and indices from `05`/`22`. The **explainability-and-trust pillar dominates every phase**, and the rule is absolute: the **deterministic, countable products (stand count, canopy cover) run first**, are inspectable on their own, and **no ML detection ever precedes or replaces them**. Every ML detection carries a confidence score, cites its evidence tile/zone, and is human-verifiable before it drives a field action. The single M5 closed-loop story (detection → propose targeted re-fly/treatment) is bounded, approval-gated, and never auto-dispatches.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 23-01 · M1 · S · P0 — Model registry and versioning
- **Story**: As `PA`, I want every model registered with a version, metadata, and provenance, so that any detection can be traced to the exact model that produced it.
- **Deterministic / evidence**: persist `{model_id, version, task, training_set_ref, metrics, created_at}` via `30`; a detection run records the model version it used; an unregistered/unversioned model cannot run inference.
- **Acceptance**:
  - Given a trained model, when it is registered, then a versioned record with metadata and provenance is created and listable.
  - Given an inference request referencing an unknown/unregistered model, when submitted, then it is rejected (4xx) and audited (no untraceable detections).
- **Tests**: unit (version record), API contract (register/list), failure path (unregistered model rejected).
- **Depends on**: `30` (provenance), `10` (field/crop context).

### STORY 23-02 · M1 · S · P1 — Inference run identity and lifecycle
- **Story**: As `DSP`, I want each detection/count run to have a stable ID linked to mosaic, field, and season, so that results are traceable and reproducible.
- **Deterministic / evidence**: persist `{run_id, mosaic_ref, field_id, season_id, model_version|"deterministic", status, created_at}`; lifecycle `Queued→Running→Completed|Failed`; a failure records a reason code.
- **Acceptance**:
  - Given a published mosaic from `05`/`22`, when a run is submitted, then a run record is created with linkage and `Queued` status.
  - Given a run referencing an unpublished/unproven mosaic, when submitted, then it is rejected (no detection on an ungated mosaic, see 22-16).
- **Tests**: unit (state transitions), API contract (submit/status/result), failure path (unpublished mosaic rejected).
- **Depends on**: 23-01, `22` (published mosaic gate), `10`.

---

## M2 — Captured / Observable

### STORY 23-03 · M2 · M · P1 — Tiled inference pipeline (CRS/extent preserved)
- **Story**: As `DSP`, I want the mosaic tiled and each tile processed with its CRS/extent preserved, so that detection holds up over a large field and every result is georeferenced.
- **Deterministic / evidence**: tile the mosaic into a grid with per-tile `{tile_id, crs, extent, resolution}`; run the configured evaluator/model per tile; reassemble results into the field CRS; per-tile geometry round-trips.
- **Acceptance**:
  - Given a large mosaic, when the pipeline runs, then every tile carries CRS/extent, results reassemble into the field CRS, and footprints round-trip.
  - Given a tile with corrupt/missing georeferencing, when processed, then it is skipped and flagged (not placed at a default origin).
- **Tests**: unit (tiling + reassembly), geospatial (per-tile round-trip), failure path (bad-georeference tile flagged).
- **Depends on**: 23-02, `22`.

### STORY 23-04 · M2 · S · P2 — Run progress and coverage observability
- **Story**: As `OPS`, I want live run progress with tiles-processed and coverage, so that a long inference run is observable rather than opaque.
- **Deterministic / evidence**: emit `{tiles_total, tiles_done, coverage_fraction, stage}` with timestamps; record stalls as events.
- **Acceptance**:
  - Given a running pipeline, when tiles complete, then progress and coverage stream with timestamps.
  - Given a stalled run, when no progress arrives within a window, then a stall event is recorded and flagged.
- **Tests**: fixture (staged progress), unit (stall detection), failure path (stall flagged).
- **Depends on**: 23-03.

---

## M3 — Explainable (deterministic crop CV first, then defensible detection)

### STORY 23-05 · M3 · M · P0 — Stand count / plant count (deterministic)
- **Story**: As `GR`, I want plants detected and counted on the mosaic, so that I get a countable, verifiable stand count I can act on — independent of any ML health claim.
- **Deterministic / evidence**: deterministic detection + count of plants per tile, reassembled per field/zone with a retained count, density, and per-tile breakdown; the count is inspectable (locations returned), not a black-box number.
- **Acceptance**:
  - Given a mosaic of an emerged crop, when counting runs, then a per-field and per-zone plant count with density is returned, georeferenced, with inspectable plant locations.
  - Given a tile with no valid crop pixels (bare/cloud), when counting runs, then it contributes zero with a reason code (not a fabricated count).
- **Tests**: unit (count + density), fixture (mosaic with known plant count), failure path (empty/invalid tile → zero with reason).
- **Depends on**: 23-03, `22`.

### STORY 23-06 · M3 · M · P0 — Canopy cover fraction (deterministic)
- **Story**: As `AG`, I want canopy cover fraction computed per field/zone with a QA mask, so that I have a defensible cover number, not an impression.
- **Deterministic / evidence**: classify vegetation vs background deterministically (index threshold / cover rule), compute cover fraction per zone, and retain the binary mask and the nodata/QA mask; assert CRS/extent.
- **Acceptance**:
  - Given a mosaic with an index, when cover runs, then a cover fraction per zone is returned with the underlying mask, georeferenced and round-tripping.
  - Given a region under cloud/nodata, when cover runs, then those cells are excluded and the cover fraction reflects only valid area (not counted as 0% silently).
- **Tests**: unit (cover fraction + mask), geospatial (CRS round-trip), failure path (nodata excluded, not counted).
- **Depends on**: 23-03, `05` (index), `22`.

### STORY 23-07 · M3 · S · P1 — Growth-stage / phenology estimation
- **Story**: As `AG`, I want a growth-stage estimate grounded in index and cover evidence, so that scouting and treatment timing reflect crop development.
- **Deterministic / evidence**: estimate stage from already-computed index trajectory and canopy cover (deterministic rules / thresholds), citing the evidence; flag low confidence when evidence is thin (single date, low cover).
- **Acceptance**:
  - Given a crop with index + cover evidence, when estimation runs, then a growth stage is returned with the evidence it used and a confidence band.
  - Given only a single date with no baseline, when estimation runs, then it returns a low-confidence/"insufficient evidence" result (never an over-confident stage).
- **Tests**: unit (stage rules + confidence), fixture (multi-date pair), failure path (single-date → low confidence).
- **Depends on**: 23-05, 23-06, `05`.

### STORY 23-08 · M3 · M · P0 — Disease lesion detection (confidence + bounded zones)
- **Story**: As `AG`, I want disease lesions detected with a confidence score and a bounded zone, so that I can scout the right spots without trusting a black box.
- **Deterministic / evidence**: the registered model runs per tile (only after deterministic cover exists for the field); each detection carries `{confidence, evidence_tile_ref, zone_geometry}`; low-confidence detections are marked, not hidden; the deterministic layer (23-06) is a prerequisite of the run.
- **Acceptance**:
  - Given a mosaic with deterministic cover already computed, when detection runs, then lesions are returned with confidence, evidence-tile citation, and bounded zones in the correct CRS.
  - Given a request to run detection on a field with no deterministic crop-CV products yet, when submitted, then it is refused (evidence-before-advice gate: detection never precedes the deterministic layer).
- **Tests**: unit (confidence + zone geometry), fixture (labeled lesion tiles), geospatial (zone round-trip), failure path (no-deterministic-layer run refused).
- **Depends on**: 23-03, 23-06, 23-01.

### STORY 23-09 · M3 · M · P1 — Pest detection (confidence + bounded zones)
- **Story**: As `AG`, I want pest presence detected with confidence and bounded zones, so that I can target scouting and treatment.
- **Deterministic / evidence**: same contract as 23-08 — per-tile inference gated behind the deterministic layer, each detection carrying confidence, evidence-tile citation, and a bounded zone; confidence threshold is configurable and recorded.
- **Acceptance**:
  - Given a deterministic-layer-present field, when pest detection runs, then detections carry confidence, evidence citation, and bounded zones.
  - Given an all-clear field, when detection runs, then zero detections are returned (no false zones), and any below-threshold detections are excluded with the threshold recorded.
- **Tests**: unit (threshold + confidence), fixture (labeled pest tiles), failure path (clear field → no false detections).
- **Depends on**: 23-03, 23-06, 23-01.

### STORY 23-10 · M3 · M · P0 — Weed mapping
- **Story**: As `AG`, I want weeds mapped into georeferenced zones with confidence, so that the map can drive a spot-spray prescription instead of a blanket application.
- **Deterministic / evidence**: classify weed presence per tile and aggregate into georeferenced weed zones with confidence and area; the deterministic cover layer is a prerequisite; zones round-trip in the field CRS.
- **Acceptance**:
  - Given a deterministic-layer-present field, when weed mapping runs, then weed zones with confidence and area are returned in the correct CRS.
  - Given a weed-free field, when mapping runs, then no zones are produced (no fabricated weed pressure).
- **Tests**: unit (zone aggregation + area), geospatial (zone round-trip), failure path (weed-free field → no zones).
- **Depends on**: 23-03, 23-06, 23-01.

### STORY 23-11 · M3 · M · P1 — Training-data and label management
- **Story**: As `DSP`, I want labeled tiles managed with provenance and train/val/test splits, so that models are trained and evaluated on traceable, non-leaking data.
- **Deterministic / evidence**: persist labeled tiles `{tile_ref, label, labeler, source_run, split}` via `30`; enforce no overlap between train/val/test for the same field/date; splits are reproducible.
- **Acceptance**:
  - Given labeled tiles, when a dataset is built, then train/val/test splits are produced with provenance and no field/date leakage across splits.
  - Given a tile assigned to two conflicting splits, when the dataset is built, then it is rejected as a leakage error (not silently duplicated).
- **Tests**: unit (split + leakage check), fixture (labeled set), failure path (leakage rejected).
- **Depends on**: 23-01, `30`.

### STORY 23-12 · M3 · M · P1 — Model evaluation and drift monitoring
- **Story**: As `PA`, I want each model version evaluated on a held-out set and monitored for drift, so that a degrading model is caught before it misleads an agronomist.
- **Deterministic / evidence**: compute precision/recall/IoU (or task-appropriate metrics) on the val/test split; track input/score distribution drift over runs; flag a model that falls below a metric floor or drifts beyond a bound.
- **Acceptance**:
  - Given a model version and a held-out set, when evaluation runs, then metrics are recorded against the model version and provenance.
  - Given a model that falls below the metric floor or shows drift beyond the bound, when evaluated/monitored, then it is flagged and blocked from being marked production-ready (not silently used).
- **Tests**: unit (metric + drift math), fixture (held-out set), failure path (sub-floor/drift model flagged + blocked).
- **Depends on**: 23-11, 23-01.

---

## M4 — Interactive (verification, findings, and prescriptions)

### STORY 23-13 · M4 · M · P0 — Human-in-the-loop verification and correction
- **Story**: As `AG`, I want to verify, confirm, or correct each detection with an audit trail, so that only human-checked findings drive a field action.
- **Deterministic / evidence**: each detection gets a verification state `{unverified→confirmed|rejected|corrected}` with actor, timestamp, and optional corrected geometry/label; corrections feed back as labels (23-11); no unverified detection can be promoted to a `09` finding by default.
- **Acceptance**:
  - Given a set of detections, when an agronomist verifies them, then each carries a verification state with actor/timestamp, and corrections are captured as new labels.
  - Given an unverified detection, when promotion to a `09` finding is attempted, then it is blocked unless explicitly allowed (human-in-the-loop enforced).
- **Tests**: unit (state transitions), API contract (verify/correct), failure path (unverified → finding blocked).
- **Depends on**: 23-08, 23-09, 23-10.

### STORY 23-14 · M4 · S · P0 — Evidence-cited findings into `09`
- **Story**: As `AG`, I want verified detections emitted as evidence-cited findings into `09`, so that they flow into the advisor's recommendation and report workflow.
- **Deterministic / evidence**: emit findings `{type, zone_geometry, confidence, evidence_tile_refs[], model_version, verification_state}` into the `09` model; each finding cites its evidence and carries its confidence/uncertainty.
- **Acceptance**:
  - Given verified detections, when findings are emitted, then `09` receives records citing evidence tiles, model version, and confidence, tied to the field zone.
  - Given a finding without an evidence reference, when emitted, then it is rejected (no un-cited findings reach the advisor).
- **Tests**: API contract (emit into `09`), unit (finding assembly), failure path (un-cited finding rejected).
- **Depends on**: 23-13, `09`.

### STORY 23-15 · M4 · M · P1 — Weed map → VRA spot-spray prescription (via `32`)
- **Story**: As `AG`, I want a verified weed map exported as a VRA spot-spray prescription, so that only weedy zones are treated instead of the whole field.
- **Deterministic / evidence**: convert verified weed zones into a per-zone rate prescription and export via `32` in a standard VRA format; assert CRS/extent so the prescription aligns with the field; the prescription is executable by `14`.
- **Acceptance**:
  - Given a verified weed map, when exported, then a VRA prescription with per-zone rates is produced via `32` in the correct CRS, ready for `14`.
  - Given weed zones whose CRS/extent do not align with the field, when export is requested, then it is refused with a mismatch error (no misaligned spray prescription).
- **Tests**: unit (zone→rate mapping), geospatial (CRS alignment), failure path (CRS/extent mismatch refused).
- **Depends on**: 23-10, 23-13, `32`.

---

## M5 — Autonomous-Assist (advisory, approval-gated closed loop)

### STORY 23-16 · M5 · M · P2 — Model-assisted closed loop (re-fly / treatment proposal)
- **Story**: As `AG`, I want verified detections to propose a bounded, targeted re-fly or treatment mission, so that I can act on a problem fast — without the system flying or spraying on its own.
- **Safety / deterministic**: only human-verified, evidence-cited findings (23-13/23-14) drive the proposal; it produces a bounded re-fly area for `01` and/or a spot-spray prescription for `14`, citing the exact zones and confidence; it is advisory and **approval-gated** at every step and never auto-dispatches a flight or sprayer.
- **Acceptance**:
  - Given verified disease/pest/weed findings, when a closed-loop action is proposed, then a bounded re-fly (`01`) or treatment (`14`) proposal is produced citing the zones/confidence, pending operator approval.
  - Given unverified or low-confidence detections, when a proposal is requested, then it is refused; and no proposal ever dispatches a mission or spray without recorded approval.
- **Tests**: unit (proposal from verified evidence), integration (`01`/`14` hand-off), failure path (unverified → no proposal; un-approved proposal never dispatches).
- **Depends on**: 23-13, 23-14, 23-15, `01`, `14`.

---

## Coverage note

These 16 stories cover all 12 capabilities in `capability-map.md`. The breakdown is M3-heavy by design: the deterministic crop-CV products (stand count, canopy cover) and the defensible detectors are the trust foundation, and **evidence before advice leads every phase** per `release-plan.md` — detection never precedes or replaces the deterministic, countable layer. Every ML detection carries confidence, cites its evidence tile/zone, and is human-verifiable. The single M5 closed-loop story stays advisory and approval-gated. The curated counts in `release-plan.md` (≈87 rows) expand several of these — per-crop count/cover variants, additional detector classes, dataset/eval/drift slices, and per-format VRA export — into sibling stories when implemented.
