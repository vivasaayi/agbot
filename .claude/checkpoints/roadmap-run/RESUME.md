# Roadmap Run — Resume

- **Run ID**: run-02-sim
- **Roadmap hash**: f7f689d5947be4fbf236914925c1af7693d98188
- **Last commit**: d76d8c00682b7601627206b26f0436d798ea36c4 (`batch-02-05` implementation commit)
- **Current batch**: none — ready to start `batch-02-06`
- **Completed batches**: 5 committed
- **Blocker**: none

## Latest verification

- `cmake --build flight_sim_cpp/build --target agbot_flight_sim_tests agbot_sim agbot_flight_sim_headless` — pass
- `./flight_sim_cpp/build/agbot_flight_sim_tests` — pass
- `just flight-sim-test` — pass
- `agbot_flight_sim_headless --fault gps_drift:9001:0:-:2.0:gps` — pass; manifest records fault hashes/events and a fault-specific `run_id`
- `agbot_flight_sim_headless --fault bad_tile:777:0:-:0.0:terrain/tile/z12/x655/y1583` — pass; manifest records `flat_fallback`
- `agbot_flight_sim_headless --fault gps_drift:-:0:-:1.0:gps` — expected fail on missing fault seed
- `agbot-sim diff` baseline vs GPS-fault trace — expected divergence at `position.x`

## Next action

Start `batch-02-06`: claim and implement STORY `02-09` real DEM terrain with CRS/extent assertions as the next foundational P0 terrain dependency before LiDAR/camera/preview stories.
