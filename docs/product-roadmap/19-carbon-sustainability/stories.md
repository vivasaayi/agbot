# Carbon and Sustainability Tracking: Detailed Stories

> Greenfield (M0): no code exists for this domain yet. It is gated behind the core drone platform (`01`–`12`) and the advisor MVP, and depends on the identity spine (`10`), imagery/indices (`05`), LiDAR/canopy (`06`), the GIS hub (`07`), and the advisor/report scaffolding (`09`). Stories are necessarily coarse and weighted to M1/M2 foundation; everything here is "build from scratch." Explainability and geospatial correctness are non-negotiable: every output must carry a defensible, auditable MRV evidence trail.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: the inspectable math and the **defensible/auditable MRV evidence** retained (input layers, method, version, georeference, audit) so a certifier can reproduce the claim.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 19-01 · M1 · S · P1 — Sustainability record identity (via `10`)
- **Story**: As `PA`, I want a carbon/sustainability record owned by a field, season, and operation through the `10` spine, so that every environmental number is attributed and auditable.
- **Deterministic / evidence**: persist `{record_id, field_id, season_id, operation_id, metric_type, method_version, created_at, audit_id}` linked through `10`; method version stamped at creation; record owned by exactly one field/season/operation.
- **Acceptance**:
  - Given a valid `10` field/season/operation, when a sustainability record is created, then it persists with all linkage IDs, a method version, and an audit ID.
  - Given a create request referencing an unknown field/season, when it runs, then it fails with a clear `4xx` and no record is written.
- **Tests**: API contract (create/get/list), unit (linkage validation), failure path (unknown field/season → 4xx).
- **Depends on**: `10` (field/season/operation identity).

### STORY 19-02 · M1 · M · P2 — Deterministic carbon-footprint model
- **Story**: As `AG`, I want a per-operation and per-field carbon footprint computed deterministically from logged operation inputs, so that I have a defensible number, not an AI guess.
- **Deterministic / evidence**: compute footprint from logged inputs (fuel, fertilizer, energy, passes) × published emission factors; persist `{value_co2e, inputs[], factor_set_version, evidence_refs[]}`; every factor and input retained as MRV evidence; any AI summary cites this deterministic result and flags uncertainty.
- **Acceptance**:
  - Given logged operation inputs and a factor set, when the footprint runs, then it returns a CO2e value and retains every input and factor version used.
  - Given the same inputs re-run, when computed again, then the result is identical (reproducible) and hashes equal.
  - Given an operation with missing required inputs, when the footprint is requested, then it returns an explicit "insufficient inputs" result, not a partial/fabricated number.
- **Tests**: unit (footprint math + factor versioning), determinism test (same input → same hash), failure path (missing inputs → explicit gap).
- **Depends on**: 19-01, `10` (operation inputs).

---

## M2 — Captured / Observable

### STORY 19-03 · M2 · M · P2 — Biomass / canopy estimation (consumes `06`/`05`)
- **Story**: As `AG`, I want biomass estimated from `06` canopy height and `05` vegetation indices with asserted georeferencing, so that carbon-stock claims rest on provably correct geometry.
- **Deterministic / evidence**: compute biomass over a real `06` canopy grid and `05` index grid; assert CRS/extent/resolution through `07` and round-trip the result; persist `{biomass_value, area, crs, extent, source_layer_refs[]}` as MRV evidence.
- **Acceptance**:
  - Given a `06` canopy raster and a `05` index raster sharing CRS/extent, when biomass runs, then it returns a georeferenced biomass estimate that round-trips its CRS and extent.
  - Given inputs with mismatched CRS or extent, when biomass runs, then it fails with a geospatial assertion error rather than producing a misaligned estimate.
- **Tests**: unit (biomass math), geospatial round-trip (CRS/extent preserved), failure path (CRS/extent mismatch → assertion error).
- **Depends on**: 19-01, `06` (canopy), `05` (indices), `07` (CRS/extent).

### STORY 19-04 · M2 · S · P2 — Baseline and time-series comparison
- **Story**: As `AG`, I want a metric compared to a stored season baseline across seasons, so that change is defensible rather than a single snapshot.
- **Deterministic / evidence**: persist a baseline per metric/field/season; compute delta and trend deterministically vs the stored baseline; retain both endpoints and the comparison method as MRV evidence.
- **Acceptance**:
  - Given a stored baseline and a current value, when comparison runs, then it returns a delta and trend citing both season endpoints.
  - Given no stored baseline for the metric, when comparison is requested, then it is marked "no baseline" and no delta is fabricated.
- **Tests**: unit (delta/trend math), fixture (two-season pair), failure path (no baseline → marked, not fabricated).
- **Depends on**: 19-01, 19-02 or 19-03 (a metric to compare).

---

## M3 — Explainable

### STORY 19-05 · M3 · M · P2 — MRV evidence trail
- **Story**: As `PA`, I want every sustainability output to record its inputs, method, version, georeference, and audit IDs, so that a third party can verify and reproduce the claim.
- **Deterministic / evidence**: attach an MRV trail `{input_layer_refs[], method, method_version, crs, extent, parameters, audit_id, created_at}` to each output; the trail is immutable and re-derivation from it yields the same result.
- **Acceptance**:
  - Given any carbon/biomass/KPI output, when it is produced, then a complete MRV trail is attached and retrievable.
  - Given an output and its MRV trail, when re-derived from the recorded inputs and method version, then the result matches the original.
  - Given an output missing any required MRV field, when it is finalized, then it is rejected as not certification-ready.
