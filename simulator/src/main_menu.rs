use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::app_state::{AppMode, SelectedRegion, UIState, DataLoadingState};

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            main_menu_system.run_if(in_state(AppMode::MainMenu)),
            navigation_bar_system.run_if(not(in_state(AppMode::MainMenu))),
            debug_info_system,
        ));
    }
}

fn main_menu_system(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<AppMode>>,
    selected_region: Res<SelectedRegion>,
) {
    let ctx = contexts.ctx_mut();
    
    // Main menu window
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 180)))
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add_space(100.0);
                
                // Title
                ui.heading(egui::RichText::new("🌍 Global Simulator").size(48.0).color(egui::Color32::WHITE));
                ui.add_space(20.0);
                ui.label(egui::RichText::new("Explore the world from space to street level").size(18.0).color(egui::Color32::LIGHT_GRAY));
                
                ui.add_space(50.0);
                
                // Current region info
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Current Region").size(16.0));
                    ui.label(format!("📍 {:.4}°N, {:.4}°E", selected_region.center_lat, selected_region.center_lon));
                    ui.label(format!("📏 {:.4}° × {:.4}°", selected_region.bounds_width_degrees, selected_region.bounds_height_degrees));
                });
                
                ui.add_space(30.0);
                
                // Mode selection buttons with descriptions
                ui.vertical_centered(|ui| {
                    if ui.add_sized([300.0, 60.0], egui::Button::new("🌍 Globe View\n🖱️ Click anywhere on Earth to select")).clicked() {
                        next_state.set(AppMode::Globe);
                    }
                    
                    if ui.add_sized([300.0, 60.0], egui::Button::new("🗺️ 2D Map\n📋 Top-down view of selected region")).clicked() {
                        next_state.set(AppMode::Map2D);
                    }
                    
                    if ui.add_sized([300.0, 60.0], egui::Button::new("🏙️ 3D Simulation\n🚁 Immersive street-level exploration")).clicked() {
                        next_state.set(AppMode::Simulation3D);
                    }
                });
                
                ui.add_space(20.0);
                
                // Quick actions
                ui.horizontal(|ui| {
                    if ui.button("🔍 Search Location").clicked() {
                        // TODO: Implement location search
                        info!("Location search clicked");
                    }
                    
                    if ui.button("📁 Load GeoJSON").clicked() {
                        // TODO: Implement file picker
                        info!("Load GeoJSON clicked");
                    }
                    
                    if ui.button("⚙️ Settings").clicked() {
                        // TODO: Implement settings dialog
                        info!("Settings clicked");
                    }
                });
            });
        });
}

fn navigation_bar_system(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<AppMode>>,
    current_state: Res<State<AppMode>>,
    selected_region: Res<SelectedRegion>,
    loading_state: Res<DataLoadingState>,
) {
    let ctx = contexts.ctx_mut();
    
    egui::TopBottomPanel::top("nav_bar")
        .resizable(false)
        .min_height(40.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Mode indicator and navigation
                ui.label("Mode:");
                
                let current_mode = current_state.get();
                let mode_text = match current_mode {
                    AppMode::MainMenu => "🏠 Menu",
                    AppMode::Globe => "🌍 Globe",
                    AppMode::Map2D => "🗺️ 2D Map",
                    AppMode::Simulation3D => "🏙️ 3D Simulation",
                };
                
                ui.label(egui::RichText::new(mode_text).strong());
                
                ui.separator();
                
                // Quick navigation buttons
                if ui.small_button("🏠").on_hover_text("Main Menu").clicked() {
                    next_state.set(AppMode::MainMenu);
                }
                
                if ui.small_button("🌍").on_hover_text("Globe View").clicked() {
                    next_state.set(AppMode::Globe);
                }
                
                if ui.small_button("🗺️").on_hover_text("2D Map").clicked() {
                    next_state.set(AppMode::Map2D);
                }
                
                if ui.small_button("🏙️").on_hover_text("3D Simulation").clicked() {
                    next_state.set(AppMode::Simulation3D);
                }
                
                ui.separator();
                
                // Current location
                ui.label(format!("📍 {:.4}°, {:.4}°", selected_region.center_lat, selected_region.center_lon));
                
                // Loading indicator
                if loading_state.is_loading {
                    ui.separator();
                    ui.spinner();
                    ui.label(&loading_state.status_message);
                    
                    // Progress bar
                    let progress_bar = egui::ProgressBar::new(loading_state.progress)
                        .text(format!("{:.0}%", loading_state.progress * 100.0));
                    ui.add_sized([100.0, 10.0], progress_bar);
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("❌").on_hover_text("Exit").clicked() {
                        std::process::exit(0);
                    }
                });
            });
        });
}

fn debug_info_system(
    mut contexts: EguiContexts,
    ui_state: Res<UIState>,
    current_state: Res<State<AppMode>>,
    time: Res<Time>,
) {
    if !ui_state.show_debug_info {
        return;
    }
    
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("Debug Info")
        .default_pos([10.0, 60.0])
        .default_size([200.0, 150.0])
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label(format!("Mode: {:?}", current_state.get()));
            ui.label(format!("FPS: {:.1}", 1.0 / time.delta_seconds()));
            ui.label(format!("Frame: {}", time.elapsed_seconds() as u64));
            
            ui.separator();
            
            ui.label("Controls:");
            ui.label("• Tab: Toggle debug");
            ui.label("• F11: Fullscreen");
            ui.label("• Esc: Main menu");
        });
}
