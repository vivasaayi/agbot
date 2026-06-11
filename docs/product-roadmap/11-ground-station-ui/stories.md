# Ground Station UI: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. This is the operator's live operations console (distinct from the post-flight advisor viewer in domain `08`): a trustworthy receive-only console first, then deterministic map rendering, then guarded operator actions. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence** (or **Safety** / **Operability** where it fits): what must be enforced and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation (trustworthy transport)

### STORY 11-01 · M1 · S · P0 — WebSocket client with reconnect and state
- **Story**: As `OPS`, I want the console's link to `mission_control` to reconnect with backoff and surface its connection state, so that a dropped link is visible and recovers instead of silently dying.
- **Operability**: extend the existing client (`ground_station_ui/src/lib.rs`) with a state machine `Connecting→Connected→Stale→Lost→Reconnecting` and exponential backoff; current state is exposed to both surfaces, never inferred.
- **Acceptance**:
  - Given a live connection, when `mission_control` drops, then the client transitions to `Lost` and begins backoff reconnection, surfacing each state.
  - Given the server is unreachable, when reconnection is attempted, then backoff grows up to a cap and the UI shows `Lost`, not a frozen "Connected".
- **Tests**: unit (state machine + backoff), integration (drop/recover against a stub server), failure path (server down → bounded backoff, `Lost` surfaced).
- **Depends on**: `01` (mission_control WS feed), existing dispatch in `lib.rs`.

### STORY 11-02 · M1 · S · P0 — Typed message dispatch with validation
- **Story**: As `OPS`, I want all six `WebSocketMessage` variants parsed and validated before display, so that a malformed frame never corrupts the console state.
- **Deterministic / evidence**: dispatch telemetry, mission status, LiDAR update, image captured, NDVI processed, and system status via the typed `shared::WebSocketMessage`; unparseable frames are counted and dropped, not rendered as garbage.
- **Acceptance**:
  - Given a valid frame of each variant, when received, then it routes to the correct handler with typed fields.
  - Given a malformed/unknown frame, when received, then it is rejected, the error counter increments, and prior state is preserved.
- **Tests**: unit (per-variant dispatch), failure path (malformed frame → dropped + counted), fixture (recorded message stream).
- **Depends on**: 11-01, `shared/src/schemas.rs` (WebSocketMessage).

### STORY 11-03 · M1 · S · P1 — Shared client state model (web/CLI parity)
- **Story**: As `DSP`, I want one client-side state model shared by the web and CLI surfaces, so that both show the same truth instead of diverging demo scripts.
- **Operability**: a single in-memory session state (latest telemetry, link state, event buffer) feeds both `web_server.rs` and `cli_interface.rs`; replaces the inline browser demo script.
- **Acceptance**:
  - Given a telemetry update, when it is applied, then both the web dashboard and the CLI `status` read the same values.
  - Given the state has never received telemetry, when either surface reads it, then it reports "no data," not a hardcoded default.
- **Tests**: unit (shared state mutation), parity test (web vs CLI read same snapshot), failure path (no data → "no data").
- **Depends on**: 11-02.

---

## M2 — Captured / Observable (live, trustworthy data)

### STORY 11-04 · M2 · M · P0 — Live telemetry binding with freshness
- **Story**: As `OPS`, I want the dashboard tiles bound to live telemetry with a freshness age, so that I am reading the aircraft now, not a stale snapshot.
- **Deterministic / evidence**: bind position, battery, mode/armed, ground/air speed, heading, and relative altitude to the shared state; each tile shows the age since last update; values older than a threshold are marked stale.
- **Acceptance**:
  - Given a stream of telemetry, when it arrives, then every bound tile updates and shows a last-update age.
  - Given telemetry stops for longer than the staleness threshold, when the tiles render, then they are visibly marked stale rather than showing the last value as current.
- **Tests**: unit (binding + age computation), integration (live stream), failure path (gap > threshold → stale marking).
- **Depends on**: 11-03; replaces static `web_server.rs` telemetry page.

### STORY 11-05 · M2 · S · P0 — Connection and link-health indicators
- **Story**: As `OPS`, I want a clear connected/stale/lost indicator with the last-update age, so that I never act on a dead link believing it is live.
- **Operability**: surface the 11-01 link state plus telemetry-gap detection as a single health indicator on both surfaces; the CLI `status` reflects real link/telemetry state instead of the hardcoded "Connected"/"Receiving".
- **Acceptance**:
  - Given a healthy link, when status is read, then it shows `Connected` with a fresh age.
  - Given the feed has gone silent, when status is read, then it shows `Stale` or `Lost` with the real age, never a hardcoded "Connected".
- **Tests**: unit (health derivation), CLI test (`status` reflects state), failure path (silent feed → Stale/Lost).
- **Depends on**: 11-01, 11-04.

