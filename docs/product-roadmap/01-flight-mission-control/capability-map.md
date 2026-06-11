# Flight and Mission Control: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety, geospatial correctness, data quality, operability, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Flight and Mission Control Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Mission CRUD and persistence | strong partial | 8 | Normalize mission identity and link to field/season |
| Waypoint and flight-path model | strong partial | 7 | Validate waypoint sanity and altitude/geofence bounds |
| Survey-pattern templates (grid, lawnmower, perimeter) | missing | 9 | Generate a coverage mission from a field boundary |
| Mission optimization and path planning | medium partial | 6 | Deterministic path cost with battery/time budget |
| MAVLink command interface | early partial | 10 | Command ack/timeout/retry with link health |
| Arming and pre-flight checklist | missing | 7 | Block arm until geofence/battery/GPS checks pass |
| Live telemetry streaming and history | early partial | 8 | Persist telemetry with freshness and gap detection |
| Geofence and no-fly enforcement | partial (shared with `03`) | 7 | Reject dispatch outside geofence/no-fly zone |
| Failsafe and return-to-home | missing | 6 | RTH on link loss / low battery with audit |
| Weather and airspace constraints | early partial | 5 | Flag wind/precip thresholds before dispatch |
| Mission audit and replay | missing | 5 | Persist command/telemetry log for after-action review |
</content>
