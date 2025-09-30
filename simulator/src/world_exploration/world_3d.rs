use crate::app_state::{AppMode, DataLoadingState};
use crate::flight_ui::AppState;
use crate::globe_view::{GlobeLocationSelected, GlobeState};
use crate::map_loader::{StartWorldLoad, WorldLoadedEvent};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

pub struct World3DPlugin;

impl Plugin for World3DPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::CitySearch), setup_city_search_with_globe)
            .add_systems(OnEnter(AppState::World3D), setup_3d_world)
            .add_systems(OnExit(AppState::World3D), cleanup_3d_world)
            .add_systems(
                Update,
                (handle_3d_ui, handle_3d_input).run_if(in_state(AppState::World3D)),
            )
            .add_systems(
                Update,
                (handle_city_search_ui, process_globe_selection)
                    .run_if(in_state(AppState::CitySearch)),
            )
            .add_systems(
                Update,
                (world_loading_ui, handle_world_loaded_transition)
                    .run_if(in_state(AppState::WorldLoading)),
            );
    }
}

#[derive(Component)]
struct World3DEntity;

#[derive(Resource, Default, Clone)]
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

fn setup_city_search_with_globe(mut commands: Commands, mut app_mode: ResMut<NextState<AppMode>>) {
    info!("Entering City Search mode - showing globe");

    // Switch to Globe mode to show the globe
    app_mode.set(AppMode::Globe);

    // Initialize World3D state for city search
    commands.insert_resource(World3DState::default());
}

fn setup_3d_world(
    mut commands: Commands,
    existing_state: Option<Res<World3DState>>,
    globe_state: Option<ResMut<GlobeState>>,
    mut app_mode: ResMut<NextState<AppMode>>,
) {
    info!("Entering 3D World Exploration mode");

    // Preserve previously selected target if available
    let mut state = existing_state.map(|s| s.clone()).unwrap_or_default();
    state.show_load_button = false;
    commands.insert_resource(state);

    // Configure globe for exploration mode if available
    if let Some(mut globe_state) = globe_state {
        globe_state.goto_location = false;
    }

    // Switch render pipeline to the simulation world so the OSM loader kicks in
    app_mode.set(AppMode::Simulation3D);

    // Add world 3D entity marker
    commands.spawn((World3DEntity, Name::new("World3D")));
}

fn cleanup_3d_world(mut commands: Commands, world_3d_entities: Query<Entity, With<World3DEntity>>) {
    info!("Cleaning up 3D World Exploration mode");

    // Remove all world 3D entities
    for entity in world_3d_entities.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Remove World3D state
    commands.remove_resource::<World3DState>();
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
                app_mode.set(AppMode::MainMenu);
            }

            ui.separator();

            // City search bar
            ui.label("🔍 Search for a city:");
            ui.add(
                egui::TextEdit::singleline(&mut world_state.search_query)
                    .hint_text("Type city name..."),
            );

            ui.separator();

            ui.label("�️ Click anywhere on the globe to load a detailed terrain view.");
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
            ui.label("⌨️ Cmd+F: Focus search | 🖱️ Click on globe to select cities");
        });
    });
}

fn handle_3d_ui(
    mut contexts: EguiContexts,
    mut world_state: ResMut<World3DState>,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut app_mode: ResMut<NextState<AppMode>>,
) {
    let ctx = contexts.ctx_mut();

    // Top panel with search and navigation
    egui::TopBottomPanel::top("world_3d_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Back to Menu button
            if ui.button("🠔 Back to Menu").clicked() {
                next_app_state.set(AppState::MainMenu);
                app_mode.set(AppMode::MainMenu);
            }

            ui.separator();

            // Current location info
            if let Some(ref location) = world_state.selected_location {
                ui.label("📍 Current Location:");
                ui.label(format!("{}, {}", location.name, location.country));
            } else {
                ui.label("🌍 3D World Exploration");
            }
        });
    });

    // Controls help panel
    egui::Window::new("Controls")
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

fn process_globe_selection(
    mut events: EventReader<GlobeLocationSelected>,
    mut world_state: Option<ResMut<World3DState>>,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut next_mode: ResMut<NextState<AppMode>>,
    mut loading_state: ResMut<DataLoadingState>,
    mut start_load: EventWriter<StartWorldLoad>,
) {
    let Some(mut state) = world_state else {
        // Drain events to keep reader in sync even if state unavailable
        for _ in events.read() {}
        return;
    };

    let mut selection_triggered = false;

    for event in events.read() {
        selection_triggered = true;
        state.selected_location = Some(WorldLocation {
            name: format!("Lat {:+.2}°, Lon {:+.2}°", event.latitude, event.longitude),
            latitude: event.latitude,
            longitude: event.longitude,
            country: "Custom selection".to_string(),
        });
    state.show_load_button = true;
        state.camera_target = None;

        loading_state.is_loading = true;
        loading_state.progress = 0.0;
        loading_state.status_message = format!(
            "Fetching terrain around {:+.2}°, {:+.2}°",
            event.latitude, event.longitude
        );

        // Kick off background world loading while we remain in Globe mode
        start_load.send(StartWorldLoad {
            latitude: event.latitude,
            longitude: event.longitude,
        });
    }

    if selection_triggered {
        // Show loading screen, but stay in Globe mode until data is ready for a smoother flow
        next_app_state.set(AppState::WorldLoading);
        // AppMode remains Globe; we'll switch to Simulation3D after we receive WorldLoadedEvent
    }
}

fn world_loading_ui(
    mut contexts: EguiContexts,
    world_state: Option<Res<World3DState>>,
    loading_state: Res<DataLoadingState>,
) {
    let ctx = contexts.ctx_mut();

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(120.0);
            ui.heading("Preparing detailed terrain...");
            ui.add_space(20.0);

            if let Some(state) = world_state {
                if state.show_load_button {
                    ui.label("🌍 Loading your selected destination...");
                }

                if let Some(location) = &state.selected_location {
                    ui.label(format!(
                        "📍 {:+.2}° lat, {:+.2}° lon",
                        location.latitude, location.longitude
                    ));
                    if !location.name.is_empty() {
                        ui.label(&location.name);
                    }
                }
            }

            ui.add_space(10.0);
            ui.add(egui::ProgressBar::new(loading_state.progress.clamp(0.0, 1.0))
                .desired_width(300.0)
                .text(loading_state.status_message.clone()));

            ui.add_space(30.0);
            ui.label("Tip: you can always return to the globe to pick another destination.");
        });
    });
}

fn handle_world_loaded_transition(
    mut events: EventReader<WorldLoadedEvent>,
    mut loading_state: ResMut<DataLoadingState>,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut world_state: Option<ResMut<World3DState>>,
    mut next_mode: ResMut<NextState<AppMode>>,
) {
    let mut world_ready = false;
    for _event in events.read() {
        world_ready = true;
    }

    if world_ready {
        loading_state.is_loading = false;
        loading_state.progress = 1.0;
        loading_state.status_message = "World ready".to_string();
        if let Some(mut state) = world_state {
            state.show_load_button = false;
        }
        // Now transition render pipeline to Simulation3D and show the 3D view
        next_mode.set(AppMode::Simulation3D);
        next_app_state.set(AppState::World3D);
    }
}
