use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::flight_ui::{AppState, UIOverlayState, NotificationLevel};

pub struct OverlaySystemPlugin;

impl Plugin for OverlaySystemPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, notification_system);
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
    overlay_state.notification_queue.retain_mut(|notification| {
        notification.timestamp += time.delta_seconds();
        notification.timestamp < notification.duration
    });
    
    // Display active notifications
    for (index, notification) in overlay_state.notification_queue.iter().enumerate() {
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
                    let (icon, color) = match notification.level {
                        NotificationLevel::Info => ("ℹ️", egui::Color32::LIGHT_BLUE),
                        NotificationLevel::Success => ("✅", egui::Color32::GREEN),
                        NotificationLevel::Warning => ("⚠️", egui::Color32::YELLOW),
                        NotificationLevel::Error => ("❌", egui::Color32::RED),
                    };
                    
                    ui.colored_label(color, icon);
                    ui.label(&notification.message);
                });
                
                // Progress bar showing remaining time
                let progress = (notification.duration - notification.timestamp) / notification.duration;
                ui.add(egui::ProgressBar::new(progress)
                    .desired_width(250.0)
                    .desired_height(4.0));
            });
    }
}
