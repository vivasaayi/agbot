# Fleet and Edge Operations: Capability Map

This map is service/domain-first. Each capability expands across the relevant pillars (operability, safety, data quality, performance and scale, explainability) and the workstreams in `release-plan.md`. Feature-row counts are curated estimates of shippable vertical slices, not generated.

## Fleet and Edge Operations Domain

| Capability | Current Source Status | Est. Feature Rows | Primary First Slice |
| --- | --- | --- | --- |
| Configuration and runtime modes | strong partial | 7 | Validate `AgroConfig` and assert required fields on load |
| Structured logging | medium partial | 5 | Add correlation IDs and per-node fields to `init_logging` |
| Container build and packaging | medium partial | 6 | Pin toolchain and produce reproducible runtime images |
| ARM cross-compile (Jetson/Pi) | medium partial | 6 | Verified aarch64/armv7 artifacts in CI |
| Device/drone enrollment and registry | missing | 9 | Enroll a node with stable ID, capabilities, runtime mode |
| Fleet health and maintenance tracking | missing | 8 | Node heartbeat with version and component status |
| Centralized observability (metrics/tracing) | missing | 8 | Export per-node metrics to a central collector |
| Alerting | missing | 6 | Alert on node-down and low-disk thresholds |
| Config distribution (OTA) | missing | 8 | Push signed, versioned config to enrolled nodes |
| Software/firmware OTA rollout | missing | 7 | Staged rollout with rollback on health failure |
| Secrets management | missing | 5 | Move DB/token secrets out of plaintext env/compose |
| Edge resource budgeting | missing | 6 | Enforce CPU/memory/disk budgets per node |
