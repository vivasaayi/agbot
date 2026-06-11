# Sensor Acquisition and Data Capture: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and persisted without AI — here, provenance, freshness, coverage, and integrity.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `OPS` operator/pilot, `DSP` drone service provider, `AG` agronomist, `PA` platform admin.

Capture is the input layer for imagery (`05`), LiDAR mapping (`06`), and the advisor (`09`). Every captured record carries provenance (sensor, GPS, timestamp, calibration) and links to a flight (`01`) and field/scene (`10`); no session is "captured" without freshness, coverage, and a collection-failure path. Correctness and provenance outrank breadth of formats.

---

## M1 — Foundation

### STORY 04-01 · M1 · S · P0 — Capture session lifecycle and identity
- **Story**: As `DSP`, I want each capture session to have a stable ID and start/collect/end lifecycle, so that every capture is traceable and reproducible.
- **Deterministic / evidence**: `FlightSession` persists `{session_id, flight_id, field_id, scene_id, owner, status, started_at, ended_at?}`; lifecycle `Started→Collecting→Ended|Failed`; transitions deterministic.
- **Acceptance**:
  - Given a flight, when a session starts, then it is persisted with linkage IDs and `Started` status via `data_collector`.
  - Given a session asked to collect before it has started, when invoked, then it is rejected with a state error, not silently accepted.
- **Tests**: unit (lifecycle transitions), API contract (start/collect/end), failure path (collect-before-start rejected).
- **Depends on**: `01` (flight), `10` (field/scene), `data_collector/src/lib.rs`.

### STORY 04-02 · M1 · M · P0 — Data record model and provenance
- **Story**: As `DSP`, I want every `FlightDataRecord` to carry full provenance, so that imagery/LiDAR/advisor can trust and trace each input.
- **Deterministic / evidence**: persist `{record_id, session_id, data_type, sensor_id, gps_coords, timestamp, calibration_ref}` for all twelve `DataType`s and six `DataPayload`s; reject records missing required provenance.
- **Acceptance**:
  - Given a sensor reading, when a record is created, then it persists with sensor/GPS/timestamp/calibration provenance linked to the session.
  - Given a record missing GPS or timestamp, when created, then it is rejected with a provenance error rather than stored incomplete.
- **Tests**: unit (provenance validation per DataType), API contract, failure path (missing-provenance record rejected).
- **Depends on**: 04-01, `shared/src/schemas.rs`.

### STORY 04-03 · M1 · S · P1 — Session-to-flight-and-field linkage integrity
- **Story**: As `PA`, I want session linkage to flight and field/scene validated, so that no capture is orphaned from its mission or field.
- **Deterministic / evidence**: assert referenced `flight_id`/`field_id`/`scene_id` exist before a session begins collecting; deterministic linkage check.
- **Acceptance**:
  - Given valid references, when a session starts, then linkage validates and collection proceeds.
  - Given an unknown flight_id, when a session starts, then it is rejected with a linkage error, not started orphaned.
- **Tests**: unit (linkage validation), failure path (unknown flight_id rejected), fixture.
- **Depends on**: 04-01, `01`, `10`.

---

## M2 — Captured / Observable

### STORY 04-04 · M2 · M · P0 — LiDAR capture validated against the sim
- **Story**: As `OPS`, I want the real RPLIDAR A3 serial path captured as records and validated against the `02` sim, so that real scans are trustworthy, not mock.
- **Deterministic / evidence**: parse the RPLIDAR A3 over `tokio_serial` into `LidarScan`/`LidarPoint` records with provenance; a fixture compares parser output to the `02` `SimulatedLidarReader` shape; replace mock parse.
- **Acceptance**:
  - Given a serial scan stream, when captured, then JSONL `LidarScan` records are persisted with provenance and match the sim-shaped schema.
  - Given a malformed/dropped serial frame, when parsed, then the frame is recorded as a collection failure, not silently skipped.
- **Tests**: unit (RPLIDAR parser), fixture (sim-shape comparison), failure path (malformed frame → collection failure).
- **Depends on**: 04-02, 04-07, `02` (sim shape), `sensor_collector/src/lidar_reader.rs`.

### STORY 04-05 · M2 · M · P0 — Multispectral camera capture with calibration
- **Story**: As `AG`, I want multispectral bands captured with georeference and calibration metadata, so that imagery (`05`) can process trustworthy inputs.
- **Deterministic / evidence**: capture RGB/NIR/Green/Blue bands to TIFF + metadata JSON with GPS, timestamp, and calibration; assert band completeness; replace mock capture.
- **Acceptance**:
  - Given a capture trigger, when bands are captured, then all configured bands are written with georeference and calibration metadata.
  - Given a missing band (sensor fault), when captured, then the record is flagged incomplete and a collection failure recorded, not stored as if complete.
- **Tests**: unit (band completeness + metadata), failure path (missing band → incomplete + failure), fixture.
- **Depends on**: 04-02, 04-07, `sensor_collector/src/camera_reader.rs`.

