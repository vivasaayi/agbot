# Water Management: Detailed Stories

> Greenfield domain (M0 named): no code exists yet. Every story below is **built from scratch** and is gated behind the core drone platform (`01`–`12`) and the advisor MVP (`09`), which supplies the management zones a water plan rests on. The **agronomic-value and data-quality pillars lead**: deterministic ET and water-need must run and be inspectable before any scheduling recommendation, and every moisture reading carries source, freshness, and a QA flag. Stories are coarser, M1/M2-weighted, and almost entirely P2 (only the soil-moisture data model is P1).

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 16-01 · M1 · M · P1 — Soil-moisture data model
- **Story**: As `PA`, I want to persist a moisture reading linked to a field/zone with source, freshness, and a QA flag, so that all moisture evidence is identified and trustworthy.
- **Deterministic / evidence**: persist `{reading_id, field_id, zone_ref, value, source, captured_at, qa_flag}`; readings are located and dated; scoped to field/zone via `10`/`07`.
- **Acceptance**:
  - Given a sensor reading, when it is ingested, then it persists with field/zone linkage, source, capture time, and a QA flag.
  - Given a reading with no field/zone linkage, when ingest is attempted, then it is rejected (4xx) and audited (no orphan reading).
- **Tests**: unit (model + QA flag), API contract (ingest/list), failure path (unlinked reading rejected).
- **Depends on**: `10` (field/zone identity), `07` (CRS discipline).

### STORY 16-02 · M1 · S · P2 — Remote-sensing moisture proxies (NDWI/NDMI from `05`)
- **Story**: As `AG`, I want to ingest one NDWI/NDMI layer from `05` as a zone moisture proxy, so that moisture evidence exists even without ground sensors.
- **Deterministic / evidence**: ingest a `05` moisture index layer; assert CRS/extent; map per-zone proxy values onto `10`/`07` zones with source and date; flag as proxy (not a direct measurement).
- **Acceptance**:
  - Given an NDWI/NDMI layer for a field, when ingested, then per-zone proxy values are produced in the correct CRS, dated and labelled "proxy".
  - Given a layer whose CRS/extent does not match the field, when ingested, then it is refused with a mismatch error (no misaligned proxy).
- **Tests**: unit (zone mapping), geospatial (CRS/extent), failure path (CRS mismatch refused).
- **Depends on**: 16-01, `05`, `07`.

---

## M2 — Captured / Observable

### STORY 16-03 · M2 · S · P2 — Weather-input contract (from `15`)
- **Story**: As `PA`, I want a defined, validated ET-driver input contract with `15`, so that water calculations have reliable weather drivers with known freshness.
- **Deterministic / evidence**: define the input contract `{temp, humidity, wind, radiation, precip}` keyed on field/date from `15`; validate completeness and freshness on each fetch; stale input degrades gracefully, not silently.
- **Acceptance**:
  - Given `15` supplies complete fresh inputs, when fetched, then the contract validates and inputs are available with their freshness.
  - Given a stale or incomplete `15` input, when fetched, then it is flagged degraded and downstream ET is blocked or marked low-confidence (never silently used).
- **Tests**: unit (contract validation), integration (`15` fetch), failure path (stale/incomplete input degraded).
- **Depends on**: 16-01, `15`.

### STORY 16-04 · M2 · S · P2 — Per-field irrigation history
- **Story**: As `GR`, I want an auditable per-field irrigation event log, so that water use is repeatable and defensible season over season.
- **Deterministic / evidence**: persist append-only irrigation events `{field_id, zone_ref, applied_amount, source, timestamp, actor}`; queryable by field and date range.
- **Acceptance**:
  - Given irrigation events for a field, when queried by date range, then they return in order with amounts and actors.
  - Given a query for a field with no events, when run, then an explicit empty result returns (not an error).
- **Tests**: API contract (query), fixture (seeded events), failure path (empty history).
- **Depends on**: 16-01.

---

## M3 — Explainable (deterministic ET, need, and scheduling)

### STORY 16-05 · M3 · M · P2 — Evapotranspiration (ET) calculation
- **Story**: As `AG`, I want reference ET computed deterministically from `15` weather inputs with the method cited, so that water need rests on defensible numbers, not a guess.
- **Deterministic / evidence**: compute reference/crop ET from the validated `15` inputs by a documented method; output cites method and inputs; runs before any recommendation.
- **Acceptance**:
  - Given complete validated inputs, when ET runs, then a value is produced citing its method and inputs.
  - Given a missing required input, when ET runs, then it returns "insufficient inputs" rather than a partial value.
- **Tests**: unit (ET math), fixture (known case), failure path (missing input).
- **Depends on**: 16-03, `15`.

