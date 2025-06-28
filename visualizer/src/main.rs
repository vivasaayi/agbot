use anyhow::Result;
use bevy::prelude::*;
use tracing::{info, warn};

mod app;
mod camera;
mod communication;
mod components;
mod drone_controller;
mod hud;
mod resources;
mod systems;
mod terrain;
mod ui;

use app::VisualizerApp;
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

    // Run the app (this blocks until the app exits)
    app.run();

    Ok(())
}
