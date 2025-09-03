// Flight Simulator-style UI system
pub mod app_state;
pub mod main_menu;
pub mod ui_plugin;
pub mod overlay_system;

pub use app_state::*;
pub use main_menu::*;
pub use ui_plugin::*;
pub use overlay_system::*;

use bevy::prelude::*;

/// Main plugin that integrates the Flight Simulator-style UI
pub struct FlightUIPlugin;

impl Plugin for FlightUIPlugin {
    fn build(&self, app: &mut App) {
        // Add the new UI system
        app.add_plugins(UISystemPlugin);
    }
}
