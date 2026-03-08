//! Terrain Mesh Generator
//!
//! Creates 3D terrain meshes from elevation data with satellite imagery textures.
//! This is the core rendering component that brings real-world geography into Bevy.

use anyhow::Result;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use super::elevation::{composite_elevation, ElevationTile};
use super::imagery::{composite_imagery, ImageryTile};
use super::GeoBounds;

pub struct TerrainMeshPlugin;

impl Plugin for TerrainMeshPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainMeshConfig>()
            .add_event::<SpawnRealTerrainEvent>()
            .add_event::<TerrainReadyEvent>()
            .add_systems(Update, handle_spawn_terrain_event);
    }
}

/// Configuration for terrain mesh generation
#[derive(Resource, Clone)]
pub struct TerrainMeshConfig {
    /// Resolution of terrain mesh (vertices per side)
    pub mesh_resolution: u32,
    /// Resolution of textures
    pub texture_resolution: u32,
    /// Vertical exaggeration (1.0 = real scale)
    pub vertical_scale: f32,
    /// Whether to show NDVI overlay
    pub show_ndvi: bool,
    /// Whether to show CDL overlay
    pub show_cdl: bool,
    /// Whether to show OSM features
    pub show_osm: bool,
    /// Overlay blend opacity (0.0-1.0)
    pub overlay_opacity: f32,
}

impl Default for TerrainMeshConfig {
    fn default() -> Self {
        Self {
            mesh_resolution: 128,
            texture_resolution: 512,
            vertical_scale: 1.0,
            show_ndvi: false,
            show_cdl: false,
            show_osm: false,
            overlay_opacity: 0.6,
        }
    }
}

/// Event to trigger terrain spawning
#[derive(Event)]
pub struct SpawnRealTerrainEvent {
    pub bounds: GeoBounds,
    pub elevation_tiles: Vec<ElevationTile>,
    pub imagery_tiles: Vec<ImageryTile>,
}

/// Event fired when terrain is ready
#[derive(Event)]
pub struct TerrainReadyEvent {
    pub entity: Entity,
    pub bounds: GeoBounds,
}

/// Component marking real GIS terrain
#[derive(Component)]
pub struct RealTerrain {
    pub bounds: GeoBounds,
    pub min_elevation: f32,
    pub max_elevation: f32,
    /// Cached NDVI overlay texture
    pub ndvi_texture: Option<Handle<Image>>,
    /// Cached CDL overlay texture
    pub cdl_texture: Option<Handle<Image>>,
    /// Base satellite imagery texture
    pub base_texture: Option<Handle<Image>>,
    /// Current active overlay mode
    pub active_overlay: TerrainOverlay,
}

/// Terrain overlay modes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerrainOverlay {
    /// Just satellite imagery
    #[default]
    None,
    /// NDVI vegetation index overlay
    Ndvi,
    /// Crop classification overlay
    Cdl,
    /// Blended satellite + NDVI
    BlendedNdvi,
}

fn handle_spawn_terrain_event(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    config: Res<TerrainMeshConfig>,
    ndvi_config: Res<super::ndvi::NdviConfig>,
    mut spawn_events: EventReader<SpawnRealTerrainEvent>,
    mut ready_events: EventWriter<TerrainReadyEvent>,
    existing_terrain: Query<Entity, With<RealTerrain>>,
) {
    for event in spawn_events.read() {
        // Remove existing terrain
        for entity in existing_terrain.iter() {
            commands.entity(entity).despawn_recursive();
        }

        info!(
            "Spawning real terrain: lat {:.4} to {:.4}, lon {:.4} to {:.4}",
            event.bounds.min_lat, event.bounds.max_lat, event.bounds.min_lon, event.bounds.max_lon
        );

        // Generate heightmap from elevation tiles
        let heightmap =
            composite_elevation(&event.elevation_tiles, event.bounds, config.mesh_resolution);

        // Generate texture from imagery tiles
        let texture_pixels = composite_imagery(
            &event.imagery_tiles,
            event.bounds,
            config.texture_resolution,
        );

        // Find elevation range
        let min_elev = heightmap.iter().cloned().fold(f32::MAX, f32::min);
        let max_elev = heightmap.iter().cloned().fold(f32::MIN, f32::max);

        info!(
            "Terrain elevation range: {:.1}m to {:.1}m",
            min_elev, max_elev
        );

        // Create the terrain mesh
        let mesh = create_terrain_mesh(
            &heightmap,
            config.mesh_resolution,
            event.bounds.width_m() as f32,
            event.bounds.height_m() as f32,
            min_elev,
            config.vertical_scale,
        );

        // Create base satellite texture
        let base_texture = create_terrain_texture(&texture_pixels, config.texture_resolution);
        let base_texture_handle = images.add(base_texture);

        // Compute NDVI overlay
        let ndvi_data = super::ndvi::compute_pseudo_ndvi_from_tiles(
            &event.imagery_tiles,
            event.bounds,
            config.texture_resolution,
        );

        info!(
            "NDVI computed: mean={:.3}, vegetation={:.1}%, healthy={:.1}%",
            ndvi_data.stats.mean_ndvi,
            ndvi_data.stats.vegetation_coverage,
            ndvi_data.stats.healthy_vegetation
        );

        // Create NDVI overlay texture
        let ndvi_texture = super::ndvi::ndvi_to_texture(&ndvi_data, &ndvi_config);
        let ndvi_texture_handle = images.add(ndvi_texture);

        // Create material - use base texture or NDVI based on config
        let active_texture = if config.show_ndvi {
            // Blend base with NDVI overlay
            let blended = super::ndvi::blend_ndvi_overlay(
                &texture_pixels,
                &super::ndvi::ndvi_to_texture(&ndvi_data, &ndvi_config).data,
                config.overlay_opacity,
            );
            let blended_tex = create_terrain_texture(&blended, config.texture_resolution);
            images.add(blended_tex)
        } else {
            base_texture_handle.clone()
        };

        // Create material with satellite imagery
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(active_texture),
            perceptual_roughness: 0.9,
            metallic: 0.0,
            ..default()
        });

        let active_overlay = if config.show_ndvi {
            TerrainOverlay::BlendedNdvi
        } else {
            TerrainOverlay::None
        };

        // Spawn the terrain entity
        let entity = commands
            .spawn((
                PbrBundle {
                    mesh: meshes.add(mesh),
                    material,
                    transform: Transform::from_translation(Vec3::ZERO),
                    ..default()
                },
                RealTerrain {
                    bounds: event.bounds,
                    min_elevation: min_elev,
                    max_elevation: max_elev,
                    ndvi_texture: Some(ndvi_texture_handle),
                    cdl_texture: None, // TODO: Load CDL if enabled
                    base_texture: Some(base_texture_handle),
                    active_overlay,
                },
                Name::new("RealTerrain"),
            ))
            .id();

        ready_events.send(TerrainReadyEvent {
            entity,
            bounds: event.bounds,
        });

        info!("Real terrain spawned successfully");
    }
}

