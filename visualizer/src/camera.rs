use bevy::prelude::*;
use crate::components::CameraController;
use crate::resources::AppConfig;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, setup_camera)
            .add_systems(Update, (
                camera_movement,
                camera_follow_drone,
                camera_mouse_control,
            ));
    }
}

fn setup_camera(
    mut commands: Commands,
    config: Res<AppConfig>,
) {
    let camera_pos = config.camera.initial_position;
    
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_translation(camera_pos)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CameraController::default(),
    ));
}

fn camera_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<(&mut Transform, &mut CameraController), With<Camera3d>>,
    time: Res<Time>,
    config: Res<AppConfig>,
) {
    for (mut transform, mut controller) in camera_query.iter_mut() {
        if controller.follow_drone.is_some() {
            continue; // Skip manual control when following a drone
        }

        let mut movement = Vec3::ZERO;
        let speed = config.camera.movement_speed;
        
        // WASD movement
        if keyboard_input.pressed(KeyCode::KeyW) {
            movement += *transform.forward();
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            movement -= *transform.forward();
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            movement -= *transform.right();
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            movement += *transform.right();
        }
        
        // QE for up/down
        if keyboard_input.pressed(KeyCode::KeyQ) {
            movement.y -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyE) {
            movement.y += 1.0;
        }
        
        // Apply movement
        transform.translation += movement.normalize_or_zero() * speed * time.delta_seconds();
        
        // Update controller target
        controller.target = transform.translation + transform.forward() * controller.distance;
    }
}

fn camera_follow_drone(
    mut camera_query: Query<(&mut Transform, &CameraController), With<Camera3d>>,
    drone_query: Query<&Transform, (With<crate::components::Drone>, Without<Camera3d>)>,
    time: Res<Time>,
) {
    for (mut camera_transform, controller) in camera_query.iter_mut() {
        if let Some(drone_entity) = controller.follow_drone {
            if let Ok(drone_transform) = drone_query.get(drone_entity) {
                let target_pos = drone_transform.translation + Vec3::new(0.0, 10.0, 15.0);
                
                // Smooth camera following
                camera_transform.translation = camera_transform.translation.lerp(
                    target_pos,
                    time.delta_seconds() * 2.0,
                );
                
                // Look at the drone
                camera_transform.look_at(drone_transform.translation, Vec3::Y);
            }
        }
    }
}

fn camera_mouse_control(
    mut mouse_motion_events: EventReader<bevy::input::mouse::MouseMotion>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    config: Res<AppConfig>,
) {
    if !mouse_button_input.pressed(MouseButton::Right) {
        return;
    }

    for mut transform in camera_query.iter_mut() {
        for event in mouse_motion_events.read() {
            let delta = event.delta;
            let sensitivity = config.camera.rotation_speed * 0.01;
            
            // Yaw (Y-axis rotation)
            transform.rotate_y(-delta.x * sensitivity);
            
            // Pitch (local X-axis rotation)
            let pitch_delta = -delta.y * sensitivity;
            transform.rotate_local_x(pitch_delta);
        }
    }
}
