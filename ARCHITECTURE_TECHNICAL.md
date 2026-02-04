# AgBot GIS Architecture & Technical Deep Dive

## System Overview

```
┌─────────────────────────────────────────────────────────┐
│                  BEVY 3D ENGINE (0.14.2)                │
│  ECS Architecture: Systems, Components, Resources        │
└─────────────────────────────────────────────────────────┘
                           ↓
            ┌──────────────────────────────┐
            │   GIS PIPELINE SYSTEM        │
            └──────────────────────────────┘
                           ↓
        ┌─────────────────┬──────────────┬─────────────────┐
        ↓                 ↓              ↓                 ↓
    ELEVATION        IMAGERY         TERRAIN MESH     OVERLAYS
    AWS Terrain      ESRI World      3D Mesh Gen      NDVI/CDL/OSM
    Tiles            Imagery         Displace          Color Maps
        ↓                 ↓              ↓                 ↓
    ┌───────────────────────────────────────────────────────┐
    │         TILE CACHE (LRU + Disk Persistence)           │
    └───────────────────────────────────────────────────────┘
        ↓                 ↓              ↓                 ↓
    ELEVATION         IMAGERY        NDVI           CDL
    Cache             Cache          Cache          Cache
```

## Component Lifecycle

### 1. User Interaction
```rust
// User presses '1' key
KeyCode::Digit1 pressed
    ↓
event_writer.send(LoadRealWorldEvent::nebraska_farm())
```

### 2. Async Data Fetching
```rust
// RealWorldLoaderPlugin spawns tokio task
rt.spawn(async {
    // Parallel fetching (async/await)
    let elevation = fetch_elevation_for_bounds().await
    let imagery = fetch_imagery_for_bounds().await
    
    // Check cache first, network second
    cache.get_tile() → cache.store_tile()
    
    return LoadedWorldData { ... }
})
```

### 3. Mesh Generation
```rust
// Back on main Bevy thread
composite_elevation()  // Interpolate tiles → heightmap
composite_imagery()    // Blend satellite tiles → texture

compute_pseudo_ndvi_from_tiles()  // RGB → NDVI values
ndvi_to_texture()                 // NDVI → color overlay

create_terrain_mesh()             // Heightmap → 3D geometry
```

### 4. Rendering
```rust
// Bevy spawns PbrBundle with:
- Mesh (128x128 vertices with heights from elevation)
- Material (StandardMaterial with satellite texture)
- RealTerrain component (bounds, elevation range, overlays)
- Transform (positioned at world origin)
```

### 5. Visibility Management
```rust
// Automatic globe hide/show
if has_real_terrain:
    globe.visibility = Hidden
else:
    globe.visibility = Visible
```

### 6. Camera Control
```rust
// Input handling
Input → Camera Transform
- WASD: translation
- Mouse: rotation (local Y-axis yaw, local X-axis pitch)
- Scroll: forward/backward movement
- Q/E: Y-axis translation
- Clamped to min=10m, max=2000m height
```

## Data Structures

### Main Components

```rust
// Represents a loaded real-world location
#[derive(Component)]
pub struct RealTerrain {
    pub bounds: GeoBounds,                    // lat/lon box
    pub min_elevation: f32,                   // meters
    pub max_elevation: f32,                   // meters
    pub ndvi_texture: Option<Handle<Image>>,  // cached overlay
    pub cdl_texture: Option<Handle<Image>>,   // cached overlay
    pub base_texture: Option<Handle<Image>>,  // satellite
    pub active_overlay: TerrainOverlay,       // current mode
}

#[derive(Component)]
pub struct TerrainCamera {
    pub height_above_ground: f32,
    pub zoom_distance: f32,
    pub min_height: f32,
    pub max_height: f32,
}
```

### Resources

```rust
// Configuration
#[derive(Resource, Clone)]
pub struct TerrainMeshConfig {
    pub mesh_resolution: u32,         // 128
    pub texture_resolution: u32,      // 512
    pub vertical_scale: f32,          // exaggeration
    pub show_ndvi: bool,              // toggle
    pub show_cdl: bool,               // toggle
    pub show_osm: bool,               // toggle
    pub overlay_opacity: f32,         // 0.0-1.0
}

#[derive(Resource)]
pub struct TileCache {
    pub cache_dir: PathBuf,
    pub max_memory_tiles: usize,      // 256
    pub max_concurrent_requests: usize, // 4
    memory_cache: HashMap<String, CachedTile>,
    lru_order: VecDeque<String>,      // LRU tracking
    pub stats: CacheStats,
}

#[derive(Resource)]
pub struct NdviConfig {
    pub enabled: bool,
    pub opacity: f32,
    pub min_threshold: f32,
    pub color_scheme: NdviColorScheme,
}
```

