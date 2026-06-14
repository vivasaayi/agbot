# Imagery and Remote Sensing: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (geospatial correctness, data quality, agronomic value, performance/scale, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated. Geospatial correctness is the dominant pillar for this domain.

## Imagery and Remote Sensing Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Band ingest and metadata mapping | partial | 7 | Load bands from `metadata_*.json` and resolve band names per sensor preset |
| Spectral index computation (12 indices) | partial (catalog defined) | 9 | Compute NDVI/NDRE with valid-pixel stats and reason codes |
| Sensor presets and band overrides | strong partial | 6 | Sentinel-2/Landsat-8/DJI mapping with per-band override |
| Radiometric calibration | missing | 6 | DN → reflectance per preset before index math |
| Georeferencing and CRS/extent assertion | missing | 9 | Assert CRS/extent/resolution and round-trip through GeoTIFF |
| GeoTIFF / PNG product output | partial (`gdal-io`) | 7 | Harden GeoTIFF write with sidecar transform |
| Thermal LST pipeline | partial | 8 | Radiance → BT → LST with constant emissivity |
| NDVI-based emissivity and split-window | partial | 5 | Emissivity from NDVI image; split-window when two TIR bands |
| QA masking (cloud/shadow/snow/water/clear) | partial | 7 | Generate clear-sky mask and apply before stats |
| Classification (threshold and k-means) | partial | 6 | Threshold vegetation classes over an index raster |
| Overlay rendering and colormaps | strong partial | 7 | Vegetation/thermal colormaps via `OverlayProcessor` for the viewer |
| Product provenance and scene linkage | missing | 6 | Link product to scene/field/season for the advisor |
