use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::ui::*;

pub struct UISystemPlugin;

impl Plugin for UISystemPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add Bevy states
            .init_state::<AppState>()
            .init_state::<MenuState>()
            
            // Add UI resources
            .init_resource::<UITheme>()
            .init_resource::<UIOverlayState>()
            .init_resource::<WorldMapState>()
            .init_resource::<SimulationHudState>()
            .init_resource::<SettingsState>()
            
            // Add plugins for each UI module
            .add_plugins((
                EguiPlugin,
                MainMenuPlugin,
                WorldMapPlugin,
                SimulationHudPlugin,
                SettingsPlugin,
                OverlaySystemPlugin,
            ))
            
            // Add shared systems
            .add_systems(Startup, setup_ui_theme)
            .add_systems(Update, (
                handle_escape_key,
                update_ui_theme,
                auto_transition_from_splash.run_if(in_state(AppState::Splash)),
            ));
    }
}

fn setup_ui_theme(mut commands: Commands) {
    info!("Setting up UI theme");
    // Theme is already initialized as a resource, we can customize it here if needed
}

fn handle_escape_key(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    current_app_state: Res<State<AppState>>,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        match current_app_state.get() {
            AppState::Splash => {
                // Skip splash screen
                next_app_state.set(AppState::MainMenu);
            },
            AppState::MainMenu => {
                // Exit application from main menu
                // (handled in main_menu.rs)
            },
            AppState::WorldMap => {
                next_app_state.set(AppState::MainMenu);
            },
            AppState::Settings => {
                next_app_state.set(AppState::MainMenu);
            },
            AppState::Simulation => {
                next_app_state.set(AppState::Paused);
            },
            AppState::Paused => {
                next_app_state.set(AppState::Simulation);
            },
            AppState::LoadingSimulation => {
                // Can't escape from loading screen
            },
        }
        
        // Reset menu state when escaping
        next_menu_state.set(MenuState::None);
    }
}

fn update_ui_theme(
    time: Res<Time>,
    mut theme: ResMut<UITheme>,
) {
    // Update any animated theme properties here
    // For example, pulsing colors or animated backgrounds
    let elapsed = time.elapsed_seconds();
    
    // Subtle animation for accent colors (optional)
    let pulse = (elapsed * 2.0).sin() * 0.1 + 0.9;
    theme.accent_color.r = (theme.accent_color.r * pulse).min(1.0);
    theme.accent_color.g = (theme.accent_color.g * pulse).min(1.0);
    theme.accent_color.b = (theme.accent_color.b * pulse).min(1.0);
}

fn auto_transition_from_splash(
    time: Res<Time>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Auto transition from splash screen after 3 seconds
    if time.elapsed_seconds() > 3.0 {
        next_state.set(AppState::MainMenu);
    }
}

/// Utility function to show a notification
pub fn show_notification(
    overlay_state: &mut ResMut<UIOverlayState>,
    message: String,
    notification_type: NotificationType,
) {
    overlay_state.add_notification_with_type(message, notification_type);
}

/// Utility function to show a dialog
pub fn show_dialog(
    overlay_state: &mut ResMut<UIOverlayState>,
    title: String,
    message: String,
    dialog_type: DialogType,
) {
    overlay_state.show_dialog(title, message, dialog_type);
}

/// Development helper to quickly transition between states
#[cfg(debug_assertions)]
pub fn debug_ui_transitions(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut next_app_state: ResMut<NextState<AppState>>,
) {
    // Debug hotkeys for quick testing (only in debug builds)
    if keyboard_input.just_pressed(KeyCode::F1) {
        next_app_state.set(AppState::MainMenu);
        info!("Debug: Switched to MainMenu");
    }
    if keyboard_input.just_pressed(KeyCode::F2) {
        next_app_state.set(AppState::WorldMap);
        info!("Debug: Switched to WorldMap");
    }
    if keyboard_input.just_pressed(KeyCode::F3) {
        next_app_state.set(AppState::Simulation);
        info!("Debug: Switched to Simulation");
    }
    if keyboard_input.just_pressed(KeyCode::F4) {
        next_app_state.set(AppState::Settings);
        info!("Debug: Switched to Settings");
    }
}
