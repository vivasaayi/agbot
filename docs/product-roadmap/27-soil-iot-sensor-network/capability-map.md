# Soil and IoT Sensor Network: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (data quality and geospatial correctness first, then operability, agronomic value, performance and scale, explainability) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. Data quality dominates: a ground sensor runs unattended for months, so an uncalibrated or stuck reading that is silently trusted is the core risk. Time-series storage is delegated to `28`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Soil and IoT Sensor Network Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Device registry and provisioning | missing (greenfield) | 8 | Register a geolocated sensor with type + calibration profile via `10` |
| Gateway ingest pipeline (MQTT / LoRaWAN) | missing (greenfield) | 9 | Ingest a reading through a mockable gateway adapter with freshness |
| Reading validation and calibration (deterministic QA) | missing (greenfield) | 9 | Range-check + calibrate a raw reading with reason-coded QA flags |
| Stuck-sensor / flatline detection | missing (greenfield) | 6 | Flag a flatlined series window deterministically |
| Geolocated readings tied to field/zone | missing (greenfield) | 7 | Attach CRS/position and field/zone ref to every reading |
| Time-series storage (delegated to `28`) | missing (greenfield) | 5 | Persist a validated reading series via the `28` contract |
| Soil-moisture / EC / temperature products | missing (greenfield) | 8 | Compute a zone soil-moisture summary with freshness |
| Sensor health, battery, connectivity monitoring (-> `29`) | missing (greenfield) | 7 | Detect a stale/low-battery device and emit a `29` event |
| Ground-truth fusion with aerial indices (`05`) | missing (greenfield) | 6 | Compare zone soil moisture against `05` NDVI at the same field |
| Irrigation trigger inputs (-> `16`) | missing (greenfield) | 6 | Emit a deterministic moisture-threshold trigger to `16` |
| Provisioning / firmware and config rollout | missing (greenfield) | 4 | Track device config version and a config push outcome |
| Network coverage and gap reporting | missing (greenfield) | 4 | Report which zones have no live sensor coverage |
