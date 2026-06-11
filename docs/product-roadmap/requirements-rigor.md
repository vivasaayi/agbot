# Requirements Rigor Model

## Domain Definition of Done

A domain (flight, sensor, imagery, LiDAR, GIS, viewer, advisor, etc.) is not complete until it has:

1. **Identity and provenance**: stable IDs, ownership/field/season linkage, sensor and source metadata, timestamps, and lifecycle state.
2. **Capture or ingest contract**: defined inputs, freshness tracking, coverage, sampling limits, and collection-failure handling.
3. **Geospatial correctness**: CRS, extent, resolution, and transform preserved and asserted; georeferenced outputs round-trip. Tolerance classes are defined in `tolerance-profiles.md` — use named profiles, not inline magic constants.
4. **Deterministic products**: indices, occupancy grids, statistics, or findings computed without an LLM, with reason codes and raw evidence retained.
5. **Pillar posture**: explicit handling of safety, geospatial correctness, data quality, agronomic value, performance/scale, operability, and explainability where relevant.
6. **Interaction**: inspect, compare, annotate, dry-run, execute (where safe), audit, and export.
7. **Agronomic linkage**: outputs tie to a field action — scout, treat, irrigate, re-fly, or report — not a dead-end visualization.
8. **Safety controls**: for flight/coordination, geofence, altitude, no-fly-zone, battery, and abort; for data, validation and integrity checks.
9. **Reporting and export**: API pagination, GeoJSON/CSV/GeoTIFF/PDF export where relevant, saved views, and shareable deliverables.
10. **Tests**: unit tests for math/evaluators, fixture tests for captured/ingested data, API contract tests, and at least one failure path.
11. **Operations**: feature flag or runtime mode, collection/processing health, retry/backoff, cache policy, retention policy, metrics, audit IDs, external-dependency failure behavior, and a support runbook. See "Operability Checklist" below.
12. **Versioned interface contract**: any domain that other domains call must version its wire format. See "Versioned Shared Contracts" below.

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
- Any AI output cites its evidence layer, includes a confidence/uncertainty signal, and refuses when evidence is missing.
- Geospatial outputs assert correct CRS, extent, and resolution using named tolerance profiles from `tolerance-profiles.md`.
- Flight/coordination mutations are impossible without guardrails and abort.
- The output ties to a field action or a downstream product.
- The happy path and one failure path are tested.
- The user can export or share the result where relevant.

## Vertical-Slice Contract

Each backlog row is intended to be shippable as one slice, carrying:

- **Release phase**: M1 foundation, M2 captured, M3 explainable, M4 interactive, or M5 autonomous-assist.
- **Ship size**: S, M, or L based on operational risk and engineering scope.
- **Owner crate**: the Rust crate or C++ component that owns this slice's logic.
- **Input/output contract**: types consumed and produced; versioned schema if shared across domains.
- **Slice**: data contract, collector/processor/adapter, backend API or CLI, deterministic evaluator, UI/overlay, tests, docs, and runbook.
- **API/CLI contract**: endpoint or command behavior, pagination, freshness, audit IDs, error codes, and export support.
- **Geospatial/telemetry contract**: CRS/extent assertions, collection health, freshness, evaluator counts, and action audit metrics. Tolerance class named from `tolerance-profiles.md`.
- **Failure modes**: at least one explicit failure path tested. External dependency failures must produce a named error code, not a panic or silent fallback.
- **Test plan**: unit, fixture, API/CLI contract, UI/overlay, and failure-path coverage. Fixtures must be deterministic (seeded or static).
- **Rollout guardrail**: feature flag or `RUNTIME_MODE`, simulation-first, permissions, and rollback/disable.
- **Audit/provenance behavior**: what is logged, what is immutable, and who can read it.
- **Docs/runbook**: setup, permissions, limits, known failures, triage, and verification.

## Traceability and Dependency Review

Before a story moves from roadmap to implementation, it must pass a dependency and traceability review:

