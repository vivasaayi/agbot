use crate::state::{
    assert_manifest_layer_placement, CursorMapState, MapCamera, MapViewState, SceneExtent,
    SceneManifestState, TileId, TileRenderState, ViewerState, MAP_UNITS_PER_DEGREE,
};
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
};
use shared::schemas::FieldRecord;
use std::collections::BTreeSet;

pub struct ViewerMapPlugin;

impl Plugin for ViewerMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera).add_systems(
            Update,
            (
                sync_map_camera,
                update_cursor_map_state,
                render_field_boundary,
            ),
        );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2dBundle::default(), MapCamera));
}

pub fn sync_map_camera(
    windows: Query<&Window>,
    input: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut viewer_state: ResMut<ViewerState>,
    tile_state: Res<TileRenderState>,
    mut map_view: ResMut<MapViewState>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<MapCamera>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((mut transform, mut projection)) = camera_query.get_single_mut() else {
        return;
    };

    if map_view.needs_fit
        && tile_state.world_dimensions.x > 0.0
        && tile_state.world_dimensions.y > 0.0
    {
        map_view.base_scale = (tile_state.world_dimensions.x / window.width())
            .max(tile_state.world_dimensions.y / window.height())
            * 1.1;
        map_view.center = Vec2::ZERO;
        map_view.needs_fit = false;
    }

    let scroll_delta = mouse_wheel_events.read().map(|event| event.y).sum::<f32>();
    if scroll_delta.abs() > f32::EPSILON {
        viewer_state.zoom_level =
            (viewer_state.zoom_level * 1.15_f32.powf(scroll_delta)).clamp(0.2, 5.0);
    }

    let camera_scale = map_view.base_scale / viewer_state.zoom_level.max(0.2);
    let drag_delta = mouse_motion_events
        .read()
        .fold(Vec2::ZERO, |acc, event| acc + event.delta);
    if mouse_buttons.pressed(MouseButton::Middle) {
        map_view.center.x -= drag_delta.x * camera_scale;
        map_view.center.y -= drag_delta.y * camera_scale;
    }

    let pan_speed = (map_view.base_scale.max(0.0001) / viewer_state.zoom_level.max(0.2))
        * 500.0
        * time.delta_seconds();
    if input.pressed(KeyCode::ArrowLeft) || input.pressed(KeyCode::KeyA) {
        map_view.center.x -= pan_speed;
    }
    if input.pressed(KeyCode::ArrowRight) || input.pressed(KeyCode::KeyD) {
        map_view.center.x += pan_speed;
    }
    if input.pressed(KeyCode::ArrowUp) || input.pressed(KeyCode::KeyW) {
        map_view.center.y += pan_speed;
    }
    if input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::KeyS) {
        map_view.center.y -= pan_speed;
    }

    if let Projection::Orthographic(orthographic) = &mut *projection {
        orthographic.scale = camera_scale;
    }
    transform.translation.x = map_view.center.x;
    transform.translation.y = map_view.center.y;
}

pub fn update_cursor_map_state(
    windows: Query<&Window>,
    manifest_state: Res<SceneManifestState>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MapCamera>>,
    mut cursor_map: ResMut<CursorMapState>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        cursor_map.world_position = None;
        cursor_map.geo_position = None;
        return;
    };
    let Some(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position)
    else {
        cursor_map.world_position = None;
        cursor_map.geo_position = None;
        return;
    };

    cursor_map.world_position = Some(world_position);
    cursor_map.geo_position = cursor_world_to_geo(&manifest_state, world_position);
}

pub fn render_field_boundary(mut gizmos: Gizmos, manifest_state: Res<SceneManifestState>) {
    let Some(field) = manifest_state.field.as_ref() else {
        return;
    };
    let Ok(mut points) = boundary_overlay_points(field, &manifest_state.geospatial) else {
        return;
    };
    if let Some(first) = points.first().copied() {
        points.push(first);
    }

    for segment in points.windows(2) {
        gizmos.line_2d(segment[0], segment[1], Color::srgb(1.0, 0.85, 0.1));
    }
}

pub fn boundary_overlay_points(
    field: &FieldRecord,
    geospatial: &crate::state::SceneGeospatialMetadata,
) -> anyhow::Result<Vec<Vec2>> {
    let boundary_crs = field
        .boundary
        .crs
        .as_deref()
        .filter(|crs| !crs.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("field boundary CRS is required before drawing"))?;
    let layer_crs = geospatial
        .crs
        .as_deref()
        .filter(|crs| !crs.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("layer CRS is required before drawing boundary"))?;
    if boundary_crs != layer_crs {
        anyhow::bail!("boundary CRS mismatch: boundary {boundary_crs} != layer {layer_crs}");
    }
    let extent = geospatial
        .extent
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("layer extent is required before drawing boundary"))?;
    if field.boundary.coordinates.len() < 3 {
        anyhow::bail!("field boundary requires at least 3 coordinates");
    }

    Ok(field
        .boundary
        .coordinates
        .iter()
        .map(|coordinate| geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude))
        .collect())
}

