# Fleet and Edge Operations: Detailed Stories

Story-level breakdown of the capabilities in `capability-map.md`, ordered by release phase. Each story is one shippable vertical slice. This is the operability backbone under every other domain: harden the deployment surface and define node identity first, make the fleet observable, then add deterministic, reversible config and software distribution. Operability is the dominant pillar throughout — every node mutation must be validated, versioned, and reversible. Format:

- **ID** · phase · size · priority
- **Story**: As a `<role>`, I want `<capability>`, so that `<outcome>`.
- **Operability / evidence** (or **Safety** where it fits): what must be enforced and inspectable without AI.
- **Acceptance**: Given/When/Then, including one failure path.
- **Tests**, **Depends on**.

Roles: `AG` agronomist, `DSP` drone service provider, `GR` grower, `OPS` operator, `PA` platform admin.

---

## M1 — Foundation (hardened deployment + node identity)

### STORY 12-01 · M1 · S · P0 — Validated configuration and runtime modes
- **Story**: As `PA`, I want `AgroConfig` to validate and assert required fields on load, so that a node refuses to start misconfigured instead of failing mid-flight.
- **Operability**: extend `shared/src/config.rs` `load()` to assert required fields and value ranges per `RuntimeMode`; in `Flight` mode, required hardware/server settings must be present; `Simulation` remains the default.
- **Acceptance**:
  - Given a complete config, when a node loads it, then validation passes and the resolved runtime mode is logged.
  - Given a config missing a field required in `Flight` mode, when loaded, then startup fails fast with a clear error naming the field, not a partial boot.
- **Tests**: unit (per-mode required-field assertions), failure path (missing flight field → fail-fast), fixture (sample env/.env).
- **Depends on**: `shared/src/config.rs`, `shared/src/lib.rs` (RuntimeMode).

### STORY 12-02 · M1 · S · P1 — Structured logging with correlation and node fields
- **Story**: As `OPS`, I want logs tagged with a node ID and correlation ID, so that I can trace a request across services and pin it to the node that emitted it.
- **Operability**: extend `init_logging` (`shared/src/lib.rs`) to inject `node_id` and a per-operation correlation ID into the `tracing` context; preserve `RUST_LOG`/`EnvFilter` behavior.
- **Acceptance**:
  - Given a node with an ID, when it logs, then every line carries the `node_id` and any active correlation ID.
  - Given no `node_id` is configured, when logging initializes, then it uses a stable derived fallback and warns once, rather than emitting blank node fields.
- **Tests**: unit (field injection), integration (correlation propagates across a call), failure path (missing node_id → derived fallback + warning).
- **Depends on**: 12-01.

### STORY 12-03 · M1 · S · P0 — Reproducible container build and packaging
- **Story**: As `PA`, I want pinned, reproducible runtime images, so that what I deploy is exactly what I tested.
- **Operability**: pin the toolchain and base image in the multi-stage `Dockerfile`; build `mission_control`, `sensor_collector`, `imagery_processor`, `lidar_mapper`, and `ground_station_ui` into the slim non-root runtime; record an image digest/build manifest.
- **Acceptance**:
  - Given a fixed commit, when the image is built twice, then both produce the same pinned toolchain and a recorded digest.
  - Given an unpinned/floating dependency is introduced, when the build runs in CI, then it is flagged rather than silently drifting.
- **Tests**: build test (digest stability), CI check (no floating pins), failure path (unpinned dep → CI fail).
- **Depends on**: `Dockerfile`.

### STORY 12-04 · M1 · S · P1 — Verified ARM cross-compile artifacts (Jetson/Pi)
- **Story**: As `DSP`, I want CI-verified aarch64 and armv7 artifacts, so that edge nodes on Jetson and Raspberry Pi run binaries I know build and boot.
- **Operability**: run the `Cross.toml` + `justfile` `arm`/`arm64` recipes in CI for `aarch64-unknown-linux-gnu` (Jetson) and `armv7-unknown-linux-gnueabihf` (Pi); publish the artifacts and a smoke-boot check.
- **Acceptance**:
  - Given a commit, when CI runs, then both ARM artifacts are produced and pass a smoke-boot in the target architecture.
  - Given a cross-compile break, when CI runs, then the affected target fails the build rather than shipping an untested artifact.
- **Tests**: CI matrix (aarch64/armv7), smoke-boot, failure path (cross-compile break → target fails).
- **Depends on**: `Cross.toml`, `justfile`; 12-03.

