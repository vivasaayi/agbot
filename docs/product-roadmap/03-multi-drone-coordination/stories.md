# Multi-Drone Coordination: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Safety / deterministic**: what must be enforced or computed without AI, with a working abort and minimum-separation guarantee.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `OPS` operator/pilot, `DSP` drone service provider, `AG` agronomist, `PA` platform admin.

This is Phase 4, safety-critical, and gated: it depends on reliable single-drone flight (`01`) and is validated in the `02` twin before any real swarm flight. Every coordinated maneuver verifies minimum separation before and after, enforces global geofence/altitude/no-fly/battery, and is approval-gated. No autonomous swarm maneuver executes without an operator gate in v1.

---

## M1 — Foundation

### STORY 03-01 · M1 · S · P0 — Swarm registry and lifecycle
- **Story**: As `OPS`, I want to register and remove swarms with linked drone identity, so that I always know which aircraft are under coordinated control.
- **Safety / deterministic**: `MultiDroneController` registry persists `{swarm_id, drone_ids[], owner, status}`; lifecycle transitions are deterministic; a drone cannot be in two active swarms.
- **Acceptance**:
  - Given drones, when a swarm is formed, then it is registered with linked drone identity and a status.
  - Given a drone already in an active swarm, when it is added to a second, then the operation is rejected with a conflict error.
- **Tests**: unit (registry + lifecycle), API contract (register/remove/list), failure path (double-membership rejected).
- **Depends on**: `multi_drone_control/src/lib.rs`, `01` (drone identity).

### STORY 03-02 · M1 · S · P1 — Global constraints model and persistence
- **Story**: As `PA`, I want global constraints (max altitude, geofence polygon, no-fly zones, max concurrent drones, emergency sites) defined and persisted per swarm, so that every coordinated action has explicit bounds.
- **Safety / deterministic**: persist `GlobalConstraints` and validate them (non-empty geofence, ≥1 emergency site, sane altitude) on save; reject incomplete constraint sets.
- **Acceptance**:
  - Given a complete constraint set, when saved, then it persists and is retrievable per swarm.
  - Given a constraint set with no emergency landing site, when saved, then it is rejected — a swarm cannot operate without a failsafe target.
- **Tests**: unit (constraint validation), API contract, failure path (no emergency site rejected).
- **Depends on**: 03-01.

### STORY 03-03 · M1 · S · P1 — Formation definition (Line/Grid/Circle/V)
- **Story**: As `OPS`, I want to define a formation type with slot geometry, so that I can describe how the swarm should be arranged.
- **Safety / deterministic**: deterministic geometry generator per `FormationType` producing slot offsets from a leader; assert slots respect a minimum inter-slot spacing.
- **Acceptance**:
  - Given a Grid formation and N drones, when geometry is generated, then N non-overlapping slots are produced with spacing ≥ the minimum.
  - Given a spacing below the minimum separation, when geometry is generated, then it is rejected, not allowed to define an unsafe formation.
- **Tests**: unit (slot geometry per type), failure path (sub-minimum spacing rejected), fixture.
- **Depends on**: 03-01, `multi_drone_control/src/swarm.rs`.

---

## M2 — Captured / Observable

### STORY 03-04 · M2 · S · P1 — Per-drone state tracking and swarm telemetry
- **Story**: As `OPS`, I want each swarm drone's position/battery/mode tracked with freshness, so that I can supervise the whole swarm in one view.
- **Safety / deterministic**: maintain `DroneState` per drone with freshness; mark a drone `Stale` when its telemetry ages out; aggregate into swarm status.
- **Acceptance**:
  - Given a steady swarm, when states update, then each drone reads `Fresh` and the swarm status aggregates them.
  - Given one drone's telemetry stopping, when freshness is computed, then it reads `Stale` and the swarm status reflects degradation.
- **Tests**: unit (per-drone freshness + aggregation), failure path (one drone stale), fixture.
- **Depends on**: 03-01, `01` (telemetry), `multi_drone_control/src/coordination.rs`.