### STORY 11-06 · M2 · M · P0 — Capture event timeline (LiDAR/image/NDVI)
- **Story**: As `AG`, I want LiDAR, image-captured, and NDVI-processed events collected into an ordered, filterable feed, so that I can see what the aircraft captured as it happened.
- **Deterministic / evidence**: append capture events to a time-ordered buffer with `{type, timestamp, summary}` (e.g. LiDAR scan count, mean NDVI / vegetation %); the feed is filterable by type and bounded with a retention limit.
- **Acceptance**:
  - Given a sequence of capture events, when they arrive, then they appear in timestamp order and are filterable by type.
  - Given the buffer reaches its limit, when more events arrive, then the oldest are evicted deterministically (no unbounded growth, no loss of ordering).
- **Tests**: unit (ordering + filter + eviction), fixture (recorded capture stream), failure path (buffer overflow → bounded eviction).
- **Depends on**: 11-02.

### STORY 11-07 · M2 · S · P1 — System alerts and status panel
- **Story**: As `OPS`, I want system-status events ranked by severity in an alert panel, so that I can triage problems by importance during a flight.
- **Deterministic / evidence**: map system-status events to a severity (info/warn/critical); the panel sorts by severity then recency; unacknowledged critical alerts are visually distinct.
- **Acceptance**:
  - Given mixed-severity status events, when rendered, then critical alerts sort above warnings and info.
  - Given an unknown severity value, when received, then it is classified as `warn` (fail-safe), not dropped silently.
- **Tests**: unit (severity ranking + fallback), failure path (unknown severity → warn).
- **Depends on**: 11-02.

---

## M3 — Explainable (correct map and mission overlays)

### STORY 11-08 · M3 · L · P0 — Map rendering: basemap, position, flight path
- **Story**: As `OPS`, I want a real basemap with the drone's live position and flight path, so that I can see where the aircraft is in the field — the current maps page is empty placeholders.
- **Deterministic / evidence**: replace the three `.map-placeholder` divs with a real rendering engine; assert the basemap and overlay CRS/extent match before drawing; plot position from telemetry and accumulate the path; a wrong-CRS overlay is refused, not drawn misaligned.
- **Acceptance**:
  - Given live telemetry, when the map renders, then the drone marker and accumulated path appear at the correct geographic position in the asserted CRS.
  - Given an overlay whose CRS/extent does not match the basemap, when rendering is attempted, then it is refused with an error rather than drawn misaligned.
- **Tests**: unit (CRS/extent assertion + path accumulation), geospatial round-trip (telemetry → map coordinate), failure path (CRS mismatch → refused).
- **Depends on**: 11-04; basemap source via `07`/`08` conventions.

### STORY 11-09 · M3 · M · P0 — Mission overlay: waypoints, geofence, no-fly zones
- **Story**: As `OPS`, I want the mission's waypoints, geofence, and no-fly zones drawn on the map, so that I can judge whether the aircraft is inside its safe envelope.
- **Deterministic / evidence**: render waypoints, geofence polygon, and no-fly zones from mission status in the asserted CRS; the drone position is checked against the geofence and breaches are visually flagged deterministically.
- **Acceptance**:
  - Given a mission with a geofence and no-fly zones, when rendered, then all are drawn in the correct CRS aligned with the basemap.
  - Given the drone position falls outside the geofence, when rendered, then a breach is flagged; given missing geofence data, the overlay is omitted rather than drawn at a default location.
- **Tests**: unit (geofence containment check), geospatial (overlay alignment), failure path (missing geofence → omitted, not faked).
- **Depends on**: 11-08, `01` (mission status / geofence).

### STORY 11-10 · M3 · S · P1 — Capture markers on the map
- **Story**: As `AG`, I want capture events plotted as markers at their geolocation, so that I can see coverage and gaps spatially, not just as a list.
- **Deterministic / evidence**: place a marker per capture event at its position in the map CRS; markers link back to the 11-06 timeline entry; events without a position are listed but not mislocated.
- **Acceptance**:
  - Given geolocated capture events, when rendered, then markers appear at the correct positions and select the corresponding timeline entry.
  - Given a capture event without position data, when rendered, then it stays in the timeline and is excluded from the map (no marker at 0,0).
- **Tests**: unit (marker placement + linkage), failure path (no-position event → not plotted).
- **Depends on**: 11-06, 11-08.

---

## M4 — Interactive (guarded operator actions)

### STORY 11-11 · M4 · S · P0 — Operator auth and session
- **Story**: As `PA`, I want an authenticated, role-bound session before any action route is reachable, so that only authorized operators can command the aircraft.
- **Safety**: a login/session gate fronts the action API; operator role and identity resolve from domain `10`; no action route is reachable without an authenticated session.
- **Acceptance**:
  - Given valid operator credentials, when they log in, then a session is established and action routes become reachable.
  - Given no or expired session, when an action route is called, then it is rejected with `401` and nothing is sent to `mission_control`.
- **Tests**: unit (session gating), API contract (login/expiry), failure path (no session → 401, no action dispatched).
- **Depends on**: `10` (User/role model).

