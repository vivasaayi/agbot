# Fleet and Edge Operations: Epic Breakdown

## Epic Template

Each epic ships as a vertical slice:

- API/CLI: registry or distribution endpoint/command, persistence, auth, pagination, and audit events.
- Safety: config/OTA changes are validated and reversible; no node mutation without a rollback path.
- Deterministic: enrollment, health, and rollout logic that runs without AI; signed and versioned artifacts.
- Telemetry: node heartbeat, metrics, and tracing with freshness and gap detection.
- UI: fleet health and alerts surfaced to the operator console (domain `11`).
- Tests: unit (config validation, version compare), fixture (recorded node reports), API contract, and one failure path (node-down, bad rollout).
- Operations: runtime mode (`Simulation` first), staged rollout, rollback, and a runbook.

## Category Epics

### EPIC-01: Configuration, Packaging, and Edge Targets
- Goal: a hardened, reproducible deployment surface for x86_64 and Jetson/Pi.
- First release: validated `AgroConfig` load, pinned container build, and verified aarch64/armv7 artifacts.
- Expansion: correlation IDs and per-node fields in structured logs; secrets pulled out of plaintext env/compose.
- Hardening: reproducible builds, CI cross-compile checks, and a deployment runbook.

### EPIC-02: Device Registry and Fleet Health
- Goal: know every node and its state.
- First release: enroll a node with a stable ID, capabilities, and runtime mode; report a heartbeat.
- Expansion: version, component status, and maintenance state; surface fleet health to domain `11`.
- Hardening: stale-node detection, ownership/field linkage, and registry contract tests.

### EPIC-03: Observability, Alerting, and Distribution
- Goal: see the fleet centrally and change it safely.
- First release: export per-node metrics to a central collector and alert on node-down/low-disk.
- Expansion: signed, versioned config distribution (OTA) and per-node resource budgeting.
- Hardening: staged software rollout with rollback on health failure, plus bad-rollout failure-path tests.
