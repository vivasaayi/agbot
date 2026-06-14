# Real-time Communication and Collaboration: Release Plan

## Shipment Strategy

This is a greenfield (M0 named) domain, so the plan is weighted to the M1 foundation and M2 captured phases: first establish messaging/channels, presence, and role-based access through `10`, then stand up the real-time capture layer (M2 live video pipeline and emergency alert wiring), then make sessions explainable and auditable (M3 session recording/audit), then interactive collaborative planning and remote-expert annotation (M4). Priority is mostly P2 (post-MVP) because the domain is sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP; only the foundational channel/identity slice is P1. The live-video pipeline is a new infrastructure boundary and is tracked as its own hardening line.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 18 |
| M3 explainable | 12 |
| M4 interactive | 14 |
| M5 autonomous-assist | 2 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P1 | 6 |
| P2 | 46 |
| P3 | 10 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 10 |
| M | 32 |
| S | 20 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- | --- |
| M1 foundation | S | P1 | Messaging and channels (farm teams) | operability | identity |
| M1 foundation | S | P2 | Role-based collaboration (via `10`) | explainability and trust | identity |
| M1 foundation | S | P2 | Presence and notifications | operability | capture |
| M2 captured | L | P2 | Live drone video streaming (`04`/`01`) | performance and scale | capture |
| M2 captured | M | P2 | Emergency alert system (`01` safety + `12`) | safety | evaluator |
| M3 explainable | M | P2 | Session recording and audit | explainability and trust | evaluator |
| M4 interactive | M | P2 | Collaborative mission planning (over `01`) | safety | operations |

## Execution Rules

- Sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP; the live-video pipeline is a new infrastructure track and must prove latency, reconnection, and drop handling before it ships.
- Emergency alerts must be audited end to end and subscribe to real `01` safety state and `12` fleet alerts before being relied on operationally.
- Collaborative mission edits must never bypass the `01` dispatch guardrails; every edit is attributed and audited, and dispatch still enforces geofence/altitude/battery/abort.
- All channels, sessions, streams, and alerts resolve through the `10` role model; no cross-tenant join, and every session is recordable for after-action review.
