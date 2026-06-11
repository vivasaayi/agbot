# Fleet and Edge Operations: Release Plan

## Shipment Strategy

Ship in maturity order. Harden the existing deployment surface and define node identity first (M1), then make the fleet observable with heartbeats, metrics, and alerts (M2), then add deterministic config distribution and resource budgeting (M3), then interactive, reversible OTA rollout with staged release and rollback (M4). This domain maps to milestones M3/M4 (operations, scale, alerts) and underpins every other domain's deployability. Operability is the dominant pillar throughout.

## Phase Counts (curated)

| Release Phase | Est. Feature Rows |
| --- | --- |
| M1 foundation | 16 |
| M2 captured | 16 |
| M3 explainable | 16 |
| M4 interactive | 16 |
| M5 autonomous-assist | 4 |

## Priority Counts

| Priority | Est. Feature Rows |
| --- | --- |
| P0 | 32 |
| P1 | 24 |
| P2 | 12 |

## Ship Size Counts

| Ship Size | Est. Feature Rows |
| --- | --- |
| L | 10 |
| M | 32 |
| S | 26 |

## First P0 Vertical Slices

| Phase | Size | Capability | Pillar | Workstream |
| --- | --- | --- | --- | --- |
| M1 foundation | S | Configuration and runtime modes | operability | identity |
| M1 foundation | M | Device/drone enrollment and registry | operability | identity |
| M1 foundation | S | ARM cross-compile (Jetson/Pi) | performance and scale | packaging |
| M2 captured | M | Fleet health and maintenance tracking | operability | capture |
| M2 captured | M | Centralized observability (metrics/tracing) | operability | capture |
| M2 captured | S | Alerting | operability | capture |
| M3 explainable | M | Config distribution (OTA) | operability | distribution |
| M4 interactive | L | Software/firmware OTA rollout | safety | operations |

## Execution Rules

- Every node mutation (config or software) must be validated, versioned, and reversible; no rollout without a rollback path.
- Validate every deployment path in `Simulation` mode before enrolling flight nodes.
- Secrets are never committed; move them out of plaintext env/compose before M2 ships.
- Every health P0 must record heartbeat freshness and detect stale/down nodes.
- Fleet alerts route to the operator console (domain `11`); node identity links back to fields/owners (domain `10`).
