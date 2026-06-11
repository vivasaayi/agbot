# Real-time Communication and Collaboration

An integrated communication platform for farm teams, stakeholders, and remote experts: live drone video, collaborative mission planning, shared field annotation, presence, and an emergency alert system.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#22 Real-time Communication & Collaboration); no code exists.
- The surfaces it would tie together are partially real in concept: flight telemetry/missions (`01`), the camera/video source (`04`), the shared viewer (`08`), the operator console (`11`), and fleet alerts (`12`). Roles come from `10`.
- Live drone video is a new infrastructure boundary: there is no media/streaming pipeline today, so this domain introduces a real-time transport layer the rest of the platform does not yet have.

## Where We Should Be

- Farm teams have messaging and channels, scoped by `10` roles, tied to fields and missions.
- Live drone video streams from the `04` camera over a new video pipeline into the operator console (`11`) and portal (`13`).
- Collaborative mission planning lets a team co-edit `01` missions, and remote experts annotate a live field through the `08` viewer.
- An emergency alert system ties `01` safety state and `12` fleet alerts into a fast, audited notification path, with session recording for after-action review.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Messaging/channels for farm teams, scoped by `10` roles.
2. Presence and notifications, tied to fields and missions.
3. Live drone video pipeline consuming `04` camera / `01` telemetry (new transport).
4. Emergency alert system tying `01` safety + `12` fleet alerts into an audited path.
5. Collaborative mission planning (shared editing over `01` missions).
6. Remote-expert sessions (annotate a live field via `08`), with session recording/audit and operator-console (`11`) / portal (`13`) integration.

## Primary Crates

New crate(s) TBD (a collaboration/signaling service plus a new media/streaming pipeline). Builds on domains `01` (telemetry/missions), `04` (camera/video), `08` (shared annotation), `11` (operator console), `12` (alerts), and `10` (roles). Sequenced after the core drone platform (`01`-`12`) and gated by the advisor MVP; the live-video pipeline is its own infrastructure track.
