use crate::plugins::annotations::{
    add_polygon_vertex_from_cursor, annotation_matches_filters, build_annotation_commit_payload,
    clear_annotation_draft_geometry, clear_annotations, draft_geometry, load_annotation_into_draft,
    next_annotation_audit_id, remove_polygon_vertex_at_index, selected_annotation,
    set_draft_point_from_cursor, start_annotation_create, start_annotation_delete,
    start_annotation_fetch, start_annotation_update,
};
use crate::plugins::network::{
    clear_loaded_tiles, clear_manifest_state, start_farm_field_history_fetch,
    start_farm_list_fetch, start_field_import, start_field_list_fetch, start_field_scenes_fetch,
    start_manifest_fetch,
};
use crate::plugins::recommendations::{
    build_recommendation_create_payload, clear_recommendation_draft, clear_recommendations,
    load_recommendation_into_draft, recommendation_matches_filters,
    seed_recommendation_from_annotation, selected_recommendation, start_recommendation_create,
    start_recommendation_delete, start_recommendation_fetch, start_recommendation_update,
};
use crate::plugins::reports::{clear_reports, start_report_fetch, start_report_generate};
use crate::state::{
    active_product_selection, assert_manifest_layer_placement, layer_metadata_readout,
    select_catalog_scene, switch_active_product, AnnotationCreateTask, AnnotationDeleteTask,
    AnnotationFetchTask, AnnotationOverlayState, AnnotationUpdateTask, CursorMapState, DraftMode,
    FarmFieldHistoryFetchTask, FarmListFetchTask, FieldCatalogState, FieldImportState,
    FieldImportTask, FieldListFetchTask, FieldScenesFetchTask, ManifestFetchTask, MapViewState,
    ProductLegend, RecommendationCreateTask, RecommendationDeleteTask, RecommendationFetchTask,
    RecommendationOverlayState, RecommendationUpdateTask, ReportFetchTask, ReportGenerateTask,
    ReportOverlayState, SceneManifestState, ShapefileImportRequest, TileConfig, TileFetchTasks,
    TileRenderState, TileStatus, ViewerState,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use shared::schemas::{RecommendationPriority, RecommendationStatus};

pub struct ViewerUiPlugin;

impl Plugin for ViewerUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, render_ui);
    }
}

