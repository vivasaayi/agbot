use anyhow::{Context, Result};
use bevy::tasks::{IoTaskPool, Task};
use bevy::{
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
use tracing::info;

const APP_TITLE: &str = "Geo Viewer";
const DEFAULT_PRODUCT_KIND: &str = "ndvi";

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
struct ManifestFetchTask(Option<Task<anyhow::Result<SceneManifest>>>);

#[derive(Resource, Default)]
struct TileFetchTask(Option<Task<anyhow::Result<FetchedTile>>>);

struct FetchedTile {
    image: DynamicImage,
}

#[derive(Debug, Clone, Deserialize)]
struct SceneManifest {
    scene_id: String,
    sensor: Option<String>,
    acquired_at: Option<String>,
    available_products: Vec<SceneProduct>,
}

#[derive(Debug, Clone, Deserialize)]
struct SceneProduct {
    kind: String,
    filename: String,
    content_type: String,
    url_path: String,
}

#[derive(Resource, Default)]
struct SceneManifestState {
    scene_id: Option<String>,
    sensor: Option<String>,
    acquired_at: Option<String>,
    products: Vec<SceneProduct>,
}

#[derive(Resource)]
struct TileRenderState {
    entity: Option<Entity>,
    handle: Option<Handle<Image>>,
    dimensions: Vec2,
    status: TileStatus,
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
        .insert_resource(ManifestFetchTask::default())
        .insert_resource(SceneManifestState::default())
        .insert_resource(TileFetchTask::default())
        .insert_resource(TileRenderState {
            entity: None,
            handle: None,
            dimensions: Vec2::ZERO,
            status: initial_status,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (render_ui, poll_manifest_fetch, poll_tile_fetch, apply_zoom),
        )
        .run();

    Ok(())
}

fn setup(
    mut commands: Commands,
    config: Res<TileConfig>,
    mut manifest_task: ResMut<ManifestFetchTask>,
    mut manifest_state: ResMut<SceneManifestState>,
    mut fetch_task: ResMut<TileFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
) {
    commands.spawn(Camera2dBundle::default());

    if config.scene_id.is_some() {
        if let Err(err) = start_manifest_fetch(&mut manifest_task, &mut manifest_state, &config) {
            tile_state.status = TileStatus::Error(err.to_string());
        }
        fetch_task.0 = None;
    }
}

fn render_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut viewer_state: ResMut<ViewerState>,
    mut config: ResMut<TileConfig>,
    mut manifest_state: ResMut<SceneManifestState>,
    mut manifest_task: ResMut<ManifestFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
    mut fetch_task: ResMut<TileFetchTask>,
) {
    egui::SidePanel::left("layers_panel").show(contexts.ctx_mut(), |ui| {
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
                config.product_kind = DEFAULT_PRODUCT_KIND.to_string();
                tile_state.status = TileStatus::MissingScene;
                if let Some(entity) = tile_state.entity.take() {
                    commands.entity(entity).despawn_recursive();
                }
                tile_state.handle = None;
                tile_state.dimensions = Vec2::ZERO;
                clear_manifest_state(&mut manifest_state);
                fetch_task.0 = None;
                manifest_task.0 = None;
            } else {
                config.scene_id = Some(trimmed.clone());
                viewer_state.selected_layer = 0;
                config.product_kind = DEFAULT_PRODUCT_KIND.to_string();
                if let Some(entity) = tile_state.entity.take() {
                    commands.entity(entity).despawn_recursive();
                }
                tile_state.handle = None;
                tile_state.dimensions = Vec2::ZERO;
                fetch_task.0 = None;
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
    });

    let status_message = tile_state.status.message();
    let dimension_text = if matches!(tile_state.status, TileStatus::Ready) {
        format!(
            "{} × {} px",
            tile_state.dimensions.x as u32, tile_state.dimensions.y as u32
        )
    } else {
        String::new()
    };
    let base_url = config.base_url.clone();
    let active_scene = config
        .scene_id
        .clone()
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
    let acquired_at = manifest_state
        .acquired_at
        .clone()
        .unwrap_or_else(|| "n/a".to_string());

    egui::TopBottomPanel::top("status_bar").show(contexts.ctx_mut(), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Hub: {}", base_url));
            ui.separator();
            ui.label(format!("Scene: {}", active_scene));
            ui.separator();
            ui.label(format!("Layer: {}", active_layer));
            ui.separator();
            ui.label(format!("Sensor: {}", sensor));
            ui.separator();
            ui.label(format!("Acquired: {}", acquired_at));
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
    mut fetch_task: ResMut<TileFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
) {
    if let Some(mut task) = manifest_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(manifest) => {
                    let products = manifest.available_products;
                    manifest_state.scene_id = Some(manifest.scene_id);
                    manifest_state.sensor = manifest.sensor;
                    manifest_state.acquired_at = manifest.acquired_at;
                    manifest_state.products = products;

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

                    if let Err(err) = start_tile_fetch(&mut fetch_task, &mut tile_state, &config) {
                        tile_state.status = TileStatus::Error(err.to_string());
                    }
                }
                Err(err) => {
                    manifest_state.scene_id = config.scene_id.clone();
                    manifest_state.sensor = None;
                    manifest_state.acquired_at = None;
                    manifest_state.products.clear();
                    tile_state.status = TileStatus::Error(err.to_string());
                }
            }
        } else {
            manifest_task.0 = Some(task);
        }
    }
}

fn poll_tile_fetch(
    mut commands: Commands,
    mut fetch_task: ResMut<TileFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
    mut textures: ResMut<Assets<Image>>,
    viewer_state: Res<ViewerState>,
) {
    if let Some(mut task) = fetch_task.0.take() {
        if let Some(result) = future::block_on(future::poll_once(&mut task)) {
            match result {
                Ok(fetched) => {
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

                    if let Some(entity) = tile_state.entity.take() {
                        commands.entity(entity).despawn_recursive();
                    }

                    let entity = commands
                        .spawn((
                            SpriteBundle {
                                texture: handle.clone(),
                                sprite: Sprite {
                                    custom_size: Some(Vec2::new(width as f32, height as f32)),
                                    ..default()
                                },
                                transform: Transform::from_scale(Vec3::splat(
                                    viewer_state.zoom_level,
                                )),
                                ..default()
                            },
                            TileDisplay,
                        ))
                        .id();

                    tile_state.entity = Some(entity);
                    tile_state.handle = Some(handle);
                    tile_state.dimensions = Vec2::new(width as f32, height as f32);
                    tile_state.status = TileStatus::Ready;
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

fn apply_zoom(viewer_state: Res<ViewerState>, mut query: Query<&mut Transform, With<TileDisplay>>) {
    if viewer_state.is_changed() {
        for mut transform in &mut query {
            transform.scale = Vec3::splat(viewer_state.zoom_level);
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
            manifest_state.products.clear();
            return Ok(());
        }
    };

    manifest_state.scene_id = Some(scene_id.clone());
    manifest_state.sensor = None;
    manifest_state.acquired_at = None;
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
    manifest_state.products.clear();
}
