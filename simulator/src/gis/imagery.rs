//! Satellite Imagery Fetcher
//!
//! Fetches satellite imagery from various sources:
//! - ESRI World Imagery (free basemap tiles)
//! - Sentinel-2 via Copernicus (for NDVI computation)  
//! - NAIP (high-res US agriculture imagery)
//!
//! Returns images that can be used as terrain textures.

use anyhow::{anyhow, Context, Result};
use bevy::prelude::*;
use std::io::Cursor;

use super::{
    tile_cache::{TileCache, TileType},
    GeoBounds, TileCoord,
};

/// Configuration for imagery sources
#[derive(Resource, Clone)]
pub struct ImageryConfig {
    pub source: ImagerySource,
    pub tile_size: u32,
}

impl Default for ImageryConfig {
    fn default() -> Self {
        Self {
            source: ImagerySource::EsriWorldImagery,
            tile_size: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ImagerySource {
    /// ESRI World Imagery - Free, global, good quality
    #[default]
    EsriWorldImagery,
    /// OpenStreetMap style tiles (for reference)
    OpenStreetMap,
    /// USGS NAIP - High-res US agriculture (requires setup)
    Naip,
}

/// Decoded imagery tile
#[derive(Clone)]
pub struct ImageryTile {
    pub coord: TileCoord,
    pub width: u32,
    pub height: u32,
    /// RGBA pixel data, row-major
    pub pixels: Vec<u8>,
}

impl ImageryTile {
    /// Convert to Bevy Image for use as texture
    pub fn to_bevy_image(&self) -> Image {
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

        Image::new(
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            self.pixels.clone(),
            TextureFormat::Rgba8UnormSrgb,
            bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
        )
    }
}

/// Fetch imagery tile from ESRI World Imagery (free basemap)
pub async fn fetch_imagery_esri(coord: TileCoord) -> Result<ImageryTile> {
    // ESRI World Imagery - free for non-commercial use, good for development
    let url = format!(
        "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{}/{}/{}",
        coord.z, coord.y, coord.x
    );

    tracing::info!("Fetching imagery from: {}", url);

    let client = reqwest::Client::builder()
        .user_agent("AgBot-GIS/0.1")
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch imagery tile: HTTP {}",
            response.status()
        ));
    }

    let bytes = response.bytes().await?;
    decode_imagery_png(&bytes, coord)
}

/// Fetch imagery from OpenStreetMap (for reference/testing)
pub async fn fetch_imagery_osm(coord: TileCoord) -> Result<ImageryTile> {
    let url = format!(
        "https://tile.openstreetmap.org/{}/{}/{}.png",
        coord.z, coord.x, coord.y
    );

    let client = reqwest::Client::builder()
        .user_agent("AgBot-GIS/0.1")
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch OSM tile: HTTP {}",
            response.status()
        ));
    }

    let bytes = response.bytes().await?;
    decode_imagery_png(&bytes, coord)
}

/// Decode PNG/JPEG imagery into RGBA pixel data
fn decode_imagery_png(data: &[u8], coord: TileCoord) -> Result<ImageryTile> {
    // Try PNG first
    if let Ok(tile) = decode_png(data, coord) {
        return Ok(tile);
    }

    // Try JPEG
    decode_jpeg(data, coord)
}

fn decode_png(data: &[u8], coord: TileCoord) -> Result<ImageryTile> {
    let decoder = png::Decoder::new(Cursor::new(data));
    let mut reader = decoder.read_info().context("Failed to read PNG header")?;

    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .context("Failed to decode PNG frame")?;

    let width = info.width;
    let height = info.height;

    // Convert to RGBA
    let pixels = match info.color_type {
        png::ColorType::Rgba => buf[..((width * height * 4) as usize)].to_vec(),
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in buf.chunks(3) {
                if chunk.len() >= 3 {
                    rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                }
            }
            rgba
        }
        png::ColorType::Grayscale => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for &gray in buf.iter().take((width * height) as usize) {
                rgba.extend_from_slice(&[gray, gray, gray, 255]);
            }
            rgba
        }
        _ => return Err(anyhow!("Unsupported PNG color type: {:?}", info.color_type)),
    };

    Ok(ImageryTile {
        coord,
        width,
        height,
        pixels,
    })
}

fn decode_jpeg(data: &[u8], coord: TileCoord) -> Result<ImageryTile> {
    use std::io::BufReader;

    let mut decoder = jpeg_decoder::Decoder::new(BufReader::new(Cursor::new(data)));
    let pixels_raw = decoder.decode().context("Failed to decode JPEG")?;
    let info = decoder.info().context("Failed to get JPEG info")?;

    let width = info.width as u32;
    let height = info.height as u32;

    // Convert to RGBA
    let pixels = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixels_raw.chunks(3) {
                if chunk.len() >= 3 {
                    rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                }
            }
            rgba
        }
        jpeg_decoder::PixelFormat::L8 => {
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for &gray in pixels_raw.iter() {
                rgba.extend_from_slice(&[gray, gray, gray, 255]);
            }
            rgba
        }
        _ => return Err(anyhow!("Unsupported JPEG format")),
    };

    Ok(ImageryTile {
        coord,
        width,
        height,
        pixels,
    })
}

