You are contributing to a modular Rust-based monorepo that performs periodic satellite image processing for agriculture insights.

Please generate a complete, modular Rust codebase (just the skeleton + key logic) to support the following WATER BODY DETECTION AND MONITORING features.

---

🏗️ **Monorepo Folder Structure** (target output):
```
src/
├── scheduler/                  # Periodic job runner
├── satellite_provider/         # Landsat, Sentinel support
│   └── landsat.rs
├── preprocessor/               # Image validation, band extraction, cloud masking
│   ├── validate.rs
│   ├── band_extractor.rs
│   └── cloud_mask.rs
├── analysis/                   # NDWI, NDVI, etc.
│   ├── ndvi.rs
│   ├── ndwi.rs                <-- ADD THIS
│   ├── zonal_stats.rs
│   └── drought_alert.rs
├── vectorization/              # Raster → polygon
│   └── gdal_polygonize.rs
├── alerts/                     # Farmer notification system
│   └── notifier.rs
├── models.rs                   # Reusable types: BBox, DateRange, AOI, etc.
├── main.rs                     # Orchestrator
```

---

🛰️ **Water Body Detection Requirements**

### NDWI Computation:
- Accept a multi-band GeoTIFF image (Landsat)
- Compute NDWI using: `(Green - NIR) / (Green + NIR)`
  - Landsat 8: Band 3 = Green, Band 5 = NIR
- Output a single-band raster (GeoTIFF) representing NDWI values

### Thresholding:
- Pixels with NDWI > 0.3 should be marked as WATER
- Create a binary mask for water pixels

### Vectorization:
- Convert water mask raster to polygons using `gdal_polygonize`
- Output GeoJSON with polygon metadata (area in m²)

### Area Calculation:
- Use GeoTIFF resolution to compute surface area of water bodies
- Add temporal comparison function to detect area loss over time

---

📅 **Temporal Monitoring Logic**:
- Store water area time-series per AOI
- Detect and log significant area drops between time intervals
- Raise alert if drop exceeds threshold (e.g., >10%)

---

🌧️ **Rain Prediction Integration**:
- Allow plugging in a rainfall forecast module (mock or real)
- Provide estimated time until next rain
- Combine with NDWI trends to issue drought alerts

---

📲 **Alert System**:
- Message format: 
  "🚨 Lake area dropped by X m² this week. Next rain in Y days. Watch for drought risk."

- Send via mock notifier module (e.g., print to console or webhook)

---

💡 **Modular Code Goals**
- Every function should be testable and reusable
- Keep all file I/O and logic separated
- Prefer async code with `tokio` where needed
- Use `Command::new()` for invoking GDAL CLI tools where no pure Rust equivalent exists

---

🔧 **Use the following crates**
- `gdal` for GeoTIFF I/O and metadata
- `image` for basic raster ops if needed
- `geojson` for vector output
- `chrono` for timestamps
- `serde` for all structured data types
- `tokio` and `anyhow` for async + error handling

---

✍️ Please generate:
- The `ndwi.rs` file with core NDWI + thresholding logic
- The `gdal_polygonize.rs` file to invoke raster → vector conversion
- Sample `models.rs` structs: `BBox`, `AOI`, `ImageMetadata`, `WaterAlert`
- And optionally a demo `main.rs` orchestration example

Ensure all modules use idiomatic Rust with `Result<T, Error>` patterns.
