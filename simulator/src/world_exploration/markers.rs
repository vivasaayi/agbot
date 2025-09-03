use bevy::prelude::*;
use crate::world_exploration::WorldLocation;

pub struct MarkersPlugin;

impl Plugin for MarkersPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, (
                update_markers,
                animate_markers,
            ));
    }
}

#[derive(Component)]
pub struct WorldMarker {
    pub location: WorldLocation,
    pub pulse_timer: f32,
}

#[derive(Component)]
pub struct MarkerAnimation {
    pub scale_factor: f32,
    pub pulse_speed: f32,
}

#[derive(Bundle)]
pub struct WorldMarkerBundle {
    pub marker: WorldMarker,
    pub animation: MarkerAnimation,
    pub spatial: SpatialBundle,
    pub name: Name,
}

impl WorldMarkerBundle {
    pub fn new(location: WorldLocation, position: Vec3) -> Self {
        Self {
            marker: WorldMarker {
                location: location.clone(),
                pulse_timer: 0.0,
            },
            animation: MarkerAnimation {
                scale_factor: 1.0,
                pulse_speed: 2.0,
            },
            spatial: SpatialBundle {
                transform: Transform::from_translation(position),
                ..default()
            },
            name: Name::new(format!("Marker_{}", location.name)),
        }
    }
}

/// Convert latitude/longitude to 3D position on a sphere
pub fn lat_lon_to_sphere_position(latitude: f64, longitude: f64, radius: f32) -> Vec3 {
    let lat_rad = latitude.to_radians() as f32;
    let lon_rad = longitude.to_radians() as f32;
    
    let x = radius * lat_rad.cos() * lon_rad.cos();
    let y = radius * lat_rad.sin();
    let z = radius * lat_rad.cos() * lon_rad.sin();
    
    Vec3::new(x, y, z)
}

fn update_markers(
    time: Res<Time>,
    mut marker_query: Query<&mut WorldMarker>,
) {
    for mut marker in marker_query.iter_mut() {
        marker.pulse_timer += time.delta_seconds();
    }
}

fn animate_markers(
    time: Res<Time>,
    mut marker_query: Query<(&mut Transform, &mut MarkerAnimation, &WorldMarker)>,
) {
    for (mut transform, mut animation, marker) in marker_query.iter_mut() {
        // Pulsing animation
        let pulse = (marker.pulse_timer * animation.pulse_speed).sin() * 0.2 + 1.0;
        animation.scale_factor = pulse;
        
        // Apply scale animation
        transform.scale = Vec3::splat(animation.scale_factor * 0.1); // Base scale
        
        // Always face camera (billboard effect)
        // We'll implement this when we have camera reference
    }
}

/// Spawn a marker at the given world location
pub fn spawn_marker(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    location: WorldLocation,
    globe_radius: f32,
) {
    let position = lat_lon_to_sphere_position(location.latitude, location.longitude, globe_radius * 1.05);
    
    // Create marker mesh (sphere)
    let marker_mesh = meshes.add(Sphere::new(0.05));
    let marker_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.2, 0.2), // Red marker
        emissive: Color::srgb(0.5, 0.1, 0.1).into(),
        ..default()
    });
    
    commands.spawn((
        PbrBundle {
            mesh: marker_mesh,
            material: marker_material,
            transform: Transform::from_translation(position),
            ..default()
        },
        WorldMarkerBundle::new(location, position),
    ));
}

/// Remove all markers from the scene
pub fn clear_all_markers(
    commands: &mut Commands,
    marker_query: Query<Entity, With<WorldMarker>>,
) {
    for entity in marker_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
