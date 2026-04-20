use anyhow::{Context, Result};
use bevy::ecs::system::SystemParam;
use bevy::tasks::{IoTaskPool, Task};
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        texture::ImageSampler,
    },
    window::WindowResolution,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use futures_lite::future;
use image::{self, DynamicImage};
use serde::Deserialize;
use shared::schemas::{AnnotationGeometry, AnnotationRecord, FieldRecord, GeoPoint, GpsCoords};
use tracing::info;

const APP_TITLE: &str = "Geo Viewer";
const DEFAULT_PRODUCT_KIND: &str = "ndvi";
const MAP_UNITS_PER_DEGREE: f32 = 10_000.0;

#[derive(Resource)]
struct ViewerState {
    selected_layer: usize,
    zoom_level: f32,
    scene_id_input: String,
}

#[derive(Resource)]
struct TileConfig {
    base_url: String,
    scene_id: Option<String>,
    product_kind: String,
}

#[derive(Resource, Default)]
struct FieldListFetchTask(Option<Task<anyhow::Result<Vec<FieldRecord>>>>);

#[derive(Resource, Default)]
struct FieldScenesFetchTask(Option<Task<anyhow::Result<Vec<FieldSceneSummary>>>>);

#[derive(Resource, Default)]
struct ManifestFetchTask(Option<Task<anyhow::Result<SceneManifest>>>);

#[derive(Resource, Default)]
struct TileFetchTask(Option<Task<anyhow::Result<FetchedTile>>>);

#[derive(Resource, Default)]
struct AnnotationFetchTask(Option<Task<anyhow::Result<Vec<AnnotationRecord>>>>);

#[derive(Resource, Default)]
struct AnnotationCreateTask(Option<Task<anyhow::Result<AnnotationRecord>>>);

#[derive(Resource, Default)]
struct AnnotationUpdateTask(Option<Task<anyhow::Result<AnnotationRecord>>>);

#[derive(Resource, Default)]
struct AnnotationDeleteTask(Option<Task<anyhow::Result<String>>>);

struct FetchedTile {
    image: DynamicImage,
}

