# Predictive Maintenance and Fleet Health: Release Plan

## Shipment Strategy

Ship in maturity order with safety leading every phase. The component/airframe registry and duty tracking (M1) come first, then telemetry-driven health indicators captured into the `28` time-series (M2), then the deterministic threshold health state plus the pre-flight readiness gate and degradation detection (M3), then interactive work orders, the fleet dashboard, and alerting (M4). Predictive scheduling and RUL (M5) are gated: they flag explicit uncertainty and never override the hard readiness gate. Because an unairworthy aircraft must not fly, the readiness gate's hard-block is sequenced as the first P0 the moment health indicators are real.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 17 |
| M2 captured | 16 |
| M3 explainable | 22 |
| M4 interactive | 14 |
| M5 autonomous-assist | 7 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 24 |
| P1 | 33 |
| P2 | 19 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 12 |
| M | 40 |
| S | 24 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Component / airframe registry and service history | operability | identity |
| M1 foundation | S | Flight-hours / cycles / duty tracking | operability | capture |
| M2 captured | M | Telemetry-driven health indicators | data quality | capture |
| M2 captured | S | Battery cycle-count and resistance trend | data quality | capture |
| M3 explainable | L | Pre-flight readiness check (gates dispatch) | safety | evaluator |
| M3 explainable | M | Degradation / anomaly detection over time-series | explainability and trust | evaluator |
| M4 interactive | M | Maintenance work orders and parts tracking | operability | operations |
| M4 interactive | S | Fleet health dashboard and alerts (via `29`) | operability | interaction |

## Execution Rules

- The pre-flight readiness check is a hard dispatch gate: an aircraft below airworthiness thresholds (overdue service, depleted battery health, active critical verdict) cannot be dispatched, with a reason code; on missing or stale health data, deny by default — never clear for dispatch on uncertainty.
- Deterministic threshold and trend checks must run and be inspectable before any predictive/RUL output is enabled.
- Health indicators must be written to the `28` time-series with freshness and gap tracking; a stale indicator is treated as missing data by the readiness gate.
- Every health verdict must cite its evidence and reason code and tie to an action: schedule maintenance, open a work order, ground, or clear.
- Predictive scheduling and RUL (M5) must flag explicit uncertainty (a range with confidence) and never override the readiness gate; they advise scheduling, not dispatch.
- Work orders and airworthiness/maintenance records must be reproducible and feed `24`; degradation alerts must deliver via `29`.
