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
2. If golden traces differ, run `flight_sim_cpp/build/agbot-sim diff <golden.jsonl> <new.jsonl>` and inspect the first divergent field.
3. If the runner refuses to start, verify `--seed` is present, every `--fault` has a seed, and the mission path exists.
4. If health fails on `last_run_manifest_present`, run a headless simulation and verify the sibling `.manifest.json` was written.
5. If health fails on `trace_retention_compliant`, either lower the trace count by running with `--trace-retention-keep N` or raise the retention policy for that CI job.
