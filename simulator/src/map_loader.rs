use anyhow::Result;
use bevy::math::primitives::{Cone, Cylinder};
use bevy::prelude::*;
use bevy::tasks::{
    futures_lite::future::{block_on, poll_once},
    IoTaskPool, Task,
};
use rand::Rng;
use std::f32::consts::TAU;

use crate::app_state::{AppMode, DataLoadingState, SelectedRegion};
use crate::components::{MapFeature, MapFeatureType, MapRoot};
use crate::osm::{
    fetch_osm_data, lonlat_to_local, LineKind, MapLine, MapPoint, MapPolygon, OsmMapData,
    PointKind, PolygonKind,
};

const FETCH_RADIUS_METERS: f64 = 5_000.0;

/// Plugin to load OpenStreetMap data for the current region and render it.
pub struct MapLoaderPlugin;

impl Plugin for MapLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OsmFetchTask>()
            .init_resource::<MapAssets>()
            .add_event::<WorldLoadedEvent>()
            .add_systems(Startup, initialize_map_assets)
            .add_systems(OnEnter(AppMode::Simulation3D), begin_osm_fetch)
            .add_systems(
                Update,
                poll_osm_fetch.run_if(in_state(AppMode::Simulation3D)),
            );
    }
}

#[derive(Resource, Default)]
struct OsmFetchTask {
    task: Option<Task<Result<OsmMapData>>>,
}

#[derive(Event, Clone, Debug)]
pub struct WorldLoadedEvent {
    pub map_data: OsmMapData,
}

fn begin_osm_fetch(
    mut commands: Commands,
    mut fetch_task: ResMut<OsmFetchTask>,
    selected_region: Res<SelectedRegion>,
    mut loading_state: ResMut<DataLoadingState>,
) {
    let lat = selected_region.center_lat;
    let lon = selected_region.center_lon;

    loading_state.is_loading = true;
    loading_state.progress = 0.1;
    loading_state.status_message = format!("Loading world data around ({:.4}, {:.4})", lat, lon);

    let task_pool = IoTaskPool::get();
    fetch_task.task =
        Some(task_pool.spawn(async move { fetch_osm_data(lat, lon, FETCH_RADIUS_METERS).await }));

    // Remove any leftover world geometry while we fetch fresh data
    commands.insert_resource(PendingMapCleanup);
}

#[derive(Resource)]
struct PendingMapCleanup;

fn poll_osm_fetch(
    mut commands: Commands,
    mut fetch_task: ResMut<OsmFetchTask>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    map_assets: Res<MapAssets>,
    mut loading_state: ResMut<DataLoadingState>,
    mut map_root_query: Query<Entity, With<MapRoot>>,
    mut world_loaded_events: EventWriter<WorldLoadedEvent>,
    pending_cleanup: Option<ResMut<PendingMapCleanup>>,
) {
    let Some(task) = fetch_task.task.as_mut() else {
        return;
    };

    if let Some(result) = block_on(poll_once(task)) {
        fetch_task.task = None;

        // Ensure map roots are cleared exactly once when fetch completes
        if pending_cleanup.is_some() {
            let mut to_despawn = Vec::new();
            for entity in map_root_query.iter_mut() {
                to_despawn.push(entity);
            }
            for entity in to_despawn {
                commands.entity(entity).despawn_recursive();
            }
            commands.remove_resource::<PendingMapCleanup>();
        }

        match result {
            Ok(map_data) => {
                loading_state.status_message = "Generating terrain and world assets".to_string();
                loading_state.progress = 0.6;

                let world_root = commands.spawn((SpatialBundle::default(), MapRoot)).id();

                spawn_polygons(
                    world_root,
                    &map_data.polygons,
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &map_assets,
                    map_data.center_lat as f32,
                    map_data.center_lon as f32,
                );
                spawn_lines(
                    world_root,
                    &map_data.lines,
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    map_data.center_lat as f32,
                    map_data.center_lon as f32,
                );
                spawn_points(
                    world_root,
                    &map_data.points,
                    &mut commands,
                    &map_assets,
                    map_data.center_lat as f32,
                    map_data.center_lon as f32,
                );

                loading_state.status_message = "World ready".to_string();
                loading_state.progress = 1.0;
                loading_state.is_loading = false;

                world_loaded_events.send(WorldLoadedEvent { map_data });
            }
            Err(err) => {
                error!("Failed to load OSM data: {err:#}");
                loading_state.status_message = format!("Failed to load world: {err}");
                loading_state.progress = 0.0;
                loading_state.is_loading = false;
            }
        }
    } else {
        // Update progress indicator slowly while waiting
        if loading_state.progress < 0.5 {
            loading_state.progress += 0.02f32.min(0.5 - loading_state.progress);
        }
    }
}

