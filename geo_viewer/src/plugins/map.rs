use crate::state::{
    CursorMapState, MapCamera, MapViewState, SceneExtent, SceneManifestState, TileRenderState,
    ViewerState, MAP_UNITS_PER_DEGREE,
};
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
};

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
    cursor_map.geo_position = manifest_state
        .geospatial
        .extent
        .as_ref()
        .map(|extent| scene_local_to_geo(extent, world_position));
}

pub fn render_field_boundary(mut gizmos: Gizmos, manifest_state: Res<SceneManifestState>) {
    let Some(field) = manifest_state.field.as_ref() else {
        return;
    };
    let Some(extent) = manifest_state.geospatial.extent.as_ref() else {
        return;
    };
    let mut points = Vec::with_capacity(field.boundary.coordinates.len() + 1);

    for coordinate in &field.boundary.coordinates {
        points.push(geo_to_scene_local(
            extent,
            coordinate.longitude,
            coordinate.latitude,
        ));
    }

    if points.len() < 3 {
        return;
    }
    if let Some(first) = points.first().copied() {
        points.push(first);
    }

    for segment in points.windows(2) {
        gizmos.line_2d(segment[0], segment[1], Color::srgb(1.0, 0.85, 0.1));
    }
}

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

#[cfg(test)]
mod tests {
    use super::{extent_world_size, geo_to_scene_local, scene_local_to_geo};
    use crate::state::SceneExtent;
    use bevy::prelude::Vec2;

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
}
