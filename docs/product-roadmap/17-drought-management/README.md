# Drought Management

Predict, monitor, and mitigate drought: fuse satellite and weather data into deterministic drought indices and vegetation-stress evidence, raise early warnings, and recommend adaptive strategies, with the evidence always shown before any AI forecast.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#12 Drought Management); no code exists.
- The platform spine it consumes is partially real: vegetation-stress indices come from imagery and remote sensing (`05`), satellite/GIS data and the Landsat client come from the GIS hub (`07`), weather models come from weather advisory (`15`), mitigation runs through irrigation (`16`) and the advisor (`09`), and field/region identity comes from the field/farm spine (`10`).
- This is an explainability-first module: deterministic drought indices and stress evidence must be inspectable before any AI drought prediction.

## Where We Should Be

- A field and region carry deterministic drought indices (SPI/SPEI-style) and vegetation-stress evidence (from `05`), each dated, located, and traceable to its inputs.
- Satellite (`07` Landsat) and weather (`15`) data are fused into a per-field/region drought risk score with historical baselines and seasonal trends.
- Early warnings and alerts fire on threshold crossings and route to the portal (`13`) and operator surfaces (`11`).
- Mitigation strategy recommendations tie to real field actions, primarily irrigation (`16`) and advisor guidance (`09`), with evidence cited and uncertainty flagged.

## Where We Should Be Careful

- Evidence before advice: a deterministic stress/drought index, with its inputs, must exist and be inspectable before any AI forecast is shown. A wrong drought call drives costly, hard-to-reverse farm decisions.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Drought-index data model: SPI/SPEI-style indices and vegetation-stress evidence linked to field/region via `10`/`07`.
2. Satellite + weather data fusion: ingest Landsat (`07`) and weather (`15`) into a common, dated, georeferenced store.
3. Historical baselines and seasonal trend computation.
4. Per-field/region deterministic risk scoring with evidence.
5. Early-warning and alerting on threshold crossings, routed to `13`/`11`.
6. Mitigation strategy recommendations tied to `16` (irrigation) and `09` (advisor); reporting.

## Primary Crates

New crate(s) TBD (a drought-index and risk-scoring engine plus a fused satellite/weather store). Builds on domains `05` (stress indices), `07` (satellite/GIS, Landsat), `15` (weather), `16` (mitigation via irrigation), `09` (advisor), and `10` (field/region identity).
