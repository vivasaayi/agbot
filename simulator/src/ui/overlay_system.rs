use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::ui::{AppState, UIOverlayState};

pub struct OverlaySystemPlugin;

impl Plugin for OverlaySystemPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, (
                notification_system,
                dialog_system,
                loading_overlay_system,
                pause_overlay_system,
            ));
    }
}

/// Renders notification popups
fn notification_system(
    mut contexts: EguiContexts,
    mut overlay_state: ResMut<UIOverlayState>,
    time: Res<Time>,
) {
    let ctx = contexts.ctx_mut();
    
    // Update notifications and remove expired ones
    overlay_state.notifications.retain_mut(|notification| {
        notification.remaining_time -= time.delta_seconds();
        notification.remaining_time > 0.0
    });
    
    // Display active notifications
    for (index, notification) in overlay_state.notifications.iter().enumerate() {
        let window_id = egui::Id::new("notification").with(index);
        
        egui::Window::new("")
            .id(window_id)
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-20.0, 20.0 + (index as f32 * 80.0)))
            .frame(egui::Frame::window(&ctx.style()).fill(egui::Color32::from_rgba_premultiplied(40, 40, 60, 240)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Notification icon based on type
                    let (icon, color) = match notification.notification_type {
                        crate::ui::NotificationType::Info => ("ℹ️", egui::Color32::LIGHT_BLUE),
                        crate::ui::NotificationType::Success => ("✅", egui::Color32::GREEN),
                        crate::ui::NotificationType::Warning => ("⚠️", egui::Color32::YELLOW),
                        crate::ui::NotificationType::Error => ("❌", egui::Color32::RED),
                    };
                    
                    ui.colored_label(color, icon);
                    ui.label(&notification.message);
                });
                
                // Progress bar showing remaining time
                let progress = notification.remaining_time / notification.duration;
                ui.add(egui::ProgressBar::new(progress)
                    .desired_width(250.0)
                    .desired_height(4.0));
            });
    }
}

/// Renders modal dialogs
fn dialog_system(
    mut contexts: EguiContexts,
    mut overlay_state: ResMut<UIOverlayState>,
    mut next_app_state: ResMut<NextState<AppState>>,
) {
    let ctx = contexts.ctx_mut();
    
    if let Some(dialog) = &overlay_state.active_dialog {
        let mut should_close = false;
        
        egui::Window::new(&dialog.title)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .resizable(false)
            .collapsible(false)
            .frame(egui::Frame::window(&ctx.style()).fill(egui::Color32::from_rgba_premultiplied(30, 30, 45, 250)))
            .show(ctx, |ui| {
                ui.add_space(10.0);
                
                // Dialog content
                ui.label(&dialog.message);
                
                ui.add_space(20.0);
                
                // Dialog buttons
                ui.horizontal(|ui| {
                    match &dialog.dialog_type {
                        crate::ui::DialogType::Info => {
                            if ui.button("OK").clicked() {
                                should_close = true;
                            }
                        },
                        crate::ui::DialogType::Confirmation => {
                            if ui.button("Yes").clicked() {
                                // Handle confirmation action
                                should_close = true;
                            }
                            
                            ui.add_space(10.0);
                            
                            if ui.button("No").clicked() {
                                should_close = true;
                            }
                        },
                        crate::ui::DialogType::Error => {
                            if ui.button("OK").clicked() {
                                should_close = true;
                            }
                        },
                    }
                });
                
                ui.add_space(10.0);
            });
        
        if should_close {
            overlay_state.active_dialog = None;
        }
    }
}

/// Renders loading overlay
fn loading_overlay_system(
    mut contexts: EguiContexts,
    app_state: Res<State<AppState>>,
    time: Res<Time>,
) {
    if *app_state.get() != AppState::LoadingSimulation {
        return;
    }
    
    let ctx = contexts.ctx_mut();
    
    // Full screen overlay
    egui::Area::new(egui::Id::new("loading_overlay"))
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            
            ui.allocate_ui_with_layout(
                screen_rect.size(),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    // Semi-transparent background
                    ui.painter().rect_filled(
                        screen_rect,
                        egui::Rounding::ZERO,
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 180),
                    );
                    
                    ui.vertical_centered(|ui| {
                        ui.add_space(200.0);
                        
                        // Loading animation
                        let elapsed = time.elapsed_seconds();
                        let dots = match ((elapsed * 2.0) as usize) % 4 {
                            0 => "",
                            1 => ".",
                            2 => "..",
                            _ => "...",
                        };
                        
                        ui.heading(egui::RichText::new(format!("Loading Simulation{}", dots))
                            .size(32.0)
                            .color(egui::Color32::WHITE));
                        
                        ui.add_space(30.0);
                        
                        // Mock progress
                        let progress = ((elapsed * 0.5) % 3.0) / 3.0;
                        ui.add(egui::ProgressBar::new(progress)
                            .desired_width(400.0)
                            .desired_height(20.0));
                        
                        ui.add_space(20.0);
                        
                        ui.label(egui::RichText::new("Initializing drone systems and environment...")
                            .color(egui::Color32::LIGHT_GRAY));
                    });
                },
            );
        });
}

/// Renders pause overlay
fn pause_overlay_system(
    mut contexts: EguiContexts,
    mut next_app_state: ResMut<NextState<AppState>>,
    app_state: Res<State<AppState>>,
) {
    if *app_state.get() != AppState::Paused {
        return;
    }
    
    let ctx = contexts.ctx_mut();
    
    // Full screen overlay
    egui::Area::new(egui::Id::new("pause_overlay"))
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            
            ui.allocate_ui_with_layout(
                screen_rect.size(),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    // Semi-transparent background
                    ui.painter().rect_filled(
                        screen_rect,
                        egui::Rounding::ZERO,
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 120),
                    );
                    
                    ui.vertical_centered(|ui| {
                        ui.add_space(200.0);
                        
                        ui.heading(egui::RichText::new("⏸️ SIMULATION PAUSED")
                            .size(48.0)
                            .color(egui::Color32::WHITE));
                        
                        ui.add_space(50.0);
                        
                        // Pause menu buttons
                        let button_size = egui::Vec2::new(250.0, 50.0);
                        
                        if ui.add_sized(button_size, egui::Button::new(
                            egui::RichText::new("▶️ Resume").size(18.0)
                        )).clicked() {
                            next_app_state.set(AppState::Simulation);
                        }
                        
                        ui.add_space(15.0);
                        
                        if ui.add_sized(button_size, egui::Button::new(
                            egui::RichText::new("⚙️ Settings").size(18.0)
                        )).clicked() {
                            next_app_state.set(AppState::Settings);
                        }
                        
                        ui.add_space(15.0);
                        
                        if ui.add_sized(button_size, egui::Button::new(
                            egui::RichText::new("🌍 World Map").size(18.0)
                        )).clicked() {
                            next_app_state.set(AppState::WorldMap);
                        }
                        
                        ui.add_space(15.0);
                        
                        if ui.add_sized(button_size, egui::Button::new(
                            egui::RichText::new("🏠 Main Menu").size(18.0)
                        )).clicked() {
                            next_app_state.set(AppState::MainMenu);
                        }
                        
                        ui.add_space(30.0);
                        
                        ui.label(egui::RichText::new("Press ESC to resume")
                            .size(14.0)
                            .color(egui::Color32::GRAY));
                    });
                },
            );
        });
}
