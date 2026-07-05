# Imagery and Remote Sensing: Current State and Target State

## Mission

Convert captured multispectral and thermal bands into deterministic, georeferenced raster products — spectral indices, land-surface temperature, QA masks, and classifications — that the viewer renders and the advisor turns into agronomic findings.

## Current Maturity

early-to-strong partial: the CLI surface, index catalog, sensor presets, and overlay colormaps are real and well-shaped; the actual band-loading, index math, georeferencing, and thermal/classification computation in the pipeline modules are scaffolded or thin.

## What Exists Now

- A clap CLI with `indices`, `thermal`, `classify`, and `masks` subcommands and full argument surfaces (`imagery_processor/src/lib.rs`, ~278 lines).
- `IndexKind` enum with 12 spectral indices: NDVI, NDRE, EVI, SAVI, VARI, GNDVI, NDWI, MNDWI, MSAVI, NBR, NDMI, EVI2.
- Sensor presets (Sentinel-2, Landsat-8, DJI Multispectral) with default red/NIR/red-edge band mapping and per-band overrides.
- `ThermalArgs` covering split-window, NDVI-based emissivity, radiance/brightness-temperature/LST products, and Kelvin/Celsius units.
- `ClassifyArgs` (threshold or k-means) and `MasksArgs` (cloud/shadow/snow/water/clear from QA bands).
- `OutputFormat` PNG/GeoTIFF, with GeoTIFF behind the `gdal-io` feature.
- `sensor_overlay_engine` with `NdviProcessor` (vegetation colormaps), `ThermalProcessor` (temp range + palette), composite/heatmap and IDW grid interpolation, viridis/jet/hot/grayscale colormaps, and a pluggable `OverlayProcessor` trait (`sensor_overlay_engine/src/{lib.rs,ndvi.rs,thermal.rs,composite.rs}`).
- `IndexResultMeta` capturing timestamp, source image IDs, index name, and min/max/mean statistics.

## Gaps to Close

- Pipeline modules (`pipeline/{indices,thermal,classify,masks}.rs`) parse arguments but the band-loading, index computation, and product writing are minimal/scaffolded.
- Georeferencing is not first-class: CRS, extent, resolution, and transform are not asserted or round-tripped; the GeoTIFF/`gdal-io` path needs hardening.
- No multi-sensor radiometric calibration (DN → reflectance, top-of-atmosphere correction) before index math.
- QA masking is not yet applied before index statistics or classification.
- Thermal LST chain (radiance → BT → emissivity → LST) is defined in args but the computation is thin.
- No scene/field/season linkage on products, and no provenance handoff to the viewer (`08`) or advisor (`09`).
- Test coverage on the index/thermal math and georeferencing round-trip is missing.

## Source Modules Reviewed

- `imagery_processor/src/lib.rs`, `imagery_processor/src/main.rs`
- `imagery_processor/src/pipeline/{indices.rs,thermal.rs,classify.rs,masks.rs}`
- `imagery_processor/src/io/` (band/raster IO)
- `sensor_overlay_engine/src/{lib.rs,ndvi.rs,thermal.rs,composite.rs}`
- `shared/src/schemas.rs` (`ImageMetadata`, `MultispectralImage`, `NdviResult`, `RasterSpatialRef`)

## Target Operating Model

- One deterministic index/thermal/mask product per scene with asserted CRS, extent, resolution, and a GeoTIFF that round-trips losslessly.
- QA masks applied before statistics; min/max/mean and valid-pixel coverage retained as inspectable evidence with reason codes.
- Radiometric calibration per sensor preset so NDVI/NDRE values are comparable across captures and seasons.
- Thermal LST computed through the full radiance → BT → emissivity chain, with NDVI-derived emissivity when available.
- Every product linked to scene/field/season and published as an overlay the viewer (`08`) trusts and the advisor (`09`) cites.
- Fixture-first: the pipeline runs on captured `04` fixtures before real-hardware inputs, with unit tests on the index/thermal math.

## Update (2026-07): satellite ingestion and derivation shipped

The gaps list above predates the satellite intelligence pipeline
(`docs/design/satellite-intelligence-pipeline.md`, batches 1-27). This
domain is no longer drone-only:

- **Satellite ingestion**: Earth Search STAC scene search + AOI-windowed
  remote-COG band reads (`geo_hub/src/satellite_derivation.rs`,
  `raster_io` remote backend); USGS Landsat live ingest through the
  normalized `commit_ingest` contract (`geo_hub/src/landsat.rs`); Sen2Cor
  L1C→L2A orchestration with JP2 band decode — pure-Rust OpenJPEG port, no
  GDAL (`geo_hub/src/sen2cor.rs`, `sen2cor_derive.rs`, `raster_io/src/jp2.rs`);
  HLS v2.0 harmonized Landsat+Sentinel Int16 ingestion with Fmask cloud
  masking (`geo_hub/src/hls.rs`); Sentinel-1 SAR backscatter and CHIRPS
  precipitation registration.
- **Radiometric calibration is real**: per-sensor DN→reflectance profiles
  incl. the Sentinel-2 baseline-04.00 BOA offset
  (`imagery_processor/src/pipeline/calibration.rs`), applied before index
  math; QA/Fmask masking applied before statistics.
- **Thermal LST chain is complete**: DN→radiance→brightness temperature→
  emissivity-corrected LST as a pure engine (`post_processor/src/lst.rs`)
  registered as catalog `lst` L2 products feeding TCI/VHI.
- **Provenance handoff exists**: every live pipeline (`indices`, `masks`,
  `thermal`, `classify`) writes `*.product_record.json` sidecars
  (`imagery_processor/src/product_sidecar.rs`) that register into the
  catalog with L0→L3 lineage; products carry scene/field/season scope and
  web-tile through `geo_hub`.

Remaining from the original gaps: broader fixture coverage for the drone
(`04`) capture path and the `gdal-io` GeoTIFF hardening for the CLI PNG
path (the catalog path uses `raster_io` GeoTIFFs and does not need GDAL).
