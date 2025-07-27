// Simple integration file to demonstrate the Flight Simulator UI working
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::flight_ui::{AppState, UITheme, UIOverlayState};

/// Simple system to show the splash screen works
pub fn splash_screen_demo(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    let ctx = contexts.ctx_mut();
    
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::BLACK))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(200.0);
                
                let elapsed = time.elapsed_seconds();
                let dots = match ((elapsed * 2.0) as usize) % 4 {
                    0 => "",
                    1 => ".",
                    2 => "..",
                    _ => "...",
                };
                
                ui.heading(egui::RichText::new(format!("🚁 AgBot Loading{}", dots))
                    .size(32.0)
                    .color(egui::Color32::WHITE));
                
                ui.add_space(50.0);
                
                let progress = (elapsed * 0.3).min(1.0);
                ui.add(egui::ProgressBar::new(progress).desired_width(300.0));
                
                if progress >= 1.0 {
                    next_state.set(AppState::MainMenu);
                }
                
                ui.add_space(20.0);
                ui.label("Press any key to continue...");
            });
        });
}

/// System to demonstrate main menu
pub fn main_menu_demo(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<AppState>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let ctx = contexts.ctx_mut();
    
    // Handle any key to go to world map
    if keyboard_input.get_just_pressed().count() > 0 {
        next_state.set(AppState::WorldMap);
        return;
    }
    
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(10, 10, 20, 200)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                
                ui.heading(egui::RichText::new("🚁 AGBOT DRONE VISUALIZER")
                    .size(48.0)
                    .color(egui::Color32::from_rgb(100, 150, 255)));
                
                ui.add_space(40.0);
                
                let button_size = egui::Vec2::new(300.0, 50.0);
                
                if ui.add_sized(button_size, egui::Button::new(
                    egui::RichText::new("🌍 WORLD MAP").size(18.0)
                )).clicked() {
                    next_state.set(AppState::WorldMap);
                }
                
                ui.add_space(15.0);
                
                if ui.add_sized(button_size, egui::Button::new(
                    egui::RichText::new("🚁 START SIMULATION").size(18.0)
                )).clicked() {
                    next_state.set(AppState::Simulation);
                }
                
                ui.add_space(15.0);
                
                if ui.add_sized(button_size, egui::Button::new(
                    egui::RichText::new("⚙️ SETTINGS").size(18.0)
                )).clicked() {
                    next_state.set(AppState::Settings);
                }
                
                ui.add_space(30.0);
                ui.label("Press any key to navigate");
            });
        });
}

/// Demo plugin to show basic Flight Simulator UI
pub struct DemoFlightUIPlugin;

impl Plugin for DemoFlightUIPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_state::<AppState>()
            .init_resource::<UITheme>()
            .init_resource::<UIOverlayState>()
            .add_systems(Update, (
                splash_screen_demo.run_if(in_state(AppState::Splash)),
                main_menu_demo.run_if(in_state(AppState::MainMenu)),
            ));
    }
}
