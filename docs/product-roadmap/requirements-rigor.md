# Requirements Rigor Model

## Domain Definition of Done

A domain (flight, sensor, imagery, LiDAR, GIS, viewer, advisor, etc.) is not complete until it has:

1. **Identity and provenance**: stable IDs, ownership/field/season linkage, sensor and source metadata, timestamps, and lifecycle state.
2. **Capture or ingest contract**: defined inputs, freshness tracking, coverage, sampling limits, and collection-failure handling.
3. **Geospatial correctness**: CRS, extent, resolution, and transform preserved and asserted; georeferenced outputs round-trip.
4. **Deterministic products**: indices, occupancy grids, statistics, or findings computed without an LLM, with reason codes and raw evidence retained.
5. **Pillar posture**: explicit handling of safety, geospatial correctness, data quality, agronomic value, performance/scale, operability, and explainability where relevant.
6. **Interaction**: inspect, compare, annotate, dry-run, execute (where safe), audit, and export.
7. **Agronomic linkage**: outputs tie to a field action — scout, treat, irrigate, re-fly, or report — not a dead-end visualization.
8. **Safety controls**: for flight/coordination, geofence, altitude, no-fly-zone, battery, and abort; for data, validation and integrity checks.
9. **Reporting and export**: API pagination, GeoJSON/CSV/GeoTIFF/PDF export where relevant, saved views, and shareable deliverables.
10. **Tests**: unit tests for math/evaluators, fixture tests for captured/ingested data, API contract tests, and at least one failure path.
11. **Operations**: feature flag or runtime mode, collection/processing health, retry/backoff, and a support runbook.

## Maturity Levels

| Level | Meaning | Requirement |
| --- | --- | --- |
| M0 | Named | Capability exists only as a roadmap item. |
| M1 | Foundation | Entity/contract can be created, stored, listed, and linked to field/owner/season. |
| M2 | Captured / Observable | Data flows with freshness, coverage, and failure states (telemetry, sensor streams, scene ingest). |
| M3 | Explainable | Deterministic products, scores, and findings with evidence, correct georeferencing, and tests. |
| M4 | Interactive | Safe operator workflows: plan, annotate, dry-run, action, report, export, with audit. |
| M5 | Autonomous-Assist | Bounded autonomy: autonomous missions, swarm coordination, anomaly detection, and advisory loops with approval gates. |

## The Seven Pillars

| Pillar | Question it answers |
| --- | --- |
| Safety | Can this fly/coordinate without harming people, property, or the aircraft? |
| Geospatial correctness | Is every coordinate, extent, and overlay provably right? |
| Data quality | Is the captured data fresh, calibrated, covered, and QA-masked? |
| Agronomic value | Does this output drive a real field action? |
| Performance and scale | Does this hold up on large rasters, dense clouds, and edge compute? |
| Operability | Can it be deployed, observed, and run in the field? |
| Explainability and trust | Is the output defensible, evidence-cited, and audited? |

## Acceptance Bar

For every backlog row, implementation should prove:

- Deterministic logic runs and is inspectable without AI.
- Any AI output cites its evidence layer and flags uncertainty.
- Geospatial outputs assert correct CRS, extent, and resolution.
- Flight/coordination mutations are impossible without guardrails and abort.
- The output ties to a field action or a downstream product.
- The happy path and one failure path are tested.
- The user can export or share the result where relevant.

## Vertical-Slice Contract

Each backlog row is intended to be shippable as one slice, carrying:

- **Release phase**: M1 foundation, M2 captured, M3 explainable, M4 interactive, or M5 autonomous-assist.
- **Ship size**: S, M, or L based on operational risk and engineering scope.
- **Slice**: data contract, collector/processor/adapter, backend API or CLI, deterministic evaluator, UI/overlay, tests, docs, and runbook.
- **API/CLI contract**: endpoint or command behavior, pagination, freshness, audit IDs, error codes, and export support.
- **Geospatial/telemetry contract**: CRS/extent assertions, collection health, freshness, evaluator counts, and action audit metrics.
- **Test plan**: unit, fixture, API/CLI contract, UI/overlay, and failure-path coverage.
- **Rollout guardrail**: feature flag or `RUNTIME_MODE`, simulation-first, permissions, and rollback/disable.
- **Docs/runbook**: setup, permissions, limits, known failures, triage, and verification.

## Open Confirmation Questions

These are not roadmap blockers, but should be confirmed before deep implementation:

- Is the commercial flagship the **Field Intelligence Suite advisor workflow** (capture → report), or autonomous flight/swarm operations? The roadmap currently sequences the advisor workflow first.
- Should the domain/control plane (Organization/Farm/Field/Scene) default to self-hosted/local-first, SaaS, or hybrid?
- Should the canonical flight simulator be the Rust/Bevy `simulator` or the C++ `flight_sim_cpp` viewer — or do they keep distinct roles (in-app vs. standalone)?
- Which storage backend is authoritative for scenes and sessions: PostgreSQL/PostGIS (as `mission_planner`/`geo_hub` suggest), SQLite (`geo_hub.db`), or the file-based `data_collector` store?
- In v1, may autonomous missions or swarm maneuvers ever execute without human confirmation, or is every flight approval-gated?
- Which target hardware is the primary edge baseline: Jetson Nano/Xavier, Raspberry Pi 4+, or x86_64 only for now?
</content>
