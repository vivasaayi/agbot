use anyhow::Result;
use bevy::prelude::*;
use tracing::{info, warn};

mod app;
mod app_state;
mod earth_textures;
mod procedural_textures;
mod globe_view;
mod globe_ui;
mod input_handler;
mod location_database;
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
mod overlays { pub mod ndvi; }
mod geodesy;
mod autopilot { pub mod waypoint; }

// World exploration modules
mod world_exploration;
mod city_search;

// New Flight Simulator-style UI system
mod flight_ui;

use app::VisualizerApp;
use app_state::{AppMode, SelectedRegion, UIState, DataLoadingState, GlobeSearchState};
// use globe_view::GlobePlugin;
// use globe_ui::GlobeUIPlugin;
// use input_handler::InputHandlerPlugin;
// use map_loader::MapLoaderPlugin;
use resources::AppConfig;
use communication::setup_communication_task;
use geodesy::GeodesyPlugin;
use globe_view::GlobePlugin;
use flight_ui::FlightUIPlugin;
use world_exploration::World3DPlugin;

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
        .insert_resource(DataLoadingState::default())
        // Needed by globe_view::handle_search_animation
        .insert_resource(GlobeSearchState::default());
    
    // Add back minimal world view plugins
    app.add_plugins((
        FlightUIPlugin,          // initializes flight UI states
        GlobePlugin,             // globe view and interactions
        World3DPlugin,           // city search + world loader flow
    ));

    // Start in City Search with Globe visible
    fn set_initial_view_states(
        mut next_ui: ResMut<NextState<flight_ui::AppState>>,
        mut next_mode: ResMut<NextState<AppMode>>,
    ) {
        next_ui.set(flight_ui::AppState::CitySearch);
        next_mode.set(AppMode::Globe);
    }
    app.add_systems(Startup, set_initial_view_states);

    // Geodesy logs and helpers
    app.add_plugins(GeodesyPlugin);

    // Run the app (this blocks until the app exits)
    app.run();

    Ok(())
}
