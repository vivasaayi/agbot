# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: b565a68 (`batch-20260615001402`)
- **Latest checkpoint commit**: this checkpoint commit after b565a68 (`batch-20260615001402`)
- **Current batch**: none
- **Completed feature rows**: 403 committed; 1 tests_passed; 2 skipped; 1 blocked; 91 pending in this run.
- **Blocker**: None

## Latest verification

- `cargo test -p shared tractor_guidance` — pass
- `cargo test -p shared tractor_cross_track` — pass
- `cargo check -p shared` — pass
- `14-02` — committed as simulation-only GPS/RTK straight-path guidance with cross-track halt faults

## Next action

- Select and claim the next pending feature (`14-03` is the next P2 item).
