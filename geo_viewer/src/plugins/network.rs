use crate::plugins::annotations::{clear_annotation_draft_geometry, start_annotation_fetch};
use crate::plugins::map::{tile_center_world, tile_world_size, visible_tiles_for_view};
use crate::plugins::recommendations::{clear_recommendations, start_recommendation_fetch};
use crate::plugins::reports::{clear_reports, start_report_fetch};
use crate::state::{
    active_product_selection, assert_manifest_layer_placement, manifest_world_dimensions,
    AnnotationFetchTask, AnnotationOverlayState, FarmFieldHistoryFetchTask, FarmListFetchTask,
    FetchedTile, FieldCatalogState, FieldImportState, FieldImportTask, FieldListFetchTask,
    FieldSceneSummary, FieldScenesFetchTask, FieldSeasonGroup, ManifestFetchTask, MapCamera,
    MapViewState, RecommendationCreateTask, RecommendationDeleteTask, RecommendationFetchTask,
    RecommendationOverlayState, RecommendationUpdateTask, RenderedTile, ReportFetchTask,
    ReportGenerateTask, ReportOverlayState, SceneManifest, SceneManifestState,
    ShapefileImportRequest, TileConfig, TileDisplay, TileFetchTasks, TileId, TilePresence,
    TileRenderState, TileSource, TileStatus, ViewerState, DEFAULT_TILE_ZOOM,
};
use anyhow::{Context, Result};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::{
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        texture::ImageSampler,
    },
    tasks::IoTaskPool,
};
use futures_lite::future;
use image::{self, DynamicImage};
use shared::schemas::{FarmRecord, FieldRecord};
use tracing::info;

pub struct ViewerNetworkPlugin;

#[derive(SystemParam)]
struct ManifestCatalogState<'w, 's> {
    field_catalog: ResMut<'w, FieldCatalogState>,
    farm_field_history_task: ResMut<'w, FarmFieldHistoryFetchTask>,
    field_scenes_task: ResMut<'w, FieldScenesFetchTask>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct ManifestAnnotationState<'w, 's> {
    annotation_fetch_task: ResMut<'w, AnnotationFetchTask>,
    annotations: ResMut<'w, AnnotationOverlayState>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct ManifestRecommendationState<'w, 's> {
    recommendation_fetch_task: ResMut<'w, RecommendationFetchTask>,
    recommendation_create_task: ResMut<'w, RecommendationCreateTask>,
    recommendation_update_task: ResMut<'w, RecommendationUpdateTask>,
    recommendation_delete_task: ResMut<'w, RecommendationDeleteTask>,
    recommendations: ResMut<'w, RecommendationOverlayState>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
struct ManifestReportState<'w, 's> {
    report_fetch_task: ResMut<'w, ReportFetchTask>,
    report_generate_task: ResMut<'w, ReportGenerateTask>,
    reports: ResMut<'w, ReportOverlayState>,
    #[system_param(ignore)]
    marker: std::marker::PhantomData<&'s ()>,
}

impl Plugin for ViewerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, bootstrap_network_state)
            .add_systems(
                Update,
                (
                    poll_farm_list_fetch,
                    poll_farm_field_history_fetch,
                    poll_field_list_fetch,
                    poll_field_scenes_fetch,
                    poll_field_import,
                    poll_manifest_fetch,
                    poll_tile_fetch,
                ),
            )
            .add_systems(
                Update,
                sync_visible_tiles.after(crate::plugins::map::sync_map_camera),
            );
    }
}