## Tile System

### Web Mercator Projection
```rust
// Convert lat/lon to tile coordinates
let tile = TileCoord::from_latlon(lat, lon, zoom);
// z=13: ~8000m tiles, z=14: ~4000m tiles

// Tile URL patterns:
// Elevation: .../aws-terrain/z/x/y.png
// Imagery:   .../ArcGIS/tile/z/y/x.jpg (note y,x reversed!)
```

### Cache Key System
```rust
// Format: "TileType_z_x_y"
// Example: "Imagery_14_2626_6440"

// Storage:
~/.cache/agbot/tiles/
├── elevation/14/2626/6440.bin
├── imagery/14/2626/6440.bin
├── ndvi/14/2626/6440.bin
├── cdl/14/2626/6440.bin
└── osm/14/2626/6440.bin
```

### LRU Eviction Algorithm

```rust
// 1. Check memory cache
cache.get_tile(coord) {
    if memory_cache.contains(key) {
        lru_order.remove(key)
        lru_order.push_back(key)  // Move to end (most recent)
        return tile
    }
}

// 2. Check disk
if disk.exists(path) {
    tile = load_from_disk()
    memory_cache.insert(key, tile)
    lru_order.push_back(key)
    evict_if_needed()
}

// 3. Evict LRU when limit reached
while memory_cache.len() > max (256) {
    oldest = lru_order.pop_front()  // Remove from front
    memory_cache.remove(oldest)
    stats.evictions += 1
}
```

## Elevation Encoding (AWS Terrarium Format)

```rust
// RGB pixels encode elevation in this formula:
elevation_meters = (R*256 + G + B/256) - 32768

// Example:
// RGB(128, 100, 128)
// height = (128*256 + 100 + 128/256) - 32768
//        = 32868 - 32768
//        = 100m

// This gives -32768m to 32767m range with 0.01m precision
```

## Pseudo-NDVI Computation

```rust
// RGB → NDVI approximation (Excess Green Index)
for each pixel (R, G, B):
    // Extract channel values
    r = R / 255.0
    g = G / 255.0
    b = B / 255.0
    
    // Compute Excess Green (correlates with NIR-based NDVI)
    exg = (2*g - r - b) / (r + g + b + 0.001)
    
    // Clamp to valid range
    ndvi = clamp(exg, -1.0, 1.0)

// Color mapping:
if ndvi < 0.0:
    // Water/bare soil: brown→gray
    color = interpolate(brown, gray, (ndvi+1)/1)
else if ndvi < 0.3:
    // Low vegetation: yellow→light green
    color = interpolate(yellow, lightgreen, ndvi/0.3)
else:
    // Dense vegetation: light→dark green
    color = interpolate(lightgreen, darkgreen, (ndvi-0.3)/0.7)
```

## OSM Feature Classification

```rust
// Overpass API query structure
[out:json][timeout:30];
(
  way["landuse"~"farmland|vineyard|orchard"](bbox);
  way["building"~"barn|silo|farm"](bbox);
  way["highway"~"track|path"](bbox);
  way["waterway"~"stream|ditch|canal"](bbox);
);
out body geom;

// Response parsing
for each element {
    tags = extract_osm_tags()
    classify_feature(tags) → OsmFeatureType
    geometry = extract_coordinates()
    
    create OsmFeature {
        id, feature_type, name, geometry, tags
    }
}
```

## WebSocket Reconnection Logic

```rust
// Main loop with exponential backoff
let mut backoff_secs = 1
let mut failures = 0

loop {
    match connect_async(url).await {
        Ok(ws) => {
            backoff_secs = 1  // Reset on success
            failures = 0
            
            // Bidirectional messaging loop
            tokio::select! {
                Some(msg) = ws_receiver.next() => handle_incoming(msg)
                msg = outgoing_receiver.recv_async() => handle_outgoing(msg)
            }
        }
        Err(e) => {
            failures += 1
            
            // Log based on failure count
            if failures <= 3:
                error!("Connection failed (attempt {}): {}", failures, e)
            else if failures % 10 == 0:
                warn!("Still unreachable after {} attempts", failures)
            
            // Wait with exponential backoff
            sleep(Duration::from_secs(backoff_secs)).await
            backoff_secs = min(backoff_secs * 2, 60)  // Cap at 60s
        }
    }
}
```

