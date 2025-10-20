use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::ui::{AppState, UITheme, MenuState};

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
    mut next_menu_state: ResMut<NextState<MenuState>>,
    theme: Res<UITheme>,
    mut exit: EventWriter<bevy::app::AppExit>,
) {
    let ctx = contexts.ctx_mut();
    
    // Set custom theme
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = egui::Color32::from_rgba_premultiplied(26, 26, 38, 240);
    style.visuals.panel_fill = egui::Color32::from_rgba_premultiplied(20, 20, 30, 200);
    ctx.set_style(style);

    // Full screen central panel for main menu
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(10, 10, 20, 200)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                
                // Title
                ui.heading(egui::RichText::new("🚁 AGBOT DRONE VISUALIZER")
                    .size(48.0)
                    .color(egui::Color32::from_rgb(100, 150, 255)));
                
                ui.add_space(20.0);
                ui.label(egui::RichText::new("Advanced Agricultural Drone Simulation & Control Platform")
                    .size(16.0)
                    .color(egui::Color32::LIGHT_GRAY));
                
                ui.add_space(80.0);
                
                // Main menu buttons
                ui.vertical_centered(|ui| {
                    let button_size = egui::Vec2::new(300.0, 50.0);
                    
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("🌍 WORLD MAP").size(18.0)
                    )).clicked() {
                        next_app_state.set(AppState::WorldMap);
                    }
                    
                    ui.add_space(15.0);
                    
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("🚁 START SIMULATION").size(18.0)
                    )).clicked() {
                        next_app_state.set(AppState::LoadingSimulation);
                    }
                    
                    ui.add_space(15.0);
                    
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("⚙️ SETTINGS").size(18.0)
                    )).clicked() {
                        next_app_state.set(AppState::Settings);
                    }
                    
                    ui.add_space(15.0);
                    
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("📊 MISSION PLANNER").size(18.0)
                    )).clicked() {
                        next_menu_state.set(MenuState::MissionBriefing);
                    }
                    
                    ui.add_space(30.0);
                    
                    if ui.add_sized(button_size, egui::Button::new(
                        egui::RichText::new("❌ EXIT").size(18.0)
                    )).clicked() {
                        next_menu_state.set(MenuState::ConfirmExit);
                    }
                });
                
                ui.add_space(50.0);
                
                // Version info
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Version 1.0.0 | Build 2025.01")
                        .size(12.0)
                        .color(egui::Color32::DARK_GRAY));
                });
            });
        });
    
    // Handle modal dialogs
    handle_main_menu_modals(ctx, &mut next_app_state, &mut next_menu_state, &mut exit);
}

fn handle_main_menu_modals(
    ctx: &egui::Context,
    next_app_state: &mut ResMut<NextState<AppState>>,
    next_menu_state: &mut ResMut<NextState<MenuState>>,
    exit: &mut EventWriter<bevy::app::AppExit>,
) {
    // Exit confirmation dialog
    egui::Window::new("Confirm Exit")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.label("Are you sure you want to exit?");
                ui.add_space(20.0);
                
                ui.horizontal(|ui| {
                    if ui.button("Yes, Exit").clicked() {
                        exit.send(bevy::app::AppExit::Success);
                    }
                    
                    ui.add_space(20.0);
                    
                    if ui.button("Cancel").clicked() {
                        next_menu_state.set(MenuState::None);
                    }
                });
                ui.add_space(10.0);
            });
        });
}

/// Splash screen for initial loading
pub fn splash_screen_ui(
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
                
                // Animated logo or loading text
                let elapsed = time.elapsed_seconds();
                let dots = match ((elapsed * 2.0) as usize) % 4 {
                    0 => "",
                    1 => ".",
                    2 => "..",
                    _ => "...",
                };
                
                ui.heading(egui::RichText::new(format!("Loading AgBot{}", dots))
                    .size(32.0)
                    .color(egui::Color32::WHITE));
                
                ui.add_space(50.0);
                
                // Progress bar simulation
                let progress = (elapsed * 0.3).min(1.0);
                ui.add(egui::ProgressBar::new(progress).desired_width(300.0));
                
                // Auto-transition to main menu after loading
                if progress >= 1.0 {
                    next_state.set(AppState::MainMenu);
                }
            });
        });
}
