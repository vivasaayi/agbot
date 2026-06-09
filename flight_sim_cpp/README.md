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

1. Add manual flight controls beside the waypoint autopilot.
2. Add terrain/crop-row geometry and field polygons.
3. Stream missions and telemetry from the Rust services.
4. Add a Vulkan renderer backend with explicit swapchain, GPU buffers, and debug overlays.
5. Add PX4/ArduPilot SITL visualization through MAVLink telemetry.