fn bootstrap_network_state(
    config: Res<TileConfig>,
    mut farm_list_task: ResMut<FarmListFetchTask>,
    mut field_list_task: ResMut<FieldListFetchTask>,
    mut manifest_task: ResMut<ManifestFetchTask>,
    mut manifest_state: ResMut<SceneManifestState>,
    mut tile_fetch_tasks: ResMut<TileFetchTasks>,
    mut annotation_fetch_task: ResMut<AnnotationFetchTask>,
    mut recommendation_fetch_task: ResMut<RecommendationFetchTask>,
    mut report_fetch_task: ResMut<ReportFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Err(err) = start_farm_list_fetch(&mut farm_list_task, &config) {
        tile_state.status = TileStatus::Error(err.to_string());
    }
    if let Err(err) = start_field_list_fetch(&mut field_list_task, &config) {
        tile_state.status = TileStatus::Error(err.to_string());
    }

    if config.scene_id.is_some() {
        if let Err(err) = start_manifest_fetch(&mut manifest_task, &mut manifest_state, &config) {
            tile_state.status = TileStatus::Error(err.to_string());
        }
        tile_fetch_tasks.0.clear();
        annotation_fetch_task.0 = None;
        recommendation_fetch_task.0 = None;
        report_fetch_task.0 = None;
    }
}

