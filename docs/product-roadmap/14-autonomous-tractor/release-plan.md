# Autonomous Tractor: Release Plan

## Shipment Strategy

Ship in maturity order, weighted to the M1 foundation because this is a greenfield domain. Vehicle identity and a simulation-only guidance loop (M1) come first, then field-ops telemetry capture (M2), then the deterministic ground-safety core and coverage/prescription logic (M3), then interactive dispatch with guardrails (M4). Autonomous field operation (M5) is gated behind a reliable single-vehicle safety core and the `03` coordination work — and behind operator approval at every step. The safety pillar leads every phase: a ground vehicle moves among people and equipment.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 22 |
| M2 captured | 16 |
| M3 explainable | 18 |
| M4 interactive | 14 |
| M5 autonomous-assist | 6 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 0 |
| P1 | 9 |
| P2 | 67 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 12 |
| M | 38 |
| S | 26 |

## First P0/P1 Vertical Slices

| Phase | Size | Priority | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | M | P1 | Tractor vehicle identity and registry | safety | identity |
| M1 foundation | L | P2 | GPS/RTK guidance and path following | safety | guidance |
| M1 foundation | M | P2 | Coverage / path planning from a boundary | agronomic value | template |
| M2 captured | M | P2 | Field-ops session logging and telemetry | data quality | capture |
| M3 explainable | M | P2 | Geofence and boundary enforcement | safety | evaluator |
| M3 explainable | M | P2 | E-stop and operator approval | safety | evaluator |
| M3 explainable | M | P2 | Obstacle detection | safety | evaluator |
| M4 interactive | M | P2 | Prescription-map execution (consumes `09`/`05`) | agronomic value | operations |

## Execution Rules

- This domain is sequenced AFTER the core drone platform (domains `01`-`12`) and is gated by the advisor MVP: prescription-map execution depends on management zones from `09`/`05` and field context from `07`/`10`.
- The foundational P1 slice is tractor vehicle identity; every other row is P2 (post-MVP).
- Do not enable any real ground motion until the full path — guidance, geofence, e-stop, and obstacle halt — passes in simulation mode.
- Every motion P0/P1 must enforce geofence, e-stop, and operator approval, with a working abort; obstacle detection halts motion in the path.
- Do not start M5 autonomous operation until the single-vehicle safety core and `03`-style coordination are reliable; every step stays operator-approved until then.
