I'll scan the ndvi_processor crate files and search the workspace for references to it to provide an accurate review and identify gaps. 

Read [](file:///Users/rajanpanneerselvam/work/agbot/ndvi_processor)

Read [](file:///Users/rajanpanneerselvam/work/agbot/ndvi_processor/src)

Read [](file:///Users/rajanpanneerselvam/work/agbot/ndvi_processor/Cargo.toml)

Read [](file:///Users/rajanpanneerselvam/work/agbot/ndvi_processor/src/lib.rs)

Read [](file:///Users/rajanpanneerselvam/work/agbot/ndvi_processor/src/main.rs)

I'll scan shared schemas to understand image metadata structure and look for workspace references to ndvi_processor to inform rename and integration steps. 

Read [](file:///Users/rajanpanneerselvam/work/agbot/shared/src/schemas.rs)

I'll search the workspace for references to ndvi_processor to assess integration points and impacts of a rename. 

Searched text for `ndvi_processor`, 20 results

Created 6 todos

To evaluate rename impact and potential naming conflicts, I'll open the referenced files in sensor_overlay_engine that mention NdviProcessor. 

Read [](file:///Users/rajanpanneerselvam/work/agbot/sensor_overlay_engine/src/composite.rs#1-1), lines 1 to 120

I'll check if other crates depend on ndvi_processor to assess rename impact across the workspace. 

Searched text for `ndvi_processor =` (`**/Cargo.toml`), no results

Ran terminal command: cargo check -p ndvi_processor

Read [](file:///Users/rajanpanneerselvam/work/agbot/Cargo.toml)

Starting (2/6) *Locate workspace references for rename impact*

Ran terminal command: cargo check --manifest-path /Users/rajanpanneerselvam/work/agbot/ndvi_processor/Cargo.toml

Completed (2/6) *Locate workspace references for rename impact*

I’ll review the ndvi_processor crate and outline gaps, then propose how to expand it into a full remote-sensing imagery analytics module and how to rename it cleanly.

## Quick review of the crate

What it does now:
- Walks an input directory, finds `metadata_*.json` (as `shared::schemas::MultispectralImage`)
- Loads band files for Red and NIR using the `image` crate
- Computes NDVI per pixel using 8-bit RGB red channel
- Saves a grayscale PNG and a JSON summary (`NdviResult`)
- Async file I/O with Tokio; simple CLI for input/output directories

What’s solid:
- Clean separation of CLI and library
- Uses shared schema types consistently
- Reasonable logging and error handling
- Output artifacts are predictable and structured (PNG + JSON)

Key gaps and improvement opportunities:
- Not included in workspace: The root Cargo.toml doesn’t include ndvi_processor in `[workspace].members`, so `cargo` commands at workspace root won’t build/run this crate.
- No geospatial awareness: The `image` crate drops georeferencing; output PNGs are not GeoTIFF/COG and can’t be used for GIS workflows.
- Radiometry issues: Converts to `rgb8`, losing precision. Multispectral bands are often single-channel 16-bit (or float). No radiometric/atmospheric correction or sensor calibration.
- Band access is brittle: Requires band names “Red” and “NIR” in JSON; doesn’t understand sensor-specific naming (Sentinel-2 B04/B08, Landsat OLI/TIRS bands, etc.).
- No reprojection/resampling/registration: Assumes Red/NIR are identically aligned. In the wild, bands might differ in resolution and grid.
- No cloud/mask handling: No cloud/shadow/water/invalid pixel masks considered.
- Performance/scale: Reads entire images into memory; no tiling/streaming; no parallelism across files or tiles.
- Limited CLI: Only NDVI; no subcommands, no band mapping, no masking, no output formats, no sensors.
- Testing: No unit/integration tests or golden samples.
- Unused deps: `imageproc` is listed but not used in the code.

## What you can add: analysis menu (beyond NDVI)

Vegetation/spectral indices (per sensor):
- NDRE (NIR-RedEdge)/(NIR+RedEdge), EVI, SAVI/MSAVI, GNDVI, ARVI, GCI, VARI
Water/snow/burn/urban/soil:
- NDWI/MNDWI, NDSI (snow), NBR, BAI, NDBI
Chlorophyll/Red Edge:
- CIgreen/CIred-edge, REIP
Thermal:
- LST (split-window/single-channel) from thermal bands (+ emissivity)
Topography/texture:
- Slope/aspect from DEM, GLCM textures (contrast, entropy)
Change/time series:
- Multi-date index trends, anomaly detection, change maps (delta, z-scores)
Classification/segmentation:
- Simple thresholding or unsupervised (k-means), then supervised models (e.g., Linfa/SmartCore) or ONNX segmentation
Vector products:
- Polygonization of classes, per-field statistics, area summaries
Mosaicking/stacking:
- Tile-based processing, mosaics, resample/warp to a common grid, COG output

## Proposed architecture for a full imagery analytics crate

High-level modules:
- io: Readers/writers for rasters with georeferencing
  - Prefer GDAL: `gdal` crate to read GeoTIFF/COG, preserve CRS/GeoTransform, tiled reading
  - Outputs: GeoTIFF/COG + quicklook PNG; worldfile and overviews where appropriate
- sensors: Band schema/mappings per sensor family (Sentinel-2, Landsat 8/9, PlanetScope, DJI)
- preprocess: Radiometric scaling, atmospheric correction hooks, resampling/warping (GDAL Warp), masks (cloud/shadow/water)
- indices: Trait-based calculators (NDVI, NDRE, EVI, SAVI, NDWI, NBR, etc.), configurable band maps
- thermal: LST and emissivity workflows
- stats: Streaming stats per tile, histograms, percentiles, vegetation area masks, class areas
- classify: Thresholding, k-means, ML hooks (Linfa/SmartCore/ONNX)
- pipeline: Tile-based execution engine, concurrency (rayon or async with bounded tasks), backpressure
- cli: Subcommands: indices, thermal, stats, mosaic, stack, classify, timeseries
- formats: Export utilities (COG options, overview levels, compression, nodata)
- config: YAML/JSON config for pipelines, band mapping, thresholds, output options

Tiny contract for indices:
- Input: One or more raster bands with consistent geotransform/extent
- Output: Single-band float32 raster + stats; optional mask
- Error modes: Missing band, misaligned grids, unsupported data type; supply helpful diagnostics
- Success criteria: Georeferenced output, correct nodata propagation, numerically stable for full range

Edge cases to handle:
- Different resolutions among bands (resample)
- Missing bands for requested index (explicit error)
- Huge rasters (tile-based processing, memory limits)
- No-data values and masks
- Coordinate reprojection to target CRS/grid
- 16-bit and float rasters (preserve dynamic range)

Dependencies to consider:
- gdal (core), proj (via gdal), rayon (parallel compute), ndarray (math buffers), linfa/smartcore (ML, optional), onnxruntime (optional)

Note on macOS: GDAL needs to be installed. For example:
```bash
brew install gdal
```

## Immediate gaps to fix in the current crate

- Add to workspace so it builds:
  - Edit root Cargo.toml `[workspace].members` to include `"ndvi_processor"`
- Read single-channel images, not `rgb8`:
  - Use `gdal` to load actual band arrays (preserve 16-bit/float) or, if staying with `image`, pick appropriate grayscale channels and handle 16-bit TIFF via `tiff` crate
- Save georeferenced output:
  - Write GeoTIFF/COG via GDAL with proper CRS, geotransform, nodata, compression (e.g., DEFLATE/LZW), and overviews
- Align bands:
  - Reproject/resample one band to the other band’s grid before computing indices
- Add CLI options:
  - `--index ndvi|ndre|evi|...`, `--sensor sentinel2|landsat8|custom`, `--out-format geotiff|cog|png`, `--threads`, `--tile-size`, `--mask <path>`
- Parallelize:
  - Use `rayon` or `FuturesUnordered` to process files concurrently; tile-based loops inside each file
- Tests:
  - Unit test NDVI with tiny synthetic arrays; integration test on a 64x64 sample GeoTIFF
- Clean up deps:
  - Remove `imageproc` if unused (or use it for quicklook rendering)

## Suggested rename

If you want this crate to be the general imagery analytics engine, pick a more general name.

Safe, descriptive options:
- imagery_processor (bin: imagery)
- raster_analytics (bin: raster)
- remote_sensing_processor (bin: rsproc)
- geospatial_imagery (bin: geoimg)
- agbot_imagery (bin: agbot-imagery)
- spectral_analytics (bin: spectral)

If you want an agriculture tilt:
- agro_imagery
- agri_analytics
- agro_vision

My pick for clarity and room to grow: imagery_processor (crate) with submodules `indices`, `thermal`, `classify`, etc.

Exact steps to rename:
1) Rename directory and package/bin:
- Move folder: ndvi_processor -> imagery_processor
- Edit imagery_processor/Cargo.toml:
  - [package] name = "imagery_processor"
  - [[bin]] name = "imagery_processor"
2) Update library crate path in main.rs:
- use imagery_processor::{...} instead of ndvi_processor::{...}
3) Update CLI naming in `#[command(name = "...")]` and help text in `Args`
4) Add to workspace:
- In root Cargo.toml: add "imagery_processor" under `[workspace].members`
5) Update docs:
- Replace ndvi_processor references in READMEs/prompts with the new name
6) Verify:
- Build from workspace root, run the binary with new name, ensure outputs and logs use new naming

Backward-compatibility tip:
- If you want to keep the current tool runnable while transitioning docs, you can keep the bin name the same initially and only change the crate name, then flip the bin later.

## CLI evolution sketch

- imagery_processor indices --index ndvi --input … --output … [--sensor sentinel2 --red B04 --nir B08 --mask cloud.tif --out-format cog]
- imagery_processor thermal --lst single-channel --band TIRS1 --output … --emissivity …
- imagery_processor stats --input … --percentiles 2,50,98 --json-out …
- imagery_processor classify --input … --method kmeans --k 5 --output …
- imagery_processor mosaic --inputs …/*.tif --output mosaic.tif --resampling average
- imagery_processor timeseries --inputs /path/{date}/index.tif --areas fields.geojson --output trends.csv

## Roadmap (phased)

Phase 1 (quick wins)
- Add to workspace, rename, tidy CLI help
- Replace RGB loading with band-specific single-channel reads
- Add NDRE, SAVI; make index generic with sensor band mapping
- Output GeoTIFF with CRS/nodata; PNG quicklook optional
- Unit tests for index math and simple I/O

Phase 2 (geospatial + performance)
- Introduce GDAL raster I/O; tile-based processing
- Resample/warp to common grid; COG outputs with overviews
- Concurrency (rayon) per tile/file; streaming stats

Phase 3 (masks, classification, timeseries)
- Cloud/shadow/water masks; nodata propagation
- KMeans classification; vectorization (polygonize) + area stats
- Multi-date pipelines; change detection

Phase 4 (thermal, advanced indices, ML)
- LST workflows; emissivity models
- Additional red-edge/chlorophyll indices
- ONNX/ML models for segmentation (optional feature flag)

If you’d like, I can apply the rename, wire it into the workspace, and stub the module layout for indices/thermal/stats so you have a solid scaffold to build on.