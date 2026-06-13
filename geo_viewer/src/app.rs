use crate::plugins::{
    annotations::ViewerAnnotationsPlugin, map::ViewerMapPlugin, network::ViewerNetworkPlugin,
    recommendations::ViewerRecommendationsPlugin, reports::ViewerReportsPlugin, ui::ViewerUiPlugin,
};
use crate::state::{
    initial_tile_config, AnnotationCreateTask, AnnotationDeleteTask, AnnotationFetchTask,
    AnnotationOverlayState, AnnotationUpdateTask, CompareModeState, CursorMapState,
    FarmFieldHistoryFetchTask, FarmListFetchTask, FieldCatalogState, FieldImportState,
    FieldImportTask, FieldListFetchTask, FieldScenesFetchTask, ManifestFetchTask, MapViewState,
    RecommendationCreateTask, RecommendationDeleteTask, RecommendationFetchTask,
    RecommendationOverlayState, RecommendationUpdateTask, ReportFetchTask, ReportGenerateTask,
    ReportOverlayState, SceneManifestState, TileFetchTasks, TileRenderState, DEFAULT_TILE_ZOOM,
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
            ViewerRecommendationsPlugin,
            ViewerReportsPlugin,
            ViewerUiPlugin,
        ))
        .insert_resource(viewer_state)
        .insert_resource(tile_config)
        .insert_resource(FieldListFetchTask::default())
        .insert_resource(FieldScenesFetchTask::default())
        .insert_resource(FarmListFetchTask::default())
        .insert_resource(FarmFieldHistoryFetchTask::default())
        .insert_resource(FieldImportTask::default())
        .insert_resource(ManifestFetchTask::default())
        .insert_resource(SceneManifestState::default())
        .insert_resource(CompareModeState::default())
        .insert_resource(FieldCatalogState::default())
        .insert_resource(FieldImportState::default())
        .insert_resource(TileFetchTasks::default())
        .insert_resource(AnnotationFetchTask::default())
        .insert_resource(AnnotationCreateTask::default())
        .insert_resource(AnnotationUpdateTask::default())
        .insert_resource(AnnotationDeleteTask::default())
        .insert_resource(RecommendationFetchTask::default())
        .insert_resource(RecommendationCreateTask::default())
        .insert_resource(RecommendationUpdateTask::default())
        .insert_resource(RecommendationDeleteTask::default())
        .insert_resource(ReportFetchTask::default())
        .insert_resource(ReportGenerateTask::default())
        .insert_resource(TileRenderState {
            tiles: Default::default(),
            visible_tiles: Default::default(),
            image_dimensions: Vec2::ZERO,
            world_dimensions: Vec2::ZERO,
            current_zoom: DEFAULT_TILE_ZOOM,
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
        .insert_resource(RecommendationOverlayState {
            draft_title: "Recommended action".to_string(),
            ..default()
        })
        .insert_resource(ReportOverlayState {
            draft_title: "Scene agronomy report".to_string(),
            ..default()
        })
        .run();

    Ok(())
}
