# Real-time Communication and Collaboration: Current State and Target State

## Mission

Let farm teams, stakeholders, and remote experts work a field together in real time: message and coordinate, watch live drone video, co-plan missions, annotate a live scene, and raise emergency alerts on a fast, audited path.

## Current Maturity

greenfield pending (M0 named): no implementation exists; this domain is a product-vision module from `product-summary.md` (#22 Real-time Communication & Collaboration). Nothing in the repository implements messaging, live video streaming, collaborative mission editing, presence, or an emergency alert system.

## What Exists Now

- Nothing is built for this domain. There is no collaboration crate, signaling service, media/streaming pipeline, or alert system.
- Live drone video is a new infrastructure boundary: there is no media transport, encoder/relay, or streaming surface today, so this domain must introduce a real-time media layer the platform does not yet have.
- Adjacent surfaces it would build on (already partially real):
  - Domain `01` (flight and mission control): the mission model collaborative planning co-edits and the telemetry/safety state emergency alerts subscribe to (MAVLink/WebSocket skeleton exists, telemetry thin).
  - Domain `04` (sensor acquisition / camera): the camera/video source the live stream originates from (hardware/sim abstraction exists, real-hardware paths untested).
  - Domain `08` (geo viewer): the Bevy annotation surface a remote expert marks up a live field through.
  - Domain `11` (ground-station UI / operator console): the operations surface streams, sessions, and alerts integrate into (WebSocket client exists, UI thin).
  - Domain `12` (fleet and edge operations): the fleet alert path emergency notifications tie into.
  - Domain `10` (org/roles): the role model collaboration permissions resolve through. Itself greenfield-pending.

## Gaps to Close

- No messaging or channel model for farm teams, scoped to fields and missions.
- No real-time transport, signaling, or session model for live collaboration.
- No live drone video pipeline (encode, relay, view) from the `04` camera.
- No collaborative mission planning (shared/locked editing over `01` missions).
- No remote-expert session that annotates a live field through `08`.
- No presence model or notification fan-out.
- No emergency alert system tying `01` safety and `12` fleet alerts into a fast, audited path.
- No role-based collaboration permissions via `10`.
- No session recording or audit trail for video, edits, and alerts.

## Related Existing Surfaces

- Domain `01` (telemetry/missions): mission model for co-editing; safety/telemetry state for alerts.
- Domain `04` (camera/video): the source the live drone video pipeline streams from.
- Domain `08` (shared annotation): the viewer a remote expert annotates a live field through.
- Domain `11` (operator console): the operations surface streams, sessions, and alerts integrate into.
- Domain `12` (fleet alerts): the fleet alert path emergency notifications tie into.
- Domain `10` (org/roles): the role model collaboration permissions resolve through.
- `docs/reference/product-summary.md` (#22 Real-time Communication & Collaboration): the source description for this module.

## Target Operating Model

- A new collaboration/signaling crate owns channels, presence, sessions, and notifications, scoped by `10` roles and tied to fields and missions.
- A new media/streaming pipeline carries live drone video from the `04` camera into the operator console (`11`) and portal (`13`), treated as its own hardened infrastructure track.
- Collaborative mission planning co-edits `01` missions with conflict-safe locking; every edit is attributed and audited, and dispatch still passes the `01` safety guardrails.
- Remote-expert sessions annotate a live field through `08`, with the annotations persisted and attributed.
- The emergency alert system ties `01` safety state and `12` fleet alerts into a fast, audited notification path — the safety pillar leads here, alongside operability.
- Session recording and an audit trail cover video, edits, and alerts for after-action review.
