use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::render::render_resource::Face;
use crate::app_state::{AppMode, SelectedRegion, GlobeSearchState};
use crate::earth_textures::{EarthTextures, load_earth_textures, check_texture_loading, update_earth_material_with_textures};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use crate::resources::*;
use crate::procedural_textures::create_placeholder_earth_textures;

pub struct GlobePlugin;

impl Plugin for GlobePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_earth_textures) // Only load real textures initially
            .add_systems(OnEnter(AppMode::Globe), setup_globe)
            .add_systems(OnExit(AppMode::Globe), cleanup_globe)
            .add_systems(Update, (
                rotate_globe,
                zoom_globe,
                handle_globe_click,
                update_globe_camera,
                update_selection_marker,
                handle_search_animation,
                check_texture_loading,
                update_earth_material_with_textures,
                fallback_to_procedural_textures, // Add fallback system
            ).run_if(in_state(AppMode::Globe)));
    }
}

#[derive(Component)]
pub struct Globe;

#[derive(Component)]
pub struct GlobeAtmosphere;

#[derive(Component)]
pub struct GlobeClouds;

#[derive(Component)]
pub struct StarField;

#[derive(Component)]
pub struct GlobeCamera;

#[derive(Component)]
pub struct RegionMarker;

#[derive(Resource)]
pub struct GlobeState {
    rotation_x: f32,
    rotation_y: f32,
    zoom: f32,
    is_dragging: bool,
    pub target_latitude: f32,
    pub target_longitude: f32,
    pub goto_location: bool,
}

impl Default for GlobeState {
    fn default() -> Self {
        Self {
            rotation_x: 0.0,
            rotation_y: 0.0,
            zoom: 5.0, // Distance from globe
            is_dragging: false,
            target_latitude: 0.0,
            target_longitude: 0.0,
            goto_location: false,
        }
    }
}

fn setup_globe(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _asset_server: Res<AssetServer>, // Will be used for texture loading
) {
    info!("Setting up enhanced globe view");
    
    // Initialize globe state
    commands.insert_resource(GlobeState::default());
    
    // Create enhanced Earth material optimized for texture loading
    let earth_material = materials.add(StandardMaterial {
        // Neutral base for texture overlay - will be replaced when textures load
        base_color: Color::WHITE, // White base to show texture colors accurately
        metallic: 0.0, // Will be controlled by metallic_roughness_texture
        perceptual_roughness: 0.8, // Slightly rough for realistic surface
        reflectance: 0.04, // Realistic Earth reflectance
        
        // Enhanced properties for texture mapping
        alpha_mode: AlphaMode::Opaque,
        double_sided: false,
        cull_mode: Some(Face::Back),
        
        // Textures will be loaded dynamically by the texture system
        // This allows for graceful fallback if textures aren't available
        base_color_texture: None, // Will be set to daymap
        normal_map_texture: None, // Will be set to normalmap  
        metallic_roughness_texture: None, // Will be set to specular
        emissive_texture: None, // Will be set to nightmap
        emissive: Color::srgb(0.0, 0.0, 0.0).into(), // Will be activated with night map
        
        ..default()
    });
    
    // Spawn the main Earth globe with higher subdivision for smooth appearance
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(create_earth_sphere_mesh(1.0, 64, 32)), // Custom UV-mapped sphere
            material: earth_material.clone(),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        Globe,
    ));
    
    // Create subtle atmosphere effect (larger transparent sphere)
    let atmosphere_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.5, 0.7, 1.0, 0.1), // Blue atmospheric glow
        alpha_mode: AlphaMode::Blend,
        metallic: 0.0,
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        double_sided: true,
        cull_mode: None, // Render both sides for atmosphere
        ..default()
    });
    
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(1.05).mesh().ico(4).unwrap()), // Slightly larger
            material: atmosphere_material,
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        GlobeAtmosphere,
    ));
    
    // Create globe camera with better positioning
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 5.0)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        GlobeCamera,
    ));
    
    // Create a starfield background for space atmosphere
    create_starfield(&mut commands, &mut meshes, &mut materials);
    
    // Add coordinate grid for debugging
    create_coordinate_grid(&mut commands, &mut meshes, &mut materials);
    
    // Add enhanced lighting for the globe
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.8), // Warm sunlight
            illuminance: 20000.0, // Brighter for better contrast
            shadows_enabled: true,
            shadow_depth_bias: 0.26,
            shadow_normal_bias: 0.6,
        },
        transform: Transform::from_xyz(20.0, 30.0, 20.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        cascade_shadow_config: CascadeShadowConfigBuilder {
            first_cascade_far_bound: 0.3,
            maximum_distance: 30.0,
            ..default()
        }.into(),
        ..default()
    });
    
    // Ambient light - subtle fill light
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.8, 0.85, 1.0), // Slightly blue ambient
        brightness: 0.15, // Much softer ambient for contrast
    });
}

