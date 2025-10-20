use crate::app_state::AppMode;
use crate::components::{SensorOverlay, TerrainTile};
use crate::resources::AppConfig;
use bevy::prelude::*;

pub mod streamer;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppMode::Simulation3D), setup_terrain)
            .add_systems(OnExit(AppMode::Simulation3D), cleanup_terrain)
            .add_systems(
                Update,
                (
                    streamer::terrain_streamer_system,
                    streamer::update_terrain_lod,
                    render_ndvi_overlay,
                    handle_terrain_click,
                )
                    .run_if(in_state(AppMode::Simulation3D)),
            );
    }
}

#[derive(Component)]
struct TerrainRoot;

fn setup_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _config: Res<AppConfig>,
) {
    info!("Setting up terrain system...");

    let root = commands.spawn((TerrainRoot, Name::new("TerrainRoot"))).id();

    // Create initial terrain tiles
    let tile_size = 100.0;
    let tiles_per_side = 10;
    let half_tiles = tiles_per_side / 2;

    for x in -half_tiles..half_tiles {
        for z in -half_tiles..half_tiles {
            let world_x = x as f32 * tile_size;
            let world_z = z as f32 * tile_size;

            let tile = commands
                .spawn((
                    PbrBundle {
                        mesh: meshes.add(Plane3d::default().mesh().size(tile_size, tile_size)),
                        material: materials.add(StandardMaterial {
                            base_color: Color::srgb(0.2 + (x + z) as f32 * 0.05 % 0.3, 0.6, 0.2),
                            ..default()
                        }),
                        transform: Transform::from_xyz(world_x, 0.0, world_z),
                        ..default()
                    },
                    TerrainTile { x, z, loaded: true },
                    SensorOverlay {
                        ndvi_value: 0.5 + (x + z) as f32 * 0.1 % 0.5,
                        visible: false,
                    },
                ))
                .id();
            commands.entity(root).add_child(tile);
        }
    }
}

fn cleanup_terrain(mut commands: Commands, query: Query<Entity, With<TerrainRoot>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

// LOD now handled in streamer module

fn render_ndvi_overlay(
    mut terrain_query: Query<(&mut Handle<StandardMaterial>, &SensorOverlay), With<TerrainTile>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<AppConfig>,
) {
    if !config.rendering.show_ndvi_overlay {
        return;
    }

    for (material_handle, overlay) in terrain_query.iter_mut() {
        if overlay.visible {
            if let Some(material) = materials.get_mut(&*material_handle) {
                // Map NDVI value to color (red = low, green = high)
                let ndvi = overlay.ndvi_value;
                material.base_color = Color::srgb(1.0 - ndvi, ndvi, 0.0);
            }
        }
    }
}

fn handle_terrain_click(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !mouse_button_input.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok((camera, camera_transform)) = camera_q.get_single() else {
        return;
    };
    let Ok(window) = windows.get_single() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        // Convert screen coordinates to world coordinates
        if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
            let distance = -ray.origin.y / ray.direction.y; // Assuming ground at y=0
            let world_pos = ray.origin + ray.direction * distance;
            info!(
                "Clicked at world position: ({:.2}, {:.2}, {:.2})",
                world_pos.x, world_pos.y, world_pos.z
            );

            // For now, just spawn a marker cube at the click position
            commands.spawn(PbrBundle {
                mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.0, 0.0),
                    ..default()
                }),
                transform: Transform::from_translation(world_pos),
                ..default()
            });
        }
    }
}
