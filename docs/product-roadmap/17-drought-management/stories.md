# Drought Management: Detailed Stories

> Greenfield domain (M0 named): no code exists yet. Every story below is **built from scratch** and is gated behind the core drone platform (`01`–`12`) and the advisor MVP (`09`), which carries the stress evidence and recommendations. **Evidence before advice is non-negotiable**: a deterministic index and risk score must run and be inspectable before any AI forecast is shown, and every AI forecast cites its evidence layer and flags uncertainty. Stories are coarser, M1/M2-weighted, and almost entirely P2 (only the drought-index data model is P1).

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 17-01 · M1 · M · P1 — Drought-index data model (SPI/SPEI-style)
- **Story**: As `PA`, I want to persist a deterministic drought index linked to a field/region with its inputs cited, so that every drought number is identified and traceable.
- **Deterministic / evidence**: persist `{index_id, field_or_region_ref, index_type, value, period, input_refs[], computed_at}`; the index is computed deterministically and traces to its inputs; scoped via `10`.
- **Acceptance**:
  - Given precipitation/water-balance inputs, when an index is computed, then it persists with field/region linkage, period, and input references.
  - Given a request to store an index with no input references, when attempted, then it is rejected (no untraceable index).
- **Tests**: unit (index math + model), API contract (compute/list), failure path (untraceable index rejected).
- **Depends on**: `10` (field/region identity).

### STORY 17-02 · M1 · S · P2 — Vegetation-stress evidence (from `05`)
- **Story**: As `AG`, I want to ingest one stress index from `05` as dated, georeferenced evidence, so that drought has a vegetation signal alongside meteorological indices.
- **Deterministic / evidence**: ingest a `05` stress index; assert CRS/extent; store as dated evidence linked to field/region; labelled as observed evidence with its source scene.
- **Acceptance**:
  - Given a `05` stress layer, when ingested, then it persists as dated, georeferenced evidence citing its source scene, in the correct CRS.
  - Given a layer whose CRS/extent does not match the field/region, when ingested, then it is refused with a mismatch error.
- **Tests**: unit (evidence model), geospatial (CRS/extent), failure path (CRS mismatch refused).
- **Depends on**: 17-01, `05`, `07`.

---

## M2 — Captured / Observable

### STORY 17-03 · M2 · M · P2 — Satellite + weather data fusion
- **Story**: As `AG`, I want Landsat (`07`) and weather (`15`) joined into one dated, georeferenced store, so that drought evidence draws on both signals consistently.
- **Deterministic / evidence**: fuse Landsat-derived and weather inputs into a common store keyed on field/region and time; assert CRS/extent alignment; track freshness and coverage per input; stale input degrades gracefully.
- **Acceptance**:
  - Given Landsat and weather inputs for a field/period, when fusion runs, then a joined, dated, georeferenced record is produced with per-input freshness and coverage.
  - Given a stale or missing input source, when fusion runs, then the record is marked degraded/partial-coverage (never silently completed).
- **Tests**: unit (join + coverage), geospatial (CRS alignment), failure path (stale/missing source degraded).
- **Depends on**: 17-01, 17-02, `07`, `15`.

---

## M3 — Explainable (deterministic baselines and risk)

### STORY 17-04 · M3 · S · P2 — Historical baselines and seasonal trends
- **Story**: As `AG`, I want a per-field baseline and trend computed for one index, so that a current reading is interpretable, not a bare number.
- **Deterministic / evidence**: compute a deterministic baseline and trend per field/index from history; output cites the period and sample count used; insufficient history is flagged.
- **Acceptance**:
  - Given enough historical index records, when baseline/trend runs, then a baseline and trend are produced citing their period and sample count.
  - Given too little history, when run, then it returns "insufficient baseline" rather than a baseline computed from too few samples.
- **Tests**: unit (baseline/trend math), fixture (history series), failure path (insufficient history).
- **Depends on**: 17-01.

### STORY 17-05 · M3 · M · P2 — Per-field/region drought risk scoring
- **Story**: As `AG`, I want a deterministic risk score from the index + stress evidence that I can inspect, so that the drought picture is defensible before any AI is involved.
- **Deterministic / evidence**: compute a risk score per field/region from index value, stress evidence, and baseline; the score carries `{value, band, evidence_refs[], thresholds}` and is fully inspectable; no AI in this path.
- **Acceptance**:
  - Given index, stress, and baseline evidence, when scoring runs, then a risk score and band are produced citing every input used.
  - Given missing required evidence, when scoring runs, then it returns "insufficient evidence" rather than a score (no scoring on partial inputs).
- **Tests**: unit (scoring + banding), fixture (drought case), failure path (insufficient evidence).
- **Depends on**: 17-03, 17-04.

---

## M4 — Interactive (gated AI, warnings, mitigation)

