# Flight and Mission Control: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Safety / deterministic**: what must be enforced or computed without AI, with a working abort.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `OPS` operator/pilot, `DSP` drone service provider, `GR` grower, `PA` platform admin.

This is a safety-critical domain. No real-flight dispatch is enabled until the full path passes in `Simulation` mode against domain `02`, and every dispatch enforces geofence, altitude ceiling, battery budget, and a working abort.

---

## M1 — Foundation

### STORY 01-01 · M1 · S · P0 — Mission identity and linkage
- **Story**: As `DSP`, I want each mission to have a stable ID linked to field, season, and capture session, so that every flight is traceable and reproducible.
- **Safety / deterministic**: persist `{mission_id, field_id, season_id, session_id?, owner_id, status, created_at}`; lifecycle `Draft→Validated→Armed→InFlight→Completed|Aborted|Failed`; transitions are deterministic and rejected if out of order.
- **Acceptance**:
  - Given a field, when a mission is created, then a row is persisted with all linkage IDs and `Draft` status via `mission_planner` CRUD.
  - Given a mission in `Draft`, when arming is requested before validation, then the transition is rejected with a state-error code.
- **Tests**: unit (state machine transitions), API contract (create/get/list), failure path (arm-before-validate rejected).
- **Depends on**: `10` (field/season model), existing `mission_planner/src/database.rs`.

### STORY 01-02 · M1 · S · P1 — Mission CRUD listing and versioning
- **Story**: As `OPS`, I want to list, filter, and re-open missions for a field, so that I can reuse and audit prior plans.
- **Safety / deterministic**: results paginated and filterable by field/season/status/date; each edit bumps a version and retains the prior revision.
- **Acceptance**:
  - Given multiple missions, when listed, then they paginate and filter by field/season/status.
  - Given a mission edited twice, when its history is read, then both prior versions are retrievable.
- **Tests**: API contract (pagination + filters), unit (version bump), fixture (seeded missions).
- **Depends on**: 01-01.

### STORY 01-03 · M1 · M · P0 — Waypoint model and sanity validation
- **Story**: As `OPS`, I want waypoints validated for sanity before a mission is accepted, so that obviously unflyable plans never reach dispatch.
- **Safety / deterministic**: validate waypoint sequence, action types (takeoff/fly/land/loiter ordering), monotonic-or-bounded altitude steps, leg distance, and duplicate/zero-length legs; reject with per-waypoint reason codes.
- **Acceptance**:
  - Given a valid waypoint list, when validated, then it passes and the mission becomes `Validated`.
  - Given a mission missing a takeoff or landing, when validated, then it is rejected with a specific reason code, not silently accepted.
- **Tests**: unit (sequence/action/altitude rules), failure path (no-landing mission rejected), fixture (sample missions).
- **Depends on**: 01-01, `mission_planner/src/waypoint.rs`.

### STORY 01-04 · M1 · S · P1 — Altitude and geofence bound check at plan time
- **Story**: As `OPS`, I want waypoints checked against an altitude ceiling and field geofence at plan time, so that out-of-bounds plans fail early.
- **Safety / deterministic**: assert each waypoint is within the field geofence polygon and below the altitude ceiling; emit the violating waypoint index and bound.
- **Acceptance**:
  - Given a plan inside the geofence and under the ceiling, when checked, then it passes.
  - Given a waypoint above the ceiling, when checked, then validation fails citing the waypoint and the ceiling value.
- **Tests**: unit (point-in-polygon + ceiling), failure path (over-ceiling waypoint), geospatial round-trip (polygon CRS).
- **Depends on**: 01-03, `03` (geofence primitives shared).

### STORY 01-05 · M1 · M · P0 — Survey-pattern templates from a field boundary
- **Story**: As `OPS`, I want to generate a coverage mission (grid, lawnmower, perimeter) from a field boundary, so that I can plan a capture flight without placing waypoints by hand.
- **Safety / deterministic**: deterministic pattern generator takes a boundary polygon + spacing/overlap + altitude and emits a validated waypoint mission within the geofence; coverage and leg count are reported.
- **Acceptance**:
  - Given a field boundary and lawnmower spacing, when a template runs, then a waypoint mission is produced fully inside the boundary with reported coverage fraction.
  - Given a spacing larger than the field, when generation runs, then it fails with a "spacing exceeds extent" error rather than emitting an empty mission.
- **Tests**: unit (pattern geometry, coverage), failure path (spacing too large), geospatial round-trip.
- **Depends on**: 01-03, 01-04, `10` (field boundary).

---

## M2 — Captured / Observable

