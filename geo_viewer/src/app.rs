use crate::plugins::{
    annotations::ViewerAnnotationsPlugin, map::ViewerMapPlugin, network::ViewerNetworkPlugin,
    ui::ViewerUiPlugin,
};
use crate::state::{
    initial_tile_config, AnnotationCreateTask, AnnotationDeleteTask, AnnotationFetchTask,
    AnnotationOverlayState, AnnotationUpdateTask, CursorMapState, FieldCatalogState,
    FieldListFetchTask, FieldScenesFetchTask, ManifestFetchTask, MapViewState, SceneManifestState,
    TileFetchTask, TileRenderState,
};
use anyhow::Result;
use bevy::{prelude::*, window::WindowResolution};
use bevy_egui::EguiPlugin;

pub fn run() -> Result<()> {
    tracing_subscriber::fmt::init();

    let (tile_config, viewer_state, initial_status) = initial_tile_config();

    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: crate::state::APP_TITLE.into(),
                        resolution: WindowResolution::new(1600.0, 900.0),
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin)
        .add_plugins((
            ViewerNetworkPlugin,
            ViewerMapPlugin,
            ViewerAnnotationsPlugin,
            ViewerUiPlugin,
        ))
        .insert_resource(viewer_state)
        .insert_resource(tile_config)
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
        .run();

    Ok(())
}