#[derive(SystemParam)]
struct CatalogUiState<'w, 's> {
    field_catalog: ResMut<'w, FieldCatalogState>,
    farm_list_task: ResMut<'w, FarmListFetchTask>,
    farm_field_history_task: ResMut<'w, FarmFieldHistoryFetchTask>,
    field_list_task: ResMut<'w, FieldListFetchTask>,
    field_scenes_task: ResMut<'w, FieldScenesFetchTask>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct FieldImportUiState<'w, 's> {
    field_import_state: ResMut<'w, FieldImportState>,
    field_import_task: ResMut<'w, FieldImportTask>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct SceneUiState<'w, 's> {
    manifest_state: ResMut<'w, SceneManifestState>,
    manifest_task: ResMut<'w, ManifestFetchTask>,
    tile_state: ResMut<'w, TileRenderState>,
    fetch_tasks: ResMut<'w, TileFetchTasks>,
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

#[derive(SystemParam)]
struct RecommendationUiState<'w, 's> {
    recommendations: ResMut<'w, RecommendationOverlayState>,
    recommendation_fetch_task: ResMut<'w, RecommendationFetchTask>,
    recommendation_create_task: ResMut<'w, RecommendationCreateTask>,
    recommendation_update_task: ResMut<'w, RecommendationUpdateTask>,
    recommendation_delete_task: ResMut<'w, RecommendationDeleteTask>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct ReportUiState<'w, 's> {
    reports: ResMut<'w, ReportOverlayState>,
    report_fetch_task: ResMut<'w, ReportFetchTask>,
    report_generate_task: ResMut<'w, ReportGenerateTask>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

fn render_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut viewer_state: ResMut<ViewerState>,
    mut config: ResMut<TileConfig>,
    mut catalog_ui: CatalogUiState,
    mut field_import_ui: FieldImportUiState,
    mut scene_ui: SceneUiState,
    cursor_map: Res<CursorMapState>,
    mut annotation_ui: AnnotationUiState,
    mut recommendation_ui: RecommendationUiState,
    mut report_ui: ReportUiState,
) {
    let field_catalog = &mut catalog_ui.field_catalog;
    let farm_list_task = &mut catalog_ui.farm_list_task;
    let farm_field_history_task = &mut catalog_ui.farm_field_history_task;
    let field_list_task = &mut catalog_ui.field_list_task;
    let field_scenes_task = &mut catalog_ui.field_scenes_task;
    let field_import_state = &mut field_import_ui.field_import_state;
    let field_import_task = &mut field_import_ui.field_import_task;
    let manifest_state = &mut scene_ui.manifest_state;
    let manifest_task = &mut scene_ui.manifest_task;
    let tile_state = &mut scene_ui.tile_state;
    let fetch_tasks = &mut scene_ui.fetch_tasks;
    let map_view = &mut scene_ui.map_view;
    let annotations = &mut annotation_ui.annotations;
    let annotation_fetch_task = &mut annotation_ui.annotation_fetch_task;
    let annotation_create_task = &mut annotation_ui.annotation_create_task;
    let annotation_update_task = &mut annotation_ui.annotation_update_task;
    let annotation_delete_task = &mut annotation_ui.annotation_delete_task;
    let recommendations = &mut recommendation_ui.recommendations;
    let recommendation_fetch_task = &mut recommendation_ui.recommendation_fetch_task;
    let recommendation_create_task = &mut recommendation_ui.recommendation_create_task;
    let recommendation_update_task = &mut recommendation_ui.recommendation_update_task;
    let recommendation_delete_task = &mut recommendation_ui.recommendation_delete_task;
    let reports = &mut report_ui.reports;
    let report_fetch_task = &mut report_ui.report_fetch_task;
    let report_generate_task = &mut report_ui.report_generate_task;

    egui::SidePanel::left("layers_panel").show(contexts.ctx_mut(), |ui| {
        render_fields_panel(
            ui,
            field_catalog,
            farm_list_task,
            farm_field_history_task,
            field_list_task,
            field_scenes_task,
            field_import_state,
            field_import_task,
            &mut viewer_state,
            &mut config,
            manifest_state,
            manifest_task,
            tile_state,
            fetch_tasks,
            map_view,
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
            recommendations,
            recommendation_fetch_task,
            recommendation_create_task,
            recommendation_update_task,
            recommendation_delete_task,
            reports,
            report_fetch_task,
            report_generate_task,
            &mut commands,
        );

        render_layers_panel(
            ui,
            manifest_state,
            &mut viewer_state,
            &mut config,
            &mut commands,
            fetch_tasks,
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
            fetch_tasks,
            map_view,
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
            recommendations,
            recommendation_fetch_task,
            recommendation_create_task,
            recommendation_update_task,
            recommendation_delete_task,
            reports,
            report_fetch_task,
            report_generate_task,
            &mut commands,
        );
        render_view_panel(ui, &mut viewer_state, map_view);
        render_annotations_panel(
            ui,
            &config,
            manifest_state,
            &cursor_map,
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
            tile_state,
        );
        render_recommendations_panel(
            ui,
            &config,
            annotations,
            recommendations,
            recommendation_fetch_task,
            recommendation_create_task,
            recommendation_update_task,
            recommendation_delete_task,
            tile_state,
        );
        render_reports_panel(
            ui,
            &config,
            reports,
            report_fetch_task,
            report_generate_task,
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
        recommendations,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_fields_panel(
    ui: &mut egui::Ui,
    field_catalog: &mut FieldCatalogState,
    farm_list_task: &mut FarmListFetchTask,
    farm_field_history_task: &mut FarmFieldHistoryFetchTask,
    field_list_task: &mut FieldListFetchTask,
    field_scenes_task: &mut FieldScenesFetchTask,
    field_import_state: &mut FieldImportState,
    field_import_task: &mut FieldImportTask,
    viewer_state: &mut ViewerState,
    config: &mut TileConfig,
    manifest_state: &mut SceneManifestState,
    manifest_task: &mut ManifestFetchTask,
    tile_state: &mut TileRenderState,
    fetch_tasks: &mut TileFetchTasks,
    map_view: &mut MapViewState,
    annotations: &mut AnnotationOverlayState,
    annotation_fetch_task: &mut AnnotationFetchTask,
    annotation_create_task: &mut AnnotationCreateTask,
    annotation_update_task: &mut AnnotationUpdateTask,
    annotation_delete_task: &mut AnnotationDeleteTask,
    recommendations: &mut RecommendationOverlayState,
    recommendation_fetch_task: &mut RecommendationFetchTask,
    recommendation_create_task: &mut RecommendationCreateTask,
    recommendation_update_task: &mut RecommendationUpdateTask,
    recommendation_delete_task: &mut RecommendationDeleteTask,
    reports: &mut ReportOverlayState,
    report_fetch_task: &mut ReportFetchTask,
    report_generate_task: &mut ReportGenerateTask,
    commands: &mut Commands,
) {
    ui.heading("Boundary Import");
    ui.label("Import a local polygon .shp file in geographic lon/lat.");
    ui.text_edit_singleline(&mut field_import_state.shapefile_path);
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut field_import_state.name_prefix);
    });
    ui.horizontal(|ui| {
        ui.label("Crop");
        ui.text_edit_singleline(&mut field_import_state.crop);
    });
    ui.horizontal(|ui| {
        ui.label("Season");
        ui.text_edit_singleline(&mut field_import_state.season);
    });
    ui.label("Notes");
    ui.text_edit_multiline(&mut field_import_state.notes);
    if ui.button("Import .shp").clicked() {
        let request = ShapefileImportRequest {
            path: field_import_state.shapefile_path.trim().to_string(),
            name_prefix: optional_text(&field_import_state.name_prefix),
            farm_id: field_catalog.selected_farm_id.clone(),
            crop: optional_text(&field_import_state.crop),
            season: optional_text(&field_import_state.season),
            notes: optional_text(&field_import_state.notes),
        };
        field_import_state.status_message = Some("Importing shapefile...".to_string());
        if let Err(err) = start_field_import(field_import_task, config, request) {
            field_import_state.status_message = Some(err.to_string());
            tile_state.status = TileStatus::Error(err.to_string());
        }
    }
    if let Some(message) = field_import_state.status_message.as_deref() {
        ui.small(message);
    }
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.heading("Farms");
        if ui.button("Refresh").clicked() {
            if let Err(err) = start_farm_list_fetch(farm_list_task, config) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
            if let Err(err) = start_field_list_fetch(field_list_task, config) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
        }
    });
    ui.separator();
    if field_catalog.farms.is_empty() {
        ui.label("No farms loaded");
    } else {
        let farms = field_catalog.farms.clone();
        for farm in farms {
            let selected = field_catalog.selected_farm_id.as_deref() == Some(&farm.farm_id);
            let response = ui.selectable_label(selected, farm.name.clone());
            if response.clicked() && !selected {
                field_catalog.selected_farm_id = Some(farm.farm_id.clone());
                field_catalog.selected_field_id = None;
                field_catalog.selected_scene_id = None;
                field_catalog.scenes.clear();
                field_catalog.season_groups.clear();
                if let Err(err) =
                    start_farm_field_history_fetch(farm_field_history_task, config, &farm.farm_id)
                {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
            response.on_hover_text(farm.notes.unwrap_or_else(|| "No farm notes".to_string()));
        }
    }
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.heading("Fields");
        if ui.button("Refresh Fields").clicked() {
            if let Err(err) = start_field_list_fetch(field_list_task, config) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
            if let Some(farm_id) = field_catalog.selected_farm_id.as_deref() {
                if let Err(err) =
                    start_farm_field_history_fetch(farm_field_history_task, config, farm_id)
                {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        }
    });
    ui.separator();
    if let Some(selected_farm_id) = field_catalog.selected_farm_id.as_deref() {
        if field_catalog
            .farms
            .iter()
            .all(|farm| farm.farm_id != selected_farm_id)
        {
            field_catalog.selected_farm_id = None;
        }
    }
    if field_catalog.selected_farm_id.is_some() {
        if field_catalog.season_groups.is_empty() {
            ui.label("No fields loaded for selected farm");
        } else {
            let groups = field_catalog.season_groups.clone();
            for group in groups {
                ui.collapsing(
                    group.season.as_deref().unwrap_or("Season Unspecified"),
                    |ui| {
                        for field in group.fields {
                            let selected =
                                field_catalog.selected_field_id.as_deref() == Some(&field.field_id);
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
                                if let Err(err) = start_field_scenes_fetch(
                                    field_scenes_task,
                                    config,
                                    &field.field_id,
                                ) {
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
                    },
                );
            }
        }
    } else if field_catalog.fields.is_empty() {
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

    ui.heading("Scene History");
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
                let selection = match select_catalog_scene(field_catalog, &scene) {
                    Ok(selection) => selection,
                    Err(err) => {
                        tile_state.status = TileStatus::Error(err.to_string());
                        continue;
                    }
                };
                viewer_state.scene_id_input = selection.scene_id.clone();
                config.scene_id = Some(selection.scene_id.clone());
                viewer_state.selected_layer = 0;
                config.product_kind = crate::state::DEFAULT_PRODUCT_KIND.to_string();
                clear_loaded_tiles(commands, tile_state);
                clear_manifest_state(manifest_state);
                clear_annotations(
                    annotations,
                    annotation_fetch_task,
                    annotation_create_task,
                    annotation_update_task,
                    annotation_delete_task,
                );
                clear_recommendations(
                    recommendations,
                    recommendation_fetch_task,
                    recommendation_create_task,
                    recommendation_update_task,
                    recommendation_delete_task,
                );
                clear_reports(reports, report_fetch_task, report_generate_task);
                fetch_tasks.0.clear();
                map_view.center = Vec2::ZERO;
                map_view.needs_fit = true;
                if let Err(err) = start_manifest_fetch(manifest_task, manifest_state, config) {
                    tile_state.status = TileStatus::Error(err.to_string());
                } else {
                    tile_state.status = TileStatus::Fetching;
                }
            }
            response.on_hover_text(format!(
                "Sensor: {}\nOwner: {}\nSeason: {}\nLinked: {}",
                scene.sensor,
                scene.owner.as_deref().unwrap_or("n/a"),
                scene.season_id.as_deref().unwrap_or("n/a"),
                scene.linked_at.as_deref().unwrap_or("unlinked")
            ));
        }
    }
    ui.add_space(8.0);
    if field_catalog.selected_field_id.is_some() {
        ui.small(format!(
            "Loaded-scene recommendations for this field: {}",
            recommendations.items.len()
        ));
        ui.add_space(8.0);
    }
}

fn optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn render_layers_panel(
    ui: &mut egui::Ui,
    manifest_state: &SceneManifestState,
    viewer_state: &mut ViewerState,
    config: &mut TileConfig,
    commands: &mut Commands,
    fetch_tasks: &mut TileFetchTasks,
    tile_state: &mut TileRenderState,
) {
    ui.heading("Layers");
    ui.separator();
    if manifest_state.products.is_empty() {
        ui.label("No layers loaded");
    } else {
        let placement_error = assert_manifest_layer_placement(
            &manifest_state.geospatial,
            manifest_state.width,
            manifest_state.height,
        )
        .err()
        .map(|err| err.to_string());
        if let Some(reason) = placement_error.as_ref() {
            ui.label(format!("Products unavailable for overlay: {reason}"));
        }
        for (idx, product) in manifest_state.products.iter().enumerate() {
            let label = format!(
                "{} ({}, {})",
                product.kind.to_uppercase(),
                product.filename,
                product.content_type
            );
            if placement_error.is_none() {
                let selected = viewer_state.selected_layer == idx;
                let response = ui.radio(selected, label);
                if response.clicked() && !selected {
                    match switch_active_product(manifest_state, viewer_state, config, idx) {
                        Ok(_) => {
                            clear_loaded_tiles(commands, tile_state);
                            fetch_tasks.0.clear();
                            tile_state.status = TileStatus::Fetching;
                        }
                        Err(err) => {
                            tile_state.status =
                                TileStatus::Error(format!("layer switch refused: {err}"));
                        }
                    }
                }
                response.on_hover_text(product.url_path.clone());
            } else {
                let selected = viewer_state.selected_layer == idx;
                let response = ui.add_enabled(false, egui::RadioButton::new(selected, label));
                response.on_hover_text(format!(
                    "{}\n{}",
                    product.url_path,
                    placement_error
                        .as_deref()
                        .unwrap_or("product is unplaceable")
                ));
            }
        }
        if placement_error.is_none() {
            if let Ok(selection) =
                active_product_selection(manifest_state, config, viewer_state.selected_layer)
            {
                render_product_legend(ui, &selection.legend);
            }
        }
    }
    ui.add_space(8.0);
}

fn render_product_legend(ui: &mut egui::Ui, legend: &ProductLegend) {
    ui.add_space(6.0);
    ui.small(format!(
        "{} legend ({})",
        legend.product_kind.to_uppercase(),
        legend.colormap
    ));
    ui.horizontal_wrapped(|ui| {
        for stop in &legend.stops {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter().rect_filled(
                rect,
                1.0,
                egui::Color32::from_rgba_unmultiplied(
                    stop.color[0],
                    stop.color[1],
                    stop.color[2],
                    stop.alpha,
                ),
            );
            ui.small(format!("{:.2}", stop.value));
        }
    });
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
    fetch_tasks: &mut TileFetchTasks,
    map_view: &mut MapViewState,
    annotations: &mut AnnotationOverlayState,
    annotation_fetch_task: &mut AnnotationFetchTask,
    annotation_create_task: &mut AnnotationCreateTask,
    annotation_update_task: &mut AnnotationUpdateTask,
    annotation_delete_task: &mut AnnotationDeleteTask,
    recommendations: &mut RecommendationOverlayState,
    recommendation_fetch_task: &mut RecommendationFetchTask,
    recommendation_create_task: &mut RecommendationCreateTask,
    recommendation_update_task: &mut RecommendationUpdateTask,
    recommendation_delete_task: &mut RecommendationDeleteTask,
    reports: &mut ReportOverlayState,
    report_fetch_task: &mut ReportFetchTask,
    report_generate_task: &mut ReportGenerateTask,
    commands: &mut Commands,
) {
    ui.heading("Scene");
    let open_recommendations = recommendations
        .items
        .iter()
        .filter(|recommendation| recommendation.status == RecommendationStatus::Open)
        .count();
    let reviewed_recommendations = recommendations
        .items
        .iter()
        .filter(|recommendation| recommendation.status == RecommendationStatus::Reviewed)
        .count();
    let completed_recommendations = recommendations
        .items
        .iter()
        .filter(|recommendation| recommendation.status == RecommendationStatus::Completed)
        .count();
    let dismissed_recommendations = recommendations
        .items
        .iter()
        .filter(|recommendation| recommendation.status == RecommendationStatus::Dismissed)
        .count();
    ui.small(format!(
        "Recommendations: {} total, {} open, {} reviewed, {} completed, {} dismissed",
        recommendations.items.len(),
        open_recommendations,
        reviewed_recommendations,
        completed_recommendations,
        dismissed_recommendations
    ));
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
        clear_loaded_tiles(commands, tile_state);
        tile_state.image_dimensions = Vec2::ZERO;
        tile_state.world_dimensions = Vec2::ZERO;
        clear_manifest_state(manifest_state);
        clear_annotations(
            annotations,
            annotation_fetch_task,
            annotation_create_task,
            annotation_update_task,
            annotation_delete_task,
        );
        clear_recommendations(
            recommendations,
            recommendation_fetch_task,
            recommendation_create_task,
            recommendation_update_task,
            recommendation_delete_task,
        );
        clear_reports(reports, report_fetch_task, report_generate_task);
        fetch_tasks.0.clear();
        manifest_task.0 = None;
        map_view.center = Vec2::ZERO;
        map_view.needs_fit = true;
        return;
    }

    config.scene_id = Some(trimmed.clone());
    field_catalog.selected_scene_id = Some(trimmed.clone());
    viewer_state.selected_layer = 0;
    config.product_kind = crate::state::DEFAULT_PRODUCT_KIND.to_string();
    clear_loaded_tiles(commands, tile_state);
    clear_annotations(
        annotations,
        annotation_fetch_task,
        annotation_create_task,
        annotation_update_task,
        annotation_delete_task,
    );
    clear_recommendations(
        recommendations,
        recommendation_fetch_task,
        recommendation_create_task,
        recommendation_update_task,
        recommendation_delete_task,
    );
    clear_reports(reports, report_fetch_task, report_generate_task);
    fetch_tasks.0.clear();
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
    manifest_state: &SceneManifestState,
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
    ui.label("Author");
    ui.text_edit_singleline(&mut annotations.draft_author);
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
            match build_annotation_commit_payload(
                annotations,
                manifest_state,
                geometry,
                next_annotation_audit_id(),
            ) {
                Ok(payload) => {
                    if let Err(err) =
                        start_annotation_create(annotation_create_task, config, payload)
                    {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                }
                Err(err) => {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
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
    if let Some(error) = annotations.last_error.as_ref() {
        ui.small(format!("Last save error: {error}"));
    }
    if !annotations.failed_commits.is_empty() {
        ui.small(format!(
            "Unsaved annotations: {}",
            annotations.failed_commits.len()
        ));
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
            "Author: {}",
            selected.author.as_deref().unwrap_or("n/a")
        ));
        ui.small(format!("CRS: {}", selected.crs.as_deref().unwrap_or("n/a")));
        ui.small(format!(
            "Audit: {}",
            selected.audit_id.as_deref().unwrap_or("n/a")
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

#[allow(clippy::too_many_arguments)]
fn render_recommendations_panel(
    ui: &mut egui::Ui,
    config: &TileConfig,
    annotations: &AnnotationOverlayState,
    recommendations: &mut RecommendationOverlayState,
    recommendation_fetch_task: &mut RecommendationFetchTask,
    recommendation_create_task: &mut RecommendationCreateTask,
    recommendation_update_task: &mut RecommendationUpdateTask,
    recommendation_delete_task: &mut RecommendationDeleteTask,
    tile_state: &mut TileRenderState,
) {
    ui.add_space(8.0);
    ui.heading("Recommendations");
    ui.horizontal(|ui| {
        ui.label(format!("Count: {}", recommendations.items.len()));
        if ui.button("Refresh").clicked() {
            if let Err(err) = start_recommendation_fetch(recommendation_fetch_task, config) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
        }
    });

    ui.collapsing("Filters", |ui| {
        ui.label("Status");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut recommendations.status_filter, None, "All");
            ui.selectable_value(
                &mut recommendations.status_filter,
                Some(RecommendationStatus::Open),
                "Open",
            );
            ui.selectable_value(
                &mut recommendations.status_filter,
                Some(RecommendationStatus::Reviewed),
                "Reviewed",
            );
            ui.selectable_value(
                &mut recommendations.status_filter,
                Some(RecommendationStatus::Completed),
                "Completed",
            );
            ui.selectable_value(
                &mut recommendations.status_filter,
                Some(RecommendationStatus::Dismissed),
                "Dismissed",
            );
        });
        ui.label("Priority");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut recommendations.priority_filter, None, "All");
            ui.selectable_value(
                &mut recommendations.priority_filter,
                Some(RecommendationPriority::Critical),
                "Critical",
            );
            ui.selectable_value(
                &mut recommendations.priority_filter,
                Some(RecommendationPriority::High),
                "High",
            );
            ui.selectable_value(
                &mut recommendations.priority_filter,
                Some(RecommendationPriority::Medium),
                "Medium",
            );
            ui.selectable_value(
                &mut recommendations.priority_filter,
                Some(RecommendationPriority::Low),
                "Low",
            );
        });
    });