#[cfg(test)]
pub fn extent_world_size(extent: &SceneExtent) -> Vec2 {
    Vec2::new(
        ((extent.max_lon - extent.min_lon) as f32).abs() * MAP_UNITS_PER_DEGREE,
        ((extent.max_lat - extent.min_lat) as f32).abs() * MAP_UNITS_PER_DEGREE,
    )
}

pub fn geo_to_scene_local(extent: &SceneExtent, longitude: f64, latitude: f64) -> Vec2 {
    let center_lon = (extent.min_lon + extent.max_lon) / 2.0;
    let center_lat = (extent.min_lat + extent.max_lat) / 2.0;
    Vec2::new(
        ((longitude - center_lon) as f32) * MAP_UNITS_PER_DEGREE,
        ((latitude - center_lat) as f32) * MAP_UNITS_PER_DEGREE,
    )
}

pub fn scene_local_to_geo(extent: &SceneExtent, world_position: Vec2) -> (f64, f64) {
    let center_lon = (extent.min_lon + extent.max_lon) / 2.0;
    let center_lat = (extent.min_lat + extent.max_lat) / 2.0;
    let longitude = center_lon + (world_position.x as f64 / MAP_UNITS_PER_DEGREE as f64);
    let latitude = center_lat + (world_position.y as f64 / MAP_UNITS_PER_DEGREE as f64);
    (longitude, latitude)
}

pub fn cursor_world_to_geo(
    manifest_state: &SceneManifestState,
    world_position: Vec2,
) -> Option<(f64, f64)> {
    let placement = assert_manifest_layer_placement(
        &manifest_state.geospatial,
        manifest_state.width,
        manifest_state.height,
    )
    .ok()?;

    Some(scene_local_to_geo(&placement.extent, world_position))
}

pub fn tile_world_size(world_dimensions: Vec2, zoom: u8) -> Vec2 {
    let tiles_per_axis = 1_u32 << zoom;
    Vec2::new(
        world_dimensions.x / tiles_per_axis as f32,
        world_dimensions.y / tiles_per_axis as f32,
    )
}

pub fn tile_center_world(world_dimensions: Vec2, tile_id: TileId) -> Vec2 {
    let tile_size = tile_world_size(world_dimensions, tile_id.z);
    let world_left = -world_dimensions.x / 2.0;
    let world_top = world_dimensions.y / 2.0;

    Vec2::new(
        world_left + tile_size.x * (tile_id.x as f32 + 0.5),
        world_top - tile_size.y * (tile_id.y as f32 + 0.5),
    )
}

pub fn visible_tiles_for_view(
    camera_center: Vec2,
    camera_scale: f32,
    window_size: Vec2,
    world_dimensions: Vec2,
    zoom: u8,
) -> BTreeSet<TileId> {
    if world_dimensions.x <= 0.0 || world_dimensions.y <= 0.0 {
        return BTreeSet::new();
    }

    let half_view = Vec2::new(
        window_size.x * camera_scale / 2.0,
        window_size.y * camera_scale / 2.0,
    );
    let view_min = camera_center - half_view;
    let view_max = camera_center + half_view;

    let tiles_per_axis = 1_u32 << zoom;
    let tile_size = tile_world_size(world_dimensions, zoom);
    let world_left = -world_dimensions.x / 2.0;
    let world_right = world_dimensions.x / 2.0;
    let world_bottom = -world_dimensions.y / 2.0;
    let world_top = world_dimensions.y / 2.0;

    let clamped_min_x = view_min.x.max(world_left);
    let clamped_max_x = view_max.x.min(world_right);
    let clamped_min_y = view_min.y.max(world_bottom);
    let clamped_max_y = view_max.y.min(world_top);
    if clamped_min_x > clamped_max_x || clamped_min_y > clamped_max_y {
        return BTreeSet::new();
    }

    let start_x = (((clamped_min_x - world_left) / tile_size.x).floor() as i32)
        .clamp(0, tiles_per_axis as i32 - 1) as u32;
    let end_x = ((((clamped_max_x - world_left) / tile_size.x).ceil() as i32) - 1)
        .clamp(0, tiles_per_axis as i32 - 1) as u32;

    let start_y = ((((world_top - clamped_max_y) / tile_size.y).floor()) as i32)
        .clamp(0, tiles_per_axis as i32 - 1) as u32;
    let end_y = ((((world_top - clamped_min_y) / tile_size.y).ceil() as i32) - 1)
        .clamp(0, tiles_per_axis as i32 - 1) as u32;

    let mut tiles = BTreeSet::new();
    for x in start_x..=end_x {
        for y in start_y..=end_y {
            tiles.insert(TileId { z: zoom, x, y });
        }
    }

    tiles
}

