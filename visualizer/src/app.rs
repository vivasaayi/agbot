use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use crate::camera::CameraPlugin;
use crate::communication::{CommunicationPlugin, CommunicationChannels};
use crate::drone_controller::DroneControllerPlugin;
use crate::hud::HudPlugin;
use crate::resources::{AppConfig, AppState, DroneRegistry, MissionData, TerrainData};
use crate::systems::*;
use crate::terrain::TerrainPlugin;
use crate::ui::UiPlugin;

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
            
            // Custom plugins
            .add_plugins(CameraPlugin)
            .add_plugins(TerrainPlugin)
            .add_plugins(DroneControllerPlugin)
            .add_plugins(CommunicationPlugin)
            .add_plugins(HudPlugin)
            .add_plugins(UiPlugin)
            
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

    // Add ground plane
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::default().mesh().size(1000.0, 1000.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.5, 0.3),
            ..default()
        }),
        ..default()
    });
}
