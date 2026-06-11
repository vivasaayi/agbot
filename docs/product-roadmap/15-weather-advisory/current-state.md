# Weather Advisory System: Current State and Target State

## Mission

Turn weather into trustworthy field decisions: ingest forecast and on-field sensor data, produce a hyper-local per-field forecast with provenance and freshness, and derive operational windows, risk alerts, and crop-stage-aware recommendations that gate flight (`01`), tractor ops (`14`), irrigation (`16`), and drought response (`17`).

## Current Maturity

greenfield pending (M0 named): no implementation exists; this domain is a product-vision module from `product-summary.md` (#10 Weather Advisory System). Nothing in the repository implements weather ingestion, a per-field forecast, a window advisor, or risk alerting. Note that domain `01` references a scaffolded, minimal weather/airspace constraint hook, but no weather service backs it.

## What Exists Now

- Nothing is built for this domain. There is no weather crate, ingestion adapter, forecast model, or advisory/alerting engine.
- Adjacent surfaces it would build on and feed (already partially real):
  - Domain `10` (field-farm-data): field identity and boundaries the per-field forecast keys on. Itself greenfield-pending, so this domain is gated on it.
  - Domain `01` (flight and mission control): the scaffolded weather/airspace constraint hook this domain would supply; flight windows feed dispatch gating.
  - Domain `14` (autonomous tractor): ground-ops windows consume the same spray/field-window advisor.
  - Domains `16`/`17` (water/drought, greenfield): downstream consumers of growing-degree-day and evapotranspiration inputs.
  - Domains `11`/`13` (ground station / farmers portal): alert routing destinations.

## Gaps to Close

- No weather data ingestion from forecast APIs or on-field sensors, and no normalization to a common model.
- No data provenance or freshness tracking, despite advice gating real field actions.
- No hyper-local per-field forecast keyed on `10` field identity.
- No spray/flight window advisor producing operational windows for `01`/`14`.
- No frost/heat/wind/precip risk alerting, and none crop-stage-aware.
- No growing-degree-day or evapotranspiration computation to feed `16`/`17`.
- No historical weather store per field for trend and after-action use.
- No alert routing to the operator console (`11`) or farmers portal (`13`).

## Related Existing Surfaces

- Domain `10` (field-farm-data): field identity/boundaries the forecast keys on.
- Domain `01` (flight and mission control): the weather/airspace constraint hook this domain supplies; flight-window consumer.
- Domain `14` (autonomous tractor): ground-ops window consumer.
- Domains `16`/`17` (water/drought): GDD/ET input consumers.
- Domains `11`/`13` (ground station / farmers portal): alert routing destinations.
- `docs/reference/product-summary.md` (#10 Weather Advisory System): the source description for this module.

## Target Operating Model

- Weather is ingested from forecast APIs and on-field sensors, normalized to one model, with source, freshness, and provenance asserted on every value — the data-quality pillar leads.
- Each field has a hyper-local forecast keyed on its `10` boundary/identity.
- A spray/flight window advisor produces deterministic operational windows from wind/precip/temperature thresholds, feeding `01` flight constraints and `14` tractor ops.
- Frost, heat, wind, and precipitation risk alerts are crop-stage-aware and explainable: each alert cites the inputs, thresholds, and freshness behind it.
- Growing-degree-day and evapotranspiration inputs feed irrigation (`16`) and drought (`17`).
- Historical weather per field supports trends and after-action review, and alerts route to `11` and `13`.
