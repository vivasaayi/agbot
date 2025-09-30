Observed gaps and opportunities
Geospatial fidelity: GeoTIFF writer copies metadata only when GDAL feature is enabled; otherwise outputs lack CRS/transform altogether.
Band management: Sensor presets exist but there’s no validation against metadata; red-edge/green/blue fetches assume image.file_paths contains defaults with exact names.
Masking & nodata: Masks are binary; no weighting or morphological cleanup. Float outputs mix NaN and -9999 nodata conventions.
Performance: All per-pixel loops are single-threaded; large scenes will be slow. No streaming/chunked processing.
Thermal accuracy: LST still uses a very rough emissivity correction and doesn’t support split-window, atmospheric corrections, or per-pixel emissivity lookup from NDVI.
Classification: Works only on 8-bit PNGs; no multi-band or probability outputs, no model-based classifiers.
I/O abstraction: Non-GDAL path still depends on PNGs for indices; no native Sentinel/Landsat product readers.
Testing/validation: No unit/integration tests around math, metadata writing, or CLI flows.
Feature ideas to consider
🌱 Near-term, high-impact
CRS-aware GeoTIFF output without GDAL

Use tiff/geotiff crates or embed simple tags so PNG users still get georeferencing when GDAL isn’t compiled.
Mirror metadata from MultispectralImage (bounding box, resolution).
Parallel pixel processing

Adopt rayon for CPU-bound loops or spawn tasks per tile. Simple speed win for NDVI/classification.
Consistent nodata handling

Define a shared enum for nodata strategy (NaN vs sentinel) and enforce on read/write.
Propagate nodata into stats (currently stats ignore masked pixels but not explicit nodata).
Better CLI ergonomics

Add --pattern/--image-id filters instead of scanning every metadata file.
Provide --overwrite/--skip-existing flags to avoid reprocessing.
Unit tests for formula sanity

NDVI, EVI, thermal conversions using tiny fixture arrays.
Mask bit decoding table tests.
🚀 Medium-term feature expansions
Timeseries pipeline

Aggregate multiple acquisitions, compute trend/seasonal metrics, export charts or GeoTIFF stacks.
Per-field statistics

Accept polygon shapefiles/GeoJSON, compute mean/min/max per field, output CSV + summary map.
Cloud/shadow refinements

Integrate FMask-like logic or morphological cleanup (dilation/erosion) to reduce speckle.
Multi-band classifiers

Extend classify to ingest multi-channel arrays (e.g., NDVI + NDWI) and run simple random-forest or SVM (via linfa or smartcore crates).
Support model serialization for re-use.
On-the-fly radiometric calibration

For indices, resolve scaling factors/gains directly from metadata (e.g., Sentinel SAFE, Landsat MTL) so values are normalized reflectance.
Split-window LST / emissivity-from-NDVI

When two thermal bands are provided, implement a more accurate split-window algorithm.
Allow emissivity maps derived from NDVI thresholds rather than a constant.
Sensor auto-detection & band alignment

Parse MultispectralImage metadata to auto-detect sensor type, verify required bands, and resample misaligned bands with GDAL where available.
🛰️ Advanced / longer-term roadmap
Cloud Optimized GeoTIFF (COG) + STAC metadata

Produce COG outputs and STAC-compliant item metadata for downstream pipelines.
ONNX segmentation / ML inference

Integrate optional ONNX models (e.g., canopy segmentation, building masks) with GPU acceleration when available.
Change detection & alerts

Pair two timepoints (t1/t2) to produce delta maps, detect anomalies, and output PDF/HTML reports.
WebAssembly / server mode

Expose pipelines as an API service (Actix/Axum) for automation or run in WebAssembly for browser demos.
Interactive QA dashboard

Generate quicklook tiles (XYZ) and a JSON manifest so the ground-station UI can preview outputs interactively.