# Alerting and Notification: Detailed Stories

> Greenfield, cross-cutting backbone (M0 named): no code exists yet. Every story below is **built from scratch** in the new `alerting` crate (likely in or alongside `shared`). This is **infrastructure many domains feed** — weather (`15`), drought (`17`), compliance (`24`), maintenance (`25`), sensor health (`27`), emergencies (`21`), and the advisor (`09`) — surfaced through `11`/`13`. The **operability and explainability/trust pillars dominate every phase**: a flood, a dropped critical alert, or an unexplained fire are the defining failure modes. Notification channels are external boundaries kept behind a mockable interface; no story requires a real provider to be testable.

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Deterministic / evidence**: what must be computed and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation

### STORY 29-01 · M1 · M · P0 — Typed event / source-adapter contract
- **Story**: As `PA`, I want any domain to emit a typed alert event through one source-adapter contract, so that every emitter feeds one backbone instead of building its own path.
- **Deterministic / evidence**: define `AlertEvent { source_domain, event_type, subject_ref, severity_hint, evidence, occurred_at, idempotency_key }`; expose a `SourceAdapter::emit(event)` interface; persist accepted events with provenance; reject events missing required fields.
- **Acceptance**:
  - Given a well-formed event from a source domain (e.g. `27` sensor health), when emitted, then it is accepted, persisted with provenance, and assigned an internal alert candidate ID.
  - Given an event missing `source_domain` or `subject_ref`, when emitted, then it is rejected with a reason code and counted (not partially stored).
- **Tests**: unit (schema validation), API contract (emit/list), failure path (malformed event rejected).
- **Depends on**: `shared` schemas; consumed by `15`/`17`/`24`/`25`/`27`/`21`/`09`.

### STORY 29-02 · M1 · M · P0 — Deterministic rule engine with explanation
- **Story**: As `OPS`, I want a deterministic rule engine that evaluates threshold/event subscriptions and explains why an alert fired, so that no alert is a black box.
- **Deterministic / evidence**: a rule is `{rule_id, match (event_type/threshold/predicate), severity, channels}`; evaluation is pure and inspectable; a fired alert records `{matched_rule_id, evidence, explanation}`; no AI in the path.
- **Acceptance**:
  - Given an event and a matching rule, when evaluated, then an alert is fired carrying the matched rule ID and an explanation citing the triggering evidence.
  - Given an event that matches no rule, when evaluated, then no alert fires (no spurious alerts), and the non-match is observable.
- **Tests**: unit (predicate/threshold evaluation + explanation), fixture (rule sets + events), failure path (no-match → no alert).
- **Depends on**: 29-01.

### STORY 29-03 · M1 · S · P1 — Rule and subscription management
- **Story**: As `PA`, I want to create, list, enable/disable, and version rules and subscriptions, so that domains can manage what triggers alerts.
- **Deterministic / evidence**: persist rules with status `Active/Disabled`; a disabled rule never fires; rule edits are versioned; subscriptions bind a recipient/role to a rule.
- **Acceptance**:
  - Given an active rule, when disabled, then it stops firing and the change is audited.
  - Given a malformed rule (e.g. invalid predicate), when created, then it is rejected before it can fire (no half-valid rule active).
- **Tests**: unit (rule lifecycle), API contract (CRUD + enable/disable), failure path (invalid rule rejected).
- **Depends on**: 29-02.

---

## M2 — Captured / Observable

### STORY 29-04 · M2 · S · P0 — Alert audit and history
- **Story**: As `DSP`, I want every fired alert persisted with its rule and evidence, so that any alert can be defended and reviewed after the fact.
- **Deterministic / evidence**: persist `{alert_id, matched_rule_id, source_event_ref, evidence, severity, fired_at, explanation}`; history is immutable and queryable by source/field/severity/time; analog of `04` capture-provenance.
- **Acceptance**:
  - Given a fired alert, when stored, then it is retrievable with its rule, evidence, and explanation, and is paginable/filterable.
  - Given a request for an alert that never fired, when queried, then an empty result is returned (no fabricated history).
- **Tests**: API contract (pagination + filters), fixture (seeded alerts), failure path (unknown alert → not found).
- **Depends on**: 29-02.

### STORY 29-05 · M2 · S · P1 — Idempotent re-emit handling
- **Story**: As `OPS`, I want a source re-emitting the same event (same idempotency key) to not create duplicate alerts, so that retries from a flaky emitter are safe.
- **Deterministic / evidence**: an event's `idempotency_key` within a window collapses to one alert candidate; re-emits update, not duplicate; the dedup decision is recorded.
- **Acceptance**:
  - Given the same event emitted twice with one idempotency key, when processed, then exactly one alert candidate exists.
  - Given two genuinely distinct events with different keys, when processed, then both are retained (no false collapse).
- **Tests**: unit (idempotency keying), fixture (duplicate emits), failure path (distinct events not collapsed).
- **Depends on**: 29-01.

