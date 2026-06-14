# Predictive Maintenance and Fleet Health: Detailed Stories

> Greenfield domain (M0 named) that overlaps domain `12`: no predictive-maintenance code exists yet. Every story below is **built from scratch** in a new `fleet_health` crate (or an extension of `12`) and is gated behind fleet enrollment (`12`), telemetry (`01`), session/duty records (`04`), the time-series subsystem (`28`), and alerting (`29`). An unairworthy aircraft must not fly, so the **safety pillar dominates every phase**: the pre-flight readiness check is a hard, deterministic dispatch gate, and predictive (RUL) outputs flag uncertainty and never override it. The readiness gate's hard-block is the earliest P0 once health indicators are real.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Safety / deterministic**: the guardrail or inspectable logic that must hold without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `FLEET-TECH` fleet technician, `OPS` operator, `DSP` drone service provider, `PA` platform admin, `AG` agronomist.

---

## M1 — Foundation

### STORY 25-01 · M1 · M · P0 — Component / airframe registry and service history
- **Story**: As `FLEET-TECH`, I want each component and airframe registered with serials, install/removal history, and service history, linked to the `12` fleet model, so that I always know what is installed and when it was last serviced.
- **Safety / deterministic**: persist `{component_id, type, serial, airframe_id?, installed_at, removed_at?, service_history[]}` linked to a `12`-enrolled aircraft; install/removal events are append-only; a component cannot be installed on two airframes at once.
- **Acceptance**:
  - Given an enrolled aircraft, when a component is installed, then a registry record exists with serial and install timestamp linked to the airframe.
  - Given a component already installed elsewhere, when installed again, then it is rejected (no double-install) and audited.
- **Tests**: unit (install/removal lifecycle), API contract (register/list/history), failure path (double-install rejected).
- **Depends on**: `12` (fleet enrollment model).

### STORY 25-02 · M1 · S · P0 — Flight-hours / cycles / duty tracking
- **Story**: As `FLEET-TECH`, I want flight-hours, cycles, and duty accrued per component and airframe from each session, so that service intervals are driven by real usage.
- **Safety / deterministic**: from each `01`/`04` session, deterministically accrue `{hours, cycles, duty}` to the installed components; accrual is idempotent per session (a re-ingested session does not double-count).
- **Acceptance**:
  - Given a completed session, when accrual runs, then the installed components' hours/cycles/duty increase by the session's measured amounts.
  - Given the same session re-ingested, when accrual runs again, then totals are unchanged (idempotent, no double-count).
- **Tests**: unit (accrual + idempotency), fixture (session record), failure path (duplicate session ignored).
- **Depends on**: 25-01, `01`, `04`.

---

## M2 — Captured / Observable

### STORY 25-03 · M2 · M · P0 — Telemetry-driven health indicators
- **Story**: As `FLEET-TECH`, I want battery internal-resistance, motor vibration, and ESC temperature derived from telemetry and stored in the `28` time-series, so that component health is observable over time.
- **Safety / deterministic**: compute each indicator deterministically from the `01` telemetry stream; write samples to `28` with `{component_id, indicator, value, ts}`; record telemetry gaps explicitly (a gap is not interpolated).
- **Acceptance**:
  - Given a telemetry stream, when indicators are derived, then resistance/vibration/ESC-temp samples persist in `28` with timestamps and component linkage.
  - Given a telemetry dropout, when derivation runs, then the gap is recorded and the indicator is marked stale (not back-filled with a fabricated value).
- **Tests**: unit (indicator derivation), fixture (telemetry trace), failure path (dropout → stale, not interpolated).
- **Depends on**: 25-01, `01`, `28`.

### STORY 25-04 · M2 · S · P1 — Battery cycle-count and internal-resistance trend
- **Story**: As `FLEET-TECH`, I want each battery pack's charge/discharge cycle count and internal-resistance trend tracked, so that I can retire packs before they fail.
- **Safety / deterministic**: increment cycle count per completed charge/discharge; maintain a resistance trend series in `28`; a pack over its cycle limit or above a resistance threshold is flagged `degraded` deterministically.
- **Acceptance**:
  - Given a pack completing a discharge cycle, when accrual runs, then its cycle count increments and a resistance sample is appended.
  - Given a pack over its cycle limit, when evaluated, then it is flagged `degraded` with the limit and measured value cited.
- **Tests**: unit (cycle counting + resistance threshold), fixture (battery telemetry), failure path (over-limit pack flagged).
- **Depends on**: 25-03, `28`.

---

## M3 — Explainable (the deterministic health core and the dispatch gate)

