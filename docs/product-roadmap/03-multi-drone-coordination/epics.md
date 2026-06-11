# Multi-Drone Coordination: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: `ControlCommand` or service call, persistence, and audit events.
- Safety: global geofence, altitude, no-fly, battery, and minimum-separation checks before any coordinated mutation, plus abort.
- Deterministic: formation geometry, collision prediction, and assignment logic that run without AI.
- Telemetry: per-drone state and swarm status with link quality and `SafetyViolation`s.
- UI: swarm view, formation preview, and coordinated-action dispatch (consumed by domain `11`).
- Tests: unit (geometry/prediction/assignment math), fixture (zone polygons, trajectories), and one failure path (separation breach / geofence exit).
- Operations: runtime mode (`Simulation` against `02` first), health, and a runbook.

## Category Epics

### EPIC-01: Swarm Safety and Constraints
- Goal: a swarm that cannot violate geofence, altitude, no-fly, or separation limits without an audited violation and abort.
- First release: harden global-constraint checks and `SafetyViolation` detection with tests.
- Expansion: minimum-separation enforcement and comm-loss/low-battery rule actions.
- Hardening: full audit trail and approval gate on every coordinated command.

### EPIC-02: Collision Avoidance and Formations
- Goal: drones that maintain separation and hold formation geometry under drift.
- First release: compute collision-maneuver target positions and verify separation after the maneuver.
- Expansion: one formation (Grid or Line) that holds geometry end to end in the `02` twin.
- Hardening: formation optimization (slot assignment) and multi-conflict resolution.

### EPIC-03: Coordinated Coverage and Assignment
- Goal: a swarm that covers a real field boundary efficiently with sensible task allocation.
- First release: execute a synchronized survey over a boundary from domain `10`.
- Expansion: a non-trivial assignment strategy (load-balanced or priority) with role/workload accounting.
- Hardening: coverage optimization, dry-run preview, and export of the coordinated plan.
