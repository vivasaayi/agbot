# Task 02: 3D World Exploration

## Objective
Implement 3D globe exploration with city search functionality, marker placement, and smooth camera transitions.

## Requirements
1. **3D Globe Display**
   - Clear screen when entering 3D mode
   - Show interactive 3D globe
   - Maintain existing globe rendering quality

2. **City Search Interface**
   - Search bar for city names
   - Auto-complete suggestions
   - Real-time search results

3. **Marker Placement**
   - Place visual markers on searched cities
   - Smooth camera rotation to show the city
   - Marker should be clearly visible

4. **Load Button**
   - "Load" button appears after marker is placed
   - Button click triggers world loading for that location

## Technical Tasks

### 1. Create World3D State Handler
**File:** `src/world_exploration/world_3d.rs` (new)
- [ ] Create World3DPlugin
- [ ] Set up 3D world state systems
- [ ] Integrate with existing globe_view.rs
- [ ] Handle cleanup when exiting state

### 2. Implement City Search UI
**File:** `src/city_search/search_interface.rs` (new)
- [ ] Create search bar component
- [ ] Implement auto-complete functionality
- [ ] Add search result dropdown
- [ ] Handle keyboard input and selection

### 3. Location Database
**File:** `src/city_search/location_database.rs` (new)
- [ ] Extend existing location database
- [ ] Add comprehensive city data
- [ ] Implement fuzzy search
- [ ] Add geographical coordinates

### 4. Marker System
**File:** `src/world_exploration/markers.rs` (new)
- [ ] Create 3D marker components
- [ ] Implement marker placement on globe
- [ ] Add marker visibility and styling
- [ ] Handle multiple markers

### 5. Camera Controls
**File:** `src/world_exploration/camera_3d.rs` (new)
- [ ] Smooth camera transitions to markers
- [ ] Zoom and rotation controls
- [ ] Mac keyboard navigation support
- [ ] Camera state persistence

### 6. Load Button Integration
**File:** `src/world_exploration/world_3d.rs`
- [ ] Show Load button when marker is placed
- [ ] Handle Load button click
- [ ] Transition to WorldLoading state

## Acceptance Criteria
- [ ] Clicking "Explore in 3D World" shows 3D globe
- [ ] Search bar allows typing city names
- [ ] Searching places marker on globe
- [ ] Globe rotates smoothly to show searched city
- [ ] Load button appears after marker placement
- [ ] Mac keyboard controls work for navigation
- [ ] Performance remains smooth (60+ FPS)

## Implementation Notes
- Reuse existing globe rendering from `globe_view.rs`
- Use egui for search interface overlay
- Implement smooth interpolation for camera movements
- Consider using Bevy's animation system for transitions

## Mac Keyboard Controls
- WASD: Camera movement
- Mouse: Look around
- Scroll: Zoom in/out
- Space: Reset camera
- Cmd+F: Focus search bar

## Estimated Time
**12 hours**

## Dependencies
- Task 01 (UI Redesign) must be completed
- Existing globe_view.rs system

## Testing
- [ ] Test city search with various names
- [ ] Verify marker placement accuracy
- [ ] Test camera movement smoothness
- [ ] Validate Mac keyboard controls
- [ ] Performance testing with multiple markers

---
**Status:** 🔄 Not Started  
**Assigned:** -  
**Started:** -  
**Completed:** -
