# Time-Series and Change Detection: Detailed Stories

> Flagship reusable subsystem (thin partial → promote): today only slivers exist — a "compare" capability in `08` and a "trend vs last flight" line in `09`. These stories promote them into a dedicated, general-purpose `timeseries` engine plus a co-registration-gated change-detection layer that many domains plug into (`09`/`15`/`16`/`17`/`19`/`25`/`27`). The dominant rule across every phase: **no two-date comparison and no change map without a proven co-registration** — alignment failure is a clean, tested refusal, never a misaligned result. Reusability is the central theme: the first consumer (`09`) ports onto the shared engine in M3 to prove the API before the rest follow.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 28-01 · M1 · M · P0 — Generic time-series store (scalar + raster)
- **Story**: As `PA`, I want a single store that holds scalar AND raster series keyed by `(entity, metric, time)`, so that every domain records values over time in one place instead of re-inventing storage.
- **Deterministic / evidence**: persist series points `{entity_ref, metric, t, value | raster_ref, source_ref, crs?, extent?, created_at}`; scalar values store inline, raster values store a georeferenced product ref with CRS/extent; entity is generic (field, zone, drone, sensor, scene).
- **Acceptance**:
  - Given an entity and metric, when scalar points are appended, then they persist in time order and are retrievable by `(entity, metric, time-range)`.
  - Given a raster point, when appended, then its product ref, CRS, and extent persist and round-trip.
  - Given a duplicate `(entity, metric, t)`, when appended, then it is rejected or versioned deterministically (never silently double-counted).
- **Tests**: unit (key uniqueness + ordering), fixture (mixed scalar/raster series), failure path (duplicate key).
- **Depends on**: `shared` (CRS/extent types), `07`/`10` (entity identity).

### STORY 28-02 · M1 · M · P0 — Reusable time-series API
- **Story**: As `DSP`, I want an append/query API any domain can call, so that `09`/`15`/`16`/`17`/`19`/`25`/`27` plug into one engine.
- **Deterministic / evidence**: API exposes `append(entity, metric, point)`, `query(entity, metric, range)`, and `list_metrics(entity)`; query is paginated and returns points with provenance refs; the engine is agnostic to which domain calls it.
- **Acceptance**:
  - Given series exist, when queried by entity/metric/range, then paginated points return with source refs.
  - Given a query for an unknown entity/metric, when run, then an empty (not error) result is returned with a clear "no series" marker.
- **Tests**: API contract (append/query/list + pagination), unit (range filtering), failure path (unknown metric → empty marker).
- **Depends on**: 28-01.

### STORY 28-03 · M1 · S · P1 — Series identity, provenance, and metric registry
- **Story**: As `PA`, I want each metric defined once (name, unit, kind, expected cadence) and each point to carry provenance, so that series are comparable and trustworthy.
- **Deterministic / evidence**: a metric registry holds `{metric, unit, kind=scalar|raster, expected_cadence}`; every point references its `source_ref` (scene/product/sensor reading); unknown metrics are rejected at append.
- **Acceptance**:
  - Given a registered metric, when a point with matching unit is appended, then it is accepted with provenance.
  - Given a point whose metric is not registered or whose unit mismatches, when appended, then it is refused with a reason code.
- **Tests**: unit (registry validation), API contract (register/list metric), failure path (unit mismatch refused).
- **Depends on**: 28-01.

---

## M2 — Captured / Observable

### STORY 28-04 · M2 · M · P1 — Multi-date series capture from products
- **Story**: As `DSP`, I want products from `05`/`06` and readings from `27` to flow into the store as they are produced, so that a field accumulates a real history.
- **Deterministic / evidence**: an ingest adapter writes a series point whenever a new scene product or sensor reading is finalized; record freshness and the source date; never backfill a fabricated point.
- **Acceptance**:
  - Given a new NDVI product for a field, when ingest runs, then a raster series point appears with the scene date, CRS, and extent.
  - Given two products for the same `(entity, metric, t)`, when ingest runs, then the conflict is recorded and resolved deterministically (no silent overwrite).