### STORY 11-12 · M4 · M · P0 — Operator actions: dispatch, pause, RTH, abort
- **Story**: As `OPS`, I want to send dispatch, pause, return-to-home, and abort commands that route only through `mission_control` guardrails, so that I can act on the aircraft safely.
- **Safety**: the UI never commands the vehicle directly; every action is forwarded to `mission_control` (domain `01`), which applies its guardrails and returns an ack; actions are disabled until the full loop passes in `Simulation` mode against domain `02`.
- **Acceptance**:
  - Given an authenticated operator in a validated session, when they issue RTH or abort, then the command is forwarded to `mission_control` and the returned ack is shown.
  - Given `mission_control` rejects the command (guardrail) or does not ack, when the operator acts, then the UI shows the rejection/timeout and does not report success.
- **Tests**: integration (action → ack against `01` stub), simulation gate (disabled until `02` loop passes), failure path (guardrail reject / no ack → surfaced, not faked success).
- **Depends on**: 11-11, `01` (action path + guardrails), `02` (simulation validation).

### STORY 11-13 · M4 · S · P0 — Operator action audit
- **Story**: As `PA`, I want every operator action logged with who, what, when, and the resulting ack, so that commands to the aircraft are accountable.
- **Deterministic / evidence**: append an immutable audit record `{operator_id, action, target_mission, request_at, ack/result}` per attempt (including rejected ones); records are queryable and write before the action is reported complete.
- **Acceptance**:
  - Given any action attempt, when it is sent, then an audit record is written with operator, action, and the ack/result.
  - Given an action whose audit write fails, when it is attempted, then the action is not reported complete (audit-before-confirm).
- **Tests**: unit (record completeness), API contract (audit query), failure path (audit write fails → action not confirmed).
- **Depends on**: 11-12, `10` (audit trail).

### STORY 11-14 · M4 · S · P1 — CLI operations console parity
- **Story**: As `OPS`, I want the CLI console to show real link/telemetry status and issue the same guarded actions, so that I can operate from a terminal in the field.
- **Operability**: extend `cli_interface.rs` so `status` reflects live state (11-05) and add audited action commands routed through 11-12; web and CLI share the 11-03 state and the same action path.
- **Acceptance**:
  - Given a live link, when the operator runs `status`, then it prints real telemetry and link health, not "Connected"/"Receiving".
  - Given an action command without an authenticated session, when issued from the CLI, then it is refused identically to the web surface.
- **Tests**: CLI test (real status + action gating), parity test (web/CLI same outcome), failure path (unauthenticated CLI action → refused).
- **Depends on**: 11-03, 11-05, 11-12.

---

## M5 — Autonomous-Assist (gated advisories)

### STORY 11-15 · M5 · M · P2 — Operator-assist alerting advisory
- **Story**: As `OPS`, I want the console to proactively surface a suggested response when telemetry/alerts cross a threshold (e.g. low battery, geofence proximity), so that I am prompted to act — but I stay in command.
- **Safety**: advisories are computed from deterministic telemetry/geofence signals (11-04, 11-09); each cites the signal that triggered it and proposes an action without ever auto-executing; gated behind reliable single-drone control in `01`/`03`.
- **Acceptance**:
  - Given battery crosses the low threshold near the geofence edge, when the advisory runs, then it suggests RTH and cites the triggering signals, executing nothing.
  - Given nominal telemetry, when the advisory runs, then no suggestion is raised (no nuisance prompts).
- **Tests**: unit (threshold logic + evidence), gating test (no auto-execute), failure path (nominal → no advisory).
- **Depends on**: 11-04, 11-09, 11-12; `01`/`03` single-drone control.

### STORY 11-16 · M5 · S · P2 — Multi-drone fleet status overview (read-only)
- **Story**: As `DSP`, I want a read-only overview of multiple aircraft on one console, so that I can monitor a small fleet before any multi-drone control exists.
- **Operability**: aggregate per-drone link state and telemetry freshness from domain `12` fleet health into a single panel; strictly read-only — no fleet-wide action path in this story.
- **Acceptance**:
  - Given several enrolled aircraft, when the overview renders, then each shows its link state and freshness, sorted by health.
  - Given a drone with a stale heartbeat, when the overview renders, then it is flagged down/stale, never shown as healthy by omission.
- **Tests**: unit (aggregation + health sort), failure path (stale heartbeat → flagged), read-only assertion (no action route exposed).
- **Depends on**: 11-05, `12` (fleet health).

---

## Coverage note

These ~16 stories cover the 12 capabilities in `capability-map.md`, ordered by phase: a trustworthy receive-only console (M1/M2), deterministic map and mission overlays with asserted CRS (M3), then guarded, authed, audited operator actions (M4), with gated advisories last (M5). The curated counts in `release-plan.md` (≈70 feature rows: M1 12 / M2 16 / M3 18 / M4 18 / M5 6) expand several of these into sibling rows — per-tile telemetry bindings, additional overlay layers, per-action command variants, and reconnect/buffering refinements. Two execution rules are enforced as cross-cutting acceptance on every relevant story: the UI never commands the vehicle directly (all actions route through `mission_control` guardrails and return an ack), and no action control is enabled until the full loop passes in `Simulation` mode against domain `02`. This domain stays distinct from the post-flight advisor viewer in domain `08`.
