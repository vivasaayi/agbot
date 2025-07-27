# Task 01: UI Redesign & Clean Startup

## Objective
Redesign the main menu to show a clean interface with 2D/3D world exploration options, eliminating the complex Flight Simulator UI in favor of a simpler, more focused experience.

## Requirements
1. **Clean Startup**
   - Application starts with blank screen
   - No splash screen or loading animations
   - Direct to main menu

2. **Simplified Main Menu**
   - Title: "AgBot Visualizer"
   - Two main options: "Explore in 3D World" and "Explore in 2D World"
   - Clean, modern design
   - Centered layout

3. **State Management**
   - Remove complex flight simulator states
   - New states: `MainMenu`, `World3D`, `World2D`, `CitySearch`, `WorldLoading`, `Simulation`

## Technical Tasks

### 1. Update App States
**File:** `src/flight_ui/app_state.rs`
- [ ] Simplify `AppState` enum
- [ ] Remove unused states (`Splash`, `LoadingSimulation`, etc.)
- [ ] Add new world exploration states

### 2. Redesign Main Menu
**File:** `src/flight_ui/main_menu.rs`
- [ ] Remove flight simulator theming
- [ ] Create clean, minimal design
- [ ] Add "Explore in 3D World" button
- [ ] Add "Explore in 2D World" button
- [ ] Remove other complex menu options

### 3. Update UI Plugin
**File:** `src/flight_ui/ui_plugin.rs`
- [ ] Remove splash screen auto-transition
- [ ] Start directly in MainMenu state
- [ ] Update state transition handlers

### 4. Clean Up Demo
**File:** `src/flight_ui/demo.rs`
- [ ] Remove splash screen demo
- [ ] Simplify main menu demo
- [ ] Remove flight simulator references

## Acceptance Criteria
- [ ] Application starts with blank screen + main menu (no splash)
- [ ] Main menu shows only two options: 3D/2D world exploration
- [ ] Clean, minimal design without flight simulator theming
- [ ] Proper state transitions when buttons are clicked
- [ ] No compilation errors or warnings

## Implementation Notes
- Keep the underlying egui and Bevy architecture
- Maintain the plugin system for modularity
- Focus on simplicity and clarity

## Estimated Time
**4 hours**

## Dependencies
- None (foundational task)

## Testing
- [ ] Manual testing of startup flow
- [ ] Verify state transitions work correctly
- [ ] Check UI responsiveness and appearance

---
**Status:** 🔄 Not Started  
**Assigned:** -  
**Started:** -  
**Completed:** -
