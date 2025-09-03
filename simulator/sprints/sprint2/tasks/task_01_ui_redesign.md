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
- [x] Simplify `AppState` enum
- [x] Remove unused states (`Splash`, `LoadingSimulation`, etc.)
- [x] Add new world exploration states

### 2. Redesign Main Menu
**File:** `src/flight_ui/main_menu.rs`
- [x] Remove flight simulator theming
- [x] Create clean, minimal design
- [x] Add "Explore in 3D World" button
- [x] Add "Explore in 2D World" button
- [x] Remove other complex menu options

### 3. Update UI Plugin
**File:** `src/flight_ui/ui_plugin.rs`
- [x] Remove splash screen auto-transition
- [x] Start directly in MainMenu state
- [x] Update state transition handlers

### 4. Clean Up Demo
**File:** `src/flight_ui/demo.rs`
- [x] Remove splash screen demo
- [x] Simplify main menu demo
- [x] Remove flight simulator references

## Acceptance Criteria
- [x] Application starts with blank screen + main menu (no splash)
- [x] Main menu shows only two options: 3D/2D world exploration
- [x] Clean, minimal design without flight simulator theming
- [x] Proper state transitions when buttons are clicked
- [x] No compilation errors or warnings
- [x] **FIXED**: Resolved egui widget ID conflicts and duplicate UI systems

## Implementation Notes
- Keep the underlying egui and Bevy architecture
- Maintain the plugin system for modularity
- Focus on simplicity and clarity

## Estimated Time
**4 hours**

## Dependencies
- None (foundational task)

## Testing
- [x] Manual testing of startup flow
- [x] Verify state transitions work correctly
- [x] Check UI responsiveness and appearance
- [x] **RESOLVED**: Fixed overlapping text and widget ID conflicts

---
**Status:** ✅ **COMPLETED**  
**Assigned:** GitHub Copilot  
**Started:** July 27, 2025  
**Completed:** July 27, 2025

### **SOLUTION SUMMARY:**
**Fixed egui widget ID conflicts causing overlapping UI elements:**
1. **Removed duplicate `MainMenuPlugin`** registrations (main.rs + flight_ui)
2. **Deleted old `main_menu.rs`** with conflicting "Global Simulator" title
3. **Removed demo module** exports that could cause widget conflicts
4. **Clean state management** with single UI system

**Result:** Clean main menu without overlapping text, red boxes, or "Second use of widget ID" errors. Application runs smoothly with professional UI as designed.
