# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 6724746f74529016571ffdf463c9fcc76a08befe (`batch-27-11`)
- **Latest checkpoint commit**: 37c33f16ddf5fee65c2a5525da6d122844ed436e (`batch-26-10` metadata; `batch-27-11` checkpoint pending)
- **Current batch**: none
- **Completed feature rows**: 201 committed; 1 blocked; 296 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p soil_iot sensor_health_monitor_emits` — failed before implementation with missing sensor-health APIs and missing `alerting` dependency; pass after implementation with 1 focused test
- `cargo test -p soil_iot sensor_health_monitor_resolves` — pass with 1 focused test
- `cargo test -p soil_iot sensor_health_monitor_detects` — pass with 1 focused test
- `cargo test -p soil_iot` — pass with 19 unit tests and 0 doc tests
- `cargo check -p soil_iot` — pass
- `cargo fmt --check` — pass
- `git diff --check` — pass

## Next action

Select and claim the next deterministic P0 roadmap batch.
