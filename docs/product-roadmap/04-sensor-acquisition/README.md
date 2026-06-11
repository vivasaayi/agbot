# Sensor Acquisition and Data Capture

Capture LiDAR and multispectral data during a flight and persist it with provenance, freshness, and index so downstream analysis can trust it.

## Where We Are

- `sensor_collector` has a real hardware/sim abstraction: `LidarReader` (RPLIDAR A3 async serial, JSONL out), `CameraReader`, and simulated counterparts, with dual runtime modes and async file I/O.
- `data_collector` has the capture session lifecycle: `FlightSession`, `FlightDataRecord` (telemetry/sensor/media/pointcloud/logs), a file-based `StorageEngine` with compression and retention, a spatial/temporal/type `DataIndexer`, and a `DataExporter`.
- Real-hardware paths are untested and the simulated sensor data is thin; several exports and session aggregates are stubbed.

## Where We Should Be

- Real LiDAR and multispectral capture with full provenance (sensor, GPS, timestamp, calibration) linked to the flight (`01`) and field/scene (`10`).
- Capture sessions with freshness, coverage, and collection-failure handling, indexed for fast spatial/temporal query.
- Working exports (including geospatial formats) and real session aggregates (distance/area/battery) feeding domains `05`/`06`/`09`.

## Files

- `current-state.md`: source modules reviewed, maturity, gaps, and target operating model.
- `capability-map.md`: capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0 slices.

## Build Order

1. Define the capture contract and link a session to flight (`01`) and field/scene (`10`).
2. Persist `FlightDataRecord`s with full provenance, freshness, and coverage.
3. Make spatial/temporal/type indexing query-complete (load/list/search).
4. Compute real session aggregates (distance, area, battery) instead of 0.0 placeholders.
5. Finish the export path (CSV/JSON working; add geospatial formats, replace `unimplemented!`).
6. Exercise and validate the real-hardware LiDAR/camera paths against the sim.

## Primary Crates

`sensor_collector` (readers), `data_collector` (session/storage/index/export), with `shared` for schemas. Inputs come from flight (`01`) and the `02` sim; outputs feed imagery (`05`), LiDAR mapping (`06`), and the advisor (`09`).
