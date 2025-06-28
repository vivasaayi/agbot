use bevy::prelude::*;
use crate::components::{Drone, DroneModel, DroneTrail, DroneStatus};
use crate::resources::{DroneRegistry, MissionData, AppState};

pub struct DroneControllerPlugin;

impl Plugin for DroneControllerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, (
                spawn_drone_models,
                update_drone_positions,
                update_drone_trails,
                animate_drones,
                update_drone_status_colors,
            ));
    }
}

fn spawn_drone_models(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drone_query: Query<Entity, (With<Drone>, Without<DroneModel>)>,
) {
    for drone_entity in drone_query.iter() {
        // Add visual model to drone
        let model_entity = commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(2.0, 0.5, 2.0)),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.0, 0.0, 1.0),
                    ..default()
                }),
                transform: Transform::from_xyz(0.0, 5.0, 0.0),
                ..default()
            },
            DroneModel,
        )).id();
        
        // Make the model a child of the drone entity
        commands.entity(drone_entity).push_children(&[model_entity]);
        
        // Add trail component
        commands.entity(drone_entity).insert(DroneTrail::default());
    }
}

fn update_drone_positions(
    mut drone_query: Query<(&mut Transform, &Drone)>,
    app_state: Res<AppState>,
    mission_data: Res<MissionData>,
    time: Res<Time>,
) {
    if app_state.paused {
        return;
    }

    for (mut transform, drone) in drone_query.iter_mut() {
        // In a real implementation, this would update based on:
        // 1. Live telemetry data from communication module
        // 2. Replay data when in replay mode
        // 3. Simulation data for testing
        
        if app_state.replay_mode {
            // Update from replay data
            if let Some(data_point) = mission_data.replay_data.get(mission_data.replay_index) {
                if data_point.drone_id == drone.id {
                    transform.translation = data_point.position;
                    transform.rotation = data_point.rotation;
                }
            }
        } else {
            // Simple test animation - move in a circle
            let time_sec = time.elapsed_seconds();
            let radius = 20.0;
            let speed = 0.5;
            
            transform.translation.x = (time_sec * speed).cos() * radius;
            transform.translation.z = (time_sec * speed).sin() * radius;
            transform.translation.y = 10.0 + (time_sec * 2.0).sin() * 2.0;
            
            // Face movement direction
            let forward = Vec3::new(
                -(time_sec * speed).sin(),
                0.0,
                (time_sec * speed).cos(),
            );
            transform.look_to(forward, Vec3::Y);
        }
    }
}

fn update_drone_trails(
    mut drone_query: Query<(&Transform, &mut DroneTrail), With<Drone>>,
    time: Res<Time>,
) {
    // Update trails every few frames to avoid too many points
    if time.elapsed_seconds() % 0.1 < time.delta_seconds() {
        for (transform, mut trail) in drone_query.iter_mut() {
            trail.points.push(transform.translation);
            
            // Keep trail at max length
            if trail.points.len() > trail.max_points {
                trail.points.remove(0);
            }
        }
    }
}

fn animate_drones(
    mut drone_query: Query<&mut Transform, With<DroneModel>>,
    time: Res<Time>,
) {
    for mut transform in drone_query.iter_mut() {
        // Add subtle hover animation
        let hover_height = (time.elapsed_seconds() * 2.0).sin() * 0.2;
        transform.translation.y = 5.0 + hover_height;
        
        // Add slight rotation for propeller effect
        transform.rotate_y(time.delta_seconds() * 10.0);
    }
}

fn update_drone_status_colors(
    drone_query: Query<(&Drone, &Children)>,
    mut model_query: Query<&mut Handle<StandardMaterial>, With<DroneModel>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (drone, children) in drone_query.iter() {
        for &child in children.iter() {
            if let Ok(mut material_handle) = model_query.get_mut(child) {
                if let Some(material) = materials.get_mut(&*material_handle) {
                    material.base_color = match drone.status {
                        DroneStatus::Idle => Color::srgb(0.5, 0.5, 0.5),
                        DroneStatus::Flying => Color::srgb(0.0, 0.0, 1.0),
                        DroneStatus::Mission => Color::srgb(0.0, 1.0, 0.0),
                        DroneStatus::Returning => Color::srgb(1.0, 1.0, 0.0),
                        DroneStatus::Landing => Color::srgb(1.0, 0.5, 0.0),
                        DroneStatus::Error => Color::srgb(1.0, 0.0, 0.0),
                    };
                }
            }
        }
    }
}

// Helper function to spawn a new drone
pub fn spawn_drone(
    commands: &mut Commands,
    registry: &mut ResMut<DroneRegistry>,
    id: String,
    position: Vec3,
) -> Entity {
    let drone_entity = commands.spawn((
        Drone {
            id,
            drone_type: crate::components::DroneType::Quadcopter,
            status: DroneStatus::Idle,
        },
        SpatialBundle {
            transform: Transform::from_translation(position),
            ..default()
        },
    )).id();
    
    registry.drones.push(drone_entity);
    drone_entity
}
