# Alerting and Notification: Release Plan

## Shipment Strategy

Ship in maturity order, treating this as cross-cutting infrastructure rather than a single product feature. The typed event contract and a rule engine that explains every match (M1) come first, then observable event capture with idempotency and audit (M2), then the deterministic anti-storm core — severity, dedup/aggregation, and the delivery state machine (M3), then interactive routing, escalation, quiet hours, and preferences (M4). Adaptive/advisory alerting behavior (M5) stays gated behind a trustworthy deterministic core. The operability and explainability/trust pillars lead every phase: a flood, a dropped critical alert, or an unexplained fire are the defining failure modes. Channels are external boundaries kept behind a mockable interface; no slice depends on a real provider to be testable.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 17 |
| M2 captured | 14 |
| M3 explainable | 26 |
| M4 interactive | 19 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 30 |
| P1 | 34 |
| P2 | 18 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 10 |
| M | 44 |
| S | 28 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | Event bus / source-adapter contract | operability | contract |
| M1 foundation | M | Alert rule engine (deterministic evaluation) | explainability and trust | evaluator |
| M2 captured | S | Alert audit and history | explainability and trust | capture |
| M3 explainable | S | Severity classification | explainability and trust | evaluator |
| M3 explainable | M | Dedup / aggregation (anti-storm) | operability | evaluator |
| M3 explainable | M | Notification channels (in-app first) | operability | delivery |
| M3 explainable | M | Delivery tracking (state machine + retry/backoff) | operability | delivery |
| M4 interactive | M | Routing and escalation | operability | interaction |
| M4 interactive | S | Acknowledgement and resolution lifecycle | operability | interaction |

## Execution Rules

- This backbone is consumed by `15`, `17`, `24`, `25`, `27`, `21`, and `09`, and surfaced through `11`/`13`; every emitter goes through the one typed source-adapter contract, not an ad-hoc path.
- The rule engine must be deterministic and inspectable; every fired alert must explain which rule matched and cite the evidence that triggered it before any channel is touched.
- Dedup/aggregation must key on a deterministic dedup key so a repeating source cannot storm operators; critical/emergency alerts may hard-override quiet hours and dedup suppression.
- Notification channels (email/SMS/webhook/push) are external boundaries kept behind a mockable interface; no slice may require a real provider to be testable.
- Delivery must run through an explicit state machine with bounded retry/backoff and per-delivery status; a channel failure must never silently lose an alert.
- Escalation must fire on no-ack within a configured window; every alert, delivery attempt, ack, and escalation must be audited.
