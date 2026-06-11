# Weather Advisory System

Hyper-local weather forecasting and advisory per field: spray/flight windows, frost/heat/wind/precip risk alerts, and crop-stage-aware recommendations, feeding flight (`01`), tractor (`14`), irrigation (`16`), and drought (`17`).

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#10 Weather Advisory System); no code exists.
- The field identity it keys on is partially real (`10`), and the operational consumers it feeds are mission control (`01`), the tractor (`14`), irrigation (`16`), and drought (`17`).
- Because advice gates real field actions, the data-quality and explainability pillars dominate: every forecast must carry source, freshness, and provenance.

## Where We Should Be

- Weather data is ingested from forecast APIs and on-field sensors, normalized, and tracked for freshness and provenance.
- Each field has a hyper-local forecast keyed on its boundary/identity from `10`.
- A spray/flight window advisor produces operational windows that feed `01` flight constraints and `14` tractor ops.
- Frost, heat, wind, and precipitation risk alerts are crop-stage-aware and routed to the operator console (`11`) and farmers portal (`13`).
- Growing-degree-day and evapotranspiration inputs feed irrigation (`16`) and drought (`17`).

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Weather data ingestion (forecast APIs + on-field sensors) with provenance/freshness.
2. Per-field hyper-local forecast keyed on `10` field identity.
3. Spray/flight window advisor feeding `01` and `14`.
4. Frost/heat/wind/precip risk alerts, crop-stage-aware.
5. GDD + ET inputs feeding `16` and `17`.
6. Historical weather per field and alert routing to `11`/`13`.

## Primary Crates

Planned `weather_advisory` crate (a weather ingestion/forecast service plus an advisory/alerting engine). Builds on domain `10` (field identity); feeds `01`/`14` (operational windows), `16`/`17` (GDD/ET inputs), and `11`/`13` (alerting).