    ui.label("Title");
    ui.text_edit_singleline(&mut recommendations.draft_title);
    ui.label("Category");
    ui.text_edit_singleline(&mut recommendations.draft_category);
    ui.label("Note");
    ui.text_edit_multiline(&mut recommendations.draft_note);
    ui.horizontal(|ui| {
        ui.label("Priority");
        egui::ComboBox::from_id_source("recommendation_priority")
            .selected_text(priority_label(recommendations.draft_priority))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut recommendations.draft_priority,
                    RecommendationPriority::Critical,
                    "Critical",
                );
                ui.selectable_value(
                    &mut recommendations.draft_priority,
                    RecommendationPriority::High,
                    "High",
                );
                ui.selectable_value(
                    &mut recommendations.draft_priority,
                    RecommendationPriority::Medium,
                    "Medium",
                );
                ui.selectable_value(
                    &mut recommendations.draft_priority,
                    RecommendationPriority::Low,
                    "Low",
                );
            });
    });
    ui.horizontal(|ui| {
        ui.label("Status");
        egui::ComboBox::from_id_source("recommendation_status")
            .selected_text(status_label(recommendations.draft_status))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut recommendations.draft_status,
                    RecommendationStatus::Open,
                    "Open",
                );
                ui.selectable_value(
                    &mut recommendations.draft_status,
                    RecommendationStatus::Reviewed,
                    "Reviewed",
                );
                ui.selectable_value(
                    &mut recommendations.draft_status,
                    RecommendationStatus::Completed,
                    "Completed",
                );
                ui.selectable_value(
                    &mut recommendations.draft_status,
                    RecommendationStatus::Dismissed,
                    "Dismissed",
                );
            });
    });

    let selected_annotation =
        annotations
            .selected_annotation_id
            .as_ref()
            .and_then(|annotation_id| {
                annotations
                    .items
                    .iter()
                    .find(|annotation| annotation.annotation_id == *annotation_id)
            });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                config.scene_id.is_some() && selected_annotation.is_some(),
                egui::Button::new("Link Selected Annotation"),
            )
            .clicked()
        {
            if let Some(annotation) = selected_annotation {
                seed_recommendation_from_annotation(
                    recommendations,
                    &annotation.annotation_id,
                    &annotation.label,
                );
            }
        }
        if ui.button("Clear Draft").clicked() {
            clear_recommendation_draft(recommendations);
        }
    });
    if recommendations.linked_annotation_ids.is_empty() {
        ui.small("No linked annotations");
    } else {
        ui.small(format!(
            "Linked annotations: {}",
            recommendations.linked_annotation_ids.join(", ")
        ));
    }

    let create_enabled = config.scene_id.is_some()
        && !recommendations.draft_title.trim().is_empty()
        && !recommendations.linked_annotation_ids.is_empty();
    if ui
        .add_enabled(
            create_enabled && recommendations.selected_recommendation_id.is_none(),
            egui::Button::new("Create Recommendation"),
        )
        .clicked()
    {
        match build_recommendation_create_payload(recommendations) {
            Ok(payload) => {
                if let Err(err) =
                    start_recommendation_create(recommendation_create_task, config, payload)
                {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
            Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
        }
    }

    let update_enabled =
        config.scene_id.is_some() && recommendations.selected_recommendation_id.is_some();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(update_enabled, egui::Button::new("Update Selected"))
            .clicked()
        {
            if let Some(recommendation_id) = recommendations.selected_recommendation_id.clone() {
                if let Err(err) = start_recommendation_update(
                    recommendation_update_task,
                    config,
                    &recommendation_id,
                    recommendations.draft_title.clone(),
                    recommendations.draft_note.clone(),
                    recommendations.draft_category.clone(),
                    recommendations.draft_priority,
                    recommendations.draft_status,
                    recommendations.linked_annotation_ids.clone(),
                ) {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        }
        if ui
            .add_enabled(update_enabled, egui::Button::new("Close Selected"))
            .clicked()
        {
            if let Some(recommendation_id) = recommendations.selected_recommendation_id.clone() {
                if let Err(err) = start_recommendation_update(
                    recommendation_update_task,
                    config,
                    &recommendation_id,
                    recommendations.draft_title.clone(),
                    recommendations.draft_note.clone(),
                    recommendations.draft_category.clone(),
                    recommendations.draft_priority,
                    RecommendationStatus::Completed,
                    recommendations.linked_annotation_ids.clone(),
                ) {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        }
        if ui
            .add_enabled(update_enabled, egui::Button::new("Delete Selected"))
            .clicked()
        {
            if let Some(recommendation_id) = recommendations.selected_recommendation_id.clone() {
                if let Err(err) = start_recommendation_delete(
                    recommendation_delete_task,
                    config,
                    &recommendation_id,
                ) {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        }
    });

    if let Some(selected) = selected_recommendation(recommendations) {
        ui.separator();
        ui.heading("Selected Recommendation");
        ui.small(format!("ID: {}", selected.recommendation_id));
        ui.small(format!("Priority: {}", priority_label(selected.priority)));
        ui.small(format!("Status: {}", status_label(selected.status)));
        if let Some(category) = selected.category.as_deref() {
            ui.small(format!("Category: {}", category));
        }
        if let Some(note) = selected.note.as_deref() {
            ui.small(format!("Note: {}", note));
        }
        ui.small(format!(
            "Linked annotations: {}",
            if selected.annotation_ids.is_empty() {
                "none".to_string()
            } else {
                selected.annotation_ids.join(", ")
            }
        ));
    }

    ui.separator();
    let filtered_recommendations: Vec<_> = recommendations
        .items
        .iter()
        .filter(|recommendation| recommendation_matches_filters(recommendation, recommendations))
        .cloned()
        .collect();
    if filtered_recommendations.is_empty() {
        ui.label("No recommendations for this scene");
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            for recommendation in &filtered_recommendations {
                let selected = recommendations.selected_recommendation_id.as_deref()
                    == Some(recommendation.recommendation_id.as_str());
                let response = ui.selectable_label(
                    selected,
                    format!(
                        "{} [{} / {}]",
                        recommendation.title,
                        status_label(recommendation.status),
                        priority_label(recommendation.priority)
                    ),
                );
                if response.clicked() {
                    recommendations.selected_recommendation_id =
                        Some(recommendation.recommendation_id.clone());
                    load_recommendation_into_draft(recommendations, recommendation);
                }
                if let Some(note) = recommendation.note.as_deref() {
                    ui.small(note);
                }
                ui.small(recommendation.created_at.as_str());
                ui.separator();
            }
        });
}

fn render_reports_panel(
    ui: &mut egui::Ui,
    config: &TileConfig,
    reports: &mut ReportOverlayState,
    report_fetch_task: &mut ReportFetchTask,
    report_generate_task: &mut ReportGenerateTask,
    tile_state: &mut TileRenderState,
) {
    ui.add_space(8.0);
    ui.heading("Reports");
    ui.horizontal(|ui| {
        ui.label(format!("Generated: {}", reports.items.len()));
        if ui.button("Refresh").clicked() {
            if let Err(err) = start_report_fetch(report_fetch_task, config) {
                tile_state.status = TileStatus::Error(err.to_string());
            }
        }
    });
    ui.label("Title");
    ui.text_edit_singleline(&mut reports.draft_title);
    if ui
        .add_enabled(
            config.scene_id.is_some() && !reports.draft_title.trim().is_empty(),
            egui::Button::new("Generate Report"),
        )
        .clicked()
    {
        if let Err(err) =
            start_report_generate(report_generate_task, config, reports.draft_title.clone())
        {
            tile_state.status = TileStatus::Error(err.to_string());
        }
    }
    ui.separator();
    if reports.items.is_empty() {
        ui.label("No reports generated for this scene");
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(180.0)
        .show(ui, |ui| {
            for report in &reports.items {
                ui.label(report.title.as_str());
                ui.small(format!(
                    "{} • {} annotations • {} recommendations",
                    report.created_at, report.annotation_count, report.recommendation_count
                ));
                ui.small(report.download_url.as_str());
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
    recommendations: &RecommendationOverlayState,
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
    let layer_readout = layer_metadata_readout(
        &manifest_state.geospatial,
        manifest_state.width,
        manifest_state.height,
    );
    let source_dimensions = layer_readout.dimensions.clone();
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
    let crs_text = layer_readout.crs;
    let center_text = manifest_state
        .geospatial
        .center
        .as_ref()
        .map(|gps| format!("{:.5}, {:.5}", gps.latitude, gps.longitude))
        .unwrap_or_else(|| "n/a".to_string());
    let extent_text = layer_readout.extent;
    let resolution_text = layer_readout.resolution;
    let cursor_text = cursor_readout_text(cursor_map);
    let annotation_count = annotations.items.len();
    let recommendation_count = recommendations.items.len();
    let open_recommendations = recommendations
        .items
        .iter()
        .filter(|recommendation| recommendation.status == RecommendationStatus::Open)
        .count();
    let tile_counts = format!(
        "Tiles: {}/{} ready, {} loading, {} missing, {} failed",
        tile_state.ready_tile_count(),
        tile_state.visible_tiles.len(),
        tile_state.loading_tile_count(),
        tile_state.missing_tile_count(),
        tile_state.failed_tile_count()
    );

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
            ui.label(format!("Resolution: {}", resolution_text));
            ui.separator();
            ui.label(format!("Cursor: {}", cursor_text));
            ui.separator();
            ui.label(format!("Annotations: {}", annotation_count));
            ui.separator();
            ui.label(format!(
                "Recommendations: {} ({} open)",
                recommendation_count, open_recommendations
            ));
            ui.separator();
            ui.label(tile_counts);
            ui.separator();
            ui.label(status_message);
            if !dimension_text.is_empty() {
                ui.separator();
                ui.label(dimension_text);
            }
        });
    });
}

fn cursor_readout_text(cursor_map: &CursorMapState) -> String {
    match (cursor_map.geo_position, cursor_map.world_position) {
        (Some((longitude, latitude)), _) => format!("{:.5}, {:.5}", latitude, longitude),
        (None, Some(_)) => "no georeference".to_string(),
        (None, None) => "n/a".to_string(),
    }
}

fn priority_label(priority: RecommendationPriority) -> &'static str {
    match priority {
        RecommendationPriority::Critical => "Critical",
        RecommendationPriority::High => "High",
        RecommendationPriority::Medium => "Medium",
        RecommendationPriority::Low => "Low",
    }
}

fn status_label(status: RecommendationStatus) -> &'static str {
    match status {
        RecommendationStatus::Open => "Open",
        RecommendationStatus::Reviewed => "Reviewed",
        RecommendationStatus::Completed => "Completed",
        RecommendationStatus::Dismissed => "Dismissed",
        RecommendationStatus::Closed => "Closed",
    }
}

#[cfg(test)]
mod tests {
    use super::cursor_readout_text;
    use crate::state::CursorMapState;
    use bevy::prelude::Vec2;

    #[test]
    fn cursor_readout_formats_live_lat_lon() {
        let cursor = CursorMapState {
            world_position: Some(Vec2::new(10.0, 20.0)),
            geo_position: Some((-88.5, 40.25)),
        };

        assert_eq!(cursor_readout_text(&cursor), "40.25000, -88.50000");
    }

    #[test]
    fn cursor_readout_reports_no_georeference_when_world_position_has_no_geo() {
        let cursor = CursorMapState {
            world_position: Some(Vec2::new(10.0, 20.0)),
            geo_position: None,
        };

        assert_eq!(cursor_readout_text(&cursor), "no georeference");
    }

    #[test]
    fn cursor_readout_reports_na_without_cursor() {
        let cursor = CursorMapState::default();

        assert_eq!(cursor_readout_text(&cursor), "n/a");
    }
}