### STORY 01-06 · M2 · M · P0 — Live telemetry streaming and persistence
- **Story**: As `OPS`, I want live telemetry streamed and persisted during a flight, so that I can supervise the aircraft and replay the flight afterward.
- **Safety / deterministic**: persist a `Telemetry` history (position, altitude, battery, mode, link state) with monotonic timestamps from `mission_control`; stream over WebSocket to the ground station.
- **Acceptance**:
  - Given an in-flight mission, when telemetry arrives, then each sample is persisted with a timestamp and streamed to subscribers.
  - Given a subscriber that reconnects, when it resubscribes, then it receives the latest state without gaps in the persisted record.
- **Tests**: API/WebSocket contract (subscribe/replay), unit (sample ordering), fixture (seeded telemetry stream).
- **Depends on**: 01-01, `mission_control/src/websocket_server.rs`, `shared` Telemetry schema.

### STORY 01-07 · M2 · S · P0 — Telemetry freshness and gap detection
- **Story**: As `OPS`, I want stale or missing telemetry flagged, so that I never trust a frozen position display.
- **Safety / deterministic**: compute per-stream freshness (age since last sample) and detect gaps over a threshold; mark the link `Stale` and surface it to the operator.
- **Acceptance**:
  - Given a steady stream, when freshness is computed, then the link reads `Fresh`.
  - Given samples stop for longer than the threshold, when freshness is computed, then the link transitions to `Stale` and a gap event is recorded.
- **Tests**: unit (freshness + gap math), failure path (stream stops → Stale), fixture.
- **Depends on**: 01-06.

### STORY 01-08 · M2 · S · P1 — Link-health and failsafe-state telemetry
- **Story**: As `OPS`, I want link quality and failsafe state tracked, so that I see degradation before it becomes a loss.
- **Safety / deterministic**: track link RSSI/loss-rate and the aircraft failsafe flags from MAVLink; persist transitions with timestamps.
- **Acceptance**:
  - Given a degrading link, when health is tracked, then loss-rate is recorded and a warning state is raised.
  - Given a failsafe flag set by the aircraft, when telemetry is parsed, then the transition is persisted and surfaced, not dropped.
- **Tests**: unit (link-health thresholds), failure path (failsafe flag persisted), fixture.
- **Depends on**: 01-06.
- **Note**: dependency on 01-09 (MAVLink command ack/timeout/retry, M3) removed. Link-health and failsafe-flag tracking is passive telemetry parsing and does not require command acknowledgment infrastructure. 01-09 is a prerequisite for dispatch stories (01-15+), not for read-only health observation.

---

## M3 — Explainable

### STORY 01-09 · M3 · M · P0 — MAVLink command ack/timeout/retry
- **Story**: As `OPS`, I want every command to require an ack with timeout and bounded retry, so that I know whether the aircraft received it.
- **Safety / deterministic**: each command carries a correlation ID; wait for ack within a timeout, retry up to a bound, then surface a hard failure; no command is assumed delivered without an ack.
- **Acceptance**:
  - Given a command, when the aircraft acks, then the command is marked `Acked` with latency recorded.
  - Given no ack within the timeout after max retries, when the deadline passes, then the command is marked `Failed` and the operator is alerted — never silently dropped.
- **Tests**: unit (ack/timeout/retry state machine), failure path (no-ack → Failed), simulation contract against `02`.
- **Depends on**: 01-06, `02` (twin command path), `mission_control/src/mavlink_client.rs`.

### STORY 01-10 · M3 · M · P0 — Geofence and no-fly enforcement at dispatch
- **Story**: As `OPS`, I want dispatch rejected when any waypoint or current position violates the geofence or a no-fly zone, so that the aircraft cannot be sent out of bounds.
- **Safety / deterministic**: re-run point-in-polygon geofence and no-fly checks at arm/dispatch (not only plan time); raise a `SafetyViolation` with severity and block the transition.
- **Acceptance**:
  - Given a compliant mission, when dispatch is requested, then the check passes and the mission may arm.
  - Given a no-fly zone intersecting a leg, when dispatch is requested, then dispatch is rejected with a violation record and the aircraft never arms.
- **Tests**: unit (geofence/no-fly evaluator), failure path (no-fly intersection blocks dispatch), geospatial round-trip.
- **Depends on**: 01-04, `03` (shared geofence/no-fly primitives).

