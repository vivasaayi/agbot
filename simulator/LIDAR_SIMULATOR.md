# LiDAR Simulator for AgBot Visualizer

This module adds simulated LiDAR functionality to drones in the AgBot visualizer.

## Features

- **Automatic LiDAR Attachment**: Automatically attaches LiDAR sensors to any drone spawned in the scene
- **2D/3D Scanning**: Supports both 2D horizontal and 3D volumetric LiDAR scanning
- **Real-time Visualization**: Displays point clouds in the 3D scene with distance-based coloring
- **Configurable Parameters**: Adjustable range, resolution, scan frequency, and field of view
- **Export Capability**: Can export scan data in JSON format compatible with the `lidar_mapper` crate

## Configuration

LiDAR sensors are automatically configured with these default settings:
- **Range**: 100 meters
- **Angular Resolution**: 1 degree per ray (360 rays for 2D scan)
- **Scan Frequency**: 10 Hz
- **3D Mode**: Disabled by default (2D horizontal scan)
- **Vertical FOV**: ±15 degrees (for 3D mode)
- **Vertical Resolution**: 2 degrees (for 3D mode)

## Usage

The LiDAR simulator runs automatically when drones are present in the scene:

1. **Spawn a Drone**: Any drone entity will automatically get a LiDAR sensor attached
2. **View Point Cloud**: LiDAR hits are visualized as colored spheres (red = close, blue = far)
3. **Real-time Updates**: Scans update at the configured frequency (default 10 Hz)

## Data Export

To export LiDAR scan data for use with the `lidar_mapper`:

```rust
use crate::lidar_simulator::export_lidar_scan_to_json;

// Export current scan data
export_lidar_scan_to_json(&scan_data, "scan_001.json")?;
```

The exported JSON format matches the schema expected by `lidar_mapper`:
```json
{
  "timestamp": "2025-07-26T12:00:00Z",
  "scan_id": "123e4567-e89b-12d3-a456-426614174000",
  "points": [
    {
      "timestamp": "2025-07-26T12:00:00Z",
      "angle": 0.0,
      "distance": 1000.0,
      "quality": 30
    }
  ]
}
```

## Integration with Physics

Currently uses a simple fallback raycasting implementation. For realistic collision detection, integrate with a physics engine like Bevy Rapier:

1. Add `bevy_rapier3d` to dependencies
2. Set up collision meshes for terrain and obstacles
3. Replace the `cast_ray` function with proper physics raycasting

## Customization

To modify LiDAR parameters, edit the `LidarSensor::default()` implementation or add a configuration system to load settings from files.

## Visualization Controls

- Point cloud colors indicate distance (red = close, blue = far)
- Each scan replaces the previous point cloud visualization
- Points are rendered as small spheres in the 3D scene

## Performance Notes

- 2D scans generate ~360 rays per scan
- 3D scans can generate thousands of rays (depends on resolution settings)
- Point cloud visualization creates individual entities for each hit point
- Consider batching or instancing for large point clouds in production
