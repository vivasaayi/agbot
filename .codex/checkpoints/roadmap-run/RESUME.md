# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: a31b7599baeca6ae61e9cb90c3daf82c4a65652c (`batch-27-07`)
- **Latest checkpoint commit**: pending for `batch-27-07` metadata
- **Current batch**: none — STORY `27-07` is implemented, validated, and marked committed in SQLite
- **Completed feature rows**: 155 committed; 1 blocked; 342 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p soil_iot validation_applies_linear_calibration_and_retains_raw_value` — failed before implementation with missing validation/calibration APIs; pass after implementation
- `cargo test -p soil_iot validation` — pass
- `cargo test -p soil_iot` — pass
- `cargo fmt --check` — pass
- `git diff --check` — pass
- `cargo check` — pass with existing warnings

## Next action

Commit the checkpoint metadata for STORY `27-07`, then re-read the checkpoint and select the next deterministic roadmap batch.
