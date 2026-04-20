use crate::plugins::map::geo_to_scene_local;
use crate::state::{
    AnnotationCreateTask, AnnotationDeleteTask, AnnotationFetchTask, AnnotationOverlayState,
    AnnotationUpdateTask, CursorMapState, DraftMode, MapCamera, SceneExtent, SceneManifestState,
    TileConfig, TileRenderState, TileStatus,
};
use anyhow::{Context, Result};
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use bevy_egui::{egui, EguiContexts};
use futures_lite::future;
use shared::schemas::{AnnotationGeometry, AnnotationRecord, GeoPoint};

const ANNOTATION_HIT_RADIUS: f32 = 24.0;
const VERTEX_HIT_RADIUS: f32 = 16.0;
const SEGMENT_HIT_RADIUS: f32 = 14.0;

pub struct ViewerAnnotationsPlugin;

impl Plugin for ViewerAnnotationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                poll_annotation_fetch,
                poll_annotation_create,
                poll_annotation_update,
                poll_annotation_delete,
                handle_annotation_keyboard_shortcuts,
                handle_annotation_map_interaction
                    .after(crate::plugins::map::update_cursor_map_state),
                render_annotations,
                render_annotation_labels,
            ),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityBucket {
    Low,
    Medium,
    High,
    Critical,
    Other,
}

pub fn poll_annotation_fetch(
    mut annotation_fetch_task: ResMut<AnnotationFetchTask>,
    mut annotations: ResMut<AnnotationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = annotation_fetch_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(items) => annotations.items = items,
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            annotation_fetch_task.0 = Some(task);
        }
    }
}

pub fn poll_annotation_create(
    mut annotation_create_task: ResMut<AnnotationCreateTask>,
    mut annotations: ResMut<AnnotationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = annotation_create_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(annotation) => {
                    annotations.items.push(annotation);
                    annotations
                        .items
                        .sort_by(|left, right| left.created_at.cmp(&right.created_at));
                    annotations.selected_annotation_id = None;
                    annotations.hovered_annotation_id = None;
                    annotations.draft_note.clear();
                    clear_annotation_draft_geometry(&mut annotations);
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            annotation_create_task.0 = Some(task);
        }
    }
}

pub fn poll_annotation_update(
    mut annotation_update_task: ResMut<AnnotationUpdateTask>,
    mut annotations: ResMut<AnnotationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = annotation_update_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(updated) => {
                    if let Some(existing) = annotations
                        .items
                        .iter_mut()
                        .find(|annotation| annotation.annotation_id == updated.annotation_id)
                    {
                        *existing = updated.clone();
                    } else {
                        annotations.items.push(updated.clone());
                    }
                    annotations.selected_annotation_id = Some(updated.annotation_id);
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            annotation_update_task.0 = Some(task);
        }
    }
}

pub fn poll_annotation_delete(
    mut annotation_delete_task: ResMut<AnnotationDeleteTask>,
    mut annotations: ResMut<AnnotationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = annotation_delete_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(annotation_id) => {
                    annotations
                        .items
                        .retain(|annotation| annotation.annotation_id != annotation_id);
                    if annotations.selected_annotation_id.as_deref() == Some(annotation_id.as_str())
                    {
                        annotations.selected_annotation_id = None;
                        clear_annotation_draft_geometry(&mut annotations);
                    }
                    if annotations.hovered_annotation_id.as_deref() == Some(annotation_id.as_str())
                    {
                        annotations.hovered_annotation_id = None;
                    }
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            annotation_delete_task.0 = Some(task);
        }
    }
}