/// Fetch multiple imagery tiles to cover a geographic area
pub async fn fetch_imagery_for_bounds(
    bounds: GeoBounds,
    zoom: u8,
    source: ImagerySource,
    cache: &mut TileCache,
) -> Result<Vec<ImageryTile>> {
    let min_tile = TileCoord::from_latlon(bounds.max_lat, bounds.min_lon, zoom);
    let max_tile = TileCoord::from_latlon(bounds.min_lat, bounds.max_lon, zoom);

    let mut tiles = Vec::new();

    for x in min_tile.x..=max_tile.x {
        for y in min_tile.y..=max_tile.y {
            let coord = TileCoord { x, y, z: zoom };

            // Check cache first
            if let Some(cached) = cache.get_tile(&coord, TileType::Imagery) {
                if let Ok(tile) = decode_imagery_png(&cached.data, coord) {
                    tiles.push(tile);
                    continue;
                }
            }

            // Fetch from network
            let result = match source {
                ImagerySource::EsriWorldImagery => fetch_imagery_esri(coord).await,
                ImagerySource::OpenStreetMap => fetch_imagery_osm(coord).await,
                ImagerySource::Naip => fetch_imagery_esri(coord).await, // Fallback for now
            };

            match result {
                Ok(tile) => tiles.push(tile),
                Err(e) => tracing::warn!("Failed to fetch imagery tile {:?}: {}", coord, e),
            }
        }
    }

    Ok(tiles)
}

/// Composite multiple imagery tiles into a single texture
pub fn composite_imagery(tiles: &[ImageryTile], bounds: GeoBounds, output_size: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (output_size * output_size * 4) as usize];

    if tiles.is_empty() {
        // Fill with green placeholder
        for i in 0..(output_size * output_size) as usize {
            pixels[i * 4] = 50; // R
            pixels[i * 4 + 1] = 100; // G
            pixels[i * 4 + 2] = 50; // B
            pixels[i * 4 + 3] = 255; // A
        }
        return pixels;
    }

    for py in 0..output_size {
        for px in 0..output_size {
            let u = px as f64 / (output_size - 1) as f64;
            let v = py as f64 / (output_size - 1) as f64;

            let lat = bounds.max_lat - v * (bounds.max_lat - bounds.min_lat);
            let lon = bounds.min_lon + u * (bounds.max_lon - bounds.min_lon);

            // Find the tile containing this point
            for tile in tiles {
                let tile_bounds = tile.coord.bounds();
                if lat >= tile_bounds.min_lat
                    && lat <= tile_bounds.max_lat
                    && lon >= tile_bounds.min_lon
                    && lon <= tile_bounds.max_lon
                {
                    let tu = ((lon - tile_bounds.min_lon)
                        / (tile_bounds.max_lon - tile_bounds.min_lon))
                        as f32;
                    let tv = ((lat - tile_bounds.min_lat)
                        / (tile_bounds.max_lat - tile_bounds.min_lat))
                        as f32;
                    let tv = 1.0 - tv; // Flip Y

                    let tx = (tu * (tile.width - 1) as f32).round() as usize;
                    let ty = (tv * (tile.height - 1) as f32).round() as usize;
                    let src_idx = (ty * tile.width as usize + tx) * 4;
                    let dst_idx = ((py * output_size + px) * 4) as usize;

                    if src_idx + 3 < tile.pixels.len() && dst_idx + 3 < pixels.len() {
                        pixels[dst_idx..dst_idx + 4]
                            .copy_from_slice(&tile.pixels[src_idx..src_idx + 4]);
                    }
                    break;
                }
            }
        }
    }

    pixels
}

/// Compute NDVI-like index from RGB imagery (approximate)
///
/// Real NDVI requires NIR band, but we can approximate vegetation
/// from visible bands using: (G - R) / (G + R + 0.01)
pub fn compute_pseudo_ndvi(imagery: &[u8], width: u32, height: u32) -> Vec<f32> {
    let mut ndvi = Vec::with_capacity((width * height) as usize);

    for i in 0..(width * height) as usize {
        let r = imagery[i * 4] as f32 / 255.0;
        let g = imagery[i * 4 + 1] as f32 / 255.0;
        let _b = imagery[i * 4 + 2] as f32 / 255.0;

        // Excess Green Index (approximates vegetation)
        let egi = (2.0 * g - r - _b) / (g + r + _b + 0.01);
        ndvi.push(egi.clamp(-1.0, 1.0));
    }

    ndvi
}

/// Convert NDVI values to a color-coded RGBA image
pub fn ndvi_to_rgba(ndvi: &[f32], width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);

    for &value in ndvi {
        // Color ramp: brown -> yellow -> green
        let (r, g, b) = if value < 0.0 {
            // Bare soil / water - brown to gray
            let t = (value + 1.0) / 1.0; // -1..0 -> 0..1
            (
                (139.0 * (1.0 - t) + 100.0 * t) as u8,
                (90.0 * (1.0 - t) + 100.0 * t) as u8,
                (43.0 * (1.0 - t) + 100.0 * t) as u8,
            )
        } else if value < 0.3 {
            // Low vegetation - yellow to light green
            let t = value / 0.3;
            (
                (255.0 * (1.0 - t) + 144.0 * t) as u8,
                (255.0 * (1.0 - t) + 238.0 * t) as u8,
                (0.0 * (1.0 - t) + 144.0 * t) as u8,
            )
        } else {
            // Dense vegetation - light green to dark green
            let t = (value - 0.3) / 0.7;
            (
                (144.0 * (1.0 - t) + 0.0 * t) as u8,
                (238.0 * (1.0 - t) + 100.0 * t) as u8,
                (144.0 * (1.0 - t) + 0.0 * t) as u8,
            )
        };

        pixels.extend_from_slice(&[r, g, b, 200]); // Semi-transparent overlay
    }

    pixels
}
