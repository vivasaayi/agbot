use bevy::prelude::*;

/// Main application states for the simulator
#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum AppMode {
    #[default]
    MainMenu,
    Globe,
    Map2D,
    Simulation3D,
}

/// Coordinates for selected region
#[derive(Resource, Debug, Clone)]
pub struct SelectedRegion {
    pub center_lat: f64,
    pub center_lon: f64,
    pub bounds_width_degrees: f64,
    pub bounds_height_degrees: f64,
}

impl Default for SelectedRegion {
    fn default() -> Self {
        // Default to Rome area (from your current GeoJSON)
        Self {
            center_lat: 41.8992,
            center_lon: 12.4784,
            bounds_width_degrees: 0.01,
            bounds_height_degrees: 0.01,
        }
    }
}

/// UI state and preferences
#[derive(Resource, Debug, Clone)]
pub struct UIState {
    pub show_debug_info: bool,
    pub show_coordinates: bool,
    pub map_layer_roads: bool,
    pub map_layer_buildings: bool,
    pub map_layer_terrain: bool,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            show_debug_info: true,
            show_coordinates: true,
            map_layer_roads: true,
            map_layer_buildings: true,
            map_layer_terrain: true,
        }
    }
}

/// Data loading state
#[derive(Resource, Debug, Clone)]
pub struct DataLoadingState {
    pub is_loading: bool,
    pub progress: f32, // 0.0 to 1.0
    pub status_message: String,
}

impl Default for DataLoadingState {
    fn default() -> Self {
        Self {
            is_loading: false,
            progress: 0.0,
            status_message: "Ready".to_string(),
        }
    }
}

/// Globe search state
#[derive(Resource, Debug, Clone)]
pub struct GlobeSearchState {
    pub search_query: String,
    pub show_suggestions: bool,
    pub is_animating: bool,
    pub animation_start_time: f32,
    pub animation_duration: f32,
    pub start_lat: f64,
    pub start_lon: f64,
    pub target_lat: f64,
    pub target_lon: f64,
    pub target_zoom: f32,
}

impl Default for GlobeSearchState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            show_suggestions: false,
            is_animating: false,
            animation_start_time: 0.0,
            animation_duration: 2.0, // 2 seconds
            start_lat: 0.0,
            start_lon: 0.0,
            target_lat: 0.0,
            target_lon: 0.0,
            target_zoom: 5.0,
        }
    }
}