fn create_starfield(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    // Create star material
    let star_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.9), // Warm white stars
        emissive: Color::srgb(0.8, 0.8, 0.7).into(), // Self-illuminated
        metallic: 0.0,
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Opaque,
        unlit: true, // Don't be affected by lighting
        ..default()
    });
    
    // Create many small stars in a large sphere around the scene
    for _ in 0..200 {
        // Random position on a large sphere
        let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI; // 0 to 2π
        let phi = rng.gen::<f32>() * std::f32::consts::PI; // 0 to π
        let radius = 50.0; // Far away
        
        let x = radius * phi.sin() * theta.cos();
        let y = radius * phi.cos();
        let z = radius * phi.sin() * theta.sin();
        
        let star_size = rng.gen::<f32>() * 0.02 + 0.005; // Variable star sizes
        
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Sphere::new(star_size).mesh().ico(2).unwrap()),
                material: star_material.clone(),
                transform: Transform::from_xyz(x, y, z),
                ..default()
            },
            StarField,
        ));
    }
}

fn cleanup_globe(
    mut commands: Commands,
    globe_query: Query<Entity, With<Globe>>,
    camera_query: Query<Entity, With<GlobeCamera>>,
    marker_query: Query<Entity, With<RegionMarker>>,
    atmosphere_query: Query<Entity, With<GlobeAtmosphere>>,
    starfield_query: Query<Entity, With<StarField>>,
) {
    info!("Cleaning up globe view");
    
    // Remove globe entities
    for entity in globe_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    // Remove atmosphere
    for entity in atmosphere_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    // Remove starfield
    for entity in starfield_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    // Remove globe camera
    for entity in camera_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    // Remove selection markers
    for entity in marker_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    // Remove globe state
    commands.remove_resource::<GlobeState>();
}

fn rotate_globe(
    mut globe_state: ResMut<GlobeState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut globe_query: Query<&mut Transform, With<Globe>>,
) {
    // Handle mouse dragging
    if mouse_button.pressed(MouseButton::Left) {
        globe_state.is_dragging = true;
        
        for motion in mouse_motion.read() {
            globe_state.rotation_y -= motion.delta.x * 0.01;
            globe_state.rotation_x -= motion.delta.y * 0.01;
            
            // Clamp rotation_x to prevent flipping
            globe_state.rotation_x = globe_state.rotation_x.clamp(-1.5, 1.5);
        }
    } else {
        globe_state.is_dragging = false;
    }
    
    // Apply rotation to globe
    for mut transform in globe_query.iter_mut() {
        transform.rotation = Quat::from_rotation_y(globe_state.rotation_y) 
            * Quat::from_rotation_x(globe_state.rotation_x);
    }
}

fn zoom_globe(
    mut globe_state: ResMut<GlobeState>,
    mut scroll_events: EventReader<MouseWheel>,
) {
    for event in scroll_events.read() {
        globe_state.zoom -= event.y * 0.5;
        globe_state.zoom = globe_state.zoom.clamp(1.5, 20.0);
    }
}

