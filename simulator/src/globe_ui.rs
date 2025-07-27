use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::app_state::{AppMode, SelectedRegion, UIState, GlobeSearchState};
use crate::location_database::LocationDatabase;

pub struct GlobeUIPlugin;

impl Plugin for GlobeUIPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobeSearchState::default())
            .insert_resource(LocationDatabase::new())
            .add_systems(Update, (
                globe_search_ui,
                globe_controls_ui,
                globe_coordinates_ui,
            ).run_if(in_state(AppMode::Globe)));
    }
}

// Helper function to start location animation
fn start_location_animation(
    search_state: &mut GlobeSearchState,
    location: &crate::location_database::Location,
    current_region: &SelectedRegion,
    current_time: f32,
) {
    search_state.start_lat = current_region.center_lat;
    search_state.start_lon = current_region.center_lon;
    search_state.target_lat = location.lat;
    search_state.target_lon = location.lon;
    search_state.target_zoom = location.zoom_level;
    search_state.animation_start_time = current_time;
    search_state.is_animating = true;
    search_state.show_suggestions = false;
}

fn globe_search_ui(
    mut contexts: EguiContexts,
    mut search_state: ResMut<GlobeSearchState>,
    selected_region: Res<SelectedRegion>,
    location_db: Res<LocationDatabase>,
    time: Res<Time>,
) {
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("🔍 Location Search")
        .default_pos([10.0, 60.0])
        .default_size([300.0, 300.0])
        .collapsible(true)
        .show(ctx, |ui| {
            ui.heading("Search Locations");
            
            // Search input box
            let response = ui.text_edit_singleline(&mut search_state.search_query);
            
            if response.changed() {
                search_state.show_suggestions = !search_state.search_query.is_empty();
            }
            
            // Search button
            ui.horizontal(|ui| {
                if ui.button("🔍 Search").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    if !search_state.search_query.is_empty() {
                        let results = location_db.search(&search_state.search_query);
                        if let Some(location) = results.first() {
                            start_location_animation(&mut search_state, location, &*selected_region, time.elapsed_seconds());
                            info!("Animating to: {} ({:.4}, {:.4})", location.name, location.lat, location.lon);
                        }
                    }
                }
                
                if ui.button("🧹 Clear").clicked() {
                    search_state.search_query.clear();
                    search_state.show_suggestions = false;
                }
            });
            
            // Suggestions dropdown
            if search_state.show_suggestions && !search_state.search_query.is_empty() {
                let suggestions = location_db.search(&search_state.search_query);
                if !suggestions.is_empty() {
                    ui.separator();
                    ui.label("Suggestions:");
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for location in suggestions.iter().take(6) {
                                if ui.selectable_label(false, &location.name).clicked() {
                                    search_state.search_query = location.name.clone();
                                    search_state.show_suggestions = false;
                                    start_location_animation(&mut search_state, location, &*selected_region, time.elapsed_seconds());
                                    info!("Selected from suggestions: {}", location.name);
                                }
                            }
                        });
                }
            }
            
            ui.separator();
            
            // Quick location buttons
            ui.heading("Quick Locations");
            ui.horizontal(|ui| {
                if ui.button("🗽 New York").clicked() {
                    if let Some(location) = location_db.get_location("new york, usa") {
                        info!("🎯 QUICK LOCATION: New York clicked - Expected: Lat {:.4}°, Lon {:.4}°", location.lat, location.lon);
                        start_location_animation(&mut search_state, location, &*selected_region, time.elapsed_seconds());
                    }
                }
                if ui.button("🗼 Paris").clicked() {
                    if let Some(location) = location_db.get_location("paris, france") {
                        info!("🎯 QUICK LOCATION: Paris clicked - Expected: Lat {:.4}°, Lon {:.4}°", location.lat, location.lon);
                        start_location_animation(&mut search_state, location, &*selected_region, time.elapsed_seconds());
                    }
                }
            });
            
            ui.horizontal(|ui| {
                if ui.button("🗾 Tokyo").clicked() {
                    if let Some(location) = location_db.get_location("tokyo, japan") {
                        info!("🎯 QUICK LOCATION: Tokyo clicked - Expected: Lat {:.4}°, Lon {:.4}°", location.lat, location.lon);
                        start_location_animation(&mut search_state, location, &*selected_region, time.elapsed_seconds());
                    }
                }
                if ui.button("🏛️ London").clicked() {
                    if let Some(location) = location_db.get_location("london, uk") {
                        info!("🎯 QUICK LOCATION: London clicked - Expected: Lat {:.4}°, Lon {:.4}°", location.lat, location.lon);
                        start_location_animation(&mut search_state, location, &*selected_region, time.elapsed_seconds());
                    }
                }
            });
            
            // Animation status
            if search_state.is_animating {
                ui.separator();
                let progress = ((time.elapsed_seconds() - search_state.animation_start_time) / search_state.animation_duration).clamp(0.0, 1.0);
                ui.label(format!("🌍 Flying to location... {:.0}%", progress * 100.0));
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            }
        });
}

