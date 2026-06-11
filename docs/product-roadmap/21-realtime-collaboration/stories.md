# Real-time Communication and Collaboration: Detailed Stories

> Greenfield (M0): no code exists for this domain yet. It is gated behind the core drone platform (`01`–`12`) and the advisor MVP, and depends on the identity/role spine (`10`), flight/mission control (`01`), the camera/video source (`04`), the geo viewer (`08`), the operator console (`11`), and fleet alerts (`12`). Stories are necessarily coarse and weighted to M1/M2 foundation; everything here is "build from scratch." Live drone video is a **new infrastructure boundary** — there is no media transport today — and is tracked as its own hardening line; the safety pillar leads for alerts and collaborative dispatch.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: the inspectable logic — permission resolution, state, locking, audit trail — that holds without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `EXPERT` remote expert, `TEAM-MEMBER` farm-team member, `OPS` operator, `PA` platform admin, `AG` agronomist.

---

## M1 — Foundation

### STORY 21-01 · M1 · S · P1 — Messaging and channels (farm teams)
- **Story**: As `TEAM-MEMBER`, I want a team channel scoped to a field and owned via `10`, so that a farm team can coordinate in one audited place.
- **Deterministic / evidence**: persist `{channel_id, org_id, field_ref, member_account_ids[], created_at}` and `{message_id, channel_id, author_id, body, sent_at}`; channel owned by one org; messages append-only and audited; membership resolves through `10`.
- **Acceptance**:
  - Given a field and an org, when a channel is created and a member posts, then the channel and message persist scoped to the org and are listable in order.
  - Given a user not in the channel's org, when they attempt to read it, then access is denied (no cross-tenant join).
  - Given a post to a non-existent channel, when sent, then it is rejected with a clear `4xx`.
- **Tests**: API contract (create channel/post/list), authz (cross-tenant read denied), failure path (post to missing channel → 4xx).
- **Depends on**: `10` (org/roles), `01` (field/mission context).

### STORY 21-02 · M1 · S · P2 — Role-based collaboration (via `10`)
- **Story**: As `PA`, I want channel and session permissions resolved through `10` roles, so that no one joins, streams, or edits beyond their role.
- **Deterministic / evidence**: resolve `{can_join, can_post, can_stream, can_annotate, can_dispatch, can_alert}` deterministically from `10` roles within an org; every collaboration action checks the resolved permission; no permission crosses a tenant boundary.
- **Acceptance**:
  - Given a user with the operator role, when permissions resolve, then `can_stream` and `can_dispatch` are granted within their org.
  - Given a viewer-only user, when they attempt to dispatch or stream, then it is denied and audited.
- **Tests**: unit (role→permission mapping), authz (viewer dispatch denied), failure path (cross-org action → denied).
- **Depends on**: `10` (roles), 21-01.

### STORY 21-03 · M1 · S · P2 — Presence and notifications
- **Story**: As `TEAM-MEMBER`, I want to see who is online and be notified on a field event, so that the team coordinates in real time.
- **Deterministic / evidence**: maintain presence `{account_id, channel_id, state(Online|Away|Offline), last_seen}`; fan out notifications on subscribed field/channel events; presence and notifications scoped per org; delivery state audited.
- **Acceptance**:
  - Given two members in a channel, when one connects, then the other sees their presence as Online within the heartbeat window.
  - Given a field event on a subscribed channel, when it fires, then subscribed members receive a notification recorded with delivery state.
  - Given a member who disconnects, when the heartbeat lapses, then their presence transitions to Offline (not stuck Online).
- **Tests**: unit (presence state machine), integration (notification fan-out), failure path (lapsed heartbeat → Offline).
- **Depends on**: 21-01, 21-02.

---

## M2 — Captured / Observable