- **Tests**: fixture (sequence of dated products), unit (freshness/date capture), failure path (same-timestamp conflict).
- **Depends on**: 28-01, `05`/`06`, `27`.

### STORY 28-05 · M2 · S · P1 — Series freshness, gaps, and cadence health
- **Story**: As `AG`, I want a series to report its freshness, gaps, and whether it meets its expected cadence, so that I know if a trend is built on sparse or stale data.
- **Deterministic / evidence**: compute last-point age, count gaps vs expected cadence, and flag staleness; expose this on query so consumers can refuse weak comparisons.
- **Acceptance**:
  - Given a series with a missing interval, when health is computed, then the gap is reported (not interpolated away).
  - Given a stale series, when queried, then a staleness flag is returned for the caller to honor.
- **Tests**: unit (gap/cadence detection), fixture (sparse series), failure path (stale flag set).
- **Depends on**: 28-03, 28-04.

---

## M3 — Explainable (the deterministic, co-registration-gated change core)

### STORY 28-06 · M3 · L · P0 — Temporal alignment / co-registration
- **Story**: As `AG`, I want two same-field scenes from different dates resampled onto a common grid, CRS, and resolution, so that they can actually be compared pixel-for-pixel.
- **Deterministic / evidence**: align a two-date raster pair to a common target grid (CRS, extent, resolution); record the transform, resampling method, and the resulting aligned extent; alignment is a first-class evidence object the change step consumes.
- **Acceptance**:
  - Given two scenes of one field with compatible footprints, when alignment runs, then both are resampled to a shared grid and the alignment evidence (CRS/extent/resolution/transform) is recorded.
  - Given a pair whose overlap is below a configured minimum, when alignment runs, then it fails with an `insufficient_overlap` reason and produces no aligned grid.
- **Tests**: unit (resample + transform), geospatial (round-trip CRS/extent), failure path (insufficient overlap).
- **Depends on**: 28-01, `05`/`06`, `shared` (CRS/extent).

### STORY 28-07 · M3 · M · P0 — Alignment QA guard (refuse uncoregistered comparisons)
- **Story**: As `AG`, I want any comparison refused when the two scenes are not co-registerable, so that I never see a change map built on misaligned data.
- **Deterministic / evidence**: before any delta, a deterministic guard asserts CRS match (or proven reprojection), extent overlap ≥ threshold, and resolution compatibility; a failure returns `{reason_code, mismatch_detail}` and blocks the change job; the guard is the single gate every two-date path must pass.
- **Acceptance**:
  - Given a co-registerable pair, when the guard runs, then it passes and emits an alignment-proof reference.
  - Given a pair with incompatible CRS/extent/resolution, when the guard runs, then the change job is refused with a specific mismatch reason and no change map is produced.
- **Tests**: unit (each mismatch class), API contract (refusal shape), failure path (CRS mismatch → no change map).
- **Depends on**: 28-06.

### STORY 28-08 · M3 · M · P0 — Raster change detection: per-pixel delta and threshold mask
- **Story**: As `AG`, I want a per-pixel delta and a threshold change mask between two aligned dates, so that I can see exactly where a metric rose or fell.
- **Deterministic / evidence**: compute `delta = later − earlier` over the aligned grid; build a change mask by absolute and/or std-based threshold; assert the output CRS/extent equals the aligned grid; retain the threshold and method per output.
- **Acceptance**:
  - Given an aligned pair (post-guard), when change runs, then a delta raster and a threshold mask are produced in the aligned CRS/extent, with the threshold recorded.
  - Given an unaligned pair, when change is requested, then it is refused by the guard before any delta is computed.
  - Given two identical scenes, when change runs, then the mask is empty (no spurious change).
- **Tests**: unit (delta + threshold math), geospatial (CRS/extent of output), failure path (unaligned refused; identical → empty mask).
- **Depends on**: 28-07.

