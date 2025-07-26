use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::components::{Drone, LidarSensor};

pub struct LidarControlsPlugin;

impl Plugin for LidarControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, lidar_controls_ui);
    }
}

fn lidar_controls_ui(
    mut contexts: EguiContexts,
    mut lidar_query: Query<&mut LidarSensor, With<Drone>>,
) {
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("LiDAR Controls")
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.heading("LiDAR Sensor Configuration");
            
            for mut lidar in lidar_query.iter_mut() {
                ui.separator();
                
                // Range control
                ui.horizontal(|ui| {
                    ui.label("Range (m):");
                    ui.add(egui::Slider::new(&mut lidar.range, 10.0..=200.0));
                });
                
                // Angular resolution
                ui.horizontal(|ui| {
                    ui.label("Angular Resolution (°):");
                    ui.add(egui::Slider::new(&mut lidar.angular_resolution, 0.1..=5.0));
                });
                
                // Scan frequency
                ui.horizontal(|ui| {
                    ui.label("Scan Frequency (Hz):");
                    ui.add(egui::Slider::new(&mut lidar.scan_frequency, 1.0..=30.0));
                });
                
                // 3D mode toggle
                ui.horizontal(|ui| {
                    ui.label("3D Mode:");
                    ui.checkbox(&mut lidar.is_3d, "Enable");
                });
                
                // 3D specific controls (only show if 3D mode is enabled)
                if lidar.is_3d {
                    ui.horizontal(|ui| {
                        ui.label("Vertical FOV (°):");
                        ui.add(egui::Slider::new(&mut lidar.vertical_fov, 5.0..=60.0));
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Vertical Resolution (°):");
                        ui.add(egui::Slider::new(&mut lidar.vertical_resolution, 0.5..=5.0));
                    });
                }
                
                ui.separator();
                
                // Display current stats
                ui.label(format!("Last scan: {:.2}s ago", lidar.last_scan_time));
                
                let estimated_points = if lidar.is_3d {
                    let h_rays = (360.0 / lidar.angular_resolution) as usize;
                    let v_rays = (lidar.vertical_fov / lidar.vertical_resolution) as usize;
                    h_rays * v_rays
                } else {
                    (360.0 / lidar.angular_resolution) as usize
                };
                
                ui.label(format!("Estimated points per scan: {}", estimated_points));
                
                // Warning for high point counts
                if estimated_points > 10000 {
                    ui.colored_label(
                        egui::Color32::RED,
                        "⚠ High point count may impact performance!"
                    );
                }
                
                ui.separator();
                
                // Export controls
                ui.heading("Export Controls");
                
                if ui.button("Export Current Scan").clicked() {
                    // TODO: Trigger export of current scan data
                    info!("Export scan button clicked");
                }
                
                if ui.button("Start Continuous Export").clicked() {
                    // TODO: Start continuous export
                    info!("Start continuous export clicked");
                }
                
                if ui.button("Stop Continuous Export").clicked() {
                    // TODO: Stop continuous export
                    info!("Stop continuous export clicked");
                }
            }
            
            if lidar_query.is_empty() {
                ui.label("No drones with LiDAR sensors found.");
                ui.label("Spawn a drone to see LiDAR controls.");
            }
        });
}