fn poll_manifest_fetch(
    mut commands: Commands,
    mut manifest_task: ResMut<ManifestFetchTask>,
    mut manifest_state: ResMut<SceneManifestState>,
    mut config: ResMut<TileConfig>,
    mut viewer_state: ResMut<ViewerState>,
    mut catalog_state: ManifestCatalogState,
    mut tile_fetch_tasks: ResMut<TileFetchTasks>,
    mut annotation_state: ManifestAnnotationState,
    mut recommendation_state: ManifestRecommendationState,
    mut report_state: ManifestReportState,
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
                    manifest_state.owner = manifest.owner.clone();
                    manifest_state.sensor = manifest.sensor;
                    manifest_state.acquired_at = manifest.acquired_at;
                    manifest_state.width = manifest.width;
                    manifest_state.height = manifest.height;
                    manifest_state.bands = manifest.bands;
                    manifest_state.gps_position = manifest.gps_position;
                    manifest_state.data_path = manifest.data_path;
                    manifest_state.field_id = manifest.field_id.clone();
                    manifest_state.season_id = manifest.season_id.clone();
                    manifest_state.linked_at = manifest.linked_at.clone();
                    manifest_state.field = linked_field.clone();
                    manifest_state.geospatial = manifest.geospatial;
                    manifest_state.products = products;
                    catalog_state.field_catalog.selected_scene_id = Some(scene_id);
                    catalog_state.field_catalog.selected_owner = manifest.owner;
                    catalog_state.field_catalog.selected_season_id = manifest.season_id;
                    catalog_state.field_catalog.selected_linked_at = manifest.linked_at;
                    if let Some(field) = linked_field {
                        if let Some(farm_id) = field.farm_id.clone() {
                            let farm_changed =
                                catalog_state.field_catalog.selected_farm_id.as_deref()
                                    != Some(&farm_id);
                            catalog_state.field_catalog.selected_farm_id = Some(farm_id.clone());
                            if farm_changed {
                                if let Err(err) = start_farm_field_history_fetch(
                                    &mut catalog_state.farm_field_history_task,
                                    &config,
                                    &farm_id,
                                ) {
                                    tile_state.status = TileStatus::Error(err.to_string());
                                }
                            }
                        }
                        let field_changed =
                            catalog_state.field_catalog.selected_field_id.as_deref()
                                != Some(&field.field_id);
                        catalog_state.field_catalog.selected_field_id =
                            Some(field.field_id.clone());
                        if !catalog_state
                            .field_catalog
                            .fields
                            .iter()
                            .any(|known| known.field_id == field.field_id)
                        {
                            catalog_state.field_catalog.fields.push(field.clone());
                            catalog_state
                                .field_catalog
                                .fields
                                .sort_by(|left, right| left.name.cmp(&right.name));
                        }
                        if field_changed {
                            if let Err(err) = start_field_scenes_fetch(
                                &mut catalog_state.field_scenes_task,
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
                    annotation_state.annotations.items.clear();
                    annotation_state.annotations.selected_annotation_id = None;
                    clear_annotation_draft_geometry(&mut annotation_state.annotations);
                    clear_recommendations(
                        &mut recommendation_state.recommendations,
                        &mut recommendation_state.recommendation_fetch_task,
                        &mut recommendation_state.recommendation_create_task,
                        &mut recommendation_state.recommendation_update_task,
                        &mut recommendation_state.recommendation_delete_task,
                    );
                    clear_reports(
                        &mut report_state.reports,
                        &mut report_state.report_fetch_task,
                        &mut report_state.report_generate_task,
                    );

                    clear_loaded_tiles(&mut commands, &mut tile_state);
                    tile_fetch_tasks.0.clear();
                    tile_state.current_zoom = DEFAULT_TILE_ZOOM;
                    tile_state.image_dimensions = Vec2::new(
                        manifest_state.width.unwrap_or_default() as f32,
                        manifest_state.height.unwrap_or_default() as f32,
                    );
                    if let Err(err) = assert_manifest_layer_placement(
                        &manifest_state.geospatial,
                        manifest_state.width,
                        manifest_state.height,
                    ) {
                        tile_state.status =
                            TileStatus::Error(format!("layer placement mismatch: {err}"));
                        return;
                    }
                    tile_state.world_dimensions = manifest_world_dimensions(
                        &manifest_state.geospatial,
                        manifest_state.width,
                        manifest_state.height,
                    );

                    tile_state.status = TileStatus::Fetching;

                    if let Err(err) =
                        start_annotation_fetch(&mut annotation_state.annotation_fetch_task, &config)
                    {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                    if let Err(err) = start_recommendation_fetch(
                        &mut recommendation_state.recommendation_fetch_task,
                        &config,
                    ) {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                    if let Err(err) =
                        start_report_fetch(&mut report_state.report_fetch_task, &config)
                    {
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
                    manifest_state.geospatial = Default::default();
                    manifest_state.products.clear();
                    annotation_state.annotations.items.clear();
                    annotation_state.annotations.selected_annotation_id = None;
                    clear_annotation_draft_geometry(&mut annotation_state.annotations);
                    clear_recommendations(
                        &mut recommendation_state.recommendations,
                        &mut recommendation_state.recommendation_fetch_task,
                        &mut recommendation_state.recommendation_create_task,
                        &mut recommendation_state.recommendation_update_task,
                        &mut recommendation_state.recommendation_delete_task,
                    );
                    clear_reports(
                        &mut report_state.reports,
                        &mut report_state.report_fetch_task,
                        &mut report_state.report_generate_task,
                    );
                    clear_loaded_tiles(&mut commands, &mut tile_state);
                    tile_fetch_tasks.0.clear();
                    tile_state.image_dimensions = Vec2::ZERO;
                    tile_state.world_dimensions = Vec2::ZERO;
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        } else {
            manifest_task.0 = Some(task);
        }
    }
}

pub fn poll_field_list_fetch(
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

pub fn poll_field_scenes_fetch(
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

pub fn poll_tile_fetch(
    mut commands: Commands,
    mut tile_fetch_tasks: ResMut<TileFetchTasks>,
    mut tile_state: ResMut<TileRenderState>,
    mut textures: ResMut<Assets<Image>>,
) {
    let pending_ids: Vec<_> = tile_fetch_tasks.0.keys().copied().collect();
    let mut completed = Vec::new();

    for tile_id in pending_ids {
        let Some(task) = tile_fetch_tasks.0.get_mut(&tile_id) else {
            continue;
        };

        if let Some(result) = future::block_on(future::poll_once(task)) {
            completed.push(tile_id);
            match result {
                Ok(fetched) => {
                    let tile_size = tile_world_size(tile_state.world_dimensions, fetched.tile_id.z);
                    let Some(rendered) = tile_state.tiles.get_mut(&fetched.tile_id) else {
                        continue;
                    };
                    if fetched.missing {
                        rendered.presence = TilePresence::Missing;
                        commands.entity(rendered.entity).insert(Sprite {
                            color: missing_tile_color(fetched.tile_id),
                            custom_size: Some(tile_size),
                            ..default()
                        });
                        continue;
                    }

                    let rgba = fetched.image.to_rgba8();
                    let (width, height) = rgba.dimensions();
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
                    rendered.presence = TilePresence::Ready;
                    commands.entity(rendered.entity).insert(handle);
                    commands.entity(rendered.entity).insert(Sprite {
                        color: Color::WHITE,
                        custom_size: Some(tile_size),
                        ..default()
                    });
                    info!(tile = %fetched.tile_id, width, height, "tile loaded");
                }
                Err(err) => {
                    let message = err.to_string();
                    let tile_size = tile_world_size(tile_state.world_dimensions, tile_id.z);
                    if let Some(rendered) = tile_state.tiles.get_mut(&tile_id) {
                        rendered.presence = TilePresence::Failed;
                        commands.entity(rendered.entity).insert(Sprite {
                            color: failed_tile_color(tile_id),
                            custom_size: Some(tile_size),
                            ..default()
                        });
                    }
                    tile_state.status = TileStatus::Error(message);
                }
            }
        }
    }

    for tile_id in completed {
        tile_fetch_tasks.0.remove(&tile_id);
    }

    update_tile_status(&mut tile_state, &tile_fetch_tasks);
}

pub fn sync_visible_tiles(
    mut commands: Commands,
    windows: Query<&Window>,
    camera_query: Query<(&Transform, &Projection), With<MapCamera>>,
    config: Res<TileConfig>,
    manifest_state: Res<SceneManifestState>,
    mut tile_fetch_tasks: ResMut<TileFetchTasks>,
    mut tile_state: ResMut<TileRenderState>,
) {
    let Some(tile_source) = active_tile_source(&manifest_state, &config) else {
        return;
    };
    if tile_state.world_dimensions.x <= 0.0 || tile_state.world_dimensions.y <= 0.0 {
        return;
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((camera_transform, projection)) = camera_query.get_single() else {
        return;
    };
    let Projection::Orthographic(orthographic) = projection else {
        return;
    };

    let desired_tiles = visible_tiles_for_view(
        camera_transform.translation.truncate(),
        orthographic.scale,
        Vec2::new(window.width(), window.height()),
        tile_state.world_dimensions,
        tile_state.current_zoom,
    );

    let stale_tiles: Vec<_> = tile_state
        .tiles
        .keys()
        .copied()
        .filter(|tile_id| !desired_tiles.contains(tile_id))
        .collect();
    for tile_id in stale_tiles {
        if let Some(rendered) = tile_state.tiles.remove(&tile_id) {
            commands.entity(rendered.entity).despawn_recursive();
        }
        tile_fetch_tasks.0.remove(&tile_id);
    }
    tile_fetch_tasks
        .0
        .retain(|tile_id, _| desired_tiles.contains(tile_id));

    let tile_size = tile_world_size(tile_state.world_dimensions, tile_state.current_zoom);
    for tile_id in &desired_tiles {
        if tile_state.tiles.contains_key(tile_id) || tile_fetch_tasks.0.contains_key(tile_id) {
            continue;
        }

        let entity = commands
            .spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: loading_tile_color(*tile_id),
                        custom_size: Some(tile_size),
                        ..default()
                    },
                    transform: Transform::from_translation(
                        tile_center_world(tile_state.world_dimensions, *tile_id).extend(0.0),
                    ),
                    ..default()
                },
                TileDisplay,
            ))
            .id();
        tile_state.tiles.insert(
            *tile_id,
            RenderedTile {
                entity,
                presence: TilePresence::Loading,
            },
        );

        if let Err(err) = start_tile_fetch(&mut tile_fetch_tasks, &tile_source, *tile_id) {
            tile_state.status = TileStatus::Error(err.to_string());
        }
    }

    tile_state.visible_tiles = desired_tiles;
    update_tile_status(&mut tile_state, &tile_fetch_tasks);
}

pub fn start_tile_fetch(
    tile_fetch_tasks: &mut TileFetchTasks,
    tile_source: &TileSource,
    tile_id: TileId,
) -> Result<()> {
    if tile_fetch_tasks.0.contains_key(&tile_id) {
        return Ok(());
    }

    let url = tile_source.tile_url(tile_id);
    tile_fetch_tasks.0.insert(
        tile_id,
        IoTaskPool::get().spawn(async move { fetch_tile_from_url(&url, tile_id) }),
    );

    Ok(())
}

fn fetch_tile_from_url(url: &str, tile_id: TileId) -> Result<FetchedTile> {
    let response =
        reqwest::blocking::get(url).with_context(|| format!("request failed: {}", url))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(FetchedTile {
            tile_id,
            image: DynamicImage::new_rgba8(1, 1),
            missing: true,
        });
    }
    if !response.status().is_success() {
        anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
    }
    let bytes = response.bytes().context("failed to read response body")?;
    let dynamic = image::load_from_memory(&bytes).context("failed to decode image bytes")?;
    Ok(FetchedTile {
        tile_id,
        image: dynamic,
        missing: false,
    })
}

pub fn start_field_list_fetch(
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

pub fn start_farm_list_fetch(
    farm_list_task: &mut FarmListFetchTask,
    config: &TileConfig,
) -> Result<()> {
    let url = format!("{}/api/farms", config.base_url);
    farm_list_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response.bytes().context("failed to read farm list body")?;
        let farms =
            serde_json::from_slice::<Vec<FarmRecord>>(&bytes).context("failed to decode farms")?;
        Ok(farms)
    }));

    Ok(())
}

