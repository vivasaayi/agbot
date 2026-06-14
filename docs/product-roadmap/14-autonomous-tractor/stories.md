# Autonomous Tractor (AruviTrac): Detailed Stories

> Greenfield domain (M0 named): no code exists yet. Every story below is **built from scratch** and is gated behind the core drone platform (`01`–`12`) and the advisor MVP (`09`). This domain moves a heavy vehicle among people and equipment, so the **safety pillar dominates every phase**: no real ground motion is enabled until guidance, geofence, e-stop, and obstacle halt all pass in simulation. Stories are coarser, M1/M2-weighted, and almost entirely P2 (only tractor identity is P1).

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Safety / deterministic**: the guardrail or inspectable logic that must hold without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `TRACTOR-OPS` field operator of the tractor, `PA` platform admin, `AG` agronomist, `GR` grower, `OPS` operator.

---

## M1 — Foundation

### STORY 14-01 · M1 · M · P1 — Tractor vehicle identity and registry
- **Story**: As `PA`, I want to register a tractor and its attached implement linked to an org and field via `10`, so that every field-ops action is tied to a known, owned vehicle.
- **Safety / deterministic**: persist `{tractor_id, org_id, capabilities, implement_ref, status}`; lifecycle `Registered→Available→InUse→OutOfService`; no motion command may reference an unregistered or out-of-service tractor.
- **Acceptance**:
  - Given an org and field, when a tractor + implement is registered, then a record is created with capabilities and `10` linkage.
  - Given a command targeting an unknown or out-of-service tractor, when it is submitted, then it is rejected (4xx) and audited.
- **Tests**: unit (lifecycle), API contract (register/list), failure path (command to unregistered vehicle).
- **Depends on**: `10` (org/field model), reuses `01` mission-identity patterns.

### STORY 14-02 · M1 · L · P2 — GPS/RTK guidance and path following (simulation)
- **Story**: As `TRACTOR-OPS`, I want the tractor to follow a straight planned path with bounded cross-track error in simulation, so that guidance is proven before any real motion.
- **Safety / deterministic**: path-following controller computes cross-track error each tick and keeps it within a configured bound; runs in `RUNTIME_MODE=simulation` only — real motion is hard-disabled at this stage.
- **Acceptance**:
  - Given a straight path in simulation, when guidance runs, then cross-track error stays within the configured bound for the whole run.
  - Given an induced disturbance that exceeds the bound, when guidance cannot recover, then the run halts and flags a guidance fault (does not silently drift off-path).
- **Tests**: unit (cross-track error math), simulation (path-following run), failure path (unrecoverable disturbance halts).
- **Depends on**: 14-01, reuses `01` dispatch/command-ack skeleton.

### STORY 14-03 · M1 · M · P2 — Coverage / path planning from a boundary
- **Story**: As `TRACTOR-OPS`, I want a swath coverage plan generated from a field boundary, so that the tractor has a validated path that fills the field.
- **Safety / deterministic**: turn a `07`/`10` boundary into swaths at the implement's working width; assert plan stays inside the boundary and reports coverage fraction; analog of `01` survey patterns.
- **Acceptance**:
  - Given a field boundary and an implement width, when planning runs, then a swath plan is produced with coverage fraction and all swaths inside the boundary, in the correct CRS.
  - Given a boundary with a hole/exclusion, when planning runs, then swaths avoid the exclusion (no planned path crosses it).
- **Tests**: unit (swath generation + coverage), geospatial (CRS/inside-boundary), failure path (exclusion respected).
- **Depends on**: 14-01, `07`, `10`.

---

## M2 — Captured / Observable

### STORY 14-04 · M2 · M · P2 — Field-ops session logging and telemetry
- **Story**: As `TRACTOR-OPS`, I want every field-ops session logged with telemetry and coverage, so that what the tractor did is captured and reviewable.
- **Safety / deterministic**: persist `{session_id, tractor_id, field_id, started_at, telemetry[], coverage, safety_events[]}`; record freshness/gaps in telemetry; analog of `04`/`01` session capture, routed to `04`/`10`.
- **Acceptance**:
  - Given a running session, when telemetry streams, then position/speed/implement-state samples persist with timestamps and a coverage tally.
  - Given a telemetry dropout, when samples stop arriving, then the gap is recorded and flagged (not silently interpolated).
