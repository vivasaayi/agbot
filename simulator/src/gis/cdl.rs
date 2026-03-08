//! USDA Cropland Data Layer (CDL) Integration
//!
//! Fetches and displays crop classification data from the USDA National
//! Agricultural Statistics Service (NASS). The CDL provides annual crop-specific
//! land cover data for the continental United States.
//!
//! Data source: https://nassgeodata.gmu.edu/CropScape/

use anyhow::{anyhow, Context, Result};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

use super::{
    tile_cache::{TileCache, TileType},
    GeoBounds, TileCoord,
};

/// CDL configuration
#[derive(Resource, Clone)]
pub struct CdlConfig {
    /// Whether CDL overlay is enabled
    pub enabled: bool,
    /// Opacity of the overlay (0.0-1.0)
    pub opacity: f32,
    /// Year of CDL data (2008-2023 typically available)
    pub year: u16,
    /// Whether to show crop labels in UI
    pub show_labels: bool,
}

impl Default for CdlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 0.7,
            year: 2023,
            show_labels: true,
        }
    }
}

/// Crop type classification from CDL
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CropType {
    // Row Crops
    Corn = 1,
    Cotton = 2,
    Rice = 3,
    Sorghum = 4,
    Soybeans = 5,
    Sunflower = 6,
    Peanuts = 10,
    Tobacco = 11,
    SweetCorn = 12,
    PopCorn = 13,

    // Small Grains
    Barley = 21,
    DurumWheat = 22,
    SpringWheat = 23,
    WinterWheat = 24,
    Oats = 28,
    Millet = 29,
    Rye = 30,

    // Specialty Crops
    Canola = 31,
    Flaxseed = 32,
    Safflower = 33,
    Mustard = 35,
    Alfalfa = 36,
    OtherHay = 37,
    Sugarbeets = 41,
    DryBeans = 42,
    Potatoes = 43,

    // Vegetables
    Tomatoes = 54,
    Onions = 49,
    Cucumbers = 50,
    Lettuce = 227,
    Peppers = 216,

    // Fruits & Nuts
    Grapes = 69,
    Citrus = 72,
    Almonds = 75,
    Walnuts = 76,
    Pecans = 77,

    // Pasture & Forest
    GrasslandPasture = 176,
    Shrubland = 152,
    DeciduousForest = 141,
    EvergreenForest = 142,
    MixedForest = 143,
    Wetlands = 190,

    // Developed & Other
    DevelopedOpen = 121,
    DevelopedLow = 122,
    DevelopedMed = 123,
    DevelopedHigh = 124,
    Barren = 131,
    Water = 111,

    // Fallback
    Unknown = 0,
}

impl CropType {
    /// Get crop type from CDL raster value
    pub fn from_cdl_value(value: u8) -> Self {
        match value {
            1 => Self::Corn,
            2 => Self::Cotton,
            3 => Self::Rice,
            4 => Self::Sorghum,
            5 => Self::Soybeans,
            6 => Self::Sunflower,
            10 => Self::Peanuts,
            11 => Self::Tobacco,
            12 => Self::SweetCorn,
            13 => Self::PopCorn,
            21 => Self::Barley,
            22 => Self::DurumWheat,
            23 => Self::SpringWheat,
            24 => Self::WinterWheat,
            28 => Self::Oats,
            29 => Self::Millet,
            30 => Self::Rye,
            31 => Self::Canola,
            32 => Self::Flaxseed,
            33 => Self::Safflower,
            35 => Self::Mustard,
            36 => Self::Alfalfa,
            37 => Self::OtherHay,
            41 => Self::Sugarbeets,
            42 => Self::DryBeans,
            43 => Self::Potatoes,
            49 => Self::Onions,
            50 => Self::Cucumbers,
            54 => Self::Tomatoes,
            69 => Self::Grapes,
            72 => Self::Citrus,
            75 => Self::Almonds,
            76 => Self::Walnuts,
            77 => Self::Pecans,
            111 => Self::Water,
            121 => Self::DevelopedOpen,
            122 => Self::DevelopedLow,
            123 => Self::DevelopedMed,
            124 => Self::DevelopedHigh,
            131 => Self::Barren,
            141 => Self::DeciduousForest,
            142 => Self::EvergreenForest,
            143 => Self::MixedForest,
            152 => Self::Shrubland,
            176 => Self::GrasslandPasture,
            190 => Self::Wetlands,
            216 => Self::Peppers,
            227 => Self::Lettuce,
            _ => Self::Unknown,
        }
    }