pub fn start_farm_field_history_fetch(
    farm_field_history_task: &mut FarmFieldHistoryFetchTask,
    config: &TileConfig,
    farm_id: &str,
) -> Result<()> {
    let url = format!("{}/api/farms/{}/fields/history", config.base_url, farm_id);
    farm_field_history_task.0 = Some(IoTaskPool::get().spawn(async move {
        let response =
            reqwest::blocking::get(&url).with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            anyhow::bail!("geo_hub returned {} for {}", response.status(), url);
        }
        let bytes = response
            .bytes()
            .context("failed to read farm field history body")?;
        let groups = serde_json::from_slice::<Vec<FieldSeasonGroup>>(&bytes)
            .context("failed to decode farm field history")?;
        Ok(groups)
    }));

    Ok(())
}

pub fn start_field_scenes_fetch(
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

pub fn start_field_import(
    field_import_task: &mut FieldImportTask,
    config: &TileConfig,
    request: ShapefileImportRequest,
) -> Result<()> {
    let url = format!("{}/api/fields/import/shapefile", config.base_url);
    field_import_task.0 = Some(IoTaskPool::get().spawn(async move {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(&url)
            .json(&request)
            .send()
            .with_context(|| format!("request failed: {}", url))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .unwrap_or_else(|_| "failed to read error body".to_string());
            anyhow::bail!("geo_hub returned {status} for {url}: {body}");
        }
        let bytes = response
            .bytes()
            .context("failed to read shapefile import body")?;
        let fields = serde_json::from_slice::<Vec<FieldRecord>>(&bytes)
            .context("failed to decode imported fields")?;
        Ok(fields)
    }));

    Ok(())
}

