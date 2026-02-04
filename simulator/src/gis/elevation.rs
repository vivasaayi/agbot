//! Elevation Data Fetcher
//!
//! Fetches real elevation data from various sources:
//! - AWS Terrain Tiles (Mapzen format, free, global)
//! - USGS 3DEP (high-res US coverage)
//! - SRTM (30m global)
//!
//! Returns heightmaps that can be used to displace terrain meshes.

use anyhow::{anyhow, Context, Result};
use bevy::prelude::*;
use std::io::Cursor;

use super::{tile_cache::{TileCache, TileType}, GeoBounds, TileCoord};

/// Configuration for elevation data sources
#[derive(Resource, Clone)]
pub struct ElevationConfig {
    /// Primary elevation source
    pub source: ElevationSource,
    /// Tile size in pixels (typically 256 or 512)
    pub tile_size: u32,
    /// Vertical exaggeration factor
    pub vertical_scale: f32,
}

impl Default for ElevationConfig {
    fn default() -> Self {
        Self {
            source: ElevationSource::AwsTerrain,
            tile_size: 256,
            vertical_scale: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ElevationSource {
    /// AWS Terrain Tiles (Mapzen Terrarium format) - Free, global
    #[default]
    AwsTerrain,
    /// MapTiler terrain RGB
    MapTiler,
    /// Local file (for testing)
    LocalFile,
}

/// Decoded elevation data for a tile
#[derive(Clone)]
pub struct ElevationTile {
    pub coord: TileCoord,
    pub width: u32,
    pub height: u32,
    /// Elevation values in meters, row-major
    pub elevations: Vec<f32>,
    pub min_elevation: f32,
    pub max_elevation: f32,
}

impl ElevationTile {
    /// Sample elevation at normalized UV coordinates (0-1)
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let x = (u * (self.width - 1) as f32).round() as usize;
        let y = (v * (self.height - 1) as f32).round() as usize;
        let idx = y * self.width as usize + x;
        self.elevations.get(idx).copied().unwrap_or(0.0)
    }
    
    /// Sample elevation with bilinear interpolation
    pub fn sample_bilinear(&self, u: f32, v: f32) -> f32 {
        let fx = u * (self.width - 1) as f32;
        let fy = v * (self.height - 1) as f32;
        
        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1).min(self.width as usize - 1);
        let y1 = (y0 + 1).min(self.height as usize - 1);
        
        let fx = fx.fract();
        let fy = fy.fract();
        
        let v00 = self.elevations[y0 * self.width as usize + x0];
        let v10 = self.elevations[y0 * self.width as usize + x1];
        let v01 = self.elevations[y1 * self.width as usize + x0];
        let v11 = self.elevations[y1 * self.width as usize + x1];
        
        let v0 = v00 * (1.0 - fx) + v10 * fx;
        let v1 = v01 * (1.0 - fx) + v11 * fx;
        
        v0 * (1.0 - fy) + v1 * fy
    }
}

/// Fetch elevation tile from AWS Terrain Tiles (Mapzen Terrarium format)
/// 
/// The Terrarium format encodes elevation as RGB:
/// elevation = (R * 256 + G + B / 256) - 32768
pub async fn fetch_elevation_aws(coord: TileCoord) -> Result<ElevationTile> {
    // AWS Terrain Tiles endpoint (public, no API key needed)
    let url = format!(
        "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{}/{}/{}.png",
        coord.z, coord.x, coord.y
    );
    
    tracing::info!("Fetching elevation from: {}", url);
    
    let client = reqwest::Client::builder()
        .user_agent("AgBot-GIS/0.1")
        .build()?;
    
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        return Err(anyhow!("Failed to fetch elevation tile: HTTP {}", response.status()));
    }
    
    let bytes = response.bytes().await?;
    decode_terrarium_png(&bytes, coord)
}

