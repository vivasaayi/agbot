# AgBot FlightSim C++

This is the first standalone C++ simulator/viewer for the long-term AgBot virtual flight environment.

The initial implementation is intentionally small:

- Pure C++20 simulation core
- Fixed-timestep waypoint follower
- Mission JSON loader for a small AgBot schema
- JSONL telemetry recording
- Headless executable for repeatable runs
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
flight_sim_cpp/build/agbot_flight_sim_headless
```

This writes telemetry to:

```text
flight_sim_cpp/out/telemetry.jsonl
```

Use a custom mission:

```bash
flight_sim_cpp/build/agbot_flight_sim_headless \
  --mission flight_sim_cpp/samples/sample_field_loop.json \
  --output flight_sim_cpp/out/sample.jsonl
```

## Run Viewer

On macOS:

```bash
flight_sim_cpp/build/agbot_flight_sim_viewer
```

Controls:

- `Space`: pause/resume
- `R`: reset mission
- `C`: toggle chase camera
- Arrow keys: pan camera
- Mouse wheel, `+`, `-`: zoom
- `M`: toggle manual/autopilot
- `W/A/S/D`: manual pitch/roll
- `Q/E`: manual yaw
- Up/down arrows: manual throttle
- `T` / `L`: manual takeoff/land assist
- Click: select/add waypoint
- Drag: move selected waypoint
- Option-click: delete waypoint
- Command-S: save edited mission to `flight_sim_cpp/out/edited_mission.json`
- `G`: load/toggle telemetry replay from `flight_sim_cpp/out/telemetry.jsonl`

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
  --mission flight_sim_cpp/out/bridged_mission.json
```

## Mission Coordinates

The local simulator uses meters:

- `x`: east/west
- `y`: altitude
- `z`: north/south

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