### STORY 25-05 · M3 · L · P0 — Pre-flight readiness check (gates dispatch)
- **Story**: As `OPS`, I want dispatch hard-blocked for any aircraft that is not airworthy, so that no unsafe aircraft flies.
- **Safety / deterministic**: the readiness evaluator checks overdue service intervals (25-02), battery health (25-04), and active critical health verdicts (25-06) against airworthiness thresholds; a failure returns a hard block with `{reason_code, component_ref}`; on missing/stale health data, deny by default; the gate runs in the `01`/`12` dispatch path and abort/deny is the default.
- **Acceptance**:
  - Given an aircraft within all airworthiness thresholds, when readiness is checked, then dispatch is permitted and the decision is recorded.
  - Given an aircraft with an overdue service interval or a critical health verdict, when checked, then dispatch is hard-blocked with a reason code citing the component.
  - Given missing or stale health data for the aircraft, when checked, then dispatch is denied by default (never cleared on uncertainty).
- **Tests**: unit (readiness evaluator incl. deny-on-stale), API contract (dispatch gate), failure path (overdue/critical blocks; stale denies).
- **Depends on**: 25-02, 25-04, 25-06, `01`, `12`.

### STORY 25-06 · M3 · M · P0 — Threshold health verdict per component
- **Story**: As `FLEET-TECH`, I want each component to carry a deterministic health verdict (`ok|watch|degraded|critical`) from its indicators, so that the readiness gate and dashboard have a clear state to act on.
- **Safety / deterministic**: map each indicator (25-03/25-04) to a verdict via configured thresholds; the worst indicator sets the component verdict; each verdict stores `{reason_code, indicator, threshold, value}`; a `critical` verdict is a hard input to the readiness gate.
- **Acceptance**:
  - Given indicators within thresholds, when evaluated, then the component verdict is `ok`.
  - Given an indicator above its critical threshold, when evaluated, then the component verdict is `critical` with the threshold and value cited.
- **Tests**: unit (threshold-to-verdict mapping), fixture (indicator sets), failure path (critical verdict gates dispatch).
- **Depends on**: 25-03, 25-04.

### STORY 25-07 · M3 · M · P1 — Degradation / anomaly detection over time-series
- **Story**: As `FLEET-TECH`, I want trend breaks and drift in a health indicator detected over the `28` time-series, so that I catch gradual degradation before it crosses a hard threshold.
- **Safety / deterministic**: a deterministic detector (slope change, drift, rate-of-change) over the indicator series raises a degradation event with `{reason_code, window, slope|delta}`; detection cites the series window it used; absent enough history it returns "insufficient history" (never a false trend).
- **Acceptance**:
  - Given a stable indicator series, when detection runs, then no degradation event is raised (no false positive).
  - Given a series with a sustained adverse slope, when detection runs, then a degradation event is raised citing the window and slope.
  - Given too little history, when detection runs, then it returns "insufficient history" rather than a fabricated trend.
- **Tests**: unit (slope/drift detection), fixture (degrading vs stable series), failure path (insufficient history surfaced).
- **Depends on**: 25-03, 25-04, `28`.

### STORY 25-08 · M3 · S · P1 — Health evidence retention and reproducibility
- **Story**: As `PA`, I want every health verdict and degradation event to retain its raw evidence and reason codes, so that a verdict can be re-derived and defended.
- **Safety / deterministic**: re-running a verdict/detector on the same series window yields an identical result; each verdict stores the indicator refs, thresholds/method, and reason code, append-only.
- **Acceptance**:
  - Given the same indicator inputs, when a verdict re-runs, then it produces an identical verdict and reason code.
  - Given a threshold/method version change, when re-run, then the new verdict records the new version while the prior verdict is retained.
- **Tests**: determinism (same input → same verdict hash), fixture, failure path (no in-place overwrite of a prior verdict).
- **Depends on**: 25-06, 25-07.

---

## M4 — Interactive (work orders, dashboard, alerts)

### STORY 25-09 · M4 · M · P1 — Maintenance work orders and parts tracking
- **Story**: As `FLEET-TECH`, I want to open and close work orders against a component with parts used, so that maintenance closes the loop and updates service history.
- **Safety / deterministic**: a work order persists `{wo_id, component_id, reason, opened_at, parts[], closed_at?, technician}`; closing a work order updates the component's service history (25-01) and can clear a `degraded`/`critical` verdict only after a recorded action; an open critical work order keeps the aircraft grounded.
- **Acceptance**:
  - Given a degraded component, when a work order is opened and closed with parts, then service history updates and the verdict can be re-evaluated.
  - Given an aircraft with an open critical work order, when dispatch is requested, then it stays blocked until the work order is closed.
- **Tests**: unit (work-order lifecycle + service-history update), API contract (open/close), failure path (open critical WO keeps aircraft grounded).
- **Depends on**: 25-01, 25-06, feeds `24`.

