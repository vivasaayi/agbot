# Sensor Acquisition and Data Capture: Current State and Target State

## Mission

Capture LiDAR and multispectral data during a flight and persist every record with provenance, freshness, coverage, and an index, so the imagery (`05`), LiDAR mapping (`06`), and advisor (`09`) domains can trust their inputs and trace them back to a flight (`01`) and field/scene (`10`).

## Current Maturity

medium partial: `sensor_collector` has a real hardware/sim reader abstraction with dual runtime modes and async file I/O; `data_collector` has the session lifecycle, file-based storage, indexing, and an export dispatcher. Real-hardware paths are untested, simulated data is thin, and several exports and session aggregates are stubbed.

## What Exists Now

- `SensorCollectorService` orchestrating LiDAR and camera readers by `RuntimeMode` (`sensor_collector/src/lib.rs`).
- `LidarReader` reading an RPLIDAR A3 over `tokio_serial` and writing scans as JSONL, plus `SimulatedLidarReader` generating 360-point scans with obstacles and noise (`sensor_collector/src/lidar_reader.rs`).
- `CameraReader` capturing multispectral bands (RGB/NIR/Green/Blue) to TIFF plus metadata JSON, with `SimulatedCameraReader` generating vegetation-pattern band images (`sensor_collector/src/camera_reader.rs`).
- `DataCollectorService` with the `FlightSession` lifecycle (start/collect/end), `FlightDataRecord` and a twelve-variant `DataType`, and a six-variant `DataPayload` (telemetry/sensor/media/pointcloud/track/raw) (`data_collector/src/lib.rs`).
- File-based `StorageEngine` with date-partitioned layout, compression/backup hooks, and retention config (`data_collector/src/storage.rs`).
- `DataIndexer` with spatial-grid, temporal-bucket, and type indices and `find_by_location`/`find_by_time_range`/`find_by_type` lookups, plus a `SearchQuery` model (`data_collector/src/indexing.rs`).
- `DataExporter` with a format dispatcher; JSON and CSV exports work (`data_collector/src/export.rs`).

## Gaps to Close

- Real-hardware LiDAR and camera paths are mock/untested; the RPLIDAR parser and camera capture return mock data.
- Simulated sensor data is thin (fixed patterns) and not yet georeferenced to a flight path.
- `data_collector` exports are incomplete: `export_parquet()` and `export_hdf5()` are `unimplemented!()`; the broader GeoTIFF/PDF/HTML/KML/Shapefile ambition is not yet wired.
- Session aggregates currently return `0.0`: `calculate_flight_duration/distance_covered/area_covered/battery_consumption` do not yet derive values from telemetry (`data_collector/src/lib.rs`).
- `export_session` does not yet load session records before exporting (passes an empty vec).
- Storage load/list paths are stubbed: `list_sessions`, `load_session`, `load_data`, `cleanup_before_date`, and `get_stats` return empty/None/zeroed values.
- Indexer async methods (`index_session`, `search`, `rebuild`) are no-ops; only the in-memory lookups work.
- Tests are thin: a few construction/lifecycle/JSON-export smoke tests, no fixture or failure-path coverage.

## Source Modules Reviewed

- `sensor_collector/src/lib.rs`, `lidar_reader.rs`, `camera_reader.rs`, `main.rs`
- `data_collector/src/lib.rs`, `storage.rs`, `indexing.rs`, `export.rs`
- `shared/src/schemas.rs` (`LidarScan`, `LidarPoint`, `MultispectralImage`, `ImageMetadata`, `GpsCoords`, `RuntimeMode`)

## Target Operating Model

- A defined capture contract per sensor: inputs, freshness, coverage, sampling limits, and collection-failure handling.
- Every `FlightDataRecord` carries provenance (sensor, GPS, timestamp, calibration) and links to a flight (`01`) and field/scene (`10`).
- Storage is query-complete: load, list, search, retention, and stats all work, with integrity checks.
- The indexer answers spatial/temporal/type queries over persisted records, not just in-memory ones.
- Real session aggregates (distance, area, battery) computed from telemetry, not fixed zero values.
- Exports cover JSON/CSV and at least one geospatial format with CRS/extent preserved; `unimplemented!` paths removed or feature-gated.
- Real-hardware LiDAR/camera paths validated against the `02` sim with calibration and QA masks.