### STORY 03-05 · M2 · S · P1 — Inter-drone link quality and heartbeat
- **Story**: As `OPS`, I want link quality and heartbeats tracked between the controller and each drone, so that comm loss is detected and acted on.
- **Safety / deterministic**: track heartbeat interval and link quality per drone; trigger the comm-loss `CoordinationRule` deterministically on timeout.
- **Acceptance**:
  - Given regular heartbeats, when link is tracked, then quality reads healthy and no rule fires.
  - Given a heartbeat timeout, when tracked, then the comm-loss rule fires and is audited, not ignored.
- **Tests**: unit (heartbeat timeout), failure path (timeout fires comm-loss rule), fixture.
- **Depends on**: 03-04.

---

## M3 — Explainable

### STORY 03-06 · M3 · M · P0 — Global constraint enforcement on swarm actions
- **Story**: As `OPS`, I want any swarm action outside the geofence, altitude ceiling, or a no-fly zone rejected, so that no drone in the swarm leaves safe bounds.
- **Safety / deterministic**: re-run ray-casting point-in-polygon geofence/no-fly and altitude checks for every drone's target before any coordinated action; reject the whole action on any violation.
- **Acceptance**:
  - Given all targets inside bounds, when a coordinated action is requested, then the constraint check passes.
  - Given one drone's target inside a no-fly zone, when the action is requested, then the entire action is rejected and a `SafetyViolation` is raised — not partially executed.
- **Tests**: unit (per-drone geofence/no-fly/altitude), failure path (one target in no-fly blocks whole action), geospatial round-trip.
- **Depends on**: 03-02, `01` (shared geofence primitives), `multi_drone_control/src/lib.rs`.

### STORY 03-07 · M3 · S · P0 — Safety violation detection and audit
- **Story**: As `DSP`, I want every safety violation raised with type, severity, and context and persisted, so that swarm incidents are explainable after the fact.
- **Safety / deterministic**: raise `SafetyViolation` across the six violation types and four severities; persist `{type, severity, drone_id, position, timestamp, action_ref}`; append-only.
- **Acceptance**:
  - Given a geofence breach, when detected, then a `SafetyViolation` is persisted with type/severity/context.
  - Given a violation that should be audited but is dropped, when the audit-completeness check runs, then the gap is detected and reported.
- **Tests**: unit (violation classification), failure path (audit gap detected), fixture.
- **Depends on**: 03-06.

### STORY 03-08 · M3 · L · P0 — Collision-avoidance maneuver target and separation verification
- **Story**: As `OPS`, I want collision avoidance to compute an actual maneuver target and verify minimum separation, so that predicted conflicts are resolved, not just flagged.
- **Safety / deterministic**: replace the `target_position: None` maneuver gap — predict trajectories, select a maneuver (altitude/horizontal/speed/stop/RTB/hover), compute the target, and verify minimum separation holds both before and after the maneuver.
- **Acceptance**:
  - Given two converging trajectories, when avoidance runs, then a maneuver target is computed and post-maneuver separation is ≥ the minimum.
  - Given a conflict with no maneuver that preserves separation, when avoidance runs, then it escalates to emergency stop/hover rather than returning a target that still breaches separation.
- **Tests**: unit (trajectory prediction + maneuver math), failure path (no safe maneuver → emergency stop), simulation against `02`.
- **Depends on**: 03-04, `02` (twin validation), `multi_drone_control/src/collision_avoidance.rs`.

### STORY 03-09 · M3 · M · P0 — Collision risk assessment from trajectory prediction
- **Story**: As `OPS`, I want pairwise separation breaches predicted from trajectories, so that conflicts are caught early.
- **Safety / deterministic**: 3D trajectory prediction over a horizon; classify risk by distance threshold; emit predicted conflict pairs with time-to-conflict.
- **Acceptance**:
  - Given two drones on a converging path, when risk is assessed, then a conflict pair with time-to-conflict is produced before the breach.
  - Given diverging paths, when risk is assessed, then no conflict is flagged (no false positives).
- **Tests**: unit (prediction + thresholds), failure path (diverging paths → no flag), fixture.
- **Depends on**: 03-04, `multi_drone_control/src/collision_avoidance.rs`.

