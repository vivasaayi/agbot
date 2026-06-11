# Alerting and Notification: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (operability and explainability/trust first, then data quality, performance and scale, agronomic value) and the workstreams in `release-plan.md`. Because this is a greenfield, cross-cutting backbone (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. Operability and explainability dominate: a backbone that floods operators, drops a critical alert, or fires without an explanation is worse than none. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Alerting and Notification Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Event bus / source-adapter contract | missing (greenfield) | 8 | Any domain emits one typed event with a stable schema |
| Alert rule engine (deterministic evaluation) | missing (greenfield) | 9 | Evaluate a threshold/event subscription and explain the match |
| Severity classification | missing (greenfield) | 6 | Classify a fired alert's severity from rule + evidence |
| Dedup / aggregation (anti-storm) | missing (greenfield) | 8 | Dedup repeat alerts by a deterministic key |
| Notification channels (in-app/email/SMS/webhook/push) | missing (greenfield) | 9 | Deliver one alert in-app behind a mockable channel boundary |
| Delivery tracking (state machine + retry/backoff) | missing (greenfield) | 8 | Track delivery through a state machine with retry |
| Routing and escalation | missing (greenfield) | 8 | Route an alert to a recipient; escalate on no-ack |
| Quiet hours and per-user preferences | missing (greenfield) | 6 | Honor a user's quiet hours and channel preference |
| Alert audit and history | missing (greenfield) | 6 | Persist every fired alert with its rule + evidence |
| Templates | missing (greenfield) | 5 | Render an alert message from a template + evidence |
| Acknowledgement and resolution lifecycle | missing (greenfield) | 5 | Ack/resolve an alert with actor and timestamp |
| Surfacing through `11` / `13` | missing (greenfield) | 4 | Expose an alert feed to the ground station / portal |
