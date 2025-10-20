# UI System Integration Guide

## Overview
This guide explains how to integrate the new Flight Simulator-style UI system into your AgBot application.

## Architecture
The UI system consists of several modular components:

### State Management (`app_state.rs`)
- **AppState**: Main application states (Splash, MainMenu, WorldMap, Simulation, etc.)
- **MenuState**: Sub-menu states for modal dialogs and overlays
- **UITheme**: Centralized theme management with consistent colors and styling
- **UIOverlayState**: Notification and dialog system

### Core UI Modules
1. **Main Menu** (`main_menu.rs`): Professional splash screen and main navigation
2. **World Map** (`world_map.rs`): Interactive location selection with 3D globe integration
3. **Simulation HUD** (`simulation_hud.rs`): Real-time telemetry and mission control interface
4. **Settings** (`settings.rs`): Comprehensive configuration with tabbed interface
5. **Overlay System** (`overlay_system.rs`): Notifications, dialogs, loading screens, pause menu

## Integration Steps

### 1. Update main.rs
Replace your existing main.rs with the following integration:

```rust
use bevy::prelude::*;
use bevy_egui::EguiPlugin;

mod globe_view;
mod earth_textures;
mod ui;

use globe_view::GlobeViewPlugin;
use earth_textures::EarthTexturesPlugin;
use ui::UISystemPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "AgBot Drone Visualizer".into(),
                    resolution: (1920.0, 1080.0).into(),
                    ..default()
                }),
                ..default()
            }),
            
            // Core plugins
            EarthTexturesPlugin,
            GlobeViewPlugin,
            
            // New UI system
            UISystemPlugin,
        ))
        .run();
}
```

### 2. Update globe_view.rs Integration
Your globe_view.rs needs to be updated to work with the new state system:

```rust
// Add this to your GlobeViewPlugin
.add_systems(Update, (
    update_globe_rotation,
    handle_mouse_input,
    update_camera_position,
).run_if(in_state(AppState::WorldMap).or_else(in_state(AppState::Simulation))))
```

### 3. State Transitions
The UI system handles automatic state transitions:

- **Splash → MainMenu**: Auto-transition after 3 seconds
- **MainMenu → WorldMap**: User selects "World Map" 
- **WorldMap → Simulation**: User selects location and starts mission
- **Simulation ⟷ Paused**: ESC key toggles pause
- **Any State → MainMenu**: ESC key (context-dependent)

### 4. Integration with Existing Systems

#### Globe Integration
```rust
// In your globe update systems, check for state changes
fn update_globe_for_ui(
    globe_state: Res<GlobeState>,
    world_map_state: Res<WorldMapState>,
    // ... other resources
) {
    if globe_state.goto_location {
        // Move camera to target coordinates
        // This integrates with the WorldMap location selection
    }
}
```

#### Telemetry Integration
```rust
// Update SimulationHudState with real drone data
fn update_simulation_telemetry(
    mut hud_state: ResMut<SimulationHudState>,
    // Your drone data resources
) {
    hud_state.telemetry_data.altitude = real_drone.altitude;
    hud_state.telemetry_data.battery = real_drone.battery;
    // ... update other fields
}
```

## Key Features

### Professional Navigation
- **Splash Screen**: Animated loading with progress
- **Main Menu**: Clean, game-like interface with clear navigation
- **Tab System**: Organized settings with multiple categories
- **Modal Dialogs**: Confirmation dialogs and notifications

### Flight Simulator-Style HUD
- **Real-time Telemetry**: Altitude, speed, battery, GPS coordinates
- **Mission Control**: Waypoint management, tool selection
- **Visual Indicators**: Artificial horizon, compass, progress bars
- **Contextual Controls**: Dynamic based on mission state

### State-Driven Design
- **Modular States**: Easy to extend with new screens
- **Escape Key Handling**: Intelligent navigation based on context
- **Background Transitions**: Smooth state changes
- **Resource Management**: Automatic cleanup on state exit

### Responsive Design
- **Scalable UI**: Works across different screen resolutions
- **Configurable Opacity**: HUD elements can be made transparent
- **Toggle Panels**: Users can hide/show interface elements
- **Adaptive Layout**: Responsive to window resizing

## Customization

### Adding New States
1. Add to `AppState` enum in `app_state.rs`
2. Create new module in `ui/` directory
3. Implement `Plugin` trait with state-specific systems
4. Add to `UISystemPlugin` in `ui_plugin.rs`

### Styling Customization
Modify `UITheme` resource to change:
- Color schemes
- Font sizes
- Button styles
- Panel transparency

### Notification System
```rust
// Show notifications from anywhere in your code
overlay_state.add_notification("Mission completed!".to_string());
overlay_state.add_notification_with_type(
    "Low battery warning".to_string(),
    NotificationType::Warning
);
```

## Testing & Debug Features

In debug builds, press:
- **F1**: Jump to Main Menu
- **F2**: Jump to World Map  
- **F3**: Jump to Simulation
- **F4**: Jump to Settings

This allows rapid testing of different UI states during development.
