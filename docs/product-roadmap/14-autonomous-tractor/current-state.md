# Autonomous Tractor: Current State and Target State

## Mission

Turn a field objective into a safe, GPS/RTK-guided ground operation: register a tractor and implement, plan coverage over a field boundary, execute a prescription map under hard safety guardrails, and log every field-ops session, reusing the flight mission and coordination architecture for a ground vehicle.

## Current Maturity

greenfield pending (M0 named): no implementation exists; this domain is a product-vision module from `product-summary.md` (#7 Autonomous Tractor / AruviTrac). Nothing in the repository implements a tractor registry, ground guidance, implement control, or prescription-map execution.

## What Exists Now

- Nothing is built for this domain. There is no tractor crate, guidance loop, implement adapter, or field-ops session model.
- Adjacent surfaces it would build on and parallel (already partially real):
  - Domain `01` (flight and mission control): the mission identity, dispatch, command-ack, and failsafe patterns a ground vehicle reuses; `mission_control` has a dual-mode (flight/simulation) command skeleton.
  - Domain `03` (multi-drone coordination): the geofence/altitude safety-check model and the multi-vehicle coordination type model — directly applicable to multi-tractor coordination and ground geofencing.
  - Domains `07`/`10` (GIS hub / field-farm-data): field boundaries and prescription/zone layers the coverage planner and implement controller consume.
  - Domains `05`/`09` (imagery / advisor): the management zones and recommendations a prescription map is built from.

## Gaps to Close

- No tractor vehicle identity/registry (vehicle, capabilities, attached implement) linked to org/field via `10`.
- No GPS/RTK guidance loop, path-following controller, or cross-track-error handling.
- No coverage/path planning that turns a field boundary into a fillable swath plan (ground analog of `01` survey patterns).
- No implement control abstraction (planter, sprayer, tiller) or per-zone rate control.
- No ground safety core: geofence, e-stop, obstacle detection, and operator approval before motion.
- No prescription-map execution that consumes management zones from `09`/`05`.
- No field-ops session logging, telemetry persistence, or after-action replay (analog of `04`/`01`).
- No multi-vehicle coordination for ground fleets (analog of `03`).

## Related Existing Surfaces

- Domain `01` (flight and mission control): mission identity, dispatch, command-ack/failsafe, and survey-pattern patterns to reuse.
- Domain `03` (multi-drone coordination): geofence/safety-check and multi-vehicle coordination type model to reuse for ground vehicles.
- Domains `07`/`10` (GIS hub / field-farm-data): field boundaries and prescription/zone storage the planner consumes.
- Domains `05`/`09` (imagery / advisor): management zones and recommendations that define a prescription map.
- `docs/reference/product-summary.md` (#7 Autonomous Tractor): the source description for this module.

## Target Operating Model

- A tractor is a first-class registered vehicle with identity, capabilities, and an attached implement, owned by an org and linked to fields via `10`.
- GPS/RTK guidance follows a planned path with bounded cross-track error; the guidance loop runs in simulation before any real ground motion.
- A field boundary becomes a validated coverage/path plan, the ground analog of the `01` survey-pattern templates.
- Implement control executes a prescription map (management zones from `09`/`05`) with per-zone rate control, retaining raw evidence and reason codes.
- Safety is hard and non-negotiable: geofence, e-stop, obstacle detection, and operator approval gate every motion, with abort always available — this is the dominant pillar for the domain.
- Field-ops sessions are logged with telemetry and are replayable for after-action review; multiple tractors coordinate the way drones do in `03`.
