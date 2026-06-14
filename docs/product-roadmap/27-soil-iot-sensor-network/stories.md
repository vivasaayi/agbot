# Soil and IoT Sensor Network: Detailed Stories

> Greenfield domain (M0 named): no code exists yet. Every story below is **built from scratch** in the new `soil_iot` (a.k.a. `sensor_network`) crate, gated behind the core platform (`01`–`12`) and the field/zone model (`10`). This domain runs unattended sensors on battery and intermittent links, so the **data-quality and geospatial-correctness pillars dominate every phase**: no reading is trusted until it is range-checked, calibrated, and proven fresh, and time-series storage is **delegated to `28`** rather than reinvented. The MQTT / LoRaWAN gateway is a true external boundary kept behind a mockable interface.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 27-01 · M1 · M · P0 — Device identity and registry
- **Story**: As `PA`, I want to register a geolocated sensor with its type and calibration profile linked to an org and field/zone via `10`, so that every reading is tied to a known, owned, located device.
- **Deterministic / evidence**: persist `{device_id, org_id, field_id, zone_id, type, position{lat,lon,crs}, calibration_profile_ref, status}`; lifecycle `Registered→Active→Maintenance→Retired`; no reading is accepted from an unregistered or retired device.
- **Acceptance**:
  - Given an org and field/zone, when a device is registered with a position and calibration profile, then a record is created with all linkage IDs and `Active` status.
  - Given a reading referencing an unknown or retired device, when it is ingested, then it is rejected and the rejection is audited.
- **Tests**: unit (lifecycle transitions), API contract (register/list), failure path (reading from unregistered device rejected).
- **Depends on**: `10` (org/field/zone model); reuses `04` provenance patterns.

### STORY 27-02 · M1 · M · P0 — Gateway ingest pipeline (MQTT / LoRaWAN)
- **Story**: As `OPS`, I want raw readings to arrive through a gateway adapter behind a clear interface, so that ingest works with real hardware or a simulated gateway without changing the pipeline.
- **Deterministic / evidence**: a `GatewayAdapter` trait with `{subscribe, next_reading}`; MQTT and LoRaWAN are mockable external boundaries; a `SimulatedGateway` emits readings under `RUNTIME_MODE=SIMULATION`; each ingested reading records `{device_id, raw_value, gateway_ts, received_at}`.
- **Acceptance**:
  - Given a simulated gateway emitting readings, when ingest runs, then each reading is associated with a registered device and stamped with receive time.
  - Given a malformed/undecodable gateway payload, when ingested, then it is rejected with a reason code and counted, not silently dropped.
- **Tests**: unit (adapter decode), fixture (simulated gateway stream), failure path (malformed payload rejected and counted).
- **Depends on**: 27-01; reuses `04` collection-failure handling.

### STORY 27-03 · M1 · S · P1 — Provisioning and config version tracking
- **Story**: As `OPS`, I want each device to carry a config/firmware version and a push outcome, so that I know what every field device is running.
- **Deterministic / evidence**: persist `{device_id, config_version, pushed_at, push_status}`; a config push is recorded as pending/applied/failed with a reason; version is read-only history.
- **Acceptance**:
  - Given a registered device, when a config push is recorded, then its version and push status persist and are listable.
  - Given a push that the device never acknowledges, when the ack window elapses, then the push is marked failed (not silently "applied").
- **Tests**: unit (push state machine), API contract (config history), failure path (unacked push → failed).
- **Depends on**: 27-01.

---

## M2 — Captured / Observable

### STORY 27-04 · M2 · M · P0 — Geolocated readings tied to field/zone
- **Story**: As `AG`, I want every reading to carry CRS/position and a field/zone reference, so that soil data is spatially correct and can be compared to aerial layers.
- **Deterministic / evidence**: each reading inherits the device position and CRS; assert the position lies within (or is associated with) the device's field/zone; reject readings whose device has no valid position.
- **Acceptance**:
  - Given a registered device with a position, when a reading is ingested, then the reading carries `{position, crs, field_id, zone_id}` and round-trips geospatially.
  - Given a device with a missing/invalid position, when a reading arrives, then it is flagged "no geolocation" and excluded from geospatial products (not assigned to a default point).
- **Tests**: unit (position inheritance), geospatial (CRS round-trip), failure path (missing position flagged).
- **Depends on**: 27-01, 27-02, `07`.

### STORY 27-05 · M2 · S · P0 — Time-series persistence via `28`
- **Story**: As `OPS`, I want validated readings stored as a series through the `28` time-series contract, so that we do not reinvent time-series storage and series are queryable over time.
- **Deterministic / evidence**: append `{device_id, metric, value, ts, qa_flags}` to the `28` series store via its contract; this domain holds no bespoke time-series tables; reads go through `28` query APIs.
- **Acceptance**:
  - Given a validated reading, when persisted, then it is written via the `28` contract and retrievable by device + time range.
  - Given the `28` subsystem is unavailable, when a reading must be persisted, then ingest backpressures/queues and the failure is surfaced (no silent data loss).
