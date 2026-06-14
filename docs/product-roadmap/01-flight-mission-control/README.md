# Flight and Mission Control

Plan, dispatch, and supervise drone missions via MAVLink with safe guardrails and trustworthy live telemetry.

## Where We Are

- Real mission CRUD, waypoints, optimization, and a PostgreSQL backend exist in `mission_planner`.
- `mission_control` has the async MAVLink/WebSocket/API skeleton with flight/simulation modes.
- Command handling, telemetry persistence, safety checks, and tests are still thin.

## Where We Should Be

- Validated missions linked to field/season, with survey-pattern templates from boundaries.
- Safe dispatch with pre-flight checklist, geofence/altitude/battery enforcement, acks, and failsafe.
- Persisted, fresh live telemetry feeding the ground station (`11`) and capture provenance (`04`).

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Normalize mission identity and link to field/season.
2. Add waypoint validation and one survey-pattern template from a boundary.
3. Persist live telemetry with freshness and gap detection.
4. Add geofence/altitude/battery pre-flight enforcement.
5. Implement MAVLink command ack/retry and failsafe/return-to-home.
6. Add mission audit, replay, and export.

## Primary Crates

`mission_planner`, `mission_control`, with `shared` for config/schemas. Exercised against domain `02` (simulation) before flight hardware.
</content>