#[cfg(test)]
mod tests {
    use super::{
        boundary_overlay_points, cursor_world_to_geo, extent_world_size, geo_to_scene_local,
        scene_local_to_geo, tile_center_world, tile_world_size, visible_tiles_for_view,
    };
    use crate::state::{SceneExtent, SceneGeospatialMetadata, SceneManifestState, TileId};
    use bevy::prelude::Vec2;
    use shared::schemas::{
        FarmFieldEntityStatus, FieldBoundary, FieldRecord, GeoBounds, GeoPoint, RasterResolution,
        RasterSpatialRef,
    };

    fn sample_extent() -> SceneExtent {
        SceneExtent {
            min_lon: -89.5,
            min_lat: 40.0,
            max_lon: -88.5,
            max_lat: 41.0,
        }
    }

    #[test]
    fn extent_world_size_scales_from_extent() {
        let world_size = extent_world_size(&sample_extent());
        assert!(world_size.x > 0.0);
        assert!(world_size.y > 0.0);
    }

    #[test]
    fn geo_round_trip_matches_original_coordinate() {
        let extent = sample_extent();
        let world = geo_to_scene_local(&extent, -89.0, 40.25);
        let (longitude, latitude) = scene_local_to_geo(&extent, world);

        assert!((longitude - -89.0).abs() < 0.000_001);
        assert!((latitude - 40.25).abs() < 0.000_001);
    }

    #[test]
    fn scene_center_maps_to_extent_center() {
        let extent = sample_extent();
        let (longitude, latitude) = scene_local_to_geo(&extent, Vec2::ZERO);

        assert!((longitude - -89.0).abs() < 0.000_001);
        assert!((latitude - 40.5).abs() < 0.000_001);
    }

    #[test]
    fn cursor_world_to_geo_requires_asserted_layer_transform() {
        let manifest = SceneManifestState {
            width: Some(100),
            height: Some(50),
            geospatial: sample_geospatial_with_spatial_ref(),
            ..Default::default()
        };

        let (longitude, latitude) =
            cursor_world_to_geo(&manifest, Vec2::ZERO).expect("valid transform should project");

        assert!((longitude - -89.0).abs() < 0.000_001);
        assert!((latitude - 40.5).abs() < 0.000_001);
    }

    #[test]
    fn cursor_world_to_geo_refuses_missing_or_ungeoreferenced_transform() {
        let missing_spatial_ref = SceneManifestState {
            width: Some(100),
            height: Some(50),
            geospatial: sample_geospatial(),
            ..Default::default()
        };
        assert_eq!(cursor_world_to_geo(&missing_spatial_ref, Vec2::ZERO), None);

        let ungeoreferenced = SceneManifestState {
            width: Some(100),
            height: Some(50),
            geospatial: SceneGeospatialMetadata {
                georeferenced: false,
                crs: Some("EPSG:4326".to_string()),
                center: None,
                extent: Some(sample_extent()),
                spatial_ref: Some(sample_spatial_ref()),
            },
            ..Default::default()
        };
        assert_eq!(cursor_world_to_geo(&ungeoreferenced, Vec2::ZERO), None);
    }

    #[test]
    fn tile_world_size_divides_scene_evenly() {
        let tile_size = tile_world_size(Vec2::new(400.0, 200.0), 2);
        assert_eq!(tile_size, Vec2::new(100.0, 50.0));
    }

    #[test]
    fn top_left_tile_center_matches_scene_grid() {
        let center = tile_center_world(Vec2::new(400.0, 200.0), TileId { z: 2, x: 0, y: 0 });
        assert_eq!(center, Vec2::new(-150.0, 75.0));
    }

    #[test]
    fn visible_tiles_cover_center_of_scene() {
        let tiles = visible_tiles_for_view(
            Vec2::ZERO,
            0.5,
            Vec2::new(200.0, 200.0),
            Vec2::new(400.0, 400.0),
            2,
        );
        assert!(tiles.contains(&TileId { z: 2, x: 1, y: 1 }));
        assert!(tiles.contains(&TileId { z: 2, x: 2, y: 1 }));
        assert!(tiles.contains(&TileId { z: 2, x: 1, y: 2 }));
        assert!(tiles.contains(&TileId { z: 2, x: 2, y: 2 }));
    }

