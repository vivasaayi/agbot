use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::flight_ui::{AppState, UITheme, UIOverlayState};
use crate::globe_view::{GlobeCamera, GlobeState};

pub struct WorldMapPlugin;

impl Plugin for WorldMapPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnEnter(AppState::WorldMap), setup_world_map)
            .add_systems(Update, world_map_ui.run_if(in_state(AppState::WorldMap)))
            .add_systems(OnExit(AppState::WorldMap), cleanup_world_map);
    }
}

#[derive(Component)]
struct WorldMapEntity;

#[derive(Resource, Default)]
pub struct WorldMapState {
    pub search_text: String,
    pub selected_location: Option<String>,
    pub favorites: Vec<WorldMapLocation>,
    pub recent_locations: Vec<WorldMapLocation>,
    pub show_location_details: bool,
    pub location_filter: LocationFilter,
}

#[derive(Default, Clone, Debug)]
pub enum LocationFilter {
    #[default]
    All,
    Cities,
    Agricultural,
    Emergency,
    Favorites,
}

#[derive(Clone, Debug)]
pub struct WorldMapLocation {
    pub name: String,
    pub latitude: f32,
    pub longitude: f32,
    pub location_type: LocationType,
    pub description: String,
}

#[derive(Clone, Debug)]
pub enum LocationType {
    City,
    Agricultural,
    Emergency,
    Custom,
}

fn setup_world_map(
    mut commands: Commands,
    mut world_map_state: ResMut<WorldMapState>,
) {
    info!("Setting up world map");
    
    // Initialize with some preset locations
    world_map_state.favorites = vec![
        WorldMapLocation {
            name: "San Francisco".to_string(),
            latitude: 37.7749,
            longitude: -122.4194,
            location_type: LocationType::City,
            description: "Tech hub and urban agriculture testing site".to_string(),
        },
        WorldMapLocation {
            name: "Iowa Farmlands".to_string(),
            latitude: 42.0308,
            longitude: -93.5811,
            location_type: LocationType::Agricultural,
            description: "Large-scale corn and soybean agricultural region".to_string(),
        },
        WorldMapLocation {
            name: "Netherlands Greenhouse".to_string(),
            latitude: 52.0907,
            longitude: 5.1214,
            location_type: LocationType::Agricultural,
            description: "Advanced precision agriculture and greenhouse farming".to_string(),
        },
    ];
    
    commands.spawn((
        WorldMapEntity,
        Name::new("WorldMapInterface"),
    ));
}