### STORY 29-06 · M2 · M · P1 — Templates rendered from evidence
- **Story**: As `AG`, I want alert messages rendered from a template plus the triggering evidence, so that alerts are readable and consistent across channels.
- **Deterministic / evidence**: a template binds named evidence fields into a message; rendering is deterministic; a missing required field fails rendering with a clear error rather than emitting a blank/`{{placeholder}}` message.
- **Acceptance**:
  - Given a template and an alert's evidence, when rendered, then a complete message is produced with the evidence values substituted.
  - Given evidence missing a required template field, when rendered, then it fails with a clear error (no blank/placeholder message sent).
- **Tests**: unit (rendering + substitution), fixture (templates + evidence), failure path (missing field → render error).
- **Depends on**: 29-02.

---

## M3 — Explainable (the deterministic anti-storm and delivery core)

### STORY 29-07 · M3 · S · P0 — Severity classification
- **Story**: As `OPS`, I want each fired alert's severity classified deterministically from the rule and evidence, so that routing and quiet-hour overrides have a consistent basis.
- **Deterministic / evidence**: severity (e.g. `info/warning/critical/emergency`) is derived from the matched rule and evidence thresholds; the derivation is recorded; emergency/critical are flagged for hard-override downstream.
- **Acceptance**:
  - Given an alert whose evidence crosses a critical threshold, when classified, then it is severity `critical` with the deriving rule/threshold recorded.
  - Given conflicting severity hints, when classified, then the deterministic rule (not the source's hint) decides, and the decision is explained.
- **Tests**: unit (severity derivation), fixture (threshold bands), failure path (hint/rule conflict resolved deterministically).
- **Depends on**: 29-02.

### STORY 29-08 · M3 · M · P0 — Dedup and aggregation (anti-storm)
- **Story**: As `OPS`, I want repeated or related alerts deduplicated and aggregated by a deterministic key, so that a misbehaving source cannot flood me.
- **Deterministic / evidence**: a dedup key is computed from `{source_domain, subject_ref, rule_id}`; within a window, repeats increment a count on one alert instead of firing N; related alerts roll up into a summary; critical/emergency may bypass suppression.
- **Acceptance**:
  - Given a source firing the same condition 100 times in a window, when processed, then one alert is surfaced with an occurrence count (not 100 alerts).
  - Given a critical alert during suppression, when processed, then it bypasses dedup suppression and is surfaced immediately.
- **Tests**: unit (dedup key + window count), fixture (storm stream), failure path (critical bypasses suppression).
- **Depends on**: 29-07.

### STORY 29-09 · M3 · M · P0 — Notification channels (in-app first, mockable boundary)
- **Story**: As `OPS`, I want an alert delivered through a channel behind a mockable boundary, starting with in-app, so that delivery works without a real external provider in tests.
- **Deterministic / evidence**: a `ChannelAdapter` trait `{send(alert, recipient) -> DeliveryOutcome}`; in-app is the first adapter; email/SMS/webhook/push are external boundaries with mock adapters; each send produces a recorded outcome.
- **Acceptance**:
  - Given an alert and an in-app recipient, when delivered, then a delivery record is created and the alert appears in the recipient's feed.
  - Given a channel adapter that errors, when delivery is attempted, then the failure is recorded as a delivery outcome (not swallowed).
- **Tests**: unit (adapter contract), fixture (mock channels), failure path (adapter error recorded).
- **Depends on**: 29-08.

### STORY 29-10 · M3 · M · P0 — Delivery tracking (state machine + retry/backoff)
- **Story**: As `OPS`, I want each delivery tracked through an explicit state machine with bounded retry/backoff, so that a transient channel failure does not lose an alert.
- **Deterministic / evidence**: delivery states `queued→sending→delivered|failed→retrying`; transitions are deterministic; retries use bounded backoff with a max attempt cap; terminal `failed` is recorded with the last error.
- **Acceptance**:
  - Given a transient send failure, when delivery runs, then it retries with backoff and reaches `delivered` within the attempt cap.
  - Given a channel down past the attempt cap, when retries exhaust, then the delivery is terminal `failed` with the last error (no infinite retry, no silent drop).
- **Tests**: unit (state transitions + backoff), fixture (flaky channel), failure path (exhausted retries → terminal failed).
- **Depends on**: 29-09.

### STORY 29-11 · M3 · S · P1 — Multi-channel delivery (email/SMS/webhook/push)
- **Story**: As `OPS`, I want email, SMS, webhook, and push channels behind the same adapter contract, so that an alert can reach recipients beyond the in-app feed.
- **Deterministic / evidence**: each channel implements `ChannelAdapter` with a mock for tests; channel selection comes from the rule/subscription/preferences; per-channel delivery records reuse the 29-10 state machine.
- **Acceptance**:
  - Given a subscription specifying email and SMS, when an alert fires, then both channels produce independent tracked deliveries.
  - Given an unconfigured channel, when selected, then the alert routes to a configured fallback or records an unroutable outcome (never silently nothing).
- **Tests**: unit (per-channel adapters), integration (multi-channel fan-out), failure path (unconfigured channel → fallback/unroutable recorded).
- **Depends on**: 29-09, 29-10.

---

## M4 — Interactive (routing, escalation, preferences)

### STORY 29-12 · M4 · M · P0 — Routing to recipients
- **Story**: As `PA`, I want alerts routed to recipients by role, field, and severity, so that the right people get the right alerts.
- **Deterministic / evidence**: a routing rule maps `{severity, source_domain, field_id, role}` to one or more recipients/subscriptions; routing is deterministic and audited; an unrouted alert (no matching recipient) is flagged, not dropped.
- **Acceptance**:
  - Given a critical field alert and a routing rule for that field's agronomist, when routed, then the agronomist is a recipient and the routing decision is audited.
  - Given an alert that matches no routing rule, when routed, then it is flagged unrouted and surfaced to a default operator (never silently dropped).
- **Tests**: unit (routing match), API contract (routing rules), failure path (no match → unrouted flagged).
- **Depends on**: 29-04, 29-09.

### STORY 29-13 · M4 · S · P0 — Acknowledgement and resolution lifecycle
- **Story**: As `OPS`, I want to acknowledge and resolve an alert with actor and timestamp, so that the team knows what is handled.
- **Deterministic / evidence**: alert lifecycle `fired→acknowledged→resolved` (or `auto_resolved` when the source condition clears); each transition records actor and timestamp; resolution is idempotent.
- **Acceptance**:
  - Given a fired alert, when acknowledged then resolved, then both transitions are recorded with actor and timestamp.
  - Given an already-resolved alert, when resolved again, then it is a no-op (idempotent), not an error or a duplicate.
- **Tests**: unit (lifecycle + idempotency), API contract (ack/resolve), failure path (double-resolve is a no-op).
- **Depends on**: 29-04.

### STORY 29-14 · M4 · M · P1 — Escalation on no-ack
- **Story**: As `OPS`, I want an unacknowledged alert escalated to the next recipient within a window, so that a critical alert is never silently ignored.
- **Deterministic / evidence**: an escalation policy defines `{ack_window, escalation_chain[]}`; if an alert is not acknowledged within the window, it escalates to the next recipient deterministically; every escalation is audited; resolution stops escalation.
- **Acceptance**:
  - Given an unacknowledged critical alert past its ack window, when escalation runs, then it routes to the next recipient in the chain and records the escalation.
  - Given an alert acknowledged within the window, when the window elapses, then no escalation occurs (ack stops the chain).
- **Tests**: unit (escalation timing + chain), fixture (no-ack vs ack), failure path (ack within window → no escalation).
- **Depends on**: 29-12, 29-13.

### STORY 29-15 · M4 · S · P1 — Quiet hours and per-user preferences
- **Story**: As `GR`, I want quiet hours and per-channel preferences honored, so that I am not paged at night for non-critical alerts — but still reached for emergencies.
- **Deterministic / evidence**: a user preference holds `{quiet_hours, channel_prefs, min_severity}`; non-critical alerts during quiet hours are deferred/suppressed deterministically; critical/emergency (29-07) hard-override quiet hours; every suppression/override is recorded.
- **Acceptance**:
  - Given a warning alert during a user's quiet hours, when delivered, then it is deferred/suppressed per preference and the decision is recorded.
  - Given an emergency alert during quiet hours, when delivered, then it overrides quiet hours and is delivered immediately (override recorded).
- **Tests**: unit (quiet-hours + severity override), fixture (preferences), failure path (emergency overrides quiet hours).
- **Depends on**: 29-07, 29-09, 29-12.

---

## M5 — Autonomous-Assist (gated, uncertainty-flagged)

### STORY 29-16 · M5 · M · P2 — Adaptive aggregation / storm-prediction advisory
- **Story**: As `OPS`, I want an advisory that flags when an alert pattern looks like an emerging storm or a flapping source, so that I can tune rules — without the system silently muting alerts on its own.
- **Deterministic / evidence**: the storm/flapping signal is composed only from already-recorded alert history and dedup counts (29-04, 29-08); every advisory carries an uncertainty band and cites the alert evidence; feature-flagged and approval-gated; it never auto-suppresses critical/emergency alerts.
- **Acceptance**:
  - Given a history with a flapping source, when the advisory runs, then it surfaces a tuning recommendation with an uncertainty band citing the alert pattern.
  - Given insufficient history, when the advisory is requested, then it is unavailable (never fabricated), and no automatic suppression is applied to critical alerts.
- **Tests**: unit (pattern composition + uncertainty), gating test (disabled until history exists), failure path (no auto-suppression of critical alerts).
- **Depends on**: 29-04, 29-08.

---

## Coverage note

These 16 stories cover all 12 capabilities in `capability-map.md`. The breakdown is operability- and explainability-led, with a deliberately heavy M3 anti-storm/delivery core (severity, dedup/aggregation, channels, delivery state machine) reflecting that **operability and explainability/trust lead every phase** in `release-plan.md`. Every fired alert explains its rule and evidence (29-02, 29-04), channels stay behind a mockable boundary (29-09, 29-11), and the single M5 story (adaptive aggregation advisory) stays approval-gated and never auto-suppresses critical alerts. The curated counts in `release-plan.md` (~82 rows) expand several of these (per-channel adapter variants, additional rule/routing/escalation slices) into sibling stories when implemented.