    #[test]
    fn visible_tiles_clamp_to_scene_bounds() {
        let tiles = visible_tiles_for_view(
            Vec2::new(-150.0, 150.0),
            1.0,
            Vec2::new(200.0, 200.0),
            Vec2::new(400.0, 400.0),
            2,
        );
        assert!(tiles.contains(&TileId { z: 2, x: 0, y: 0 }));
    }

    #[test]
    fn tile_geo_alignment_matches_scene_extent() {
        let extent = sample_extent();
        let world_dimensions = extent_world_size(&extent);
        let tile_id = TileId { z: 1, x: 0, y: 0 };
        let tile_size = tile_world_size(world_dimensions, tile_id.z);
        let tile_center = tile_center_world(world_dimensions, tile_id);
        let top_left = tile_center + Vec2::new(-tile_size.x / 2.0, tile_size.y / 2.0);
        let bottom_right = tile_center + Vec2::new(tile_size.x / 2.0, -tile_size.y / 2.0);

        let (min_lon, max_lat) = scene_local_to_geo(&extent, top_left);
        let (max_lon, min_lat) = scene_local_to_geo(&extent, bottom_right);

        assert!((min_lon - -89.5).abs() < 0.000_001);
        assert!((max_lon - -89.0).abs() < 0.000_001);
        assert!((min_lat - 40.5).abs() < 0.000_001);
        assert!((max_lat - 41.0).abs() < 0.000_001);
    }

    #[test]
    fn boundary_overlay_points_project_when_boundary_crs_matches_layer() {
        let points =
            boundary_overlay_points(&sample_field(Some("EPSG:4326")), &sample_geospatial())
                .expect("matching CRS boundary should project");

        assert_eq!(points.len(), 4);
        assert_eq!(points[0], Vec2::new(-5_000.0, -5_000.0));
    }

    #[test]
    fn boundary_overlay_points_refuse_missing_boundary_crs() {
        let err = boundary_overlay_points(&sample_field(None), &sample_geospatial())
            .expect_err("missing boundary CRS should be refused");

        assert!(err.to_string().contains("boundary CRS"));
    }

    #[test]
    fn boundary_overlay_points_refuse_mismatched_boundary_crs() {
        let err = boundary_overlay_points(&sample_field(Some("EPSG:3857")), &sample_geospatial())
            .expect_err("mismatched boundary CRS should be refused");

        assert!(err.to_string().contains("CRS mismatch"));
    }

    fn sample_geospatial() -> SceneGeospatialMetadata {
        SceneGeospatialMetadata {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            center: None,
            extent: Some(sample_extent()),
            spatial_ref: None,
        }
    }

    fn sample_geospatial_with_spatial_ref() -> SceneGeospatialMetadata {
        SceneGeospatialMetadata {
            spatial_ref: Some(sample_spatial_ref()),
            ..sample_geospatial()
        }
    }

    fn sample_spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -89.5,
                min_lat: 40.0,
                max_lon: -88.5,
                max_lat: 41.0,
            }),
            geo_transform: Some([-89.5, 0.01, 0.0, 40.0, 0.0, 0.02]),
            resolution: Some(RasterResolution { x: 0.01, y: 0.02 }),
        }
    }

    fn sample_field(crs: Option<&str>) -> FieldRecord {
        FieldRecord {
            farm_id: Some("farm-1".to_string()),
            field_id: "field-1".to_string(),
            org_id: "org-alpha".to_string(),
            owner: "org-alpha".to_string(),
            name: "North Field".to_string(),
            area_ha: Some(12.4),
            crop: Some("corn".to_string()),
            season: Some("2026".to_string()),
            notes: None,
            boundary: FieldBoundary {
                crs: crs.map(ToOwned::to_owned),
                coordinates: vec![
                    GeoPoint {
                        longitude: -89.5,
                        latitude: 40.0,
                    },
                    GeoPoint {
                        longitude: -88.5,
                        latitude: 40.0,
                    },
                    GeoPoint {
                        longitude: -88.5,
                        latitude: 41.0,
                    },
                    GeoPoint {
                        longitude: -89.5,
                        latitude: 41.0,
                    },
                ],
            },
            extent: GeoBounds {
                min_lon: -89.5,
                min_lat: 40.0,
                max_lon: -88.5,
                max_lat: 41.0,
            },
            status: FarmFieldEntityStatus::Active,
            created_at: "2026-05-01T00:00:00Z".to_string(),
            updated_at: "2026-05-01T00:00:00Z".to_string(),
        }
    }
}