    /// Get the display name for this crop type
    pub fn name(&self) -> &'static str {
        match self {
            Self::Corn => "Corn",
            Self::Cotton => "Cotton",
            Self::Rice => "Rice",
            Self::Sorghum => "Sorghum",
            Self::Soybeans => "Soybeans",
            Self::Sunflower => "Sunflower",
            Self::Peanuts => "Peanuts",
            Self::Tobacco => "Tobacco",
            Self::SweetCorn => "Sweet Corn",
            Self::PopCorn => "Popcorn",
            Self::Barley => "Barley",
            Self::DurumWheat => "Durum Wheat",
            Self::SpringWheat => "Spring Wheat",
            Self::WinterWheat => "Winter Wheat",
            Self::Oats => "Oats",
            Self::Millet => "Millet",
            Self::Rye => "Rye",
            Self::Canola => "Canola",
            Self::Flaxseed => "Flaxseed",
            Self::Safflower => "Safflower",
            Self::Mustard => "Mustard",
            Self::Alfalfa => "Alfalfa",
            Self::OtherHay => "Other Hay",
            Self::Sugarbeets => "Sugarbeets",
            Self::DryBeans => "Dry Beans",
            Self::Potatoes => "Potatoes",
            Self::Tomatoes => "Tomatoes",
            Self::Onions => "Onions",
            Self::Cucumbers => "Cucumbers",
            Self::Lettuce => "Lettuce",
            Self::Peppers => "Peppers",
            Self::Grapes => "Grapes",
            Self::Citrus => "Citrus",
            Self::Almonds => "Almonds",
            Self::Walnuts => "Walnuts",
            Self::Pecans => "Pecans",
            Self::GrasslandPasture => "Grassland/Pasture",
            Self::Shrubland => "Shrubland",
            Self::DeciduousForest => "Deciduous Forest",
            Self::EvergreenForest => "Evergreen Forest",
            Self::MixedForest => "Mixed Forest",
            Self::Wetlands => "Wetlands",
            Self::DevelopedOpen => "Developed (Open)",
            Self::DevelopedLow => "Developed (Low)",
            Self::DevelopedMed => "Developed (Medium)",
            Self::DevelopedHigh => "Developed (High)",
            Self::Barren => "Barren",
            Self::Water => "Water",
            Self::Unknown => "Unknown",
        }
    }

    /// Get the standard CDL color for this crop type (RGB)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            // Row crops - yellows and oranges
            Self::Corn => (255, 211, 0),      // Yellow
            Self::Soybeans => (38, 115, 0),   // Dark green
            Self::Cotton => (255, 165, 226),  // Pink
            Self::Rice => (0, 168, 228),      // Blue
            Self::Sorghum => (255, 109, 9),   // Orange
            Self::Sunflower => (255, 255, 0), // Bright yellow

            // Wheat - tans and browns
            Self::WinterWheat => (166, 112, 0),  // Brown
            Self::SpringWheat => (210, 166, 92), // Light brown
            Self::DurumWheat => (176, 137, 51),  // Tan
            Self::Barley => (255, 221, 165),     // Light tan
            Self::Oats => (209, 255, 115),       // Light green

            // Legumes and forage
            Self::Alfalfa => (0, 175, 75),     // Green
            Self::OtherHay => (215, 215, 158), // Pale green
            Self::DryBeans => (255, 190, 190), // Light pink
            Self::Potatoes => (112, 68, 137),  // Purple

            // Vegetables
            Self::Tomatoes => (255, 0, 0),    // Red
            Self::Onions => (190, 120, 130),  // Pink-brown
            Self::Cucumbers => (0, 200, 100), // Green
            Self::Lettuce => (150, 255, 150), // Light green
            Self::Peppers => (255, 80, 80),   // Red-orange

            // Fruits and nuts
            Self::Grapes => (111, 0, 138),  // Purple
            Self::Citrus => (255, 170, 0),  // Orange
            Self::Almonds => (166, 82, 41), // Brown
            Self::Walnuts => (139, 90, 43), // Dark brown
            Self::Pecans => (160, 82, 45),  // Sienna

            // Natural vegetation
            Self::GrasslandPasture => (227, 227, 194), // Pale yellow-green
            Self::Shrubland => (204, 191, 138),        // Tan
            Self::DeciduousForest => (109, 163, 73),   // Forest green
            Self::EvergreenForest => (27, 120, 55),    // Dark green
            Self::MixedForest => (71, 140, 65),        // Green
            Self::Wetlands => (126, 196, 193),         // Cyan

            // Developed areas
            Self::DevelopedOpen => (222, 166, 149), // Light red
            Self::DevelopedLow => (217, 146, 130),  // Medium red
            Self::DevelopedMed => (211, 127, 112),  // Darker red
            Self::DevelopedHigh => (173, 83, 77),   // Dark red

            // Other
            Self::Water => (76, 112, 163),    // Blue
            Self::Barren => (179, 174, 163),  // Gray
            Self::Unknown => (128, 128, 128), // Gray

            // Fill in remaining
            _ => (200, 200, 200),
        }
    }
}

