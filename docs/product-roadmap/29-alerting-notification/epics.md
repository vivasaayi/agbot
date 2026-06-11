# Alerting and Notification: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: event-emit, rule, subscription, and alert-feed routes or commands, persistence, auth scoped to org/role, pagination, and audit IDs.
- Contract: a typed source-adapter event schema with provenance and an idempotency key; any domain emits through one interface.
- Deterministic: rule evaluation, severity classification, dedup keying, and delivery-state transitions computed without AI, with reason codes and the raw evidence retained.
- Explainability: every fired alert explains which rule matched and cites the evidence that triggered it.
- Channels: in-app/email/SMS/webhook/push behind a mockable boundary; no real send without an adapter and a delivery record.
- Operability: delivery state machine, retry/backoff, quiet hours, preferences, escalation, and an audited history.
- Tests: unit (rule/severity/dedup/delivery-state math), fixture (event streams, rule sets), API contract, and one failure path (channel failure, alert storm, missed ack).
- Operations: feature flag, channel health, retry/backoff, and a runbook.

## Category Epics

### EPIC-01: Event Contract and Deterministic Rules
- Goal: any domain emits a typed event and a deterministic rule engine evaluates it explainably.
- First release: the typed source-adapter event contract (with provenance + idempotency key) and a rule engine that evaluates threshold/event subscriptions and explains every match.
- Expansion: severity classification from rule + evidence, and alert audit/history.
- Hardening: reproducibility (same event + rules → same alert), idempotent re-emit handling, and evidence retention.

### EPIC-02: Anti-Storm and Delivery
- Goal: operators are never flooded, and every alert's delivery is tracked.
- First release: dedup/aggregation by a deterministic key and an in-app channel behind a mockable boundary.
- Expansion: email/SMS/webhook/push channels and templates rendered from evidence.
- Hardening: the delivery state machine with retry/backoff, per-channel delivery status, and channel-failure negative-path tests.

### EPIC-03: Routing, Escalation, and Preferences
- Goal: the right alert reaches the right person, respecting their preferences, and escalates when ignored.
- First release: routing alerts to recipients by role/field/severity and an acknowledgement/resolution lifecycle.
- Expansion: escalation on no-ack within a window, quiet hours, and per-user channel preferences (with hard-override for critical/emergency).
- Hardening: surfacing the feed through `11`/`13`, audit export, and missed-ack/escalation negative-path tests.