### STORY 12-05 · M1 · M · P0 — Device/drone enrollment and registry
- **Story**: As `PA`, I want each drone and edge node to enroll with a stable ID, capabilities, owner, and runtime mode, so that the fleet has identity to build health, config, and OTA on.
- **Operability**: persist a registry record `{node_id, kind, capabilities[], owner_org_id, runtime_mode, enrolled_at, status}`; enrollment issues/binds a stable identity; node identity links back to fields/owners in domain `10`.
- **Acceptance**:
  - Given a new node, when it enrolls, then it receives a stable ID and a persisted capability/owner record scoped to an org.
  - Given a duplicate enrollment for the same hardware identity, when attempted, then it is rejected (or re-binds the existing ID), never creating a second conflicting record.
- **Tests**: unit (identity binding), API contract (enroll/list/get), failure path (duplicate enrollment → rejected/rebind).
- **Depends on**: `10` (owner/org linkage); 12-01.

### STORY 12-06 · M1 · S · P1 — Node capability and runtime-mode record
- **Story**: As `OPS`, I want each enrolled node to declare its capabilities and current runtime mode, so that I only assign work a node can actually do and never treat a sim node as a flight node.
- **Operability**: capabilities (sensors, compute class, ARM target) and `runtime_mode` are stored on the registry record and refreshed on heartbeat; flight-only operations check the node is in `Flight` mode.
- **Acceptance**:
  - Given an enrolled node, when its capabilities are queried, then they reflect its declared sensors/compute and current runtime mode.
  - Given a node in `Simulation` mode, when a flight-only operation targets it, then it is refused with a mode mismatch, not allowed to proceed.
- **Tests**: unit (capability/mode check), failure path (sim node + flight op → refused).
- **Depends on**: 12-05.

---

## M2 — Captured / Observable (heartbeat, metrics, alerts)

### STORY 12-07 · M2 · M · P0 — Fleet health and maintenance heartbeat
- **Story**: As `OPS`, I want each node to send a heartbeat with version and component status, so that I can see at a glance which nodes are up, stale, or down.
- **Operability**: nodes emit `{node_id, version, components[], uptime, at}` on an interval; the registry records heartbeat freshness and marks a node stale/down past a threshold; surfaced to the operator console (domain `11`).
- **Acceptance**:
  - Given a healthy node, when it heartbeats, then its status shows up with version, component status, and a fresh age.
  - Given a node stops heartbeating past the threshold, when health is evaluated, then it is marked stale then down, never left showing healthy by omission.
- **Tests**: unit (freshness/stale/down transitions), integration (heartbeat stream), failure path (silent node → stale/down).
- **Depends on**: 12-05; surfaced via `11`.

### STORY 12-08 · M2 · S · P1 — Maintenance state and version inventory
- **Story**: As `DSP`, I want a fleet-wide inventory of node versions and maintenance state, so that I know what is deployed where before I roll anything out.
- **Operability**: aggregate heartbeat version/component data into a queryable inventory; nodes can be flagged `maintenance` to exclude them from rollouts and assignment.
- **Acceptance**:
  - Given heartbeating nodes, when the inventory is queried, then it lists versions and maintenance state per node, filterable.
  - Given a node flagged `maintenance`, when a rollout targets the fleet, then that node is excluded.
- **Tests**: unit (inventory aggregation + maintenance exclusion), API contract (inventory query), failure path (maintenance node excluded from rollout set).
- **Depends on**: 12-07.

### STORY 12-09 · M2 · M · P0 — Centralized observability (metrics/tracing)
- **Story**: As `OPS`, I want per-node metrics and traces exported to a central collector, so that I can see fleet behavior beyond per-node `tracing` logs.
- **Operability**: export metrics (resource use, throughput, error rates) and propagate trace/correlation IDs (12-02) to a central collector; export is best-effort and never blocks the node's primary work.
- **Acceptance**:
  - Given a running node, when metrics export, then they arrive at the collector tagged with `node_id` and are queryable centrally.
  - Given the collector is unreachable, when export is attempted, then the node continues operating and buffers/drops bounded, never stalls on the export path.
- **Tests**: unit (metric tagging + non-blocking export), integration (collector ingest), failure path (collector down → node unaffected).
- **Depends on**: 12-02, 12-07.

### STORY 12-10 · M2 · S · P0 — Alerting on health and capacity thresholds
- **Story**: As `OPS`, I want alerts on node-down, low-disk, low-fleet-battery, and processing-stall, so that I am told about failures instead of discovering them later.
- **Operability**: deterministic alert rules over heartbeat/metrics with thresholds and severities; alerts route to the operator console (domain `11`); each alert cites the metric and threshold that fired it.
- **Acceptance**:
  - Given disk crosses the low threshold, when the rule evaluates, then an alert fires with the node, metric value, and threshold, routed to the console.
  - Given metrics within bounds, when rules evaluate, then no alert fires (no flapping/false positives on baseline).
- **Tests**: unit (rule evaluation + severity), integration (alert → console), failure path (baseline → no alert).
- **Depends on**: 12-07, 12-09; routes to `11`.

