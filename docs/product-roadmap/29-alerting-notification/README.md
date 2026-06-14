# Alerting and Notification

One notification and alerting backbone that every domain feeds, instead of each reinventing it: a deterministic rule engine, a typed source-adapter contract, multi-channel delivery, severity and dedup, routing and escalation, and an audit trail where every fired alert explains why.

## Where We Are

- Not started / vision only. This is a greenfield, cross-cutting backbone (M0 named): there is no `alerting` crate, rule engine, event bus, channel adapter, or delivery tracker. Today each domain that wants to warn an operator would have to build its own ad-hoc path.
- It is infrastructure, not a product feature: many domains emit events into it — weather (`15`), drought early warning (`17`), compliance deadlines (`24`), maintenance/fleet health (`25`), sensor health (`27`), emergencies (`21`), and the advisor (`09`) — and it is surfaced through the ground station (`11`) and farmers portal (`13`).
- The operability and explainability/trust pillars dominate: an alert backbone that floods operators, drops a critical alert, or fires without explaining why is worse than no backbone.

## Where We Should Be

- A single typed event contract any domain can emit into, plus a deterministic rule engine that evaluates threshold/event subscriptions and explains every match.
- Severity classification, deduplication, and aggregation that prevent alert storms; routing and escalation that send the right alert to the right person and escalate on no-ack.
- Notification channels (in-app, email, SMS, webhook, push) behind a mockable boundary, with a delivery state machine, retry/backoff, and per-delivery tracking.
- Quiet hours and per-user preferences, a full alert audit/history, and templates — with every fired alert citing the rule and the evidence that triggered it.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P0/P1 slices.
- `stories.md`: detailed vertical-slice stories by release phase.

## Build Order

1. Typed event/source-adapter contract: any domain emits a typed event with a stable schema and provenance.
2. Deterministic rule engine: threshold/event subscriptions evaluated without AI, each match explaining which rule and evidence fired it.
3. Severity classification plus dedup/aggregation keying to prevent alert storms.
4. Notification channels (in-app, email, SMS, webhook, push) behind a mockable boundary, with templates.
5. Delivery tracking: a delivery state machine with retry/backoff and per-channel delivery status.
6. Routing, escalation, quiet hours, per-user preferences, and alert audit/history.

## Primary Crates

New crate `alerting` (likely living in or alongside `shared`). Consumes typed events from `15`, `17`, `24`, `25`, `27`, `21`, and `09`; surfaced through `11` (ground station) and `13` (farmers portal). Channel adapters (email/SMS/webhook/push) are external boundaries kept behind a mockable interface.
