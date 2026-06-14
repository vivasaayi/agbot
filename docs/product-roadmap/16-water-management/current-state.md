# Water Management: Current State and Target State

## Mission

Turn moisture evidence into trustworthy irrigation decisions: model soil moisture from sensors and remote-sensing proxies, compute evapotranspiration, and schedule water per management zone so a field gets the right amount at the right time, with the savings and the reasoning auditable.

## Current Maturity

greenfield pending (M0 named): no implementation exists; product-vision module from `docs/reference/product-summary.md` (#11 Water Management). Several inputs it depends on are partially real in adjacent domains, but no soil-moisture model, ET calculation, or scheduling engine is built.

## What Exists Now

Nothing is built for this domain. The following adjacent surfaces are real or in progress and are what this module will build on:

- Moisture-related spectral indices (NDWI/NDMI) and the imagery index pipeline scaffolding in `05` (`imagery_processor`).
- Weather inputs and the hyper-local forecast surface intended in `15` (Weather Advisory, also greenfield), which is the planned source of ET drivers.
- Management zones produced by the advisor (`09`, `post_processor`) and imagery clustering (`05`).
- Field/farm/zone identity and tenant scoping in the field-and-data spine (`10`).
- CRS/extent/transform discipline and layer storage in the GIS hub (`07`, `geo_hub`).

## Gaps to Close

- No soil-moisture data model: no sensor reading entity, no remote-sensing proxy ingestion, no per-reading freshness or QA flag.
- No evapotranspiration calculation (reference ET or crop ET) and no method/provenance record.
- No irrigation scheduling engine and no per-zone water plan.
- No zone-based water-need mapping that consumes `09`/`05` management zones.
- No weather-input contract with `15` for ET drivers (temperature, humidity, wind, radiation, precipitation).
- No irrigation hardware/valve control interface, dry-run, or execute path.
- No water-use or savings reporting and no alert routing to `13`/`11`.
- No per-field irrigation history or audit trail.

## Related Existing Surfaces

- `05` imagery and remote sensing (`imagery_processor`): NDWI/NDMI and index pipeline.
- `15` weather advisory (greenfield): planned ET driver inputs.
- `09` post-flight analytics and advisor (`post_processor`): management zones, recommendation patterns.
- `10` field, farm, and data management: field/zone identity and tenant scoping.
- `07` GIS and geospatial hub (`geo_hub`): CRS/extent discipline and layer storage.
- `docs/reference/product-summary.md` (#11 Water Management): source description.

## Target Operating Model

- A soil-moisture data model fuses ground-sensor readings and remote-sensing proxies (NDWI/NDMI from `05`), each reading located, dated, and quality-flagged, scoped to a field and zone via `10`/`07`.
- Evapotranspiration is computed deterministically from weather inputs (`15`) with the method and inputs cited, before any recommendation.
- Zone-based water-need mapping consumes management zones from `09`/`05`, so a recommendation always names the zone and the evidence it rests on.
- An irrigation scheduling engine produces a per-zone water plan, with water-use and savings reporting and alerts routed to `13`/`11`.
- An irrigation hardware/valve control interface can dry-run and execute a schedule, gated by safety checks and a full audit trail.
- Every field carries an irrigation history that makes water use repeatable and defensible season over season.