### STORY 12-11 · M2 · S · P0 — Secrets management
- **Story**: As `PA`, I want DB passwords and tokens moved out of plaintext env/compose, so that credentials are not committed and can be rotated.
- **Safety**: secrets resolve from a managed source (env-injected secret store/mount), not from committed `docker-compose.yml`/`.env`; a CI check fails the build if a plaintext secret is detected; rotation does not require a code change.
- **Acceptance**:
  - Given a managed secret source, when a service starts, then it resolves credentials from it with no plaintext secret in the repo.
  - Given a plaintext secret is committed, when CI runs, then the build fails with the offending location named.
- **Tests**: unit (secret resolution), CI scan (plaintext detection), failure path (committed secret → CI fail).
- **Depends on**: `docker-compose.yml`; before M2 ships per execution rules.

---

## M3 — Explainable (deterministic config distribution + budgets)

### STORY 12-12 · M3 · M · P0 — Signed, versioned config distribution (OTA)
- **Story**: As `PA`, I want to push signed, versioned config to enrolled nodes, so that configuration changes are deliberate, verifiable, and traceable instead of hand-edited per node.
- **Safety**: each config bundle carries a version and signature; a node verifies the signature and version before applying; the applied config version is reported back on heartbeat; unsigned/older bundles are refused.
- **Acceptance**:
  - Given a signed config bundle newer than the node's current version, when pushed, then the node verifies, applies, and reports the new version.
  - Given an unsigned or downgrade bundle, when pushed, then the node refuses it and keeps its current config, reporting the rejection.
- **Tests**: unit (signature + version check), integration (push → apply → report), failure path (unsigned/downgrade → refused).
- **Depends on**: 12-05, 12-07.

### STORY 12-13 · M3 · S · P1 — Config validation and dry-run before apply
- **Story**: As `OPS`, I want a config bundle validated and dry-run against a node before it is applied, so that I catch a bad config before it takes a node down.
- **Operability**: reuse 12-01 validation on the incoming bundle in a dry-run that reports what would change without mutating running state; only a passing dry-run may apply.
- **Acceptance**:
  - Given a config bundle, when dry-run runs, then it reports the diff and validation result without changing the node.
  - Given a bundle that fails validation, when apply is attempted, then it is blocked and the node keeps its current config.
- **Tests**: unit (dry-run diff + validation gate), failure path (invalid bundle → apply blocked).
- **Depends on**: 12-01, 12-12.

### STORY 12-14 · M3 · M · P0 — Edge resource budgeting
- **Story**: As `DSP`, I want per-node CPU/memory/disk budgets enforced on Jetson/Pi-class nodes, so that one workload cannot starve the node or crash it under load.
- **Operability**: enforce configurable CPU/memory/disk limits per node with backpressure when a budget is approached; budget breaches emit a metric/alert (12-10) and shed or throttle work deterministically rather than OOM.
- **Acceptance**:
  - Given a node nearing its memory budget, when load rises, then work is throttled/shed and a budget alert fires, rather than the node OOM-killing.
  - Given disk approaches its budget, when capture continues, then writes are backpressured and flagged, not allowed to fill the disk.
- **Tests**: unit (budget enforcement + backpressure), load test (Pi/Jetson class), failure path (over-budget → throttle/shed, not crash).
- **Depends on**: 12-09, 12-10.

### STORY 12-15 · M3 · S · P2 — Thermal and energy budget for edge nodes
- **Story**: As `DSP`, I want thermal and energy budgeting on flight/edge nodes, so that a node throttles before it overheats or drains its battery mid-mission.
- **Operability**: read thermal/power signals where available, throttle workload against a thermal/energy budget, and report the budget state on heartbeat; absent sensors, the capability is cleanly unavailable, not faked.
- **Acceptance**:
  - Given a node approaching its thermal limit, when load is high, then it throttles and reports the thermal state.
  - Given no thermal sensor, when budgeting runs, then it reports "thermal budget unavailable" rather than a fabricated value.
- **Tests**: unit (throttle on thermal signal), failure path (no sensor → unavailable, not faked).
- **Depends on**: 12-14.

---

## M4 — Interactive (reversible OTA rollout)

### STORY 12-16 · M4 · L · P0 — Staged software/firmware OTA with rollback
- **Story**: As `PA`, I want software/firmware rolled out in stages with automatic rollback on health failure, so that a bad release never takes the whole fleet down.
- **Safety**: rollout proceeds canary→staged→fleet; after each stage, health (12-07) and alerts (12-10) are checked; a health regression halts and rolls the affected nodes back to the prior signed version; no rollout without a rollback path.
- **Acceptance**:
  - Given a new signed version, when the canary stage passes health checks, then rollout advances; when a stage regresses, then it halts and rolls those nodes back automatically.
  - Given a rollback target is missing/unsigned, when a regression occurs, then the rollout refuses to proceed past canary (fail-safe), rather than leaving nodes on a bad version with no path back.
