# Soil and IoT Sensor Network: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: device registry and reading routes or commands, persistence, auth scoped to org/field (via `10`), pagination, freshness, and audit IDs.
- Ingest: a gateway adapter (MQTT / LoRaWAN) behind a clear, mockable interface; ingest contract with freshness, coverage, and ingest-failure handling; simulation-first runtime mode.
- Deterministic: range checks, calibration-profile application, stuck/flatline detection, and threshold triggers computed without AI, with reason codes and raw readings retained.
- Geospatial: every reading carries CRS/position and a field/zone ref; soil products round-trip as GeoJSON.
- Storage: validated series persisted through the `28` time-series contract — no reinvented time-series store.
- Explainability: every QA flag and trigger cites the reading and the rule/threshold used.
- Agronomic: soil products tie to an irrigation trigger (`16`), a ground-truth comparison (`05`), or a sensor-health alert (`29`).
- Tests: unit (calibration/QA/threshold math), fixture (raw reading streams, calibration profiles), API contract, and one failure path (out-of-range, stuck sensor, ingest dropout).
- Operations: device health, battery/connectivity monitoring, retry/backoff on ingest, and a runbook.

## Category Epics

### EPIC-01: Device Identity and Ingest Foundation
- Goal: a geolocated device is registered and its readings flow through a gateway with freshness.
- First release: device registry/provisioning (sensor ID, geolocation, type, calibration profile, via `10`) and a mockable gateway ingest adapter (MQTT / LoRaWAN) with an ingest contract.
- Expansion: provisioning/config-version rollout and network coverage/gap reporting.
- Hardening: ingest retry/backoff, freshness/coverage tracking, and ingest-failure audit.

### EPIC-02: Deterministic Validation, Calibration, and Soil Products
- Goal: only validated, calibrated readings are trusted, and they become geolocated soil products.
- First release: reading validation (range checks + calibration-profile application) with reason-coded QA flags, stuck/flatline detection, and series persisted via `28`.
- Expansion: soil-moisture / EC / temperature products tied to field/zone with freshness and correct position.
- Hardening: reproducibility (same readings + profile → same series/flags), QA-mask retention, and large-network performance.

### EPIC-03: Health, Fusion, and Action
- Goal: the network monitors itself and drives real field actions.
- First release: sensor-health, battery, and connectivity monitoring with events emitted to `29`.
- Expansion: ground-truth fusion against aerial NDVI (`05`) and deterministic moisture-threshold irrigation triggers into `16`.
- Hardening: stale/missing-data gating (no triggers on stale data), fusion-mismatch handling, and export of soil products and QA history.