- **Tests**: unit (trail completeness check), determinism test (re-derive matches), failure path (incomplete trail → rejected).
- **Depends on**: 19-02, 19-03 (outputs to wrap).

### STORY 19-06 · M3 · S · P2 — Biodiversity assessment from imagery
- **Story**: As `AG`, I want a habitat/heterogeneity proxy computed from `05` imagery over a field, so that biodiversity claims have an evidence-cited, georeferenced basis.
- **Deterministic / evidence**: compute heterogeneity/cover proxies (e.g. index variance, cover-class fractions) over a `05` product; assert CRS/extent through `07`; persist proxy values with source layer refs as MRV evidence; uncertainty explicit.
- **Acceptance**:
  - Given a `05` imagery product over a field, when the biodiversity proxy runs, then it returns georeferenced heterogeneity/cover metrics citing the source layer.
  - Given a degenerate (uniform or all-nodata) product, when the proxy runs, then it returns an explicit "no signal" result rather than a misleading score.
- **Tests**: unit (heterogeneity/cover math), geospatial round-trip, failure path (degenerate product → "no signal").
- **Depends on**: 19-01, `05` (imagery), `07` (CRS/extent).

### STORY 19-07 · M3 · S · P2 — Soil-carbon proxies
- **Story**: As `AG`, I want a proxy soil-carbon model with explicit uncertainty bounds, so that I can report soil carbon without overstating confidence.
- **Deterministic / evidence**: compute a proxy from available evidence (indices, biomass, logged practices) with an explicit uncertainty band; persist `{proxy_value, uncertainty_band, evidence_refs[], method_version}`; the band is always emitted alongside the value.
- **Acceptance**:
  - Given sufficient evidence, when the proxy runs, then it returns a value with an uncertainty band and cited evidence.
  - Given insufficient evidence, when the proxy is requested, then it is unavailable (never a value without a band).
- **Tests**: unit (proxy + uncertainty math), presentation test (band always present), failure path (insufficient evidence → unavailable).
- **Depends on**: 19-03 (biomass), 19-05 (MRV trail).

### STORY 19-08 · M3 · S · P2 — Sustainability KPI tracking
- **Story**: As `GR`, I want a KPI catalog tracked against a per-field target, so that I can see progress toward sustainability goals.
- **Deterministic / evidence**: persist `{kpi_id, field_id, metric_ref, target, current_value, status(OnTrack|AtRisk|Met)}`; status computed deterministically from current vs target; current value sourced from a real metric record with its evidence.
- **Acceptance**:
  - Given a KPI with a target and a current metric value, when status is computed, then it returns On Track / At Risk / Met deterministically and cites the metric record.
  - Given a KPI whose source metric has no current value, when status is requested, then it is marked "no data" rather than defaulting to Met.
- **Tests**: unit (status thresholds), API contract (catalog/track), failure path (no source value → "no data").
- **Depends on**: 19-02 or 19-03 (a metric), 19-01.

---

## M4 — Interactive

### STORY 19-09 · M4 · M · P2 — Certification evidence packs (via `09`)
- **Story**: As `PA`, I want to export a verifiable evidence pack for one certification claim through `09`, so that a certifier can audit and reproduce the claim end to end.
- **Deterministic / evidence**: assemble a pack from one claim's metric, its MRV trail, source layer refs, georeference, and method versions; the encoder asserts the MRV trail is complete before rendering; reuses `09` report scaffolding.
- **Acceptance**:
  - Given a claim whose outputs all carry complete MRV trails, when a pack is generated, then it produces a verifiable bundle containing values, methods/versions, georeference, and evidence layer refs.
  - Given a claim with any output missing its MRV trail, when a pack is requested, then generation fails with a clear error rather than exporting an unverifiable pack.
- **Tests**: golden-file (pack structure), unit (completeness gate), failure path (incomplete MRV → generation refused).
- **Depends on**: 19-05 (MRV trail), `09` (report scaffolding).

### STORY 19-10 · M4 · S · P2 — Export and reporting
- **Story**: As `GR`, I want a field sustainability summary exported as GeoJSON/CSV/PDF, so that I can share carbon, biomass, and KPI results outside the platform.
- **Deterministic / evidence**: assemble field-level summary from the deterministic records; GeoJSON outputs carry correct CRS and properties; CSV rows match the metric records; PDF cites method versions and evidence.
- **Acceptance**:
  - Given a field with sustainability records, when exported, then GeoJSON round-trips its CRS/extent, CSV matches the records, and the PDF cites methods and evidence.
  - Given a field with no records, when exported, then a valid empty export is produced (not an error or a blank certified-looking document).
- **Tests**: geospatial round-trip, schema validation (CSV/GeoJSON), failure path (empty field → valid empty export).
- **Depends on**: 19-02, 19-03, 19-08, `07` (CRS/extent).

---

## Coverage note

This file covers all 10 capabilities in `capability-map.md` with ~10 greenfield stories (≈1 per capability), weighted to M1/M2/M3 with only one M4 pair, matching the M1/M2-heavy, mostly-P2 shape of `release-plan.md` (only the record-identity slice, 19-01, is P1; no P0; no M5 stories authored since release-plan lists just 2 M5 rows). The curated counts in `release-plan.md` (≈60 rows) expand several of these into sibling slices when implemented (e.g. per-input emission-factor sets, per-index biodiversity proxies, multi-season time series, additional KPI catalogs, per-scheme certification packs). The explainability and geospatial-correctness pillars lead throughout: every output retains a defensible, auditable MRV evidence trail and asserts/round-trips its georeference, because a wrong number or geometry invalidates a certification claim.
