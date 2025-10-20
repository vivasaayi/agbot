# NDVI Processor - Water Body Detection

This crate provides both NDVI vegetation analysis and NDWI water body detection capabilities for satellite imagery processing.

## Features

### NDVI Processing
- Vegetation index calculation using Near-Infrared (NIR) and Red bands
- Vegetation percentage analysis
- Statistical reporting (min, max, mean NDVI)

### NDWI Water Body Detection (New)
- Water body detection using Normalized Difference Water Index
- Binary water mask generation  
- Water area calculation in square meters
- Vector polygon generation (GeoJSON output)
- Temporal monitoring and drought alerts

## Usage

### NDVI Processing
```bash
cargo run --bin ndvi_processor -- --input-dir /path/to/images --output-dir /path/to/output
```

### NDWI Water Body Detection
```bash
cargo run --bin ndvi_processor -- --input-dir /path/to/images --output-dir /path/to/output --ndwi
```

### Water Body Monitoring
```bash
cargo run --bin water_monitor
```

## Water Body Detection Workflow

1. **NDWI Computation**: Calculate `(Green - NIR) / (Green + NIR)` for each pixel
2. **Thresholding**: Apply threshold (default 0.3) to create binary water mask
3. **Vectorization**: Convert raster mask to polygon features using GDAL
4. **Area Calculation**: Compute water body areas in square meters
5. **Temporal Analysis**: Compare with previous measurements to detect changes
6. **Alert Generation**: Generate drought risk alerts when water area drops significantly

## Data Structures

### Water Alert
```rust
pub struct WaterAlert {
    pub aoi_id: String,
    pub prev_area: f64,
    pub curr_area: f64,
    pub drop_pct: f64,
    pub timestamp: DateTime<Utc>,
    pub next_rain_days: Option<u32>,
    pub alert_level: AlertLevel,
}
```

### NDWI Result
```rust
pub struct NdwiResult {
    pub timestamp: DateTime<Utc>,
    pub source_images: Vec<Uuid>,
    pub output_path: String,
    pub water_mask_path: String,
    pub geojson_path: String,
    pub total_water_area: f64, // m²
    pub water_bodies_count: usize,
    pub min_ndwi: f32,
    pub max_ndwi: f32,
    pub mean_ndwi: f32,
}
```

## Dependencies

- `gdal`: GeoTIFF I/O and spatial operations
- `geojson`: Vector data format for water body polygons
- `chrono`: Timestamp handling
- `tokio`: Async runtime for I/O operations

## Example Output

```
🚨 CRITICAL Lake area dropped by 1500 m² (15.0%) this week. Next rain in 5 days. Watch for drought risk.
```

## Integration

The water body detection functionality integrates with the existing monorepo structure:

- Shared data structures in `shared/src/schemas.rs`
- WebSocket message types for real-time alerts
- Compatible with existing configuration and error handling
