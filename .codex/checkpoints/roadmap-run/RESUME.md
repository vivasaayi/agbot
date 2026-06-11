# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: 457e3842748483d3 (docs/product-roadmap/02-simulation-digital-twin/*.md + tolerance-profiles.md)
- **Last commit**: d7203dd (`batch-02-01` committed)
- **Current batch**: `batch-02-02` — implemented and verified, awaiting commit
- **Completed batches**: 1 committed

## Architecture decision (2026-06-11)
Consolidated to a SINGLE simulator: C++ `flight_sim_cpp` is canonical for both interactive AND headless CI regression. Rust `drone_simulator` crate RETIRED (git rm'd, removed from workspace). Orphaned Bevy `simulator/` dir deleted. "Cross-runner parity" → single-runner cross-build determinism.

## batch-02-01 status (committed)
- 02-25 deterministic runner — DONE (flight_sim_cpp `DeterministicRunner`, `agbot_flight_sim_headless --seed` required, byte-identical, manifest emitted)
- 02-01 / 02-02 golden physics+controller regression — DONE (golden `flight_sim_cpp/tests/golden/unit_mission.jsonl`)
- 02-24 TwinContractV1 — SEEDED (contract_version 1.0.0; full schema pending)
- 02-28 scenario manifest — SEEDED (per-run manifest with FNV-1a hashes; full field set pending)
- Verify: `just flight-sim-test` -> 100% pass; `cargo check` -> pass with pre-existing warnings

## Next action
Commit `batch-02-02` after staging reviewed FlightSim reliability changes:
- 02-26 safety parity harness (`SafetyRules`, required-rule coverage, `DroneSimulation` failsafe integration)
- 02-27 terrain no-data model (`TerrainTileState`, `TerrainTileStatus`, `flat_fallback` composite evidence)
- 02-29 trace diff CLI (`agbot-sim diff`)

Verified: `just flight-sim-test`; `agbot-sim diff` smoke checks for identical and divergent traces.
