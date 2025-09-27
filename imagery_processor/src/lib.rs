use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use clap::{Parser, Subcommand, Args as ClapArgs, ValueEnum};
use shared::{config::AgroConfig, AgroResult};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "imagery_processor")]
#[command(about = "Imagery Processor: indices, thermal, and classification for remote sensing imagery")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compute spectral indices (e.g., NDVI, NDRE, EVI, SAVI)
    Indices(IndicesArgs),
    /// Estimate Land Surface Temperature (LST) from thermal band
    Thermal(ThermalArgs),
    /// Simple classification (threshold or k-means) over a raster
    Classify(ClassifyArgs),
    /// Generate QA-based masks (cloud/shadow/snow/water/clear) from QA bands
    Masks(MasksArgs),
}

#[derive(ClapArgs, Debug)]
pub struct IndicesArgs {
    /// Input directory containing metadata_*.json and band files
    #[arg(long)]
    pub input_dir: PathBuf,
    /// Output directory for results
    #[arg(long)]
    pub output_dir: PathBuf,
    /// Index type
    #[arg(long, value_enum, default_value_t = IndexKind::Ndvi)]
    pub index: IndexKind,
    /// Override band mapping: name for red band (e.g., Red, B04)
    #[arg(long)]
    pub red: Option<String>,
    /// Override band mapping: name for nir band (e.g., NIR, B08)
    #[arg(long)]
    pub nir: Option<String>,
    /// Override band mapping: name for red-edge band (e.g., RE, B05)
    #[arg(long)]
    pub red_edge: Option<String>,
    /// Override band mapping: name for green band (e.g., Green, B03)
    #[arg(long)]
    pub green: Option<String>,
    /// Override band mapping: name for blue band (e.g., Blue, B02)
    #[arg(long)]
    pub blue: Option<String>,
    /// Override band mapping: name for SWIR1 band (e.g., SWIR1, B11/B6)
    #[arg(long)]
    pub swir1: Option<String>,
    /// Override band mapping: name for SWIR2 band (e.g., SWIR2, B12/B7)
    #[arg(long)]
    pub swir2: Option<String>,
    /// Output format: png (default) or geotiff (requires --feature gdal-io)
    #[arg(long, value_enum, default_value_t = OutputFormat::Png)]
    pub out_format: OutputFormat,
    /// Sensor preset for band mapping (overrides can still be applied)
    #[arg(long, value_enum)]
    pub sensor: Option<SensorPreset>,
    /// Optional mask image path (non-zero = valid). Applied before stats.
    #[arg(long)]
    pub mask: Option<PathBuf>,
}

#[derive(ClapArgs, Debug)]
pub struct ThermalArgs {
    /// Input directory containing metadata_*.json and thermal band files
    #[arg(long)]
    pub input_dir: PathBuf,
    /// Output directory for results
    #[arg(long)]
    pub output_dir: PathBuf,
    /// Band name for thermal channel (e.g., TIRS1, Thermal)
    #[arg(long)]
    pub thermal_band: String,
    /// Optional second thermal band (e.g., B11) for split-window
    #[arg(long)]
    pub thermal_band2: Option<String>,
    /// Optional radiance multiplicative factor (ML)
    #[arg(long)]
    pub ml: Option<f32>,
    /// Optional radiance additive factor (AL)
    #[arg(long)]
    pub al: Option<f32>,
    /// Optional brightness temperature constant K1
    #[arg(long)]
    pub k1: Option<f32>,
    /// Optional brightness temperature constant K2
    #[arg(long)]
    pub k2: Option<f32>,
    /// Surface emissivity (epsilon), default 0.98
    #[arg(long, default_value_t = 0.98)]
    pub emissivity: f32,
    /// Effective wavelength of thermal band in micrometers (e.g., 10.895 for Landsat-8 B10)
    #[arg(long, default_value_t = 10.895)]
    pub lambda_um: f32,
    /// Temperature output unit
    #[arg(long, value_enum, default_value_t = TemperatureUnit::Kelvin)]
    pub unit: TemperatureUnit,
    /// Products to output: radiance, bt (brightness temperature), lst (comma-separated)
    #[arg(long, value_enum, value_delimiter = ',', num_args = 1.., default_values_t = [ThermalProduct::Lst])]
    pub products: Vec<ThermalProduct>,
    /// Use split-window when two TIR bands available (simple average by default)
    #[arg(long, default_value_t = false)]
    pub split_window: bool,
    /// Use NDVI-based emissivity instead of constant
    #[arg(long, default_value_t = false)]
    pub emissivity_from_ndvi: bool,
    /// Optional NDVI image path (PNG or GeoTIFF) for emissivity estimation
    #[arg(long)]
    pub ndvi_image: Option<PathBuf>,
    /// Optional red/nir overrides to compute NDVI on the fly if ndvi_image is not provided
    #[arg(long)]
    pub red: Option<String>,
    #[arg(long)]
    pub nir: Option<String>,
    /// Output format: png (default) or geotiff (requires --feature gdal-io)
    #[arg(long, value_enum, default_value_t = OutputFormat::Png)]
    pub out_format: OutputFormat,
    /// Optional mask image path (non-zero = valid)
    #[arg(long)]
    pub mask: Option<PathBuf>,
}