fn update_globe_camera(
    globe_state: Res<GlobeState>,
    mut camera_query: Query<&mut Transform, (With<GlobeCamera>, Without<Globe>)>,
) {
    for mut transform in camera_query.iter_mut() {
        let distance = globe_state.zoom;
        transform.translation = Vec3::new(0.0, 0.0, distance);
        transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}

fn handle_globe_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    globe_state: Res<GlobeState>,
    mut selected_region: ResMut<SelectedRegion>,
    camera_query: Query<(&Camera, &GlobalTransform), With<GlobeCamera>>,
    windows: Query<&Window>,
    globe_query: Query<&GlobalTransform, (With<Globe>, Without<GlobeCamera>)>,
) {
    if mouse_button.just_pressed(MouseButton::Left) && !globe_state.is_dragging {
        if let Ok(window) = windows.get_single() {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_query.get_single() {
                    if let Ok(globe_transform) = globe_query.get_single() {
                        // Cast ray from camera through cursor position
                        if let Some(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
                            // Check intersection with sphere (simplified)
                            let globe_center = globe_transform.translation();
                            let ray_dir = *ray.direction; // Convert Dir3 to Vec3
                            let to_globe = globe_center - ray.origin;
                            let projection = to_globe.dot(ray_dir);
                            
                            if projection > 0.0 {
                                let closest_point = ray.origin + ray_dir * projection;
                                let distance_to_center = (closest_point - globe_center).length();
                                
                                // If ray intersects sphere (radius = 1.0)
                                if distance_to_center <= 1.0 {
                                    // Calculate intersection point on sphere
                                    let intersection = closest_point - globe_center;
                                    let normalized = intersection.normalize();
                                    
                                    // Convert sphere coordinates to lat/lon
                                    let lat = (normalized.y).asin().to_degrees();
                                    let lon = normalized.z.atan2(normalized.x).to_degrees();
                                    
                                    // Update selected region
                                    selected_region.center_lat = lat as f64;
                                    selected_region.center_lon = lon as f64;
                                    
                                    info!("Selected location: {:.4}°N, {:.4}°E", lat, lon);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn update_selection_marker(
    mut commands: Commands,
    selected_region: Res<SelectedRegion>,
    globe_state: Res<GlobeState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    marker_query: Query<Entity, With<RegionMarker>>,
    mut marker_transform_query: Query<&mut Transform, (With<RegionMarker>, Without<Globe>)>,
) {
    // If the selected region changed, recreate the marker
    if selected_region.is_changed() {
        // Remove existing selection markers
        for entity in marker_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        
        // If we have a valid selection, create a new marker
        if selected_region.center_lat != 0.0 || selected_region.center_lon != 0.0 {
            // Convert lat/lon to sphere coordinates
            let lat_rad = (selected_region.center_lat as f32).to_radians();
            let lon_rad = (selected_region.center_lon as f32).to_radians();
            
            // Calculate position on sphere surface (correct spherical to cartesian conversion)
            // Standard Earth coordinate mapping: lat = Y rotation, lon = XZ plane rotation
            let sphere_pos = Vec3::new(
                lat_rad.cos() * lon_rad.sin(),   // X: longitude affects X component 
                lat_rad.sin(),                   // Y: latitude directly maps to Y
                -lat_rad.cos() * lon_rad.cos(),  // Z: longitude affects Z component (negative for proper orientation)
            );

            // Debug: Print coordinate conversion details
            info!(
                "🎯 MARKER DEBUG: Lat/Lon ({:.4}°, {:.4}°) → 3D Position ({:.3}, {:.3}, {:.3})",
                selected_region.center_lat, selected_region.center_lon,
                sphere_pos.x, sphere_pos.y, sphere_pos.z
            );
            
            // Apply globe's current rotation to the marker position
            let globe_rotation = Quat::from_rotation_y(globe_state.rotation_y) 
                * Quat::from_rotation_x(globe_state.rotation_x);
            let marker_pos = globe_rotation * sphere_pos;
            
            // Offset slightly from surface to avoid z-fighting
            let marker_position = marker_pos * 1.02;
            
            // Create selection marker (red circle)
            let marker_material = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.0, 0.0), // Bright red
                metallic: 0.0,
                perceptual_roughness: 0.1,
                emissive: Color::srgb(0.3, 0.0, 0.0).into(), // Slight glow
                ..default()
            });
            
            // Create a small sphere as marker
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Sphere::new(0.03).mesh().ico(3).unwrap()),
                    material: marker_material,
                    transform: Transform::from_translation(marker_position),
                    ..default()
                },
                RegionMarker,
            ));
        }
    }
    // If globe rotation changed, update marker position
    else if globe_state.is_changed() && (selected_region.center_lat != 0.0 || selected_region.center_lon != 0.0) {
        for mut marker_transform in marker_transform_query.iter_mut() {
            // Recalculate marker position with new globe rotation - USING CORRECT FORMULA
            let lat_rad = (selected_region.center_lat as f32).to_radians();
            let lon_rad = (selected_region.center_lon as f32).to_radians();
            
            // Use the SAME coordinate conversion as above for consistency
            let sphere_pos = Vec3::new(
                lat_rad.cos() * lon_rad.sin(),   // X: longitude affects X component 
                lat_rad.sin(),                   // Y: latitude directly maps to Y
                -lat_rad.cos() * lon_rad.cos(),  // Z: longitude affects Z component (negative for proper orientation)
            );
            
            let globe_rotation = Quat::from_rotation_y(globe_state.rotation_y) 
                * Quat::from_rotation_x(globe_state.rotation_x);
            let marker_pos = globe_rotation * sphere_pos;
            
            marker_transform.translation = marker_pos * 1.02;
        }
    }
}

fn handle_search_animation(
    mut search_state: ResMut<GlobeSearchState>,
    mut selected_region: ResMut<SelectedRegion>,
    mut globe_state: ResMut<GlobeState>,
    time: Res<Time>,
) {
    if !search_state.is_animating {
        return;
    }
    
    let elapsed = time.elapsed_seconds() - search_state.animation_start_time;
    let progress = (elapsed / search_state.animation_duration).clamp(0.0, 1.0);
    
    // Use smooth easing function (ease-in-out)
    let eased_progress = if progress < 0.5 {
        2.0 * progress * progress
    } else {
        -1.0 + (4.0 - 2.0 * progress) * progress
    };
    
    // Interpolate coordinates
    let current_lat = search_state.start_lat + (search_state.target_lat - search_state.start_lat) * eased_progress as f64;
    let current_lon = search_state.start_lon + (search_state.target_lon - search_state.start_lon) * eased_progress as f64;
    
    // Interpolate zoom
    let current_zoom = globe_state.zoom + (search_state.target_zoom - globe_state.zoom) * eased_progress;
    
    // Update selected region
    selected_region.center_lat = current_lat;
    selected_region.center_lon = current_lon;
    
    // Update zoom
    globe_state.zoom = current_zoom;
    
    // Calculate target globe rotation to show the location in front of the camera
    let lat_rad = (current_lat as f32).to_radians();
    let lon_rad = (current_lon as f32).to_radians();
    
    // Camera is at (0, 0, 5) looking at origin (0, 0, 0)
    // To bring a location to face the camera (positive Z direction):
    // - We need to rotate the globe so the target location points toward +Z
    // - longitude rotation: negative rotation around Y brings eastern locations to front
    // - latitude rotation: negative rotation around X brings northern locations to front
    globe_state.rotation_y = -lon_rad; // Negative to bring longitude to face camera
    globe_state.rotation_x = lat_rad;   // Positive to bring latitude to face camera
    
    // Clamp rotation_x to prevent excessive flipping
    globe_state.rotation_x = globe_state.rotation_x.clamp(-1.5, 1.5);
    
    info!("🌍 ROTATION DEBUG: Rotating globe for location ({:.4}°, {:.4}°) → Y: {:.3}, X: {:.3}", 
          current_lat, current_lon, globe_state.rotation_y, globe_state.rotation_x);
    
    // Check if animation is complete
    if progress >= 1.0 {
        search_state.is_animating = false;
        selected_region.center_lat = search_state.target_lat;
        selected_region.center_lon = search_state.target_lon;
        globe_state.zoom = search_state.target_zoom;
        info!("Animation completed. Arrived at: {:.4}, {:.4}", search_state.target_lat, search_state.target_lon);
    }
}

// Fallback system to use procedural textures if real textures fail to load
fn fallback_to_procedural_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    earth_textures: Res<EarthTextures>,
    asset_server: Res<AssetServer>,
) {
    // Only run this once when we detect a loading failure
    if earth_textures.loading_complete {
        return; // Already loaded successfully or fallback already applied
    }
    
    // Check if the real texture failed to load
    if let Some(ref handle) = earth_textures.daymap {
        match asset_server.get_load_state(handle) {
            Some(bevy::asset::LoadState::Failed(_)) => {
                warn!("Real Earth texture failed to load, falling back to procedural textures");
                crate::procedural_textures::create_placeholder_earth_textures(commands, images);
            }
            _ => {
                // Still loading or loaded successfully, do nothing
            }
        }
    }
}

