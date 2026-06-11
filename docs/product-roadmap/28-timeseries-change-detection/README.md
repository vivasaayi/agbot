# Time-Series and Change Detection

A dedicated, reusable subsystem that answers "what changed since last flight?" for any field, metric, and date: a generic time-series store plus deterministic, co-registration-gated change detection that many domains plug into.

## Where We Are

- Only thin slivers exist today. Domain `08` has a thin "compare mode (season/product)" capability (side-by-side/swipe of two scenes), and domain `09` has a one-line "trend vs last flight" inside its vegetation summary.
- There is no shared time-series store, no temporal alignment/co-registration step, no raster change detection, and no reusable API any domain can call.
- Two-date comparison anywhere in the platform currently depends on whatever the calling domain happens to do; there is no enforced co-registration guard, so a "change map" can be produced from scenes that are not actually aligned.

## Where We Should Be

- A first-class, general-purpose `timeseries` engine: scalar AND raster series keyed by `(entity, metric, time)` with a reusable API that domains `09`, `15`, `16`, `17`, `19`, `25`, and `27` all plug into instead of each re-implementing trends.
- A deterministic change-detection layer: temporal alignment/co-registration onto a common grid/CRS/resolution, per-pixel and zonal change, threshold change masks, baseline/seasonality, and ranked change events — every output cites its evidence and asserts geospatial correctness.
- A hard guard: no two-date comparison and no change map without a proven co-registration; alignment failure is a clean, tested refusal, never a misaligned result.
- Reusable consumers and exports: time-series CSV, change-mask GeoTIFF, change-zone GeoJSON, and a compare view that feeds domain `08`.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.
- `stories.md`: detailed vertical-slice stories.

## Build Order

1. Generic time-series store: scalar and raster series keyed by `(entity, metric, time)` with a reusable read/write API.
2. Temporal alignment / co-registration of same-field scenes onto a common grid/CRS/resolution, with alignment QA that refuses non-co-registerable pairs.
3. Raster change detection: per-pixel delta, normalized change, and threshold change masks with CRS/extent assertions.
4. Zonal trend analysis: metric trajectory and slope per field/zone, with anomaly vs a seasonal baseline.
5. Change events: detect and rank significant changes with retained evidence and reason codes ("NDVI dropped 0.2 in the NE zone since the last flight").
6. Reusable consumers (`09`/`15`/`16`/`17`/`19`/`25`/`27`) and exports (CSV / change-mask GeoTIFF / change-zone GeoJSON / compare view to `08`).

## Primary Crates

New crate `timeseries` (the generic engine) plus a change-detection module, with `shared` for schemas and CRS/extent types. Builds on `07`/`10` (scene/field history), `05`/`06` (products over time), feeds `08` (compare view), and is consumed by `09`, `15`, `16`, `17`, `19`, `25`, and `27`.