- **Stable identity**: keep the story ID stable; if an ID is split or retired, document the mapping in the domain coverage note.
- **Owner boundary**: name the crate, component, or planned service that owns the behavior. If a greenfield service is still unnamed, use "planned new crate to be named during activation" and define the name before coding.
- **Prerequisite ordering**: every `Depends on` entry must point to an earlier-phase prerequisite, a same-phase foundation story, or an explicitly threaded backbone. If an M2 story depends on an M3 capability, move one of them or document the exception.
- **Versioned contracts**: any output consumed by another domain must name the versioned contract and compatibility expectation.
- **Named tolerances**: any geospatial, raster, point-cloud, image, or telemetry assertion must name a profile from `tolerance-profiles.md`.
- **Failure evidence**: each story must include at least one tested failure path with a reason code or explicit user-visible state.
- **Observability and audit**: operational stories must state the health signal, metric, audit event, and alert/failure behavior they emit.
- **Retention and replay**: telemetry, traces, imagery, LiDAR scans, reports, and advisory turns must name the retained evidence needed to reproduce or audit the result.
- **CI gate**: deterministic products need a fixture, golden file, or contract test that fails on regression and names the divergence.

## Safety Parity — Flight-Adjacent Domains (M1/M2 P0)

For any domain adjacent to flight execution (01, 02, 03, 04, 12, 24, 25), safety guardrails are not M3 polish — they are M1/M2 P0 prerequisites:

- **Pre-flight authorization check**: a gate that blocks dispatch unless all safety conditions are green; must exist before any flight command is wired.
- **Geofence and altitude parity**: the domain must enforce the same geofence, altitude ceiling, and no-fly-zone rules as the authoritative safety source (`01`/`03`). A CI parity test must prove enforcement is identical; a gap in the parity test is a P0 blocking defect.
- **Battery abort parity**: low-battery RTH threshold must match the authoritative value. Never approximate.
- **Command ack/timeout behavior**: every dispatched command must have a documented ack path, a timeout, and a retry or abort fallback.
- **Abort completeness**: the abort/RTH path must be exercised in CI with a test that names what would happen if abort were missing.

The simulation digital twin (domain 02) has a formal safety parity harness defined in its stories (STORY 02-26). Other flight-adjacent domains should adopt the same pattern: a named CI test per safety rule, with a coverage check that fails when a rule is unregistered.

## Versioned Shared Contracts

Any schema shared across two or more domains must be versioned before implementation begins on the second consumer. A consumer that reads an unversioned schema will silently break when the producer changes.

Required contracts to define before deep implementation:

| Contract | Domains | Notes |
| --- | --- | --- |
| `TelemetryV1` | 01, 02, 03, 04, 11, 12 | Position, attitude, battery, link state, timestamps. Already partially defined in `shared`; needs versioning and drift tests. |
| `FlightCommandV1` | 01, 02, 03, 11 | Command enum, payload, ack scheme, timeout behavior. |
| `SimulationTraceV1` | 02 (`flight_sim_cpp` canonical runner) | JSONL telemetry trace format; used by trace diff CLI and golden fixtures. |
| `ScenarioManifestV1` | 02 | Per-run metadata and hash registry; format for replay and audit. |
| `RasterSpatialRefV1` | 05, 06, 07, 08, 22, 28, 32 | CRS, extent, resolution, transform, nodata value. Prevents silent CRS drift. |
| `CaptureRecordV1` | 04, 02, 06, 22 | Flight session, sensor stream, provenance fields. |
| `ProvenanceEventV1` | 30, 04, 05, 06, 09, 22, 23, 28 | Append-only audit event; hash-chained. |
| `AlertEventV1` | 29, 09, 15, 17, 24, 25, 27 | Alert type, severity, source, rule ID, evidence reference, per-alert explanation. |
| `ImportExportJobV1` | 32, 07, 09, 10 | Job state, format, source/target references, CRS, error codes. |
| `SafetyRuleV1` | 01, 02, 03, 12, 24, 25 | Rule ID, threshold, source authority, enforcement mode, violation event, and parity-test coverage. |

