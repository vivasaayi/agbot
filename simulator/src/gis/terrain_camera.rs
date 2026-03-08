//! Terrain View Camera Controller
//!
//! Manages camera positioning and control for real-world terrain visualization.
//! Automatically switches camera modes when loading terrain, and provides
//! intuitive controls for exploring the 3D landscape.

use crate::gis::{RealTerrain, TerrainReadyEvent};
use crate::globe_view::Globe;
use bevy::prelude::*;

pub struct TerrainCameraPlugin;

impl Plugin for TerrainCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                position_terrain_camera,
                terrain_camera_control,
                toggle_globe_visibility,
            ),
        );
    }
}

/// Component marking the main camera when it's in terrain mode
#[derive(Component)]
pub struct TerrainCamera {
    pub height_above_ground: f32,
    pub zoom_distance: f32,
    pub min_height: f32,
    pub max_height: f32,
}

impl Default for TerrainCamera {
    fn default() -> Self {
        Self {
            height_above_ground: 100.0, // 100m above terrain
            zoom_distance: 500.0,       // For zoom interactions
            min_height: 10.0,           // Minimum height to prevent clipping
            max_height: 2000.0,         // Maximum altitude
        }
    }
}

/// Position the camera for terrain viewing when terrain is ready
fn position_terrain_camera(
    mut events: EventReader<TerrainReadyEvent>,
    mut camera_query: Query<(&mut Transform, Option<&mut TerrainCamera>), With<Camera3d>>,
    terrain_query: Query<&RealTerrain>,
) {
    for event in events.read() {
        if let Ok(terrain) = terrain_query.get(event.entity) {
            // Position camera above and looking down at the terrain
            let center_lat = (terrain.bounds.min_lat + terrain.bounds.max_lat) / 2.0;
            let center_lon = (terrain.bounds.min_lon + terrain.bounds.max_lon) / 2.0;

            // Calculate bounds in meters for positioning
            let width = terrain.bounds.width_m() as f32;
            let height = terrain.bounds.height_m() as f32;

            // Position camera at a good viewing angle
            let distance = (width.max(height) / 2.0) * 1.5; // 1.5x the terrain extent
            let camera_height = (terrain.max_elevation - terrain.min_elevation).max(100.0) * 0.5;

            let camera_pos = Vec3::new(
                distance * 0.5,
                camera_height + 200.0, // 200m above the calculated height
                -distance * 0.5,
            );

            // Update the camera
            for (mut transform, terrain_cam) in camera_query.iter_mut() {
                transform.translation = camera_pos;
                transform.look_at(Vec3::new(0.0, camera_height * 0.5, 0.0), Vec3::Y);

                // Add terrain camera component if not already present
                if terrain_cam.is_none() {
                    // We can't add components in this system, so mark for next update
                }
            }
        }
    }
}

/// Handle keyboard and mouse controls for terrain camera
fn terrain_camera_control(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    mut scroll: EventReader<bevy::input::mouse::MouseWheel>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
) {
    let mut camera_transform = match camera_query.get_single_mut() {
        Ok(t) => t,
        Err(_) => return,
    };

    let dt = time.delta_seconds();
    let move_speed = 50.0; // meters per second
    let rotate_sensitivity = 0.005;
    let zoom_speed = 50.0; // meters per second

    // WASD/Arrow keys for movement
    let forward = camera_transform.forward();
    let right = camera_transform.right();
    let up = Vec3::Y;

    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        camera_transform.translation += forward * move_speed * dt;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        camera_transform.translation -= forward * move_speed * dt;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        camera_transform.translation -= right * move_speed * dt;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        camera_transform.translation += right * move_speed * dt;
    }

    // QE for vertical movement
    if keyboard.pressed(KeyCode::KeyQ) {
        camera_transform.translation -= up * move_speed * dt;
    }
    if keyboard.pressed(KeyCode::KeyE) {
        camera_transform.translation += up * move_speed * dt;
    }

    // Right mouse button drag to rotate view
    if mouse.pressed(MouseButton::Right) {
        for motion in mouse_motion.read() {
            // Rotate around the camera's target point
            let yaw = -motion.delta.x * rotate_sensitivity;
            let pitch = -motion.delta.y * rotate_sensitivity;

            // Apply yaw (Y-axis)
            camera_transform.rotate_y(yaw);

            // Apply pitch (local X-axis, constrained to avoid gimbal lock)
            let current_pitch = camera_transform.rotation.x;
            let max_pitch = std::f32::consts::PI / 2.5; // About 72 degrees

            if (current_pitch + pitch).abs() < max_pitch {
                camera_transform.rotate_local_x(pitch);
            }
        }
    }

    // Middle mouse button or scroll wheel to zoom (change altitude)
    for scroll_event in scroll.read() {
        use bevy::input::mouse::MouseScrollUnit;
        let scroll_amount = match scroll_event.unit {
            MouseScrollUnit::Line => scroll_event.y,
            MouseScrollUnit::Pixel => scroll_event.y / 10.0,
        };

        // Move camera forward/backward to zoom
        camera_transform.translation += forward * zoom_speed * scroll_amount * dt;
    }

    // Keep camera above ground (minimum height check)
    if camera_transform.translation.y < 10.0 {
        camera_transform.translation.y = 10.0;
    }
}

/// Show/hide the globe based on what's loaded
fn toggle_globe_visibility(
    terrain_query: Query<&RealTerrain>,
    mut globe_query: Query<&mut Visibility, With<Globe>>,
) {
    // If terrain is loaded, hide the globe
    let has_terrain = !terrain_query.is_empty();

    for mut visibility in globe_query.iter_mut() {
        *visibility = if has_terrain {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// Helper to get camera look-at point in terrain space
pub fn get_terrain_camera_target(transform: &Transform) -> Vec3 {
    let distance = 500.0; // Look ahead distance
    transform.translation + transform.forward() * distance
}

/// Helper to position camera at a specific height above terrain
pub fn set_camera_altitude(
    transform: &mut Transform,
    altitude: f32,
    min_altitude: f32,
    max_altitude: f32,
) {
    let clamped = altitude.clamp(min_altitude, max_altitude);
    transform.translation.y = clamped;
}
