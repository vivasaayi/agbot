# Time-Series and Change Detection: Current State and Target State

## Mission

Provide a dedicated, reusable time-series and change-detection subsystem so the platform can answer "what changed since last flight?" for any field, metric, and date — deterministically, with proven co-registration and cited evidence — and so every domain that tracks a value over time plugs into one engine instead of re-implementing trends and comparisons.

## Current Maturity

thin partial → promote: no shared subsystem exists. The platform has only two slivers of this behavior — a "compare mode (season/product)" capability in domain `08` (side-by-side/swipe of two scenes) and a one-line "trend vs last flight" delta inside the `09` vegetation summary. Neither is generic, neither enforces co-registration, and neither is callable as a reusable API. This domain promotes those slivers into a first-class `timeseries` engine plus a deterministic change-detection layer.

## What Exists Now

- Domain `08` compare mode (capability, largely unbuilt): the intent to render two comparable scenes of one field in a shared georeferenced view, with a documented refusal when CRS/extent are incompatible (`08-12`). This is the only place a two-date comparison is described, and it is UI-side, not a reusable engine.
- Domain `09` trend line: the vegetation summary computes a delta "vs last flight" only when a comparable prior scene exists, else marks "no baseline" (`09-06`). This is a single hardcoded two-date delta inside the advisor, not a general series.
- Upstream data that a time-series store would key on already exists in shape: `05`/`06` produce georeferenced products (NDVI/index/thermal/elevation) per scene, and `07`/`10` hold the scene/field/season history those products attach to.
- There is no `timeseries` crate, no `(entity, metric, time)` store, no temporal alignment/co-registration step, no raster change detection, no baseline/seasonality, no change-event ranking, and no shared API.

## Gaps to Close

- No generic time-series store for scalar AND raster series keyed by `(entity, metric, time)`; each consumer would otherwise re-invent storage and query.
- No reusable append/query API; `09`/`15`/`16`/`17`/`19`/`25`/`27` have no shared engine to call.
- No temporal alignment / co-registration of same-field scenes across dates onto a common grid/CRS/resolution.
- No alignment QA guard: nothing today refuses a comparison when two scenes are not co-registerable, so a change map can be produced from misaligned inputs.
- No deterministic raster change detection (per-pixel delta, normalized change, threshold change masks) with CRS/extent assertions.
- No zonal trend analysis (metric trajectory, slope/trend per field/zone) or anomaly vs a seasonal baseline.
- No baseline/seasonality model (rolling baseline, season-over-season comparison, phenology curve).
- No change-event detection/ranking with retained evidence and reason codes.
- No exports specific to this subsystem (time-series CSV, change-mask GeoTIFF, change-zone GeoJSON) and no compare-view feed back to `08`.
- No forecast/gap-fill, and no closed-loop hook that turns a significant detected change into an approval-gated re-fly/treatment proposal.

## Related Existing Surfaces

- Domain `08` (geo viewer): the compare mode that consumes an aligned two-date pair and change mask from this engine instead of comparing raw scenes itself.
- Domain `09` (post-flight advisor): the primary repeat-use hook ("what changed since last flight?"); its NDVI/vegetation trend moves onto the shared engine.
- Domains `05`/`06` (imagery / LiDAR): the georeferenced products over time that become raster series.
- Domains `07`/`10` (GIS hub / field-farm-data): scene/field/season history that supplies the `entity` and time keys.
- Domains `15`/`16`/`17`/`19`/`25`/`27` (weather, water, drought, carbon, fleet health, soil/IoT): scalar-series consumers (weather series, water-balance series, drought-index series, carbon stock over time, telemetry health trend / RUL, soil/IoT sensor reading series).
- Domains `01`/`14` (flight/mission, autonomous tractor): the closed-loop targets a significant change auto-proposes a mission against (approval-gated).

## Target Operating Model

- One generic engine: scalar and raster series live in a single `timeseries` store keyed by `(entity, metric, time)`, with a reusable append/query API every domain plugs into; no consumer re-implements trends.
- Geospatial correctness is non-negotiable: any two-date comparison first asserts CRS, extent, and resolution and proves co-registration onto a common grid; an alignment failure is a clean, tested refusal — no change map without alignment proof.
- Deterministic before any forecast: per-pixel delta, normalized change, threshold masks, zonal slope, baseline/seasonality, and ranked change events are all computed and inspectable without AI, each citing its evidence layer and reason codes.
- Repeat-use hook: "NDVI dropped 0.2 in the NE zone since the last flight" is produced as a ranked change event with retained evidence, and surfaced to `09` and `08`.
- Reusability is the central theme: the same engine serves vegetation trend (`09`), weather series (`15`), water-balance series (`16`), drought-index series (`17`), carbon stock (`19`), telemetry health/RUL (`25`), and soil/IoT readings (`27`).
- Exports and compare: time-series CSV, change-mask GeoTIFF, and change-zone GeoJSON round-trip with correct CRS; the compare view feeds `08`.
- Bounded autonomy last: forecast/gap-fill carries an uncertainty band, and a significant detected change can auto-propose an approval-gated re-fly/treatment mission (`09`→`01`/`14`) — never executed without human approval.