### STORY 04-06 · M2 · S · P1 — Georeference simulated sensor readers to a flight path
- **Story**: As `DSP`, I want simulated LiDAR/camera readers georeferenced to a real flight path, so that the capture pipeline can be exercised end to end without hardware.
- **Deterministic / evidence**: drive `SimulatedLidarReader`/`SimulatedCameraReader` along a `01` flight path, tagging each reading with the path's GPS/time so output is capture-shaped and provenance-complete.
- **Acceptance**:
  - Given a flight path, when simulated capture runs, then readings carry GPS/time from the path and persist as provenance-complete records.
  - Given a path with a gap, when simulated capture runs, then the gap is reflected as a coverage hole, not interpolated away.
- **Tests**: unit (path georeferencing), failure path (path gap → coverage hole), fixture.
- **Depends on**: 04-02, `01` (flight path), `02`, `sensor_collector/src/lidar_reader.rs`, `camera_reader.rs`.

### STORY 04-07 · M2 · S · P0 — Freshness, coverage, and collection-failure handling
- **Story**: As `OPS`, I want capture freshness, coverage, and collection failures tracked per session, so that I know whether a flight actually captured the field.
- **Deterministic / evidence**: compute per-session freshness (age of last record), coverage (captured fraction of the planned area), and a typed collection-failure log; a session is only "captured" when these exist.
- **Acceptance**:
  - Given a steady capture, when freshness/coverage are computed, then the session reports recent freshness and a coverage fraction.
  - Given a sensor dropout mid-flight, when computed, then a collection-failure is logged and coverage reflects the gap — the session is not marked fully captured.
- **Tests**: unit (freshness + coverage math), failure path (dropout → failure + reduced coverage), fixture.
- **Depends on**: 04-01, 04-02.

### STORY 04-08 · M2 · S · P1 — Capture health and retry/backoff
- **Story**: As `OPS`, I want transient sensor errors retried with backoff and surfaced as health, so that a brief glitch does not lose the whole session.
- **Deterministic / evidence**: deterministic retry/backoff on transient reader errors; persist a capture-health signal (rate, error counts); escalate to a collection failure after a bound.
- **Acceptance**:
  - Given a transient serial error, when capture retries, then it recovers within the retry bound and health reflects the blip.
  - Given persistent errors past the bound, when retries exhaust, then a collection failure is recorded and the operator alerted, not retried forever.
- **Tests**: unit (retry/backoff + escalation), failure path (persistent error escalates), fixture.
- **Depends on**: 04-04, 04-05, 04-07.

---

## M3 — Explainable

### STORY 04-09 · M3 · M · P0 — Query-complete file storage and retention
- **Story**: As `DSP`, I want storage load/list/cleanup/stats to work over persisted records, so that captured data is actually retrievable, not just written.
- **Deterministic / evidence**: implement the stubbed `list_sessions`, `load_session`, `load_data`, `cleanup_before_date`, `get_stats` over the date-partitioned `StorageEngine`; retention is deterministic and audited.
- **Acceptance**:
  - Given persisted sessions, when listed/loaded, then they return the actual records (not empty/None), and stats reflect real counts/sizes.
  - Given a retention cutoff, when cleanup runs, then only records before the cutoff are removed and the action is audited; a cleanup that would delete an in-progress session is refused.
- **Tests**: unit (load/list/stats), failure path (cleanup refuses active session), fixture (`data_collector/src/storage.rs`).
- **Depends on**: 04-02.

### STORY 04-10 · M3 · M · P0 — Persisted spatial/temporal/type indexing
- **Story**: As `AG`, I want spatial/temporal/type queries answered over persisted records, so that I can find captures by where/when/what without scanning everything.
- **Deterministic / evidence**: implement the no-op `index_session`/`search`/`rebuild` so the spatial-grid/temporal-bucket/type indices cover persisted records; `SearchQuery` returns from disk-backed state.
- **Acceptance**:
  - Given indexed sessions, when a `SearchQuery` runs by location/time/type, then it returns the matching persisted records.
  - Given a corrupted index, when `rebuild` runs, then the index is reconstructed from records and a subsequent query succeeds — queries never silently return partial results from a stale index.
- **Tests**: unit (find_by_location/time/type over persisted), failure path (rebuild recovers corrupt index), fixture (`data_collector/src/indexing.rs`).
- **Depends on**: 04-09.

### STORY 04-11 · M3 · S · P1 — Session aggregates from telemetry
- **Story**: As `DSP`, I want flight duration/distance/area/battery computed from the telemetry track, so that session metrics are real, not `0.0` placeholders.
- **Deterministic / evidence**: replace the `0.0`-returning `calculate_flight_duration/distance_covered/area_covered/battery_consumption` with telemetry-derived computation; each value cites the track it used.
- **Acceptance**:
  - Given a session with a telemetry track, when aggregates compute, then duration/distance/area/battery are non-placeholder values derived from the track.
  - Given a session with no telemetry, when aggregates compute, then they return an explicit "no track" result, not a misleading `0.0`.