pub fn render_annotations(
    mut gizmos: Gizmos,
    manifest_state: Res<SceneManifestState>,
    annotations: Res<AnnotationOverlayState>,
) {
    let Some(extent) = manifest_state.geospatial.extent.as_ref() else {
        return;
    };

    for annotation in &annotations.items {
        if !annotation_matches_filters(annotation, &annotations) {
            continue;
        }

        let is_selected = annotations.selected_annotation_id.as_deref()
            == Some(annotation.annotation_id.as_str());
        let is_hovered =
            annotations.hovered_annotation_id.as_deref() == Some(annotation.annotation_id.as_str());
        let color = annotation_color(annotation.severity.as_deref(), is_selected, is_hovered);
        match &annotation.geometry {
            AnnotationGeometry::Point { coordinate } => {
                let marker_scale = if is_selected || is_hovered { 1.4 } else { 1.0 };
                draw_point_marker(
                    &mut gizmos,
                    geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude),
                    color,
                    marker_scale,
                );
            }
            AnnotationGeometry::Polygon { coordinates } => {
                draw_polygon_outline(&mut gizmos, extent, coordinates, color);
                if is_selected || is_hovered {
                    draw_polygon_vertex_markers(&mut gizmos, extent, coordinates, color, 1.0);
                }
            }
        }
    }

    let draft_color = Color::srgba(0.25, 0.95, 0.95, 0.9);
    match annotations.draft_mode {
        DraftMode::Point => {
            if let Some(point) = annotations.draft_point.as_ref() {
                draw_point_marker(
                    &mut gizmos,
                    geo_to_scene_local(extent, point.longitude, point.latitude),
                    draft_color,
                    1.2,
                );
            }
        }
        DraftMode::Polygon => {
            draw_polygon_outline(
                &mut gizmos,
                extent,
                &annotations.draft_polygon_vertices,
                draft_color,
            );
            draw_polygon_vertex_markers(
                &mut gizmos,
                extent,
                &annotations.draft_polygon_vertices,
                draft_color,
                1.0,
            );
            if let Some(index) = annotations.hovered_draft_vertex_index {
                if let Some(coordinate) = annotations.draft_polygon_vertices.get(index) {
                    draw_vertex_marker(
                        &mut gizmos,
                        geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude),
                        Color::WHITE,
                        1.35,
                    );
                }
            }
            if let Some(index) = annotations.hovered_draft_segment_index {
                if let Some(midpoint) =
                    polygon_segment_midpoint(extent, &annotations.draft_polygon_vertices, index)
                {
                    draw_vertex_marker(&mut gizmos, midpoint, Color::WHITE, 1.1);
                }
            }
        }
    }
}

pub fn render_annotation_labels(
    mut contexts: EguiContexts,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MapCamera>>,
    manifest_state: Res<SceneManifestState>,
    annotations: Res<AnnotationOverlayState>,
) {
    let Some(extent) = manifest_state.geospatial.extent.as_ref() else {
        return;
    };
    let Ok(_window) = windows.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    let ctx = contexts.ctx_mut();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("annotation_labels"),
    ));

    for annotation in &annotations.items {
        if !annotation_matches_filters(annotation, &annotations) {
            continue;
        }

        let Some(anchor_world) = annotation_anchor_world(extent, &annotation.geometry) else {
            continue;
        };
        let Some(screen_position) =
            camera.world_to_viewport(camera_transform, anchor_world.extend(0.0))
        else {
            continue;
        };

        let is_selected = annotations.selected_annotation_id.as_deref()
            == Some(annotation.annotation_id.as_str());
        let is_hovered =
            annotations.hovered_annotation_id.as_deref() == Some(annotation.annotation_id.as_str());
        let fill = bevy_color_to_egui(annotation_color(
            annotation.severity.as_deref(),
            is_selected,
            is_hovered,
        ));
        let text = annotation.label.as_str();
        let font_id = egui::FontId::proportional(12.0);
        let galley =
            painter.layout_no_wrap(text.to_string(), font_id.clone(), egui::Color32::BLACK);
        let rect = egui::Rect::from_min_size(
            egui::pos2(screen_position.x + 10.0, screen_position.y - 26.0),
            galley.size() + egui::vec2(12.0, 8.0),
        );
        painter.rect_filled(rect, 4.0, fill.gamma_multiply(0.85));
        painter.text(
            rect.left_top() + egui::vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            text,
            font_id,
            egui::Color32::BLACK,
        );
    }
}

