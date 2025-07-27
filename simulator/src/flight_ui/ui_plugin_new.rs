use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::flight_ui::*;

pub struct UISystemPlugin;

impl Plugin for UISystemPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add Bevy states
            .init_state::<AppState>()
            
            // Add UI resources
            .init_resource::<UITheme>()
            .init_resource::<UIOverlayState>()
            
            // Add plugins for each UI module
            .add_plugins((
                EguiPlugin,
                MainMenuPlugin,
                OverlaySystemPlugin,
            ))
            
            // Add shared systems
            .add_systems(Startup, setup_ui_theme)
            .add_systems(Update, (
                handle_escape_key,
                update_ui_theme,
            ));
    }
}

fn setup_ui_theme(_commands: Commands) {
    info!("Setting up UI theme");
    // Theme is already initialized as a resource, we can customize it here if needed
}

fn handle_escape_key(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    current_app_state: Res<State<AppState>>,
    mut next_app_state: ResMut<NextState<AppState>>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        match current_app_state.get() {
            AppState::MainMenu => {
                // Exit application from main menu (handled in main_menu.rs)
            },
            AppState::World3D | AppState::World2D => {
                next_app_state.set(AppState::MainMenu);
            },
            AppState::CitySearch => {
                next_app_state.set(AppState::MainMenu);
            },
            AppState::WorldLoading => {
                // Can't escape from loading screen
            },
            AppState::Simulation => {
                next_app_state.set(AppState::MainMenu);
            },
        }
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
    theme.accent_color = Color::srgb(
        (theme.accent_color.to_srgba().red * pulse).min(1.0),
        (theme.accent_color.to_srgba().green * pulse).min(1.0),
        (theme.accent_color.to_srgba().blue * pulse).min(1.0),
    );
}