fn globe_controls_ui(
    mut contexts: EguiContexts,
) {
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("🌍 Globe Controls")
        .default_pos([320.0, 60.0])
        .default_size([200.0, 150.0])
        .collapsible(true)
        .show(ctx, |ui| {
            ui.heading("Navigation");
            ui.label("🖱️ Left drag: Rotate globe");
            ui.label("🔄 Mouse wheel: Zoom in/out");
            ui.label("🖱️ Left click: Select location");
            
            ui.separator();
            
            ui.heading("Region Size");
            ui.label("📏 Simulation area:");
            ui.label("• Small: 0.005° (~500m)");
            ui.label("• Medium: 0.01° (~1km)");
            ui.label("• Large: 0.02° (~2km)");
        });
}

fn globe_coordinates_ui(
    mut contexts: EguiContexts,
    selected_region: Res<SelectedRegion>,
    ui_state: Res<UIState>,
) {
    if !ui_state.show_coordinates {
        return;
    }
    
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("📍 Selected Location")
        .default_pos([10.0, 380.0])
        .default_size([250.0, 240.0])
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label(format!("Latitude: {:.6}°", selected_region.center_lat));
            ui.label(format!("Longitude: {:.6}°", selected_region.center_lon));
            
            ui.separator();
            
            // Add coordinate conversion debugging
            ui.colored_label(egui::Color32::YELLOW, "🔍 DEBUG INFO:");
            let lat_rad = selected_region.center_lat.to_radians();
            let lon_rad = selected_region.center_lon.to_radians();
            
            // Show the 3D conversion (same as in globe_view.rs)
            let sphere_x = lat_rad.cos() * lon_rad.sin();
            let sphere_y = lat_rad.sin();
            let sphere_z = -lat_rad.cos() * lon_rad.cos();
            
            ui.label(format!("3D Position: ({:.3}, {:.3}, {:.3})", sphere_x, sphere_y, sphere_z));
            ui.label(format!("Lat Radians: {:.3}", lat_rad));
            ui.label(format!("Lon Radians: {:.3}", lon_rad));
            
            ui.separator();
            
            ui.label(format!("Region size: {:.4}° × {:.4}°", 
                selected_region.bounds_width_degrees, 
                selected_region.bounds_height_degrees));
            
            let area_km = selected_region.bounds_width_degrees * selected_region.bounds_height_degrees * 111.32 * 111.32;
            ui.label(format!("Area: ~{:.1} km²", area_km));
            
            ui.separator();
            
            // Reverse geocoding placeholder
            ui.label("📍 Location: Unknown");
            ui.small("(Reverse geocoding not implemented)");
            
            ui.separator();
            
            if ui.button("🗺️ View in 2D Map").clicked() {
                info!("Switching to 2D map view");
                // TODO: Switch to 2D map view
            }
            
            if ui.button("🏙️ Enter 3D Simulation").clicked() {
                info!("Switching to 3D simulation");
                // TODO: Switch to 3D simulation
            }
        });
}
