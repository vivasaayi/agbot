# AgBot Visualizer

A 3D visualization tool for the AgBot drone system built with the Bevy game engine.

## Features

- **Real-time 3D Visualization**: View drone missions in a realistic 3D environment
- **Multi-drone Support**: Track and visualize multiple drones simultaneously
- **Sensor Overlays**: 
  - NDVI (Normalized Difference Vegetation Index) terrain overlays
  - LiDAR point cloud visualization
  - Sensor data display and overlays
- **Interactive HUD**: Real-time telemetry display including:
  - Speed indicator
  - Altitude meter
  - Battery level
  - GPS status
  - Mission progress
  - Compass
- **Mission Replay**: Playback recorded mission data with time controls
- **Live Data**: Connect to running drone systems via WebSocket
- **Camera Controls**: 
  - Free-roam camera
  - Drone following mode
  - Smooth camera transitions

## Controls

### Keyboard
- `WASD` - Move camera
- `Q/E` - Move camera up/down
- `Space` - Pause/unpause
- `H` - Toggle UI
- `I` - Toggle inspector
- `R` - Toggle replay mode
- `←/→` - Control replay speed (in replay mode)
- `ESC` - Exit application

### Mouse
- `Right Click + Drag` - Look around
- `Mouse Wheel` - Zoom in/out

## Configuration

The visualizer can be configured via the `visualizer_config.toml` file:

```toml
# Connection settings
websocket_url = "ws://localhost:8080/ws"
grpc_endpoint = "http://localhost:50051"

# Data paths
terrain_data_path = "./data/terrain"
mission_data_path = "./missions"

[camera]
initial_position = [0.0, 50.0, 50.0]
movement_speed = 10.0
rotation_speed = 2.0

[rendering]
show_ndvi_overlay = true
show_lidar_points = true
show_sensor_data = true
```

## Building and Running

### Prerequisites
- Rust 1.70+ 
- Graphics drivers supporting OpenGL 3.3+ or Vulkan

### Build
```bash
cargo build --bin visualizer
```

### Run
```bash
cargo run --bin visualizer
```

### Development Mode (with dynamic linking for faster builds)
```bash
cargo run --bin visualizer --features bevy/dynamic_linking
```

## Architecture

The visualizer is built using a modular plugin architecture:

- **App Plugin** - Main application setup and configuration
- **Camera Plugin** - Camera controls and movement
- **Communication Plugin** - WebSocket/gRPC communication with drone systems
- **Drone Controller Plugin** - Drone spawning, positioning, and animation
- **Terrain Plugin** - Terrain rendering and sensor overlays
- **HUD Plugin** - Heads-up display elements
- **UI Plugin** - ImGui-style control panels

## Data Formats

### Drone Telemetry
```json
{
  "drone_id": "drone_001",
  "position": [x, y, z],
  "rotation": [x, y, z, w],
  "status": "flying",
  "battery_level": 85.5,
  "altitude": 15.2,
  "speed": 12.0,
  "timestamp": 1635724800.0
}
```

### Mission Data
```json
{
  "mission_id": "mission_001",
  "waypoints": [[x1, y1, z1], [x2, y2, z2], ...],
  "current_waypoint": 2,
  "status": "active"
}
```

## Integration

The visualizer integrates with other AgBot components:

- Connects to `multi_drone_control` for live telemetry
- Reads mission data from `mission_planner`
- Visualizes sensor data from `sensor_overlay_engine`
- Displays processed NDVI data from `ndvi_processor`

## Troubleshooting

### Performance Issues
- Reduce `terrain_resolution` in config
- Disable unnecessary overlays
- Use release build for better performance

### Connection Issues
- Check WebSocket URL in configuration
- Ensure `multi_drone_control` is running
- Check firewall settings

### Graphics Issues
- Update graphics drivers
- Try different rendering backends (add `--features bevy/vulkan`)
- Reduce window resolution
