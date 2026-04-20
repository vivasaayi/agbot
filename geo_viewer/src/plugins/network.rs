use crate::plugins::annotations::{clear_annotation_draft_geometry, start_annotation_fetch};
use crate::plugins::map::extent_world_size;
use crate::state::{
    AnnotationFetchTask, AnnotationOverlayState, FetchedTile, FieldCatalogState,
    FieldListFetchTask, FieldSceneSummary, FieldScenesFetchTask, ManifestFetchTask, MapViewState,
    SceneManifest, SceneManifestState, TileConfig, TileDisplay, TileFetchTask, TileId,
    TileRenderState, TileStatus, ViewerState, DEFAULT_TILE_ZOOM,
};
use anyhow::{Context, Result};
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
use image::{self};
use shared::schemas::FieldRecord;
use tracing::info;

pub struct ViewerNetworkPlugin;

impl Plugin for ViewerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, bootstrap_network_state)
            .add_systems(
                Update,
                (
                    poll_field_list_fetch,
                    poll_field_scenes_fetch,
                    poll_manifest_fetch,
                    poll_tile_fetch,
                ),
            );
    }
}

fn bootstrap_network_state(
    config: Res<TileConfig>,
    mut field_list_task: ResMut<FieldListFetchTask>,
    mut manifest_task: ResMut<ManifestFetchTask>,
    mut manifest_state: ResMut<SceneManifestState>,
    mut fetch_task: ResMut<TileFetchTask>,
    mut annotation_fetch_task: ResMut<AnnotationFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
) {
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

pub fn poll_manifest_fetch(
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
                    annotations.selected_annotation_id = None;
                    clear_annotation_draft_geometry(&mut annotations);

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
                    manifest_state.geospatial = Default::default();
                    manifest_state.products.clear();
                    annotations.items.clear();
                    annotations.selected_annotation_id = None;
                    clear_annotation_draft_geometry(&mut annotations);
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
                    let tile_id = fetched.tile_id;
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
                    info!(tile = %tile_id, width, height, "tile loaded");
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

pub fn start_tile_fetch(
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
        Ok(FetchedTile {
            tile_id: TileId {
                z: DEFAULT_TILE_ZOOM,
                x: 0,
                y: 0,
            },
            image: dynamic,
        })
    }));

    Ok(())
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
}

pub fn clear_loaded_tile(commands: &mut Commands, tile_state: &mut TileRenderState) {
    if let Some(entity) = tile_state.entity.take() {
        commands.entity(entity).despawn_recursive();
    }
    tile_state.handle = None;
    tile_state.image_dimensions = Vec2::ZERO;
    tile_state.world_dimensions = Vec2::ZERO;
}