/// Create a terrain mesh from heightmap data
fn create_terrain_mesh(
    heightmap: &[f32],
    resolution: u32,
    width: f32,
    depth: f32,
    base_elevation: f32,
    vertical_scale: f32,
) -> Mesh {
    let vertex_count = resolution * resolution;
    let mut positions = Vec::with_capacity(vertex_count as usize);
    let mut normals = Vec::with_capacity(vertex_count as usize);
    let mut uvs = Vec::with_capacity(vertex_count as usize);

    // Center the mesh at origin
    let half_width = width / 2.0;
    let half_depth = depth / 2.0;

    // Generate vertices
    for z in 0..resolution {
        for x in 0..resolution {
            let u = x as f32 / (resolution - 1) as f32;
            let v = z as f32 / (resolution - 1) as f32;

            let px = u * width - half_width;
            let pz = v * depth - half_depth;

            let height_idx = (z * resolution + x) as usize;
            let height = (heightmap[height_idx] - base_elevation) * vertical_scale;

            positions.push([px, height, pz]);
            uvs.push([u, 1.0 - v]); // Flip V for correct texture orientation

            // Placeholder normal (will calculate properly)
            normals.push([0.0, 1.0, 0.0]);
        }
    }

    // Calculate proper normals using finite differences
    for z in 0..resolution {
        for x in 0..resolution {
            let idx = (z * resolution + x) as usize;

            // Get neighboring heights
            let h_l = if x > 0 {
                positions[(z * resolution + x - 1) as usize][1]
            } else {
                positions[idx][1]
            };
            let h_r = if x < resolution - 1 {
                positions[(z * resolution + x + 1) as usize][1]
            } else {
                positions[idx][1]
            };
            let h_d = if z > 0 {
                positions[((z - 1) * resolution + x) as usize][1]
            } else {
                positions[idx][1]
            };
            let h_u = if z < resolution - 1 {
                positions[((z + 1) * resolution + x) as usize][1]
            } else {
                positions[idx][1]
            };

            let step_x = width / (resolution - 1) as f32;
            let step_z = depth / (resolution - 1) as f32;

            // Gradient
            let dx = (h_r - h_l) / (2.0 * step_x);
            let dz = (h_u - h_d) / (2.0 * step_z);

            // Normal from gradient
            let normal = Vec3::new(-dx, 1.0, -dz).normalize();
            normals[idx] = [normal.x, normal.y, normal.z];
        }
    }

    // Generate indices for triangle strip
    let mut indices = Vec::new();
    for z in 0..(resolution - 1) {
        for x in 0..(resolution - 1) {
            let top_left = z * resolution + x;
            let top_right = top_left + 1;
            let bottom_left = (z + 1) * resolution + x;
            let bottom_right = bottom_left + 1;

            // Two triangles per quad
            indices.push(top_left);
            indices.push(bottom_left);
            indices.push(top_right);

            indices.push(top_right);
            indices.push(bottom_left);
            indices.push(bottom_right);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Create a Bevy Image from RGBA pixel data
fn create_terrain_texture(pixels: &[u8], resolution: u32) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    Image::new(
        Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels.to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Helper to create NDVI overlay texture
pub fn create_ndvi_overlay_texture(ndvi_pixels: &[u8], resolution: u32) -> Image {
    create_terrain_texture(ndvi_pixels, resolution)
}
