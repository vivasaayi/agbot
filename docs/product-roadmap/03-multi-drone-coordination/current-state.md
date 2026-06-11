# Multi-Drone Coordination: Current State and Target State

## Mission

Coordinate multiple drones to cover large fields faster than one aircraft can, while guaranteeing separation, geofence, altitude, and no-fly-zone safety across the whole swarm — with every coordinated maneuver dry-run and approval-gated.

## Current Maturity

early partial: `multi_drone_control` has a strong type model, swarm registry, global constraints, and working point-in-polygon geofence/no-fly detection; but formation optimization, coordinated-action execution, collision-maneuver targets, and most assignment strategies are TODO stubs with almost no test coverage.

## What Exists Now

- `MultiDroneController` with a `HashMap<Uuid, DroneSwarm>` registry and `GlobalConstraints` (max altitude, geofence polygon, no-fly zones, max concurrent drones, emergency landing sites) (`multi_drone_control/src/lib.rs`).
- Ray-casting point-in-polygon geofence and no-fly-zone detection with altitude restrictions, plus `SafetyViolation` tracking across six violation types and four severities (`multi_drone_control/src/lib.rs`).
- `MultiDroneControlService` wrapping the coordination, assignment, and collision-avoidance engines with an mpsc command queue and `ControlCommand` handling: `AssignMission`, `FormSwarm`, `ExecuteCoordinatedAction`, `EmergencyLand`, `ReturnToBase`, `UpdateConstraints`.
- `DroneSwarm` and `SwarmController` with formation types (Line/Grid/Circle/V/Custom), leader assignment, swarm status machine, and a broadcast `SwarmMessage` channel (`multi_drone_control/src/swarm.rs`).
- `CoordinationEngine` with per-drone `DroneState`, priority-based `CoordinationRule`s (proximity, low battery, comm loss), and emergency handling (`multi_drone_control/src/coordination.rs`).
- `CollisionAvoidanceSystem` with 3D tracking, trajectory prediction, distance-threshold risk levels, and maneuver-type selection (altitude change, horizontal deviation, speed reduction, emergency stop, RTB, hover) (`multi_drone_control/src/collision_avoidance.rs`).
- `MissionAssignmentEngine` with a working FirstAvailable and BestFit (fitness-score) strategy (`multi_drone_control/src/mission_assignment.rs`).

## Gaps to Close

- Formation optimization is a no-op: `optimize_formations()` and `execute_action()` return `Ok(())` with TODOs (`coordination.rs`).
- Coordinated-action execution (synchronized survey, pattern search, coverage optimization) is defined but not executed.
- Collision avoidance computes risk but never sets a maneuver target — `target_position: None // TODO` (`collision_avoidance.rs`).
- Assignment beyond FirstAvailable/BestFit is stubbed: LoadBalanced, PriorityBased, and Auction return `Ok(None)` (`mission_assignment.rs`).
- Coordination rules detect conditions (weather, custom) but rule actions are not executed (TODOs in `coordination.rs`).
- Almost no test coverage: a handful of construction/registration smoke tests, none for geofence violations, formation geometry, assignment, or maneuver planning.

## Source Modules Reviewed

- `multi_drone_control/src/lib.rs` (`MultiDroneController`, `GlobalConstraints`, `NoFlyZone`, `ControlCommand`, `Formation`, `CoordinatedAction`, `SafetyViolation`, point-in-polygon)
- `multi_drone_control/src/swarm.rs` (`DroneSwarm`, `SwarmController`, `FormationType`, `SwarmMessage`)
- `multi_drone_control/src/coordination.rs` (`CoordinationEngine`, `CoordinationRule`, `DroneState`, stubs)
- `multi_drone_control/src/collision_avoidance.rs` (`CollisionAvoidanceSystem`, `AvoidanceManeuver`, `ManeuverType`)
- `multi_drone_control/src/mission_assignment.rs` (`MissionAssignmentEngine`, `AssignmentAlgorithm`, `DroneAssignment`)

## Target Operating Model

- Global safety enforced across the whole swarm: geofence, altitude ceiling, no-fly zones, and battery, with `SafetyViolation`s raised and audited.
- Collision avoidance that predicts trajectories, computes maneuver targets, and verifies minimum separation before and after execution.
- Formations that hold geometry under wind/drift, validated in the `02` twin before real flight.
- Coordinated actions (synchronized survey, coverage optimization) that actually execute over a real field boundary from domain `10`.
- Mission assignment with at least one non-trivial strategy (load-balanced or priority) and clear role/workload accounting.
- Every coordinated maneuver dry-run and approval-gated; no autonomous swarm action without a human gate in v1.
