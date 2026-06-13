# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 9fdc3622811530c3647d2b0f8e6a1ec230a1ef95 (`batch-04-14`)
- **Latest checkpoint commit**: pending checkpoint commit for `batch-04-14` metadata
- **Current batch**: `batch-04-14` / STORY `04-14` — geospatial session export committed
- **Completed feature rows**: 274 committed; 1 skipped; 1 blocked; 222 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p data_collector` — pass
- `cargo check -p data_collector` — pass
- `cargo fmt --all --check` — pass
- `git diff --check` — pass
- `git diff --cached --check` — pass
- `cargo test -p data_collector test_export_session_kml -- --nocapture` — pass
- `cargo test -p data_collector test_export_session_gated_formats -- --nocapture` — pass

## Next action

Select and claim the next pending roadmap batch.
