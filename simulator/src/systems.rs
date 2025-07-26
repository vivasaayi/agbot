use bevy::prelude::*;
use crate::resources::AppState;

pub fn handle_keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut app_state: ResMut<AppState>,
    mut exit: EventWriter<AppExit>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }
    
    if keyboard_input.just_pressed(KeyCode::Space) {
        app_state.paused = !app_state.paused;
    }
    
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        app_state.show_ui = !app_state.show_ui;
    }
    
    if keyboard_input.just_pressed(KeyCode::KeyI) {
        app_state.show_inspector = !app_state.show_inspector;
    }
    
    if keyboard_input.just_pressed(KeyCode::KeyR) {
        app_state.replay_mode = !app_state.replay_mode;
    }

    // Time control
    if keyboard_input.pressed(KeyCode::ArrowRight) && app_state.replay_mode {
        app_state.time_scale = (app_state.time_scale * 1.1).min(10.0);
    }
    
    if keyboard_input.pressed(KeyCode::ArrowLeft) && app_state.replay_mode {
        app_state.time_scale = (app_state.time_scale * 0.9).max(0.1);
    }
}

pub fn update_time(
    time: Res<Time>,
    mut app_state: ResMut<AppState>,
) {
    if !app_state.paused {
        app_state.current_time += time.delta_seconds_f64() * app_state.time_scale as f64;
    }
}

pub fn update_app_state(
    _app_state: ResMut<AppState>,
    // Add other resources as needed
) {
    // Update connection status and other state here
    // This would typically check communication status
}