pub fn start_manifest_fetch(
    manifest_task: &mut ManifestFetchTask,
    manifest_state: &mut SceneManifestState,
    config: &TileConfig,
) -> Result<()> {
    let scene_id = match &config.scene_id {
        Some(id) => id.clone(),
        None => {
            clear_manifest_state(manifest_state);
            return Ok(());
        }
    };

    manifest_state.scene_id = Some(scene_id.clone());
    manifest_state.owner = None;
    manifest_state.sensor = None;
    manifest_state.acquired_at = None;
    manifest_state.width = None;
    manifest_state.height = None;
    manifest_state.bands.clear();
    manifest_state.gps_position = None;
    manifest_state.data_path = None;
    manifest_state.field_id = None;
    manifest_state.season_id = None;
    manifest_state.linked_at = None;
    manifest_state.field = None;
    manifest_state.geospatial = Default::default();
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

pub fn clear_manifest_state(manifest_state: &mut SceneManifestState) {
    manifest_state.scene_id = None;
    manifest_state.owner = None;
    manifest_state.sensor = None;
    manifest_state.acquired_at = None;
    manifest_state.width = None;
    manifest_state.height = None;
    manifest_state.bands.clear();
    manifest_state.gps_position = None;
    manifest_state.data_path = None;
    manifest_state.field_id = None;
    manifest_state.season_id = None;
    manifest_state.linked_at = None;
    manifest_state.field = None;
    manifest_state.geospatial = Default::default();
    manifest_state.products.clear();
}

fn poll_farm_list_fetch(
    mut farm_list_task: ResMut<FarmListFetchTask>,
    mut field_catalog: ResMut<FieldCatalogState>,
) {
    if let Some(mut task) = farm_list_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            if let Ok(farms) = result {
                field_catalog.farms = farms;
                if let Some(selected_farm_id) = field_catalog.selected_farm_id.as_ref() {
                    if !field_catalog
                        .farms
                        .iter()
                        .any(|farm| &farm.farm_id == selected_farm_id)
                    {
                        field_catalog.selected_farm_id = None;
                        field_catalog.season_groups.clear();
                    }
                }
            }
        } else {
            farm_list_task.0 = Some(task);
        }
    }
}

