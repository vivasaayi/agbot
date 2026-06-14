# Multi-Drone Coordination: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety, geospatial correctness, data quality, performance and scale, operability, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Multi-Drone Coordination Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Swarm registry and lifecycle | strong partial | 7 | Register/remove swarms with linked drone identity |
| Global constraints (geofence/altitude/no-fly) | strong partial | 8 | Reject any swarm action outside geofence/no-fly |
| Safety violation detection and audit | medium partial | 7 | Raise and persist `SafetyViolation` with severity |
| Formation definition (Line/Grid/Circle/V) | partial (types only) | 7 | One formation that holds geometry end to end |
| Formation optimization | missing (current no-op scaffold) | 6 | Deterministic slot assignment for a Grid formation |
| Collision risk assessment | medium partial | 8 | Predict trajectories and flag separation breaches |
| Collision-avoidance maneuvers | early partial (target stubbed) | 8 | Compute maneuver target and verify separation |
| Coordinated actions (survey/coverage) | early partial (no-op) | 8 | Execute a synchronized survey over a boundary |
| Mission assignment strategies | medium partial (FirstAvailable/BestFit) | 7 | Add load-balanced or priority assignment |
| Swarm command handling (EmergencyLand/RTB/FormSwarm) | medium partial | 6 | Dry-run and audit each coordinated command |
| Inter-drone communication and heartbeat | early partial | 5 | Track link quality and trigger comm-loss rules |
| Approval-gated coordinated execution | missing | 6 | Require operator confirm before any maneuver |