/// CDL tile data
#[derive(Clone)]
pub struct CdlTile {
    pub coord: TileCoord,
    pub width: u32,
    pub height: u32,
    /// Raw CDL class values (0-255)
    pub values: Vec<u8>,
}

/// Computed CDL data for a region
#[derive(Clone)]
pub struct CdlData {
    /// Crop type per pixel
    pub crop_types: Vec<CropType>,
    /// Resolution
    pub width: u32,
    pub height: u32,
    /// Geographic bounds
    pub bounds: GeoBounds,
    /// Crop acreage statistics
    pub stats: CdlStats,
}

#[derive(Clone, Debug, Default)]
pub struct CdlStats {
    /// Percentage of each crop type
    pub crop_percentages: HashMap<CropType, f32>,
    /// Dominant crop type
    pub dominant_crop: Option<CropType>,
    /// Total agricultural land percentage
    pub agricultural_percent: f32,
}

/// Fetch CDL data from USDA CropScape WMS
pub async fn fetch_cdl_for_bounds(
    bounds: GeoBounds,
    year: u16,
    resolution: u32,
    _cache: &mut TileCache,
) -> Result<CdlData> {
    // CropScape WMS endpoint
    let url = format!(
        "https://nassgeodata.gmu.edu/CropScapeService/wms_cdl.cgi?\
         SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&\
         LAYERS=cdl_{year}&\
         CRS=EPSG:4326&\
         BBOX={},{},{},{}&\
         WIDTH={resolution}&HEIGHT={resolution}&\
         FORMAT=image/png",
        bounds.min_lon,
        bounds.min_lat,
        bounds.max_lon,
        bounds.max_lat,
        year = year
    );

    tracing::info!("Fetching CDL data from: {}", url);

    let client = reqwest::Client::builder()
        .user_agent("AgBot-GIS/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch CDL data: HTTP {}",
            response.status()
        ));
    }

    let bytes = response.bytes().await?;
    decode_cdl_png(&bytes, bounds, resolution)
}

/// Decode CDL PNG response
fn decode_cdl_png(data: &[u8], bounds: GeoBounds, resolution: u32) -> Result<CdlData> {
    use std::io::Cursor;

    let decoder = png::Decoder::new(Cursor::new(data));
    let mut reader = decoder
        .read_info()
        .context("Failed to read CDL PNG header")?;

    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .context("Failed to decode CDL PNG")?;

    let width = info.width;
    let height = info.height;

    // CDL typically returns indexed PNG with palette
    // For simplicity, we'll extract the dominant channel as the class value
    let values: Vec<u8> = match info.color_type {
        png::ColorType::Indexed => buf[..(width * height) as usize].to_vec(),
        png::ColorType::Rgba => {
            // Take first channel (should be palette index in some cases)
            buf.chunks(4).map(|c| c[0]).collect()
        }
        png::ColorType::Rgb => {
            // Map RGB back to CDL class - lookup nearest color
            buf.chunks(3)
                .map(|rgb| rgb_to_cdl_class(rgb[0], rgb[1], rgb[2]))
                .collect()
        }
        png::ColorType::Grayscale => buf[..(width * height) as usize].to_vec(),
        _ => return Err(anyhow!("Unsupported CDL color type: {:?}", info.color_type)),
    };

    // Convert to crop types
    let crop_types: Vec<CropType> = values
        .iter()
        .map(|&v| CropType::from_cdl_value(v))
        .collect();

    // Compute statistics
    let stats = compute_cdl_stats(&crop_types);

    Ok(CdlData {
        crop_types,
        width,
        height,
        bounds,
        stats,
    })
}

