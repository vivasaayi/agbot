use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::flight_ui::{AppState, UIOverlayState, NotificationLevel};

pub struct SimulationHudPlugin;

impl Plugin for SimulationHudPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SimulationHudState>()
            .add_systems(OnEnter(AppState::Simulation), setup_simulation_hud)
            .add_systems(Update, simulation_hud_ui.run_if(in_state(AppState::Simulation)))
            .add_systems(OnExit(AppState::Simulation), cleanup_simulation_hud);
    }
}

#[derive(Component)]
struct SimulationHudEntity;

#[derive(Resource, Default)]
pub struct SimulationHudState {
    pub show_telemetry: bool,
    pub show_mission_panel: bool,
    pub show_instruments: bool,
    pub show_minimap: bool,
    pub hud_opacity: f32,
    pub active_drone_id: Option<u32>,
    pub mission_status: MissionStatus,
    pub telemetry_data: DronetelemetryData,
    pub flight_mode: FlightMode,
    pub selected_tool: DroneTools,
}

#[derive(Default, Debug, Clone)]
pub enum MissionStatus {
    #[default]
    Standby,
    Takeoff,
    InProgress,
    Returning,
    Landing,
    Completed,
    Aborted,
}

#[derive(Default, Debug, Clone)]
pub enum FlightMode {
    #[default]
    Manual,
    Assisted,
    Autonomous,
    Emergency,
}

#[derive(Default, Debug, Clone)]
pub enum DroneTools {
    #[default]
    Camera,
    Sprayer,
    Seeder,
    Sampler,
    Scanner,
    Multispectral,
}

#[derive(Default)]
pub struct DronetelemetryData {
    pub altitude: f32,
    pub speed: f32,
    pub battery: f32,
    pub heading: f32,
    pub latitude: f64,
    pub longitude: f64,
    pub signal_strength: f32,
    pub wind_speed: f32,
    pub wind_direction: f32,
    pub temperature: f32,
    pub humidity: f32,
}

fn setup_simulation_hud(
    mut commands: Commands,
    mut hud_state: ResMut<SimulationHudState>,
) {
    info!("Setting up simulation HUD");
    
    // Initialize default HUD state
    hud_state.show_telemetry = true;
    hud_state.show_mission_panel = true;
    hud_state.show_instruments = true;
    hud_state.show_minimap = true;
    hud_state.hud_opacity = 0.9;
    
    // Mock telemetry data
    hud_state.telemetry_data = DronetelemetryData {
        altitude: 100.0,
        speed: 12.5,
        battery: 87.0,
        heading: 245.0,
        latitude: 37.7749,
        longitude: -122.4194,
        signal_strength: 95.0,
        wind_speed: 5.2,
        wind_direction: 220.0,
        temperature: 22.5,
        humidity: 65.0,
    };
    
    commands.spawn((
        SimulationHudEntity,
        Name::new("SimulationHUD"),
    ));
}

