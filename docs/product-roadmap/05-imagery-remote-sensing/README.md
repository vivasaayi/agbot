# Imagery and Remote Sensing

Turn captured multispectral and thermal bands into trustworthy, georeferenced raster products: spectral indices, land-surface temperature, QA masks, and classifications the viewer renders and the advisor reasons over.

## Where We Are

- `imagery_processor` is a clap CLI with `indices`/`thermal`/`classify`/`masks` commands, 12 spectral indices, sensor presets (Sentinel-2, Landsat-8, DJI Multispectral), and PNG/GeoTIFF output (GeoTIFF behind the `gdal-io` feature).
- `sensor_overlay_engine` has `NdviProcessor`, `ThermalProcessor`, composite/IDW utilities, and viridis/jet/hot/grayscale colormaps behind a pluggable `OverlayProcessor` trait.
- The pipeline modules (`pipeline/{indices,thermal,classify,masks}.rs`) parse and run, but the computation and georeferencing path is thin and partly scaffolded.

## Where We Should Be

- Each index/thermal/mask product asserts CRS, extent, resolution, and transform, and round-trips through GeoTIFF without drift.
- Deterministic statistics (min/max/mean, valid-pixel coverage) accompany every product with QA masking applied before stats.
- Products link to scene/field/season and flow to the viewer (`08`) and advisor (`09`) as evidence layers, with multi-sensor band/calibration handling.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Harden the index pipeline: real band loading, NDVI/NDRE math, valid-pixel stats, and reason codes.
2. Assert and round-trip georeferencing (CRS/extent/resolution) through PNG sidecars and GeoTIFF.
3. Apply QA masks (cloud/shadow/water) before index statistics and classification.
4. Complete the thermal pipeline: radiance → brightness temperature → LST with NDVI emissivity.
5. Wire deterministic classification (threshold and k-means) over real index rasters.
6. Link products to scene/field/season and publish overlays to the viewer (`08`).

## Primary Crates

`imagery_processor`, `sensor_overlay_engine`, with `shared` for config, schemas, and `RasterSpatialRef`. Consumes capture from domain `04` (or fixtures), feeds the GIS hub (`07`), viewer (`08`), and advisor (`09`). This is a core Advisor-MVP domain.
