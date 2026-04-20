use crate::plugins::annotations::{
    add_polygon_vertex_from_cursor, annotation_matches_filters, clear_annotation_draft_geometry,
    clear_annotations, draft_geometry, load_annotation_into_draft, remove_polygon_vertex_at_index,
    selected_annotation, set_draft_point_from_cursor, start_annotation_create,
    start_annotation_delete, start_annotation_fetch, start_annotation_update,
};
use crate::plugins::network::{
    clear_loaded_tile, clear_manifest_state, start_field_list_fetch, start_field_scenes_fetch,
    start_manifest_fetch, start_tile_fetch,
};
use crate::state::{
    AnnotationCreateTask, AnnotationDeleteTask, AnnotationFetchTask, AnnotationOverlayState,
    AnnotationUpdateTask, CursorMapState, DraftMode, FieldCatalogState, FieldListFetchTask,
    FieldScenesFetchTask, ManifestFetchTask, MapViewState, SceneManifestState, TileConfig,
    TileFetchTask, TileRenderState, TileStatus, ViewerState,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

pub struct ViewerUiPlugin;

impl Plugin for ViewerUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, render_ui);
    }
}

#[derive(SystemParam)]
struct CatalogUiState<'w, 's> {
    field_catalog: ResMut<'w, FieldCatalogState>,
    field_list_task: ResMut<'w, FieldListFetchTask>,
    field_scenes_task: ResMut<'w, FieldScenesFetchTask>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct SceneUiState<'w, 's> {
    manifest_state: ResMut<'w, SceneManifestState>,
    manifest_task: ResMut<'w, ManifestFetchTask>,
    tile_state: ResMut<'w, TileRenderState>,
    fetch_task: ResMut<'w, TileFetchTask>,
    map_view: ResMut<'w, MapViewState>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct AnnotationUiState<'w, 's> {
    annotations: ResMut<'w, AnnotationOverlayState>,
    annotation_fetch_task: ResMut<'w, AnnotationFetchTask>,
    annotation_create_task: ResMut<'w, AnnotationCreateTask>,
    annotation_update_task: ResMut<'w, AnnotationUpdateTask>,
    annotation_delete_task: ResMut<'w, AnnotationDeleteTask>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

fn render_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut viewer_state: ResMut<ViewerState>,
    mut config: ResMut<TileConfig>,
    mut catalog_ui: CatalogUiState,
    mut scene_ui: SceneUiState,
    cursor_map: Res<CursorMapState>,
    mut annotation_ui: AnnotationUiState,
) {
    let field_catalog = &mut catalog_ui.field_catalog;
    let field_list_task = &mut catalog_ui.field_list_task;
    let field_scenes_task = &mut catalog_ui.field_scenes_task;
    let manifest_state = &mut scene_ui.manifest_state;
    let manifest_task = &mut scene_ui.manifest_task;
    let tile_state = &mut scene_ui.tile_state;
    let fetch_task = &mut scene_ui.fetch_task;
    let map_view = &mut scene_ui.map_view;
    let annotations = &mut annotation_ui.annotations;
    let annotation_fetch_task = &mut annotation_ui.annotation_fetch_task;
    let annotation_create_task = &mut annotation_ui.annotation_create_task;
    let annotation_update_task = &mut annotation_ui.annotation_update_task;
    let annotation_delete_task = &mut annotation_ui.annotation_delete_task;

    egui::SidePanel::left("layers_panel").show(contexts.ctx_mut(), |ui| {
        render_fields_panel(
            ui,
            field_catalog,
            field_list_task,
            field_scenes_task,
            &mut viewer_state,
            &mut config,
            manifest_state,
            manifest_task,
            tile_state,
            fetch_task,
            map_view,
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
            &mut commands,
        );

        render_layers_panel(
            ui,
            manifest_state,
            &mut viewer_state,
            &mut config,
            fetch_task,
            tile_state,
        );
        render_scene_panel(
            ui,
            field_catalog,
            &mut viewer_state,
            &mut config,
            manifest_state,
            manifest_task,
            tile_state,
            fetch_task,
            map_view,
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
            &mut commands,
        );
        render_view_panel(ui, &mut viewer_state, map_view);
        render_annotations_panel(
            ui,
            &config,
            &cursor_map,
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
            tile_state,
        );
    });

    render_status_bar(
        contexts.ctx_mut(),
        &config,
        &viewer_state,
        manifest_state,
        tile_state,
        &cursor_map,
        annotations,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_fields_panel(
    ui: &mut egui::Ui,
    field_catalog: &mut FieldCatalogState,
    field_list_task: &mut FieldListFetchTask,
    field_scenes_task: &mut FieldScenesFetchTask,
    viewer_state: &mut ViewerState,
    config: &mut TileConfig,
    manifest_state: &mut SceneManifestState,
    manifest_task: &mut ManifestFetchTask,
    tile_state: &mut TileRenderState,
    fetch_task: &mut TileFetchTask,
    map_view: &mut MapViewState,
    annotations: &mut AnnotationOverlayState,
    annotation_fetch_task: &mut AnnotationFetchTask,
    annotation_create_task: &mut AnnotationCreateTask,
    annotation_update_task: &mut AnnotationUpdateTask,
    annotation_delete_task: &mut AnnotationDeleteTask,
    commands: &mut Commands,
) {
    ui.horizontal(|ui| {
        ui.heading("Fields");
        if ui.button("Refresh").clicked() {
            if let Err(err) = start_field_list_fetch(field_list_task, config) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
        }
    });
    ui.separator();
    if field_catalog.fields.is_empty() {
        ui.label("No fields loaded");
    } else {
        let fields = field_catalog.fields.clone();
        for field in fields {
            let selected = field_catalog.selected_field_id.as_deref() == Some(&field.field_id);
            let response = ui.selectable_label(
                selected,
                format!(
                    "{} ({})",
                    field.name,
                    field.crop.as_deref().unwrap_or("crop n/a")
                ),
            );
            if response.clicked() && !selected {
                field_catalog.selected_field_id = Some(field.field_id.clone());
                field_catalog.selected_scene_id = None;
                field_catalog.scenes.clear();
                if let Err(err) =
                    start_field_scenes_fetch(field_scenes_task, config, &field.field_id)
                {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
            response.on_hover_text(format!(
                "Season: {}\nExtent: {:.5}, {:.5} -> {:.5}, {:.5}",
                field.season.as_deref().unwrap_or("n/a"),
                field.extent.min_lon,
                field.extent.min_lat,
                field.extent.max_lon,
                field.extent.max_lat
            ));
        }
    }
    ui.add_space(8.0);

    ui.heading("Scenes");
    ui.separator();
    if field_catalog.selected_field_id.is_none() {
        ui.label("Select a field");
    } else if field_catalog.scenes.is_empty() {
        ui.label("No scenes for selected field");
    } else {
        let scenes = field_catalog.scenes.clone();
        for scene in scenes {
            let selected = field_catalog.selected_scene_id.as_deref() == Some(&scene.scene_id);
            let response = ui.selectable_label(
                selected,
                format!("{} ({})", scene.scene_id, scene.acquired_at),
            );
            if response.clicked() {
                field_catalog.selected_scene_id = Some(scene.scene_id.clone());
                viewer_state.scene_id_input = scene.scene_id.clone();
                config.scene_id = Some(scene.scene_id.clone());
                viewer_state.selected_layer = 0;
                config.product_kind = crate::state::DEFAULT_PRODUCT_KIND.to_string();
                clear_loaded_tile(commands, tile_state);
                clear_manifest_state(manifest_state);
                clear_annotations(
                    annotations,
                    annotation_fetch_task,
                    annotation_create_task,
                    annotation_update_task,
                    annotation_delete_task,
                );
                fetch_task.0 = None;
                map_view.center = Vec2::ZERO;
                map_view.needs_fit = true;
                if let Err(err) = start_manifest_fetch(manifest_task, manifest_state, config) {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
            response.on_hover_text(format!("Sensor: {}", scene.sensor));
        }
    }
    ui.add_space(8.0);
}

fn render_layers_panel(
    ui: &mut egui::Ui,
    manifest_state: &SceneManifestState,
    viewer_state: &mut ViewerState,
    config: &mut TileConfig,
    fetch_task: &mut TileFetchTask,
    tile_state: &mut TileRenderState,
) {
    ui.heading("Layers");
    ui.separator();
    if manifest_state.products.is_empty() {
        ui.label("No layers loaded");
    } else {
        for (idx, product) in manifest_state.products.iter().enumerate() {
            let label = format!(
                "{} ({}, {})",
                product.kind.to_uppercase(),
                product.filename,
                product.content_type
            );
            let response = ui.radio_value(&mut viewer_state.selected_layer, idx, label);
            if response.changed() {
                config.product_kind = product.kind.clone();
                if let Err(err) = start_tile_fetch(fetch_task, tile_state, config) {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
            response.on_hover_text(product.url_path.clone());
        }
    }
    ui.add_space(8.0);
}

#[allow(clippy::too_many_arguments)]
fn render_scene_panel(
    ui: &mut egui::Ui,
    field_catalog: &mut FieldCatalogState,
    viewer_state: &mut ViewerState,
    config: &mut TileConfig,
    manifest_state: &mut SceneManifestState,
    manifest_task: &mut ManifestFetchTask,
    tile_state: &mut TileRenderState,
    fetch_task: &mut TileFetchTask,
    map_view: &mut MapViewState,
    annotations: &mut AnnotationOverlayState,
    annotation_fetch_task: &mut AnnotationFetchTask,
    annotation_create_task: &mut AnnotationCreateTask,
    annotation_update_task: &mut AnnotationUpdateTask,
    annotation_delete_task: &mut AnnotationDeleteTask,
    commands: &mut Commands,
) {
    ui.heading("Scene");
    ui.label("Scene ID");
    let mut load_requested = false;
    let response = ui.text_edit_singleline(&mut viewer_state.scene_id_input);
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        load_requested = true;
    }
    if ui.button("Load Scene").clicked() {
        load_requested = true;
    }

    if !load_requested {
        return;
    }

    let trimmed = viewer_state.scene_id_input.trim().to_string();
    if trimmed.is_empty() {
        config.scene_id = None;
        field_catalog.selected_scene_id = None;
        config.product_kind = crate::state::DEFAULT_PRODUCT_KIND.to_string();
        tile_state.status = TileStatus::MissingScene;
        clear_loaded_tile(commands, tile_state);
        clear_manifest_state(manifest_state);
        clear_annotations(
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
        );
        fetch_task.0 = None;
        manifest_task.0 = None;
        map_view.center = Vec2::ZERO;
        map_view.needs_fit = true;
        return;
    }

    config.scene_id = Some(trimmed.clone());
    field_catalog.selected_scene_id = Some(trimmed.clone());
    viewer_state.selected_layer = 0;
    config.product_kind = crate::state::DEFAULT_PRODUCT_KIND.to_string();
    clear_loaded_tile(commands, tile_state);
    clear_annotations(
        annotations,
        annotation_fetch_task,
        annotation_create_task,
        annotation_update_task,
        annotation_delete_task,
    );
    fetch_task.0 = None;
    map_view.center = Vec2::ZERO;
    map_view.needs_fit = true;
    if let Err(err) = start_manifest_fetch(manifest_task, manifest_state, config) {
        tile_state.status = TileStatus::Error(err.to_string());
    }
}

fn render_view_panel(
    ui: &mut egui::Ui,
    viewer_state: &mut ViewerState,
    map_view: &mut MapViewState,
) {
    ui.add_space(8.0);
    ui.heading("View");
    ui.label("Zoom");
    ui.add(egui::Slider::new(&mut viewer_state.zoom_level, 0.2..=5.0).logarithmic(true));
    ui.small("Mouse wheel zooms. Middle-drag pans the map.");
    if ui.button("Reset View").clicked() {
        map_view.center = Vec2::ZERO;
        map_view.needs_fit = true;
    }
}

#[allow(clippy::too_many_arguments)]
fn render_annotations_panel(
    ui: &mut egui::Ui,
    config: &TileConfig,
    cursor_map: &CursorMapState,
    annotations: &mut AnnotationOverlayState,
    annotation_fetch_task: &mut AnnotationFetchTask,
    annotation_create_task: &mut AnnotationCreateTask,
    annotation_update_task: &mut AnnotationUpdateTask,
    annotation_delete_task: &mut AnnotationDeleteTask,
    tile_state: &mut TileRenderState,
) {
    ui.add_space(8.0);
    ui.heading("Annotations");
    ui.horizontal(|ui| {
        ui.label(format!("Count: {}", annotations.items.len()));
        if ui.button("Refresh").clicked() {
            if let Err(err) = start_annotation_fetch(annotation_fetch_task, config) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
        }
    });
    ui.horizontal(|ui| {
        ui.selectable_value(&mut annotations.draft_mode, DraftMode::Point, "Point");
        ui.selectable_value(&mut annotations.draft_mode, DraftMode::Polygon, "Polygon");
    });
    ui.label("Label");
    ui.text_edit_singleline(&mut annotations.draft_label);
    ui.label("Severity");
    ui.text_edit_singleline(&mut annotations.draft_severity);
    ui.label("Note");
    ui.text_edit_multiline(&mut annotations.draft_note);
    ui.collapsing("Display Filters", |ui| {
        ui.label("Label contains");
        ui.text_edit_singleline(&mut annotations.filter_label);
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut annotations.show_points, "Points");
            ui.checkbox(&mut annotations.show_polygons, "Polygons");
        });
        ui.label("Severity");
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(egui::Color32::from_rgb(209, 26, 26), "Critical");
            ui.checkbox(&mut annotations.show_critical, "");
            ui.colored_label(egui::Color32::from_rgb(242, 89, 26), "High");
            ui.checkbox(&mut annotations.show_high, "");
            ui.colored_label(egui::Color32::from_rgb(242, 191, 26), "Medium");
            ui.checkbox(&mut annotations.show_medium, "");
            ui.colored_label(egui::Color32::from_rgb(51, 204, 64), "Low");
            ui.checkbox(&mut annotations.show_low, "");
            ui.colored_label(egui::Color32::from_rgb(38, 217, 242), "Other");
            ui.checkbox(&mut annotations.show_other, "");
        });
        if ui.button("Reset Filters").clicked() {
            annotations.filter_label.clear();
            annotations.show_points = true;
            annotations.show_polygons = true;
            annotations.show_low = true;
            annotations.show_medium = true;
            annotations.show_high = true;
            annotations.show_critical = true;
            annotations.show_other = true;
        }
    });
    let has_scene = config.scene_id.is_some();
    let has_cursor_geo = cursor_map.geo_position.is_some();
    ui.horizontal(|ui| {
        let point_button_label = if annotations.selected_annotation_id.is_some()
            && annotations.draft_mode == DraftMode::Point
        {
            "Set Point From Cursor"
        } else {
            "Use Cursor As Point"
        };
        if ui
            .add_enabled(
                has_scene && has_cursor_geo,
                egui::Button::new(point_button_label),
            )
            .clicked()
        {
            set_draft_point_from_cursor(annotations, cursor_map.geo_position);
        }
        if ui
            .add_enabled(
                has_scene && has_cursor_geo && annotations.draft_mode == DraftMode::Polygon,
                egui::Button::new("Add Polygon Vertex"),
            )
            .clicked()
        {
            add_polygon_vertex_from_cursor(annotations, cursor_map.geo_position);
        }
        if ui.button("Clear Draft Geometry").clicked() {
            clear_annotation_draft_geometry(annotations);
        }
    });
    if annotations.draft_mode == DraftMode::Polygon {
        ui.small(format!(
            "Polygon vertices: {}",
            annotations.draft_polygon_vertices.len()
        ));
        if let Some(index) = annotations.hovered_draft_vertex_index {
            ui.small(format!("Hovered vertex: {}", index + 1));
            if ui.button("Remove Hovered Vertex").clicked() {
                remove_polygon_vertex_at_index(annotations, index);
                annotations.hovered_draft_vertex_index = None;
            }
        }
        if let Some(index) = annotations.hovered_draft_segment_index {
            ui.small(format!("Hovered edge after vertex {}", index + 1));
            ui.small("Left-click the highlighted edge on the map to insert a vertex.");
        }
        ui.small("Backspace or Ctrl+Z removes the last vertex.");
        ui.small("Right-click or Delete removes the hovered vertex.");
    } else {
        ui.small(if annotations.draft_point.is_some() {
            "Draft point is set"
        } else {
            "Draft point not set"
        });
    }
    ui.small("Esc clears the current draft.");
    let create_enabled = has_scene && draft_geometry(annotations).is_some();
    if ui
        .add_enabled(
            create_enabled && annotations.selected_annotation_id.is_none(),
            egui::Button::new(match annotations.draft_mode {
                DraftMode::Point => "Create Point Annotation",
                DraftMode::Polygon => "Create Polygon Annotation",
            }),
        )
        .clicked()
    {
        if let Some(geometry) = draft_geometry(annotations) {
            if let Err(err) = start_annotation_create(
                annotation_create_task,
                config,
                annotations.draft_label.clone(),
                annotations.draft_note.clone(),
                annotations.draft_severity.clone(),
                geometry,
            ) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
        }
    }
    let update_enabled = has_scene
        && annotations.selected_annotation_id.is_some()
        && draft_geometry(annotations).is_some();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(update_enabled, egui::Button::new("Update Selected"))
            .clicked()
        {
            if let (Some(annotation_id), Some(geometry)) = (
                annotations.selected_annotation_id.clone(),
                draft_geometry(annotations),
            ) {
                if let Err(err) = start_annotation_update(
                    annotation_update_task,
                    config,
                    &annotation_id,
                    annotations.draft_label.clone(),
                    annotations.draft_note.clone(),
                    annotations.draft_severity.clone(),
                    geometry,
                ) {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        }
        if ui
            .add_enabled(
                has_scene && annotations.selected_annotation_id.is_some(),
                egui::Button::new("Delete Selected"),
            )
            .clicked()
        {
            if let Some(annotation_id) = annotations.selected_annotation_id.clone() {
                if let Err(err) =
                    start_annotation_delete(annotation_delete_task, config, &annotation_id)
                {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        }
    });
    if !has_scene || !has_cursor_geo {
        ui.small(
            "Load a georeferenced scene and move the cursor over the map to draft annotations.",
        );
    }
    ui.separator();
    if let Some(selected) = selected_annotation(annotations) {
        ui.heading("Selected Annotation");
        ui.small(format!("ID: {}", selected.annotation_id));
        ui.small(format!(
            "Severity: {}",
            selected.severity.as_deref().unwrap_or("n/a")
        ));
        ui.small(format!(
            "Geometry: {}",
            match selected.geometry {
                shared::schemas::AnnotationGeometry::Point { .. } => "Point",
                shared::schemas::AnnotationGeometry::Polygon { .. } => "Polygon",
            }
        ));
        if let Some(note) = selected.note.as_deref() {
            ui.small(format!("Note: {}", note));
        }
        ui.small(format!("Created: {}", selected.created_at));
        ui.small(format!("Updated: {}", selected.updated_at));
        ui.separator();
    }

    let filtered_annotations: Vec<_> = annotations
        .items
        .iter()
        .filter(|annotation| annotation_matches_filters(annotation, annotations))
        .cloned()
        .collect();
    if filtered_annotations.is_empty() {
        ui.label("No annotations for this scene");
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            for annotation in filtered_annotations.iter().rev() {
                let selected = annotations.selected_annotation_id.as_deref()
                    == Some(annotation.annotation_id.as_str());
                let hovered = annotations.hovered_annotation_id.as_deref()
                    == Some(annotation.annotation_id.as_str());
                let response = ui.selectable_label(
                    selected,
                    format!(
                        "{} [{}]",
                        annotation.label,
                        annotation.severity.as_deref().unwrap_or("n/a")
                    ),
                );
                if response.clicked() {
                    annotations.selected_annotation_id = Some(annotation.annotation_id.clone());
                    load_annotation_into_draft(annotations, annotation);
                }
                if hovered {
                    ui.small("Hovered on map");
                }
                if let Some(note) = annotation.note.as_deref() {
                    ui.small(note);
                }
                ui.small(annotation.created_at.as_str());
                ui.separator();
            }
        });
}

fn render_status_bar(
    ctx: &egui::Context,
    config: &TileConfig,
    viewer_state: &ViewerState,
    manifest_state: &SceneManifestState,
    tile_state: &TileRenderState,
    cursor_map: &CursorMapState,
    annotations: &AnnotationOverlayState,
) {
    let status_message = tile_state.status.message();
    let dimension_text = if matches!(tile_state.status, TileStatus::Ready) {
        format!(
            "{} × {} px",
            tile_state.image_dimensions.x as u32, tile_state.image_dimensions.y as u32
        )
    } else {
        String::new()
    };
    let base_url = config.base_url.clone();
    let active_scene = config
        .scene_id
        .clone()
        .unwrap_or_else(|| "(none)".to_string());
    let active_field = manifest_state
        .field
        .as_ref()
        .map(|field| field.name.clone())
        .unwrap_or_else(|| "(none)".to_string());
    let active_layer = if manifest_state.products.is_empty() {
        "(none)".to_string()
    } else {
        manifest_state
            .products
            .get(viewer_state.selected_layer)
            .map(|p| p.kind.to_uppercase())
            .unwrap_or_else(|| "(invalid)".to_string())
    };
    let sensor = manifest_state
        .sensor
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let crop = manifest_state
        .field
        .as_ref()
        .and_then(|field| field.crop.clone())
        .unwrap_or_else(|| "n/a".to_string());
    let season = manifest_state
        .field
        .as_ref()
        .and_then(|field| field.season.clone())
        .unwrap_or_else(|| "n/a".to_string());
    let acquired_at = manifest_state
        .acquired_at
        .clone()
        .unwrap_or_else(|| "n/a".to_string());
    let source_dimensions = match (manifest_state.width, manifest_state.height) {
        (Some(width), Some(height)) => format!("{}x{}", width, height),
        _ => "n/a".to_string(),
    };
    let gps_text = manifest_state
        .gps_position
        .as_ref()
        .map(|gps| format!("{:.5}, {:.5}", gps.latitude, gps.longitude))
        .unwrap_or_else(|| "n/a".to_string());
    let georef_status = if manifest_state.geospatial.georeferenced {
        "yes"
    } else {
        "no"
    };
    let crs_text = manifest_state
        .geospatial
        .crs
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let center_text = manifest_state
        .geospatial
        .center
        .as_ref()
        .map(|gps| format!("{:.5}, {:.5}", gps.latitude, gps.longitude))
        .unwrap_or_else(|| "n/a".to_string());
    let extent_text = manifest_state
        .geospatial
        .extent
        .as_ref()
        .map(|extent| {
            format!(
                "{:.5}, {:.5} -> {:.5}, {:.5}",
                extent.min_lon, extent.min_lat, extent.max_lon, extent.max_lat
            )
        })
        .unwrap_or_else(|| "n/a".to_string());
    let cursor_text = cursor_map
        .geo_position
        .map(|(longitude, latitude)| format!("{:.5}, {:.5}", latitude, longitude))
        .unwrap_or_else(|| "n/a".to_string());
    let annotation_count = annotations.items.len();

    egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Hub: {}", base_url));
            ui.separator();
            ui.label(format!("Scene: {}", active_scene));
            ui.separator();
            ui.label(format!("Field: {}", active_field));
            ui.separator();
            ui.label(format!("Layer: {}", active_layer));
            ui.separator();
            ui.label(format!("Sensor: {}", sensor));
            ui.separator();
            ui.label(format!("Crop: {}", crop));
            ui.separator();
            ui.label(format!("Season: {}", season));
            ui.separator();
            ui.label(format!("Acquired: {}", acquired_at));
            ui.separator();
            ui.label(format!("Source: {}", source_dimensions));
            ui.separator();
            ui.label(format!("Bands: {}", manifest_state.bands.len()));
            ui.separator();
            ui.label(format!("GPS: {}", gps_text));
            ui.separator();
            ui.label(format!("Georef: {}", georef_status));
            ui.separator();
            ui.label(format!("CRS: {}", crs_text));
            ui.separator();
            ui.label(format!("Center: {}", center_text));
            ui.separator();
            ui.label(format!("Extent: {}", extent_text));
            ui.separator();
            ui.label(format!("Cursor: {}", cursor_text));
            ui.separator();
            ui.label(format!("Annotations: {}", annotation_count));
            ui.separator();
            ui.label(status_message);
            if !dimension_text.is_empty() {
                ui.separator();
                ui.label(dimension_text);
            }
        });
    });
}