fn cleanup_simulation_hud(
    mut commands: Commands,
    hud_entities: Query<Entity, With<SimulationHudEntity>>,
) {
    for entity in hud_entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn simulation_hud_ui(
    mut contexts: EguiContexts,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut hud_state: ResMut<SimulationHudState>,
    mut overlay_state: ResMut<UIOverlayState>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let ctx = contexts.ctx_mut();
    
    // Handle escape key to pause
    if keyboard_input.just_pressed(KeyCode::Escape) {
        next_app_state.set(AppState::Paused);
        return;
    }
    
    // Update mock telemetry data with some animation
    update_mock_telemetry(&mut hud_state.telemetry_data, &time);
    
    // Top HUD bar - Mission status and basic controls
    egui::TopBottomPanel::top("simulation_top_hud")
        .resizable(false)
        .min_height(50.0)
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(20, 20, 30, (255.0 * hud_state.hud_opacity) as u8)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                
                // Mission status indicator
                let status_color = match hud_state.mission_status {
                    MissionStatus::Standby => egui::Color32::YELLOW,
                    MissionStatus::InProgress => egui::Color32::GREEN,
                    MissionStatus::Completed => egui::Color32::BLUE,
                    MissionStatus::Aborted => egui::Color32::RED,
                    _ => egui::Color32::ORANGE,
                };
                
                ui.colored_label(status_color, format!("● {:?}", hud_state.mission_status));
                
                ui.separator();
                
                // Flight mode
                ui.label(format!("Mode: {:?}", hud_state.flight_mode));
                
                ui.separator();
                
                // Quick stats
                ui.label(format!("Alt: {:.1}m", hud_state.telemetry_data.altitude));
                ui.label(format!("Spd: {:.1}m/s", hud_state.telemetry_data.speed));
                ui.label(format!("Bat: {:.0}%", hud_state.telemetry_data.battery));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    
                    if ui.button("⏸️ Pause").clicked() {
                        next_app_state.set(AppState::Paused);
                    }
                    
                    if ui.button("🏠 RTH").clicked() {
                        hud_state.mission_status = MissionStatus::Returning;
                        overlay_state.add_simple_notification("Return to Home initiated".to_string());
                    }
                    
                    if ui.button("🚨 Emergency").clicked() {
                        hud_state.flight_mode = FlightMode::Emergency;
                        hud_state.mission_status = MissionStatus::Aborted;
                        overlay_state.add_notification_with_level("EMERGENCY STOP ACTIVATED".to_string(), NotificationLevel::Error);
                    }
                });
            });
        });
    
    // Left telemetry panel
    if hud_state.show_telemetry {
        egui::SidePanel::left("telemetry_panel")
            .resizable(true)
            .default_width(280.0)
            .width_range(250.0..=350.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(15, 15, 25, (230.0 * hud_state.hud_opacity) as u8)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(10.0);
                    
                    // Telemetry header
                    ui.horizontal(|ui| {
                        ui.heading("📊 Telemetry");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                hud_state.show_telemetry = false;
                            }
                        });
                    });
                    
                    ui.separator();
                    ui.add_space(10.0);
                    
                    // Flight data
                    render_telemetry_section(ui, "Flight Data", &[
                        ("Altitude", format!("{:.1} m", hud_state.telemetry_data.altitude)),
                        ("Speed", format!("{:.1} m/s", hud_state.telemetry_data.speed)),
                        ("Heading", format!("{:.0}°", hud_state.telemetry_data.heading)),
                        ("Signal", format!("{:.0}%", hud_state.telemetry_data.signal_strength)),
                    ]);
                    
                    ui.add_space(10.0);
                    
                    // Battery with visual indicator
                    ui.group(|ui| {
                        ui.label("🔋 Power System");
                        ui.add_space(5.0);
                        
                        let battery_color = if hud_state.telemetry_data.battery > 50.0 {
                            egui::Color32::GREEN
                        } else if hud_state.telemetry_data.battery > 20.0 {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::RED
                        };
                        
                        ui.horizontal(|ui| {
                            ui.add(egui::ProgressBar::new(hud_state.telemetry_data.battery / 100.0)
                                .desired_width(150.0)
                                .fill(battery_color));
                            ui.label(format!("{:.0}%", hud_state.telemetry_data.battery));
                        });
                    });
                    
                    ui.add_space(10.0);
                    
                    // GPS coordinates
                    render_telemetry_section(ui, "GPS Position", &[
                        ("Latitude", format!("{:.6}°", hud_state.telemetry_data.latitude)),
                        ("Longitude", format!("{:.6}°", hud_state.telemetry_data.longitude)),
                    ]);
                    
                    ui.add_space(10.0);
                    
                    // Environmental data
                    render_telemetry_section(ui, "Environment", &[
                        ("Wind Speed", format!("{:.1} m/s", hud_state.telemetry_data.wind_speed)),
                        ("Wind Dir", format!("{:.0}°", hud_state.telemetry_data.wind_direction)),
                        ("Temperature", format!("{:.1}°C", hud_state.telemetry_data.temperature)),
                        ("Humidity", format!("{:.0}%", hud_state.telemetry_data.humidity)),
                    ]);
                });
            });
    }
    
    // Right mission panel
    if hud_state.show_mission_panel {
        egui::SidePanel::right("mission_panel")
            .resizable(true)
            .default_width(300.0)
            .width_range(280.0..=400.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(15, 15, 25, (230.0 * hud_state.hud_opacity) as u8)))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.add_space(10.0);
                    
                    // Mission panel header
                    ui.horizontal(|ui| {
                        ui.heading("🎯 Mission Control");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                hud_state.show_mission_panel = false;
                            }
                        });
                    });
                    
                    ui.separator();
                    ui.add_space(10.0);
                    
                    // Tool selection
                    ui.group(|ui| {
                        ui.label("🔧 Active Tool");
                        ui.add_space(5.0);
                        
                        egui::ComboBox::from_label("")
                            .selected_text(format!("{:?}", hud_state.selected_tool))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut hud_state.selected_tool, DroneTools::Camera, "📷 Camera");
                                ui.selectable_value(&mut hud_state.selected_tool, DroneTools::Sprayer, "💧 Sprayer");
                                ui.selectable_value(&mut hud_state.selected_tool, DroneTools::Seeder, "🌱 Seeder");
                                ui.selectable_value(&mut hud_state.selected_tool, DroneTools::Sampler, "🧪 Sampler");
                                ui.selectable_value(&mut hud_state.selected_tool, DroneTools::Scanner, "📡 Scanner");
                                ui.selectable_value(&mut hud_state.selected_tool, DroneTools::Multispectral, "🌈 Multispectral");
                            });
                    });
                    
                    ui.add_space(10.0);
                    
                    // Mission waypoints
                    ui.group(|ui| {
                        ui.label("📍 Waypoints");
                        ui.add_space(5.0);
                        
                        egui::ScrollArea::vertical()
                            .max_height(150.0)
                            .show(ui, |ui| {
                                for i in 1..=5 {
                                    ui.horizontal(|ui| {
                                        let completed = i <= 2;
                                        let icon = if completed { "✅" } else { "⏳" };
                                        ui.label(format!("{} Waypoint {}", icon, i));
                                        
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if !completed && ui.small_button("Skip").clicked() {
                                                overlay_state.add_simple_notification(format!("Skipped waypoint {}", i));
                                            }
                                        });
                                    });
                                    ui.separator();
                                }
                            });
                    });
                    
                    ui.add_space(10.0);
                    
                    // Mission controls
                    ui.group(|ui| {
                        ui.label("🎮 Controls");
                        ui.add_space(5.0);
                        
                        ui.horizontal(|ui| {
                            if ui.button("▶️ Start").clicked() {
                                hud_state.mission_status = MissionStatus::InProgress;
                                overlay_state.add_notification_with_level("Mission started".to_string(), NotificationLevel::Success);
                            }
                            
                            if ui.button("⏸️ Pause").clicked() {
                                next_app_state.set(AppState::Paused);
                            }
                            
                            if ui.button("⏹️ Stop").clicked() {
                                hud_state.mission_status = MissionStatus::Aborted;
                                overlay_state.add_notification_with_level("Mission aborted".to_string(), NotificationLevel::Warning);
                            }
                        });
                    });
                });
            });
    }
    
    // Bottom HUD - Instruments and controls
    if hud_state.show_instruments {
        egui::TopBottomPanel::bottom("simulation_bottom_hud")
            .resizable(false)
            .min_height(100.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(15, 15, 25, (200.0 * hud_state.hud_opacity) as u8)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(20.0);
                    
                    // Artificial horizon indicator (simplified)
                    render_artificial_horizon(ui, hud_state.telemetry_data.heading);
                    
                    ui.add_space(30.0);
                    
                    // Altitude indicator
                    render_altitude_indicator(ui, hud_state.telemetry_data.altitude);
                    
                    ui.add_space(30.0);
                    
                    // Speed indicator
                    render_speed_indicator(ui, hud_state.telemetry_data.speed);
                    
                    ui.add_space(30.0);
                    
                    // Compass
                    render_compass(ui, hud_state.telemetry_data.heading);
                });
            });
    }
    
    // HUD opacity control (always visible)
    egui::Window::new("HUD Settings")
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-10.0, 70.0))
        .resizable(false)
        .collapsible(true)
        .default_open(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                ui.add(egui::Slider::new(&mut hud_state.hud_opacity, 0.1..=1.0));
            });
            
            ui.checkbox(&mut hud_state.show_telemetry, "Telemetry");
            ui.checkbox(&mut hud_state.show_mission_panel, "Mission Panel");
            ui.checkbox(&mut hud_state.show_instruments, "Instruments");
            ui.checkbox(&mut hud_state.show_minimap, "Mini Map");
        });
}

