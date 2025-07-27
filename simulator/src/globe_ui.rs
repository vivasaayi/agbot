use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::app_state::{AppMode, SelectedRegion, UIState};

pub struct GlobeUIPlugin;

impl Plugin for GlobeUIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            globe_controls_ui,
            globe_coordinates_ui,
        ).run_if(in_state(AppMode::Globe)));
    }
}

fn globe_controls_ui(
    mut contexts: EguiContexts,
) {
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("🌍 Globe Controls")
        .default_pos([10.0, 60.0])
        .default_size([250.0, 200.0])
        .collapsible(true)
        .show(ctx, |ui| {
            ui.heading("Navigation");
            ui.label("🖱️ Left drag: Rotate globe");
            ui.label("🔄 Mouse wheel: Zoom in/out");
            ui.label("🖱️ Left click: Select location");
            
            ui.separator();
            
            ui.heading("Quick Locations");
            ui.horizontal(|ui| {
                if ui.button("🗽 New York").clicked() {
                    // TODO: Jump to New York
                    info!("Jump to New York requested");
                }
                if ui.button("🗼 Paris").clicked() {
                    // TODO: Jump to Paris
                    info!("Jump to Paris requested");
                }
            });
            
            ui.horizontal(|ui| {
                if ui.button("🗾 Tokyo").clicked() {
                    // TODO: Jump to Tokyo
                    info!("Jump to Tokyo requested");
                }
                if ui.button("🏛️ Rome").clicked() {
                    // TODO: Jump to Rome (current data)
                    info!("Jump to Rome requested");
                }
            });
            
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
        .default_pos([10.0, 300.0])
        .default_size([250.0, 150.0])
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label(format!("Latitude: {:.6}°", selected_region.center_lat));
            ui.label(format!("Longitude: {:.6}°", selected_region.center_lon));
            
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
