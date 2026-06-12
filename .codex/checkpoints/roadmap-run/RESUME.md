# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: cb56fee9f4f727af1f60940dc5c344585277d996
- **Last implementation commit**: 2a58b72a039b505c854e92ec2f0ed613899afa12 (`batch-27-01`)
- **Latest checkpoint commit**: 7f57d2203c153bd8d258780109951d36d6d490a8 (`batch-26-02`)
- **Current batch**: none — STORY `27-01` implementation is committed and checkpoint commit is pending
- **Completed feature rows**: 104 committed; 1 blocked; 393 pending rows remain in the full-roadmap inventory
- **Blocker**: STORY `07-11` is blocked on the documented storage-authority confirmation question.

## Latest verification

- `cargo test -p geo_hub soil_iot_device_registry_registers_and_lists_geolocated_devices` — failed as expected before implementation
- `cargo test -p soil_iot` — pass
- `cargo test -p geo_hub soil_iot_device_registry_registers_and_lists_geolocated_devices` — pass
- `cargo test -p geo_hub` — pass
- `cargo fmt --check` — pass
- `cargo check` — pass with existing warnings
- `git diff --check` — pass

## Next action

Commit the checkpoint metadata for STORY `27-01`, then select the next deterministic roadmap batch.