#[derive(Debug, Clone, Deserialize)]
struct FieldSceneSummary {
    scene_id: String,
    sensor: String,
    acquired_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SceneManifest {
    scene_id: String,
    sensor: Option<String>,
    acquired_at: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    bands: Vec<String>,
    gps_position: Option<GpsCoords>,
    data_path: Option<String>,
    field: Option<FieldRecord>,
    geospatial: SceneGeospatialMetadata,
    available_products: Vec<SceneProduct>,
}

#[derive(Debug, Clone, Deserialize)]
struct SceneProduct {
    kind: String,
    filename: String,
    content_type: String,
    url_path: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SceneGeospatialMetadata {
    georeferenced: bool,
    crs: Option<String>,
    center: Option<GpsCoords>,
    extent: Option<SceneExtent>,
}

#[derive(Debug, Clone, Deserialize)]
struct SceneExtent {
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
}

#[derive(Resource, Default)]
struct SceneManifestState {
    scene_id: Option<String>,
    sensor: Option<String>,
    acquired_at: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    bands: Vec<String>,
    gps_position: Option<GpsCoords>,
    data_path: Option<String>,
    field: Option<FieldRecord>,
    geospatial: SceneGeospatialMetadata,
    products: Vec<SceneProduct>,
}

#[derive(Resource, Default)]
struct FieldCatalogState {
    fields: Vec<FieldRecord>,
    scenes: Vec<FieldSceneSummary>,
    selected_field_id: Option<String>,
    selected_scene_id: Option<String>,
}

#[derive(Resource)]
struct TileRenderState {
    entity: Option<Entity>,
    handle: Option<Handle<Image>>,
    image_dimensions: Vec2,
    world_dimensions: Vec2,
    status: TileStatus,
}

#[derive(Resource)]
struct MapViewState {
    center: Vec2,
    base_scale: f32,
    needs_fit: bool,
}

#[derive(Resource, Default)]
struct CursorMapState {
    world_position: Option<Vec2>,
    geo_position: Option<(f64, f64)>,
}

#[derive(Resource, Default)]
struct AnnotationOverlayState {
    items: Vec<AnnotationRecord>,
    selected_annotation_id: Option<String>,
    draft_label: String,
    draft_note: String,
    draft_severity: String,
    draft_mode: DraftMode,
    draft_point: Option<GeoPoint>,
    draft_polygon_vertices: Vec<GeoPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DraftMode {
    #[default]
    Point,
    Polygon,
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

#[derive(Debug, Clone)]
enum TileStatus {
    Idle,
    Fetching,
    Ready,
    MissingScene,
    Error(String),
}

impl TileStatus {
    fn message(&self) -> String {
        match self {
            TileStatus::Idle => "Idle".to_string(),
            TileStatus::Fetching => "Fetching tile data…".to_string(),
            TileStatus::Ready => "Tile ready".to_string(),
            TileStatus::MissingScene => "Enter a scene ID to load a product".to_string(),
            TileStatus::Error(err) => format!("Error: {}", err),
        }
    }
}

#[derive(Component)]
struct TileDisplay;

#[derive(Component)]
struct MapCamera;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let base_url =
        std::env::var("GEO_HUB_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let scene_id_env = std::env::var("GEO_VIEWER_SCENE_ID")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let initial_status = if scene_id_env.is_some() {
        TileStatus::Idle
    } else {
        TileStatus::MissingScene
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: APP_TITLE.into(),
                        resolution: WindowResolution::new(1600.0, 900.0),
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin)
        .insert_resource(ViewerState {
            selected_layer: 0,
            zoom_level: 1.0,
            scene_id_input: scene_id_env.clone().unwrap_or_default(),
        })
        .insert_resource(TileConfig {
            base_url,
            scene_id: scene_id_env,
            product_kind: DEFAULT_PRODUCT_KIND.to_string(),
        })
        .insert_resource(FieldListFetchTask::default())
        .insert_resource(FieldScenesFetchTask::default())
        .insert_resource(ManifestFetchTask::default())
        .insert_resource(SceneManifestState::default())
        .insert_resource(FieldCatalogState::default())
        .insert_resource(TileFetchTask::default())
        .insert_resource(AnnotationFetchTask::default())
        .insert_resource(AnnotationCreateTask::default())
        .insert_resource(AnnotationUpdateTask::default())
        .insert_resource(AnnotationDeleteTask::default())
        .insert_resource(TileRenderState {
            entity: None,
            handle: None,
            image_dimensions: Vec2::ZERO,
            world_dimensions: Vec2::ZERO,
            status: initial_status,
        })
        .insert_resource(MapViewState {
            center: Vec2::ZERO,
            base_scale: 1.0,
            needs_fit: true,
        })
        .insert_resource(CursorMapState::default())
        .insert_resource(AnnotationOverlayState {
            draft_label: "Issue".to_string(),
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                render_ui,
                poll_field_list_fetch,
                poll_field_scenes_fetch,
                poll_manifest_fetch,
                poll_tile_fetch,
                poll_annotation_fetch,
                poll_annotation_create,
            ),
        )
        .add_systems(
            Update,
            (
                poll_annotation_update,
                poll_annotation_delete,
                sync_map_camera,
                update_cursor_map_state,
                render_field_boundary,
                render_annotations,
            ),
        )
        .run();

    Ok(())
}

fn setup(
    mut commands: Commands,
    config: Res<TileConfig>,
    mut field_list_task: ResMut<FieldListFetchTask>,
    mut manifest_task: ResMut<ManifestFetchTask>,
    mut manifest_state: ResMut<SceneManifestState>,
    mut fetch_task: ResMut<TileFetchTask>,
    mut annotation_fetch_task: ResMut<AnnotationFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
) {
    commands.spawn((Camera2dBundle::default(), MapCamera));

    if let Err(err) = start_field_list_fetch(&mut field_list_task, &config) {
        tile_state.status = TileStatus::Error(err.to_string());
    }

    if config.scene_id.is_some() {
        if let Err(err) = start_manifest_fetch(&mut manifest_task, &mut manifest_state, &config) {
            tile_state.status = TileStatus::Error(err.to_string());
        }
        fetch_task.0 = None;
        annotation_fetch_task.0 = None;
    }
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
    let mut field_list_task = &mut catalog_ui.field_list_task;
    let mut field_scenes_task = &mut catalog_ui.field_scenes_task;
    let mut manifest_state = &mut scene_ui.manifest_state;
    let mut manifest_task = &mut scene_ui.manifest_task;
    let mut tile_state = &mut scene_ui.tile_state;
    let mut fetch_task = &mut scene_ui.fetch_task;
    let map_view = &mut scene_ui.map_view;
    let mut annotations = &mut annotation_ui.annotations;
    let mut annotation_fetch_task = &mut annotation_ui.annotation_fetch_task;
    let mut annotation_create_task = &mut annotation_ui.annotation_create_task;
    let mut annotation_update_task = &mut annotation_ui.annotation_update_task;
    let mut annotation_delete_task = &mut annotation_ui.annotation_delete_task;

    egui::SidePanel::left("layers_panel").show(contexts.ctx_mut(), |ui| {
        ui.horizontal(|ui| {
            ui.heading("Fields");
            if ui.button("Refresh").clicked() {
                if let Err(err) = start_field_list_fetch(&mut field_list_task, &config) {
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
                        start_field_scenes_fetch(&mut field_scenes_task, &config, &field.field_id)
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
                    config.product_kind = DEFAULT_PRODUCT_KIND.to_string();
                    clear_loaded_tile(&mut commands, &mut tile_state);
                    clear_manifest_state(&mut manifest_state);
                    clear_annotations(
                        &mut annotations,
                        &mut annotation_fetch_task,
                        &mut annotation_create_task,
                        &mut annotation_update_task,
                        &mut annotation_delete_task,
                    );
                    fetch_task.0 = None;
                    map_view.center = Vec2::ZERO;
                    map_view.needs_fit = true;
                    if let Err(err) =
                        start_manifest_fetch(&mut manifest_task, &mut manifest_state, &config)
                    {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                }
                response.on_hover_text(format!("Sensor: {}", scene.sensor));
            }
        }
        ui.add_space(8.0);

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
                    if let Err(err) = start_tile_fetch(&mut fetch_task, &mut tile_state, &config) {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                }
                response.on_hover_text(product.url_path.clone());
            }
        }
        ui.add_space(8.0);

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

        if load_requested {
            let trimmed = viewer_state.scene_id_input.trim().to_string();
            if trimmed.is_empty() {
                config.scene_id = None;
                field_catalog.selected_scene_id = None;
                config.product_kind = DEFAULT_PRODUCT_KIND.to_string();
                tile_state.status = TileStatus::MissingScene;
                clear_loaded_tile(&mut commands, &mut tile_state);
                clear_manifest_state(&mut manifest_state);
                clear_annotations(
                    &mut annotations,
                    &mut annotation_fetch_task,
                    &mut annotation_create_task,
                    &mut annotation_update_task,
                    &mut annotation_delete_task,
                );
                fetch_task.0 = None;
                manifest_task.0 = None;
                map_view.center = Vec2::ZERO;
                map_view.needs_fit = true;
            } else {
                config.scene_id = Some(trimmed.clone());
                field_catalog.selected_scene_id = Some(trimmed.clone());
                viewer_state.selected_layer = 0;
                config.product_kind = DEFAULT_PRODUCT_KIND.to_string();
                clear_loaded_tile(&mut commands, &mut tile_state);
                clear_annotations(
                    &mut annotations,
                    &mut annotation_fetch_task,
                    &mut annotation_create_task,
                    &mut annotation_update_task,
                    &mut annotation_delete_task,
                );
                fetch_task.0 = None;
                map_view.center = Vec2::ZERO;
                map_view.needs_fit = true;
                if let Err(err) =
                    start_manifest_fetch(&mut manifest_task, &mut manifest_state, &config)
                {
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        }

        ui.add_space(8.0);
        ui.heading("View");
        ui.label("Zoom");
        ui.add(egui::Slider::new(&mut viewer_state.zoom_level, 0.2..=5.0).logarithmic(true));
        ui.small("Mouse wheel zooms. Middle-drag pans the map.");
        if ui.button("Reset View").clicked() {
            map_view.center = Vec2::ZERO;
            map_view.needs_fit = true;
        }

        ui.add_space(8.0);
        ui.heading("Annotations");
        ui.horizontal(|ui| {
            ui.label(format!("Count: {}", annotations.items.len()));
            if ui.button("Refresh").clicked() {
                if let Err(err) = start_annotation_fetch(&mut annotation_fetch_task, &config) {
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
                set_draft_point_from_cursor(&mut annotations, cursor_map.geo_position);
            }
            if ui
                .add_enabled(
                    has_scene && has_cursor_geo && annotations.draft_mode == DraftMode::Polygon,
                    egui::Button::new("Add Polygon Vertex"),
                )
                .clicked()
            {
                add_polygon_vertex_from_cursor(&mut annotations, cursor_map.geo_position);
            }
            if ui.button("Clear Draft Geometry").clicked() {
                clear_annotation_draft_geometry(&mut annotations);
            }
        });
        if annotations.draft_mode == DraftMode::Polygon {
            ui.small(format!(
                "Polygon vertices: {}",
                annotations.draft_polygon_vertices.len()
            ));
        } else {
            ui.small(if annotations.draft_point.is_some() {
                "Draft point is set"
            } else {
                "Draft point not set"
            });
        }
        let create_enabled = has_scene && draft_geometry(&annotations).is_some();
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
            if let Some(geometry) = draft_geometry(&annotations) {
                if let Err(err) = start_annotation_create(
                    &mut annotation_create_task,
                    &config,
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
            && draft_geometry(&annotations).is_some();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(update_enabled, egui::Button::new("Update Selected"))
                .clicked()
            {
                if let (Some(annotation_id), Some(geometry)) = (
                    annotations.selected_annotation_id.clone(),
                    draft_geometry(&annotations),
                ) {
                    if let Err(err) = start_annotation_update(
                        &mut annotation_update_task,
                        &config,
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
                    if let Err(err) = start_annotation_delete(
                        &mut annotation_delete_task,
                        &config,
                        &annotation_id,
                    ) {
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
        if annotations.items.is_empty() {
            ui.label("No annotations for this scene");
        } else {
            let annotation_items = annotations.items.clone();
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for annotation in annotation_items.iter().rev() {
                        let selected = annotations.selected_annotation_id.as_deref()
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
                            annotations.selected_annotation_id =
                                Some(annotation.annotation_id.clone());
                            load_annotation_into_draft(&mut annotations, annotation);
                        }
                        if let Some(note) = annotation.note.as_deref() {
                            ui.small(note);
                        }
                        ui.small(annotation.created_at.as_str());
                        ui.separator();
                    }
                });
        }
    });

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

    egui::TopBottomPanel::top("status_bar").show(contexts.ctx_mut(), |ui| {
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

fn poll_manifest_fetch(
    mut manifest_task: ResMut<ManifestFetchTask>,
    mut manifest_state: ResMut<SceneManifestState>,
    mut config: ResMut<TileConfig>,
    mut viewer_state: ResMut<ViewerState>,
    mut field_catalog: ResMut<FieldCatalogState>,
    mut field_scenes_task: ResMut<FieldScenesFetchTask>,
    mut fetch_task: ResMut<TileFetchTask>,
    mut annotation_fetch_task: ResMut<AnnotationFetchTask>,
    mut annotations: ResMut<AnnotationOverlayState>,
    mut tile_state: ResMut<TileRenderState>,
    mut map_view: ResMut<MapViewState>,
) {
    if let Some(mut task) = manifest_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(manifest) => {
                    let scene_id = manifest.scene_id.clone();
                    let linked_field = manifest.field.clone();
                    let products = manifest.available_products;
                    manifest_state.scene_id = Some(scene_id.clone());
                    manifest_state.sensor = manifest.sensor;
                    manifest_state.acquired_at = manifest.acquired_at;
                    manifest_state.width = manifest.width;
                    manifest_state.height = manifest.height;
                    manifest_state.bands = manifest.bands;
                    manifest_state.gps_position = manifest.gps_position;
                    manifest_state.data_path = manifest.data_path;
                    manifest_state.field = linked_field.clone();
                    manifest_state.geospatial = manifest.geospatial;
                    manifest_state.products = products;
                    field_catalog.selected_scene_id = Some(scene_id);
                    if let Some(field) = linked_field {
                        let field_changed =
                            field_catalog.selected_field_id.as_deref() != Some(&field.field_id);
                        field_catalog.selected_field_id = Some(field.field_id.clone());
                        if !field_catalog
                            .fields
                            .iter()
                            .any(|known| known.field_id == field.field_id)
                        {
                            field_catalog.fields.push(field.clone());
                            field_catalog
                                .fields
                                .sort_by(|left, right| left.name.cmp(&right.name));
                        }
                        if field_changed {
                            if let Err(err) = start_field_scenes_fetch(
                                &mut field_scenes_task,
                                &config,
                                &field.field_id,
                            ) {
                                tile_state.status = TileStatus::Error(err.to_string());
                            }
                        }
                    }

                    if manifest_state.products.is_empty() {
                        tile_state.status =
                            TileStatus::Error("Scene has no available products".to_string());
                        return;
                    }

                    let selected_idx = manifest_state
                        .products
                        .iter()
                        .position(|product| product.kind == config.product_kind)
                        .unwrap_or(0);
                    viewer_state.selected_layer = selected_idx;
                    config.product_kind = manifest_state.products[selected_idx].kind.clone();
                    map_view.center = Vec2::ZERO;
                    map_view.needs_fit = true;
                    annotations.items.clear();

                    if let Err(err) = start_tile_fetch(&mut fetch_task, &mut tile_state, &config) {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                    if let Err(err) = start_annotation_fetch(&mut annotation_fetch_task, &config) {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                }
                Err(err) => {
                    manifest_state.scene_id = config.scene_id.clone();
                    manifest_state.sensor = None;
                    manifest_state.acquired_at = None;
                    manifest_state.width = None;
                    manifest_state.height = None;
                    manifest_state.bands.clear();
                    manifest_state.gps_position = None;
                    manifest_state.data_path = None;
                    manifest_state.field = None;
                    manifest_state.geospatial = SceneGeospatialMetadata::default();
                    manifest_state.products.clear();
                    annotations.items.clear();
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        } else {
            manifest_task.0 = Some(task);
        }
    }
}

fn poll_field_list_fetch(
    mut field_list_task: ResMut<FieldListFetchTask>,
    mut field_catalog: ResMut<FieldCatalogState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = field_list_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(fields) => {
                    field_catalog.fields = fields;
                    if let Some(selected_field_id) = field_catalog.selected_field_id.as_ref() {
                        if !field_catalog
                            .fields
                            .iter()
                            .any(|field| &field.field_id == selected_field_id)
                        {
                            field_catalog.selected_field_id = None;
                            field_catalog.scenes.clear();
                        }
                    }
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            field_list_task.0 = Some(task);
        }
    }
}

fn poll_field_scenes_fetch(
    mut field_scenes_task: ResMut<FieldScenesFetchTask>,
    mut field_catalog: ResMut<FieldCatalogState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = field_scenes_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(scenes) => {
                    field_catalog.scenes = scenes;
                    if let Some(selected_scene_id) = field_catalog.selected_scene_id.as_ref() {
                        if !field_catalog
                            .scenes
                            .iter()
                            .any(|scene| &scene.scene_id == selected_scene_id)
                        {
                            field_catalog.selected_scene_id = None;
                        }
                    }
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            field_scenes_task.0 = Some(task);
        }
    }
}

fn poll_tile_fetch(
    mut commands: Commands,
    mut fetch_task: ResMut<TileFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
    manifest_state: Res<SceneManifestState>,
    mut map_view: ResMut<MapViewState>,
    mut textures: ResMut<Assets<Image>>,
) {
    if let Some(mut task) = fetch_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(fetched) => {
                    let rgba = fetched.image.to_rgba8();
                    let (width, height) = rgba.dimensions();
                    let world_dimensions = manifest_state
                        .geospatial
                        .extent
                        .as_ref()
                        .map(extent_world_size)
                        .unwrap_or_else(|| Vec2::new(width as f32, height as f32));

                    let mut bevy_image = Image::new(
                        Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        rgba.into_raw(),
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::default(),
                    );
                    bevy_image.texture_descriptor.usage =
                        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
                    bevy_image.sampler = ImageSampler::nearest();

                    let handle = textures.add(bevy_image);

                    if let Some(entity) = tile_state.entity.take() {
                        commands.entity(entity).despawn_recursive();
                    }

                    let entity = commands
                        .spawn((
                            SpriteBundle {
                                texture: handle.clone(),
                                sprite: Sprite {
                                    custom_size: Some(world_dimensions),
                                    ..default()
                                },
                                transform: Transform::from_translation(Vec3::ZERO),
                                ..default()
                            },
                            TileDisplay,
                        ))
                        .id();

                    tile_state.entity = Some(entity);
                    tile_state.handle = Some(handle);
                    tile_state.image_dimensions = Vec2::new(width as f32, height as f32);
                    tile_state.world_dimensions = world_dimensions;
                    tile_state.status = TileStatus::Ready;
                    map_view.center = Vec2::ZERO;
                    map_view.needs_fit = true;
                    info!(width, height, "tile loaded");
                }
                Err(err) => {
                    tile_state.status = TileStatus::Error(err.to_string());
                    fetch_task.0 = None;
                }
            }
        } else {
            fetch_task.0 = Some(task);
        }
    }
}

fn poll_annotation_fetch(
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

fn poll_annotation_create(
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

fn poll_annotation_update(
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

fn poll_annotation_delete(
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
                    tile_state.status = TileStatus::Ready;
                }
                Err(err) => tile_state.status = TileStatus::Error(err.to_string()),
            }
        } else {
            annotation_delete_task.0 = Some(task);
        }
    }
}

fn sync_map_camera(
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

fn update_cursor_map_state(
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

fn render_field_boundary(mut gizmos: Gizmos, manifest_state: Res<SceneManifestState>) {
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

fn render_annotations(
    mut gizmos: Gizmos,
    manifest_state: Res<SceneManifestState>,
    annotations: Res<AnnotationOverlayState>,
) {
    let Some(extent) = manifest_state.geospatial.extent.as_ref() else {
        return;
    };

    for annotation in &annotations.items {
        let is_selected = annotations.selected_annotation_id.as_deref()
            == Some(annotation.annotation_id.as_str());
        let color = annotation_color(annotation.severity.as_deref(), is_selected);
        match &annotation.geometry {
            AnnotationGeometry::Point { coordinate } => {
                draw_point_marker(
                    &mut gizmos,
                    geo_to_scene_local(extent, coordinate.longitude, coordinate.latitude),
                    color,
                );
            }
            AnnotationGeometry::Polygon { coordinates } => {
                draw_polygon_outline(&mut gizmos, extent, coordinates, color);
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
        }
    }
}

fn clear_loaded_tile(commands: &mut Commands, tile_state: &mut TileRenderState) {
    if let Some(entity) = tile_state.entity.take() {
        commands.entity(entity).despawn_recursive();
    }
    tile_state.handle = None;
    tile_state.image_dimensions = Vec2::ZERO;
    tile_state.world_dimensions = Vec2::ZERO;
}

fn extent_world_size(extent: &SceneExtent) -> Vec2 {
    Vec2::new(
        ((extent.max_lon - extent.min_lon) as f32).abs() * MAP_UNITS_PER_DEGREE,
        ((extent.max_lat - extent.min_lat) as f32).abs() * MAP_UNITS_PER_DEGREE,
    )
}

fn geo_to_scene_local(extent: &SceneExtent, longitude: f64, latitude: f64) -> Vec2 {
    let center_lon = (extent.min_lon + extent.max_lon) / 2.0;
    let center_lat = (extent.min_lat + extent.max_lat) / 2.0;
    Vec2::new(
        ((longitude - center_lon) as f32) * MAP_UNITS_PER_DEGREE,
        ((latitude - center_lat) as f32) * MAP_UNITS_PER_DEGREE,
    )
}

fn scene_local_to_geo(extent: &SceneExtent, world_position: Vec2) -> (f64, f64) {
    let center_lon = (extent.min_lon + extent.max_lon) / 2.0;
    let center_lat = (extent.min_lat + extent.max_lat) / 2.0;
    let longitude = center_lon + (world_position.x as f64 / MAP_UNITS_PER_DEGREE as f64);
    let latitude = center_lat + (world_position.y as f64 / MAP_UNITS_PER_DEGREE as f64);
    (longitude, latitude)
}

fn annotation_color(severity: Option<&str>, is_selected: bool) -> Color {
    if is_selected {
        return Color::srgb(1.0, 0.95, 0.25);
    }

    match severity.map(|value| value.to_ascii_lowercase()) {
        Some(level) if level == "critical" => Color::srgb(0.82, 0.10, 0.10),
        Some(level) if level == "high" => Color::srgb(0.95, 0.35, 0.10),
        Some(level) if level == "medium" => Color::srgb(0.95, 0.75, 0.10),
        Some(level) if level == "low" => Color::srgb(0.20, 0.80, 0.25),
        _ => Color::srgb(0.15, 0.85, 0.95),
    }
}

fn draw_point_marker(gizmos: &mut Gizmos, center: Vec2, color: Color) {
    let half_size = 20.0;
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

fn draw_polygon_outline(
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
        draw_point_marker(gizmos, points[0], color);
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

fn set_draft_point_from_cursor(
    annotations: &mut AnnotationOverlayState,
    geo_position: Option<(f64, f64)>,
) {
    let Some((longitude, latitude)) = geo_position else {
        return;
    };
    annotations.draft_mode = DraftMode::Point;
    annotations.draft_polygon_vertices.clear();
    annotations.draft_point = Some(GeoPoint {
        longitude,
        latitude,
    });
}

fn add_polygon_vertex_from_cursor(
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

fn clear_annotation_draft_geometry(annotations: &mut AnnotationOverlayState) {
    annotations.draft_point = None;
    annotations.draft_polygon_vertices.clear();
}

fn draft_geometry(annotations: &AnnotationOverlayState) -> Option<AnnotationGeometry> {
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

fn load_annotation_into_draft(
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

fn start_tile_fetch(
    fetch_task: &mut TileFetchTask,
    tile_state: &mut TileRenderState,
    config: &TileConfig,
) -> Result<()> {
    let scene_id = match &config.scene_id {
        Some(id) => id.clone(),
        None => {
            tile_state.status = TileStatus::MissingScene;
            return Ok(());
        }
    };

    let base_url = config.base_url.clone();
    let product_kind = config.product_kind.clone();
    let url = format!(
        "{}/api/scenes/{}/products/{}",
        base_url, scene_id, product_kind
    );

    tile_state.status = TileStatus::Fetching;
    fetch_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response.bytes().context("failed to read response body")?;
        let dynamic = image::load_from_memory(&bytes).context("failed to decode image bytes")?;
        Ok(FetchedTile { image: dynamic })
    }));

    Ok(())
}

fn start_field_list_fetch(
    field_list_task: &mut FieldListFetchTask,
    config: &TileConfig,
) -> Result<()> {
    let url = format!("{}/api/fields", config.base_url);
    field_list_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response.bytes().context("failed to read field list body")?;
        let fields = serde_json::from_slice::<Vec<FieldRecord>>(&bytes)
            .context("failed to decode fields")?;
        Ok(fields)
    }));

    Ok(())
}

fn start_field_scenes_fetch(
    field_scenes_task: &mut FieldScenesFetchTask,
    config: &TileConfig,
    field_id: &str,
) -> Result<()> {
    let url = format!("{}/api/fields/{}/scenes", config.base_url, field_id);
    field_scenes_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read field scenes response body")?;
        let scenes = serde_json::from_slice::<Vec<FieldSceneSummary>>(&bytes)
            .context("failed to decode field scenes")?;
        Ok(scenes)
    }));

    Ok(())
}

fn start_annotation_fetch(
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

fn start_annotation_create(
    annotation_create_task: &mut AnnotationCreateTask,
    config: &TileConfig,
    label: String,
    note: String,
    severity: String,
    geometry: AnnotationGeometry,
) -> Result<()> {
    let scene_id = config
        .scene_id
        .clone()
        .context("scene_id is required to create annotations")?;
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

fn start_annotation_update(
    annotation_update_task: &mut AnnotationUpdateTask,
    config: &TileConfig,
    annotation_id: &str,
    label: String,
    note: String,
    severity: String,
    geometry: AnnotationGeometry,
) -> Result<()> {
    let scene_id = config
        .scene_id
        .clone()
        .context("scene_id is required to update annotations")?;
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

fn start_annotation_delete(
    annotation_delete_task: &mut AnnotationDeleteTask,
    config: &TileConfig,
    annotation_id: &str,
) -> Result<()> {
    let scene_id = config
        .scene_id
        .clone()
        .context("scene_id is required to delete annotations")?;
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

fn start_manifest_fetch(
    manifest_task: &mut ManifestFetchTask,
    manifest_state: &mut SceneManifestState,
    config: &TileConfig,
) -> Result<()> {
    let scene_id = match &config.scene_id {
        Some(id) => id.clone(),
        None => {
            manifest_state.scene_id = None;
            manifest_state.sensor = None;
            manifest_state.acquired_at = None;
            manifest_state.width = None;
            manifest_state.height = None;
            manifest_state.bands.clear();
            manifest_state.gps_position = None;
            manifest_state.data_path = None;
            manifest_state.field = None;
            manifest_state.geospatial = SceneGeospatialMetadata::default();
            manifest_state.products.clear();
            return Ok(());
        }
    };

    manifest_state.scene_id = Some(scene_id.clone());
    manifest_state.sensor = None;
    manifest_state.acquired_at = None;
    manifest_state.width = None;
    manifest_state.height = None;
    manifest_state.bands.clear();
    manifest_state.gps_position = None;
    manifest_state.data_path = None;
    manifest_state.field = None;
    manifest_state.geospatial = SceneGeospatialMetadata::default();
    manifest_state.products.clear();

    let base_url = config.base_url.clone();
    let url = format!("{}/api/scenes/{}", base_url, scene_id);

    manifest_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response.bytes().context("failed to read manifest body")?;
        let manifest = serde_json::from_slice::<SceneManifest>(&bytes)
            .context("failed to decode scene manifest")?;
        Ok(manifest)
    }));

    Ok(())
}

fn clear_manifest_state(manifest_state: &mut SceneManifestState) {
    manifest_state.scene_id = None;
    manifest_state.sensor = None;
    manifest_state.acquired_at = None;
    manifest_state.width = None;
    manifest_state.height = None;
    manifest_state.bands.clear();
    manifest_state.gps_position = None;
    manifest_state.data_path = None;
    manifest_state.field = None;
    manifest_state.geospatial = SceneGeospatialMetadata::default();
    manifest_state.products.clear();
}

fn clear_annotations(
    annotations: &mut AnnotationOverlayState,
    annotation_fetch_task: &mut AnnotationFetchTask,
    annotation_create_task: &mut AnnotationCreateTask,
    annotation_update_task: &mut AnnotationUpdateTask,
    annotation_delete_task: &mut AnnotationDeleteTask,
) {
    annotations.items.clear();
    annotations.selected_annotation_id = None;
    annotations.draft_note.clear();
    annotations.draft_severity.clear();
    annotations.draft_mode = DraftMode::Point;
    clear_annotation_draft_geometry(annotations);
    annotation_fetch_task.0 = None;
    annotation_create_task.0 = None;
    annotation_update_task.0 = None;
    annotation_delete_task.0 = None;
}