pub fn handle_annotation_map_interaction(
    mut contexts: EguiContexts,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    input: Res<ButtonInput<KeyCode>>,
    cursor_map: Res<CursorMapState>,
    manifest_state: Res<SceneManifestState>,
    mut annotations: ResMut<AnnotationOverlayState>,
) {
    if contexts.ctx_mut().wants_pointer_input() {
        clear_annotation_hover(&mut annotations);
        if mouse_buttons.just_released(MouseButton::Left) {
            annotations.dragged_draft_vertex_index = None;
        }
        return;
    }

    let Some(extent) = manifest_state.geospatial.extent.as_ref() else {
        clear_annotation_hover(&mut annotations);
        return;
    };
    let Some(world_position) = cursor_map.world_position else {
        clear_annotation_hover(&mut annotations);
        return;
    };

    annotations.hovered_annotation_id =
        nearest_annotation_id(extent, &annotations.items, world_position, &annotations);
    if annotations.draft_mode == DraftMode::Polygon {
        annotations.hovered_draft_vertex_index = nearest_polygon_vertex_index(
            extent,
            &annotations.draft_polygon_vertices,
            world_position,
            VERTEX_HIT_RADIUS,
        );
        annotations.hovered_draft_segment_index =
            if annotations.dragged_draft_vertex_index.is_none() {
                nearest_polygon_segment_index(
                    extent,
                    &annotations.draft_polygon_vertices,
                    world_position,
                    SEGMENT_HIT_RADIUS,
                )
            } else {
                None
            };
    } else {
        annotations.hovered_draft_vertex_index = None;
        annotations.hovered_draft_segment_index = None;
    }

    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Some(index) = annotations.hovered_draft_vertex_index {
            annotations.dragged_draft_vertex_index = Some(index);
            return;
        }

        if let Some(segment_index) = annotations.hovered_draft_segment_index {
            if insert_polygon_vertex_at_index(
                &mut annotations,
                segment_index,
                cursor_map.geo_position,
            ) {
                annotations.dragged_draft_vertex_index = Some(segment_index + 1);
                annotations.hovered_draft_vertex_index = Some(segment_index + 1);
                annotations.hovered_draft_segment_index = None;
                return;
            }
        }

        if let Some(annotation) = annotations.hovered_annotation_id.as_ref().and_then(|id| {
            annotations
                .items
                .iter()
                .find(|annotation| annotation.annotation_id == *id)
                .cloned()
        }) {
            annotations.selected_annotation_id = Some(annotation.annotation_id.clone());
            load_annotation_into_draft(&mut annotations, &annotation);
        }
    }

    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(index) = annotations.dragged_draft_vertex_index {
            set_polygon_vertex_from_cursor(&mut annotations, index, cursor_map.geo_position);
        }
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        annotations.dragged_draft_vertex_index = None;
    }

    let remove_hovered_vertex =
        mouse_buttons.just_pressed(MouseButton::Right) || input.just_pressed(KeyCode::Delete);
    if remove_hovered_vertex && annotations.draft_mode == DraftMode::Polygon {
        if let Some(index) = annotations.hovered_draft_vertex_index {
            remove_polygon_vertex_at_index(&mut annotations, index);
            annotations.hovered_draft_vertex_index = None;
            annotations.dragged_draft_vertex_index = None;
        }
    }
}

pub fn handle_annotation_keyboard_shortcuts(
    mut contexts: EguiContexts,
    input: Res<ButtonInput<KeyCode>>,
    mut annotations: ResMut<AnnotationOverlayState>,
) {
    if contexts.ctx_mut().wants_keyboard_input() {
        return;
    }

    if input.just_pressed(KeyCode::Escape) {
        annotations.selected_annotation_id = None;
        clear_annotation_draft_geometry(&mut annotations);
        return;
    }

    let undo_polygon_vertex = input.just_pressed(KeyCode::Backspace)
        || ((input.pressed(KeyCode::ControlLeft) || input.pressed(KeyCode::ControlRight))
            && input.just_pressed(KeyCode::KeyZ));
    if undo_polygon_vertex && annotations.draft_mode == DraftMode::Polygon {
        annotations.draft_polygon_vertices.pop();
        annotations.hovered_draft_vertex_index = None;
        annotations.hovered_draft_segment_index = None;
    }
}

pub fn severity_bucket(severity: Option<&str>) -> SeverityBucket {
    match severity.map(|value| value.trim().to_ascii_lowercase()) {
        Some(level) if level == "critical" => SeverityBucket::Critical,
        Some(level) if level == "high" => SeverityBucket::High,
        Some(level) if level == "medium" => SeverityBucket::Medium,
        Some(level) if level == "low" => SeverityBucket::Low,
        _ => SeverityBucket::Other,
    }
}