### STORY 01-11 · M3 · S · P0 — Arming and pre-flight checklist gate
- **Story**: As `OPS`, I want arming blocked until geofence, battery, and GPS-fix checks pass, so that the aircraft never launches in an unsafe state.
- **Safety / deterministic**: a deterministic pre-flight checklist evaluator (geofence OK, battery above launch budget, GPS fix/HDOP, link fresh, failsafe configured); arming is impossible until all pass, each result inspectable.
- **Acceptance**:
  - Given all checks pass, when arming is requested, then the mission transitions to `Armed`.
  - Given a low battery below the launch budget, when arming is requested, then arming is blocked with the failing check named.
- **Tests**: unit (each checklist item), failure path (low battery blocks arm), API contract (checklist result).
- **Depends on**: 01-10, 01-13 (battery budget), 01-07 (link freshness).

### STORY 01-12 · M3 · M · P1 — Deterministic path cost and battery/time budget
- **Story**: As `OPS`, I want a mission's path cost, flight time, and battery draw estimated deterministically, so that I know a plan fits one battery before launch.
- **Safety / deterministic**: optimizer computes leg distances, estimated time, and battery consumption from a draw model; flag missions that exceed the battery budget with margin.
- **Acceptance**:
  - Given a waypoint mission, when costed, then total distance, time, and battery draw are returned with the budget margin.
  - Given a mission that exceeds the battery budget, when costed, then it is flagged "over budget" and blocked from arming.
- **Tests**: unit (cost + battery model), failure path (over-budget flagged), fixture (`mission_planner/src/mission_optimizer.rs`).
- **Depends on**: 01-03, 01-11.

### STORY 01-13 · M3 · S · P1 — Weather and airspace constraint flags
- **Story**: As `OPS`, I want wind, precipitation, and airspace constraints flagged before dispatch, so that I do not launch into unsafe conditions.
- **Safety / deterministic**: evaluate wind/precip against configured thresholds and intersect the plan with airspace constraints; emit deterministic flags with the threshold and value.
- **Acceptance**:
  - Given conditions under threshold, when evaluated, then no constraint flag is raised.
  - Given wind above the threshold, when evaluated, then a blocking flag is raised citing the wind value and limit.
- **Tests**: unit (threshold logic), failure path (over-wind blocks dispatch), fixture.
- **Depends on**: 01-10.

### STORY 01-14 · M3 · S · P1 — Mission audit log persistence
- **Story**: As `DSP`, I want every command, telemetry sample, and safety event persisted to an audit log, so that any flight can be reconstructed after the fact.
- **Safety / deterministic**: append-only audit of `{command, ack, telemetry refs, safety violations, mode transitions}` keyed by mission ID, re-derivable into a timeline.
- **Acceptance**:
  - Given a completed mission, when its audit is read, then the full command/telemetry/safety timeline is reconstructable in order.
  - Given a missing audit entry for an executed command, when the audit is validated, then the gap is detected and reported.
- **Tests**: unit (timeline assembly), failure path (gap detection), fixture.
- **Depends on**: 01-06, 01-09, 01-10.

---

## M4 — Interactive

### STORY 01-15 · M4 · L · P0 — Guarded MAVLink command dispatch
- **Story**: As `OPS`, I want to dispatch arm/takeoff/goto/land commands through a guarded interface, so that I can fly a validated mission safely.
- **Safety / deterministic**: dispatch is impossible unless the mission is `Armed`, geofence/battery/link checks still hold, and an abort path is wired; every command goes through 01-09 ack/timeout and is audited.
- **Acceptance**:
  - Given an armed, compliant mission in `Simulation` mode, when commands are dispatched, then they execute against the `02` twin with acks and audit.
  - Given a geofence violation mid-dispatch, when the next command is issued, then dispatch halts and an abort is offered — no further command is sent.
- **Tests**: unit (guard preconditions), simulation integration (full path against `02`), failure path (mid-flight violation halts dispatch).
- **Depends on**: 01-09, 01-10, 01-11, 01-16, `02`.

### STORY 01-16 · M4 · M · P0 — Abort and return-to-home
- **Story**: As `OPS`, I want a one-action abort that triggers return-to-home or controlled landing, so that I can recover the aircraft at any time.
- **Safety / deterministic**: deterministic RTH/abort handler reachable from any in-flight state; chooses RTH vs. land-in-place by battery/distance; logs the trigger and outcome.
- **Acceptance**:
  - Given an in-flight mission, when abort is invoked, then the aircraft is commanded to RTH or land and the action is audited.
  - Given RTH triggered with insufficient battery to return, when evaluated, then it falls back to land-in-place at the nearest emergency site rather than attempting an unreachable return.
- **Tests**: unit (RTH-vs-land decision), failure path (insufficient battery → land-in-place), simulation against `02`.
- **Depends on**: 01-09, 01-12, `02`.

