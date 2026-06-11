# Sensor Acquisition and Data Capture: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: reader run loop or collector command, persistence, and audit events.
- Safety: capture is non-mutating, but integrity, validation, and collection-failure handling are required.
- Deterministic: storage, indexing, and aggregate math that run without AI.
- Telemetry: capture freshness, coverage, and collection-failure states per session.
- UI: session list, record inspect, and export (consumed by domains `09`/`11`).
- Tests: unit (index/aggregate math), fixture (LiDAR scan JSONL, band metadata), API/CLI contract, and one failure path (sensor dropout / disk full).
- Operations: runtime mode (`Simulation` first), collection health, retention, and a runbook.

## Category Epics

### EPIC-01: Sensor Capture and Provenance
- Goal: trustworthy LiDAR and multispectral capture with full provenance linked to a flight.
- First release: persist `FlightDataRecord`s with sensor/GPS/timestamp/calibration provenance, session linked to `01`/`10`.
- Expansion: georeferenced simulated and real capture with QA masks and freshness.
- Hardening: validate the real RPLIDAR A3 and camera paths against the `02` sim.

### EPIC-02: Storage, Indexing, and Aggregates
- Goal: a query-complete capture store with real session metrics.
- First release: make storage load/list/search work over persisted records via the indexer.
- Expansion: compute session aggregates (distance/area/battery) from the telemetry track.
- Hardening: retention, integrity checks, and storage stats.

### EPIC-03: Export and Downstream Hand-off
- Goal: captured data exported in formats the advisor and GIS domains can consume.
- First release: fix `export_session` to load records, with JSON/CSV exports verified.
- Expansion: one geospatial export (GeoTIFF/KML/Shapefile) with CRS/extent preserved.
- Hardening: remove or feature-gate `unimplemented!` Parquet/HDF5 and add export contract tests.