fn cleanup_world_map(
    mut commands: Commands,
    world_map_entities: Query<Entity, With<WorldMapEntity>>,
) {
    for entity in world_map_entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn world_map_ui(
    mut contexts: EguiContexts,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut world_map_state: ResMut<WorldMapState>,
    mut globe_state: ResMut<GlobeState>,
    mut overlay_state: ResMut<UIOverlayState>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    theme: Res<UITheme>,
) {
    let ctx = contexts.ctx_mut();
    
    // Handle escape key to return to main menu
    if keyboard_input.just_pressed(KeyCode::Escape) {
        next_app_state.set(AppState::MainMenu);
        return;
    }
    
    // Top navigation bar
    egui::TopBottomPanel::top("world_map_top_panel")
        .resizable(false)
        .min_height(60.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                
                // Back button
                if ui.button("← Back to Menu").clicked() {
                    next_app_state.set(AppState::MainMenu);
                }
                
                ui.separator();
                
                ui.heading("🌍 World Map & Location Selection");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    
                    if ui.button("Start Simulation").clicked() {
                        if world_map_state.selected_location.is_some() {
                            next_app_state.set(AppState::LoadingSimulation);
                        } else {
                            overlay_state.add_simple_notification("Please select a location first".to_string());
                        }
                    }
                });
            });
        });
    
    // Left sidebar for location search and lists
    egui::SidePanel::left("location_panel")
        .resizable(true)
        .default_width(350.0)
        .width_range(300.0..=500.0)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(10.0);
                
                // Search section
                ui.group(|ui| {
                    ui.label("🔍 Search Locations");
                    ui.add_space(5.0);
                    
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut world_map_state.search_text);
                        if ui.button("Search").clicked() {
                            search_locations(&mut world_map_state);
                        }
                    });
                });
                
                ui.add_space(10.0);
                
                // Location filter
                ui.group(|ui| {
                    ui.label("📂 Filter");
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut world_map_state.location_filter, LocationFilter::All, "All");
                        ui.selectable_value(&mut world_map_state.location_filter, LocationFilter::Cities, "Cities");
                        ui.selectable_value(&mut world_map_state.location_filter, LocationFilter::Agricultural, "Agricultural");
                        ui.selectable_value(&mut world_map_state.location_filter, LocationFilter::Emergency, "Emergency");
                        ui.selectable_value(&mut world_map_state.location_filter, LocationFilter::Favorites, "Favorites");
                    });
                });
                
                ui.add_space(10.0);
                
                // Favorites list
                ui.group(|ui| {
                    ui.label("⭐ Favorite Locations");
                    ui.add_space(5.0);
                    
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for location in &world_map_state.favorites {
                                if location_matches_filter(location, &world_map_state.location_filter) {
                                    render_location_item(ui, location, &mut world_map_state, &mut globe_state);
                                }
                            }
                        });
                });
                
                ui.add_space(10.0);
                
                // Quick coordinates input
                ui.group(|ui| {
                    ui.label("📍 Quick Coordinates");
                    ui.add_space(5.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("Lat:");
                        ui.add(egui::DragValue::new(&mut globe_state.target_latitude)
                            .range(-90.0..=90.0)
                            .speed(0.1));
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Lon:");
                        ui.add(egui::DragValue::new(&mut globe_state.target_longitude)
                            .range(-180.0..=180.0)
                            .speed(0.1));
                    });
                    
                    if ui.button("Go to Coordinates").clicked() {
                        globe_state.goto_location = true;
                        overlay_state.add_simple_notification(format!(
                            "Moving to coordinates: {:.2}, {:.2}",
                            globe_state.target_latitude,
                            globe_state.target_longitude
                        ));
                    }
                });
            });
        });
    
    // Right sidebar for location details
    if world_map_state.show_location_details {
        egui::SidePanel::right("location_details")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(10.0);
                    
                    ui.group(|ui| {
                        ui.label("📋 Location Details");
                        ui.add_space(10.0);
                        
                        if let Some(selected) = &world_map_state.selected_location {
                            if let Some(location) = world_map_state.favorites.iter()
                                .find(|loc| loc.name == *selected) {
                                
                                ui.heading(&location.name);
                                ui.add_space(10.0);
                                
                                ui.label(format!("Type: {:?}", location.location_type));
                                ui.label(format!("Coordinates: {:.4}, {:.4}", 
                                    location.latitude, location.longitude));
                                ui.add_space(10.0);
                                
                                ui.label("Description:");
                                ui.label(&location.description);
                                ui.add_space(20.0);
                                
                                // Mission options
                                ui.group(|ui| {
                                    ui.label("🎯 Mission Options");
                                    ui.add_space(5.0);
                                    
                                    if ui.button("🌾 Agricultural Survey").clicked() {
                                        start_agricultural_mission(location, &mut next_app_state);
                                    }
                                    
                                    if ui.button("🔍 Area Reconnaissance").clicked() {
                                        start_reconnaissance_mission(location, &mut next_app_state);
                                    }
                                    
                                    if ui.button("🚨 Emergency Response").clicked() {
                                        start_emergency_mission(location, &mut next_app_state);
                                    }
                                });
                            }
                        } else {
                            ui.label("Select a location to view details");
                        }
                    });
                });
            });
    }
    
    // Central panel shows 3D globe (handled by existing globe_view system)
    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(ctx, |ui| {
            // Globe interaction hints
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.label("🖱️ Mouse: Rotate | 🖱️ Scroll: Zoom | 📍 Click: Select Location");
                });
            });
        });
}

fn search_locations(world_map_state: &mut WorldMapState) {
    info!("Searching for: {}", world_map_state.search_text);
    // Implement actual location search logic here
    // This would integrate with a geocoding service or local database
}

fn location_matches_filter(location: &WorldMapLocation, filter: &LocationFilter) -> bool {
    match filter {
        LocationFilter::All => true,
        LocationFilter::Cities => matches!(location.location_type, LocationType::City),
        LocationFilter::Agricultural => matches!(location.location_type, LocationType::Agricultural),
        LocationFilter::Emergency => matches!(location.location_type, LocationType::Emergency),
        LocationFilter::Favorites => true, // Since we're showing favorites list
    }
}

fn render_location_item(
    ui: &mut egui::Ui,
    location: &WorldMapLocation,
    world_map_state: &mut WorldMapState,
    globe_state: &mut GlobeState,
) {
    let is_selected = world_map_state.selected_location.as_ref() == Some(&location.name);
    
    ui.horizontal(|ui| {
        let response = ui.selectable_label(is_selected, &location.name);
        
        if response.clicked() {
            world_map_state.selected_location = Some(location.name.clone());
            world_map_state.show_location_details = true;
            
            // Update globe to show this location
            globe_state.target_latitude = location.latitude;
            globe_state.target_longitude = location.longitude;
            globe_state.goto_location = true;
        }
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let icon = match location.location_type {
                LocationType::City => "🏙️",
                LocationType::Agricultural => "🌾",
                LocationType::Emergency => "🚨",
                LocationType::Custom => "📍",
            };
            ui.label(icon);
        });
    });
    
    ui.separator();
}

fn start_agricultural_mission(location: &WorldMapLocation, next_app_state: &mut ResMut<NextState<AppState>>) {
    info!("Starting agricultural mission at: {}", location.name);
    next_app_state.set(AppState::LoadingSimulation);
}

fn start_reconnaissance_mission(location: &WorldMapLocation, next_app_state: &mut ResMut<NextState<AppState>>) {
    info!("Starting reconnaissance mission at: {}", location.name);
    next_app_state.set(AppState::LoadingSimulation);
}

fn start_emergency_mission(location: &WorldMapLocation, next_app_state: &mut ResMut<NextState<AppState>>) {
    info!("Starting emergency mission at: {}", location.name);
    next_app_state.set(AppState::LoadingSimulation);
}
