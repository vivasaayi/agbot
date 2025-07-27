// Flight Simulator-style UI system
pub mod app_state;
pub mod demo;

pub use app_state::*;
pub use demo::*;

use bevy::prelude::*;

/// Main plugin that integrates the Flight Simulator-style UI
pub struct FlightUIPlugin;

impl Plugin for FlightUIPlugin {
    fn build(&self, app: &mut App) {
        // Add the demo UI system for now
        app.add_plugins(DemoFlightUIPlugin);
    }
}
