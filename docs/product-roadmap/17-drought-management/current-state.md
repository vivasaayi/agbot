# Drought Management: Current State and Target State

## Mission

Give every field and region a defensible drought picture: fuse satellite and weather data into deterministic drought indices and vegetation-stress evidence, score risk against historical baselines, raise early warnings, and recommend adaptive strategies, always showing the evidence before any AI forecast.

## Current Maturity

greenfield pending (M0 named): no implementation exists; product-vision module from `docs/reference/product-summary.md` (#12 Drought Management). The stress indices, satellite data, and weather inputs it fuses are partially real in adjacent domains, but no drought-index model, data fusion, risk scoring, or early-warning path is built.

## What Exists Now

Nothing is built for this domain. The following adjacent surfaces are real or in progress and are what this module will build on:

- Vegetation and stress spectral indices and the imagery index pipeline scaffolding in `05` (`imagery_processor`).
- The Landsat client and spatial DB / scene storage in `07` (`geo_hub`), the planned source of satellite inputs.
- Weather models and forecasts intended in `15` (Weather Advisory, also greenfield), the planned source of meteorological drivers.
- Irrigation/mitigation capabilities intended in `16` (Water Management, also greenfield).
- Advisor recommendations and report scaffolding in `09` (`post_processor`).
- Field/region identity and tenant scoping in the field-and-data spine (`10`).

## Gaps to Close

- No drought-index data model (SPI/SPEI-style precipitation/water-balance indices) and no vegetation-stress evidence record sourced from `05`.
- No satellite + weather data fusion: no common, dated, georeferenced store joining Landsat (`07`) and weather (`15`).
- No historical baselines or seasonal trend computation.
- No per-field/region deterministic drought risk scoring.
- No early-warning or alerting on threshold crossings, and no routing to `13`/`11`.
- No mitigation strategy recommendations tied to `16` (irrigation) or `09` (advisor).
- No drought reporting and no evidence-before-advice gate separating deterministic indices from AI forecasts.

## Related Existing Surfaces

- `05` imagery and remote sensing (`imagery_processor`): vegetation/stress indices.
- `07` GIS and geospatial hub (`geo_hub`): Landsat client, spatial DB, scene storage.
- `15` weather advisory (greenfield): planned meteorological drivers and forecasts.
- `16` water management (greenfield): irrigation-based mitigation.
- `09` post-flight analytics and advisor (`post_processor`): recommendations and reports.
- `10` field, farm, and data management: field/region identity and tenant scoping.
- `docs/reference/product-summary.md` (#12 Drought Management): source description.

## Target Operating Model

- A drought-index data model holds deterministic SPI/SPEI-style indices plus vegetation-stress evidence from `05`, each dated, located, and traceable to its inputs.
- Satellite (`07` Landsat) and weather (`15`) data are fused into a common georeferenced store with freshness and coverage tracking.
- Historical baselines and seasonal trends make a current reading interpretable, not a bare number.
- A per-field/region deterministic risk score runs and is inspectable before any AI drought prediction; the AI forecast, when shown, cites its evidence layer and flags uncertainty.
- Early warnings fire on threshold crossings and route to the portal (`13`) and operator surfaces (`11`).
- Mitigation strategy recommendations tie to real field actions, primarily irrigation (`16`) and advisor guidance (`09`), with reporting and a full audit trail.

## Update (2026-07): satellite drought pipeline shipped

The "greenfield" maturity above is superseded. The satellite intelligence
pipeline (`docs/design/satellite-intelligence-pipeline.md`, batches 1-27,
branch `field-intelligence-pipeline`) implemented the deterministic drought
core this module planned:

- **VCI** (Vegetation Condition Index): multi-year cataloged NDVI L2
  archives build a per-pixel min/max climatology
  (`post_processor/src/index_climatology.rs`) and the current period scores
  into a VCI L3 raster (`post_processor/src/drought_indices.rs`,
  `geo_hub/src/drought_rasters.rs`,
  `POST /api/drought-management/rasters/derive`).
- **TCI** (Temperature Condition Index): the same route over `lst` L2
  products; LST derives from thermal DN via the pure engine in
  `post_processor/src/lst.rs` (`POST /api/thermal/lst/derive`).
- **VHI** (Vegetation Health Index): `α·VCI + (1−α)·TCI` blend of two
  registered same-grid drought L3s
  (`POST /api/drought-management/vhi/derive`).
- **SPI** (Standardized Precipitation Index): CHIRPS monthly/dekad
  registration + fetcher, Thom-1958 gamma fit, SPI-1/3/6/12 windows
  (`post_processor/src/spi.rs`, `geo_hub/src/spi_rasters.rs`,
  `POST /api/drought-management/spi/derive`).
- Severity classes follow the Kogan 10/20/30/40 convention; every product
  is a content-addressed catalog L3 with lineage to its observations and
  climatology, web-tiled via `/api/catalog/products/<id>/tiles/...`, listed
  in `/api/stac`, and derivable from the `/browse` UI.

Still open for this domain: weather-model fusion (`15`), SPEI-style
water-balance indices, early-warning threshold routing to `13`/`11`, and
mitigation recommendations tied to `16` — the alerting/advisor rails from
Tracks C/D exist and can consume drought findings.
