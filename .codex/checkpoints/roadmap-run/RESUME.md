# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 95bbeca5428c608b09eee0088ba416deba55eac9 (`batch-27-02`)
- **Latest checkpoint commit**: 5b71dc941d5478b968c4237ec044635a874bab80 (`batch-27-01`)
- **Current batch**: none — STORY `27-02` implementation is committed and checkpoint commit is pending
- **Completed feature rows**: 105 committed; 1 blocked; 392 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p soil_iot simulated_gateway_ingest_records_registered_device_readings` — failed as expected before implementation
- `cargo test -p soil_iot` — pass
- `cargo fmt --check` — pass
- `cargo check` — pass with existing warnings
- `git diff --check` — pass

## Next action

Commit the checkpoint metadata for STORY `27-02`, then select the next deterministic roadmap batch.