For each contract:
1. Define the type in `shared/src/` with a version suffix (e.g. `telemetry_v1.rs`).
2. Write a round-trip serialization test that fails on any schema drift.
3. Add a semver-bump requirement: a breaking change without a version increment is a CI failure.
4. Document the compatibility guarantee: additive changes are backward-compatible; field removals or type changes are breaking.

## Storage Authority Decision

**This must be decided before deep implementation of domains 04, 07, 10, 28, and 30.** Storage backend ambiguity is a requirements blocker, not a preference.

Current state: the roadmap uses PostgreSQL/PostGIS (`mission_planner`, `geo_hub`), SQLite (`geo_hub.db`), and file-based stores (`data_collector`) without a declared authority. This will cause integration failures.

**Recommended resolution**:

| Store | Authoritative Use | Rationale |
| --- | --- | --- |
| PostgreSQL/PostGIS | Operational data: organizations, farms, fields, boundaries, scenes, layers, missions, recommendations, reports, work orders. CRS-aware spatial queries. | Already used in `mission_planner` and `geo_hub`; most capable for spatial queries. |
| File store (object storage or local FS) | Large binary assets: raw LiDAR scans, multispectral frames, orthomosaics, GeoTIFFs, point clouds, video. | These are too large for a row store; object storage is the standard pattern. |
| SQLite | Local edge cache only: telemetry buffering, offline tile cache, CI test fixtures. Not a primary data store. | Appropriate for edge nodes with no network; not a substitute for PostGIS. |
| Provenance ledger | Append-only hash-chained log (domain 30). Can be file-backed or Postgres append table; the key property is immutability and hash chaining. | A separate concern from operational CRUD; do not co-mingle with scene storage. |

Confirm and document the decision in `implementation-sequencing.md` Phase 0 before beginning any new migration or schema work in domains 04, 07, 10, 28, or 30.

## Operability Checklist

Every domain must address each item below before it is considered M3+ complete. Greenfield domains must address it when activated, not when shipped:

- [ ] **Health check**: `GET /health` or `agbot-X health` returns structured JSON (subsystem → pass/fail). Unhealthy subsystems do not return 200.
- [ ] **Retry and backoff**: external calls (hardware, tile fetches, remote APIs) have documented retry limits and exponential backoff. Permanent failures surface as named error codes, not panics.
- [ ] **Cache policy**: any caching layer documents TTL, eviction, invalidation triggers, and behavior on stale data. "Stale" must never silently present as fresh.
- [ ] **Retention policy**: time-bounded data (telemetry logs, trace files, sensor archives) has a documented retention period and a verified deletion/rotation mechanism.
- [ ] **Feature flag / RUNTIME_MODE**: every new operational capability is gated behind `RUNTIME_MODE` or a feature flag so it can be disabled without a redeploy.
- [ ] **Metrics and audit IDs**: operations that mutate state emit a structured event with an audit ID, actor, timestamp, and operation type. Aggregate metrics (counts, durations, error rates) are observable.
- [ ] **External dependency failure behavior**: every external dependency (MAVLink link, tile server, remote API, serial device) has a documented failure behavior: what happens when it is unreachable, slow, or returns bad data. The answer must not be "crash" or "silent zero."
- [ ] **Runbook**: a short doc covering startup, common failure modes, diagnostic commands, and how to disable the capability without data loss.

## Global Priority vs. Domain-Local Priority

**Global P2 does not mean no local P0.**

Domains 13–21 are marked globally P2, meaning they are sequenced after the core platform (01–12) and advisor MVP. This is a sequencing decision, not a quality decision. When any one of these domains is activated for implementation, it must define its own local P0 foundation stories before any M1+ work begins.

Local P0 for a P2 domain means: the domain can be created, stored, listed, and linked to its owners without data loss, with at least one tested failure path, and with the operability checklist addressed for that foundation layer.

Do not start M2 or M3 work in a globally-P2 domain before its local P0 foundation is implemented and tested.

## Greenfield Domain Activation Checklist