- **Tests**: contract test (write/read via `28`), fixture (series), failure path (`28` unavailable → queued/surfaced).
- **Depends on**: 27-02, `28`.

### STORY 27-06 · M2 · M · P1 — Capture freshness and coverage tracking
- **Story**: As `OPS`, I want per-device freshness and per-zone coverage tracked, so that I know which sensors are reporting and which zones are blind.
- **Deterministic / evidence**: compute `last_seen` and an expected-interval freshness state per device; aggregate live-device count per zone into a coverage tally; analog of `04` freshness/coverage.
- **Acceptance**:
  - Given devices reporting at an expected interval, when freshness runs, then each device shows fresh/stale against its interval and each zone shows a coverage count.
  - Given a device that stops reporting, when its interval elapses, then it is marked stale (not left "fresh" indefinitely).
- **Tests**: unit (freshness/coverage tally), fixture (interrupted stream), failure path (silent device → stale).
- **Depends on**: 27-02, 27-04.

---

## M3 — Explainable (the deterministic data-quality core)

### STORY 27-07 · M3 · M · P0 — Reading validation and calibration
- **Story**: As `AG`, I want each raw reading range-checked and calibrated against the device's profile with reason-coded QA flags, so that I only act on trustworthy values.
- **Deterministic / evidence**: apply the calibration profile (e.g. linear `a·raw + b`, sensor-specific transform) to produce a calibrated value; range-check against the metric's valid bounds; emit `{calibrated_value, qa_flags[], reason_code}`; retain the raw value.
- **Acceptance**:
  - Given a raw reading and a calibration profile, when validation runs, then a calibrated value and QA flags are produced and the raw value is retained.
  - Given a reading outside the metric's physical range, when validated, then it is flagged out-of-range with a reason code and excluded from products (retained, not dropped).
- **Tests**: unit (calibration math + range check), fixture (profiles + raw streams), failure path (out-of-range flagged + retained).
- **Depends on**: 27-01, 27-05.

### STORY 27-08 · M3 · S · P0 — Stuck-sensor / flatline detection
- **Story**: As `AG`, I want a sensor that reports an identical or non-varying value over a window flagged as stuck, so that a dead sensor is not mistaken for stable soil.
- **Deterministic / evidence**: over a configured window, flag the series when variance/range is below a threshold (flatline) or the value is pinned at a rail; emit `{reason_code=stuck, window, observed_variance}`.
- **Acceptance**:
  - Given a series with normal variation, when detection runs, then no stuck flag is raised (no false positive).
  - Given a series flatlined below the variance threshold for the window, when detection runs, then it is flagged stuck with the window and observed variance.
- **Tests**: unit (flatline/variance logic), fixture (stuck vs varying series), failure path (varying series → no flag).
- **Depends on**: 27-05, 27-07.

### STORY 27-09 · M3 · M · P1 — Soil-moisture / EC / temperature products
- **Story**: As `AG`, I want per-zone soil-moisture, EC, and temperature summaries with freshness, so that I have defensible ground numbers per zone, not just per probe.
- **Deterministic / evidence**: aggregate validated (non-QA-flagged) readings per zone into `{metric, mean, min, max, n, freshness, contributing_devices[]}`; exclude flagged readings; carry zone CRS/position.
- **Acceptance**:
  - Given validated readings in a zone, when a product is computed, then it reports the metric summary, freshness, and which devices contributed, in the correct CRS.
  - Given a zone whose only readings are QA-flagged, when a product is requested, then it returns "no valid data" with the freshness gap (not a fabricated value).
- **Tests**: unit (aggregation excluding flags), geospatial (zone CRS), failure path (all-flagged zone → no valid data).
- **Depends on**: 27-07, 27-08, `10`.

### STORY 27-10 · M3 · S · P1 — Reproducibility and QA evidence retention
- **Story**: As `DSP`, I want validated series and QA flags to be re-derivable from raw readings and the calibration profile, so that a soil result can be defended.
- **Deterministic / evidence**: re-running validation on the same raw readings and profile yields identical calibrated values and QA flags; each flag stores method, threshold, and the raw reading.
- **Acceptance**: re-validating a fixed raw series and profile produces an identical output hash; each QA flag cites its rule and the raw value.
- **Tests**: determinism (same input → same output hash), fixture (raw series + profile).
- **Depends on**: 27-07, 27-08.

---

## M4 — Interactive (monitoring, fusion, and field action)

