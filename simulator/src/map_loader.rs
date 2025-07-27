use bevy::prelude::*;
use geojson::{GeoJson, Value};
use std::fs;
use crate::app_state::AppMode;

/// Plugin to load and spawn map features from a GeoJSON file
pub struct MapLoaderPlugin;

impl Plugin for MapLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppMode::Simulation3D), load_and_spawn_map_data);
    }
}

fn load_and_spawn_map_data(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let geojson_path = "assets/export.geojson";
    let geojson_str = match fs::read_to_string(geojson_path) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read {}: {}", geojson_path, e);
            return;
        }
    };
    let geojson = match geojson_str.parse::<GeoJson>() {
        Ok(gj) => gj,
        Err(e) => {
            error!("Failed to parse GeoJSON: {}", e);
            return;
        }
    };

    if let GeoJson::FeatureCollection(collection) = geojson {
        // Gather all points for auto-centering
        let mut all_points: Vec<Vec2> = Vec::new();
        for feature in &collection.features {
            if let Some(geometry) = &feature.geometry {
                match &geometry.value {
                    Value::Polygon(polygon) => {
                        if let Some(first_ring) = polygon.first() {
                            for coord in first_ring {
                                all_points.push(Vec2::new(coord[0] as f32, coord[1] as f32));
                            }
                        }
                    }
                    Value::LineString(line) => {
                        for coord in line {
                            all_points.push(Vec2::new(coord[0] as f32, coord[1] as f32));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Compute map centroid for auto-centering
        let map_center = if !all_points.is_empty() {
            all_points.iter().fold(Vec2::ZERO, |acc, p| acc + *p) / all_points.len() as f32
        } else {
            return; // No points to render
        };
        info!("Map centroid (lon/lat): ({}, {})", map_center.x, map_center.y);

        // --- Real-world scaling setup ---
        let lat0_rad = map_center.y.to_radians();
        const METERS_PER_DEGREE_LAT: f32 = 111_320.0; // Constant for latitude
        let meters_per_degree_lon = METERS_PER_DEGREE_LAT * lat0_rad.cos();

        // Helper closure to convert a single lon/lat point to world coordinates (meters)
        let to_meters = |lon: f32, lat: f32| -> Vec2 {
            Vec2::new(
                (lon - map_center.x) * meters_per_degree_lon,
                (lat - map_center.y) * METERS_PER_DEGREE_LAT,
            )
        };

        // --- Loop through features and spawn them ---
        for feature in collection.features {
            if let Some(geometry) = feature.geometry {
                match geometry.value {
                    Value::Polygon(polygon) => {
                        if let Some(first_ring) = polygon.first() {
                            // Convert the entire polygon footprint to meters
                            let points_m: Vec<Vec2> = first_ring.iter().map(|coord| {
                                to_meters(coord[0] as f32, coord[1] as f32)
                            }).collect();

                            if points_m.len() < 3 { continue; }

                            // Calculate the bounding box of the building in meters
                            let mut min = points_m[0];
                            let mut max = points_m[0];
                            for point in points_m.iter().skip(1) {
                                min = min.min(*point);
                                max = max.max(*point);
                            }

                            let center_m = (min + max) / 2.0;
                            let size_m = max - min;
                            let height = 15.0; // Default building height

                            info!("Spawning Building at ({}, {}) with size ({}, {})", center_m.x, center_m.y, size_m.x, size_m.y);

                            commands.spawn(PbrBundle {
                                mesh: meshes.add(Cuboid::new(size_m.x.max(0.1), height, size_m.y.max(0.1))),
                                material: materials.add(StandardMaterial {
                                    base_color: Color::srgb(0.8, 0.1, 0.1), // Bright red
                                    ..default()
                                }),
                                transform: Transform::from_xyz(center_m.x, height / 2.0, center_m.y),
                                ..default()
                            });
                        }
                    }
                    Value::LineString(line) => {
                        for window in line.windows(2) {
                            let start_deg = Vec2::new(window[0][0] as f32, window[0][1] as f32);
                            let end_deg = Vec2::new(window[1][0] as f32, window[1][1] as f32);

                            // Convert segment points to meters
                            let start_m = to_meters(start_deg.x, start_deg.y);
                            let end_m = to_meters(end_deg.x, end_deg.y);

                            let mid_m = (start_m + end_m) / 2.0;
                            let segment_vec = end_m - start_m;
                            let length_m = segment_vec.length();
                            let road_width = 5.0; // Assume 5 meters wide roads

                            if length_m < 0.1 { continue; } // Skip tiny segments

                            // Angle of the road segment in the XZ plane
                            let angle = segment_vec.y.atan2(segment_vec.x);

                            info!("Spawning Road segment at ({}, {}) with length {}", mid_m.x, mid_m.y, length_m);

                            commands.spawn(PbrBundle {
                                mesh: meshes.add(Cuboid::new(length_m, 0.2, road_width)),
                                material: materials.add(StandardMaterial {
                                    base_color: Color::srgb(0.1, 0.1, 0.8), // Bright blue
                                    ..default()
                                }),
                                transform: Transform {
                                    translation: Vec3::new(mid_m.x, 0.1, mid_m.y),
                                    rotation: Quat::from_rotation_y(angle),
                                    ..default()
                                },
                                ..default()
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
