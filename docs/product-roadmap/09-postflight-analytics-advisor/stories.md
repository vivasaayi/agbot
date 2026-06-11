# Post-Flight Analytics and Advisor: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator.

---

## M1 — Foundation

### STORY 09-01 · M1 · S · P0 — Analysis job identity
- **Story**: As `DSP`, I want each analysis job to have a stable ID linked to scene, field, and season, so that results are traceable and reproducible.
- **Deterministic / evidence**: persist `{job_id, scene_id, field_id, season_id, product_refs[], created_at, status}`; status lifecycle `Queued→Processing→Completed|Failed`.
- **Acceptance**:
  - Given a scene linked to a field, when a job is submitted, then a job row is created with all linkage IDs and `Queued` status.
  - Given a job that errors, when it fails, then status is `Failed` with a reason code and the error is retrievable.
- **Tests**: unit (state transitions), API contract (submit/status/result), failure path (submit with unknown scene_id → 4xx).
- **Depends on**: `10` (scene/field/season model), existing `post_processor` job queue.

### STORY 09-02 · M1 · S · P1 — Job result retention and listing
- **Story**: As `AG`, I want to list and re-open past analysis results for a field, so that I can compare flights over time.
- **Acceptance**: results are paginated, filterable by field/season/date; a completed job's result is retrievable by ID after restart.
- **Tests**: API contract (pagination + filters), fixture (seeded results).
- **Depends on**: 09-01.

---

## M3 — Explainable (deterministic products — the trust foundation)

### STORY 09-03 · M3 · M · P0 — Deterministic zonal statistics
- **Story**: As `AG`, I want min/max/mean/std/percentiles and coverage computed per georeferenced product, so that I have defensible numbers, not just a colored picture.
- **Deterministic / evidence**: compute stats over a real `05`/`06` product grid; retain raw counts and nodata mask; assert CRS/extent/resolution on input.
- **Acceptance**:
  - Given a valid NDVI GeoTIFF, when stats run, then mean/std/percentiles/coverage are returned and round-trip with the product's CRS and extent.
  - Given a raster with a nodata mask, when stats run, then masked cells are excluded and coverage reflects valid fraction.
  - Given a degenerate (all-nodata) raster, when stats run, then the job returns an explicit "no valid data" result, not NaN.
- **Tests**: unit (stats math incl. percentiles + nodata), fixture (sample grids), failure path (all-nodata).
- **Depends on**: `05`/`06` georeferenced products, 09-01.

### STORY 09-04 · M3 · M · P0 — Threshold/outlier anomaly flagging
- **Story**: As `AG`, I want low/high anomalous cells flagged with the threshold and reason, so that I can find problem areas without scanning the whole field.
- **Deterministic / evidence**: flag by absolute threshold and by statistical outlier (e.g. mean ± k·std or percentile band); each flag carries `{reason_code, threshold, value}`.
- **Acceptance**:
  - Given a product and a chosen method, when flagging runs, then flagged cells carry a reason code and the threshold used.
  - Given a uniform raster, when flagging runs, then zero anomalies are returned (no false positives).
- **Tests**: unit (threshold + outlier logic), fixture (synthetic stress patch), failure path (uniform raster → 0 flags).
- **Depends on**: 09-03.

### STORY 09-05 · M3 · M · P0 — Zone delineation with area
- **Story**: As `AG`, I want contiguous anomalous cells grouped into zones with polygon geometry and area, so that I can act on areas, not pixels.
- **Deterministic / evidence**: connected-component grouping → polygons in the product CRS; compute area (e.g. hectares) and centroid per zone.
- **Acceptance**:
  - Given flagged cells, when delineation runs, then zones are emitted as GeoJSON polygons with area and centroid in correct CRS.
  - Given two separate patches, when delineation runs, then exactly two zones are produced.
- **Tests**: unit (connected components + area), geospatial round-trip (GeoJSON reprojects correctly), failure path (single-cell zone handling).
- **Depends on**: 09-04.

### STORY 09-06 · M3 · M · P1 — NDVI / vegetation analysis summary
- **Story**: As `AG`, I want a vegetation summary (index stats, low-vigor fraction, trend vs last flight) per field, so that I can judge crop vigor at a glance.
- **Deterministic / evidence**: aggregate 09-03 stats for the vegetation index; compute low-vigor fraction; if a prior scene exists, compute delta.
- **Acceptance**: summary cites the source product and date; trend only shown when a comparable prior scene exists, else marked "no baseline."
- **Tests**: unit (low-vigor fraction, delta), fixture (two-date pair), failure path (no baseline).
- **Depends on**: 09-03, `05`.

### STORY 09-07 · M3 · S · P1 — Thermal hotspot detection
- **Story**: As `AG`, I want thermal hotspots/coldspots flagged with area and confidence, so that I can spot irrigation or stress issues.
- **Deterministic / evidence**: reuse 09-04/09-05 over a thermal product; confidence from margin above threshold.
- **Acceptance**: hotspots carry area, mean temperature, and confidence; absent a thermal product the capability is cleanly unavailable, not faked.
- **Tests**: unit (hotspot logic), failure path (no thermal band).
- **Depends on**: 09-04, 09-05, `05` thermal.

