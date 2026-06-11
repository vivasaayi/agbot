# Soil and IoT Sensor Network: Current State and Target State

## Mission

Run a persistent ground-based sensor network — soil moisture, soil temperature, EC, and micro weather stations — that ground-truths aerial drone capture: register and geolocate every device, ingest readings through a gateway, validate and calibrate them deterministically, tie them to a field/zone, and turn trustworthy series into soil products that feed irrigation (`16`), fuse with aerial indices (`05`), and raise sensor-health and threshold alerts (`29`).

## Current Maturity

greenfield pending (M0 named): no implementation exists. There is no `soil_iot`/`sensor_network` crate, device registry, gateway adapter, calibration/QA evaluator, or soil-product pipeline. The capability is named in the AGBot mission ("multi-sensor data acquisition") but is not yet built. Time-series storage for sensor series is owned by domain `28`, not this domain.

## What Exists Now

- Nothing is built for this domain. There is no ground-sensor device model, MQTT/LoRaWAN ingest, reading validation, or soil-moisture/EC/temperature product.
- Adjacent surfaces it would build on and parallel (already partially real):
  - Domain `04` (sensor acquisition): the capture-provenance model — `FlightDataRecord` provenance (sensor, GPS, timestamp, calibration), freshness/coverage tracking, indexing, and collection-failure handling — directly reused for ground-sensor readings (`data_collector/src/lib.rs`, `indexing.rs`).
  - Domain `10` (field/farm/data): the Organization/Farm/Field/Zone model a device and its readings are tied to.
  - Domain `07` (GIS hub): geo serving and CRS/extent contracts for geolocated readings.
  - Domain `28` (time-series and change detection): the reading-series storage and query subsystem this domain delegates to — it must not reinvent time-series storage.
  - Domain `05` (imagery / remote sensing): aerial NDVI and other indices this domain ground-truths against.
  - Domains `16`/`15`/`29` (water management / weather / alerting): downstream consumers of soil products, micro-weather, and sensor-health events.

## Gaps to Close

- No device identity/registry: sensor ID, geolocation, type (moisture/temperature/EC/weather), and calibration profile, owned by an org and tied to a field/zone via `10`.
- No gateway ingest pipeline: MQTT / LoRaWAN is a true external boundary with no adapter, no ingest contract, and no freshness/ingest-failure handling.
- No deterministic reading validation or calibration: no range checks, no calibration-profile application, no reason-coded QA flags.
- No stuck-sensor / flatline detection over a reading window.
- No geolocated reading model carrying CRS/position and a field/zone ref.
- No delegation to `28` for time-series storage of validated series.
- No soil-moisture / EC / temperature products with freshness tied to field/zone.
- No sensor-health, battery, or connectivity monitoring, and no events emitted to `29`.
- No ground-truth fusion of ground sensors against aerial indices from `05`.
- No irrigation-trigger inputs to `16`, and no network coverage/gap reporting.

## Related Existing Surfaces

- Domain `04` (sensor acquisition): capture-provenance, freshness/coverage, indexing, and collection-failure patterns to reuse for ground-sensor readings.
- Domain `10` (field/farm/data): org/farm/field/zone model that owns devices and readings.
- Domain `07` (GIS hub): geo serving and CRS/extent contracts for geolocated readings.
- Domain `28` (time-series / change detection): the delegated time-series storage and query subsystem for reading series.
- Domain `05` (imagery / remote sensing): aerial indices for ground-truthing.
- Domains `16`/`15`/`29` (water / weather / alerting): downstream consumers of soil products, micro-weather, and sensor-health events.

## Target Operating Model

- Every device is a registered, geolocated entity with a type and calibration profile, owned by an org and tied to a field/zone via `10`; no reading is accepted from an unregistered device.
- Readings arrive through a gateway adapter (MQTT / LoRaWAN) behind a clear interface that is mockable for hardware-free runs (`RUNTIME_MODE=SIMULATION`); the ingest contract tracks freshness, coverage, and ingest failures.
- Evidence before advice: deterministic validation, calibration, range checks, and stuck/flatline detection run and are inspectable before any reading is trusted; QA-flagged readings are retained with reason codes, never silently dropped.
- Validated reading series are stored through the `28` time-series contract — this domain does not reinvent time-series storage.
- Soil-moisture / EC / temperature products carry CRS/position and freshness, tie to a field/zone, and round-trip geospatially.
- Soil products ground-truth aerial NDVI from `05`, feed deterministic moisture-threshold irrigation triggers into `16`, and raise sensor-health, battery, connectivity, and threshold-breach events through `29`.
- Reproducible outputs: the same raw readings and calibration profile produce the same validated series and QA flags, with tests on the calibration/QA math and at least one failure path.