### STORY 28-09 · M3 · M · P1 — Normalized change
- **Story**: As `AG`, I want a normalized change (e.g. percent or z-scored delta) in addition to the raw delta, so that change is comparable across fields and metrics.
- **Deterministic / evidence**: normalize the delta by the earlier value or by the metric's variance; carry the normalization method and guard against divide-by-zero/nodata.
- **Acceptance**:
  - Given a delta raster, when normalized, then the normalized output records its method and excludes nodata/zero-denominator cells.
  - Given an all-nodata overlap, when normalization runs, then it returns an explicit "no valid change" result, not NaN.
- **Tests**: unit (normalization + divide-by-zero guard), failure path (all-nodata).
- **Depends on**: 28-08.

### STORY 28-10 · M3 · M · P0 — Zonal trend analysis (trajectory + slope)
- **Story**: As `AG`, I want a metric's trajectory and slope per field/zone over many dates, so that I can see direction of travel, not just two-date change.
- **Deterministic / evidence**: aggregate a series per field/zone into a trajectory; fit a deterministic slope/trend (e.g. least-squares) with a goodness measure; zones come from `09`/`05` and carry CRS; retain the points used.
- **Acceptance**:
  - Given ≥3 dated points for a zone, when trend runs, then a slope, direction, and fit measure are returned with the contributing points cited.
  - Given fewer than the minimum points, when trend is requested, then it returns "insufficient history" rather than a fabricated slope.
- **Tests**: unit (slope + fit), fixture (multi-date zone series), failure path (insufficient points).
- **Depends on**: 28-02, 28-04, `09`/`05` (zones).

### STORY 28-11 · M3 · M · P1 — Rolling baseline and seasonality
- **Story**: As `AG`, I want a rolling baseline and a season-over-season comparison for a metric, so that change is judged against normal, not against an arbitrary prior date.
- **Deterministic / evidence**: compute a rolling baseline window and a season-aligned comparison (same phenological point across years); flag anomaly = deviation beyond a band; retain the baseline window and the seasons used.
- **Acceptance**:
  - Given enough history, when baseline runs, then a rolling baseline and a season-over-season delta are produced with the windows recorded.
  - Given insufficient seasonal history, when season comparison is requested, then it returns "no seasonal baseline" rather than comparing across mismatched phenology.
- **Tests**: unit (rolling window + season alignment), fixture (multi-season series), failure path (no seasonal baseline).
- **Depends on**: 28-10.

### STORY 28-12 · M3 · M · P0 — Change events: detect and rank
- **Story**: As `AG`, I want significant changes detected and ranked as events ("NDVI dropped 0.2 in the NE zone since the last flight"), so that I get the few things that matter, not a raw difference image.
- **Deterministic / evidence**: from the change mask, zonal trend, and baseline, derive change events `{zone_ref, metric, magnitude, direction, since_date, reason_code, evidence_refs[]}`; rank by magnitude/area/severity; every event retains the evidence it was built from.
- **Acceptance**:
  - Given a change mask and zones, when event detection runs, then ranked events are emitted, each citing its evidence (aligned pair, mask, zone) and a human-readable summary.
  - Given no change above threshold, when detection runs, then zero events are returned (no noise events).
- **Tests**: unit (event derivation + ranking), fixture (NE-zone drop pattern), failure path (sub-threshold → 0 events).
- **Depends on**: 28-08, 28-10, 28-11.

### STORY 28-13 · M3 · S · P1 — Evidence retention and reproducibility
- **Story**: As `DSP`, I want every change output to persist its inputs, alignment proof, and parameters, so that a change can be defended and re-derived identically.
- **Deterministic / evidence**: persist `{source_pair, alignment_ref, method, thresholds, params}` per output; re-running on the same inputs yields an identical delta/mask/events hash.
- **Acceptance**:
  - Given a completed change job, when re-run on the same inputs, then the outputs are identical (same hash).
  - Given a missing source product, when re-run, then it fails clearly rather than producing a partial result.
- **Tests**: determinism (same input → same hash), fixture, failure path (missing source).
- **Depends on**: 28-06..28-12.

