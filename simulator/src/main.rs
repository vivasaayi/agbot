use anyhow::Result;
use bevy::prelude::*;
use tracing::{info, warn};

mod app;
mod app_state;
mod globe_view;
mod globe_ui;
mod input_handler;
mod main_menu;
mod map_loader;
mod camera;
mod communication;
mod components;
mod drone_controller;
mod hud;
mod lidar_controls;
mod lidar_simulator;
mod resources;
mod systems;
mod terrain;
mod ui;

use app::VisualizerApp;
use app_state::{AppMode, SelectedRegion, UIState, DataLoadingState};
use globe_view::GlobePlugin;
use globe_ui::GlobeUIPlugin;
use input_handler::InputHandlerPlugin;
use main_menu::MainMenuPlugin;
use map_loader::MapLoaderPlugin;
use resources::AppConfig;
use communication::setup_communication_task;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("visualizer=debug,bevy=info")
        .init();

    info!("Starting AgBot Visualizer...");

    // Load configuration
    let config = AppConfig::load().unwrap_or_else(|e| {
        warn!("Failed to load config: {}, using defaults", e);
        AppConfig::default()
    });

    // Setup communication channels and spawn the async task
    let communication_channels = setup_communication_task(&config).await;

    // Create the Bevy app
    let mut app = App::new();

    // Configure the app with communication channels
    VisualizerApp::configure(&mut app, config, communication_channels);
    
    // Add state management
    app.init_state::<AppMode>()
        .insert_resource(SelectedRegion::default())
        .insert_resource(UIState::default())
        .insert_resource(DataLoadingState::default());
    
    // Add our new plugins
    app.add_plugins((
        MainMenuPlugin,
        InputHandlerPlugin,
        GlobePlugin,
        GlobeUIPlugin,
        MapLoaderPlugin,
    ));

    // Run the app (this blocks until the app exits)
    app.run();

    Ok(())
}
