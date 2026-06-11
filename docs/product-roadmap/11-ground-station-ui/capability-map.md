# Ground Station UI: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (operability, safety, data quality, geospatial correctness, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Ground Station UI Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| WebSocket client and message dispatch | strong partial | 6 | Reconnect/backoff with connection-state surfaced to UI |
| Live telemetry display and binding | early partial | 8 | Bind dashboard tiles to live telemetry with freshness |
| Connection and link-health indicators | missing | 5 | Show connected/stale/lost with last-update age |
| Map rendering (basemap, position, path) | missing (static map scaffolds only) | 9 | Render a basemap with drone position and flight path |
| Mission overlay (waypoints, geofence, no-fly) | missing | 7 | Draw mission waypoints and geofence on the map |
| Capture event timeline (LiDAR/image/NDVI) | early partial | 6 | Collect capture events into an ordered, filterable feed |
| System alerts and status panel | early partial | 5 | Severity-ranked alert list from system-status events |
| Operator actions (dispatch, pause, RTH, abort) | missing | 9 | Send a guarded abort/RTH back to `mission_control` |
| Operator auth and session | missing | 6 | Login/session gate before any action route |
| Operator action audit | missing | 5 | Log who did what, when, with the resulting ack |
| CLI operations console | early partial | 5 | Real `status` from live link/telemetry state |
| Web/CLI surface parity | early partial | 4 | Share one client state model across both surfaces |