fn update_mock_telemetry(telemetry: &mut DronetelemetryData, time: &Time) {
    let elapsed = time.elapsed_seconds();
    
    // Simulate some realistic changes
    telemetry.altitude += (elapsed * 0.5).sin() * 0.1;
    telemetry.speed = 12.5 + (elapsed * 0.3).sin() * 2.0;
    telemetry.battery = (telemetry.battery - 0.01).max(0.0);
    telemetry.heading = (telemetry.heading + 0.1) % 360.0;
    telemetry.wind_speed = 5.2 + (elapsed * 0.2).sin() * 1.0;
}

fn render_telemetry_section(ui: &mut egui::Ui, title: &str, data: &[(&str, String)]) {
    ui.group(|ui| {
        ui.label(title);
        ui.add_space(5.0);
        
        for (label, value) in data {
            ui.horizontal(|ui| {
                ui.label(*label);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(value);
                });
            });
        }
    });
}

fn render_artificial_horizon(ui: &mut egui::Ui, heading: f32) {
    ui.group(|ui| {
        ui.label("Attitude");
        ui.add_space(5.0);
        
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(80.0, 60.0), egui::Sense::hover());
        
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let center = rect.center();
            
            // Simple horizon line
            painter.line_segment(
                [center + egui::Vec2::new(-30.0, 0.0), center + egui::Vec2::new(30.0, 0.0)],
                egui::Stroke::new(2.0, egui::Color32::WHITE),
            );
            
            // Aircraft symbol
            painter.line_segment(
                [center + egui::Vec2::new(-10.0, 0.0), center + egui::Vec2::new(10.0, 0.0)],
                egui::Stroke::new(3.0, egui::Color32::YELLOW),
            );
        }
    });
}

