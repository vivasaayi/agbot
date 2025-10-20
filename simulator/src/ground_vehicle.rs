use bevy::prelude::*;

use crate::app_state::AppMode;
use crate::components::Tractor;
use crate::map_loader::WorldLoadedEvent;
use crate::osm::{lonlat_to_local, PolygonKind};

/// Plugin that manages autonomous ground vehicles like tractors.
pub struct GroundVehiclePlugin;

impl Plugin for GroundVehiclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_tractors_on_world_load, drive_tractors).run_if(in_state(AppMode::Simulation3D)),
        );
    }
}

fn spawn_tractors_on_world_load(
    mut commands: Commands,
    mut events: EventReader<WorldLoadedEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for event in events.read() {
        // Collect farmland polygons and convert them to local coordinates
        let center_lat = event.map_data.center_lat as f32;
        let center_lon = event.map_data.center_lon as f32;

        let mut farmland_bounds: Vec<(Vec2, Vec2)> = event
            .map_data
            .polygons
            .iter()
            .filter_map(|polygon| {
                if !matches!(polygon.kind, PolygonKind::Farmland) {
                    return None;
                }
                let points: Vec<Vec2> = polygon
                    .coordinates
                    .iter()
                    .map(|coord| lonlat_to_local(center_lat, center_lon, coord[0], coord[1]))
                    .collect();
                compute_center_and_extents(&points)
            })
            .collect();

        // If there is no farmland, fall back to a single generic field around the origin
        if farmland_bounds.is_empty() {
            farmland_bounds.push((Vec2::ZERO, Vec2::new(60.0, 60.0)));
        }

        for (idx, (center, extents)) in farmland_bounds.into_iter().enumerate() {
            if idx > 1 {
                // Limit to two tractors to avoid overcrowding the scene
                break;
            }

            let half_extents = extents * 0.5;
            let track_height = 0.6;

            // Create a rectangular patrol path following the field perimeter
            let mut waypoints = vec![
                Vec3::new(
                    center.x - half_extents.x,
                    track_height,
                    center.y - half_extents.y,
                ),
                Vec3::new(
                    center.x + half_extents.x,
                    track_height,
                    center.y - half_extents.y,
                ),
                Vec3::new(
                    center.x + half_extents.x,
                    track_height,
                    center.y + half_extents.y,
                ),
                Vec3::new(
                    center.x - half_extents.x,
                    track_height,
                    center.y + half_extents.y,
                ),
            ];

            // Ensure waypoints are not degenerate
            if waypoints[0].distance(waypoints[1]) < 5.0 {
                let padding = 10.0;
                waypoints = vec![
                    Vec3::new(center.x - padding, track_height, center.y - padding),
                    Vec3::new(center.x + padding, track_height, center.y - padding),
                    Vec3::new(center.x + padding, track_height, center.y + padding),
                    Vec3::new(center.x - padding, track_height, center.y + padding),
                ];
            }

            let mesh = meshes.add(Cuboid::new(2.5, 1.8, 4.0));
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.25, 0.1),
                metallic: 0.1,
                perceptual_roughness: 0.6,
                ..default()
            });

            let starting_position =
                waypoints
                    .first()
                    .copied()
                    .unwrap_or(Vec3::new(center.x, track_height, center.y));

            let next_index = if waypoints.len() > 1 { 1 } else { 0 };

            commands.spawn((
                PbrBundle {
                    mesh,
                    material,
                    transform: Transform::from_translation(starting_position),
                    ..default()
                },
                Tractor {
                    speed: 6.0,
                    waypoints: waypoints.clone(),
                    current_index: next_index,
                },
                Name::new(format!("Autonomous Tractor #{idx}")),
            ));
        }
    }
}

fn drive_tractors(mut query: Query<(&mut Transform, &mut Tractor)>, time: Res<Time>) {
    for (mut transform, mut tractor) in query.iter_mut() {
        let Some(target) = tractor.current_target() else {
            continue;
        };

        let mut flat_target = target;
        flat_target.y = transform.translation.y;

        let delta = flat_target - transform.translation;
        let distance = delta.length();
        if distance < 1.0 {
            tractor.advance_waypoint();
            continue;
        }

        let direction = delta / distance.max(0.0001);
        transform.translation += direction * tractor.speed * time.delta_seconds();
        transform.translation.y = flat_target.y; // maintain surface contact

        let forward = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
        if forward.length_squared() > 0.0 {
            transform.look_to(forward, Vec3::Y);
        }
    }
}

fn compute_center_and_extents(points: &[Vec2]) -> Option<(Vec2, Vec2)> {
    if points.is_empty() {
        return None;
    }

    let mut min = points[0];
    let mut max = points[0];
    for point in points.iter().skip(1) {
        min = min.min(*point);
        max = max.max(*point);
    }

    let center = (min + max) * 0.5;
    let extents = (max - min).abs();
    Some((center, extents.max(Vec2::splat(10.0))))
}