/// Map RGB color back to CDL class (for RGB encoded CDL)
fn rgb_to_cdl_class(r: u8, g: u8, b: u8) -> u8 {
    // Use color distance to find nearest CDL class
    let crops = [
        CropType::Corn,
        CropType::Soybeans,
        CropType::WinterWheat,
        CropType::Alfalfa,
        CropType::Cotton,
        CropType::Rice,
        CropType::GrasslandPasture,
        CropType::DeciduousForest,
        CropType::Water,
        CropType::DevelopedLow,
        CropType::Barren,
    ];

    let mut best_match = CropType::Unknown;
    let mut best_dist = f32::MAX;

    for crop in crops {
        let (cr, cg, cb) = crop.color();
        let dist = ((r as f32 - cr as f32).powi(2)
            + (g as f32 - cg as f32).powi(2)
            + (b as f32 - cb as f32).powi(2))
        .sqrt();
        if dist < best_dist {
            best_dist = dist;
            best_match = crop;
        }
    }

    best_match as u8
}

/// Compute CDL statistics
fn compute_cdl_stats(crop_types: &[CropType]) -> CdlStats {
    let mut counts: HashMap<CropType, u32> = HashMap::new();

    for &crop in crop_types {
        *counts.entry(crop).or_insert(0) += 1;
    }

    let total = crop_types.len() as f32;
    let mut percentages: HashMap<CropType, f32> = HashMap::new();
    let mut dominant_crop = None;
    let mut max_count = 0u32;
    let mut ag_count = 0u32;

    for (&crop, &count) in &counts {
        let pct = (count as f32 / total) * 100.0;
        percentages.insert(crop, pct);

        if count > max_count {
            max_count = count;
            dominant_crop = Some(crop);
        }

        // Count agricultural land (crops, not forest/water/developed)
        match crop {
            CropType::Water
            | CropType::DevelopedOpen
            | CropType::DevelopedLow
            | CropType::DevelopedMed
            | CropType::DevelopedHigh
            | CropType::Barren
            | CropType::DeciduousForest
            | CropType::EvergreenForest
            | CropType::MixedForest => {}
            _ => ag_count += count,
        }
    }

    CdlStats {
        crop_percentages: percentages,
        dominant_crop,
        agricultural_percent: (ag_count as f32 / total) * 100.0,
    }
}

/// Convert CDL data to RGBA texture
pub fn cdl_to_texture(cdl: &CdlData, config: &CdlConfig) -> Image {
    let mut pixels = Vec::with_capacity((cdl.width * cdl.height * 4) as usize);
    let alpha = (config.opacity * 255.0) as u8;

    for crop in &cdl.crop_types {
        let (r, g, b) = crop.color();
        pixels.extend_from_slice(&[r, g, b, alpha]);
    }

    Image::new(
        Extent3d {
            width: cdl.width,
            height: cdl.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Create a legend texture for CDL crop types
pub fn create_cdl_legend(crops: &[CropType], cell_size: u32) -> (Image, Vec<(CropType, u32, u32)>) {
    let num_crops = crops.len() as u32;
    let width = cell_size;
    let height = cell_size * num_crops;

    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    let mut positions = Vec::new();

    for (i, &crop) in crops.iter().enumerate() {
        let (r, g, b) = crop.color();
        positions.push((crop, 0, i as u32 * cell_size));

        for _ in 0..cell_size {
            for _ in 0..cell_size {
                pixels.extend_from_slice(&[r, g, b, 255]);
            }
        }
    }

    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );

    (image, positions)
}