#[derive(ClapArgs, Debug)]
pub struct ClassifyArgs {
    /// Path to an index image (PNG) produced by `indices` command
    #[arg(long)]
    pub input_image: PathBuf,
    /// Output path for classification mask (PNG)
    #[arg(long)]
    pub output_path: PathBuf,
    /// Threshold-based classification: vegetation threshold (e.g., 0.3)
    #[arg(long)]
    pub threshold: Option<f32>,
    /// Use k-means with k clusters instead of threshold
    #[arg(long)]
    pub kmeans: Option<usize>,
}

#[derive(ClapArgs, Debug)]
pub struct MasksArgs {
    /// Input directory containing metadata_*.json and QA band files
    #[arg(long)]
    pub input_dir: PathBuf,
    /// Output directory for mask results
    #[arg(long)]
    pub output_dir: PathBuf,
    /// QA band name (e.g., QA_PIXEL for Landsat)
    #[arg(long, default_value = "QA_PIXEL")]
    pub qa_band: String,
    /// Mask kinds to generate; if omitted, all are generated
    #[arg(long, value_enum)]
    pub kinds: Vec<MaskKind>,
    /// Output format for masks
    #[arg(long, value_enum, default_value_t = OutputFormat::Png)]
    pub out_format: OutputFormat,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Debug)]
pub enum IndexKind {
    Ndvi,
    Ndre,
    Evi,
    Savi,
    Vari,
    Gndvi,
    Ndwi,
    Mndwi,
    Msavi,
    Nbr,
    Ndmi,
    Evi2,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Debug)]
pub enum OutputFormat {
    Png,
    Geotiff,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Debug)]
pub enum TemperatureUnit {
    Kelvin,
    Celsius,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Debug)]
pub enum ThermalProduct {
    Radiance,
    Bt,
    Lst,
}

fn default_thermal_products() -> Vec<ThermalProduct> {
    vec![ThermalProduct::Lst]
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Debug)]
pub enum MaskKind {
    Cloud,
    CloudShadow,
    Snow,
    Water,
    Clear,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, ValueEnum, Debug)]
pub enum SensorPreset {
    Sentinel2,
    Landsat8,
    DjiMultispectral,
}

impl SensorPreset {
    pub fn default_bands(self) -> (Option<&'static str>, Option<&'static str>, Option<&'static str>) {
        match self {
            SensorPreset::Sentinel2 => (Some("B04"), Some("B08"), Some("B05")), // Red, NIR, RE
            SensorPreset::Landsat8 => (Some("B4"), Some("B5"), Some("B6")),     // Red, NIR, RE-ish
            SensorPreset::DjiMultispectral => (Some("Red"), Some("NIR"), Some("RE")),
        }
    }
}

pub struct Processor {
    #[allow(dead_code)]
    config: Arc<AgroConfig>,
}

impl Processor {
    pub async fn new() -> AgroResult<Self> {
        let config = Arc::new(AgroConfig::load()?);
        Ok(Self { config })
    }

    pub async fn run_indices(&self, args: &IndicesArgs) -> AgroResult<()> {
        crate::pipeline::indices::run_indices(args).await
    }

    pub async fn run_thermal(&self, args: &ThermalArgs) -> AgroResult<()> {
        crate::pipeline::thermal::run_thermal(args).await
    }

    pub async fn run_classify(&self, args: &ClassifyArgs) -> AgroResult<()> {
        crate::pipeline::classify::run_classify(args).await
    }

    pub async fn run_masks(&self, args: &MasksArgs) -> AgroResult<()> {
        crate::pipeline::masks::run_masks(args).await
    }
}

pub mod pipeline {
    pub mod indices;
    pub mod thermal;
    pub mod classify;
    pub mod masks;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResultMeta {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_images: Vec<uuid::Uuid>,
    pub output_path: String,
    pub index: String,
    pub min: f32,
    pub max: f32,
    pub mean: f32,
}