- **Tests**: unit (aggregate math from track), failure path (no track → explicit absence), fixture.
- **Depends on**: 04-09, `01` (telemetry track).

### STORY 04-12 · M3 · S · P1 — Integrity checksums and QA masking
- **Story**: As `AG`, I want records checksummed and low-quality scans flagged, so that downstream domains can trust or exclude bad data.
- **Deterministic / evidence**: compute a checksum per record on write and verify on read; deterministic QA rules flag low-quality scans (sparse points, out-of-range bands) with a reason code and mask.
- **Acceptance**:
  - Given a stored record, when read, then its checksum verifies; a tampered/corrupt record is detected and flagged.
  - Given a sparse/low-quality scan, when QA runs, then it is masked with a reason code and excluded from coverage, not counted as good.
- **Tests**: unit (checksum + QA rules), failure path (corrupt record detected), fixture.
- **Depends on**: 04-02, 04-09.

---

## M4 — Interactive

### STORY 04-13 · M4 · M · P0 — JSON/CSV export loading real session records
- **Story**: As `DSP`, I want session export to load and emit the real records, so that clients receive actual captured data, not an empty file.
- **Deterministic / evidence**: fix the `export_session` TODO to load records before exporting (currently passes an empty vec); JSON and CSV exports validate against a schema and round-trip provenance.
- **Acceptance**:
  - Given a captured session, when exported to CSV/JSON, then the export contains the real records with provenance and validates against the schema.
  - Given a session with no records, when exported, then a valid empty export is produced, not a crash or a stale buffer.
- **Tests**: schema validation, unit (record loading), failure path (empty session → valid empty export), fixture (`data_collector/src/export.rs`).
- **Depends on**: 04-09, 04-02.

### STORY 04-14 · M4 · M · P1 — One geospatial export with CRS/extent preserved
- **Story**: As `AG`, I want at least one geospatial export (GeoTIFF/KML/Shapefile) with correct CRS/extent, so that captures can be used in GIS tools.
- **Deterministic / evidence**: implement one geospatial export asserting CRS/extent/resolution and round-tripping a known coordinate; feature-gate the `unimplemented!` Parquet/HDF5 formats rather than shipping them broken.
- **Acceptance**:
  - Given a captured session, when exported geospatially, then the output carries correct CRS/extent and a known coordinate round-trips.
  - Given a request for a feature-gated format (Parquet/HDF5), when invoked, then it returns "not enabled" cleanly rather than panicking via `unimplemented!`.
- **Tests**: geospatial round-trip, failure path (gated format returns clean error), schema validation.
- **Depends on**: 04-13, 04-02.

### STORY 04-15 · M4 · S · P1 — Capture inspection and session listing API
- **Story**: As `OPS`, I want to list and inspect captured sessions with their freshness/coverage/aggregates, so that I can verify a flight before handing data downstream.
- **Deterministic / evidence**: paginated session listing with freshness, coverage, aggregates, and QA status surfaced; filterable by field/flight/date.
- **Acceptance**:
  - Given captured sessions, when listed, then they paginate and filter by field/flight/date with freshness/coverage/QA shown.
  - Given a session that failed capture, when inspected, then its collection failures and reduced coverage are visible, not hidden.
- **Tests**: API contract (pagination + filters), failure path (failed session surfaces failures), fixture.
- **Depends on**: 04-07, 04-09, 04-11, 04-12.

---

## M5 — Autonomous-Assist

### STORY 04-16 · M5 · S · P2 — Adaptive capture-gap re-fly recommendation
- **Story**: As `AG`, I want the system to recommend a re-fly of uncovered/low-quality areas, so that capture gaps are closed deterministically before analysis.
- **Deterministic / evidence**: from coverage (04-07) and QA masks (04-12), compute the uncovered/low-quality area and emit a re-fly recommendation (geometry + reason); advisory only, gated, links to a `01` mission.
- **Acceptance**:
  - Given a session with a coverage gap, when re-fly analysis runs, then it emits the gap geometry and a re-fly recommendation tied to the field.
  - Given full, high-quality coverage, when analysis runs, then no re-fly is recommended (no false re-flies).
- **Tests**: unit (gap geometry + recommendation), failure path (full coverage → no recommendation), fixture.
- **Depends on**: 04-07, 04-12, `01` (mission), `10` (field).

---

## Coverage note

~16 stories cover the 12 capabilities in `capability-map.md`, ordered by phase with the heaviest weight on M2 capture and M3 storage/indexing per `release-plan.md` (provenance and correctness over format breadth). The curated counts in `release-plan.md` (≈72 rows) expand several of these — per-sensor capture variants, per-format exports, additional QA rules, and per-index-type rebuilds — into sibling stories when implemented. Every captured record carries provenance and links to a flight (`01`) and field/scene (`10`); no session is "captured" without freshness, coverage, and a collection-failure path; storage/index/aggregate stories operate over persisted records and replace the `0.0` and `unimplemented!` placeholders.