- **Tests**: fixture (telemetry stream), unit (coverage tally), failure path (dropout flagged).
- **Depends on**: 14-01, 14-02, `04`, `10`.

### STORY 14-05 · M2 · S · P2 — After-action replay and audit
- **Story**: As `AG`, I want to replay a session's path, telemetry, and safety events, so that I can review what happened after the fact.
- **Safety / deterministic**: reconstruct the session deterministically from the logged telemetry and safety-event records; replay is read-only and reproduces the same timeline each run.
- **Acceptance**:
  - Given a completed session, when replayed, then path, telemetry, and every safety event render on a consistent timeline.
  - Given a session with a corrupt/missing telemetry segment, when replayed, then the gap is shown explicitly (no fabricated path through the gap).
- **Tests**: determinism (same log → same replay), fixture (session with e-stop event), failure path (corrupt segment).
- **Depends on**: 14-04.

---

## M3 — Explainable (the deterministic ground-safety core)

### STORY 14-06 · M3 · M · P2 — Geofence and boundary enforcement
- **Story**: As `TRACTOR-OPS`, I want motion outside the field geofence rejected, so that the tractor cannot leave its authorized area.
- **Safety / deterministic**: a deterministic geofence evaluator checks every motion command/position against the field boundary; a breach (or predicted breach) halts motion and records `{reason_code, position, boundary_ref}`; reuses the `03` geofence model.
- **Acceptance**:
  - Given a planned move inside the boundary, when evaluated, then it is permitted.
  - Given a move that would cross the geofence, when evaluated, then motion is halted with a geofence reason code before the boundary is crossed.
- **Tests**: unit (geofence evaluator incl. predicted breach), geospatial (boundary CRS), failure path (breach halts).
- **Depends on**: 14-02, 14-03, `03`, `07`.

### STORY 14-07 · M3 · M · P2 — E-stop and operator approval
- **Story**: As `TRACTOR-OPS`, I want a hardware/soft e-stop that halts all motion immediately and an explicit approval gate before any motion, so that a human is always in control.
- **Safety / deterministic**: e-stop transitions the tractor to a halted state with the highest priority, pre-empting all other commands; no motion command executes without a recorded operator approval; abort is always available; e-stop and approval are audited.
- **Acceptance**:
  - Given a moving (sim) tractor, when e-stop is triggered, then motion halts immediately and the e-stop event is recorded with actor and timestamp.
  - Given a motion command without a recorded operator approval, when it is submitted, then it is refused and audited (no un-approved motion).
- **Tests**: unit (e-stop pre-emption priority), API contract (approval gate), failure path (un-approved motion refused).
- **Depends on**: 14-01, 14-02.

### STORY 14-08 · M3 · M · P2 — Obstacle detection
- **Story**: As `TRACTOR-OPS`, I want the tractor to stop on a detected obstacle in its path, so that it does not strike people, animals, or equipment.
- **Safety / deterministic**: a deterministic detector (sim sensor) raises an obstacle event when an object enters the planned path within a stopping-distance threshold; an obstacle halts motion and records `{distance, position, reason_code}`.
- **Acceptance**:
  - Given a clear path, when the detector runs, then no false halt occurs.
  - Given an obstacle entering the path inside the stopping threshold, when detected, then motion halts before reaching it and the event is logged.
- **Tests**: unit (detection + stopping-distance), simulation (obstacle-in-path), failure path (clear path → no false halt).
- **Depends on**: 14-02, 14-07.

---

## M4 — Interactive

