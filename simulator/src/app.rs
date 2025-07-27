use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier3d::prelude::*;

use crate::camera::CameraPlugin;
use crate::communication::{CommunicationPlugin, CommunicationChannels};
use crate::drone_controller::DroneControllerPlugin;
use crate::hud::HudPlugin;
use crate::lidar_controls::LidarControlsPlugin;
use crate::lidar_simulator::LidarSimulatorPlugin;
use crate::resources::{AppConfig, AppState, DroneRegistry, MissionData, TerrainData};
use crate::systems::*;
use crate::terrain::TerrainPlugin;
use crate::flight_ui::FlightUIPlugin;

pub struct VisualizerApp;

impl VisualizerApp {
    pub fn configure(app: &mut App, config: AppConfig, communication_channels: CommunicationChannels) {
        app
            // Bevy plugins
            .add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "AgBot Drone Visualizer".into(),
                    resolution: (1920.0, 1080.0).into(),
                    resizable: true,
                    ..default()
                }),
                ..default()
            }))
            
            // External plugins
            .add_plugins(EguiPlugin)
            .add_plugins(WorldInspectorPlugin::new())
            .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .add_plugins(RapierDebugRenderPlugin::default())
            
            // Custom plugins
            .add_plugins(CameraPlugin)
            .add_plugins(TerrainPlugin)
            .add_plugins(DroneControllerPlugin)
            .add_plugins(LidarSimulatorPlugin)
            .add_plugins(LidarControlsPlugin)
            .add_plugins(CommunicationPlugin)
            .add_plugins(HudPlugin)
            .add_plugins(FlightUIPlugin)
            
            // Resources
            .insert_resource(config)
            .insert_resource(communication_channels)
            .insert_resource(AppState::default())
            .insert_resource(DroneRegistry::default())
            .insert_resource(MissionData::default())
            .insert_resource(TerrainData::default())
            
            // Systems
            .add_systems(Startup, setup_scene)
            .add_systems(Update, (
                handle_keyboard_input,
                update_time,
                update_app_state,
            ));
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.3,
    });

    // Add directional light (sun)
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::WHITE,
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(10.0, 100.0, 10.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // Add ground plane with physics collider
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Plane3d::default().mesh().size(1000.0, 1000.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.3, 0.5, 0.3),
                ..default()
            }),
            ..default()
        },
        RigidBody::Fixed,
        Collider::cuboid(500.0, 0.1, 500.0),
    ));

    // Add some test obstacles for LiDAR to detect
    spawn_test_obstacles(&mut commands, &mut meshes, &mut materials);
}

fn spawn_test_obstacles(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    use std::f32::consts::PI;
    
    // Spawn some cubes as obstacles
    for i in 0..5 {
        let x = (i as f32 - 2.0) * 10.0;
        let height = 2.0 + i as f32;
        
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(2.0, height, 2.0)),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.4, 0.2),
                    ..default()
                }),
                transform: Transform::from_xyz(x, height / 2.0, 15.0),
                ..default()
            },
            RigidBody::Fixed,
            Collider::cuboid(1.0, height / 2.0, 1.0),
        ));
    }
    
    // Add some trees (cylinders)
    for i in 0..3 {
        let angle = i as f32 * 2.0 * PI / 3.0;
        let radius = 25.0;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cylinder::new(1.0, 8.0)),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.4, 0.8, 0.2),
                    ..default()
                }),
                transform: Transform::from_xyz(x, 4.0, z),
                ..default()
            },
            RigidBody::Fixed,
            Collider::cylinder(4.0, 1.0),
        ));
    }
}