### STORY 21-04 · M2 · L · P2 — Live drone video streaming (new pipeline, consumes `04`/`01`)
- **Story**: As `OPS`, I want to stream one live drone camera feed from the `04` source, so that the team can watch the field in real time.
- **Deterministic / evidence**: stand up a new media pipeline (encode → relay → view) carrying a `04` camera feed into a viewer surface; persist `{stream_id, org_id, mission_ref, source_ref, state(Starting|Live|Reconnecting|Ended)}`; this is a **new infrastructure boundary** and must prove bounded latency, reconnection, and frame-drop handling before reliance; stream scoped per org.
- **Acceptance**:
  - Given an active `04` camera source and an authorized operator, when a stream is started, then it reaches `Live` and an authorized viewer receives frames within the latency budget.
  - Given a transient transport drop, when it occurs, then the stream enters `Reconnecting` and resumes (or ends cleanly) rather than hanging indefinitely.
  - Given an unauthorized or cross-org viewer, when they request the stream, then it is denied.
- **Tests**: integration (encode/relay/view), resilience (reconnect + drop handling within budget), authz (cross-org viewer denied), failure path (source unavailable → stream not started, clear error).
- **Depends on**: `04` (camera), `01` (mission), 21-02 (permissions).

### STORY 21-05 · M2 · M · P2 — Emergency alert system (`01` safety + `12`)
- **Story**: As `OPS`, I want to raise an audited emergency alert from real `01` safety state and `12` fleet alerts, so that the team responds fast on a trustworthy path.
- **Deterministic / evidence**: subscribe to `01` safety state (geofence/altitude/battery/abort) and `12` fleet alerts; persist `{alert_id, org_id, source(01|12), severity, trigger_ref, state(Raised|Acknowledged|Resolved), raised_at}`; alert fan-out and every state change audited end to end; the safety pillar leads — no alert is silently dropped.
- **Acceptance**:
  - Given a real `01` safety breach (e.g. geofence violation), when it fires, then an alert is raised with its trigger reference and fanned out to subscribed members, fully audited.
  - Given a raised alert, when acknowledged and resolved, then each transition is recorded with actor and timestamp.
  - Given the alert fan-out fails for a recipient, when it occurs, then the failure is recorded and retried (never silently lost).
- **Tests**: unit (alert state machine), integration (`01`/`12` subscription), audit test (end-to-end trail), failure path (delivery failure → recorded + retried).
- **Depends on**: `01` (safety state), `12` (fleet alerts), 21-03 (notifications).

---

## M3 — Explainable

### STORY 21-06 · M3 · M · P2 — Session recording and audit
- **Story**: As `PA`, I want to record and replay a video/edit/alert session, so that any collaboration can be reviewed after the fact.
- **Deterministic / evidence**: persist a session timeline `{session_id, org_id, events[]}` capturing stream segments, mission edits, annotations, and alerts with timestamps and actor IDs; replay reconstructs the ordered timeline deterministically; recordings scoped per org and immutable.
- **Acceptance**:
  - Given a session with a stream, edits, and an alert, when recorded, then replay reconstructs the events in order with correct actors and timestamps.
  - Given a session, when replayed by an out-of-org user, then access is denied.
  - Given a session with a gap (e.g. dropped stream segment), when replayed, then the gap is represented explicitly rather than silently stitched over.
- **Tests**: integration (record→replay ordering), authz (out-of-org replay denied), failure path (gap rendered explicitly).
- **Depends on**: 21-04, 21-05.
- **Note**: dependency on 21-07 (collaborative mission planning, M4) removed. Session recording captures generic event streams (stream segments, edits, annotations, alerts) and does not require the mission-planning collaboration feature to exist. When 21-07 ships, its events become additional event types recorded by 21-06, but 21-06 does not depend on 21-07 to function.

---

## M4 — Interactive

### STORY 21-07 · M4 · M · P2 — Collaborative mission planning (over `01`)
- **Story**: As `AG`, I want to co-edit an `01` mission with conflict-safe locking, so that a team can plan together without bypassing flight safety.
- **Deterministic / evidence**: shared edit of an `01` mission with optimistic/locked editing; every edit attributed and audited; **dispatch still passes the `01` safety guardrails (geofence/altitude/battery/abort)** — collaboration never bypasses them; conflicting edits resolved deterministically (lock or last-writer rejected).
- **Acceptance**:
  - Given two editors on one mission, when both edit the same waypoint, then conflict resolution prevents silent overwrite (one edit is rejected or queued) and both attempts are audited.
  - Given a co-edited mission, when it is dispatched, then the `01` safety guardrails are enforced exactly as for a single-editor mission.
  - Given an edit that would violate a geofence, when dispatch is attempted, then it is blocked by the `01` guardrails (collaboration cannot bypass safety).
