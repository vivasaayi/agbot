# Autonomous Tractor

A self-driving tractor (AruviTrac): GPS/RTK-guided navigation, automated implement control, and prescription-map execution over a field boundary, reusing the flight mission and safety architecture for a ground vehicle.

## Where We Are

- Not started / vision only. This is a greenfield product-vision module sourced from `docs/reference/product-summary.md` (#7 Autonomous Tractor / AruviTrac); no code exists.
- The patterns it would reuse are partially real: mission/control (`01`), multi-vehicle coordination and safety (`03`), field boundaries and prescriptions (`07`/`10`), and management zones (`05`/`09`).
- A ground vehicle moves among people, equipment, and obstacles, so the safety pillar dominates this domain more than any other.

## Where We Should Be

- A tractor is a registered vehicle with identity, capabilities, and an attached implement.
- A field boundary becomes a validated coverage/path plan; the tractor follows it under GPS/RTK guidance.
- Implement control (planter, sprayer, tiller) executes a prescription map (management zones from `09`/`05`) with rate control per zone.
- Geofence, e-stop, obstacle detection, and operator approval gate every motion; nothing moves without guardrails and abort.
- Field-ops sessions are logged with telemetry, and multiple vehicles coordinate the way drones do in `03`.

## Files

- `current-state.md`: maturity, what exists now (nothing; adjacent surfaces), related existing surfaces, and target operating model.
- `capability-map.md`: intended capability coverage and curated feature-row counts.
- `epics.md`: delivery slices.
- `release-plan.md`: phased backlog by maturity, priority, ship size, and first P1 slices.

## Build Order

1. Tractor vehicle identity/registry linked to org/field (via `10`).
2. GPS/RTK guidance and a field-ops session/telemetry log (parallels `01`/`04`).
3. Coverage/path planning from a field boundary (parallels `01` survey patterns).
4. Safety core: geofence, e-stop, obstacle detection, operator approval (parallels `03`).
5. Implement control and prescription-map execution (consumes `09`/`05` zones).
6. Multi-vehicle coordination (parallels `03`).

## Primary Crates

New crate(s) TBD (a ground-vehicle control plane plus guidance/implement adapters). Builds on and parallels domains `01` (mission/control), `03` (coordination/safety), `07`/`10` (boundaries/prescriptions), and `05`/`09` (management zones).
