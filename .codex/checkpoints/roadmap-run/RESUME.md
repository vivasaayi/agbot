# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 80d5f88d4a249a2ced27a5bcb7b51cdde9d6e8ca (`batch-11-02`)
- **Latest checkpoint commit**: d09169983c5d870c8dfa108c12969a52e479f08f (`batch-11-01`; `batch-11-02` checkpoint commit pending)
- **Current batch**: none — ready to select the next deterministic batch after STORY `11-02`
- **Completed feature rows**: 92 committed; 1 blocked; 405 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo fmt` — pass
- `cargo test -p ground_station_ui dispatches_all_websocket_variants_to_typed_routes` — pass
- `cargo test -p ground_station_ui malformed_frame_is_counted_and_preserves_prior_state` — pass
- `cargo test -p ground_station_ui` — pass
- `cargo check` — pass with existing warnings
- `git diff --check` — pass

## Next action

Select the next deterministic roadmap batch after STORY `11-02`.