### STORY 03-10 · M3 · M · P1 — Formation optimization (deterministic slot assignment)
- **Story**: As `OPS`, I want drones assigned to formation slots optimally and deterministically, so that the formation holds without crossing paths.
- **Safety / deterministic**: replace the `optimize_formations()` no-op with deterministic slot assignment (e.g. minimize total travel without path crossings); validate no two assigned paths breach separation.
- **Acceptance**:
  - Given N drones and a Grid formation, when optimization runs, then each drone is assigned one slot with non-crossing, separation-respecting paths.
  - Given an assignment that would cause two drones to swap through the same point, when validated, then it is rejected/re-solved, not executed.
- **Tests**: unit (assignment optimality + crossing check), failure path (crossing assignment rejected), simulation against `02`.
- **Depends on**: 03-03, 03-08, `multi_drone_control/src/coordination.rs`.

### STORY 03-11 · M3 · M · P1 — Mission assignment strategies (load-balanced / priority)
- **Story**: As `DSP`, I want at least one non-trivial assignment strategy beyond FirstAvailable/BestFit, so that work is distributed sensibly across the swarm.
- **Safety / deterministic**: implement LoadBalanced or PriorityBased assignment (currently returns `Ok(None)`); produce a deterministic `DroneAssignment` with workload accounting.
- **Acceptance**:
  - Given uneven workloads, when load-balanced assignment runs, then missions are distributed to minimize peak workload and the result is deterministic.
  - Given more missions than capacity, when assignment runs, then it returns an explicit "unassignable" set rather than silently dropping missions.
- **Tests**: unit (assignment + workload accounting), failure path (over-capacity surfaces unassignable), fixture.
- **Depends on**: 03-01, `multi_drone_control/src/mission_assignment.rs`.

---

## M4 — Interactive

### STORY 03-12 · M4 · L · P0 — Coordinated synchronized survey over a boundary
- **Story**: As `AG`, I want a swarm to execute a synchronized survey over a field boundary, so that a large field is covered faster than one drone could.
- **Safety / deterministic**: execute the currently no-op coordinated action — partition the `10` boundary into per-drone coverage lanes, run them synchronized with live separation checks (03-08/03-09) and global constraints (03-06); abort on any violation.
- **Acceptance**:
  - Given a field boundary and a swarm, when a synchronized survey runs in `Simulation`, then lanes cover the field with no separation breach and reported coverage.
  - Given a separation breach mid-survey, when detected, then the survey halts and collision avoidance/abort engages — no lane continues through a breach.
- **Tests**: integration (coverage partition + sync), simulation against `02`, failure path (mid-survey breach halts).
- **Depends on**: 03-06, 03-08, 03-09, 03-13, `01` (single-drone flight), `02`, `10` (boundary).

### STORY 03-13 · M4 · M · P0 — Approval-gated coordinated execution
- **Story**: As `OPS`, I want every coordinated maneuver to require explicit operator confirmation, so that no swarm action runs autonomously in v1.
- **Safety / deterministic**: a dry-run produces the planned maneuver with predicted separation; execution is blocked until an operator approves; the gate decision is audited.
- **Acceptance**:
  - Given a planned maneuver, when dry-run completes, then it shows predicted separation and waits for approval.
  - Given no operator approval, when execution is attempted, then it is blocked and audited — it cannot proceed unattended.
- **Tests**: unit (gate enforcement), failure path (execution without approval blocked), API contract (dry-run/approve).
- **Depends on**: 03-08, 03-14.

### STORY 03-14 · M4 · M · P0 — Swarm command handling (EmergencyLand/RTB/FormSwarm) with audit
- **Story**: As `OPS`, I want each swarm command dry-run and audited, so that EmergencyLand, ReturnToBase, and FormSwarm behave predictably and are recoverable.
- **Safety / deterministic**: process `ControlCommand` variants through the mpsc queue with a dry-run, constraint re-check, and audit; EmergencyLand routes each drone to its nearest emergency site.
- **Acceptance**:
  - Given an EmergencyLand command, when issued, then every drone is routed to a valid emergency site and the action is audited.
  - Given a FormSwarm that violates separation, when dry-run, then it is rejected before execution, not attempted.
- **Tests**: unit (command handling + audit), failure path (unsafe FormSwarm rejected), simulation against `02`.
- **Depends on**: 03-02, 03-06, 03-08, `multi_drone_control/src/lib.rs`.

