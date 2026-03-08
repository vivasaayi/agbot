//! GIS Data Pipeline
//!
//! This module provides real-world geographic data integration:
//! - Elevation data (SRTM, 3DEP)
//! - Satellite imagery (Sentinel-2, NAIP)
//! - Crop classification (USDA CDL)
//! - NDVI computation
//! - OSM features (field boundaries, buildings, roads)
//!
//! The goal is to build a custom GIS stack that can load any location
//! on Earth and render it with real terrain, imagery, and agricultural data.

pub mod cdl;
pub mod demo;
pub mod elevation;
pub mod imagery;
pub mod ndvi;
pub mod osm;
pub mod terrain_camera;
pub mod terrain_mesh;
pub mod tile_cache;
pub mod world_loader;

pub use cdl::{CdlConfig, CdlData, CdlStats, CropType};
pub use demo::RealWorldDemoPlugin;
pub use elevation::{ElevationConfig, ElevationTile};
pub use imagery::{ImageryConfig, ImageryTile};
pub use ndvi::{NdviColorScheme, NdviConfig, NdviData, NdviStats};
pub use osm::{GeoPoint, OsmConfig, OsmData, OsmFeature, OsmFeatureType};
pub use terrain_mesh::{RealTerrain, SpawnRealTerrainEvent, TerrainMeshConfig, TerrainReadyEvent};
pub use tile_cache::TileCache;
pub use world_loader::{LoadRealWorldEvent, RealWorldLoadedEvent};

use bevy::prelude::*;

pub struct GisPlugin;

impl Plugin for GisPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<tile_cache::TileCache>()
            .init_resource::<elevation::ElevationConfig>()
            .init_resource::<imagery::ImageryConfig>()
            .init_resource::<ndvi::NdviConfig>()
            .init_resource::<cdl::CdlConfig>()
            .init_resource::<osm::OsmConfig>()
            .add_plugins((
                terrain_mesh::TerrainMeshPlugin,
                world_loader::RealWorldLoaderPlugin,
                demo::RealWorldDemoPlugin,
                terrain_camera::TerrainCameraPlugin,
            ));
    }
}

/// Geographic bounding box
#[derive(Debug, Clone, Copy)]
pub struct GeoBounds {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

impl GeoBounds {
    pub fn from_center(lat: f64, lon: f64, radius_m: f64) -> Self {
        // Approximate degrees per meter at this latitude
        let lat_deg_per_m = 1.0 / 111_320.0;
        let lon_deg_per_m = 1.0 / (111_320.0 * lat.to_radians().cos().abs().max(0.01));

        let lat_delta = radius_m * lat_deg_per_m;
        let lon_delta = radius_m * lon_deg_per_m;

        Self {
            min_lat: lat - lat_delta,
            min_lon: lon - lon_delta,
            max_lat: lat + lat_delta,
            max_lon: lon + lon_delta,
        }
    }

    pub fn center(&self) -> (f64, f64) {
        (
            (self.min_lat + self.max_lat) / 2.0,
            (self.min_lon + self.max_lon) / 2.0,
        )
    }

    pub fn width_m(&self) -> f64 {
        let (lat, _) = self.center();
        let lon_deg_per_m = 1.0 / (111_320.0 * lat.to_radians().cos().abs().max(0.01));
        (self.max_lon - self.min_lon) / lon_deg_per_m
    }

    pub fn height_m(&self) -> f64 {
        (self.max_lat - self.min_lat) * 111_320.0
    }
}

/// Tile coordinate in a standard web mercator tiling scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
    pub z: u8, // zoom level
}

impl TileCoord {
    /// Convert lat/lon to tile coordinates at given zoom level
    pub fn from_latlon(lat: f64, lon: f64, zoom: u8) -> Self {
        let n = 2_u32.pow(zoom as u32) as f64;
        let x = ((lon + 180.0) / 360.0 * n).floor() as u32;
        let lat_rad = lat.to_radians();
        let y = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n).floor() as u32;
        Self { x, y, z: zoom }
    }

    /// Get the geographic bounds of this tile
    pub fn bounds(&self) -> GeoBounds {
        let n = 2_u32.pow(self.z as u32) as f64;
        let min_lon = self.x as f64 / n * 360.0 - 180.0;
        let max_lon = (self.x + 1) as f64 / n * 360.0 - 180.0;

        let min_lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * (self.y + 1) as f64 / n))
            .sinh()
            .atan();
        let max_lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * self.y as f64 / n))
            .sinh()
            .atan();

        GeoBounds {
            min_lat: min_lat_rad.to_degrees(),
            min_lon,
            max_lat: max_lat_rad.to_degrees(),
            max_lon,
        }
    }
}