- **Tests**: integration (staged rollout + auto-rollback), unit (health-gate transitions), failure path (no valid rollback target → halt at canary).
- **Depends on**: 12-07, 12-10, 12-12.

### STORY 12-17 · M4 · M · P0 — Rollout control, audit, and simulation gate
- **Story**: As `OPS`, I want to start, pause, and abort a rollout with every action audited, and validated in simulation first, so that fleet mutations are deliberate and accountable.
- **Safety**: rollout start/pause/abort are operator actions audited with `{actor, action, version, stage, at}`; every rollout path is validated in `Simulation` mode against sim nodes before any flight node is targeted.
- **Acceptance**:
  - Given a rollout, when an operator pauses or aborts it, then the action takes effect and is audited with actor and stage.
  - Given a rollout that has not passed simulation, when it targets a flight node, then it is refused until the simulation path passes.
- **Tests**: API contract (start/pause/abort + audit), simulation gate, failure path (unsimulated rollout → refused on flight nodes).
- **Depends on**: 12-16, 12-06 (runtime mode); audit per `10`.

### STORY 12-18 · M4 · S · P1 — Fleet operations dashboard feed
- **Story**: As `OPS`, I want fleet health, alerts, and rollout state exposed as a feed to the operator console, so that I can run the fleet from one place (domain `11`).
- **Operability**: publish health/inventory/alert/rollout state through a stable API consumed by domain `11`; the feed is read-only from the console and reflects real registry state, not cached defaults.
- **Acceptance**:
  - Given fleet activity, when the console reads the feed, then health, alerts, and rollout stage reflect current registry state.
  - Given the feed source is unavailable, when the console reads it, then it reports the gap explicitly rather than showing stale data as current.
- **Tests**: API contract (feed shape + freshness), integration with `11`, failure path (source down → gap surfaced).
- **Depends on**: 12-07, 12-10, 12-16; consumed by `11`.

---

## M5 — Autonomous-Assist (gated advisories)

### STORY 12-19 · M5 · M · P2 — Predictive maintenance advisory
- **Story**: As `DSP`, I want an advisory when a node's metrics trend toward failure (e.g. rising error rate, degrading component), so that I can service it before it fails in the field.
- **Operability**: trends computed from the deterministic metrics/heartbeat history; surfaced as an advisory citing the metric trend, never an automatic maintenance flag or node removal without approval.
- **Acceptance**:
  - Given a node whose error rate trends upward across heartbeats, when the advisory runs, then it flags the node and cites the trend evidence.
  - Given stable metrics, when the advisory runs, then no flag is raised (no false positives on healthy nodes).
- **Tests**: unit (trend detection + evidence), gating test (advisory only, no auto-action), failure path (stable → no flag).
- **Depends on**: 12-07, 12-09.

### STORY 12-20 · M5 · S · P2 — Suggested rollout strategy advisory
- **Story**: As `PA`, I want a suggested staging/timing strategy for a rollout based on fleet state, so that I roll out safely without hand-planning every stage.
- **Safety**: suggestion derived from inventory, health, and maintenance state (12-08); presented as a proposal an operator must approve; it never auto-starts a rollout.
- **Acceptance**:
  - Given current fleet state, when a strategy is suggested, then it proposes canary/stage groupings and cites the state it used, executing nothing.
  - Given incomplete fleet state, when a strategy is requested, then it returns "insufficient state to suggest," not a fabricated plan.
- **Tests**: unit (strategy from state), gating test (no auto-start), failure path (incomplete state → no suggestion).
- **Depends on**: 12-08, 12-16, 12-17.

---

## Coverage note

These ~20 stories cover the 12 capabilities in `capability-map.md`, ordered by phase: harden the existing deployment surface and define node identity (M1), make the fleet observable with heartbeats/metrics/alerts and move secrets out of plaintext (M2), add deterministic, validated config distribution and resource budgeting (M3), then staged, reversible OTA rollout with audit and a simulation gate (M4), with gated advisories last (M5). The curated counts in `release-plan.md` (≈68 feature rows: M1 16 / M2 16 / M3 16 / M4 16 / M5 4) expand several of these into sibling rows — per-component health checks, per-target ARM artifacts, additional alert rules, and per-bundle distribution variants. Two execution rules are enforced as cross-cutting acceptance on every relevant story: every node mutation (config or software) must be validated, versioned, and reversible with a rollback path, and every deployment path is validated in `Simulation` mode before flight nodes are enrolled or targeted. Fleet alerts route to the operator console (domain `11`); node identity links back to fields/owners (domain `10`).
