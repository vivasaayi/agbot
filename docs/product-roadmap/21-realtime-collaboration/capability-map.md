# Real-time Communication and Collaboration: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (operability, safety, explainability and trust, data quality, performance and scale) and the workstreams in `release-plan.md`. Because this is a greenfield domain (M0 named), every capability's current source status is "missing (greenfield)", and the Primary First Slice describes the M1 foundation step. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Real-time Collaboration Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Messaging and channels (farm teams) | missing (greenfield) | 7 | Team channel scoped to a field, owned via `10` |
| Role-based collaboration (via `10`) | missing (greenfield) | 6 | Resolve channel/session permissions through `10` roles |
| Presence and notifications | missing (greenfield) | 6 | Show who is online and notify on a field event |
| Live drone video streaming (consumes `04`/`01`) | missing (greenfield) | 9 | Stream one live drone camera feed (new pipeline) |
| Collaborative mission planning (over `01`) | missing (greenfield) | 8 | Shared, conflict-safe edit of an `01` mission |
| Remote-expert sessions (annotate via `08`) | missing (greenfield) | 7 | A remote expert annotates a live field through `08` |
| Emergency alert system (`01` safety + `12`) | missing (greenfield) | 8 | Raise an audited emergency alert from `01`/`12` state |
| Session recording and audit | missing (greenfield) | 6 | Record and replay a video/edit/alert session |
| Operator console integration (`11`) | missing (greenfield) | 5 | Surface live streams and alerts in the `11` console |
| Portal integration (`13`) | missing (greenfield) | 4 | Surface a scoped stream/alert feed in the portal |