### STORY 28-14 · M3 · M · P1 — First consumer integration: `09` vegetation trend on the shared engine
- **Story**: As `AG`, I want the advisor's "trend vs last flight" computed by this subsystem, so that one engine powers vegetation trends instead of a bespoke delta in `09`.
- **Deterministic / evidence**: `09`'s vegetation summary calls the time-series API for the index series and the change-event/trend for the field; the "no baseline" case is honored from series freshness (28-05); no separate trend math lives in `09`.
- **Acceptance**:
  - Given a field with a comparable prior scene, when the advisor requests trend, then the shared engine returns the delta and ranked change events with cited evidence.
  - Given no comparable prior scene, when trend is requested, then the engine returns "no baseline" and the advisor renders it as such (never a fabricated trend).
- **Tests**: integration (`09` calls engine), unit (no-baseline path), failure path (no prior scene → no baseline).
- **Depends on**: 28-02, 28-12, `09`.

---

## M4 — Interactive (compare, export, and the reusable consumers)

### STORY 28-15 · M4 · S · P0 — Export: CSV, change-mask GeoTIFF, change-zone GeoJSON
- **Story**: As `DSP`, I want a series as CSV, a change mask as GeoTIFF, and change zones as GeoJSON, so that clients can use them in other tools.
- **Deterministic / evidence**: CSV rows carry `(entity, metric, t, value)`; GeoTIFF carries the aligned CRS/extent/resolution; GeoJSON change zones carry CRS and properties (magnitude, direction, reason); all validate against a schema.
- **Acceptance**:
  - Given a series and a change result, when exported, then CSV/GeoTIFF/GeoJSON are produced and validate, with the GeoTIFF/GeoJSON in the correct CRS.
  - Given an empty change result, when exported, then a valid empty GeoJSON/CSV is produced (not an error).
- **Tests**: geospatial round-trip (GeoTIFF/GeoJSON CRS), schema validation, failure path (empty result → valid empty export).
- **Depends on**: 28-08, 28-12.

### STORY 28-16 · M4 · M · P0 — Compare-view feed to `08`
- **Story**: As `AG`, I want the geo viewer's compare mode to render the engine's aligned pair and change mask, so that side-by-side/swipe shows provably co-registered scenes.
- **Deterministic / evidence**: serve `08` the aligned two-date pair (28-06) plus the change mask (28-08) with the alignment proof; `08` no longer compares raw scenes itself; an uncoregistered pair surfaces the engine's refusal as a mismatch message in `08-12`.
- **Acceptance**:
  - Given two comparable scenes, when compare opens, then `08` renders the aligned pair and change mask locked to a shared georeferenced view.
  - Given an uncoregistered pair, when compare is attempted, then the engine's refusal is shown as a mismatch message and no misaligned panes render.
- **Tests**: integration (`08` consumes feed), unit (refusal passthrough), failure path (uncoregistered → mismatch message).
- **Depends on**: 28-07, 28-08, `08` (compare mode).

### STORY 28-17 · M4 · M · P1 — Scalar consumer integrations (`15`/`16`/`17`/`27`)
- **Story**: As `AG`, I want weather, water-balance, drought-index, and soil/IoT readings stored and trended on this engine, so that those domains reuse one time-series subsystem.
- **Deterministic / evidence**: each domain registers its metrics (28-03) and appends points; trend/baseline/anomaly (28-10/28-11) are computed by the shared engine; no domain forks the engine.
- **Acceptance**:
  - Given weather/water/drought/soil points, when appended and queried, then trend and anomaly come from the shared engine with cited points.
  - Given a domain that registers a metric with an incompatible unit, when it appends, then the point is refused (engine stays consistent).
- **Tests**: integration (one scalar consumer end-to-end), unit (metric isolation), failure path (unit mismatch refused).
- **Depends on**: 28-03, 28-10, 28-11, `15`/`16`/`17`/`27`.