pub fn annotation_matches_filters(
    annotation: &AnnotationRecord,
    annotations: &AnnotationOverlayState,
) -> bool {
    let geometry_visible = match annotation.geometry {
        AnnotationGeometry::Point { .. } => annotations.show_points,
        AnnotationGeometry::Polygon { .. } => annotations.show_polygons,
    };
    if !geometry_visible {
        return false;
    }

    let severity_visible = match severity_bucket(annotation.severity.as_deref()) {
        SeverityBucket::Low => annotations.show_low,
        SeverityBucket::Medium => annotations.show_medium,
        SeverityBucket::High => annotations.show_high,
        SeverityBucket::Critical => annotations.show_critical,
        SeverityBucket::Other => annotations.show_other,
    };
    if !severity_visible {
        return false;
    }

    let label_filter = annotations.filter_label.trim();
    if label_filter.is_empty() {
        return true;
    }

    annotation
        .label
        .to_ascii_lowercase()
        .contains(&label_filter.to_ascii_lowercase())
}

pub fn annotation_color(severity: Option<&str>, is_selected: bool, is_hovered: bool) -> Color {
    if is_selected {
        return Color::srgb(1.0, 0.95, 0.25);
    }
    if is_hovered {
        return Color::srgb(0.95, 0.98, 1.0);
    }

    match severity_bucket(severity) {
        SeverityBucket::Critical => Color::srgb(0.82, 0.10, 0.10),
        SeverityBucket::High => Color::srgb(0.95, 0.35, 0.10),
        SeverityBucket::Medium => Color::srgb(0.95, 0.75, 0.10),
        SeverityBucket::Low => Color::srgb(0.20, 0.80, 0.25),
        SeverityBucket::Other => Color::srgb(0.15, 0.85, 0.95),
    }
}

pub fn draw_point_marker(gizmos: &mut Gizmos, center: Vec2, color: Color, scale: f32) {
    let half_size = 20.0 * scale;
    gizmos.line_2d(
        center + Vec2::new(-half_size, 0.0),
        center + Vec2::new(half_size, 0.0),
        color,
    );
    gizmos.line_2d(
        center + Vec2::new(0.0, -half_size),
        center + Vec2::new(0.0, half_size),
        color,
    );
}

pub fn draw_polygon_outline(
    gizmos: &mut Gizmos,
    extent: &SceneExtent,
    coordinates: &[GeoPoint],
    color: Color,
) {
    if coordinates.is_empty() {
        return;
    }
    let mut points = Vec::with_capacity(coordinates.len() + 1);
    for coordinate in coordinates {
        points.push(geo_to_scene_local(
            extent,
            coordinate.longitude,
            coordinate.latitude,
        ));
    }
    if points.len() == 1 {
        draw_point_marker(gizmos, points[0], color, 1.0);
        return;
    }
    if points.len() >= 2 {
        for segment in points.windows(2) {
            gizmos.line_2d(segment[0], segment[1], color);
        }
    }
    if points.len() >= 3 {
        let first = points[0];
        let last = *points.last().unwrap_or(&first);
        gizmos.line_2d(last, first, color);
    }
}

pub fn draw_polygon_vertex_markers(
    gizmos: &mut Gizmos,
    extent: &SceneExtent,
    coordinates: &[GeoPoint],
    color: Color,
    scale: f32,
) {
    for coordinate in coordinates {
        draw_vertex_marker(
            gizmos,
            geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude),
            color,
            scale,
        );
    }
}

pub fn draw_vertex_marker(gizmos: &mut Gizmos, center: Vec2, color: Color, scale: f32) {
    let half_size = 8.0 * scale;
    let top_left = center + Vec2::new(-half_size, half_size);
    let top_right = center + Vec2::new(half_size, half_size);
    let bottom_right = center + Vec2::new(half_size, -half_size);
    let bottom_left = center + Vec2::new(-half_size, -half_size);

    gizmos.line_2d(top_left, top_right, color);
    gizmos.line_2d(top_right, bottom_right, color);
    gizmos.line_2d(bottom_right, bottom_left, color);
    gizmos.line_2d(bottom_left, top_left, color);
}

pub fn set_draft_point_from_cursor(
    annotations: &mut AnnotationOverlayState,
    geo_position: Option<(f64, f64)>,
) {
    let Some((longitude, latitude)) = geo_position else {
        return;
    };
    annotations.draft_mode = DraftMode::Point;
    annotations.draft_polygon_vertices.clear();
    annotations.hovered_draft_vertex_index = None;
    annotations.hovered_draft_segment_index = None;
    annotations.dragged_draft_vertex_index = None;
    annotations.draft_point = Some(GeoPoint {
        longitude,
        latitude,
    });
}

