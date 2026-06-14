# Predictive Maintenance and Fleet Health: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (safety first, then operability, explainability and trust, data quality, performance and scale) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named) that overlaps domain `12`, every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. The safety pillar dominates: the pre-flight readiness check is a hard dispatch gate, and predictive (RUL) outputs flag uncertainty and never override it. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Predictive Maintenance and Fleet Health Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Component / airframe registry and service history | missing (greenfield) | 8 | Register a component/airframe linked to the `12` fleet model |
| Flight-hours / cycles / duty tracking | missing (greenfield) | 7 | Accrue hours/cycles from `01`/`04` sessions |
| Telemetry-driven health indicators | missing (greenfield) | 9 | Battery resistance / vibration / ESC temp into `28` |
| Degradation / anomaly detection over time-series | missing (greenfield) | 8 | Trend-break detection on a health indicator (`28`) |
| Pre-flight readiness check (gates dispatch) | missing (greenfield) | 8 | Hard block on a non-airworthy aircraft with reason codes |
| Predictive maintenance scheduling and RUL | missing (greenfield) | 8 | Trend-based RUL estimate with explicit uncertainty |
| Maintenance work orders and parts tracking | missing (greenfield) | 7 | Open/close a work order against a component |
| Fleet health dashboard and alerts (via `29`) | missing (greenfield) | 6 | Surface fleet health state and raise a `29` alert |
| Battery cycle-count and resistance trend | missing (greenfield) | 6 | Track cycle count + internal-resistance trend per pack |
| Health evidence retention and reproducibility | missing (greenfield) | 5 | Persist raw evidence and reason codes per health verdict |
| Tractor/ground-vehicle health integration (via `14`) | missing (greenfield) | 4 | Ingest a tractor component's health into the registry |
