# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 695df7817da3637e3450de568236100846447688 (`batch-27-04-27-05`)
- **Latest checkpoint commit**: 50eae782aed35a7233427c272a08c50f56f66dc6 (`batch-25-03`)
- **Current batch**: none — STORIES `27-04` and `27-05` are implemented and checkpoint commit is pending
- **Completed feature rows**: 128 committed; 1 blocked; 369 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p soil_iot geolocated_reading_inherits_device_position_and_series_contract` — failed as expected before implementation with missing geolocated reading/time-series APIs; pass after implementation
- `cargo test -p soil_iot reading_with_invalid_device_position_is_flagged_no_geolocation_without_default_point` — pass
- `cargo test -p geo_hub soil_iot_readings_inherit_geolocation_and_persist_via_timeseries` — pass
- `cargo test -p geo_hub soil_iot_reading_with_invalid_device_position_is_flagged_not_defaulted` — pass
- `cargo test -p soil_iot` — pass
- `cargo test -p geo_hub soil_iot` — pass
- `cargo fmt --check` — pass
- `cargo check` — pass with existing warnings
- `git diff --check` — pass

## Next action

Commit the checkpoint metadata for STORIES `27-04` and `27-05`, then re-read the checkpoint and select the next deterministic roadmap batch.