### STORY 03-15 · M4 · M · P1 — Coverage-optimization coordinated action
- **Story**: As `AG`, I want the swarm to optimize coverage of a field (overlap, lane count) for the number of available drones, so that capture is efficient.
- **Safety / deterministic**: deterministic coverage planner produces per-drone lanes minimizing total time given overlap requirements and drone count; validate full-field coverage.
- **Acceptance**:
  - Given a boundary and 3 drones, when coverage is optimized, then lanes fully cover the field with the required overlap and balanced per-drone time.
  - Given fewer drones than required for the overlap in one pass, when planned, then it produces a multi-pass plan rather than reporting false full coverage.
- **Tests**: unit (coverage + overlap math), failure path (insufficient drones → multi-pass), fixture.
- **Depends on**: 03-12, `10` (boundary).

### STORY 03-16 · M4 · S · P2 — Comm-loss and low-battery coordination rules execution
- **Story**: As `OPS`, I want coordination rules (comm loss, low battery, proximity) to actually execute their actions, so that the swarm responds to conditions instead of merely detecting them.
- **Safety / deterministic**: wire the currently unexecuted rule actions to deterministic responses — comm-loss → RTB, low-battery → land at nearest site, proximity → avoidance; priority-ordered and audited.
- **Acceptance**:
  - Given a low-battery condition, when its rule fires, then the affected drone lands at the nearest emergency site and the action is audited.
  - Given two rules firing at once, when resolved, then the higher-priority safety action wins deterministically, not an arbitrary one.
- **Tests**: unit (rule action execution + priority), failure path (conflicting rules resolved by priority), simulation against `02`.
- **Depends on**: 03-05, 03-08, `multi_drone_control/src/coordination.rs`.

---

## M5 — Autonomous-Assist (gated behind reliable `01`/`02`)

### STORY 03-17 · M5 · M · P1 — Bounded autonomous swarm survey behind approval gate
- **Story**: As `OPS`, I want to run a routine coordinated survey autonomously behind a single approval gate, so that multi-drone capture scales without hand-flying every maneuver.
- **Safety / deterministic**: autonomy requires the approval gate (03-13), all M3 safety green, failsafe armed per drone, and continuous separation verification; operator can abort the whole swarm at once; disabled by default, simulation-first.
- **Acceptance**:
  - Given an approved survey in `Simulation`, when autonomy runs, then the swarm covers the field with continuous separation and an always-available swarm abort.
  - Given any drone's safety check turning red, when autonomy is running, then the swarm halts and engages failsafe — never continuing autonomously through a violation.
- **Tests**: gating test (disabled without approval), simulation integration against `02`, failure path (red check halts swarm).
- **Depends on**: 03-12, 03-13, 03-14, 03-16, `01` (reliable single-drone autonomy), `02`.

### STORY 03-18 · M5 · S · P2 — Adaptive swarm re-tasking on drone dropout
- **Story**: As `OPS`, I want the swarm to propose re-tasking remaining drones when one drops out, so that a flight completes safely instead of aborting entirely.
- **Safety / deterministic**: on dropout, re-run assignment (03-11) and coverage (03-15) for the remaining drones as a proposal requiring operator confirmation; re-verify separation; audited.
- **Acceptance**:
  - Given a drone dropout mid-survey, when re-tasking runs, then a separation-valid proposal for the remaining drones is presented for confirmation.
  - Given no safe re-tasking exists, when computed, then the swarm completes a safe abort rather than flying an invalid plan.
- **Tests**: unit (re-task assignment + separation), failure path (no safe re-task → abort), simulation against `02`.
- **Depends on**: 03-11, 03-15, 03-17.

---

## Coverage note

~18 stories cover the 12 capabilities in `capability-map.md`, ordered by phase and weighted toward M3/M4 safety per `release-plan.md` (P0-heavy, safety pillar dominant). The curated counts in `release-plan.md` (≈86 rows) expand several of these — per-formation-type geometry, per-maneuver-type avoidance, additional assignment strategies, and per-rule action handlers — into sibling stories when implemented. Every coordinated maneuver verifies minimum separation before and after, enforces global geofence/altitude/no-fly/battery, is approval-gated, and is validated in the `02` twin before any real swarm flight — and this whole domain is gated behind reliable single-drone flight in `01`.