"Planned new crate to be named during activation" is acceptable at M0 vision level. Before any implementation work begins on a greenfield domain, the following must be defined:

- [ ] **Owning crate or service name**: the Rust crate (or C++ component) that owns this domain's logic. Name it and add it to `Cargo.toml` as a workspace member.
- [ ] **Source-of-truth data model**: the core entities, their fields, and their relationships. Must be code (a Rust `struct` or SQLx migration), not just prose.
- [ ] **API boundary**: the public interface — HTTP endpoints, CLI commands, or Rust pub API — that other domains call. Versioned per "Versioned Shared Contracts" above.
- [ ] **First mockable external adapter**: identify the first external dependency (hardware, remote API, file source) and define the trait/interface that will be mocked in tests.
- [ ] **Non-goals**: explicitly list what this domain will not do in v1. Prevents scope creep and clarifies integration boundaries for adjacent domains.
- [ ] **Storage authority confirmed**: which backend (Postgres, file store, SQLite edge cache) owns this domain's data, per the Storage Authority Decision above.

## AI and Advisory Domain Gates

All domains that include AI, ML, computer vision, or LLM-based advisory output (domains 05 partial, 23, 26, and any domain that calls `26`) must enforce the following gates before any AI/advisory story is accepted:

1. **Deterministic baseline first**: a non-AI deterministic product must exist and be tested before any AI model is wired. The AI layer augments the deterministic baseline; it does not replace it. (Example: stand count and canopy cover must exist before pest/disease ML detection in domain 23.)

2. **Confidence and uncertainty required**: every AI/ML output must include a machine-readable confidence score or uncertainty estimate alongside the finding. A finding without a confidence value is not a valid output.

3. **Refusal behavior required**: the system must have a documented, tested refusal behavior — what it does when evidence is absent, when confidence is below threshold, or when the input is out of distribution. Silently returning a low-confidence result as if it were reliable is a blocking defect.

4. **Evaluation fixture set required before release**: a labeled test fixture set (ground-truth examples with known correct outputs) must exist and be passing in CI before any AI feature is shipped to users. The fixture set must cover at least one true positive, one true negative, and one edge/ambiguous case.

5. **Human approval required for operational actions**: no AI output may directly trigger a flight command, prescription application, irrigation change, or any physical-world action without an explicit human approval step. Approval must be logged with an audit ID.

6. **Evidence citation required for advisory output**: copilot (domain 26) and recommendation (domain 09) outputs must cite the specific data record, finding, or provenance event that supports the claim. An assertion without a citation is refused, not hedged.

## Open Confirmation Questions

The following questions must be answered before deep implementation of the named domains. They are ordered by blocking impact.

- **[BLOCKING: 04, 07, 10, 28, 30]** Which storage backend is authoritative for scenes and sessions? See "Storage Authority Decision" above for the recommended resolution. Confirm and document before any new migration work begins.
- **[BLOCKING: 01, 02, 03, 12]** In v1, may autonomous missions or swarm maneuvers ever execute without human confirmation, or is every flight approval-gated? The current roadmap assumes approval-gated; confirm this is the shipped behavior for M4.
- **[BLOCKING: 12, edge domains]** Which target hardware is the primary edge baseline: Jetson Nano/Xavier, Raspberry Pi 4+, or x86_64 only for now? Affects ARM cross-compile gate in CI and edge-deployment runbooks.
- Is the commercial flagship the **Field Intelligence Suite advisor workflow** (capture → report), or autonomous flight/swarm operations? The roadmap currently sequences the advisor workflow first.
- Should the domain/control plane (Organization/Farm/Field/Scene) default to self-hosted/local-first, SaaS, or hybrid?
- ~~Should the canonical flight simulator be the Rust/Bevy `simulator` or the C++ `flight_sim_cpp` viewer?~~ Resolved (June 2026): `flight_sim_cpp` is the single canonical simulator for both interactive viewing and headless deterministic CI regression. The Bevy `simulator` crate and Rust `drone_simulator` crate were both retired.