- **Tests**: unit (conflict resolution), integration (`01` guardrails enforced on dispatch), failure path (geofence-violating edit → dispatch blocked).
- **Depends on**: `01` (missions + safety), 21-02, 21-06 (audit).

### STORY 21-08 · M4 · M · P2 — Remote-expert sessions (annotate via `08`)
- **Story**: As `EXPERT`, I want to annotate a live field through the `08` viewer in a session, so that I can guide the on-site team remotely.
- **Deterministic / evidence**: a remote-expert session attaches `08` annotations to a live scene; annotations persisted and attributed to the expert with timestamps; annotation visibility scoped per org/session; reuses the `08` annotation surface.
- **Acceptance**:
  - Given an active session and an authorized expert, when they draw an annotation, then it is persisted, attributed to them, and visible to session participants in real time.
  - Given an unauthorized user, when they attempt to annotate, then it is denied and the session is unaffected.
- **Tests**: integration with `08` annotations, authz (unauthorized annotate denied), failure path (lost connection → annotation persisted, session recoverable).
- **Depends on**: `08` (annotations), 21-02, 21-04 (live scene).

### STORY 21-09 · M4 · S · P2 — Operator console integration (`11`)
- **Story**: As `OPS`, I want live streams and alerts surfaced in the `11` operator console, so that I run collaboration from the existing operations surface.
- **Deterministic / evidence**: the `11` console subscribes to org-scoped streams (21-04) and alerts (21-05); rendering respects `10` permissions; no cross-tenant stream or alert is shown.
- **Acceptance**:
  - Given live streams and active alerts for an operator's org, when they open the `11` console, then they see the org's streams and alerts.
  - Given a stream/alert from another org, when the console renders, then it is absent (no cross-tenant leak).
- **Tests**: integration with `11`, authz (cross-tenant not shown), failure path (stream ended → console reflects Ended, not stale Live).
- **Depends on**: `11` (console), 21-04, 21-05.

### STORY 21-10 · M4 · S · P2 — Portal integration (`13`)
- **Story**: As `TEAM-MEMBER`, I want a scoped stream/alert feed in the grower portal, so that growers can follow collaboration without the operator console.
- **Deterministic / evidence**: the portal (`13`) renders a read-only, org-scoped feed of streams and alerts; respects `10` visibility; only permitted streams/alerts are reachable.
- **Acceptance**:
  - Given a grower with access, when they open the portal feed, then they see their org's permitted streams and alerts read-only.
  - Given a stream/alert outside their visibility, when accessed via the portal, then it returns `403`/`404` (never leaks).
- **Tests**: integration with `13`, authz (out-of-scope denied), failure path (direct hit on foreign stream → 403).
- **Depends on**: `13` (portal), 21-04, 21-05, 21-02.

---

## Coverage note

This file covers all 10 capabilities in `capability-map.md` with ~10 greenfield stories (≈1 per capability), weighted to M1/M2 foundation with M3/M4 interactive slices, matching the M1/M2-heavy, mostly-P2 shape of `release-plan.md` (only the channel/identity slice, 21-01, is P1; no P0; no M5 stories authored since release-plan lists just 2 M5 rows). The curated counts in `release-plan.md` (≈62 rows) expand several of these into sibling slices when implemented (e.g. multi-stream relays, per-severity alert routing, richer presence, threaded messaging, multi-party annotation conflict handling). The safety pillar leads for the alert path (21-05) and collaborative dispatch (21-07 — collaboration never bypasses the `01` geofence/altitude/battery/abort guardrails), and the live-video pipeline (21-04) is treated as a new infrastructure boundary that must prove latency, reconnection, and drop handling before it is relied on.
