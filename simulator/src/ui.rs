use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::resources::{AppState, AppConfig, MissionData};
use crate::communication::{CommunicationChannels, OutgoingMessage, ViewMode};
use crate::components::DroneStatus;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            main_ui_system,
            control_panel_system,
            mission_panel_system,
        ));
    }
}

fn main_ui_system(
    mut contexts: EguiContexts,
    mut app_state: ResMut<AppState>,
    _config: Res<AppConfig>,
) {
    if !app_state.show_ui {
        return;
    }

    egui::Window::new("Visualizer Control")
        .default_pos(egui::pos2(10.0, 120.0))
        .default_size(egui::vec2(300.0, 400.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("AgBot Visualizer");
            ui.separator();
            
            // Connection status
            ui.horizontal(|ui| {
                ui.label("Connection:");
                let status_color = if app_state.connected {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                };
                ui.colored_label(status_color, if app_state.connected { "Connected" } else { "Disconnected" });
            });
            
            ui.separator();
            
            // Playback controls
            ui.heading("Playback");
            
            ui.horizontal(|ui| {
                if ui.button(if app_state.paused { "▶ Play" } else { "⏸ Pause" }).clicked() {
                    app_state.paused = !app_state.paused;
                }
                
                if ui.button("⏹ Stop").clicked() {
                    app_state.paused = true;
                    app_state.current_time = 0.0;
                }
                
                if ui.button("⏮ Reset").clicked() {
                    app_state.current_time = 0.0;
                }
            });
            
            // Time scale
            ui.horizontal(|ui| {
                ui.label("Speed:");
                ui.add(egui::Slider::new(&mut app_state.time_scale, 0.1..=10.0)
                    .text("x")
                    .logarithmic(true));
            });
            
            // Mode selection
            ui.separator();
            ui.heading("Mode");
            
            ui.horizontal(|ui| {
                ui.radio_value(&mut app_state.replay_mode, false, "Live");
                ui.radio_value(&mut app_state.replay_mode, true, "Replay");
            });
            
            ui.separator();
            
            // Display options
            ui.heading("Display Options");
            
            ui.checkbox(&mut app_state.show_inspector, "Show Inspector");
            
            if ui.button("Toggle HUD").clicked() {
                app_state.show_ui = !app_state.show_ui;
            }
            
            ui.separator();
            
            // Current time display
            ui.label(format!("Current Time: {:.1}s", app_state.current_time));
        });
}

fn control_panel_system(
    mut contexts: EguiContexts,
    app_state: Res<AppState>,
    mut config: ResMut<AppConfig>,
    channels: Option<Res<CommunicationChannels>>,
) {
    if !app_state.show_ui {
        return;
    }

    egui::Window::new("Control Panel")
        .default_pos(egui::pos2(10.0, 540.0))
        .default_size(egui::vec2(300.0, 300.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Visualization Settings");
            ui.separator();
            
            // Rendering options
            ui.checkbox(&mut config.rendering.show_ndvi_overlay, "Show NDVI Overlay");
            ui.checkbox(&mut config.rendering.show_lidar_points, "Show LiDAR Points");
            ui.checkbox(&mut config.rendering.show_sensor_data, "Show Sensor Data");
            
            ui.separator();
            
            // Camera settings
            ui.heading("Camera");
            ui.horizontal(|ui| {
                ui.label("Movement Speed:");
                ui.add(egui::Slider::new(&mut config.camera.movement_speed, 1.0..=50.0));
            });
            
            ui.horizontal(|ui| {
                ui.label("Rotation Speed:");
                ui.add(egui::Slider::new(&mut config.camera.rotation_speed, 0.1..=5.0));
            });
            
            ui.separator();
            
            // Communication controls
            ui.heading("Communication");
            
            ui.text_edit_singleline(&mut config.websocket_url);
            
            if let Some(channels) = channels.as_ref() {
                if ui.button("Request Mission Data").clicked() {
                    let _ = channels.outgoing_sender.send(OutgoingMessage::RequestMissionData("current".to_string()));
                }
                
                if ui.button("Request Replay Data").clicked() {
                    let _ = channels.outgoing_sender.send(OutgoingMessage::RequestReplayData {
                        start_time: 0.0,
                        end_time: 3600.0, // 1 hour
                    });
                }
                
                let view_mode = if app_state.replay_mode { ViewMode::Replay } else { ViewMode::Live };
                if ui.button("Update View Mode").clicked() {
                    let _ = channels.outgoing_sender.send(OutgoingMessage::SetViewMode(view_mode));
                }
            }
        });
}

fn mission_panel_system(
    mut contexts: EguiContexts,
    app_state: Res<AppState>,
    mission_data: Res<MissionData>,
    drone_query: Query<&crate::components::Drone>,
) {
    if !app_state.show_ui {
        return;
    }

    egui::Window::new("Mission Status")
        .default_pos(egui::pos2(330.0, 120.0))
        .default_size(egui::vec2(280.0, 400.0))
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Mission Information");
            ui.separator();
            
            // Current mission
            if let Some(ref mission_id) = mission_data.current_mission {
                ui.label(format!("Current Mission: {}", mission_id));
            } else {
                ui.label("No active mission");
            }
            
            ui.separator();
            
            // Waypoints
            ui.heading("Waypoints");
            if mission_data.waypoints.is_empty() {
                ui.label("No waypoints loaded");
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, waypoint) in mission_data.waypoints.iter().enumerate() {
                        ui.label(format!("{}: ({:.1}, {:.1}, {:.1})", 
                                        i + 1, waypoint.x, waypoint.y, waypoint.z));
                    }
                });
            }
            
            ui.separator();
            
            // Drone status
            ui.heading("Drone Status");
            egui::ScrollArea::vertical().show(ui, |ui| {
                for drone in drone_query.iter() {
                    ui.horizontal(|ui| {
                        ui.label(&drone.id);
                        
                        let (color, status_text) = match drone.status {
                            DroneStatus::Idle => (egui::Color32::GRAY, "Idle"),
                            DroneStatus::Flying => (egui::Color32::BLUE, "Flying"),
                            DroneStatus::Mission => (egui::Color32::GREEN, "Mission"),
                            DroneStatus::Returning => (egui::Color32::YELLOW, "Returning"),
                            DroneStatus::Landing => (egui::Color32::from_rgb(255, 165, 0), "Landing"),
                            DroneStatus::Error => (egui::Color32::RED, "Error"),
                        };
                        
                        ui.colored_label(color, status_text);
                    });
                }
                
                if drone_query.is_empty() {
                    ui.label("No drones connected");
                }
            });
            
            ui.separator();
            
            // Replay controls (if in replay mode)
            if app_state.replay_mode {
                ui.heading("Replay");
                ui.label(format!("Data points: {}", mission_data.replay_data.len()));
                ui.label(format!("Current index: {}", mission_data.replay_index));
                
                let progress = if mission_data.replay_data.is_empty() {
                    0.0
                } else {
                    mission_data.replay_index as f32 / mission_data.replay_data.len() as f32
                };
                
                ui.add(egui::ProgressBar::new(progress).text("Replay Progress"));
            }
        });
}
