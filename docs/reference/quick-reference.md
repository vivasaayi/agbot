# AgBot Terrain Viewer - Quick Reference

> **Note (June 2026)**: the Bevy `visualizer` (`simulator` crate) described below was removed in favor of the canonical C++ simulator. Use `just flight-sim-run` (see `flight_sim_cpp/README.md`) for the flight-sim viewer, or `cargo run -p geo_viewer` for the GIS terrain viewer. The key bindings below are historical.

## Start the App
```bash
cargo run -p visualizer  # removed — see note above
```

## Load Terrain (Choose One)
| Key | Location |
|-----|----------|
| **1** | Nebraska Farm 🌾 |
| **2** | Iowa Corn Belt 🌽 |
| **3** | California Valley 🌄 |
| **4** | Salinas Valley 🥬 |

*Globe auto-hides when terrain loads*

## Move Camera Around
| Input | Action |
|-------|--------|
| **W/A/S/D** | Move forward/left/back/right |
| **Arrow Keys** | Alternative movement |
| **Q / E** | Move down / up |
| **Scroll Wheel** | Zoom in/out |
| **Right Mouse + Drag** | Rotate view (look around) |

**💡 Tip**: Press Q to move DOWN and see elevation changes! That's the best way to see terrain height.

## Overlays (Press to Toggle)
| Key | Overlay | Shows |
|-----|---------|-------|
| **N** | NDVI | 🌿 Vegetation health |
| **C** | CDL | 🌾 Crop types |
| **O** | OSM | 🏠 Fields & buildings |

## Adjust Overlay
| Key | Action |
|-----|--------|
| **[** | Opacity down |
| **]** | Opacity up |

## Color Guide

### NDVI (Vegetation)
- 🟢 Dark Green = Healthy crops
- 🟡 Yellow = Some vegetation
- 🟤 Brown = Bare soil/fields

### CDL (Crops)
- 🟨 Yellow = Corn
- 🟩 Dark Green = Soybeans
- 🟫 Brown = Wheat
- *40+ other colors for different crops*

### OSM (Features)
- 🟩 Green areas = Farmland
- 🔴 Red = Buildings
- 🔵 Blue = Water channels
- ⚫ Gray = Roads

## Keyboard Reference
```
Load Terrain:     1  2  3  4    (or F5  F6  F7  F8)
Movement:         W  A  S  D    (or arrow keys)
Vertical:         Q  E
Rotate View:      Right mouse drag
Zoom:             Scroll wheel
Overlays:         N  C  O
Opacity:          [  ]
```

## What's This Really Showing?

### Satellite Imagery
Real photos from space (ESRI World Imagery) - what the terrain looks like from above.

### Elevation
Heights are encoded in terrain tiles from AWS. Darker areas in the mesh = lower elevation.

### NDVI
Green color = healthy plants. Shows which crops are growing well. Press **Q** to fly high and see the patterns!

### CDL
USDA data showing which crop is planted where. Each color = different crop type.

### OSM
Community data from OpenStreetMap - shows farm boundaries, buildings, roads, irrigation channels.

## Fun Things to Try

1. Load Iowa (press 2)
2. Move forward with W
3. Press E to go high
4. Press N for NDVI overlay
5. See the green crop patterns from above!
6. Press C to see crop types
7. Press [ to dim, ] to brighten

## Troubleshooting

**Q: Terrain looks flat?**
A: Press Q while moving with W - you'll see the terrain height!

**Q: Can't see overlays?**
A: Make sure N or C is pressed (not visible without pressing a key). Use ] to increase opacity.

**Q: Globe still showing?**
A: It should auto-hide. Move camera with Q to go high, or try pressing 1 again.

**Q: No imagery loading?**
A: Check internet - app needs to download from ESRI. Disk cache helps after first load.

**Q: Slow performance?**
A: Press [ to reduce overlay opacity. Try zooming out (scroll wheel up).

---

**Made by AgBot** 🤖 🌾
*Real-world GIS data + Bevy 3D engine = Amazing terrain!*
