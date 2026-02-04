# AgBot GIS Terrain Viewer - User Guide

## Quick Start

### Loading Real-World Terrain

Press one of these keys to load a demo location:

| Key | Location | What to See |
|-----|----------|------------|
| **1** or **F5** | Nebraska Farm | Flat agricultural land, elevation ~900-1000m |
| **2** or **F6** | Iowa Corn Belt | Gently rolling terrain, elevation ~270-315m |
| **3** or **F7** | California Central Valley | Larger area view, mixed elevation |
| **4** or **F8** | Salinas Valley | Coastal agricultural region |

When you press any of these keys:
- ✅ The globe will automatically hide
- ✅ Real satellite imagery will load from ESRI
- ✅ Terrain mesh will be generated from AWS Terrain Tiles
- ✅ Camera will position itself for optimal viewing

## Camera Controls

### Basic Movement (Terrain View)

| Input | Action |
|-------|--------|
| **W** / **↑** | Move camera forward |
| **S** / **↓** | Move camera backward |
| **A** / **←** | Move camera left |
| **D** / **→** | Move camera right |
| **Q** | Move camera down |
| **E** | Move camera up |
| **Mouse Wheel** | Zoom in/out |
| **Right Mouse Drag** | Rotate view (look around) |

### Tips for Exploring

1. **Start with WASD** to navigate around the terrain
2. **Hold Right Mouse Button** and drag to rotate and look around
3. **Use Q/E** to adjust altitude - very useful for seeing terrain height variation
4. **Scroll wheel** to zoom in/out smoothly
5. **Combine multiple inputs** - e.g., move forward while rotating the view

### Minimum Flight Height
- Camera altitude is clamped to a minimum of 10m to prevent clipping through terrain
- Maximum altitude is 2000m

## Overlay Features

### NDVI - Vegetation Health Index

**Press: N** to toggle

Shows vegetation health using the Excess Green Index (approximation of true NDVI):
- 🟢 **Dark Green** = Healthy dense vegetation (NDVI > 0.5)
- 🟡 **Yellow** = Moderate vegetation (NDVI 0.2-0.4)
- 🟤 **Brown** = Sparse vegetation or bare soil (NDVI < 0.2)

**Three color schemes available** (toggle with UI):
1. **Agriculture** - Optimized for crop monitoring
2. **Scientific** - Red→Yellow→Green→Blue spectrum
3. **Stress Detection** - Green→Yellow→Red (inverted for stress)

### CDL - Crop Classification

**Press: C** to toggle

Shows USDA Cropland Data Layer with 50+ crop types:
- 🌾 Corn, Soybeans, Wheat, Barley
- 🏠 Buildings, Roads, Water features
- 🌲 Forest, Meadow, Grassland

Each crop type has a distinct color based on USDA CDL standards.

### OSM - OpenStreetMap Features

**Press: O** to toggle

Shows geographic features from OpenStreetMap:
- 👨‍🌾 **Farmland** - Agricultural parcels
- 🏠 **Buildings** - Barns, silos, farmhouses
- 🛣️ **Roads** - Farm tracks and paths
- 💧 **Water** - Streams, ditches, irrigation channels

### Adjust Overlay Opacity

| Key | Action |
|-----|--------|
| **[** | Decrease overlay opacity (more transparent) |
| **]** | Increase overlay opacity (more opaque) |

Opacity range: 0.0 (invisible) to 1.0 (fully opaque)

## What You're Seeing

### Satellite Imagery
- **Source**: ESRI World Imagery
- **Resolution**: 256x256 tiles at zoom level 14
- **Update frequency**: Real satellite data (historical, not real-time)

### Elevation Data
- **Source**: AWS Terrain Tiles (Terrarium format)
- **Format**: 256x256 PNG tiles with RGB-encoded elevation
- **Height range**: Actual elevation in meters

### NDVI Computation
- **Method**: Pseudo-NDVI using visible bands (RGB)
- **Formula**: ExG = (2*G - R - B) / (R + G + B)
- **Accuracy**: Approximates true NDVI (which requires NIR band)

### Crop Classification
- **Source**: USDA NASS CropScape
- **Data Year**: 2023 (most recent available)
- **Coverage**: Continental United States only

## Performance Tips

1. **Zoom varies the mesh detail** - Close up shows more terrain variation
2. **Overlays blend over satellite imagery** - All overlays are semi-transparent by default
3. **Cache persists** - Tiles are cached locally in `~/.cache/agbot/tiles/`
4. **LRU memory management** - Uses Least Recently Used eviction for 256 tiles max in memory

## Keyboard Shortcut Reference

### Load Locations
- 1, 2, 3, 4 (or F5-F8)

### Overlays
- N = NDVI toggle
- C = CDL toggle  
- O = OSM toggle
- [ = Decrease opacity
- ] = Increase opacity

### Camera (Terrain View)
- WASD = Movement
- QE = Vertical
- Arrow keys = Alternative movement
- Right mouse drag = Rotate view
- Scroll wheel = Zoom

## Troubleshooting

### Issue: Earth globe still showing
**Fix**: This should now auto-hide when terrain loads. If not visible, the globe might be behind the terrain. Try pressing 'Q' to move the camera up.

### Issue: Terrain looks very flat
**Reason**: Camera might be looking straight down. Try rotating with right mouse drag or moving with WASD + mouse rotation to see elevation variation.

### Issue: No imagery loading
**Reason**: Network connectivity issue. The app requires internet to fetch ESRI imagery. Check your internet connection and try pressing the location key again.

### Issue: Slow performance
**Tips**:
- Reduce overlay opacity with [
- Look away from overlays temporarily
- Camera can handle 512-1024 tiles in memory efficiently

## Data Attribution

- **Elevation**: AWS Terrain Tiles (based on SRTM/3DEP)
- **Imagery**: ESRI World Imagery (Basemap)
- **Crop Data**: USDA NASS CropScape
- **Features**: OpenStreetMap contributors
- **Map Tiles**: Slippy map standard (Web Mercator projection)

## Advanced Features

### RGB Encoding in Elevation Data
AWS Terrain Tiles use this formula to encode elevation:
```
height_meters = (R * 256 + G + B/256) - 32768
```

### Web Mercator Projection
Tile coordinates use standard Web Mercator tiling scheme (z/x/y format).

### Tile Cache Structure
```
~/.cache/agbot/tiles/
├── elevation/
│   ├── 14/
│   │   ├── 2626/
│   │   │   └── 6440.bin
├── imagery/
├── ndvi/
├── cdl/
└── osm/
```

## Next Steps

Explore these agricultural regions:
1. **Iowa Corn Belt** - Peak crop production area
2. **California Central Valley** - Diverse crop types
3. **Nebraska Panhandle** - Large-scale irrigation
4. **Salinas Valley** - Intensive vegetable production

Try the different overlays to understand crop health and distribution! 🌾