fn poll_farm_field_history_fetch(
    mut farm_field_history_task: ResMut<FarmFieldHistoryFetchTask>,
    mut field_catalog: ResMut<FieldCatalogState>,
) {
    if let Some(mut task) = farm_field_history_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            if let Ok(groups) = result {
                field_catalog.season_groups = groups.clone();
                field_catalog.fields = groups
                    .into_iter()
                    .flat_map(|group| group.fields.into_iter())
                    .collect();
                if let Some(selected_field_id) = field_catalog.selected_field_id.as_ref() {
                    if !field_catalog
                        .fields
                        .iter()
                        .any(|field| &field.field_id == selected_field_id)
                    {
                        field_catalog.selected_field_id = None;
                        field_catalog.selected_scene_id = None;
                        field_catalog.scenes.clear();
                    }
                }
            }
        } else {
            farm_field_history_task.0 = Some(task);
        }
    }
}

fn poll_field_import(
    config: Res<TileConfig>,
    mut field_import_task: ResMut<FieldImportTask>,
    mut field_import_state: ResMut<FieldImportState>,
    mut farm_field_history_task: ResMut<FarmFieldHistoryFetchTask>,
    mut field_list_task: ResMut<FieldListFetchTask>,
    mut field_catalog: ResMut<FieldCatalogState>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = field_import_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(imported) => {
                    field_import_state.status_message =
                        Some(format!("Imported {} shapefile field(s)", imported.len()));
                    if let Some(first) = imported.first() {
                        field_catalog.selected_field_id = Some(first.field_id.clone());
                        field_catalog.selected_farm_id = first.farm_id.clone();
                        if let Some(farm_id) = first.farm_id.as_deref() {
                            if let Err(err) = start_farm_field_history_fetch(
                                &mut farm_field_history_task,
                                &config,
                                farm_id,
                            ) {
                                tile_state.status = TileStatus::Error(err.to_string());
                            }
                        }
                    }
                    if let Err(err) = start_field_list_fetch(&mut field_list_task, &config) {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                }
                Err(err) => {
                    field_import_state.status_message = Some(err.to_string());
                    tile_state.status = TileStatus::Error("Field import failed".to_string());
                }
            }
        } else {
            field_import_task.0 = Some(task);
        }
    }
}

pub fn clear_loaded_tiles(commands: &mut Commands, tile_state: &mut TileRenderState) {
    for rendered in tile_state.tiles.values() {
        commands.entity(rendered.entity).despawn_recursive();
    }
    tile_state.tiles.clear();
    tile_state.visible_tiles.clear();
}