fn spawn_polygons(
    root: Entity,
    polygons: &[MapPolygon],
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    map_assets: &MapAssets,
    center_lat: f32,
    center_lon: f32,
) {
    for polygon in polygons.iter() {
        if polygon.coordinates.len() < 3 {
            continue;
        }

        // Convert lat/lon into local XZ positions (meters)
        let points: Vec<Vec2> = polygon
            .coordinates
            .iter()
            .map(|coord| lonlat_to_local(center_lat, center_lon, coord[0], coord[1]))
            .collect();

        let Some((center, extents)) = compute_center_and_extents(&points) else {
            continue;
        };

        let (height, color, feature_type) = match &polygon.kind {
            PolygonKind::Building(attrs) => {
                let levels = attrs.levels.unwrap_or(3.0);
                let explicit_height = attrs.height_m.unwrap_or(levels * 3.3);
                (
                    explicit_height.max(6.0),
                    Color::srgb(0.7, 0.7, 0.75),
                    MapFeatureType::Building,
                )
            }
            PolygonKind::Farmland => (0.1, Color::srgb(0.6, 0.5, 0.3), MapFeatureType::Farmland),
            PolygonKind::Park => (0.1, Color::srgb(0.3, 0.6, 0.3), MapFeatureType::Park),
            PolygonKind::Water => (0.1, Color::srgb(0.1, 0.3, 0.6), MapFeatureType::Water),
            PolygonKind::Forest => (0.1, Color::srgb(0.15, 0.35, 0.18), MapFeatureType::Forest),
            PolygonKind::Other(_) => (0.1, Color::srgb(0.5, 0.5, 0.5), MapFeatureType::Generic),
        };

        let mesh = if height <= 0.2 {
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(extents.x.max(2.0), extents.y.max(2.0)),
            )
        } else {
            meshes.add(Cuboid::new(extents.x.max(2.0), height, extents.y.max(2.0)))
        };

        let translation = if height <= 0.2 {
            Vec3::new(center.x, 0.02, center.y)
        } else {
            Vec3::new(center.x, height / 2.0, center.y)
        };

        let entity = commands
            .spawn(PbrBundle {
                mesh,
                material: materials.add(StandardMaterial {
                    base_color: color,
                    perceptual_roughness: 0.8,
                    reflectance: 0.02,
                    ..default()
                }),
                transform: Transform::from_translation(translation),
                ..default()
            })
            .insert(MapFeature { feature_type })
            .id();

        commands.entity(root).add_child(entity);

        if matches!(&polygon.kind, PolygonKind::Farmland) {
            spawn_farmland_rows(commands, entity, map_assets, extents);
        }
    }
}