/// Creates a UV-mapped sphere mesh optimized for Earth textures to prevent seams
fn create_earth_sphere_mesh(radius: f32, longitude_segments: u32, latitude_segments: u32) -> Mesh {
    use std::f32::consts::PI;
    
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    
    // Generate vertices
    for lat in 0..=latitude_segments {
        let theta = lat as f32 * PI / latitude_segments as f32;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();
        
        for lon in 0..=longitude_segments {
            let phi = lon as f32 * 2.0 * PI / longitude_segments as f32;
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();
            
            // Position on sphere
            let x = radius * sin_theta * cos_phi;
            let y = radius * cos_theta;
            let z = radius * sin_theta * sin_phi;
            
            positions.push([x, y, z]);
            normals.push([x / radius, y / radius, z / radius]);
            
            // UV coordinates - critical for preventing seams
            let u = lon as f32 / longitude_segments as f32;
            let v = lat as f32 / latitude_segments as f32;
            uvs.push([u, v]);
        }
    }
    
    // Generate triangular indices
    for lat in 0..latitude_segments {
        for lon in 0..longitude_segments {
            let first = lat * (longitude_segments + 1) + lon;
            let second = first + longitude_segments + 1;
            
            // First triangle
            indices.push(first);
            indices.push(second);
            indices.push(first + 1);
            
            // Second triangle
            indices.push(second);
            indices.push(second + 1);
            indices.push(first + 1);
        }
    }
    
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD | bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD
    );
    
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    
    mesh
}

