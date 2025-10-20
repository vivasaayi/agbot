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
struct TileFetchTask(Option<Task<anyhow::Result<FetchedTile>>>);

struct FetchedTile {
    image: DynamicImage,
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
        .insert_resource(TileFetchTask::default())
        .insert_resource(TileRenderState {
            entity: None,
            handle: None,
            dimensions: Vec2::ZERO,
            status: initial_status,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (render_ui, poll_tile_fetch, apply_zoom))
        .run();

    Ok(())
}

fn setup(
    mut commands: Commands,
    config: Res<TileConfig>,
    mut fetch_task: ResMut<TileFetchTask>,
    mut tile_state: ResMut<TileRenderState>,
) {
    commands.spawn(Camera2dBundle::default());

    if config.scene_id.is_some() {
        if let Err(err) = start_tile_fetch(&mut fetch_task, &mut tile_state, &config) {
            tile_state.status = TileStatus::Error(err.to_string());
        }
    }
}

fn render_ui(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut viewer_state: ResMut<ViewerState>,
    mut config: ResMut<TileConfig>,
    mut tile_state: ResMut<TileRenderState>,
    mut fetch_task: ResMut<TileFetchTask>,
) {
    egui::SidePanel::left("layers_panel").show(contexts.ctx_mut(), |ui| {
        ui.heading("Layers");
        ui.separator();
        ui.radio_value(
            &mut viewer_state.selected_layer,
            0,
            "NDVI (computed by geo_hub)",
        );
        ui.add_space(8.0);

        ui.heading("Scene");
        ui.label("Scene ID");
        let mut load_requested = false;
        let response = ui.text_edit_singleline(&mut viewer_state.scene_id_input);
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            load_requested = true;
        }
        if ui.button("Load NDVI").clicked() {
            load_requested = true;
        }

        if load_requested {
            let trimmed = viewer_state.scene_id_input.trim().to_string();
            if trimmed.is_empty() {
                config.scene_id = None;
                tile_state.status = TileStatus::MissingScene;
                if let Some(entity) = tile_state.entity.take() {
                    commands.entity(entity).despawn_recursive();
                }
                tile_state.handle = None;
                tile_state.dimensions = Vec2::ZERO;
                fetch_task.0 = None;
            } else {
                config.scene_id = Some(trimmed.clone());
                if let Some(entity) = tile_state.entity.take() {
                    commands.entity(entity).despawn_recursive();
                }
                tile_state.handle = None;
                tile_state.dimensions = Vec2::ZERO;
                if let Err(err) = start_tile_fetch(&mut fetch_task, &mut tile_state, &config) {
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

    egui::TopBottomPanel::top("status_bar").show(contexts.ctx_mut(), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Hub: {}", base_url));
            ui.separator();
            ui.label(format!("Scene: {}", active_scene));
            ui.separator();
            ui.label(status_message);
            if !dimension_text.is_empty() {
                ui.separator();
                ui.label(dimension_text);
            }
        });
    });
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
