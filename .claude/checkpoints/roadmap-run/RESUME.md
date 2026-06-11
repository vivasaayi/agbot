# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: 457e3842748483d3 (docs/product-roadmap/02-simulation-digital-twin/*.md + tolerance-profiles.md)
- **Last commit**: 594c05d (`batch-02-02` committed)
- **Current batch**: none — ready to start `batch-02-03`
- **Completed batches**: 2 committed

## Architecture decision (2026-06-11)
Consolidated to a SINGLE simulator: C++ `flight_sim_cpp` is canonical for both interactive AND headless CI regression. Rust `drone_simulator` crate RETIRED (git rm'd, removed from workspace). Orphaned Bevy `simulator/` dir deleted. "Cross-runner parity" → single-runner cross-build determinism.

## batch-02-01 status (committed)
- 02-25 deterministic runner — DONE (flight_sim_cpp `DeterministicRunner`, `agbot_flight_sim_headless --seed` required, byte-identical, manifest emitted)
- 02-01 / 02-02 golden physics+controller regression — DONE (golden `flight_sim_cpp/tests/golden/unit_mission.jsonl`)
- 02-24 TwinContractV1 — SEEDED (contract_version 1.0.0; full schema pending)
- 02-28 scenario manifest — SEEDED (per-run manifest with FNV-1a hashes; full field set pending)
- Verify: `just flight-sim-test` -> 100% pass; `cargo check` -> pass with pre-existing warnings

## Next action
Start `batch-02-03`: continue the P0 reliability foundation by completing `TwinContractV1` schema work for 02-24 and expanding scenario manifest schema/hash coverage for 02-28.

Latest verified batch: `batch-02-02` committed as 594c05d. It added 02-26 safety parity harness (`SafetyRules`, required-rule coverage, `DroneSimulation` failsafe integration), 02-27 terrain no-data state evidence (`TerrainTileState`, `TerrainTileStatus`, `flat_fallback` composites), and 02-29 trace diff CLI (`agbot-sim diff`). Verified with `just flight-sim-test` plus `agbot-sim diff` smoke checks for identical and divergent traces.
