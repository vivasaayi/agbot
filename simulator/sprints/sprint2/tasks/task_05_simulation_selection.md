# Task 05: Simulation Selection & Multi-Vehicle Framework

## Objective
Create a simulation selection interface that allows users to choose between Drone, Tractor, and Car simulations, and implement the foundational architecture for multi-vehicle simulation support.

## Requirements
1. **Simulation Selection UI**
   - "Choose Simulation" button in loaded world
   - Modal/panel showing Drone, Tractor, Car options
   - Clear vehicle descriptions and capabilities

2. **Multi-Vehicle Architecture**
   - Modular simulation framework
   - Vehicle-specific behaviors and physics
   - Shared simulation state management

3. **Vehicle Implementations**
   - **Drone:** Aerial vehicle with 6DOF movement
   - **Tractor:** Ground vehicle for agricultural operations
   - **Car:** Road vehicle following road networks

4. **Simulation State**
   - Clean transition from world loading to simulation
   - Vehicle spawning and initialization
   - Simulation controls (play, pause, reset)

## Technical Tasks

### 1. Simulation Selection UI
**File:** `src/simulation_selection/selection_ui.rs` (new)
- [ ] Create "Choose Simulation" button
- [ ] Implement vehicle selection modal
- [ ] Add vehicle descriptions and previews
- [ ] Handle selection confirmation

### 2. Simulation Framework
**File:** `src/simulation/simulation_framework.rs` (new)
- [ ] Define simulation traits and interfaces
- [ ] Implement simulation state management
- [ ] Create vehicle spawning system
- [ ] Add simulation lifecycle management

### 3. Vehicle Base System
**File:** `src/simulation/vehicles/mod.rs` (new)
- [ ] Define Vehicle trait
- [ ] Implement common vehicle components
- [ ] Add physics and movement base systems
- [ ] Create vehicle state tracking

### 4. Drone Simulation
**File:** `src/simulation/vehicles/drone.rs` (new)
- [ ] Implement drone-specific physics
- [ ] Add 6DOF movement capabilities
- [ ] Create drone controller interface
- [ ] Add altitude and flight path systems

### 5. Tractor Simulation
**File:** `src/simulation/vehicles/tractor.rs` (new)
- [ ] Implement ground vehicle physics
- [ ] Add agricultural implement systems
- [ ] Create field operation capabilities
- [ ] Add terrain interaction

### 6. Car Simulation
**File:** `src/simulation/vehicles/car.rs` (new)
- [ ] Implement road-following vehicle
- [ ] Add pathfinding for road networks
- [ ] Create traffic behavior simulation
- [ ] Add collision avoidance

### 7. Simulation State Handler
**File:** `src/simulation/simulation_state.rs` (new)
- [ ] Handle Simulation app state
- [ ] Manage active simulations
- [ ] Implement simulation controls
- [ ] Add performance monitoring

### 8. Vehicle Spawning System
**File:** `src/simulation/spawning.rs` (new)
- [ ] Spawn vehicles based on selection
- [ ] Position vehicles appropriately
- [ ] Initialize vehicle-specific systems
- [ ] Handle multiple vehicle instances

## Acceptance Criteria
- [ ] "Choose Simulation" button appears in loaded world
- [ ] Clicking button shows vehicle selection options
- [ ] Each vehicle type has clear description
- [ ] Selecting vehicle spawns it in the world
- [ ] Vehicle behaves according to its type
- [ ] Multiple vehicles can be spawned
- [ ] Simulation controls work (play/pause/reset)
- [ ] Performance remains acceptable with active vehicles

## Vehicle Specifications

### Drone
- **Type:** Quadcopter/Multirotor
- **Movement:** 6DOF (pitch, yaw, roll, x, y, z)
- **Capabilities:** Aerial photography, field scanning
- **Spawn Location:** Above the loaded area
- **Controls:** Mac keyboard + mouse

### Tractor
- **Type:** Agricultural vehicle
- **Movement:** Ground-based, wheel physics
- **Capabilities:** Plowing, seeding, harvesting
- **Spawn Location:** On roads or field edges
- **Controls:** WASD movement

### Car
- **Type:** Road vehicle
- **Movement:** Road-following with pathfinding
- **Capabilities:** Transportation, delivery
- **Spawn Location:** On road network
- **Controls:** Autonomous or manual steering

## Implementation Notes
- Use component-based architecture for vehicles
- Implement shared physics systems where applicable
- Consider using Bevy's scene system for vehicle prefabs
- Add visual feedback for vehicle selection

## Mac Keyboard Controls (Simulation)
- **Tab:** Switch between vehicles
- **Space:** Pause/Resume simulation
- **R:** Reset simulation
- **1,2,3:** Quick select vehicle types
- **Esc:** Return to vehicle selection

## Estimated Time
**14 hours**

## Dependencies
- Task 04 (OSM World Loading) must be completed
- Loaded world with buildings and roads

## Testing
- [ ] Test vehicle selection UI
- [ ] Verify each vehicle type spawns correctly
- [ ] Test vehicle-specific behaviors
- [ ] Validate simulation state management
- [ ] Performance testing with multiple vehicles
- [ ] Test simulation controls
- [ ] Verify Mac keyboard controls work

---
**Status:** 🔄 Not Started  
**Assigned:** -  
**Started:** -  
**Completed:** -
