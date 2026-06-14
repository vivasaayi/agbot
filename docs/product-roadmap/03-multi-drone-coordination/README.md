# Multi-Drone Coordination

Coordinate a swarm of drones to cover large fields safely: formations, collision avoidance, mission assignment, and global geofence/altitude/no-fly enforcement.

## Where We Are

- `multi_drone_control` has a strong type model: `MultiDroneController` with a swarm registry and global constraints, `DroneSwarm` with formations (Line/Grid/Circle/V/Custom), and a `CoordinationEngine`.
- Safety primitives exist: ray-casting point-in-polygon geofence and no-fly-zone detection, altitude limits, and `SafetyViolation` tracking.
- Command handling covers `EmergencyLand`, `ReturnToBase`, and `FormSwarm`; `CollisionAvoidanceSystem` does risk assessment and trajectory prediction.
- The hard algorithms are skeletons: formation optimization, coordinated-action execution, collision-maneuver targets, and most assignment strategies are no-op or partial implementations, with almost no test coverage.

## Where We Should Be

- Reliable single-drone flight (`01`) and safety first, then bounded multi-drone coverage of large fields.
- Working collision avoidance that produces and executes maneuver targets, and formation control that holds geometry.
- Mission assignment beyond naive FirstAvailable (load-balanced, priority, auction) with approval-gated execution.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Harden global constraints (geofence/altitude/no-fly) and `SafetyViolation` detection with tests.
2. Complete collision avoidance: compute maneuver target positions and verify separation.
3. Implement one formation that holds geometry (Grid or Line) end to end.
4. Execute one coordinated action (synchronized survey) over a real field boundary.
5. Add a real assignment strategy (load-balanced or priority) beyond FirstAvailable.
6. Gate every coordinated maneuver behind dry-run and operator approval.

## Primary Crates

`multi_drone_control`, with `shared` for schemas and constraints. Depends on single-drone flight (`01`) and is validated against the digital twin (`02`) before any real swarm flight.