fn active_tile_source(
    manifest_state: &SceneManifestState,
    config: &TileConfig,
) -> Option<TileSource> {
    let target_idx = manifest_state
        .products
        .iter()
        .position(|product| product.kind == config.product_kind)?;
    active_product_selection(manifest_state, config, target_idx)
        .ok()
        .map(|selection| selection.tile_source)
}

fn update_tile_status(tile_state: &mut TileRenderState, tile_fetch_tasks: &TileFetchTasks) {
    if matches!(
        tile_state.status,
        TileStatus::MissingScene | TileStatus::Error(_)
    ) {
        return;
    }

    if tile_state.visible_tiles.is_empty() {
        tile_state.status = TileStatus::Idle;
        return;
    }

    if !tile_fetch_tasks.0.is_empty()
        || tile_state
            .tiles
            .values()
            .any(|tile| tile.presence == TilePresence::Loading)
    {
        tile_state.status = TileStatus::Fetching;
    } else {
        tile_state.status = TileStatus::Ready;
    }
}

fn loading_tile_color(tile_id: TileId) -> Color {
    if (tile_id.x + tile_id.y).is_multiple_of(2) {
        Color::srgba(0.22, 0.24, 0.28, 0.85)
    } else {
        Color::srgba(0.18, 0.20, 0.24, 0.85)
    }
}

fn missing_tile_color(tile_id: TileId) -> Color {
    if (tile_id.x + tile_id.y).is_multiple_of(2) {
        Color::srgba(0.45, 0.12, 0.12, 0.85)
    } else {
        Color::srgba(0.35, 0.10, 0.10, 0.85)
    }
}

fn failed_tile_color(tile_id: TileId) -> Color {
    if (tile_id.x + tile_id.y).is_multiple_of(2) {
        Color::srgba(0.55, 0.08, 0.18, 0.9)
    } else {
        Color::srgba(0.42, 0.06, 0.14, 0.9)
    }
}

#[cfg(test)]
mod tests {
    use super::fetch_tile_from_url;
    use crate::state::TileId;
    use image::{DynamicImage, ImageOutputFormat};
    use std::io::{Cursor, Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    fn serve_once(status: &str, body: Vec<u8>) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test HTTP server");
        let addr = listener.local_addr().expect("read listener address");
        let status = status.to_string();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept one request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("write response headers");
            stream.write_all(&body).expect("write response body");
        });

        (format!("http://{addr}/tile.png"), handle)
    }

    fn png_tile_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        DynamicImage::new_rgba8(1, 1)
            .write_to(&mut Cursor::new(&mut bytes), ImageOutputFormat::Png)
            .expect("encode test PNG");
        bytes
    }

    #[test]
    fn fetch_tile_from_url_decodes_present_tile() {
        let tile_id = TileId { z: 2, x: 1, y: 0 };
        let (url, server) = serve_once("200 OK", png_tile_bytes());

        let fetched = fetch_tile_from_url(&url, tile_id).expect("tile should fetch");
        server.join().expect("server thread should complete");

        assert_eq!(fetched.tile_id, tile_id);
        assert!(!fetched.missing);
        assert_eq!(fetched.image.width(), 1);
        assert_eq!(fetched.image.height(), 1);
    }

    #[test]
    fn fetch_tile_from_url_marks_not_found_as_missing() {
        let tile_id = TileId { z: 2, x: 9, y: 9 };
        let (url, server) = serve_once("404 Not Found", Vec::new());

        let fetched = fetch_tile_from_url(&url, tile_id).expect("404 should be a missing tile");
        server.join().expect("server thread should complete");

        assert_eq!(fetched.tile_id, tile_id);
        assert!(fetched.missing);
    }

    #[test]
    fn fetch_tile_from_url_rejects_server_error() {
        let tile_id = TileId { z: 2, x: 3, y: 1 };
        let (url, server) = serve_once("500 Internal Server Error", Vec::new());

        let err = match fetch_tile_from_url(&url, tile_id) {
            Ok(_) => panic!("500 should fail the tile"),
            Err(err) => err,
        };
        server.join().expect("server thread should complete");

        assert!(err.to_string().contains("geo_hub returned 500"));
    }
}
