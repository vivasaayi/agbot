use bevy::prelude::*;
use crate::components::TerrainTile;

#[derive(Resource, Debug, Clone)]
pub struct TerrainStreamConfig {
    pub tile_size: f32,
    pub ring_tile_counts: [i32; 3], // R0, R1, R2 tiles per side
    #[allow(dead_code)]
    pub max_tiles: usize,
}

impl Default for TerrainStreamConfig {
    fn default() -> Self {
        Self { tile_size: 100.0, ring_tile_counts: [5, 9, 13], max_tiles: 13 * 13 } // up to R2
    }
}

pub fn terrain_streamer_system(
    mut commands: Commands,
    camera_q: Query<&Transform, With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing_tiles: Query<(Entity, &TerrainTile)>,
    stream_cfg: Option<Res<TerrainStreamConfig>>,
) {
    let cfg = stream_cfg.map(|r| r.clone()).unwrap_or_default();
    let tile_size = cfg.tile_size;

    let Ok(cam_t) = camera_q.get_single() else { return; };
    let cam_x = cam_t.translation.x;
    let cam_z = cam_t.translation.z;
    let cx = (cam_x / tile_size).round() as i32;
    let cz = (cam_z / tile_size).round() as i32;

    // Determine desired set of tiles within R2 ring
    let r2 = cfg.ring_tile_counts[2] / 2;
    let mut desired: Vec<(i32, i32)> = Vec::new();
    for dx in -r2..=r2 { for dz in -r2..=r2 { desired.push((cx + dx, cz + dz)); } }

    // Build a map of existing tiles for quick lookup
    use std::collections::HashSet;
    let mut existing_set: HashSet<(i32, i32)> = HashSet::new();
    for (_e, t) in existing_tiles.iter() { existing_set.insert((t.x, t.z)); }

    // Spawn missing tiles
    let mut spawned = 0;
    for (tx, tz) in desired.iter() {
        if !existing_set.contains(&(*tx, *tz)) && spawned < cfg.max_tiles {
            let world_x = *tx as f32 * tile_size;
            let world_z = *tz as f32 * tile_size;

            // Simple material color by position for visualization
            let hue = (((tx + tz) % 360) as f32).abs() / 360.0;
            let color = Color::hsl(hue, 0.5, 0.4);

            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Plane3d::default().mesh().size(tile_size, tile_size)),
                    material: materials.add(StandardMaterial { base_color: color, ..default() }),
                    transform: Transform::from_xyz(world_x, sample_height(*tx, *tz, &cfg), world_z),
                    ..default()
                },
                TerrainTile { x: *tx, z: *tz, loaded: true },
            ));
            spawned += 1;
        }
    }

    // Evict tiles beyond R2 bounds to keep memory in check
    for (entity, tile) in existing_tiles.iter() {
        if (tile.x - cx).abs() > r2 || (tile.z - cz).abs() > r2 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn update_terrain_lod(
    camera_q: Query<&Transform, With<Camera3d>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&Transform, &mut Handle<StandardMaterial>, &TerrainTile)>,
) {
    let Ok(cam_t) = camera_q.get_single() else { return; };
    for (t, mat_h, tile) in q.iter_mut() {
        let d = cam_t.translation.distance(t.translation);
        // Adjust lightness by LOD tier for visual feedback
        let hue = (((tile.x + tile.z) % 360) as f32).abs() / 360.0;
        let light = if d < 200.0 { 0.5 } else if d < 400.0 { 0.4 } else { 0.3 };
        if let Some(mat) = materials.get_mut(&*mat_h) { mat.base_color = Color::hsl(hue, 0.5, light); }
    }
}

// Stub height generator: gentle sine wave for variation
fn sample_height(tx: i32, tz: i32, _cfg: &TerrainStreamConfig) -> f32 {
    let fx = tx as f32 * 0.1;
    let fz = tz as f32 * 0.1;
    (fx.sin() * fz.cos()) * 2.0
}