## System Execution Order

```
Startup:
  1. setup_camera
  2. load_earth_textures
  3. (GlobeMesh setup)

Update (each frame):
  1. keyboard input handling
  2. check TerrainReadyEvent
  3. position_terrain_camera (if terrain ready)
  4. terrain_camera_control (if terrain loaded)
  5. toggle_globe_visibility (sphere ↔ cube)
  6. overlay_toggle_handler (N/C/O keys)
  7. check_texture_loading
  8. update_earth_material_with_textures

Async:
  - RealWorldLoaderPlugin polls async tasks
  - Communication loop runs on tokio thread
```

## Memory Profile

### Per Location Load
- Elevation tiles: 6-12 tiles × ~65KB = 400-780KB
- Imagery tiles: 12-24 tiles × ~150KB = 1.8-3.6MB
- NDVI texture: 512×512 RGBA = 1MB
- CDL texture: 512×512 RGBA = 1MB
- Terrain mesh: 128×128 vertices = ~100KB
- **Total per location: ~4-6MB**

### With LRU Cache
- Memory tiles (256 max): ~16-32MB
- Disk cache: Unlimited (persists across sessions)
- Typical session: Uses 20-50 tiles = 1-3MB active memory

## Performance Characteristics

### Tile Loading
- Network fetch: ~100-500ms per tile (depends on bandwidth)
- Disk read (cached): ~10-50ms per tile
- Decompression (PNG): ~5-20ms per tile
- **Total for location: 2-5 seconds first time, <100ms cached**

### Mesh Generation
- Heightmap interpolation: ~50-100ms
- Normal calculation: ~100-200ms
- NDVI computation: ~50-100ms
- **Total: 200-400ms**

### Rendering
- Frame time with overlays: 8-16ms @ 60fps
- Mesh vertices: 128×128 = 16,384 vertices
- Texture memory: ~3-5MB active

## Error Handling

### Network Errors
```rust
if let Err(e) = fetch_elevation_for_bounds().await {
    // Log error
    tracing::warn!("Failed to fetch elevation: {}", e);
    
    // Try cache
    if let Some(cached) = cache.get_tile() {
        return cached;
    }
    
    // Fail gracefully
    return Err(e);
}
```

### Async Task Management
```rust
// Spawn task with timeout
let result = tokio::time::timeout(
    Duration::from_secs(60),
    async_load_task
).await;

match result {
    Ok(Ok(data)) => spawn_terrain(data),
    Ok(Err(e)) => error!("Load failed: {}", e),
    Err(_) => error!("Load timeout"),
}
```

### Camera Bounds
```rust
// Prevent clipping
if camera.translation.y < MIN_HEIGHT {
    camera.translation.y = MIN_HEIGHT;
}
if camera.translation.y > MAX_HEIGHT {
    camera.translation.y = MAX_HEIGHT;
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_ndvi_color_range() {
        // NDVI values should produce appropriate colors
    }
    
    #[test]
    fn test_lru_eviction() {
        // Items evicted when exceeding limit
    }
}
```

### Integration Testing
- Load different terrains
- Toggle overlays
- Verify camera movement
- Check cache operations
- Monitor memory usage

---

## Performance Tips

1. **Reduce mesh resolution** if frame rate drops
2. **Decrease overlay opacity** for faster rendering
3. **Use LRU cache** - disk cache helps on reload
4. **Batch requests** - concurrent_requests limiter prevents overload
5. **Monitor backoff** - WebSocket reconnection is non-blocking

---

## Future Architectural Improvements

1. **Streaming tiles** - Load tiles as camera moves
2. **Level-of-detail** - Different mesh resolutions at different distances
3. **GPU compute** - NDVI computation on GPU
4. **Tiling protocol** - Custom format instead of PNG
5. **Multi-threaded** - More concurrent tile fetches
6. **Frustum culling** - Only render visible tiles

---

**Build System**: Cargo + Bevy
**Dependencies**: bevy 0.14.2, reqwest, tokio, serde, png, jpeg-decoder
**Target**: x86_64 macOS, Linux, Windows
**Performance Target**: 60 FPS @ 1080p with overlays
