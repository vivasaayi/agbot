# Weather Advisory System: Detailed Stories

> Greenfield domain (M0 named): no code exists yet (domain `01` has only a scaffolded weather/airspace hook with no service behind it). Every story below is **built from scratch** and is gated behind the core drone platform (`01`–`12`) and the advisor MVP (`09`). Because advice here gates real field actions, the **data-quality and explainability pillars lead**: every weather value carries source and freshness, and every window and alert cites its inputs. Stories are coarser, M1/M2-weighted, and almost entirely P2 (only weather ingestion is P1).

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `GR` grower, `OPS` operator, `DSP` drone service provider, `TRACTOR-OPS` tractor operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 15-01 · M1 · M · P1 — Weather data ingestion (forecast APIs)
- **Story**: As `PA`, I want to pull a forecast for a field point, normalized and timestamped, so that the platform has a trustworthy weather source.
- **Deterministic / evidence**: a forecast adapter fetches from a provider, normalizes to a common model `{field_ref, valid_time, vars{temp,wind,precip,humidity,radiation}, source, fetched_at}`; every value carries source and fetch time.
- **Acceptance**:
  - Given a field point, when a forecast is pulled, then values are normalized to the common model with source and timestamp on each.
  - Given the provider is unreachable, when a pull is attempted, then the ingest records a fetch failure with reason (no partial/silent insert).
- **Tests**: unit (normalization), API contract (pull), failure path (provider unreachable).
- **Depends on**: `10` (field identity).

### STORY 15-02 · M1 · M · P2 — Hyper-local per-field forecast
- **Story**: As `GR`, I want a forecast resolved and keyed on my field's `10` boundary, so that the weather I see is for my field, not a distant station.
- **Deterministic / evidence**: resolve a forecast to a field by its `10` boundary/centroid; assert the forecast's location lies in/near the boundary in the correct CRS; key the stored forecast on `field_id`.
- **Acceptance**:
  - Given a field with a boundary, when a forecast is resolved, then it is keyed on the field and its location validates against the boundary CRS/extent.
  - Given a field with no boundary, when resolution is attempted, then it fails with an explicit "no field geometry" error (no defaulting to a wrong location).
- **Tests**: unit (resolution), geospatial (boundary/CRS), failure path (no geometry).
- **Depends on**: 15-01, `10`.

### STORY 15-03 · M1 · S · P2 — Data provenance and freshness
- **Story**: As `AG`, I want every weather value to assert its source and freshness, so that I never act on stale data without knowing it.
- **Deterministic / evidence**: each value carries `{source, fetched_at, valid_time, freshness_state}`; a deterministic freshness rule marks values stale past a configured age; stale values are flagged, never silently used.
- **Acceptance**:
  - Given a fresh value, when read, then it reports `fresh` with its source and age.
  - Given a value older than the freshness threshold, when read, then it reports `stale` and downstream consumers see the stale flag (not a bare number).
- **Tests**: unit (freshness rule), fixture (aged values), failure path (stale flagged downstream).
- **Depends on**: 15-01.

---

## M2 — Captured / Observable

### STORY 15-04 · M2 · S · P2 — On-field sensor ingestion
- **Story**: As `OPS`, I want to ingest one on-field sensor stream with freshness and provenance, so that local measurements complement forecasts.
- **Deterministic / evidence**: ingest sensor samples into the common model with `{sensor_id, field_ref, source=sensor, fetched_at}`; track per-stream freshness and coverage; record gaps.
- **Acceptance**:
  - Given a sensor stream, when samples arrive, then they persist with source/provenance and per-stream freshness.
  - Given a stream that stops reporting, when freshness is checked, then the stream is marked stale and the gap is recorded (not back-filled).
- **Tests**: fixture (sensor stream), unit (freshness/coverage), failure path (stream dropout flagged).
- **Depends on**: 15-01, 15-03, `10`.

### STORY 15-05 · M2 · S · P2 — Historical weather per field
- **Story**: As `AG`, I want a field's weather history stored and queryable, so that I can review trends and support after-action review.
- **Deterministic / evidence**: persist ingested values per field over time; support query by field/date-range; history is append-only and retains source/freshness per record.
- **Acceptance**:
  - Given accumulated weather records, when queried by field and date range, then matching records return with their source/freshness.
  - Given a query for a field with no history, when run, then an explicit empty result returns (not an error).
- **Tests**: API contract (query/pagination), fixture (seeded history), failure path (empty history).
- **Depends on**: 15-01, 15-03.

---

## M3 — Explainable (deterministic windows and alerts)

### STORY 15-06 · M3 · M · P2 — Spray/flight window advisor
- **Story**: As `DSP`, I want a deterministic wind/precip operational window for a field, so that flight (`01`) and tractor ops (`14`) only run in safe conditions.
- **Deterministic / evidence**: compute windows from wind/precip/temperature thresholds over the forecast; each window carries `{start, end, gating_vars, thresholds, freshness}`; no window is emitted on stale/missing inputs.
- **Acceptance**:
  - Given a forecast that meets thresholds, when the advisor runs, then a window is emitted citing the gating variables, thresholds, and input freshness.
  - Given stale or missing forecast inputs, when the advisor runs, then no window is emitted and the gap is reported (consumers cannot enforce an unbacked window).
