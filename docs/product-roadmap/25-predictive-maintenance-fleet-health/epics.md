# Predictive Maintenance and Fleet Health: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: component/health route or command, persistence, auth scoped to org/fleet (via `12`), pagination, and audit events.
- Safety: the pre-flight readiness check hard-blocks dispatch of a non-airworthy aircraft with a reason code — no dispatch mutation bypasses it; on missing/stale health data, deny by default.
- Deterministic: health-indicator math, threshold verdicts, trend/degradation detection, and RUL computed without AI, with reason codes and raw evidence retained.
- Telemetry/time-series: indicator history written to `28` with freshness, gaps, and trend support.
- Explainability: every health verdict cites its evidence; predictive/RUL outputs flag explicit uncertainty and never override the readiness gate.
- Tests: unit (indicator/trend/RUL math), fixture (telemetry traces, battery cycles), API contract, and one failure path (readiness gate blocks).
- Operations: alert delivery via `29`, fleet-health dashboard, and a runbook.

## Category Epics

### EPIC-01: Component Inventory and Duty Tracking
- Goal: every component and airframe is a registered entity with service history and accrued duty.
- First release: component/airframe registry (via `12`) and flight-hours / cycles / duty tracking accrued from `01`/`04` sessions.
- Expansion: install/removal history and battery cycle-count/resistance tracking per pack.
- Hardening: tractor/ground-vehicle component integration (via `14`) and service-history audit.

### EPIC-02: Health Indicators and the Readiness Gate
- Goal: derive deterministic health from telemetry and gate dispatch on airworthiness.
- First release: telemetry-driven health indicators (battery resistance, vibration, ESC temperature) into `28`, and the pre-flight readiness check that hard-blocks a non-airworthy aircraft.
- Expansion: degradation/anomaly detection over the time-series (trend break, drift).
- Hardening: evidence retention, reproducibility, and full negative-path tests (readiness gate blocks; deny on stale data).

### EPIC-03: Predictive Scheduling, Work Orders, and Fleet Visibility
- Goal: predict maintenance and close the loop on it.
- First release: trend-based RUL estimate with explicit uncertainty and maintenance scheduling.
- Expansion: maintenance work orders / parts tracking and a fleet health dashboard with `29` alerts.
- Hardening: RUL calibration tests, airworthiness/maintenance records to `24`, and export.
