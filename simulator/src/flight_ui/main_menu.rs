use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::flight_ui::{AppState, UITheme};

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(OnEnter(AppState::MainMenu), setup_main_menu)
            .add_systems(Update, main_menu_ui.run_if(in_state(AppState::MainMenu)))
            .add_systems(OnExit(AppState::MainMenu), cleanup_main_menu);
    }
}

#[derive(Component)]
struct MainMenuEntity;

fn setup_main_menu(mut commands: Commands) {
    info!("Setting up main menu");
    
    // Add main menu background entity if needed
    commands.spawn((
        MainMenuEntity,
        Name::new("MainMenuBackground"),
    ));
}

fn cleanup_main_menu(
    mut commands: Commands,
    menu_entities: Query<Entity, With<MainMenuEntity>>,
) {
    for entity in menu_entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn main_menu_ui(
    mut contexts: EguiContexts,
    mut next_app_state: ResMut<NextState<AppState>>,
    theme: Res<UITheme>,
    mut exit: EventWriter<bevy::app::AppExit>,
) {
    let ctx = contexts.ctx_mut();
    
    // Set custom theme for clean, modern look
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = egui::Color32::from_rgba_premultiplied(12, 12, 25, 250);
    style.visuals.panel_fill = egui::Color32::from_rgba_premultiplied(8, 8, 20, 200);
    ctx.set_style(style);

    // Full screen central panel for main menu
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(5, 5, 15, 255)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                
                // Clean, modern title
                ui.heading(egui::RichText::new("AgBot Visualizer")
                    .size(56.0)
                    .color(egui::Color32::from_rgb(120, 160, 255)));
                
                ui.add_space(20.0);
                ui.label(egui::RichText::new("Advanced Agricultural World Simulation Platform")
                    .size(18.0)
                    .color(egui::Color32::LIGHT_GRAY));
                
                ui.add_space(120.0);
                
                // Main exploration options
                ui.vertical_centered(|ui| {
                    let button_size = egui::Vec2::new(400.0, 60.0);
                    
                    // 3D World Exploration Button
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("🌍 Explore in 3D World")
                            .size(22.0)
                            .color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(50, 100, 200))).clicked() {
                        next_app_state.set(AppState::CitySearch);
                    }
                    
                    ui.add_space(20.0);
                    
                    // 2D World Exploration Button  
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("🗺️ Explore in 2D World")
                            .size(22.0)
                            .color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(40, 140, 80))).clicked() {
                        next_app_state.set(AppState::World2D);
                    }
                    
                    ui.add_space(60.0);
                    
                    // Quit option
                    if ui.add_sized(egui::Vec2::new(200.0, 40.0), egui::Button::new(
                        egui::RichText::new("Quit")
                            .size(16.0)
                            .color(egui::Color32::LIGHT_GRAY)
                    ).fill(egui::Color32::from_rgb(60, 60, 60))).clicked() {
                        exit.send(bevy::app::AppExit::Success);
                    }
                });
                
                ui.add_space(50.0);
                
                // Version info
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Version 2.0.0 | Sprint 2 Development")
                        .size(12.0)
                        .color(egui::Color32::DARK_GRAY));
                });
            });
        });
}