### STORY 28-18 · M4 · M · P1 — Fleet-health and carbon consumers (`25` RUL, `19` carbon stock)
- **Story**: As `OPS`, I want telemetry health/RUL trend and carbon stock over time on the same engine, so that maintenance and sustainability reuse the trend/anomaly logic.
- **Deterministic / evidence**: `25` appends component-health metrics and reads slope/anomaly for a remaining-useful-life trend; `19` appends carbon-stock estimates and reads season-over-season change; both cite the contributing points.
- **Acceptance**:
  - Given health/carbon series, when trended, then slope/anomaly (`25`) and season-over-season change (`19`) return with cited evidence.
  - Given insufficient history, when trend is requested, then "insufficient history" is returned (no fabricated RUL/stock trend).
- **Tests**: integration (`25` + `19` consumers), unit (slope/season reuse), failure path (insufficient history).
- **Depends on**: 28-10, 28-11, `25`, `19`.

---

## M5 — Autonomous-Assist (uncertainty-flagged, approval-gated)

### STORY 28-19 · M5 · M · P2 — Forecast and gap-fill (uncertainty-flagged)
- **Story**: As `AG`, I want a simple trend projection and interpolation for missing points with a clear uncertainty band, so that I can anticipate a trajectory without mistaking it for fact.
- **Deterministic / evidence**: forecast is a deterministic projection of the fitted trend (28-10); gap-fill interpolates between real points; every projected/filled value is flagged as synthetic with an uncertainty band and never written back as a real observation.
- **Acceptance**:
  - Given a trend with enough history, when forecast runs, then projected points carry an uncertainty band and a synthetic flag.
  - Given insufficient history, when forecast is requested, then it is unavailable (never a confident-looking fabricated projection).
- **Tests**: unit (projection + interpolation + uncertainty), presentation (synthetic always flagged), failure path (insufficient history → unavailable).
- **Depends on**: 28-10, 28-11.

### STORY 28-20 · M5 · M · P2 — Closed-loop change hook: auto-propose an approval-gated re-fly/treatment
- **Story**: As `AG`, I want a significant detected change to auto-draft a targeted re-fly or treatment mission for my approval, so that "what changed?" can become a next action without ever auto-executing.
- **Deterministic / evidence**: a ranked change event (28-12) above a configured severity drafts a mission proposal scoped to the changed zone (`09`→`01` re-fly, or `14` treatment), with the change event's evidence attached; the proposal is approval-gated and inert until a human approves; nothing dispatches automatically.
- **Acceptance**:
  - Given a high-severity change event, when the hook runs, then an approval-gated mission proposal is drafted for the changed zone, citing the change evidence, and remains inert until approved.
  - Given approval is withheld or absent, when the proposal exists, then no mission is dispatched (no autonomous execution).
- **Tests**: unit (proposal scoping from event), integration (`09`→`01`/`14` proposal), failure path (no approval → no dispatch).
- **Depends on**: 28-12, `09`, `01`/`14`.

---

## Coverage note

These 20 stories cover all 13 capabilities in `capability-map.md`, with deliberate extra depth on the deterministic, co-registration-gated change core (alignment 28-06, the QA guard 28-07, delta/mask 28-08, normalized change 28-09, zonal trend 28-10, baseline/seasonality 28-11, ranked change events 28-12) because that is where the subsystem's trust lives. Reusability is shown explicitly: `09` ports onto the engine (28-14), and the scalar/fleet/carbon consumers (`15`/`16`/`17`/`19`/`25`/`27`) plug in without forking it (28-17, 28-18). The single hard rule — **no change map without a proven co-registration** — appears as a tested refusal in 28-07, 28-08, and 28-16, matching the execution rules in `release-plan.md`. The two M5 stories (forecast/gap-fill 28-19, closed-loop re-fly 28-20) stay uncertainty-flagged and approval-gated. The curated counts in `release-plan.md` (≈97 rows) expand several of these (per-metric series adapters, additional alignment/resampling methods, per-consumer integration slices, and per-format export variants) into sibling stories when implemented.
