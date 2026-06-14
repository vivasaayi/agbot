# Alerting and Notification: Current State and Target State

## Mission

Provide one notification and alerting backbone that every domain feeds, instead of each reinventing it: accept typed events from any source, evaluate deterministic rules that explain why an alert fired, classify severity, dedup and aggregate to prevent storms, route and escalate to the right people, deliver across channels with retry and tracking, honor quiet hours and preferences, and keep a full audited history — surfaced through the ground station (`11`) and farmers portal (`13`).

## Current Maturity

greenfield pending (M0 named): no implementation exists. There is no `alerting` crate, event bus, rule engine, channel adapter, delivery tracker, or audit store. This is cross-cutting infrastructure: today, any domain that wanted to warn an operator would have to build its own ad-hoc, untracked path, which is exactly what this backbone exists to prevent.

## What Exists Now

- Nothing is built for this domain. There is no typed alert event, rule engine, dedup/aggregation, routing/escalation, channel adapter, or delivery state machine.
- Adjacent surfaces it would serve and build on (some partially real):
  - Domain `11` (ground station UI): the WebSocket client and message dispatch where an in-app alert feed would surface.
  - Domain `13` (farmers portal): the grower-facing surface where alert summaries and report-ready notifications would appear.
  - Source domains that would emit events into the backbone: `15` (weather alerts), `17` (drought early warning), `24` (compliance deadline/expiry), `25` (maintenance/fleet health), `27` (sensor health), `21` (emergency), and the advisor `09` (findings/recommendations ready).
  - Domain `shared`: where the crate likely lives or sits alongside, providing schemas and cross-crate types.

## Gaps to Close

- No typed event/source-adapter contract: there is no stable schema for "any domain emits a typed event" with provenance and an idempotency key.
- No deterministic rule engine: no threshold/event subscriptions, no inspectable evaluation, and no "why did this fire" explanation tying a match to a rule and evidence.
- No severity classification derived from rule + evidence.
- No dedup or aggregation: nothing prevents an alert storm when a source emits repeatedly.
- No notification channels: in-app, email, SMS, webhook, and push are all external boundaries with no mockable adapter.
- No delivery tracking: no delivery state machine, retry/backoff, or per-channel delivery status.
- No routing or escalation: nobody is mapped to alerts, and there is no escalation on no-ack.
- No quiet hours or per-user preferences, no acknowledgement/resolution lifecycle, no alert audit/history, and no templates.

## Related Existing Surfaces

- Domain `11` (ground station UI): WebSocket/message-dispatch surface for an in-app alert feed.
- Domain `13` (farmers portal): grower-facing surface for alert summaries.
- Source domains `15`, `17`, `24`, `25`, `27`, `21`, `09`: emitters of typed events into the backbone.
- Domain `shared`: likely home of the `alerting` crate and its schemas.

## Target Operating Model

- One typed event contract: any domain emits `{source_domain, event_type, subject_ref, severity_hint, evidence, occurred_at, idempotency_key}` through a source-adapter interface; the backbone owns evaluation, delivery, and audit.
- Evidence before advice: the rule engine is deterministic and inspectable; every fired alert explains which rule matched and cites the evidence that triggered it, before any channel is touched.
- Severity is classified deterministically from rule + evidence; dedup and aggregation key on a deterministic dedup key so a repeating source cannot storm operators.
- Routing maps alerts to recipients by role/field/severity; escalation fires on no-ack within a window; quiet hours and per-user channel preferences are honored (except for hard-override critical/emergency alerts).
- Channels (in-app, email, SMS, webhook, push) sit behind a mockable boundary; delivery runs through an explicit state machine (`queued→sending→delivered|failed→retrying`) with bounded retry/backoff and per-delivery status.
- Every alert and delivery is audited and queryable; templates render messages from evidence; the feed surfaces through `11` and `13`.
- Reproducible outputs: the same event and rule set produce the same alert, severity, and dedup key, with tests on the rule/dedup/delivery-state logic and at least one failure path (channel failure, storm suppression, missed ack).
