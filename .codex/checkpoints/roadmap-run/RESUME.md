# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 12afcfec3eef4851be11f139569b21038d475130 (`batch-05-14`)
- **Latest checkpoint commit**: pending checkpoint commit for `batch-05-14` metadata
- **Current batch**: `batch-05-14` / STORY `05-14` — NDVI emissivity and split-window thermal evidence committed
- **Completed feature rows**: 277 committed; 1 skipped; 1 blocked; 219 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p imagery_processor` — pass
- `cargo check -p imagery_processor` — pass
- `cargo fmt --all --check` — pass
- `git diff --check` — pass
- `git diff --cached --check` — pass
- `cargo test -p imagery_processor thermal_lst_uses_ndvi_image_emissivity -- --nocapture` — pass
- `cargo test -p imagery_processor thermal_split_window_records_two_band_method -- --nocapture` — pass
- `cargo test -p imagery_processor thermal_split_window_falls_back -- --nocapture` — pass

## Next action

Select and claim the next pending roadmap batch.
