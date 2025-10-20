# Task 06: Mac Keyboard Navigation & Controls

## Objective
Implement comprehensive Mac keyboard navigation and controls throughout the entire application, ensuring smooth and intuitive interaction for MacBook Pro users across all application states and simulation modes.

## Requirements
1. **Universal Navigation**
   - Consistent keyboard shortcuts across all states
   - Mac-specific key combinations (Cmd, Option, etc.)
   - Accessibility and ease of use

2. **Context-Sensitive Controls**
   - Different controls for each application state
   - Vehicle-specific controls in simulation mode
   - Camera controls for both 2D and 3D views

3. **Visual Feedback**
   - On-screen control hints
   - Keyboard shortcut tooltips
   - Help overlay system

4. **Configuration**
   - Customizable key bindings
   - Control sensitivity settings
   - Mac trackpad gesture support

## Technical Tasks

### 1. Keyboard Input System
**File:** `src/input/keyboard_system.rs` (new)
- [ ] Create centralized keyboard input handler
- [ ] Implement Mac-specific key mapping
- [ ] Add context-sensitive input routing
- [ ] Handle key combinations and modifiers

### 2. Navigation Controls
**File:** `src/input/navigation_controls.rs` (new)
- [ ] Menu navigation with arrow keys
- [ ] Tab navigation between UI elements
- [ ] Enter/Space for selection
- [ ] Escape for back navigation

### 3. Camera Controls
**File:** `src/input/camera_controls.rs` (new)
- [ ] WASD movement for 3D camera
- [ ] Mouse + WASD combination
- [ ] Zoom controls (+ - keys and scroll)
- [ ] Reset camera position (Space)

### 4. Search Controls
**File:** `src/input/search_controls.rs` (new)
- [ ] Cmd+F to focus search bar
- [ ] Tab/Enter for search suggestions
- [ ] Arrow keys for suggestion navigation
- [ ] Escape to clear search

### 5. Simulation Controls
**File:** `src/input/simulation_controls.rs` (new)
- [ ] Vehicle-specific movement controls
- [ ] Simulation state controls (play/pause/reset)
- [ ] Quick vehicle switching
- [ ] Camera mode switching

### 6. Help System
**File:** `src/input/help_system.rs` (new)
- [ ] Context-sensitive help overlay
- [ ] Keyboard shortcut reference
- [ ] Interactive tutorial system
- [ ] Toggle help with F1 or Cmd+?

### 7. Input Configuration
**File:** `src/input/input_config.rs` (new)
- [ ] Customizable key bindings
- [ ] Save/load control preferences
- [ ] Sensitivity settings
- [ ] Mac trackpad gesture configuration

### 8. Visual Feedback System
**File:** `src/input/visual_feedback.rs` (new)
- [ ] On-screen control hints
- [ ] Key press visualization
- [ ] Control tooltips
- [ ] Status indicators

## Control Schemes

### Main Menu
- **↑↓:** Navigate menu options
- **Enter/Space:** Select option
- **Cmd+Q:** Quit application

### 3D World Exploration
- **WASD:** Move camera
- **Mouse:** Look around
- **Scroll/+/-:** Zoom in/out
- **Space:** Reset camera
- **Cmd+F:** Focus search bar
- **Enter:** Search/Place marker
- **L:** Load selected location
- **Esc:** Return to main menu

### 2D World Exploration
- **WASD:** Pan map
- **+/-:** Zoom in/out
- **Mouse drag:** Pan map
- **Cmd+F:** Focus search bar
- **Enter:** Search/Place marker
- **L:** Load selected location
- **Esc:** Return to main menu

### World Loading
- **Esc:** Cancel loading (return to previous state)
- **Space:** Pause/Resume loading

### Simulation Mode
- **Tab:** Switch between vehicles
- **Space:** Pause/Resume simulation
- **R:** Reset simulation
- **1/2/3:** Quick select Drone/Tractor/Car
- **F1:** Toggle help overlay
- **T:** Toggle top-down/3D view
- **Esc:** Return to vehicle selection

### Vehicle-Specific Controls

#### Drone
- **WASD:** Move horizontally
- **Q/E:** Rotate left/right
- **Shift/Ctrl:** Altitude up/down
- **Mouse:** Camera look

#### Tractor
- **WASD:** Drive (forward/back/steer)
- **Shift:** Boost/Fast mode
- **Space:** Brake/Stop
- **I:** Implement up/down

#### Car
- **WASD:** Drive/Steer
- **Shift:** Accelerate
- **Space:** Brake
- **H:** Hazard lights

## Acceptance Criteria
- [ ] All keyboard controls work smoothly on Mac
- [ ] Context-sensitive controls change appropriately
- [ ] Help system shows current available controls
- [ ] Key combinations work with Mac modifiers
- [ ] Visual feedback shows active controls
- [ ] Controls are configurable and saveable
- [ ] Trackpad gestures work for zoom/pan
- [ ] No control conflicts between states

## Mac-Specific Features
- **Cmd key combinations:** Use Command instead of Ctrl
- **Trackpad gestures:** Two-finger scroll, pinch zoom
- **Function keys:** F1-F12 with proper Mac behavior
- **Option key:** Alternative actions
- **Delete key:** Mac-style delete behavior

## Implementation Notes
- Use Bevy's input system with Mac key mapping
- Consider using `winit` events for Mac-specific inputs
- Implement smooth input interpolation for better feel
- Add input buffering for complex key combinations

## Estimated Time
**10 hours**

## Dependencies
- All previous tasks (01-05) should be completed
- Mac testing environment required

## Testing
- [ ] Test all controls on MacBook Pro
- [ ] Verify key combinations work correctly
- [ ] Test trackpad gesture support
- [ ] Validate help system accuracy
- [ ] Test control customization
- [ ] Performance testing with complex input
- [ ] Accessibility testing for navigation

---
**Status:** 🔄 Not Started  
**Assigned:** -  
**Started:** -  
**Completed:** -
