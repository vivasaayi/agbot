# Soil and IoT Sensor Network: Release Plan

## Shipment Strategy

Ship in maturity order with a data-quality-before-advice discipline. Device identity and a mockable gateway ingest pipeline (M1) come first, then observable reading capture with freshness, coverage, and ingest-failure states (M2), then the deterministic validation/calibration/QA core and soil products (M3), then interactive monitoring, alerts, fusion, and irrigation triggers (M4). Predictive/advisory soil behavior (M5) stays gated behind trustworthy validated series and explicit uncertainty. The data-quality and geospatial-correctness pillars lead every phase: an unattended sensor's reading is worthless until it is range-checked, calibrated, and proven fresh. Time-series storage is delegated to `28` and is not re-implemented here.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 20 |
| M2 captured | 17 |
| M3 explainable | 22 |
| M4 interactive | 15 |
| M5 autonomous-assist | 5 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 23 |
| P1 | 33 |
| P2 | 23 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 11 |
| M | 41 |
| S | 27 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Device registry and provisioning | operability | identity |
| M1 foundation | M | Gateway ingest pipeline (MQTT / LoRaWAN) | data quality | ingest |
| M2 captured | M | Geolocated readings tied to field/zone | geospatial correctness | capture |
| M2 captured | S | Time-series storage (delegated to `28`) | operability | storage |
| M3 explainable | M | Reading validation and calibration (deterministic QA) | data quality | evaluator |
| M3 explainable | S | Stuck-sensor / flatline detection | data quality | evaluator |
| M3 explainable | M | Soil-moisture / EC / temperature products | agronomic value | evaluator |
| M4 interactive | M | Sensor health, battery, connectivity monitoring (-> `29`) | operability | interaction |
| M4 interactive | S | Irrigation trigger inputs (-> `16`) | agronomic value | operations |

## Execution Rules

- This domain is gated behind the core platform (`01`–`12`) and the field/zone model in `10`; readings are only accepted from a registered, geolocated device.
- The MQTT / LoRaWAN gateway is a true external boundary: it must sit behind a clear, mockable interface and run hardware-free under `RUNTIME_MODE=SIMULATION`.
- Deterministic validation, calibration, and stuck/flatline detection must run and be inspectable before any reading is trusted; QA-flagged readings are retained with reason codes, never silently dropped.
- Time-series storage is delegated to `28`; this domain must not re-implement a time-series store.
- Every geolocated reading and soil product must assert CRS/position and a field/zone ref.
- Irrigation triggers (`16`) and sensor-health alerts (`29`) must cite the reading and rule/threshold; no trigger or alert fires on stale or missing data.