- **Tests**: unit (threshold/window logic), fixture (window pair), failure path (stale input → no window).
- **Depends on**: 15-02, 15-03.

### STORY 15-07 · M3 · M · P2 — Frost / heat / wind / precip risk alerts
- **Story**: As `GR`, I want a threshold-breach risk alert with its inputs cited, so that I can act on frost, heat, wind, or rain before it harms the crop.
- **Deterministic / evidence**: a deterministic evaluator raises an alert when a variable crosses its risk threshold; each alert carries `{risk_type, value, threshold, valid_time, source, freshness}`.
- **Acceptance**:
  - Given a forecast crossing a risk threshold, when evaluation runs, then an alert is raised citing the value, threshold, and freshness.
  - Given values within all thresholds, when evaluation runs, then no alert is raised (no false alarm).
- **Tests**: unit (threshold evaluator), fixture (frost/heat cases), failure path (within thresholds → no alert).
- **Depends on**: 15-02, 15-03.

### STORY 15-08 · M3 · S · P2 — Growing-degree-day inputs
- **Story**: As `AG`, I want daily growing-degree-days computed per field, so that irrigation (`16`) and drought (`17`) have a crop-development input.
- **Deterministic / evidence**: compute daily GDD from min/max temperature against a base, deterministic and method-cited; output keyed on field and date.
- **Acceptance**:
  - Given a day's temperature range, when GDD runs, then the correct GDD value is produced and cites its method/base.
  - Given a missing temperature record for a day, when GDD runs, then that day is marked "no data" (not computed as zero).
- **Tests**: unit (GDD math), fixture (known day), failure path (missing temperature).
- **Depends on**: 15-02, 15-03.

### STORY 15-09 · M3 · S · P2 — Evapotranspiration inputs
- **Story**: As `AG`, I want reference evapotranspiration computed per field, so that water management (`16`) has an ET driver.
- **Deterministic / evidence**: compute reference ET from temperature/humidity/wind/radiation by a documented method, with inputs and method cited; output keyed on field and date.
- **Acceptance**:
  - Given complete weather inputs, when ET runs, then a reference-ET value is produced citing its method and inputs.
  - Given an input variable missing, when ET runs, then it returns "insufficient inputs" rather than a partial/guessed value.
- **Tests**: unit (ET math), fixture (known case), failure path (missing input variable).
- **Depends on**: 15-02, 15-03.

---

## M4 — Interactive

### STORY 15-10 · M4 · S · P2 — Alert routing
- **Story**: As `GR`, I want one alert type routed to the operator console (`11`) and farmers portal (`13`), so that risk reaches me where I work.
- **Deterministic / evidence**: route an alert to `11`/`13` with its full evidence payload (inputs, thresholds, freshness); routing is audited; field scope respected so an alert only reaches owners of that field.
- **Acceptance**:
  - Given a raised alert for an owned field, when routed, then it is delivered to `11`/`13` with its cited evidence and the delivery is audited.
  - Given a routing target that is unreachable, when delivery is attempted, then the failure is recorded and retried/queued (alert not silently dropped).
- **Tests**: integration (route to `11`/`13`), audit (delivery), failure path (unreachable target queued).
- **Depends on**: 15-07, `11`, `13`.

### STORY 15-11 · M4 · S · P2 — Crop-stage-aware recommendations
- **Story**: As `AG`, I want risk thresholds adjusted by crop stage from `10`, so that an alert reflects how vulnerable the crop is right now.
- **Deterministic / evidence**: select the threshold set by the field's crop stage (from `10`); each adjusted alert states which stage and threshold set were applied; falls back to default thresholds when stage is unknown.
- **Acceptance**:
  - Given a field at a frost-sensitive stage, when evaluation runs, then the stage-specific threshold is applied and named in the alert.
  - Given a field with unknown crop stage, when evaluation runs, then default thresholds are applied and the alert states the fallback (no silent mis-staging).
- **Tests**: unit (stage selection), fixture (sensitive stage), failure path (unknown stage → default + noted).
- **Depends on**: 15-07, `10`.

### STORY 15-12 · M4 · S · P2 — Forecast accuracy / verification
- **Story**: As `PA`, I want a past forecast compared to observed values, so that I can judge how much to trust the source.
- **Deterministic / evidence**: join a stored past forecast to observed sensor/history values for the same field/time; compute deterministic error metrics; retain the comparison record.
- **Acceptance**:
  - Given a past forecast and matching observations, when verification runs, then error metrics are computed and stored against the source.
  - Given a forecast with no matching observations, when verification runs, then it reports "not verifiable" (no metric fabricated from absent data).
- **Tests**: unit (error metrics), fixture (forecast/observation pair), failure path (no observations).
- **Depends on**: 15-04, 15-05.

---

## Coverage note

These 12 stories cover all 12 capabilities in `capability-map.md` (~1 story each). The breakdown is M1/M2-weighted with a strong M3 deterministic core (windows, risk alerts, GDD, ET), reflecting that **data quality and explainability lead every phase** in `release-plan.md`: every value carries source/freshness and every window/alert cites its inputs. Only weather ingestion (15-01) is P1; everything else is P2. There is little M5 work, so none is broken out here. The curated counts in `release-plan.md` (~68 rows) expand several of these (per-provider adapters, per-risk-type alert slices, additional ET/GDD method variants) into sibling stories when implemented.
