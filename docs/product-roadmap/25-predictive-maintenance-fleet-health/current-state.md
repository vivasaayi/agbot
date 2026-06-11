# Predictive Maintenance and Fleet Health: Current State and Target State

## Mission

Keep the fleet airworthy and predict failures before they ground a mission: register every component and airframe with its service history, accrue flight-hours/cycles/duty, derive telemetry-driven health indicators, detect degradation over the time-series, estimate remaining useful life with explicit uncertainty, drive work orders and parts, and — above all — gate dispatch with a hard deterministic pre-flight readiness check, reusing the fleet (`12`), telemetry (`01`), time-series (`28`), and alerting (`29`) subsystems.

## Current Maturity

greenfield pending (M0 named), overlapping domain `12`: no predictive-maintenance implementation exists; this domain is a product-vision module from `product-summary.md` (#20 Predictive Maintenance & Fleet Health). Domain `12` provides fleet enrollment and a basic health surface, but there is no component/airframe registry, health-indicator evaluator, readiness gate, anomaly detection, RUL estimate, or work-order model.

## What Exists Now

- Nothing is built for predictive maintenance. There is no `fleet_health` crate, component registry, health-indicator pipeline, readiness gate, or RUL evaluator.
- Adjacent surfaces it would build on, extend, and parallel (already partially real):
  - Domain `12` (fleet and edge operations): drone enrollment and a basic fleet health surface this domain extends with per-component health and service history.
  - Domain `01` (flight and mission control): the live telemetry stream (battery, motor/ESC, vibration) that health indicators derive from, and the dispatch path the readiness gate must guard.
  - Domain `04` (sensor acquisition): the session/duty records that flight-hours and cycle counts accrue from.
  - Domain `28` (time-series / change detection): the time-series subsystem that stores indicator history and supports trend/degradation detection.
  - Domain `29` (alerting/notification): the channel for degradation and maintenance-due alerts.
  - Domain `14` (autonomous tractor): ground-vehicle components whose health is folded into the same registry.
  - Domain `24` (regulatory/compliance): airworthiness/maintenance records the readiness gate and work orders feed.

## Gaps to Close

- No component/airframe registry with serials, install/removal history, and per-component service history.
- No flight-hours / cycles / duty accrual from `01`/`04` sessions.
- No telemetry-driven health indicators (battery cycle count and internal-resistance trend, motor vibration, ESC temperature) written to a time-series.
- No degradation/anomaly detection over the `28` time-series (trend break, drift, threshold crossing).
- No deterministic pre-flight readiness check that gates dispatch on a non-airworthy aircraft with reason codes.
- No predictive maintenance scheduling or remaining-useful-life (RUL) estimate with explicit uncertainty.
- No maintenance work-order or parts-tracking model.
- No fleet health dashboard or alerting via `29`.
- No retained health evidence/reason codes for reproducible, defensible verdicts.

## Related Existing Surfaces

- Domain `12` (fleet/edge): enrollment and basic health this domain extends with components and service history.
- Domain `01` (flight/mission control): telemetry source for health indicators; dispatch path the readiness gate guards.
- Domain `04` (sensor acquisition): session/duty records hours and cycles accrue from.
- Domain `28` (time-series/change detection): indicator history and trend/degradation detection.
- Domain `29` (alerting/notification): degradation and maintenance-due alert delivery.
- Domain `14` (autonomous tractor): ground-vehicle component health folded into the registry.
- Domain `24` (regulatory/compliance): airworthiness/maintenance records.
- `docs/reference/product-summary.md` (#20 Predictive Maintenance & Fleet Health): the source description for this module.

## Target Operating Model

- Safety is non-negotiable: the pre-flight readiness check is a hard, deterministic dispatch gate. An aircraft below an airworthiness threshold (overdue service, depleted battery health, an active critical health verdict) cannot be dispatched, with a reason code; on missing/stale health data, deny by default.
- Evidence before advice: deterministic threshold and trend checks run and are inspectable before any predictive/RUL output; every health verdict retains its raw evidence and reason code.
- Telemetry-driven indicators (battery resistance trend, vibration, ESC temperature) are computed deterministically and stored in the `28` time-series; degradation detection is trend-based and explainable.
- Predictive scheduling and RUL estimates flag explicit uncertainty (a range with confidence) and never override the hard readiness gate — they advise scheduling, not dispatch.
- Every health verdict ties to an action: schedule maintenance, open a work order, ground the aircraft, or clear for dispatch — not a dead-end metric.
- Work orders and parts close the loop; the fleet health dashboard and `29` alerts make degradation visible before it becomes a failure; airworthiness/maintenance records feed `24`.
