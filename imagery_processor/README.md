# imagery_processor

Imagery processing toolkit for remote sensing: spectral indices, thermal visualization, and simple classification.

Status: working MVP. PNG outputs are supported by default. Optional GeoTIFF output is available behind the `gdal-io` feature when GDAL is installed and discoverable by `pkg-config`.

## Features
- Indices: NDVI, NDRE, EVI, SAVI, VARI, GNDVI, NDWI, MNDWI, MSAVI, NBR, NDMI, EVI2
- Thermal: radiance, brightness temperature, and emissivity-corrected LST outputs
- Masks: Landsat QA_PIXEL cloud, shadow, snow, water, and clear masks
- Classify: threshold or k-means on index rasters

## Usage

Indices (NDVI by default):

```
cargo run --bin imagery_processor -- indices --input-dir ./data --output-dir ./out
```

NDRE with explicit red-edge band:

```
cargo run --bin imagery_processor -- indices --input-dir ./data --output-dir ./out --index ndre --red-edge B05
```

Thermal (placeholder):

```
cargo run --bin imagery_processor -- thermal --input-dir ./data --output-dir ./out --thermal-band Thermal
```

Classify by threshold:

```
cargo run --bin imagery_processor -- classify --input-image ./out/ndvi_xxx.png --output-path ./out/veg_mask.png --threshold 0.3
```

K-means classification:

```
cargo run --bin imagery_processor -- classify --input-image ./out/ndvi_xxx.png --output-path ./out/labels.png --kmeans 4
```

## GeoTIFF output (optional)

Enable the feature and request geotiff output:

```
cargo run --features gdal-io --bin imagery_processor -- indices --input-dir ./data --output-dir ./out --out-format geotiff
```

Note: This writes a basic GeoTIFF and attempts to copy CRS/GeoTransform from the source band. You’ll need GDAL installed on your system.

## Roadmap
- GDAL-based reading/writing with CRS and GeoTransform (COG)
- Band alignment and masks (cloud/water/shadow)
- LST with emissivity and radiance conversion
- Polygonization and per-field stats
- Timeseries and change detection
- ONNX segmentation (optional)