### STORY 27-11 · M4 · M · P0 — Sensor health, battery, and connectivity monitoring
- **Story**: As `OPS`, I want stale, low-battery, or disconnected devices detected and surfaced as events to `29`, so that I can service the network before it goes blind.
- **Deterministic / evidence**: evaluate `{freshness, battery_level, link_status}` against thresholds; emit a typed `29` event `{device_id, reason_code, severity, evidence}` on breach; events are deduped per device per condition.
- **Acceptance**:
  - Given a device whose battery drops below the threshold, when monitoring runs, then a typed sensor-health event is emitted to `29` citing the battery level and rule.
  - Given a device that recovers, when re-evaluated, then a resolve event is emitted and the condition is not re-fired every cycle (dedup holds).
- **Tests**: unit (threshold + dedup), integration (`29` event contract), failure path (flapping device does not spam alerts).
- **Depends on**: 27-06, `29`.

### STORY 27-12 · M4 · S · P0 — Irrigation trigger inputs to `16`
- **Story**: As `GR`, I want a deterministic soil-moisture threshold breach to emit an irrigation trigger to `16`, so that ground data drives a real irrigation decision.
- **Deterministic / evidence**: when a zone's validated soil-moisture product crosses a configured low threshold and is fresh, emit `{zone_id, metric, value, threshold, trigger_ts, evidence_refs[]}` to `16`; never trigger on stale or QA-flagged data.
- **Acceptance**:
  - Given a fresh zone soil-moisture product below the threshold, when evaluated, then an irrigation trigger is emitted to `16` citing the value, threshold, and contributing readings.
  - Given a zone whose latest data is stale or all-flagged, when evaluated, then no trigger fires and the reason (stale/insufficient evidence) is recorded.
- **Tests**: unit (threshold trigger), integration (`16` input contract), failure path (stale data → no trigger).
- **Depends on**: 27-09, `16`.

### STORY 27-13 · M4 · M · P1 — Ground-truth fusion with aerial indices (`05`)
- **Story**: As `AG`, I want zone soil-moisture compared against `05` NDVI for the same field and date window, so that ground sensors validate or challenge the aerial picture.
- **Deterministic / evidence**: align a zone soil product with the nearest `05` index product by field/zone and time window; compute a deterministic agreement/divergence summary with both sources cited; flag spatial/CRS or temporal mismatch instead of fusing blindly.
- **Acceptance**:
  - Given a zone soil product and a comparable `05` NDVI product, when fusion runs, then an agreement/divergence summary is produced citing both source layers, dates, and CRS.
  - Given products whose zones/CRS do not align or whose dates are outside the window, when fusion is requested, then it is refused with a mismatch reason (no blind fusion).
- **Tests**: unit (alignment + divergence), geospatial (zone/CRS alignment), failure path (CRS/temporal mismatch refused).
- **Depends on**: 27-09, `05`, `07`.

### STORY 27-14 · M4 · S · P2 — Network coverage and gap export
- **Story**: As `DSP`, I want to export which zones have no live sensor coverage as GeoJSON/CSV, so that I can plan where to add or service devices.
- **Deterministic / evidence**: from the coverage tally (27-06), emit zones with zero fresh devices as GeoJSON polygons with correct CRS plus a CSV summary; empty result is a valid empty export.
- **Acceptance**: covered and uncovered zones export with correct CRS and a coverage count per zone; a fully covered field exports a valid empty gap set.
- **Tests**: geospatial round-trip, schema validation, failure path (fully covered → valid empty export).
- **Depends on**: 27-06, 27-04.

---

## M5 — Autonomous-Assist (gated, uncertainty-flagged)

### STORY 27-15 · M5 · M · P2 — Drift/recalibration advisory
- **Story**: As `AG`, I want an advisory when a device's calibrated readings appear to drift relative to neighbors or aerial ground-truth, so that I can recalibrate before bad data spreads — without over-trusting the advisory.
- **Deterministic / evidence**: drift signal is composed only from already-validated series and fusion (27-13); every advisory carries an uncertainty band and cites the evidence; feature-flagged and approval-gated; never recalibrates automatically.
- **Acceptance**:
  - Given trustworthy validated series and a neighbor/aerial reference, when drift evaluation runs, then a recalibration advisory is produced with an uncertainty band citing its evidence.
  - Given missing or stale reference data, when drift is requested, then it is unavailable (never fabricated), and no automatic recalibration occurs.
- **Tests**: unit (drift composition + uncertainty), gating test (disabled until validated series + reference exist), failure path (stale reference → unavailable).
- **Depends on**: 27-07, 27-09, 27-13.

---

## Coverage note

These 15 stories cover all 12 capabilities in `capability-map.md`. The breakdown is data-quality-led, with a deliberately heavy M3 validation/calibration/QA core (validation, stuck/flatline, products, reproducibility) reflecting that **data quality and geospatial correctness lead every phase** in `release-plan.md`. Time-series storage is delegated to `28` (27-05) rather than reinvented. The single M5 story (drift/recalibration advisory) stays approval-gated and uncertainty-flagged. The curated counts in `release-plan.md` (~79 rows) expand several of these (per-metric product variants, additional gateway/calibration and health-rule slices) into sibling stories when implemented.
