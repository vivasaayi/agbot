// Simple integration file to demonstrate the new clean UI working
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::flight_ui::{AppState, UITheme, UIOverlayState};

/// System to demonstrate main menu
pub fn main_menu_demo(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<AppState>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let ctx = contexts.ctx_mut();
    
    // Handle any key to go to world exploration (for demo purposes)
    if keyboard_input.get_just_pressed().count() > 0 {
        next_state.set(AppState::World3D);
        return;
    }
    
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(5, 5, 15, 255)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                
                ui.heading(egui::RichText::new("AgBot Visualizer")
                    .size(48.0)
                    .color(egui::Color32::from_rgb(120, 160, 255)));
                
                ui.add_space(40.0);
                
                let button_size = egui::Vec2::new(300.0, 50.0);
                
                if ui.add_sized(button_size, egui::Button::new(
                    egui::RichText::new("🌍 Explore in 3D World").size(18.0)
                )).clicked() {
                    next_state.set(AppState::World3D);
                }
                
                ui.add_space(20.0);
                
                if ui.add_sized(button_size, egui::Button::new(
                    egui::RichText::new("🗺️ Explore in 2D World").size(18.0)
                )).clicked() {
                    next_state.set(AppState::World2D);
                }
                
                ui.add_space(40.0);
                ui.label("Press any key to start exploring...");
            });
        });
}

/// Demo plugin to show basic clean UI
pub struct DemoFlightUIPlugin;

impl Plugin for DemoFlightUIPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_state::<AppState>()
            .init_resource::<UITheme>()
            .init_resource::<UIOverlayState>()
            .add_systems(Update, 
                main_menu_demo.run_if(in_state(AppState::MainMenu))
            );
    }
}