fn render_altitude_indicator(ui: &mut egui::Ui, altitude: f32) {
    ui.group(|ui| {
        ui.label("Altitude");
        ui.add_space(5.0);
        ui.label(format!("{:.1}m", altitude));
        
        let progress = (altitude / 200.0).min(1.0);
        ui.add(egui::ProgressBar::new(progress)
            .desired_width(60.0)
            .fill(egui::Color32::LIGHT_BLUE));
    });
}

fn render_speed_indicator(ui: &mut egui::Ui, speed: f32) {
    ui.group(|ui| {
        ui.label("Speed");
        ui.add_space(5.0);
        ui.label(format!("{:.1}m/s", speed));
        
        let progress = (speed / 30.0).min(1.0);
        ui.add(egui::ProgressBar::new(progress)
            .desired_width(60.0)
            .fill(egui::Color32::GREEN));
    });
}

fn render_compass(ui: &mut egui::Ui, heading: f32) {
    ui.group(|ui| {
        ui.label("Heading");
        ui.add_space(5.0);
        ui.label(format!("{:.0}°", heading));
        
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(60.0, 60.0), egui::Sense::hover());
        
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let center = rect.center();
            let radius = 25.0;
            
            // Compass circle
            painter.circle_stroke(center, radius, egui::Stroke::new(2.0, egui::Color32::WHITE));
            
            // North indicator
            let north_angle = -heading.to_radians();
            let north_pos = center + egui::Vec2::new(
                north_angle.sin() * radius * 0.8,
                -north_angle.cos() * radius * 0.8,
            );
            painter.text(
                north_pos,
                egui::Align2::CENTER_CENTER,
                "N",
                egui::FontId::default(),
                egui::Color32::RED,
            );
        }
    });
}