pub fn add_polygon_vertex_from_cursor(
    annotations: &mut AnnotationOverlayState,
    geo_position: Option<(f64, f64)>,
) {
    let Some((longitude, latitude)) = geo_position else {
        return;
    };
    annotations.draft_mode = DraftMode::Polygon;
    annotations.draft_point = None;
    annotations.draft_polygon_vertices.push(GeoPoint {
        longitude,
        latitude,
    });
}

pub fn insert_polygon_vertex_at_index(
    annotations: &mut AnnotationOverlayState,
    segment_index: usize,
    geo_position: Option<(f64, f64)>,
) -> bool {
    let Some((longitude, latitude)) = geo_position else {
        return false;
    };
    if annotations.draft_mode != DraftMode::Polygon || annotations.draft_polygon_vertices.len() < 2
    {
        return false;
    }

    let insert_index = (segment_index + 1).min(annotations.draft_polygon_vertices.len());
    annotations.draft_polygon_vertices.insert(
        insert_index,
        GeoPoint {
            longitude,
            latitude,
        },
    );
    true
}

pub fn remove_polygon_vertex_at_index(
    annotations: &mut AnnotationOverlayState,
    vertex_index: usize,
) -> bool {
    if annotations.draft_mode != DraftMode::Polygon
        || vertex_index >= annotations.draft_polygon_vertices.len()
    {
        return false;
    }

    annotations.draft_polygon_vertices.remove(vertex_index);
    true
}

pub fn set_polygon_vertex_from_cursor(
    annotations: &mut AnnotationOverlayState,
    vertex_index: usize,
    geo_position: Option<(f64, f64)>,
) -> bool {
    let Some((longitude, latitude)) = geo_position else {
        return false;
    };
    let Some(vertex) = annotations.draft_polygon_vertices.get_mut(vertex_index) else {
        return false;
    };
    vertex.longitude = longitude;
    vertex.latitude = latitude;
    true
}

pub fn clear_annotation_draft_geometry(annotations: &mut AnnotationOverlayState) {
    annotations.draft_point = None;
    annotations.draft_polygon_vertices.clear();
    annotations.hovered_draft_vertex_index = None;
    annotations.hovered_draft_segment_index = None;
    annotations.dragged_draft_vertex_index = None;
}

pub fn draft_geometry(annotations: &AnnotationOverlayState) -> Option<AnnotationGeometry> {
    match annotations.draft_mode {
        DraftMode::Point => annotations
            .draft_point
            .clone()
            .map(|coordinate| AnnotationGeometry::Point { coordinate }),
        DraftMode::Polygon => {
            (annotations.draft_polygon_vertices.len() >= 3).then(|| AnnotationGeometry::Polygon {
                coordinates: annotations.draft_polygon_vertices.clone(),
            })
        }
    }
}

pub fn load_annotation_into_draft(
    annotations: &mut AnnotationOverlayState,
    annotation: &AnnotationRecord,
) {
    annotations.draft_label = annotation.label.clone();
    annotations.draft_note = annotation.note.clone().unwrap_or_default();
    annotations.draft_severity = annotation.severity.clone().unwrap_or_default();
    clear_annotation_draft_geometry(annotations);

    match &annotation.geometry {
        AnnotationGeometry::Point { coordinate } => {
            annotations.draft_mode = DraftMode::Point;
            annotations.draft_point = Some(coordinate.clone());
        }
        AnnotationGeometry::Polygon { coordinates } => {
            annotations.draft_mode = DraftMode::Polygon;
            annotations.draft_polygon_vertices = coordinates.clone();
        }
    }
}

pub fn start_annotation_fetch(
    annotation_fetch_task: &mut AnnotationFetchTask,
    config: &TileConfig,
) -> Result<()> {
    let scene_id = match &config.scene_id {
        Some(id) => id.clone(),
        None => {
            annotation_fetch_task.0 = None;
            return Ok(());
        }
    };

    let url = format!("{}/api/scenes/{}/annotations", config.base_url, scene_id);
    annotation_fetch_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read annotations response body")?;
        let annotations = serde_json::from_slice::<Vec<AnnotationRecord>>(&bytes)
            .context("failed to decode annotations")?;
        Ok(annotations)
    }));

    Ok(())
}