### STORY 01-17 · M4 · M · P0 — Automated failsafe on link loss / low battery
- **Story**: As `OPS`, I want the system to auto-trigger failsafe on link loss or critical battery, so that the aircraft is protected even if I am not watching.
- **Safety / deterministic**: deterministic failsafe rules on link-loss (from 01-07/01-08) and battery thresholds (from 01-12) that invoke 01-16; transitions audited; rule configuration inspectable.
- **Acceptance**:
  - Given link loss beyond the threshold, when the rule fires, then failsafe RTH is invoked and audited.
  - Given a misconfigured failsafe (no emergency site set), when arming is attempted, then arming is blocked, not allowed to fly without a failsafe target.
- **Tests**: unit (rule firing), failure path (no failsafe target blocks arm), simulation against `02`.
- **Depends on**: 01-07, 01-08, 01-16.

### STORY 01-18 · M4 · S · P1 — Mission replay and after-action review
- **Story**: As `DSP`, I want to replay a completed mission from its audit log, so that I can review what happened and explain it to a client.
- **Safety / deterministic**: deterministic replay reconstructs the position/telemetry/command timeline from 01-14; exportable.
- **Acceptance**:
  - Given a completed mission, when replayed, then the timeline reproduces persisted positions and commands in order.
  - Given a corrupted/incomplete audit, when replay is attempted, then it reports the gap rather than fabricating a continuous track.
- **Tests**: unit (replay reconstruction), failure path (corrupt audit reported), fixture.
- **Depends on**: 01-14, 01-06.

### STORY 01-19 · M4 · S · P2 — Telemetry and mission export
- **Story**: As `DSP`, I want to export a mission's plan and telemetry, so that clients and downstream tools can use the flight record.
- **Safety / deterministic**: export plan (waypoints, CRS) and telemetry (CSV/GeoJSON track) with correct CRS/extent; validate against a schema.
- **Acceptance**:
  - Given a completed mission, when exported, then the track is valid GeoJSON in the correct CRS and the plan exports as CSV.
  - Given a mission with no telemetry, when exported, then a valid empty export is produced, not an error.
- **Tests**: geospatial round-trip, schema validation, failure path (empty telemetry).
- **Depends on**: 01-06, 01-18, `04` (capture provenance link).

---

## M5 — Autonomous-Assist (gated behind reliable single-drone control and `03`)

### STORY 01-20 · M5 · M · P1 — Approval-gated autonomous mission execution
- **Story**: As `OPS`, I want to launch a validated survey mission autonomously behind an explicit approval gate, so that I can run routine capture without hand-flying every leg.
- **Safety / deterministic**: autonomous execution requires a human approval gate, all M3 safety checks green, and failsafe armed; the operator can abort at any point; disabled by default and simulation-first.
- **Acceptance**:
  - Given a validated mission and operator approval in `Simulation`, when autonomous execution runs, then legs fly against the `02` twin with safety checks live and abort available.
  - Given any safety check turning red mid-flight, when autonomy is running, then it halts and falls back to failsafe — never continuing autonomously through a violation.
- **Tests**: gating test (disabled without approval), simulation integration, failure path (red check halts autonomy).
- **Depends on**: 01-11, 01-15, 01-16, 01-17, `03` (collision safety), `02`.

### STORY 01-21 · M5 · S · P2 — Adaptive re-plan on constraint change
- **Story**: As `OPS`, I want a mission to propose a re-plan when a constraint changes mid-flight (wind, no-fly update), so that the aircraft adapts safely instead of failing the whole flight.
- **Safety / deterministic**: re-plan is a proposal that re-runs all validation/safety checks and requires operator confirmation before applying; bounded and audited.
- **Acceptance**:
  - Given a new no-fly zone mid-flight, when re-plan runs, then a compliant proposal is generated and presented for confirmation.
  - Given no compliant re-plan exists, when re-plan runs, then it returns "no safe re-plan" and triggers failsafe rather than flying an invalid path.
- **Tests**: unit (re-plan validation), failure path (no safe re-plan → failsafe), simulation against `02`.
- **Depends on**: 01-20, 01-13, 01-10.

---

## Coverage note

~21 stories cover the 11 capabilities in `capability-map.md`, ordered by phase and weighted toward M3/M4 safety per `release-plan.md` (P0-heavy, safety pillar dominant). The curated counts in `release-plan.md` (≈78 rows) expand several of these — per-survey-pattern variants, per-MAVLink-message handlers, additional pre-flight checklist items, and weather-source integrations — into sibling stories when implemented. Every dispatch story carries geofence, altitude, battery, and a working abort; no real-flight path ships before passing simulation against domain `02`.