### STORY 16-06 · M3 · M · P2 — Zone-based water-need mapping
- **Story**: As `AG`, I want water need mapped onto management zones consumed from `09`/`05`, so that a recommendation always names the zone and the evidence behind it.
- **Deterministic / evidence**: combine moisture evidence (16-01/16-02) and ET (16-05) per management zone; emit `{zone_ref, water_need, evidence_refs[]}` in the correct CRS; need is deterministic and inspectable.
- **Acceptance**:
  - Given moisture and ET evidence and management zones, when mapping runs, then each zone gets a water-need value citing the evidence it used, in the correct CRS.
  - Given a zone with no usable moisture/ET evidence, when mapping runs, then that zone is marked "insufficient evidence" (no fabricated need).
- **Tests**: unit (need computation), geospatial (zone CRS), failure path (no evidence for a zone).
- **Depends on**: 16-01, 16-02, 16-05, `09`, `05`.

### STORY 16-07 · M3 · L · P2 — Irrigation scheduling engine
- **Story**: As `AG`, I want a per-zone water plan generated from moisture + ET evidence, so that each zone gets the right amount at the right time.
- **Deterministic / evidence**: produce a deterministic per-zone schedule `{zone_ref, amount, timing, evidence_refs[]}` from water need (16-06); the plan is inspectable and cites its inputs; no AI in the core schedule.
- **Acceptance**:
  - Given water-need per zone, when scheduling runs, then a per-zone plan with amounts and timing is produced citing its evidence.
  - Given zones flagged "insufficient evidence", when scheduling runs, then those zones are excluded from the plan with a stated reason (not scheduled blindly).
- **Tests**: unit (schedule logic), fixture (multi-zone), failure path (insufficient-evidence zones excluded).
- **Depends on**: 16-06.

---

## M4 — Interactive

### STORY 16-08 · M4 · M · P2 — Irrigation hardware/valve control interface
- **Story**: As `OPS`, I want to dry-run a schedule against a valve adapter with audit before executing, so that water is never applied without a checked, bounded, abortable action.
- **Deterministic / evidence**: a valve adapter exposes dry-run and execute; dry-run validates the plan against bounds and reports what would happen; execute requires the dry-run to pass, enforces bounds, supports abort, and writes an audit record per action.
- **Acceptance**:
  - Given a valid plan, when dry-run runs, then it reports the planned valve actions and amounts without applying water.
  - Given a plan that exceeds a valve's bounds (or with no passing dry-run), when execute is requested, then it is refused and audited (no out-of-bounds or un-dry-run application).
- **Tests**: unit (bounds/abort), integration (dry-run vs execute), failure path (out-of-bounds/un-dry-run refused).
- **Depends on**: 16-07.

### STORY 16-09 · M4 · S · P2 — Water-use and savings reporting
- **Story**: As `GR`, I want applied water reported against a baseline per field and zone, so that I can see and prove the water I saved.
- **Deterministic / evidence**: compute applied-vs-baseline from the irrigation history (16-04) per field/zone; report cites the baseline method and the events it summed.
- **Acceptance**:
  - Given irrigation events and a baseline, when a report runs, then applied water and savings per field/zone are computed citing their basis.
  - Given a field with no baseline defined, when a report runs, then savings are marked "no baseline" rather than computed against zero.
- **Tests**: unit (savings math), fixture (events + baseline), failure path (no baseline).
- **Depends on**: 16-04.

### STORY 16-10 · M4 · S · P2 — Alerts and notifications
- **Story**: As `GR`, I want a low-moisture or over-irrigation alert routed to the portal (`13`) and operator (`11`), so that I catch water problems early.
- **Deterministic / evidence**: raise an alert when moisture/ET evidence crosses a threshold; the alert cites its evidence and freshness; routed to `13`/`11` with field scope respected and delivery audited.
- **Acceptance**:
  - Given moisture below the low threshold on an owned field, when evaluation runs, then an alert is raised citing its evidence and routed to `13`/`11`.
  - Given evidence within thresholds, when evaluation runs, then no alert fires (no false alarm).
- **Tests**: unit (threshold evaluator), integration (route to `13`/`11`), failure path (within thresholds → no alert).
- **Depends on**: 16-06, `13`, `11`.

---

## Coverage note

These 10 stories cover all 10 capabilities in `capability-map.md` (~1 story each). The breakdown is M1/M2-weighted with an explainable M3 core (ET, water-need mapping, scheduling) that must run and be inspectable before any valve action, matching the **agronomic-value/data-quality lead** in `release-plan.md`. Only the soil-moisture data model (16-01) is P1; everything else is P2. There is no M5 story here: closed-loop irrigation is explicitly gated behind reliable deterministic scheduling and a proven valve-control safety path. The curated counts in `release-plan.md` (~66 rows) expand several of these (per-sensor and per-proxy ingest, additional scheduling and valve-adapter slices, M5 closed-loop) into sibling stories when implemented.
