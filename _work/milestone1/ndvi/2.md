I’ll give you a focused re-review of the current crate, call out concrete gaps, suggest impactful additions, and recommend a name that fits the broader scope you want.

## What you have now (fast recap)

- Crate: imagery_processor
  - CLI subcommands: indices (NDVI/NDRE/EVI/SAVI), thermal (LST with emissivity), classify (threshold/k-means).
  - Input: band files + a metadata_*.json describing each capture.
  - Outputs: PNG quicklooks; GeoTIFF when built with GDAL, with CRS/GeoTransform copied and float32 indices/LST with nodata.
  - Band presets for common sensors; minimal band alignment via GDAL resampling.
  - Mask support for stats; some unit tests for core math.

This already moves beyond NDVI and lays a solid foundation.

## Key gaps to close

- Geospatial I/O
  - COG output: creation options (compression, tiling, overviews) and an explicit “prefer COG” flag.
  - Reprojection/warping: choose target CRS/grid; support resampling modes (nearest, bilinear, cubic).
  - Full nodata propagation across all algorithms (indices, thermal, classification), not only where f32 buffers are used.
  - Metadata management: write/retain units, scale/offset, and add STAC-like sidecar JSON.

- Calibration and sensor metadata
  - Parse scene metadata (e.g., Landsat MTL, Sentinel SAFE) to pull ML/AL/K1/K2, acquisition parameters, and nodata automatically.
  - Atmospheric correction presets/workflows (DOS, 6S, Sen2Cor handoff or integration).
  - Topographic correction using DEM (c-factor, SCS+C).

- Algorithms and products
  - More indices: VARI, GNDVI, NDWI/MNDWI, MSAVI, NBR, NDMI, EVI2.
  - Cloud/shadow/snow/water masks: Fmask integration or QA band parsing.
  - Thermal: add radiance/brightness temp products; emissivity maps by land cover or NDVI-based; split-window methods when two TIR bands available (e.g., B10/B11).
  - Pan-sharpening, super-resolution, and band math expression engine (user-defined formulas).
  - Mosaic/temporal: scene stitching, feathering, temporal composites (median, max NDVI), change detection, trend analysis.
  - Zonal stats and vectorization: per-polygon stats, raster to vector contours/iso-lines.
  - Classification: random forest/XGBoost/lightweight CNN hooks; model I/O and inference CLI.

- Scalability and robustness
  - Tiled/chunked processing for large rasters; streaming and memory bounds.
  - Concurrency with backpressure; progress reporting; resume on failure and durable logs.
  - Deterministic handling of edge cases: all-zero denominators, NaNs/Infs, masked regions.

- DX, testing, docs
  - More unit and integration tests with small fixtures (tiny GeoTIFFs).
  - Golden image tests for indices; property-based tests for band math safety.
  - CLI examples and cookbook; sensor preset docs; error messages and troubleshooting.
  - Benchmarks for hot loops; optional SIMD (e.g., faster f32 kernels).

- Integration and standards
  - STAC read/write; PROJ/CRS controls; VRT support for multi-band stacks.
  - Config file/preset packs for sensors and product pipelines.
  - Output color ramps and legends; QGIS/ArcGIS style compatibility.

## High-impact additions you can do next

- Add a --prefer-cog flag and write COGs with compression=LZW/DEFLATE, TILED=YES, BLOCKSIZE=256, overviews (2,4,8,16), and internal masks.
- Implement a warp stage: “--target-crs EPSG:XXXX --res x --resampling {nearest,bilinear,cubic}”.
- Parse Landsat MTL and Sentinel SAFE: auto-fill ML/AL/K1/K2, nodata, and unit metadata.
- Expression engine: a simple parser to compute band math expressions (e.g., “(B08-B04)/(B08+B04)”).
- Zonal stats: accept a vector file + attribute field and write per-zone JSON/CSV outputs.
- Cloud/water masking: start with QA band parsing (Landsat QA_PIXEL) and NDWI thresholds as a fallback.

## Rename suggestions

You’re already on “imagery_processor,” which is clear and accurate. If you want alternatives that reflect geospatial + analytics:

- geo_imagery or georaster_processor
- rs-imagery (remote sensing imagery)
- terra-analytics or terra-imagery
- orthokit or rasterlab

My recommendation: keep imagery_processor. It’s descriptive, broad, and aligns with your roadmap.

If you still want to rename:
- Change package name and bin in Cargo.toml.
- Update the directory name to match.
- Update workspace Cargo.toml [workspace.members].
- Grep for the old name in README/docs and references.

## Optional roadmap (phased)

- Phase A (Geo correctness): COG outputs, warp/reprojection, robust nodata/masks, CRS/units metadata.
- Phase B (Calibration/science): metadata parsing, atmospheric/topographic corrections, emissivity maps.
- Phase C (Analytics): more indices, zonal stats, cloud/water masking, temporal composites.
- Phase D (Scale/UX): tiling + streaming, concurrency, progress/resume, benchmarks, more tests.
- Phase E (ML and integrations): classifiers/segmenters, STAC ecosystem, QGIS styles/legends.

If you want, I can implement the COG flag and a warp step next, or wire up Landsat MTL parsing for calibration constants so LST is scene-accurate out of the box.