pub fn start_annotation_create(
    annotation_create_task: &mut AnnotationCreateTask,
    config: &TileConfig,
    label: String,
    note: String,
    severity: String,
    geometry: AnnotationGeometry,
) -> Result<()> {
    let scene_id = crate::state::ensure_scene_id(config, "create annotations")?;
    let url = format!("{}/api/scenes/{}/annotations", config.base_url, scene_id);
    let payload = serde_json::json!({
        "label": label,
        "note": note,
        "severity": severity,
        "geometry": geometry
    })
    .to_string();

    annotation_create_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read create annotation response body")?;
        let annotation = serde_json::from_slice::<AnnotationRecord>(&bytes)
            .context("failed to decode created annotation")?;
        Ok(annotation)
    }));

    Ok(())
}

pub fn start_annotation_update(
    annotation_update_task: &mut AnnotationUpdateTask,
    config: &TileConfig,
    annotation_id: &str,
    label: String,
    note: String,
    severity: String,
    geometry: AnnotationGeometry,
) -> Result<()> {
    let scene_id = crate::state::ensure_scene_id(config, "update annotations")?;
    let url = format!(
        "{}/api/scenes/{}/annotations/{}",
        config.base_url, scene_id, annotation_id
    );
    let payload = serde_json::json!({
        "label": label,
        "note": note,
        "severity": severity,
        "geometry": geometry
    })
    .to_string();

    annotation_update_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .put(&url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read update annotation response body")?;
        let annotation = serde_json::from_slice::<AnnotationRecord>(&bytes)
            .context("failed to decode updated annotation")?;
        Ok(annotation)
    }));

    Ok(())
}

pub fn start_annotation_delete(
    annotation_delete_task: &mut AnnotationDeleteTask,
    config: &TileConfig,
    annotation_id: &str,
) -> Result<()> {
    let scene_id = crate::state::ensure_scene_id(config, "delete annotations")?;
    let url = format!(
        "{}/api/scenes/{}/annotations/{}",
        config.base_url, scene_id, annotation_id
    );
    let annotation_id = annotation_id.to_string();

    annotation_delete_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .delete(&url)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        Ok(annotation_id)
    }));

    Ok(())
}

pub fn clear_annotations(
    annotations: &mut AnnotationOverlayState,
    annotation_fetch_task: &mut AnnotationFetchTask,
    annotation_create_task: &mut AnnotationCreateTask,
    annotation_update_task: &mut AnnotationUpdateTask,
    annotation_delete_task: &mut AnnotationDeleteTask,
) {
    annotations.items.clear();
    annotations.selected_annotation_id = None;
    annotations.hovered_annotation_id = None;
    annotations.draft_note.clear();
    clear_annotation_draft_geometry(annotations);
    annotation_fetch_task.0 = None;
    annotation_create_task.0 = None;
    annotation_update_task.0 = None;
    annotation_delete_task.0 = None;
}

pub fn selected_annotation<'a>(
    annotations: &'a AnnotationOverlayState,
) -> Option<&'a AnnotationRecord> {
    annotations
        .selected_annotation_id
        .as_ref()
        .and_then(|annotation_id| {
            annotations
                .items
                .iter()
                .find(|annotation| annotation.annotation_id == *annotation_id)
        })
}

pub fn annotation_anchor_world(
    extent: &SceneExtent,
    geometry: &AnnotationGeometry,
) -> Option<Vec2> {
    match geometry {
        AnnotationGeometry::Point { coordinate } => Some(geo_to_scene_local(
            extent,
            coordinate.longitude,
            coordinate.latitude,
        )),
        AnnotationGeometry::Polygon { coordinates } => polygon_centroid_world(extent, coordinates),
    }
}

pub fn polygon_centroid_world(extent: &SceneExtent, coordinates: &[GeoPoint]) -> Option<Vec2> {
    if coordinates.is_empty() {
        return None;
    }

    let (sum, count) = coordinates
        .iter()
        .fold((Vec2::ZERO, 0.0_f32), |(acc, count), point| {
            (
                acc + geo_to_scene_local(extent, point.longitude, point.latitude),
                count + 1.0,
            )
        });
    Some(sum / count)
}

pub fn nearest_annotation_id(
    extent: &SceneExtent,
    items: &[AnnotationRecord],
    world_position: Vec2,
    annotations: &AnnotationOverlayState,
) -> Option<String> {
    items
        .iter()
        .filter(|annotation| annotation_matches_filters(annotation, annotations))
        .filter_map(|annotation| {
            let distance =
                annotation_distance_to_cursor(extent, &annotation.geometry, world_position)?;
            (distance <= ANNOTATION_HIT_RADIUS)
                .then(|| (distance, annotation.annotation_id.clone()))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, annotation_id)| annotation_id)
}

