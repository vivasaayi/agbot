# AgBot FlightSim C++

This is the first standalone C++ simulator/viewer for the long-term AgBot virtual flight environment.

The initial implementation is intentionally small:

- Pure C++20 simulation core
- Fixed-timestep waypoint follower
- Mission JSON loader for a small AgBot schema
- JSONL telemetry recording
- Deterministic headless executable for repeatable runs
- macOS OpenGL viewer using system Cocoa/OpenGL frameworks only

The renderer is isolated from the simulation core so a Vulkan backend can replace the OpenGL viewer later.

## Build

```bash
cmake -S flight_sim_cpp -B flight_sim_cpp/build
cmake --build flight_sim_cpp/build
ctest --test-dir flight_sim_cpp/build --output-on-failure
```

## Run Headless

```bash
flight_sim_cpp/build/agbot_flight_sim_headless --seed 42
```

The headless runner requires an explicit seed and writes telemetry plus a
manifest to:

```text
flight_sim_cpp/out/telemetry.jsonl
flight_sim_cpp/out/telemetry.manifest.json
```

Every run logs `sim`, `contract`, `seed`, `timestep_ms`, and a deterministic
`run_id`. The same mission, seed, timestep, record interval, max time,
simulator version, and contract schema produce the same `run_id`.

Use a custom mission:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --output flight_sim_cpp/out/sample.jsonl
```

Use explicit trace retention on a dedicated run directory:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --output flight_sim_cpp/out/runs/ci_run.jsonl \
  --trace-retention-keep 20
```

The manifest records deleted traces in `trace_retention_deleted`.

Inject a seeded fault:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --output flight_sim_cpp/out/faulted.jsonl \
  --fault gps_drift:9001:0:-:2.0:gps
```

Fault specs use `class:seed:start_step:end_step:magnitude[:target]`. Use `-`
for an open `end_step`. Supported classes are `wind_gust`, `gps_drift`,
`imu_noise`, `sensor_dropout`, `comm_loss`, `low_battery`, `stale_terrain`,
`bad_tile`, and `actuator_lag`. The runner rejects any injected fault without a
seed. The manifest records `faults`, `faults_hash`, `fault_events`, and
`fault_events_hash`; terrain faults also update `terrain_tiles`.

## Operational Health

Run a structured health check:

```bash
flight_sim_cpp/build/agbot-sim health \
  --seed 42 \
  --last-manifest flight_sim_cpp/out/telemetry.manifest.json \
  --trace-dir flight_sim_cpp/out \
  --cache-dir flight_sim_cpp/out/map_tiles \
  --retention-keep 20
```

The command exits 0 only when runner mode, seed state, tile cache state,
last-run manifest presence, and trace retention compliance pass.

Clear tile caches:

```bash
flight_sim_cpp/build/agbot-sim cache clear --cache-dir flight_sim_cpp/out/map_tiles
flight_sim_cpp/build/agbot-sim cache clear --cache-dir flight_sim_cpp/out/elevation_tiles
```

See [RUNBOOK.md](RUNBOOK.md) for CI triage and operational procedures.

## Diff Traces

Compare two telemetry JSONL traces:

```bash
flight_sim_cpp/build/agbot-sim diff flight_sim_cpp/out/telemetry.jsonl flight_sim_cpp/out/other.jsonl
```

Identical traces exit 0 with `traces identical`. A divergence exits 1 and
names the first differing step and telemetry field.

## Run Viewer

On macOS:

```bash
flight_sim_cpp/build/agbot_flight_sim_viewer
```

Controls:

The viewer has a right-side panel for live telemetry, mission/debug state, and command buttons.
Keyboard controls are still available when the simulator view has focus.
Live viewer runs are recorded automatically to `flight_sim_cpp/out/runs/flight_*.jsonl` and mirrored to `flight_sim_cpp/out/telemetry.jsonl`.
If the mission has a real `home` coordinate, the viewer tries to load OpenStreetMap tiles under the flight area and caches them in `flight_sim_cpp/out/map_tiles`.
The viewer can also load a real-world location directly from latitude/longitude. Use the `Location` button to enter a coordinate and area in square kilometers. The C++ viewer creates a local flight footprint, fetches/caches OSM imagery and AWS Terrarium elevation tiles, and renders an elevation-tinted terrain layer behind the mission.

- `Space`: pause/resume
- `R`: reset mission
- `C`: toggle chase camera
- `B`: toggle globe picker
- `V`: toggle 3D terrain view
- `F`: fit mission camera
- Arrow keys: pan camera
- Mouse wheel, `+`, `-`: zoom
- `M`: toggle manual/autopilot
- `X`: arm/disarm manual flight
- `W/A/S/D`: manual pitch/roll
- `Q/E`: manual yaw
- Up/down arrows: manual throttle
- `T` / `L`: manual takeoff/land assist
- Click: select/add waypoint
- Drag: move selected waypoint
- Option-click: delete waypoint
- Command-S: save edited mission to `flight_sim_cpp/out/edited_mission.json`
- Command-O: load a mission JSON file
- `G`: load/toggle telemetry replay from `flight_sim_cpp/out/telemetry.jsonl`
- Use the replay slider to scrub loaded telemetry.
- Use `Replay File` to load an archived JSONL run from `flight_sim_cpp/out/runs`.
- Use `Location` to load a real-world coordinate and area. Elevation tiles are cached in `flight_sim_cpp/out/elevation_tiles`.
- Use `Globe` to rotate an in-view globe and click a point to load that location through the same C++ terrain pipeline. In Globe mode, mouse wheel and `+`/`-` deep-zoom the globe; the viewer fetches only the visible OSM tile patch, up to z17, and shows cursor/pin coordinates at 6 decimal places.
- Use `3D` or `V` to switch from the 2D map into a perspective terrain view of the loaded area. In 3D mode, drag to orbit, mouse wheel or `+`/`-` zooms the camera, arrow keys pan the target, and `C` follows the drone while it flies over the terrain.

## Mission Planner Bridge

Convert the Rust mission-planner sample format into the C++ local-meter schema:

```bash
flight_sim_cpp/build/agbot_mission_bridge
```

This writes:

```text
flight_sim_cpp/out/bridged_mission.json
```

Run that converted mission:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --seed 42 \
  --mission flight_sim_cpp/out/bridged_mission.json
```

## Mission Coordinates

The simulator flies in local meters:

- `x`: east/west
- `y`: altitude
- `z`: north/south

Mission JSON can also include real earth coordinates. When `home` or `home_position` has `latitude` and `longitude`, the viewer uses that point as the map origin. Waypoints may either keep local `x/y/z` or provide `latitude`/`longitude`/`altitude` directly.

Example waypoint:

```json
{
  "name": "north_entry",
  "action": "fly",
  "x": 0.0,
  "y": 30.0,
  "z": 120.0
}
```

Example geodetic waypoint:

```json
{
  "name": "north_entry",
  "action": "fly",
  "latitude": 36.779378,
  "longitude": -119.417900,
  "altitude": 30.0
}
```

Supported actions:

- `takeoff`
- `fly`
- `loiter`
- `return_home`
- `land`

## Next Milestones

1. Move the viewer from immediate-mode OpenGL calls to the `Renderer` frame boundary.
2. Add terrain/crop-row geometry and field polygons.
3. Replace the file-based mission bridge with live Rust service streaming.
4. Add a Vulkan renderer backend with explicit swapchain, GPU buffers, and debug overlays.
5. Add PX4/ArduPilot SITL visualization through MAVLink telemetry.