/// Decode Terrarium-format PNG into elevation data
fn decode_terrarium_png(png_data: &[u8], coord: TileCoord) -> Result<ElevationTile> {
    let decoder = png::Decoder::new(Cursor::new(png_data));
    let mut reader = decoder.read_info().context("Failed to read PNG header")?;
    
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).context("Failed to decode PNG frame")?;
    
    let width = info.width;
    let height = info.height;
    
    // Terrarium format: elevation = (R * 256 + G + B / 256) - 32768
    let mut elevations = Vec::with_capacity((width * height) as usize);
    let mut min_elevation = f32::MAX;
    let mut max_elevation = f32::MIN;
    
    let bytes_per_pixel = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return Err(anyhow!("Unexpected PNG color type: {:?}", info.color_type)),
    };
    
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize * bytes_per_pixel;
            let r = buf[idx] as f32;
            let g = buf[idx + 1] as f32;
            let b = buf[idx + 2] as f32;
            
            let elevation = (r * 256.0 + g + b / 256.0) - 32768.0;
            elevations.push(elevation);
            
            min_elevation = min_elevation.min(elevation);
            max_elevation = max_elevation.max(elevation);
        }
    }
    
    tracing::info!(
        "Decoded elevation tile z={} x={} y={}: {}x{}, elevation range: {:.1}m to {:.1}m",
        coord.z, coord.x, coord.y, width, height, min_elevation, max_elevation
    );
    
    Ok(ElevationTile {
        coord,
        width,
        height,
        elevations,
        min_elevation,
        max_elevation,
    })
}

/// Fetch multiple elevation tiles to cover a geographic area
pub async fn fetch_elevation_for_bounds(
    bounds: GeoBounds,
    zoom: u8,
    cache: &mut TileCache,
) -> Result<Vec<ElevationTile>> {
    let min_tile = TileCoord::from_latlon(bounds.max_lat, bounds.min_lon, zoom); // Note: y is inverted
    let max_tile = TileCoord::from_latlon(bounds.min_lat, bounds.max_lon, zoom);
    
    let mut tiles = Vec::new();
    
    for x in min_tile.x..=max_tile.x {
        for y in min_tile.y..=max_tile.y {
            let coord = TileCoord { x, y, z: zoom };
            
            // Check cache first
            if let Some(cached) = cache.get_tile(&coord, TileType::Elevation) {
                if let Ok(tile) = decode_terrarium_png(&cached.data, coord) {
                    tiles.push(tile);
                    continue;
                }
            }
            
            // Fetch from network
            match fetch_elevation_aws(coord).await {
                Ok(tile) => {
                    // Cache the raw PNG data for next time
                    // (We'd need to re-encode or store raw - for now, just proceed)
                    tiles.push(tile);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch elevation tile {:?}: {}", coord, e);
                }
            }
        }
    }
    
    Ok(tiles)
}

/// Generate a composite heightmap from multiple tiles
pub fn composite_elevation(
    tiles: &[ElevationTile],
    bounds: GeoBounds,
    output_size: u32,
) -> Vec<f32> {
    let mut heightmap = vec![0.0f32; (output_size * output_size) as usize];
    
    if tiles.is_empty() {
        return heightmap;
    }
    
    let (center_lat, center_lon) = bounds.center();
    let width_m = bounds.width_m();
    let height_m = bounds.height_m();
    
    for py in 0..output_size {
        for px in 0..output_size {
            // Convert pixel to geographic coordinates
            let u = px as f64 / (output_size - 1) as f64;
            let v = py as f64 / (output_size - 1) as f64;
            
            let lat = bounds.max_lat - v * (bounds.max_lat - bounds.min_lat);
            let lon = bounds.min_lon + u * (bounds.max_lon - bounds.min_lon);
            
            // Find the tile containing this point and sample
            let mut elevation = 0.0f32;
            for tile in tiles {
                let tile_bounds = tile.coord.bounds();
                if lat >= tile_bounds.min_lat && lat <= tile_bounds.max_lat &&
                   lon >= tile_bounds.min_lon && lon <= tile_bounds.max_lon {
                    // Calculate UV within this tile
                    let tu = ((lon - tile_bounds.min_lon) / (tile_bounds.max_lon - tile_bounds.min_lon)) as f32;
                    let tv = 1.0 - ((lat - tile_bounds.min_lat) / (tile_bounds.max_lat - tile_bounds.min_lat)) as f32;
                    elevation = tile.sample_bilinear(tu, tv);
                    break;
                }
            }
            
            heightmap[(py * output_size + px) as usize] = elevation;
        }
    }
    
    heightmap
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tile_coord_from_latlon() {
        // Nebraska farmland
        let coord = TileCoord::from_latlon(41.5, -100.0, 10);
        assert!(coord.x > 0);
        assert!(coord.y > 0);
        assert_eq!(coord.z, 10);
    }
    
    #[test]
    fn test_geo_bounds_from_center() {
        let bounds = GeoBounds::from_center(41.5, -100.0, 1000.0);
        assert!(bounds.max_lat > bounds.min_lat);
        assert!(bounds.max_lon > bounds.min_lon);
        
        // Should be roughly 2km across
        let width = bounds.width_m();
        let height = bounds.height_m();
        assert!((width - 2000.0).abs() < 100.0);
        assert!((height - 2000.0).abs() < 100.0);
    }
}
