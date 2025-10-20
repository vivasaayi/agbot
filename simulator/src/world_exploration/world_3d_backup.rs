use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::flight_ui::AppState;
use crate::globe_view::GlobeState;
use crate::app_state::AppMode;

pub struct World3DPlugin;

impl Plugin for World3DPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnEnter(AppState::CitySearch), setup_city_search_with_globe)
            .add_systems(OnEnter(AppState::World3D), setup_3d_world)
            .add_systems(OnExit(AppState::World3D), cleanup_3d_world)
            .add_systems(Update, (
                handle_3d_ui,
                handle_3d_input,
            ).run_if(in_state(AppState::World3D)))
            .add_systems(Update, (
                handle_city_search_ui,
            ).run_if(in_state(AppState::CitySearch)));
    }
}

#[derive(Component)]
struct World3DEntity;

#[derive(Resource, Default)]
pub struct World3DState {
    pub search_query: String,
    pub selected_location: Option<WorldLocation>,
    pub show_load_button: bool,
    pub camera_target: Option<Vec3>,
}

#[derive(Debug, Clone)]
pub struct WorldLocation {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: String,
}

fn setup_3d_world(
    mut commands: Commands,
    globe_state: Option<ResMut<GlobeState>>,
) {
    info!("Entering 3D World Exploration mode");
    
    // Initialize World3D state
    commands.insert_resource(World3DState::default());
    
    // Configure globe for exploration mode if available
    if let Some(mut globe_state) = globe_state {
        globe_state.goto_location = false;
    }
    
    // Add world 3D entity marker
    commands.spawn((
        World3DEntity,
        Name::new("World3D"),
    ));
}

fn setup_city_search_with_globe(
    mut commands: Commands,
    mut app_mode: ResMut<NextState<AppMode>>,
) {
    info!("Entering City Search mode - showing globe");
    
    // Switch to Globe mode to show the globe
    app_mode.set(AppMode::Globe);
    
    // Initialize World3D state for city search
    commands.insert_resource(World3DState::default());
}

fn cleanup_3d_world(
    mut commands: Commands,
    world_3d_entities: Query<Entity, With<World3DEntity>>,
) {
    info!("Exiting 3D World Exploration mode");
    
    // Remove World3D resources
    commands.remove_resource::<World3DState>();
    
    // Clean up entities
    for entity in world_3d_entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn world_3d_ui(
    mut contexts: EguiContexts,
    mut world_state: ResMut<World3DState>,
    mut next_app_state: ResMut<NextState<AppState>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let ctx = contexts.ctx_mut();
    
    // Handle Cmd+F for search focus (Mac)
    if keyboard_input.just_pressed(KeyCode::KeyF) && 
       (keyboard_input.pressed(KeyCode::SuperLeft) || keyboard_input.pressed(KeyCode::SuperRight)) {
        // Focus search bar - egui will handle this automatically when we create it
    }
    
    // Top panel for search interface
    egui::TopBottomPanel::top("world_3d_search")
        .min_height(60.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                
                // Back button
                if ui.button("← Back to Menu").clicked() {
                    next_app_state.set(AppState::MainMenu);
                }
                
                ui.add_space(20.0);
                
                // Search bar
                ui.label("🔍 Search for a city:");
                ui.add_space(10.0);
                
                let search_response = ui.add_sized(
                    [300.0, 25.0],
                    egui::TextEdit::singleline(&mut world_state.search_query)
                        .hint_text("Type city name...")
                );
                
                // Simple search trigger (we'll expand this)
                if search_response.changed() && !world_state.search_query.is_empty() {
                    // For now, create a dummy location
                    if world_state.search_query.to_lowercase().contains("paris") {
                        world_state.selected_location = Some(WorldLocation {
                            name: "Paris".to_string(),
                            latitude: 48.8566,
                            longitude: 2.3522,
                            country: "France".to_string(),
                        });
                        world_state.show_load_button = true;
                    }
                }
                
                ui.add_space(20.0);
                
                // Load button (appears when location is selected)
                if world_state.show_load_button {
                    if ui.button("🚀 Load This Location").clicked() {
                        info!("Loading world for location: {:?}", world_state.selected_location);
                        next_app_state.set(AppState::WorldLoading);
                    }
                }
            });
        });
    
    // Status panel
    if let Some(ref location) = world_state.selected_location {
        egui::Window::new("Selected Location")
            .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-10.0, 70.0))
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("📍 {}", location.name));
                ui.label(format!("🌍 {}", location.country));
                ui.label(format!("📐 {:.4}°N, {:.4}°E", location.latitude, location.longitude));
            });
    }
    
    // Help panel
    egui::Window::new("Navigation Help")
        .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2::new(10.0, -10.0))
        .resizable(false)
        .collapsible(true)
        .default_open(false)
        .show(ctx, |ui| {
            ui.label("🖱️ Mouse: Look around");
            ui.label("🖱️ Scroll: Zoom in/out");
            ui.label("⌨️ WASD: Move camera");
            ui.label("⌨️ Space: Reset view");
            ui.label("⌨️ Cmd+F: Focus search");
        });
}

fn handle_3d_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    _globe_state: Option<ResMut<GlobeState>>,
) {
    // Handle space key for camera reset
    if keyboard_input.just_pressed(KeyCode::Space) {
        info!("Resetting 3D camera view");
}

fn handle_city_search_ui(
    mut contexts: EguiContexts,
    mut world_state: ResMut<World3DState>,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut app_mode: ResMut<NextState<AppMode>>,
) {
    let ctx = contexts.ctx_mut();
    
    // Top panel with city search and load button
    egui::TopBottomPanel::top("city_search_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Back to Menu button
            if ui.button("🠔 Back to Menu").clicked() {
                next_app_state.set(AppState::MainMenu);
                app_mode.set(AppMode::Menu);
            }
            
            ui.separator();
            
            // City search bar
            ui.label("🔍 Search for a city:");
            ui.add(egui::TextEdit::singleline(&mut world_state.search_query)
                .hint_text("Type city name..."));
            
            ui.separator();
            
            // Load Location button (only show if city selected)
            if world_state.show_load_button {
                if ui.button("📍 Load Location").clicked() {
                    info!("Loading 3D world for selected location");
                    app_mode.set(AppMode::Menu); // Exit globe mode
                    next_app_state.set(AppState::World3D); // Enter 3D world
                }
            }
        });
        
        // Show selected location info
        if let Some(ref location) = world_state.selected_location {
            ui.horizontal(|ui| {
                ui.label("📍 Selected:");
                ui.label(format!("{}, {}", location.name, location.country));
            });
        }
        
        // Keyboard shortcuts info
        ui.horizontal(|ui| {
            ui.label("⌨️ Cmd+F: Focus search");
        });
    });
}

fn handle_3d_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    _globe_state: Option<ResMut<GlobeState>>,
) {
    // Handle space key for camera reset
    if keyboard_input.just_pressed(KeyCode::Space) {
        info!("Resetting 3D camera view");
        // Reset globe camera to default position
        // We'll implement this when we integrate with globe_view.rs
    }
    
    // Handle WASD for camera movement
    let mut movement = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) {
        movement.z -= 0.1;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        movement.z += 0.1;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        movement.x -= 0.1;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        movement.x += 0.1;
    }
    
    // Apply movement to globe state
    if movement != Vec3::ZERO {
        // We'll implement camera movement when we integrate with globe_view.rs
        info!("Camera movement: {:?}", movement);
    }
}
