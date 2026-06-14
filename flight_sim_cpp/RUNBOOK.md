# FlightSim Operability Runbook

This runbook covers the canonical `flight_sim_cpp` simulator used for interactive viewing and headless deterministic CI regression.

## Health Check

Run health against the last manifest produced by a headless run:

```bash
flight_sim_cpp/build/agbot-sim health \
  --seed 42 \
  --last-manifest flight_sim_cpp/out/telemetry.manifest.json \
  --trace-dir flight_sim_cpp/out \
  --cache-dir flight_sim_cpp/out/map_tiles \
  --retention-keep 20
```

Expected healthy output exits 0 and returns JSON with these checks as `pass`:

- `runner_mode`
- `prng_seeded`
- `terrain_cache_state`
- `last_run_manifest_present`
- `trace_retention_compliant`

If `prng_seeded` is `fail`, rerun the simulator with an explicit `--seed N`. Headless deterministic mode intentionally refuses to start without a seed.

## Deterministic Run Header

Every headless run prints the simulator version, contract version, seed, timestep, and deterministic `run_id`. The same mission, seed, timestep, record interval, max time, simulator version, and contract schema produce the same `run_id`.

The sibling manifest records the same `run_id`, input hashes, output hash, PRNG nonce, and retention evidence.

## Wind Field

Use `--wind-mps X,Y,Z` on headless runs to apply a deterministic steady wind vector in m/s:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --wind-mps 3.0,0.0,0.0 \
  --output flight_sim_cpp/out/windy.jsonl
```

The runner records the vector in `weather_config.wind_mps`; nonzero wind also records `source: steady_wind`. A zero vector preserves the no-wind golden trace.

## Sensor Profiles

Use `--sensor-profile NAME` to record deterministic calibration/noise settings for simulated GPS, IMU, barometer, and magnetometer readings:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --sensor-profile cheap_gps \
  --output flight_sim_cpp/out/sensor_profile.jsonl
```

Supported profiles are `ideal`, `cheap_gps`, `rtk_gps`, and `noisy_imu`. The manifest records the selected profile under `sensor_config` with `deterministic_uniform` noise distribution and `sensor_config_hash`.

## LiDAR Raycast

Headless runs emit a deterministic LiDAR sidecar by default. If telemetry is
written to `flight_sim_cpp/out/sample.jsonl`, the LiDAR scans are written to
`flight_sim_cpp/out/sample.lidar.jsonl`.

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --lidar-samples 72,4 \
  --lidar-max-range 100 \
  --lidar-range-noise 0.01 \
  --output flight_sim_cpp/out/lidar.jsonl
```

Each line is a capture-shaped `LidarScan` JSON object consumable by the Rust
`shared::schemas::LidarScan` contract: scan-level `timestamp`, `points`, and
`scan_id`, with point-level `timestamp`, `angle`, `distance`, and `quality`.
Extra point-cloud fields record hit coordinates and ray evidence for regression
tests. The manifest records `lidar_config`, `lidar_config_hash`,
`lidar_scan_count`, and `lidar_output_hash`. Use `--disable-lidar` for
telemetry-only runs.

## Trace Retention

Use retention only on a dedicated run output directory:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --output flight_sim_cpp/out/runs/ci_run.jsonl \
  --trace-retention-keep 20
```

The runner keeps the newest N `.jsonl` traces in the output directory and deletes older traces. The manifest field `trace_retention_deleted` lists the deleted trace and manifest paths.

## Fault Injection

Inject one or more seeded faults with `--fault`:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --output flight_sim_cpp/out/faulted.jsonl \
  --fault gps_drift:9001:0:-:2.0:gps \
  --fault sensor_dropout:1234:100:220:0.0:telemetry
```

Fault specs use `class:seed:start_step:end_step:magnitude[:target]`. Use `-` for an open `end_step`. Supported classes are:

- `wind_gust`
- `gps_drift`
- `imu_noise`
- `sensor_dropout`
- `comm_loss`
- `low_battery`
- `stale_terrain`
- `bad_tile`
- `actuator_lag`

Every fault must provide a seed. The manifest records `faults`, `faults_hash`, `fault_events`, and `fault_events_hash`. Bad-tile faults mark the affected terrain tile as `flat_fallback`; stale-terrain faults mark it as `stale`.

For geodetic missions, the manifest also records requested DEM tile evidence under `terrain_tiles`: CRS (`EPSG:4326`), tile bounds, grid resolution, meter resolution, state, and reason. Missing DEM coverage is explicit `flat_fallback`, not silent zero elevation.

To inspect a faulted trace:

```bash
flight_sim_cpp/build/agbot-sim diff flight_sim_cpp/out/baseline.jsonl flight_sim_cpp/out/faulted.jsonl
```

## Tile Cache

Clear the map-tile cache when terrain or OSM fetch behavior is suspect:

```bash
flight_sim_cpp/build/agbot-sim cache clear --cache-dir flight_sim_cpp/out/map_tiles
flight_sim_cpp/build/agbot-sim cache clear --cache-dir flight_sim_cpp/out/elevation_tiles
```

The command leaves the cache directory present and removes cached entries under it.

## CI Failure Triage

1. Run `just flight-sim-test`.
2. Run `flight_sim_cpp/build/agbot-sim regress` to get the reference case, environment, trace diff, and manifest hash status.
3. If a golden trace differs, run `flight_sim_cpp/build/agbot-sim diff <golden.jsonl> <new.jsonl>` and inspect the first divergent field.
4. If the runner refuses to start, verify `--seed` is present, every `--fault` has a seed, and the mission path exists.
5. If health fails on `last_run_manifest_present`, run a headless simulation and verify the sibling `.manifest.json` was written.
6. If health fails on `trace_retention_compliant`, either lower the trace count by running with `--trace-retention-keep N` or raise the retention policy for that CI job.
