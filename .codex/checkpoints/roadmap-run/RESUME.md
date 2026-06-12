# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 07880d45739244f4379a8f9bc888314fa537a791 (`batch-22-01`)
- **Latest checkpoint commit**: c0d3e6ad75070225eaafabd8415350ce3816ea7b (`batch-12-05`; `batch-22-01` checkpoint commit pending)
- **Current batch**: none — ready to select the next deterministic batch after STORY `22-01`
- **Completed feature rows**: 95 committed; 1 blocked; 402 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p orthomosaic` — pass
- `cargo test -p geo_hub orthomosaic_frame_set_ingest` — pass
- `cargo test -p geo_hub --test products_api` — pass
- `cargo test -p geo_hub` — pass
- `cargo fmt --check` — pass
- `cargo check` — pass with existing warnings
- `git diff --check` — pass

## Next action

Select the next deterministic roadmap batch after STORY `22-01`.
