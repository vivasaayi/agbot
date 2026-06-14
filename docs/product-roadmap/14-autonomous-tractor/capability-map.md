# Autonomous Tractor: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety first, then geospatial correctness, data quality, agronomic value, operability, explainability and trust) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. The safety pillar dominates: a ground vehicle moves among people and equipment. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Autonomous Tractor Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Tractor vehicle identity and registry | missing (greenfield) | 7 | Register a tractor + implement linked to org/field via `10` |
| GPS/RTK guidance and path following | missing (greenfield) | 9 | Follow a straight path with bounded cross-track error (sim) |
| Coverage / path planning from a boundary | missing (greenfield) | 8 | Generate a swath plan from a field boundary (`07`/`10`) |
| Implement control (planter/sprayer/tiller) | missing (greenfield) | 8 | Abstract one implement with on/off + rate setpoint |
| Geofence and boundary enforcement | missing (greenfield) | 7 | Reject motion outside the field geofence |
| E-stop and operator approval | missing (greenfield) | 7 | Hardware/soft e-stop halts motion immediately |
| Obstacle detection | missing (greenfield) | 8 | Stop on a detected obstacle in the path (sim sensor) |
| Prescription-map execution (consumes `09`/`05`) | missing (greenfield) | 8 | Execute per-zone rate from a management-zone map |
| Field-ops session logging and telemetry | missing (greenfield) | 7 | Log a session with telemetry and coverage (-> `04`/`10`) |
| Multi-vehicle coordination (parallels `03`) | missing (greenfield) | 6 | Deconflict two tractors sharing a field boundary |
| After-action replay and audit | missing (greenfield) | 5 | Replay a session's path, telemetry, and safety events |
| Weather/operational window gating (via `15`) | missing (greenfield) | 4 | Block ops outside a `15` spray/field window |
