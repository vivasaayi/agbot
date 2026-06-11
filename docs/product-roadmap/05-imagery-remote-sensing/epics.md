# Imagery and Remote Sensing: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: subcommand or route, input/output contract, output format, and product provenance.
- Geospatial: CRS, extent, resolution, and transform asserted and round-tripped through GeoTIFF.
- Deterministic: index/thermal/classification math that runs without AI, with min/max/mean and valid-pixel stats.
- Data quality: sensor calibration and QA masks applied before statistics.
- UI: overlay published via the `OverlayProcessor` trait and rendered by domain `08`.
- Tests: unit (index/thermal math), fixture (captured `04` bands), georeferencing round-trip, and one failure path (missing band/CRS).
- Operations: runtime mode, `gdal-io` feature flag, processing health, and a runbook.

## Category Epics

### EPIC-01: Deterministic Index Products
- Goal: a scene's bands become a georeferenced index raster with inspectable statistics.
- First release: NDVI/NDRE from real bands with valid-pixel min/max/mean and reason codes.
- Expansion: the remaining indices (EVI, SAVI, VARI, GNDVI, NDWI, MNDWI, MSAVI, NBR, NDMI, EVI2) and per-sensor presets.
- Hardening: radiometric calibration so values are comparable across captures and seasons.

### EPIC-02: Georeferencing and Product Output
- Goal: every product carries provably correct CRS, extent, and resolution.
- First release: assert CRS/extent/resolution and round-trip a product through GeoTIFF without drift.
- Expansion: PNG sidecar transforms and the `gdal-io` GeoTIFF write path hardened.
- Hardening: product provenance linking to scene/field/season for the advisor (`09`).

### EPIC-03: Thermal and Land-Surface Temperature
- Goal: thermal bands become a defensible LST product.
- First release: radiance → brightness temperature → LST with constant emissivity in Kelvin/Celsius.
- Expansion: NDVI-based emissivity and split-window when two TIR bands are present.
- Hardening: thermal stats, QA masking, and overlay rendering for the viewer.

### EPIC-04: Masks, Classification, and Overlays
- Goal: QA-masked, classified products that render as trustworthy overlays.
- First release: clear-sky QA mask applied before index statistics.
- Expansion: threshold and k-means classification over index rasters.
- Hardening: vegetation/thermal colormaps published via `OverlayProcessor` to domain `08`.