fn spawn_lines(
    root: Entity,
    lines: &[MapLine],
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    center_lat: f32,
    center_lon: f32,
) {
    for line in lines.iter() {
        if line.coordinates.len() < 2 {
            continue;
        }

        let width = match &line.kind {
            LineKind::Road(classification) => road_width_for(classification),
            LineKind::Other(_) => 4.0,
        };

        let color = match &line.kind {
            LineKind::Road(classification) => road_color_for(classification),
            LineKind::Other(_) => Color::srgb(0.8, 0.8, 0.8),
        };

        let coords: Vec<Vec2> = line
            .coordinates
            .iter()
            .map(|coord| lonlat_to_local(center_lat, center_lon, coord[0], coord[1]))
            .collect();

        for window in coords.windows(2) {
            let start = window[0];
            let end = window[1];

            let segment_vec = end - start;
            let length = segment_vec.length();
            if length < 1.0 {
                continue;
            }

            let mid = (start + end) * 0.5;
            let angle = segment_vec.y.atan2(segment_vec.x);

            let road_entity = commands
                .spawn(PbrBundle {
                    mesh: meshes.add(Cuboid::new(length, 0.15, width)),
                    material: materials.add(StandardMaterial {
                        base_color: color,
                        perceptual_roughness: 0.9,
                        ..default()
                    }),
                    transform: Transform {
                        translation: Vec3::new(mid.x, 0.08, mid.y),
                        rotation: Quat::from_rotation_y(-angle),
                        ..default()
                    },
                    ..default()
                })
                .insert(MapFeature {
                    feature_type: MapFeatureType::Road,
                })
                .id();

            commands.entity(root).add_child(road_entity);
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
    let extents = max - min;
    Some((center, extents.abs()))
}

fn spawn_points(
    root: Entity,
    points: &[MapPoint],
    commands: &mut Commands,
    map_assets: &MapAssets,
    center_lat: f32,
    center_lon: f32,
) {
    let Some(trunk_mesh) = map_assets.tree_trunk_mesh.clone() else {
        return;
    };
    let Some(canopy_mesh) = map_assets.tree_canopy_mesh.clone() else {
        return;
    };
    let Some(trunk_material) = map_assets.tree_trunk_material.clone() else {
        return;
    };
    let Some(canopy_material) = map_assets.tree_canopy_material.clone() else {
        return;
    };

    let max_instances = 400usize;
    let step = if points.len() > max_instances {
        (points.len() / max_instances).max(1)
    } else {
        1
    };

    let mut rng = rand::thread_rng();
    const TRUNK_HEIGHT: f32 = 2.5;
    const CANOPY_HEIGHT: f32 = 3.2;

    for (index, point) in points.iter().enumerate().step_by(step) {
        if !matches!(point.kind, PointKind::Tree) {
            continue;
        }

        let local = lonlat_to_local(center_lat, center_lon, point.coordinate[0], point.coordinate[1]);
        let yaw = rng.gen::<f32>() * TAU;

        let tree_entity = commands
            .spawn(SpatialBundle {
                transform: Transform {
                    translation: Vec3::new(local.x, 0.0, local.y),
                    rotation: Quat::from_rotation_y(yaw),
                    ..default()
                },
                ..default()
            })
            .insert(MapFeature {
                feature_type: MapFeatureType::Tree,
            })
            .with_children(|builder| {
                builder.spawn(PbrBundle {
                    mesh: trunk_mesh.clone(),
                    material: trunk_material.clone(),
                    transform: Transform::from_translation(Vec3::new(0.0, TRUNK_HEIGHT * 0.5, 0.0)),
                    ..default()
                });
                builder.spawn(PbrBundle {
                    mesh: canopy_mesh.clone(),
                    material: canopy_material.clone(),
                    transform: Transform::from_translation(Vec3::new(
                        0.0,
                        TRUNK_HEIGHT + CANOPY_HEIGHT * 0.5,
                        0.0,
                    )),
                    ..default()
                });
            })
            .id();

        commands.entity(root).add_child(tree_entity);

        // Throttle the tree count even further for extremely dense areas
        if index > max_instances && step == 1 {
            break;
        }
    }
}

fn spawn_farmland_rows(
    commands: &mut Commands,
    parent: Entity,
    map_assets: &MapAssets,
    extents: Vec2,
) {
    if extents.x.min(extents.y) < 6.0 {
        return;
    }

    let Some(row_mesh) = map_assets.farmland_row_mesh.clone() else {
        return;
    };
    let Some(row_material) = map_assets.farmland_row_material.clone() else {
        return;
    };

    let major_is_x = extents.x >= extents.y;
    let (major_length, minor_length) = if major_is_x {
        (extents.x, extents.y)
    } else {
        (extents.y, extents.x)
    };

    let target_row_width = 4.0;
    let mut row_count = (minor_length / target_row_width).floor() as i32;
    row_count = row_count.clamp(3, 18);
    if row_count <= 0 {
        row_count = 3;
    }

    let row_spacing = minor_length / row_count as f32;

    commands.entity(parent).with_children(|builder| {
        for i in 0..row_count {
            let offset = -minor_length / 2.0 + (i as f32 + 0.5) * row_spacing;
            let (translation, scale) = if major_is_x {
                (
                    Vec3::new(0.0, 0.06, offset),
                    Vec3::new(major_length.max(6.0), 0.2, (row_spacing * 0.6).max(1.0)),
                )
            } else {
                (
                    Vec3::new(offset, 0.06, 0.0),
                    Vec3::new((row_spacing * 0.6).max(1.0), 0.2, major_length.max(6.0)),
                )
            };

            builder.spawn(PbrBundle {
                mesh: row_mesh.clone(),
                material: row_material.clone(),
                transform: Transform::from_translation(translation).with_scale(scale),
                ..default()
            });
        }
    });
}

#[derive(Resource, Default)]
struct MapAssets {
    initialized: bool,
    tree_trunk_mesh: Option<Handle<Mesh>>,
    tree_canopy_mesh: Option<Handle<Mesh>>,
    tree_trunk_material: Option<Handle<StandardMaterial>>,
    tree_canopy_material: Option<Handle<StandardMaterial>>,
    farmland_row_mesh: Option<Handle<Mesh>>,
    farmland_row_material: Option<Handle<StandardMaterial>>,
}

fn initialize_map_assets(
    mut map_assets: ResMut<MapAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if map_assets.initialized {
        return;
    }

    let trunk_mesh = meshes.add(Cylinder::new(0.25, 2.5));
    let canopy_mesh = meshes.add(Cone {
        radius: 2.2,
        height: 3.2,
    });
    let row_mesh = meshes.add(Cuboid::new(1.0, 0.1, 1.0));

    let trunk_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.2, 0.1),
        perceptual_roughness: 0.9,
        reflectance: 0.02,
        ..default()
    });
    let canopy_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.45, 0.12),
        perceptual_roughness: 0.6,
        metallic: 0.0,
        ..default()
    });
    let row_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.36, 0.2),
        perceptual_roughness: 0.8,
        reflectance: 0.03,
        ..default()
    });

    map_assets.tree_trunk_mesh = Some(trunk_mesh);
    map_assets.tree_canopy_mesh = Some(canopy_mesh);
    map_assets.tree_trunk_material = Some(trunk_material);
    map_assets.tree_canopy_material = Some(canopy_material);
    map_assets.farmland_row_mesh = Some(row_mesh);
    map_assets.farmland_row_material = Some(row_material);
    map_assets.initialized = true;
}

fn road_width_for(classification: &str) -> f32 {
    match classification {
        "motorway" | "trunk" => 18.0,
        "primary" => 14.0,
        "secondary" => 10.0,
        "tertiary" => 8.0,
        "residential" | "service" => 6.0,
        "track" | "path" | "footway" => 3.0,
        _ => 5.0,
    }
}

fn road_color_for(classification: &str) -> Color {
    match classification {
        "motorway" | "trunk" => Color::srgb(0.6, 0.6, 0.6),
        "primary" | "secondary" => Color::srgb(0.7, 0.7, 0.7),
        "tertiary" | "residential" => Color::srgb(0.8, 0.8, 0.8),
        "track" | "path" | "footway" => Color::srgb(0.5, 0.4, 0.3),
        _ => Color::srgb(0.75, 0.75, 0.75),
    }
}