### STORY 25-10 · M4 · S · P1 — Fleet health dashboard and alerts
- **Story**: As `DSP`, I want a fleet health view with per-aircraft readiness and degradation alerts delivered via `29`, so that I see problems before they ground a mission.
- **Safety / deterministic**: the dashboard surfaces each aircraft's readiness verdict and worst-component state from deterministic verdicts; a new `degraded`/`critical` verdict or degradation event emits a `29` alert citing the component and evidence; a stale-data aircraft is shown as "unknown/blocked", not "ok".
- **Acceptance**:
  - Given a fleet, when the dashboard loads, then each aircraft shows its readiness verdict and worst-component state.
  - Given a component crossing into `critical`, when evaluated, then a `29` alert fires citing the component and reason code.
  - Given an aircraft with stale health data, when shown, then it renders as "unknown/blocked" (never "ok").
- **Tests**: unit (dashboard state derivation), integration (`29` alert delivery), failure path (stale aircraft not shown ready).
- **Depends on**: 25-05, 25-06, 25-07, `29`.

### STORY 25-11 · M4 · S · P2 — Tractor / ground-vehicle health integration
- **Story**: As `FLEET-TECH`, I want tractor component health from `14` folded into the same registry and dashboard, so that I manage ground and air fleet health in one place.
- **Safety / deterministic**: ingest `14` ground-vehicle component telemetry/verdicts into the registry using the same indicator/verdict contract; ground-vehicle readiness is gated by the equivalent of 25-05 before a field-ops session.
- **Acceptance**:
  - Given a tractor component, when its health is ingested, then it appears in the registry and dashboard with a verdict.
  - Given a tractor with an overdue service interval, when a field-ops session is requested, then it is blocked the same way an aircraft is.
- **Tests**: integration (`14` ingest), unit (shared verdict contract), failure path (overdue tractor blocked).
- **Depends on**: 25-06, 25-05, `14`.

---

## M5 — Autonomous-Assist (gated, uncertainty-flagged, never overrides the gate)

### STORY 25-12 · M5 · M · P1 — Trend-based remaining-useful-life (RUL) estimate
- **Story**: As `FLEET-TECH`, I want a remaining-useful-life estimate per component with an explicit confidence range, so that I can schedule maintenance ahead of failure — without it ever clearing an unairworthy aircraft.
- **Safety / deterministic**: RUL is derived only from the deterministic indicator trends (25-07); every output is a range with a confidence band and cites the series it used; it is feature-flagged and can never override the readiness gate (25-05) or clear a `critical` verdict.
- **Acceptance**:
  - Given a degrading trend with sufficient history, when RUL runs, then it returns a range with a confidence band citing the trend evidence.
  - Given insufficient or stale history, when RUL is requested, then it is unavailable (never fabricated).
  - Given a request to use RUL to clear a blocked dispatch, when made, then it is refused — the readiness gate's hard block stands.
- **Tests**: unit (RUL range + uncertainty), gating test (cannot override readiness gate), failure path (insufficient history → unavailable).
- **Depends on**: 25-07, 25-08, 25-05.

### STORY 25-13 · M5 · S · P2 — Predictive maintenance scheduling
- **Story**: As `DSP`, I want maintenance scheduled ahead of predicted end-of-life with explicit uncertainty, so that I plan downtime instead of reacting to failures.
- **Safety / deterministic**: scheduling proposes a window from the RUL range (25-12); proposals always render the uncertainty and require `FLEET-TECH` approval to become a work order; a proposal never auto-clears a verdict or auto-dispatches.
- **Acceptance**:
  - Given an RUL range, when scheduling runs, then a maintenance window is proposed with the uncertainty shown and requires approval.
  - Given an approved proposal, when accepted, then a work order (25-09) is opened; the readiness gate remains the only dispatch authority.
- **Tests**: unit (window proposal from range), presentation test (uncertainty always shown), failure path (proposal cannot auto-dispatch).
- **Depends on**: 25-12, 25-09.

---

## Coverage note

These 13 stories cover all 11 capabilities in `capability-map.md` (~1+ stories each, with the health core split across 25-05/25-06/25-08). The breakdown is safety- and operability-led, with a heavy M3 deterministic core (readiness gate, threshold verdicts, degradation detection) reflecting that **the readiness gate's hard-block leads** in `release-plan.md`. The registry, duty tracking, health indicators, and readiness gate are P0; degradation detection, work orders, dashboard, and RUL build on them. The M5 stories (RUL, predictive scheduling) stay uncertainty-flagged and can never override the hard dispatch gate. The curated counts in `release-plan.md` (~76 rows) expand several of these (per-indicator variants, per-component-type service intervals, additional work-order/parts flows) into sibling stories when implemented.
