# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 96d5f52 (`batch-20260615101500`)
- **Latest checkpoint commit**: 96d5f52 (`batch-20260615101500`)
- **Current batch**: `batch-20260615104500` (`28-14`)
- **Completed feature rows**: 350 committed; 2 tests_passed; 1 skipped; 1 blocked; 144 pending in this run.
- **Blocker**: None

## Latest verification

- `cargo test -p post_processor vegetation_summary` — pass (3 tests)
- `cargo fmt --all --check` — pass
- `cargo test -p post_processor` — pass (77 tests; doc-tests pass; existing warnings only)

## Next action

- Commit `28-14` vegetation trend shared timeseries integration, then update checkpoint commit metadata.
