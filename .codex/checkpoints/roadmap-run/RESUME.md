# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 763a1ef5208430c08a584e8f1d2d8e0e444fda53 (`batch-27-08`)
- **Latest checkpoint commit**: pending for `batch-27-08` metadata
- **Current batch**: none — STORY `27-08` is implemented, validated, and marked committed in SQLite
- **Completed feature rows**: 156 committed; 1 blocked; 341 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p soil_iot stuck_sensor_detection_flags_flatline_with_variance_evidence` — failed before implementation with missing stuck-sensor APIs; pass after implementation
- `cargo test -p soil_iot stuck_sensor_detection` — pass
- `cargo test -p soil_iot` — pass
- `cargo fmt --check` — pass
- `git diff --check` — pass
- `cargo check` — pass with existing warnings

## Next action

Commit the checkpoint metadata for STORY `27-08`, then re-read the checkpoint and select the next deterministic roadmap batch.