pub fn annotation_distance_to_cursor(
    extent: &SceneExtent,
    geometry: &AnnotationGeometry,
    world_position: Vec2,
) -> Option<f32> {
    match geometry {
        AnnotationGeometry::Point { coordinate } => Some(
            geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude)
                .distance(world_position),
        ),
        AnnotationGeometry::Polygon { coordinates } => {
            let points: Vec<Vec2> = coordinates
                .iter()
                .map(|coordinate| {
                    geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude)
                })
                .collect();
            if points.is_empty() {
                return None;
            }
            if point_in_polygon(world_position, &points) {
                return Some(0.0);
            }

            let mut min_distance = f32::MAX;
            for segment in points.windows(2) {
                min_distance = min_distance.min(point_distance_to_segment(
                    world_position,
                    segment[0],
                    segment[1],
                ));
            }
            if points.len() >= 3 {
                min_distance = min_distance.min(point_distance_to_segment(
                    world_position,
                    *points.last().unwrap_or(&points[0]),
                    points[0],
                ));
            }
            Some(min_distance)
        }
    }
}

pub fn nearest_polygon_vertex_index(
    extent: &SceneExtent,
    coordinates: &[GeoPoint],
    world_position: Vec2,
    threshold: f32,
) -> Option<usize> {
    coordinates
        .iter()
        .enumerate()
        .filter_map(|(index, coordinate)| {
            let distance = geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude)
                .distance(world_position);
            (distance <= threshold).then_some((distance, index))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, index)| index)
}

pub fn nearest_polygon_segment_index(
    extent: &SceneExtent,
    coordinates: &[GeoPoint],
    world_position: Vec2,
    threshold: f32,
) -> Option<usize> {
    if coordinates.len() < 2 {
        return None;
    }

    let mut best: Option<(f32, usize)> = None;
    for index in 0..(coordinates.len() - 1) {
        let start = geo_to_scene_local(
            extent,
            coordinates[index].longitude,
            coordinates[index].latitude,
        );
        let end = geo_to_scene_local(
            extent,
            coordinates[index + 1].longitude,
            coordinates[index + 1].latitude,
        );
        let distance = point_distance_to_segment(world_position, start, end);
        if distance <= threshold {
            match best {
                Some((current, _)) if current <= distance => {}
                _ => best = Some((distance, index)),
            }
        }
    }

    if coordinates.len() >= 3 {
        let start = geo_to_scene_local(
            extent,
            coordinates[coordinates.len() - 1].longitude,
            coordinates[coordinates.len() - 1].latitude,
        );
        let end = geo_to_scene_local(extent, coordinates[0].longitude, coordinates[0].latitude);
        let distance = point_distance_to_segment(world_position, start, end);
        if distance <= threshold {
            match best {
                Some((current, _)) if current <= distance => {}
                _ => best = Some((distance, coordinates.len() - 1)),
            }
        }
    }

    best.map(|(_, index)| index)
}

pub fn polygon_segment_midpoint(
    extent: &SceneExtent,
    coordinates: &[GeoPoint],
    segment_index: usize,
) -> Option<Vec2> {
    if coordinates.len() < 2 || segment_index >= coordinates.len() {
        return None;
    }

    let start = geo_to_scene_local(
        extent,
        coordinates[segment_index].longitude,
        coordinates[segment_index].latitude,
    );
    let end_index = if segment_index + 1 < coordinates.len() {
        segment_index + 1
    } else if coordinates.len() >= 3 {
        0
    } else {
        return None;
    };
    let end = geo_to_scene_local(
        extent,
        coordinates[end_index].longitude,
        coordinates[end_index].latitude,
    );
    Some((start + end) * 0.5)
}

pub fn point_distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }

    let projection = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * projection)
}

pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut previous = *polygon.last().unwrap_or(&polygon[0]);
    for current in polygon {
        let intersects = ((current.y > point.y) != (previous.y > point.y))
            && (point.x
                < (previous.x - current.x) * (point.y - current.y)
                    / ((previous.y - current.y).abs().max(f32::EPSILON))
                    + current.x);
        if intersects {
            inside = !inside;
        }
        previous = *current;
    }
    inside
}

