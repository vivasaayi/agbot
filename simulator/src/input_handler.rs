use bevy::prelude::*;
use crate::app_state::{AppMode, UIState};

pub struct InputHandlerPlugin;

impl Plugin for InputHandlerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_global_input);
    }
}

fn handle_global_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppMode>>,
    mut ui_state: ResMut<UIState>,
    current_state: Res<State<AppMode>>,
) {
    // Toggle debug info
    if keyboard.just_pressed(KeyCode::Tab) {
        ui_state.show_debug_info = !ui_state.show_debug_info;
    }
    
    // Return to main menu
    if keyboard.just_pressed(KeyCode::Escape) && !matches!(current_state.get(), AppMode::MainMenu) {
        next_state.set(AppMode::MainMenu);
    }
    
    // Quick mode switching
    if keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight) {
        if keyboard.just_pressed(KeyCode::Digit1) {
            next_state.set(AppMode::MainMenu);
        } else if keyboard.just_pressed(KeyCode::Digit2) {
            next_state.set(AppMode::Globe);
        } else if keyboard.just_pressed(KeyCode::Digit3) {
            next_state.set(AppMode::Map2D);
        } else if keyboard.just_pressed(KeyCode::Digit4) {
            next_state.set(AppMode::Simulation3D);
        }
    }
    
    // Toggle coordinate display
    if keyboard.just_pressed(KeyCode::KeyC) {
        ui_state.show_coordinates = !ui_state.show_coordinates;
    }
}
