# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 72f7e17fdf4014469a671f8f2ecc0cca3d52f7ec (`batch-27-12`)
- **Latest checkpoint commit**: 560be79e077a23aa1094f9526988bd68376512a9 (`batch-27-11` metadata; `batch-27-12` checkpoint pending)
- **Current batch**: none
- **Completed feature rows**: 202 committed; 1 blocked; 295 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p soil_iot irrigation_trigger_emits` — failed before implementation with missing irrigation trigger API; pass after implementation with 1 focused test
- `cargo test -p soil_iot irrigation_trigger_suppresses` — pass with 1 focused test
- `cargo test -p soil_iot` — pass with 21 unit tests and 0 doc tests
- `cargo check -p soil_iot` — pass
- `cargo fmt --check` — pass
- `git diff --check` — pass

## Next action

Select and claim the next deterministic P0 roadmap batch.