### STORY 17-06 · M4 · M · P2 — AI drought forecast (evidence-gated)
- **Story**: As `AG`, I want a drought forecast that cites its evidence layer and flags uncertainty, only after the deterministic score exists, so that I can use a prediction without over-trusting it.
- **Deterministic / evidence**: the forecast is gated on a valid deterministic risk score (17-05); every forecast output carries its evidence refs and an uncertainty band; feature-flagged and disabled until evidence exists; a wrong call is treated as high-cost.
- **Acceptance**:
  - Given a valid deterministic score and evidence, when a forecast runs, then it returns a prediction citing its evidence layer and flagging uncertainty.
  - Given the deterministic score is missing or stale, when a forecast is requested, then it is unavailable (never fabricated without evidence).
- **Tests**: unit (forecast + uncertainty), gating test (disabled without score), failure path (missing/stale evidence).
- **Depends on**: 17-05.

### STORY 17-07 · M4 · S · P2 — Early-warning and alerting
- **Story**: As `GR`, I want a threshold-crossing alert routed to the portal (`13`) and operator (`11`) with its evidence, so that I get early warning of drought risk.
- **Deterministic / evidence**: fire an alert when the deterministic risk score crosses a threshold; the alert cites its evidence and freshness; routed to `13`/`11` with field/region scope respected and delivery audited.
- **Acceptance**:
  - Given a risk score crossing the warning threshold, when evaluation runs, then an alert is raised citing its evidence and routed to `13`/`11`.
  - Given a score below threshold, when evaluation runs, then no alert fires (no false warning).
- **Tests**: unit (threshold evaluator), integration (route to `13`/`11`), failure path (below threshold → no alert).
- **Depends on**: 17-05, `13`, `11`.

### STORY 17-08 · M4 · M · P2 — Mitigation strategy recommendations
- **Story**: As `AG`, I want a mitigation recommendation (irrigation `16` / advisor `09`) tied to the risk score, so that a drought warning leads to a real field action, not a dead-end number.
- **Deterministic / evidence**: derive a recommendation from the risk score and evidence; persist `{recommendation, action_target(16|09), risk_ref, evidence_refs[], status}`; the recommendation always names the risk and evidence it rests on.
- **Acceptance**:
  - Given a high risk score, when a recommendation is generated, then it names a `16`/`09` action and cites the risk and evidence behind it.
  - Given no qualifying risk, when generation runs, then no recommendation is produced (no advice without evidence).
- **Tests**: unit (recommendation derivation), integration (`16`/`09` linkage), failure path (no risk → no recommendation).
- **Depends on**: 17-05, `16`, `09`.

### STORY 17-09 · M4 · S · P2 — Drought reporting
- **Story**: As `GR`, I want a per-field/region drought report with evidence and trend, so that I understand the drought picture and what to do about it.
- **Deterministic / evidence**: assemble a report from index, baseline/trend, risk score, and (if present) forecast and mitigation; the report asserts its evidence and freshness before rendering; deterministic sections precede any AI section.
- **Acceptance**:
  - Given a scored field/region, when a report is generated, then it contains the index, baseline/trend, risk score, and recommendations, each citing evidence.
  - Given missing required evidence, when generation runs, then it fails with a clear error rather than emitting a blank/placeholder report.
- **Tests**: unit (section assembly), golden-file (structure), failure path (missing evidence).
- **Depends on**: 17-04, 17-05, 17-08.

---

## M5 — Autonomous-Assist (gated)

### STORY 17-10 · M5 · S · P2 — Per-field/region drought history and advisory loop
- **Story**: As `AG`, I want an auditable index/score/alert history that can feed a bounded advisory loop, so that drought response improves over time without losing the evidence trail.
- **Deterministic / evidence**: persist append-only history of `{index, score, alert, recommendation}` per field/region; any advisory loop built on it stays gated behind reliable deterministic scoring and the evidence-before-advice path, and is feature-flagged off by default.
- **Acceptance**:
  - Given accumulated drought records, when history is queried, then index/score/alert/recommendation records return in order with their evidence.
  - Given the deterministic scoring path is not reliable/enabled, when the advisory loop is requested, then it stays disabled (no autonomous advice without the gate).
- **Tests**: API contract (history query), gating test (loop disabled without reliable scoring), failure path (gate enforced).
- **Depends on**: 17-05, 17-08.

---

## Coverage note

These 10 stories cover all 10 capabilities in `capability-map.md` (~1 story each). The breakdown is M1/M2-weighted with an explainable M3 core (baseline/trend, deterministic risk scoring) that must run and be inspectable before any AI forecast — enforcing the **evidence-before-advice gate** that `release-plan.md` calls non-negotiable. Only the drought-index data model (17-01) is P1; everything else is P2. The single M5 story (history + bounded advisory loop) stays gated behind reliable deterministic scoring and is off by default. The curated counts in `release-plan.md` (~65 rows) expand several of these (per-index-type variants, additional fusion-source adapters, more mitigation and reporting slices) into sibling stories when implemented.