pub fn clear_annotation_hover(annotations: &mut AnnotationOverlayState) {
    annotations.hovered_annotation_id = None;
    annotations.hovered_draft_vertex_index = None;
    annotations.hovered_draft_segment_index = None;
}

fn bevy_color_to_egui(color: Color) -> egui::Color32 {
    let srgba = color.to_srgba();
    egui::Color32::from_rgba_unmultiplied(
        (srgba.red * 255.0).round() as u8,
        (srgba.green * 255.0).round() as u8,
        (srgba.blue * 255.0).round() as u8,
        (srgba.alpha * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        annotation_matches_filters, nearest_polygon_segment_index, point_distance_to_segment,
        remove_polygon_vertex_at_index, severity_bucket, AnnotationOverlayState, SeverityBucket,
    };
    use crate::state::{DraftMode, SceneExtent};
    use bevy::prelude::Vec2;
    use shared::schemas::{AnnotationGeometry, AnnotationRecord, GeoPoint};

    fn sample_polygon() -> Vec<GeoPoint> {
        vec![
            GeoPoint {
                longitude: -89.0,
                latitude: 40.0,
            },
            GeoPoint {
                longitude: -88.0,
                latitude: 40.0,
            },
            GeoPoint {
                longitude: -88.0,
                latitude: 41.0,
            },
        ]
    }

    fn sample_extent() -> SceneExtent {
        SceneExtent {
            min_lon: -89.0,
            min_lat: 40.0,
            max_lon: -88.0,
            max_lat: 41.0,
        }
    }

    fn sample_annotation() -> AnnotationRecord {
        AnnotationRecord {
            annotation_id: "annotation-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            label: "Water Stress".to_string(),
            note: Some("Check irrigation".to_string()),
            severity: Some("high".to_string()),
            geometry: AnnotationGeometry::Polygon {
                coordinates: sample_polygon(),
            },
            created_at: "2026-04-19T00:00:00Z".to_string(),
            updated_at: "2026-04-19T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn severity_bucket_normalizes_known_values() {
        assert_eq!(severity_bucket(Some("critical")), SeverityBucket::Critical);
        assert_eq!(severity_bucket(Some("HIGH")), SeverityBucket::High);
        assert_eq!(severity_bucket(Some("medium")), SeverityBucket::Medium);
        assert_eq!(severity_bucket(Some("low")), SeverityBucket::Low);
        assert_eq!(severity_bucket(Some("unknown")), SeverityBucket::Other);
        assert_eq!(severity_bucket(None), SeverityBucket::Other);
    }

    #[test]
    fn annotation_filters_apply_label_geometry_and_severity() {
        let annotation = sample_annotation();
        let mut state = AnnotationOverlayState {
            filter_label: "water".to_string(),
            ..Default::default()
        };
        assert!(annotation_matches_filters(&annotation, &state));

        state.show_polygons = false;
        assert!(!annotation_matches_filters(&annotation, &state));

        state.show_polygons = true;
        state.show_high = false;
        assert!(!annotation_matches_filters(&annotation, &state));

        state.show_high = true;
        state.filter_label = "nitrogen".to_string();
        assert!(!annotation_matches_filters(&annotation, &state));
    }

    #[test]
    fn nearest_segment_returns_closest_edge() {
        let extent = sample_extent();
        let world_position = Vec2::new(0.0, -4_900.0);
        let segment_index =
            nearest_polygon_segment_index(&extent, &sample_polygon(), world_position, 1_000.0);
        assert_eq!(segment_index, Some(0));
    }

    #[test]
    fn remove_polygon_vertex_at_index_updates_draft_polygon() {
        let mut state = AnnotationOverlayState {
            draft_mode: DraftMode::Polygon,
            draft_polygon_vertices: sample_polygon(),
            ..Default::default()
        };

        assert!(remove_polygon_vertex_at_index(&mut state, 1));
        assert_eq!(state.draft_polygon_vertices.len(), 2);
        assert_eq!(state.draft_polygon_vertices[1].longitude, -88.0);
        assert_eq!(state.draft_polygon_vertices[1].latitude, 41.0);
    }

    #[test]
    fn point_distance_to_segment_projects_onto_edge() {
        let distance = point_distance_to_segment(
            Vec2::new(3.0, 4.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(6.0, 0.0),
        );
        assert!((distance - 4.0).abs() < 0.001);
    }
}
