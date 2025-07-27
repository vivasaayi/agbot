# Task 03: 2D World Exploration

## Objective
Implement 2D map exploration with city search functionality, marker placement, and navigation controls similar to the 3D world but in a top-down map view.

## Requirements
1. **2D Map Display**
   - Clear screen when entering 2D mode
   - Show interactive 2D world map
   - High-quality map tiles (OpenStreetMap or similar)

2. **City Search Interface**
   - Identical search functionality to 3D world
   - Shared search components and database
   - Consistent user experience

3. **Marker Placement**
   - Place visual markers on searched cities
   - Smooth pan and zoom to show the city
   - Clear marker visibility on 2D map

4. **Load Button**
   - Same "Load" button functionality as 3D world
   - Triggers transition to world loading state

## Technical Tasks

### 1. Create World2D State Handler
**File:** `src/world_exploration/world_2d.rs` (new)
- [ ] Create World2DPlugin
- [ ] Set up 2D world state systems
- [ ] Implement map tile rendering
- [ ] Handle cleanup when exiting state

### 2. Map Tile System
**File:** `src/world_exploration/map_tiles.rs` (new)
- [ ] Implement tile-based map rendering
- [ ] Add map tile loading and caching
- [ ] Support zoom levels and tile coordinates
- [ ] Handle tile requests and memory management

### 3. 2D Marker System
**File:** `src/world_exploration/markers_2d.rs` (new)
- [ ] Create 2D marker components
- [ ] Implement marker placement on map
- [ ] Add marker clustering for nearby locations
- [ ] Handle marker interaction and selection

### 4. 2D Camera Controls
**File:** `src/world_exploration/camera_2d.rs` (new)
- [ ] Pan and zoom controls for 2D map
- [ ] Smooth transitions to searched locations
- [ ] Mac keyboard navigation support
- [ ] Viewport bounds and limits

### 5. Shared Search Integration
**File:** `src/city_search/search_interface.rs` (update)
- [ ] Make search interface work for both 2D and 3D
- [ ] Shared search state management
- [ ] Consistent search result handling

### 6. Map Provider Integration
**File:** `src/world_exploration/map_provider.rs` (new)
- [ ] OpenStreetMap tile API integration
- [ ] Tile URL generation
- [ ] Attribution and terms compliance
- [ ] Fallback tile sources

## Acceptance Criteria
- [ ] Clicking "Explore in 2D World" shows 2D map
- [ ] Map displays high-quality tiles
- [ ] Search functionality identical to 3D world
- [ ] Searching places marker and pans to location
- [ ] Load button appears after marker placement
- [ ] Mac keyboard controls work for map navigation
- [ ] Smooth zoom and pan operations
- [ ] Performance remains smooth even with many tiles

## Implementation Notes
- Use Bevy's 2D rendering capabilities
- Implement tile pyramid structure for different zoom levels
- Cache frequently accessed tiles to improve performance
- Use web requests for tile loading (consider rate limits)

## Mac Keyboard Controls
- WASD: Pan map
- +/-: Zoom in/out
- Mouse wheel: Zoom
- Mouse drag: Pan
- Cmd+F: Focus search bar
- Space: Reset view to world view

## Map Tile Sources
1. **Primary:** OpenStreetMap (free, no API key required)
2. **Fallback:** Satellite tiles if available
3. **Attribution:** Proper credit display

## Estimated Time
**10 hours**

## Dependencies
- Task 01 (UI Redesign) must be completed
- Task 02 components for shared search functionality

## Testing
- [ ] Test map tile loading at different zoom levels
- [ ] Verify search and marker placement accuracy
- [ ] Test pan and zoom smoothness
- [ ] Validate Mac keyboard controls
- [ ] Performance testing with extensive panning
- [ ] Test offline behavior when tiles fail to load

---
**Status:** 🔄 Not Started  
**Assigned:** -  
**Started:** -  
**Completed:** -
