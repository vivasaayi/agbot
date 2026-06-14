# Predictive Maintenance and Fleet Health

Component and airframe health for drones (and tractors from `14`): service-history registry, flight-hours/cycles tracking, telemetry-driven health indicators, degradation/anomaly detection over time-series, predictive maintenance scheduling with remaining-useful-life (RUL), work orders/parts, a pre-flight readiness gate, and a fleet health dashboard.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#20 Predictive Maintenance & Fleet Health); it overlaps domain `12` (fleet/edge) and would either extend `12` or land as a new `fleet_health` crate.
- The surfaces it would build on are partially real: telemetry (`01`), capture/session records (`04`), fleet enrollment/health (`12`), the time-series subsystem (`28`) for trends, and alerting (`29`).
- An unairworthy aircraft must not fly, so the safety pillar dominates: the pre-flight readiness check is a hard, deterministic dispatch gate.

## Where We Should Be

- Every component and airframe is a registered entity with service history, flight-hours/cycles/duty tracking, and a deterministic health state.
- Telemetry-driven health indicators (battery cycle count and internal-resistance trend, motor vibration, ESC temperature) feed degradation/anomaly detection over the `28` time-series.
- Predictive maintenance scheduling produces a remaining-useful-life (RUL) estimate with explicit uncertainty; work orders and parts close the loop.
- A pre-flight readiness check **gates dispatch**: an aircraft below airworthiness thresholds cannot be dispatched, with a reason code — RUL/predictive outputs never override this hard gate.
- A fleet health dashboard and alerts (via `29`) make degradation visible before it becomes a failure.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0/P1 slices.

## Build Order

1. Component/airframe registry with service history, linked to the `12` fleet model.
2. Flight-hours / cycles / duty tracking accrued from `01`/`04` sessions.
3. Telemetry-driven health indicators (battery, vibration, ESC temperature) into the `28` time-series.
4. Deterministic threshold health state plus the pre-flight readiness gate that blocks dispatch.
5. Degradation/anomaly detection over the time-series; maintenance work orders and parts tracking.
6. Predictive scheduling and RUL with explicit uncertainty; fleet health dashboard and alerts via `29`.

## Primary Crates

New crate `fleet_health` (or an extension of `12`): component/airframe registry, health-indicator evaluators, readiness gate, work-order model. Builds on `01` (telemetry), `04` (sessions), `12` (enrollment/health), `28` (time-series trends), and `29` (alerting). Ties to `24` (airworthiness/maintenance compliance) and consumes tractor health from `14`.