/// Creates a coordinate grid on the globe surface for debugging lat/lon mapping
fn create_coordinate_grid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    use std::f32::consts::PI;
    
    // Create material for grid lines
    let grid_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 0.0, 0.7), // Semi-transparent yellow
        alpha_mode: AlphaMode::Blend,
        unlit: true, // Don't be affected by lighting
        ..default()
    });
    
    let sphere_radius = 1.02; // Slightly larger than Earth to avoid z-fighting
    
    // Create latitude lines (horizontal circles)
    for lat_deg in (-90..=90).step_by(30) {
        if lat_deg == 0 { continue; } // Skip equator for now
        
        let lat_rad = (lat_deg as f32).to_radians();
        let mut positions = Vec::new();
        
        // Create circle at this latitude
        for lon_deg in (0..360).step_by(5) {
            let lon_rad = (lon_deg as f32).to_radians();
            
            let x = lat_rad.cos() * lon_rad.sin() * sphere_radius;
            let y = lat_rad.sin() * sphere_radius;
            let z = -lat_rad.cos() * lon_rad.cos() * sphere_radius;
            
            positions.push([x, y, z]);
        }
        
        // Create line mesh
        if positions.len() > 1 {
            let mut mesh = Mesh::new(
                bevy::render::mesh::PrimitiveTopology::LineStrip,
                bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD | bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            
            commands.spawn(PbrBundle {
                mesh: meshes.add(mesh),
                material: grid_material.clone(),
                ..default()
            });
        }
    }
    
    // Create longitude lines (vertical meridians)
    for lon_deg in (0..360).step_by(30) {
        let mut positions = Vec::new();
        
        // Create meridian from north to south pole
        for lat_deg in (-90..=90).step_by(5) {
            let lat_rad = (lat_deg as f32).to_radians();
            let lon_rad = (lon_deg as f32).to_radians();
            
            let x = lat_rad.cos() * lon_rad.sin() * sphere_radius;
            let y = lat_rad.sin() * sphere_radius;
            let z = -lat_rad.cos() * lon_rad.cos() * sphere_radius;
            
            positions.push([x, y, z]);
        }
        
        // Create line mesh
        if positions.len() > 1 {
            let mut mesh = Mesh::new(
                bevy::render::mesh::PrimitiveTopology::LineStrip,
                bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD | bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            
            commands.spawn(PbrBundle {
                mesh: meshes.add(mesh),
                material: grid_material.clone(),
                ..default()
            });
        }
    }
    
    // Create equator line in red for easy reference
    let equator_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.0, 0.0, 0.9), // Semi-transparent red
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    
    let mut equator_positions = Vec::new();
    for lon_deg in (0..360).step_by(2) {
        let lon_rad = (lon_deg as f32).to_radians();
        let x = lon_rad.sin() * sphere_radius;
        let y = 0.0; // Equator
        let z = -lon_rad.cos() * sphere_radius;
        equator_positions.push([x, y, z]);
    }
    
    let mut equator_mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::LineStrip,
        bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD | bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD
    );
    equator_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, equator_positions);
    
    commands.spawn(PbrBundle {
        mesh: meshes.add(equator_mesh),
        material: equator_material,
        ..default()
    });
}
