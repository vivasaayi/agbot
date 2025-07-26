use bevy::prelude::*;
use crate::components::{TerrainTile, SensorOverlay};
use crate::resources::{TerrainData, AppConfig};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, setup_terrain)
            .add_systems(Update, (
                load_terrain_tiles,
                update_terrain_lod,
                render_ndvi_overlay,
            ));
    }
}

fn setup_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _config: Res<AppConfig>,
) {
    info!("Setting up terrain system...");
    
    // Create initial terrain tiles
    let tile_size = 100.0;
    let tiles_per_side = 10;
    let half_tiles = tiles_per_side / 2;
    
    for x in -half_tiles..half_tiles {
        for z in -half_tiles..half_tiles {
            let world_x = x as f32 * tile_size;
            let world_z = z as f32 * tile_size;
            
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Plane3d::default().mesh().size(tile_size, tile_size)),
                    material: materials.add(StandardMaterial {
                        base_color: Color::srgb(0.2 + (x + z) as f32 * 0.05 % 0.3, 0.6, 0.2),
                        ..default()
                    }),
                    transform: Transform::from_xyz(world_x, 0.0, world_z),
                    ..default()
                },
                TerrainTile {
                    x,
                    z,
                    loaded: true,
                },
                SensorOverlay {
                    ndvi_value: 0.5 + (x + z) as f32 * 0.1 % 0.5,
                    visible: false,
                },
            ));
        }
    }
}

fn load_terrain_tiles(
    // Implementation for dynamic terrain loading based on camera position
    _camera_query: Query<&Transform, With<Camera3d>>,
    _terrain_query: Query<&TerrainTile>,
    _config: Res<AppConfig>,
) {
    // TODO: Implement dynamic terrain tile loading
    // This would load/unload terrain tiles based on camera position
}

fn update_terrain_lod(
    // Implementation for level-of-detail updates
    _camera_query: Query<&Transform, With<Camera3d>>,
    _terrain_query: Query<&TerrainTile>,
) {
    // TODO: Implement LOD system for terrain
    // Adjust mesh detail based on distance from camera
}

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
