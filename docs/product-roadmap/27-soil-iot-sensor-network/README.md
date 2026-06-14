# Soil and IoT Sensor Network

A persistent ground-based sensor network — soil moisture, soil temperature, electrical conductivity (EC), and micro weather stations — that complements aerial drone capture and ground-truths it, with deterministic calibration, QA, and freshness before any reading is trusted.

## Where We Are

- Not started / vision only. This is a greenfield domain (M0 named) sourced from the AGBot mission's "multi-sensor data acquisition" scope; no code, crate, device registry, or ingest pipeline exists yet.
- The patterns it reuses are partially real: capture provenance and the sensor-record/freshness model from `04`, field/zone context from `10`, and the geo serving in `07`. It will fuse with aerial indices from `05` for ground-truthing.
- A ground sensor network runs unattended for months on battery and intermittent links, so data quality, geospatial correctness, and operability dominate this domain: a stuck or uncalibrated sensor that is silently trusted is worse than a missing reading.

## Where We Should Be

- Every device is a registered, geolocated entity with a type and calibration profile, owned by an org and tied to a field/zone via `10`.
- Readings arrive through a gateway (MQTT / LoRaWAN) and pass deterministic validation — range checks, stuck/flatline detection, calibration — before being trusted; QA-flagged readings are retained, never dropped silently.
- Time-series storage is delegated to `28`; this domain owns device identity, ingest, calibration, QA, and the soil products.
- Soil-moisture / EC / temperature products carry CRS/position and freshness, ground-truth aerial NDVI from `05`, feed irrigation triggers into `16`, and raise sensor-health and threshold-breach alerts through `29`.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0/P1 slices.
- `stories.md`: detailed vertical-slice stories by release phase.

## Build Order

1. Device registry and provisioning: register a geolocated sensor with type and calibration profile, linked to field/zone via `10`.
2. Ingest pipeline via a gateway adapter (MQTT / LoRaWAN as a mockable external boundary) with freshness and ingest-failure handling.
3. Deterministic reading validation, calibration, and QA masks (out-of-range, stuck/flatline) before a reading is trusted; persist series via `28`.
4. Soil-moisture / EC / temperature products tied to field/zone with correct position and freshness.
5. Sensor health, battery, and connectivity monitoring routed to `29` alerts.
6. Ground-truth fusion with aerial indices (`05`) and irrigation trigger inputs to `16`.

## Primary Crates

New crate `soil_iot` (a.k.a. `sensor_network`). Reuses capture-provenance and sensor-record patterns from `04`; builds on `10` (field/zone) and the time-series subsystem in `28` (delegated storage); feeds `16` (irrigation), `29` (alerts), and fuses with `05` (aerial indices). Geo serving through `07`; surfaced through `11` and `13`.
