# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: 457e3842748483d3 (docs/product-roadmap/02-simulation-digital-twin/*.md + tolerance-profiles.md)
- **Last commit**: 8fa0e7e (batch-02-01 NOT yet committed — awaiting user)
- **Current batch**: batch-02-01 — deterministic runner foundation, single-simulator consolidation
- **Completed batches**: 0 committed (1 implemented + verified, uncommitted)

## Architecture decision (2026-06-11)
Consolidated to a SINGLE simulator: C++ `flight_sim_cpp` is canonical for both interactive AND headless CI regression. Rust `drone_simulator` crate RETIRED (git rm'd, removed from workspace). Orphaned Bevy `simulator/` dir deleted. "Cross-runner parity" → single-runner cross-build determinism.

## batch-02-01 status (implemented + tests pass, uncommitted)
- 02-25 deterministic runner — DONE (flight_sim_cpp `DeterministicRunner`, `agbot_flight_sim_headless --seed` required, byte-identical, manifest emitted)
- 02-01 / 02-02 golden physics+controller regression — DONE (golden `flight_sim_cpp/tests/golden/unit_mission.jsonl`)
- 02-24 TwinContractV1 — SEEDED (contract_version 1.0.0; full schema pending)
- 02-28 scenario manifest — SEEDED (per-run manifest with FNV-1a hashes; full field set pending)
- Verify: `cmake --build flight_sim_cpp/build && ctest --test-dir flight_sim_cpp/build` → 100% pass

## Next action
1. (If user approves) commit batch-02-01.
2. Next batch in flight_sim_cpp: 02-26 safety parity harness, 02-29 trace diff CLI (`agbot-sim diff`), 02-27 terrain no-data model.
