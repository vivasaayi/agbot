use bevy::prelude::*;

/// Main application states for navigation flow
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Splash,
    MainMenu,
    WorldMap,
    LoadingSimulation,
    Simulation,
    Settings,
    Paused,
}

/// Substates for more granular menu control
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum MenuState {
    #[default]
    None,
    SettingsGraphics,
    SettingsControls,
    SettingsAudio,
    ConfirmExit,
    MissionBriefing,
    DroneSelection,
}

/// UI Theme colors and styles
#[derive(Resource)]
pub struct UITheme {
    pub primary_color: Color,
    pub secondary_color: Color,
    pub background_color: Color,
    pub text_color: Color,
    pub accent_color: Color,
    pub warning_color: Color,
    pub success_color: Color,
}

impl Default for UITheme {
    fn default() -> Self {
        Self {
            primary_color: Color::srgb(0.2, 0.4, 0.8),      // Blue
            secondary_color: Color::srgb(0.3, 0.3, 0.3),    // Dark Gray
            background_color: Color::srgb(0.1, 0.1, 0.15),  // Dark Blue
            text_color: Color::srgb(0.9, 0.9, 0.9),         // Light Gray
            accent_color: Color::srgb(0.0, 0.8, 0.4),       // Green
            warning_color: Color::srgb(0.9, 0.6, 0.0),      // Orange
            success_color: Color::srgb(0.2, 0.8, 0.2),      // Bright Green
        }
    }
}

/// Tracks active overlays and dialogs
#[derive(Resource, Default)]
pub struct UIOverlayState {
    pub active_overlays: Vec<OverlayType>,
    pub modal_open: bool,
    pub notification_queue: Vec<Notification>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayType {
    Notification,
    MissionBriefing,
    DroneStatus,
    WeatherInfo,
    Settings,
    ConfirmDialog,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub level: NotificationLevel,
    pub duration: f32,
    pub timestamp: f32,
}

#[derive(Debug, Clone)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// Plugin to manage app states and UI theme
pub struct AppStatePlugin;

impl Plugin for AppStatePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_state::<AppState>()
            .init_state::<MenuState>()
            .init_resource::<UITheme>()
            .init_resource::<UIOverlayState>()
            .add_systems(Update, (
                handle_escape_key,
                update_notifications,
            ));
    }
}

/// Handle escape key for navigation
fn handle_escape_key(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut overlay_state: ResMut<UIOverlayState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match current_state.get() {
            AppState::Simulation => {
                if overlay_state.modal_open {
                    // Close any open modals first
                    overlay_state.modal_open = false;
                    overlay_state.active_overlays.clear();
                } else {
                    // Open pause menu
                    next_state.set(AppState::Paused);
                }
            }
            AppState::Paused => {
                next_state.set(AppState::Simulation);
            }
            AppState::Settings => {
                next_state.set(AppState::MainMenu);
            }
            AppState::WorldMap => {
                next_state.set(AppState::MainMenu);
            }
            _ => {}
        }
    }
}

/// Update notification system
fn update_notifications(
    time: Res<Time>,
    mut overlay_state: ResMut<UIOverlayState>,
) {
    let current_time = time.elapsed_seconds();
    
    // Remove expired notifications
    overlay_state.notification_queue.retain(|notification| {
        current_time - notification.timestamp < notification.duration
    });
}

/// Helper function to add notifications
impl UIOverlayState {
    pub fn add_notification(&mut self, message: String, level: NotificationLevel, duration: f32, time: &Res<Time>) {
        self.notification_queue.push(Notification {
            message,
            level,
            duration,
            timestamp: time.elapsed_seconds(),
        });
    }
    
    /// Add a simple notification with default settings
    pub fn add_simple_notification(&mut self, message: String) {
        self.notification_queue.push(Notification {
            message,
            level: NotificationLevel::Info,
            duration: 3.0,
            timestamp: 0.0, // Will be updated by the notification system
        });
    }
    
    /// Add notification with specific level
    pub fn add_notification_with_level(&mut self, message: String, level: NotificationLevel) {
        self.notification_queue.push(Notification {
            message,
            level,
            duration: 3.0,
            timestamp: 0.0, // Will be updated by the notification system
        });
    }
}