### STORY 14-09 · M4 · M · P2 — Prescription-map execution
- **Story**: As `TRACTOR-OPS`, I want to execute a per-zone prescription map from `09`/`05` management zones, so that the implement applies the right rate in the right zone.
- **Safety / deterministic**: the implement controller reads a management-zone map, applies per-zone rate setpoints, and retains raw evidence + reason codes; execution requires geofence (14-06), approval (14-07), and obstacle halt (14-08) all active; runs in simulation first.
- **Acceptance**:
  - Given a valid zone map and an approved session, when execution runs (sim), then each zone receives its prescribed rate and the applied rates are logged against the zones.
  - Given a prescription map whose zones do not align with the field CRS/extent, when execution is requested, then it is refused with a mismatch error (no misapplied rates).
- **Tests**: unit (per-zone rate application), geospatial (zone/field alignment), failure path (CRS/extent mismatch refused).
- **Depends on**: 14-06, 14-07, 14-08, `09`, `05`, `07`.

### STORY 14-10 · M4 · M · P2 — Implement control (planter/sprayer/tiller)
- **Story**: As `TRACTOR-OPS`, I want one abstract implement interface with on/off and rate setpoint, so that different implements can be driven through a common, safe contract.
- **Safety / deterministic**: an implement adapter exposes `{enable, disable, set_rate}` with bounds; rate is clamped to the implement's valid range; the implement is forced off whenever the tractor is halted (e-stop/geofence/obstacle).
- **Acceptance**:
  - Given a valid rate within range, when set, then the implement adapter applies it and logs the setpoint.
  - Given an out-of-range rate, when set, then it is rejected (clamped/refused) and the implement never exceeds its bounds; and on e-stop the implement goes off.
- **Tests**: unit (rate bounds + clamp), integration (halt forces implement off), failure path (out-of-range rate refused).
- **Depends on**: 14-07, 14-09.

### STORY 14-11 · M4 · S · P2 — Weather/operational window gating
- **Story**: As `TRACTOR-OPS`, I want field ops blocked outside a `15` spray/field window, so that I do not operate in unsafe or unproductive conditions.
- **Safety / deterministic**: before a session starts, check the `15` operational window for the field; if outside the window (or the window data is stale/missing), block start and cite the gating inputs.
- **Acceptance**:
  - Given a field inside a valid `15` window, when a session is requested, then it is allowed.
  - Given a field outside the window or with stale window data, when a session is requested, then start is blocked with the cited reason (never started on missing/stale data).
- **Tests**: unit (window gate), integration (`15` window), failure path (stale/missing window blocks).
- **Depends on**: 14-04, 14-07, `15`.

---

## M5 — Autonomous-Assist (gated, operator-approved)

### STORY 14-12 · M5 · M · P2 — Multi-vehicle coordination
- **Story**: As `TRACTOR-OPS`, I want two tractors sharing a field boundary to deconflict, so that a ground fleet operates without collisions, the way drones do in `03`.
- **Safety / deterministic**: a deterministic deconfliction check (reusing the `03` coordination model) reserves swaths/space-time so two tractors never occupy the same area; a conflict halts the lower-priority vehicle; every step stays operator-approved and the single-vehicle safety core (14-06..14-08) must be active.
- **Acceptance**:
  - Given two tractors with non-overlapping swaths, when coordinated, then both proceed without conflict.
  - Given two tractors whose paths would intersect, when coordinated, then the deconfliction check halts the lower-priority vehicle before conflict and logs it.
- **Tests**: unit (deconfliction reservation), simulation (two-tractor conflict), failure path (conflicting paths → lower-priority halts).
- **Depends on**: 14-06, 14-07, 14-08, `03`.

---

## Coverage note

These 12 stories cover all 12 capabilities in `capability-map.md` (~1 story each). The breakdown is M1/M2-weighted with a deliberately heavy M3 ground-safety core (geofence, e-stop/approval, obstacle), reflecting that **safety leads every phase** in `release-plan.md`. Only tractor identity (14-01) is P1; everything else is P2. The single M5 story (multi-vehicle coordination) stays operator-approved and is gated behind a reliable single-vehicle safety core — matching the execution rule that no real ground motion is enabled until guidance, geofence, e-stop, and obstacle halt all pass in simulation. The curated counts in `release-plan.md` (~76 rows) expand several of these (per-implement variants, additional guidance and safety-evaluator slices) into sibling stories when implemented.
