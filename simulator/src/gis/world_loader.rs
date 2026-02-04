//! Real World Loader
//!
//! Orchestrates loading a real-world location with:
//! - Elevation data (SRTM/terrain tiles)
//! - Satellite imagery (ESRI/Sentinel-2)
//! - OSM features (buildings, roads, farms)
//! - NDVI overlays
//!
//! This is the main entry point for loading any location on Earth.

use anyhow::Result;
use bevy::prelude::*;
use tokio::runtime::Handle as TokioHandle;

use crate::app_state::{AppMode, DataLoadingState};
use crate::map_loader::TokioRuntimeHandle;

use super::elevation::{fetch_elevation_for_bounds, ElevationConfig, ElevationTile};
use super::imagery::{fetch_imagery_for_bounds, ImagerySource, ImageryTile};
use super::terrain_mesh::SpawnRealTerrainEvent;
use super::tile_cache::TileCache;
use super::GeoBounds;

/// Plugin for loading real-world terrain
pub struct RealWorldLoaderPlugin;

impl Plugin for RealWorldLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<LoadRealWorldEvent>()
            .add_event::<RealWorldLoadedEvent>()
            .init_resource::<RealWorldLoaderState>()
            .add_systems(Update, (
                handle_load_request,
                poll_loading_task,
            ));
    }
}

/// Event to request loading a real-world location
#[derive(Event, Clone, Debug)]
pub struct LoadRealWorldEvent {
    /// Center latitude
    pub latitude: f64,
    /// Center longitude  
    pub longitude: f64,
    /// Radius in meters
    pub radius_m: f64,
    /// Zoom level for tiles (10-15 recommended)
    pub zoom: u8,
}

impl LoadRealWorldEvent {
    pub fn new(lat: f64, lon: f64, radius_m: f64) -> Self {
        // Auto-select zoom based on radius
        let zoom = if radius_m > 10_000.0 {
            10
        } else if radius_m > 5_000.0 {
            11
        } else if radius_m > 2_000.0 {
            12
        } else if radius_m > 1_000.0 {
            13
        } else {
            14
        };
        
        Self { latitude: lat, longitude: lon, radius_m, zoom }
    }
    
    /// Nebraska farm location (for testing)
    pub fn nebraska_farm() -> Self {
        Self::new(41.1621, -101.3542, 2000.0)
    }
    
    /// Iowa corn belt
    pub fn iowa_cornbelt() -> Self {
        Self::new(41.878, -93.098, 2000.0)
    }
    
    /// California Central Valley
    pub fn california_valley() -> Self {
        Self::new(36.778, -119.418, 2000.0)
    }
}

/// Event fired when loading completes
#[derive(Event)]
pub struct RealWorldLoadedEvent {
    pub bounds: GeoBounds,
    pub elevation_range: (f32, f32),
}

/// State for async loading
#[derive(Resource, Default)]
struct RealWorldLoaderState {
    loading_task: Option<tokio::task::JoinHandle<Result<LoadedWorldData>>>,
    pending_bounds: Option<GeoBounds>,
}

struct LoadedWorldData {
    bounds: GeoBounds,
    elevation_tiles: Vec<ElevationTile>,
    imagery_tiles: Vec<ImageryTile>,
}

fn handle_load_request(
    mut events: EventReader<LoadRealWorldEvent>,
    mut state: ResMut<RealWorldLoaderState>,
    mut loading_state: ResMut<DataLoadingState>,
    rt_handle: Option<Res<TokioRuntimeHandle>>,
) {
    let Some(event) = events.read().last() else {
        return;
    };
    
    let Some(rt) = rt_handle else {
        error!("No Tokio runtime available for async loading");
        return;
    };
    
    info!(
        "Starting real world load: ({:.4}, {:.4}) radius {:.0}m, zoom {}",
        event.latitude, event.longitude, event.radius_m, event.zoom
    );
    
    let bounds = GeoBounds::from_center(event.latitude, event.longitude, event.radius_m);
    let zoom = event.zoom;
    
    loading_state.is_loading = true;
    loading_state.progress = 0.1;
    loading_state.status_message = format!(
        "Loading terrain for ({:.4}, {:.4})...",
        event.latitude, event.longitude
    );
    
    state.pending_bounds = Some(bounds);
    
    // Spawn async loading task
    state.loading_task = Some(rt.0.spawn(async move {
        let mut cache = TileCache::default();
        
        // Ensure cache directory exists
        if let Err(e) = std::fs::create_dir_all(&cache.cache_dir) {
            tracing::warn!("Failed to create cache dir: {}", e);
        }
        
        // Fetch elevation
        tracing::info!("Fetching elevation data...");
        let elevation_tiles = fetch_elevation_for_bounds(bounds, zoom, &mut cache).await?;
        tracing::info!("Fetched {} elevation tiles", elevation_tiles.len());
        
        // Fetch imagery
        tracing::info!("Fetching satellite imagery...");
        let imagery_tiles = fetch_imagery_for_bounds(
            bounds, 
            zoom + 1, // Slightly higher zoom for imagery
            ImagerySource::EsriWorldImagery, 
            &mut cache
        ).await?;
        tracing::info!("Fetched {} imagery tiles", imagery_tiles.len());
        
        Ok(LoadedWorldData {
            bounds,
            elevation_tiles,
            imagery_tiles,
        })
    }));
}

fn poll_loading_task(
    mut state: ResMut<RealWorldLoaderState>,
    mut loading_state: ResMut<DataLoadingState>,
    mut spawn_terrain_events: EventWriter<SpawnRealTerrainEvent>,
    mut loaded_events: EventWriter<RealWorldLoadedEvent>,
) {
    let Some(task) = state.loading_task.as_mut() else {
        return;
    };
    
    // Check if task is complete
    if !task.is_finished() {
        // Update progress animation
        if loading_state.progress < 0.9 {
            loading_state.progress += 0.01;
        }
        return;
    }
    
    // Task complete, get result
    let task = state.loading_task.take().unwrap();
    
    match futures_lite::future::block_on(task) {
        Ok(Ok(data)) => {
            info!(
                "Real world data loaded: {} elevation tiles, {} imagery tiles",
                data.elevation_tiles.len(),
                data.imagery_tiles.len()
            );
            
            loading_state.status_message = "Building terrain mesh...".to_string();
            loading_state.progress = 0.95;
            
            // Calculate elevation range
            let mut min_elev = f32::MAX;
            let mut max_elev = f32::MIN;
            for tile in &data.elevation_tiles {
                min_elev = min_elev.min(tile.min_elevation);
                max_elev = max_elev.max(tile.max_elevation);
            }
            
            // Trigger terrain spawning
            spawn_terrain_events.send(SpawnRealTerrainEvent {
                bounds: data.bounds,
                elevation_tiles: data.elevation_tiles,
                imagery_tiles: data.imagery_tiles,
            });
            
            loaded_events.send(RealWorldLoadedEvent {
                bounds: data.bounds,
                elevation_range: (min_elev, max_elev),
            });
            
            loading_state.is_loading = false;
            loading_state.progress = 1.0;
            loading_state.status_message = "Terrain ready".to_string();
        }
        Ok(Err(e)) => {
            error!("Failed to load real world data: {}", e);
            loading_state.is_loading = false;
            loading_state.status_message = format!("Load failed: {}", e);
        }
        Err(e) => {
            error!("Task join error: {}", e);
            loading_state.is_loading = false;
            loading_state.status_message = format!("Task crashed: {}", e);
        }
    }
    
    state.pending_bounds = None;
}