### STORY 09-08 · M3 · S · P1 — Evidence retention and reproducibility
- **Story**: As `DSP`, I want every finding to persist its raw evidence and reason codes, so that a result can be defended and re-derived.
- **Acceptance**: re-running a job on the same inputs yields identical stats/flags; each finding stores the evidence layer ref, method, and parameters.
- **Tests**: determinism test (same input → same output hash), fixture.
- **Depends on**: 09-03..09-05.

---

## M4 — Interactive (the report-delivered outcome)

### STORY 09-09 · M4 · M · P0 — Recommendation from a zone
- **Story**: As `AG`, I want to turn an anomalous zone into a priority-ranked, action-categorized recommendation, so that the grower gets a clear next step.
- **Deterministic / evidence**: recommendation persists into the `10` model with `{zone_ref, priority, action_category, status=open, evidence_refs[]}`.
- **Acceptance**:
  - Given a delineated zone, when a recommendation is created, then it is stored with priority, category, status, and links back to evidence and field.
  - Given a recommendation, when its status changes, then open/reviewed/completed/dismissed transitions are audited.
- **Tests**: API contract (create/transition), unit (priority ranking), failure path (recommendation without a zone → rejected).
- **Depends on**: 09-05, `10` (Recommendation entity).

### STORY 09-10 · M4 · S · P1 — Link recommendations to viewer annotations
- **Story**: As `AG`, I want recommendations to attach to annotations I draw in the viewer, so that the report reflects what I marked.
- **Acceptance**: a recommendation can reference one or more `08` annotations; the report renders both.
- **Tests**: API contract, integration with `08` annotation IDs.
- **Depends on**: 09-09, `08` annotations.

### STORY 09-11 · M4 · L · P0 — Grower-ready PDF report
- **Story**: As `GR`, I want a clear PDF with field metadata, map views, findings, and recommendations, so that I understand what changed and what to do — without logging in.
- **Deterministic / evidence**: report encoder asserts field/scene metadata and layer source details before rendering; replaces the current unimplemented `report_generator.rs` encoder paths.
- **Acceptance**:
  - Given a completed analysis, when a report is generated, then a PDF is produced containing field metadata, ≥1 map view, the findings table, and recommendations.
  - Given missing field metadata, when generation runs, then it fails with a clear error rather than emitting a blank or partial report.
- **Tests**: unit (section assembly), golden-file (structure), failure path (missing metadata).
- **Depends on**: 09-03..09-09, `10`, `08` (map views).

### STORY 09-12 · M4 · S · P0 — Findings export (CSV + GeoJSON)
- **Story**: As `DSP`, I want findings and zones exported as CSV and GeoJSON, so that clients can use them in other tools.
- **Acceptance**: GeoJSON zones carry correct CRS and properties (reason, area, priority); CSV rows match the findings table; both validate against a schema.
- **Tests**: geospatial round-trip, schema validation, failure path (empty findings → valid empty export).
- **Depends on**: 09-05, 09-09.

### STORY 09-13 · M4 · S · P2 — Shareable report delivery
- **Story**: As `AG`, I want to share a report via a link with bounded visibility, so that a client can view it without system access.
- **Acceptance**: a share artifact/link is generated; access respects report visibility rules; revocation works.
- **Tests**: API contract (share/revoke), authz (out-of-scope viewer denied).
- **Depends on**: 09-11, `10` (visibility/roles).

---

## M5 — Autonomous-Assist (gated, uncertainty-flagged)

### STORY 09-14 · M5 · M · P1 — Evidence-gated crop health index
- **Story**: As `AG`, I want a crop-health index that is explicitly derived from the deterministic products and flags its uncertainty, so that I can use it without over-trusting it.
- **Deterministic / evidence**: health index composed only from already-computed indices/stats; every output carries an uncertainty band and the evidence it used; feature-flagged and approval-gated.
- **Acceptance**:
  - Given trustworthy deterministic products, when health runs, then it returns an index with an uncertainty band and cites its evidence layers.
  - Given the deterministic products are missing/stale, when health is requested, then it is unavailable (never fabricated).
- **Tests**: unit (composition + uncertainty), gating test (disabled until products exist), failure path (stale evidence).
- **Depends on**: 09-03..09-08.

### STORY 09-15 · M5 · M · P2 — Bounded yield estimate
- **Story**: As `GR`, I want a rough yield estimate with a clearly stated confidence range, so that I can plan without mistaking it for a guarantee.
- **Acceptance**: yield output is a range with confidence; UI/report always render the uncertainty; disabled by default.
- **Tests**: unit (range math), presentation test (uncertainty always shown).
- **Depends on**: 09-14.

---

## Coverage note

This file is the **format exemplar** for story breakdowns. ~15 stories cover the 12 capabilities in `capability-map.md`; the curated counts in `release-plan.md` (≈47 rows) expand several of these (e.g. per-index variants, additional report sections) into sibling stories when implemented.
</content>
