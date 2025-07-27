use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use crate::app_state::{AppMode, SelectedRegion};

pub struct GlobePlugin;

impl Plugin for GlobePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppMode::Globe), setup_globe)
            .add_systems(OnExit(AppMode::Globe), cleanup_globe)
            .add_systems(Update, (
                rotate_globe,
                zoom_globe,
                handle_globe_click,
                update_globe_camera,
            ).run_if(in_state(AppMode::Globe)));
    }
}

#[derive(Component)]
struct Globe;

#[derive(Component)]
struct GlobeCamera;

#[derive(Resource)]
struct GlobeState {
    rotation_x: f32,
    rotation_y: f32,
    zoom: f32,
    is_dragging: bool,
}

impl Default for GlobeState {
    fn default() -> Self {
        Self {
            rotation_x: 0.0,
            rotation_y: 0.0,
            zoom: 5.0, // Distance from globe
            is_dragging: false,
        }
    }
}

fn setup_globe(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("Setting up globe view");
    
    // Initialize globe state
    commands.insert_resource(GlobeState::default());
    
    // Create Earth sphere with a procedural texture for now
    let earth_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.5, 0.8), // Ocean blue
        metallic: 0.0,
        perceptual_roughness: 0.8,
        // TODO: Add actual Earth texture
        // base_color_texture: Some(asset_server.load("textures/earth_daymap.jpg")),
        ..default()
    });
    
    // Spawn the globe
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap()),
            material: earth_material,
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        Globe,
    ));
    
    // Create globe camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 5.0)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        GlobeCamera,
    ));
    
    // Add lighting for the globe
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::WHITE,
            illuminance: 5000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(3.0, 3.0, 3.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    
    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.3,
    });
}

fn cleanup_globe(
    mut commands: Commands,
    globe_query: Query<Entity, With<Globe>>,
    camera_query: Query<Entity, With<GlobeCamera>>,
) {
    info!("Cleaning up globe view");
    
    // Remove globe entities
    for entity in globe_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    // Remove globe camera
    for entity in camera_query.iter() {
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
