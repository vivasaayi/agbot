use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use shared::{config::AgroConfig, AgroResult};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "imagery_processor")]
#[command(
    about = "Imagery Processor: indices, thermal, and classification for remote sensing imagery"
)]
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
    /// Generic band override in role=name form, e.g. --band nir=AltNIR
    #[arg(long = "band", value_name = "ROLE=NAME")]
    pub band_overrides: Vec<BandOverrideSpec>,
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Serialize, Deserialize)]
pub enum IndexBandRole {
    Blue,
    Green,
    Red,
    Nir,
    RedEdge,
    Swir1,
    Swir2,
}

impl IndexBandRole {
    pub fn key(self) -> &'static str {
        match self {
            IndexBandRole::Blue => "blue",
            IndexBandRole::Green => "green",
            IndexBandRole::Red => "red",
            IndexBandRole::Nir => "nir",
            IndexBandRole::RedEdge => "red_edge",
            IndexBandRole::Swir1 => "swir1",
            IndexBandRole::Swir2 => "swir2",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            IndexBandRole::Blue => "Blue",
            IndexBandRole::Green => "Green",
            IndexBandRole::Red => "Red",
            IndexBandRole::Nir => "NIR",
            IndexBandRole::RedEdge => "Red-edge",
            IndexBandRole::Swir1 => "SWIR1",
            IndexBandRole::Swir2 => "SWIR2",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "blue" | "b" => Some(IndexBandRole::Blue),
            "green" | "g" => Some(IndexBandRole::Green),
            "red" | "r" => Some(IndexBandRole::Red),
            "nir" => Some(IndexBandRole::Nir),
            "red_edge" | "red-edge" | "rededge" | "re" => Some(IndexBandRole::RedEdge),
            "swir1" | "swir_1" | "swir-1" => Some(IndexBandRole::Swir1),
            "swir2" | "swir_2" | "swir-2" => Some(IndexBandRole::Swir2),
            _ => None,
        }
    }
}

impl FromStr for IndexBandRole {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_key(value).ok_or_else(|| format!("unknown band role '{value}'"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandOverrideSpec {
    pub role: IndexBandRole,
    pub band_name: String,
}

impl BandOverrideSpec {
    pub fn new(role: IndexBandRole, band_name: impl Into<String>) -> Self {
        Self {
            role,
            band_name: band_name.into(),
        }
    }
}

impl FromStr for BandOverrideSpec {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (role, band_name) = value
            .split_once('=')
            .or_else(|| value.split_once(':'))
            .ok_or_else(|| "band override must be role=name".to_string())?;
        let role = role.parse::<IndexBandRole>()?;
        let band_name = band_name.trim();
        if band_name.is_empty() {
            return Err("band override name cannot be empty".to_string());
        }

        Ok(Self::new(role, band_name))
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IndexCatalogError {
    #[error("{index:?} requires missing {band:?} band")]
    MissingRequiredBand {
        index: IndexKind,
        band: IndexBandRole,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IndexBandValues {
    values: BTreeMap<IndexBandRole, f32>,
}

impl IndexBandValues {
    pub fn with_band(mut self, band: IndexBandRole, value: f32) -> Self {
        self.values.insert(band, value);
        self
    }

    pub fn insert(&mut self, band: IndexBandRole, value: f32) {
        self.values.insert(band, value);
    }

    fn required(&self, index: IndexKind, band: IndexBandRole) -> Result<f32, IndexCatalogError> {
        self.values
            .get(&band)
            .copied()
            .ok_or(IndexCatalogError::MissingRequiredBand { index, band })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexPixelValue {
    Valid(f32),
    Invalid { reason: &'static str },
}

impl IndexPixelValue {
    pub fn value(self) -> Option<f32> {
        match self {
            IndexPixelValue::Valid(value) => Some(value),
            IndexPixelValue::Invalid { .. } => None,
        }
    }

    pub fn reason(self) -> Option<&'static str> {
        match self {
            IndexPixelValue::Valid(_) => None,
            IndexPixelValue::Invalid { reason } => Some(reason),
        }
    }

    fn map_value(self, transform: impl FnOnce(f32) -> f32) -> Self {
        match self {
            IndexPixelValue::Valid(value) => IndexPixelValue::Valid(transform(value)),
            invalid => invalid,
        }
    }
}

impl IndexKind {
    pub fn catalog() -> &'static [IndexKind] {
        &[
            IndexKind::Ndvi,
            IndexKind::Ndre,
            IndexKind::Evi,
            IndexKind::Savi,
            IndexKind::Vari,
            IndexKind::Gndvi,
            IndexKind::Ndwi,
            IndexKind::Mndwi,
            IndexKind::Msavi,
            IndexKind::Nbr,
            IndexKind::Ndmi,
            IndexKind::Evi2,
        ]
    }

    pub fn required_bands(self) -> &'static [IndexBandRole] {
        match self {
            IndexKind::Ndvi | IndexKind::Savi | IndexKind::Msavi | IndexKind::Evi2 => {
                &[IndexBandRole::Red, IndexBandRole::Nir]
            }
            IndexKind::Ndre => &[IndexBandRole::RedEdge, IndexBandRole::Nir],
            IndexKind::Evi => &[IndexBandRole::Blue, IndexBandRole::Red, IndexBandRole::Nir],
            IndexKind::Vari => &[
                IndexBandRole::Blue,
                IndexBandRole::Green,
                IndexBandRole::Red,
            ],
            IndexKind::Gndvi | IndexKind::Ndwi => &[IndexBandRole::Green, IndexBandRole::Nir],
            IndexKind::Mndwi => &[IndexBandRole::Green, IndexBandRole::Swir1],
            IndexKind::Nbr => &[IndexBandRole::Nir, IndexBandRole::Swir2],
            IndexKind::Ndmi => &[IndexBandRole::Nir, IndexBandRole::Swir1],
        }
    }

    pub fn expected_value_range(self) -> (f32, f32) {
        (-1.0, 1.0)
    }

    pub fn compute_value(
        self,
        values: &IndexBandValues,
    ) -> Result<IndexPixelValue, IndexCatalogError> {
        let pixel_value = match self {
            IndexKind::Ndvi => normalized_difference(
                values.required(self, IndexBandRole::Nir)?,
                values.required(self, IndexBandRole::Red)?,
            )
            .map_value(|value| value.clamp(-1.0, 1.0)),
            IndexKind::Ndre => normalized_difference(
                values.required(self, IndexBandRole::Nir)?,
                values.required(self, IndexBandRole::RedEdge)?,
            )
            .map_value(|value| value.clamp(-1.0, 1.0)),
            IndexKind::Evi => {
                let nir = values.required(self, IndexBandRole::Nir)?;
                let red = values.required(self, IndexBandRole::Red)?;
                let blue = values.required(self, IndexBandRole::Blue)?;
                ratio_or_invalid(2.5 * (nir - red), nir + 6.0 * red - 7.5 * blue + 1.0)
            }
            IndexKind::Savi => {
                let nir = values.required(self, IndexBandRole::Nir)?;
                let red = values.required(self, IndexBandRole::Red)?;
                let soil_brightness = 0.5;
                ratio_or_invalid(
                    (1.0 + soil_brightness) * (nir - red),
                    nir + red + soil_brightness,
                )
            }
            IndexKind::Vari => {
                let green = values.required(self, IndexBandRole::Green)?;
                let red = values.required(self, IndexBandRole::Red)?;
                let blue = values.required(self, IndexBandRole::Blue)?;
                ratio_or_invalid(green - red, green + red - blue)
            }
            IndexKind::Gndvi => normalized_difference(
                values.required(self, IndexBandRole::Nir)?,
                values.required(self, IndexBandRole::Green)?,
            ),
            IndexKind::Ndwi => normalized_difference(
                values.required(self, IndexBandRole::Green)?,
                values.required(self, IndexBandRole::Nir)?,
            ),
            IndexKind::Mndwi => normalized_difference(
                values.required(self, IndexBandRole::Green)?,
                values.required(self, IndexBandRole::Swir1)?,
            ),
            IndexKind::Msavi => {
                let nir = values.required(self, IndexBandRole::Nir)?;
                let red = values.required(self, IndexBandRole::Red)?;
                let term = (2.0 * nir + 1.0).powi(2) - 8.0 * (nir - red);
                if term < 0.0 {
                    IndexPixelValue::Invalid {
                        reason: "math_domain",
                    }
                } else {
                    IndexPixelValue::Valid((2.0 * nir + 1.0 - term.sqrt()) * 0.5)
                }
            }
            IndexKind::Nbr => normalized_difference(
                values.required(self, IndexBandRole::Nir)?,
                values.required(self, IndexBandRole::Swir2)?,
            ),
            IndexKind::Ndmi => normalized_difference(
                values.required(self, IndexBandRole::Nir)?,
                values.required(self, IndexBandRole::Swir1)?,
            ),
            IndexKind::Evi2 => {
                let nir = values.required(self, IndexBandRole::Nir)?;
                let red = values.required(self, IndexBandRole::Red)?;
                ratio_or_invalid(2.5 * (nir - red), nir + 2.4 * red + 1.0)
            }
        };

        Ok(pixel_value)
    }
}

fn normalized_difference(left: f32, right: f32) -> IndexPixelValue {
    ratio_or_invalid(left - right, left + right)
}

fn ratio_or_invalid(numerator: f32, denominator: f32) -> IndexPixelValue {
    if denominator.abs() <= f32::EPSILON {
        IndexPixelValue::Invalid {
            reason: "divide_by_zero",
        }
    } else {
        IndexPixelValue::Valid(numerator / denominator)
    }
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
    pub fn default_band_for_role(self, role: IndexBandRole) -> Option<&'static str> {
        match (self, role) {
            (SensorPreset::Sentinel2, IndexBandRole::Blue) => Some("B02"),
            (SensorPreset::Sentinel2, IndexBandRole::Green) => Some("B03"),
            (SensorPreset::Sentinel2, IndexBandRole::Red) => Some("B04"),
            (SensorPreset::Sentinel2, IndexBandRole::Nir) => Some("B08"),
            (SensorPreset::Sentinel2, IndexBandRole::RedEdge) => Some("B05"),
            (SensorPreset::Sentinel2, IndexBandRole::Swir1) => Some("B11"),
            (SensorPreset::Sentinel2, IndexBandRole::Swir2) => Some("B12"),
            (SensorPreset::Landsat8, IndexBandRole::Blue) => Some("B2"),
            (SensorPreset::Landsat8, IndexBandRole::Green) => Some("B3"),
            (SensorPreset::Landsat8, IndexBandRole::Red) => Some("B4"),
            (SensorPreset::Landsat8, IndexBandRole::Nir) => Some("B5"),
            (SensorPreset::Landsat8, IndexBandRole::RedEdge) => Some("B6"),
            (SensorPreset::Landsat8, IndexBandRole::Swir1) => Some("B6"),
            (SensorPreset::Landsat8, IndexBandRole::Swir2) => Some("B7"),
            (SensorPreset::DjiMultispectral, IndexBandRole::Blue) => Some("Blue"),
            (SensorPreset::DjiMultispectral, IndexBandRole::Green) => Some("Green"),
            (SensorPreset::DjiMultispectral, IndexBandRole::Red) => Some("Red"),
            (SensorPreset::DjiMultispectral, IndexBandRole::Nir) => Some("NIR"),
            (SensorPreset::DjiMultispectral, IndexBandRole::RedEdge) => Some("RE"),
            (SensorPreset::DjiMultispectral, IndexBandRole::Swir1) => None,
            (SensorPreset::DjiMultispectral, IndexBandRole::Swir2) => None,
        }
    }

    pub fn default_bands(
        self,
    ) -> (
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
    ) {
        (
            self.default_band_for_role(IndexBandRole::Red),
            self.default_band_for_role(IndexBandRole::Nir),
            self.default_band_for_role(IndexBandRole::RedEdge),
        )
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
    pub mod classify;
    pub mod indices;
    pub mod masks;
    pub mod thermal;
}

pub mod io;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResultMeta {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_images: Vec<uuid::Uuid>,
    pub output_path: String,
    pub index: String,
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    #[serde(default)]
    pub valid_pixel_count: usize,
    #[serde(default)]
    pub invalid_pixel_reasons: BTreeMap<String, usize>,
    pub radiometric_calibration: crate::io::RadiometricCalibrationEvidence,
    pub spatial_ref: shared::schemas::RasterSpatialRef,
}

#[cfg(test)]
mod index_catalog_tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "actual {actual} did not match expected {expected}"
        );
    }

    fn reflectance_vector() -> IndexBandValues {
        IndexBandValues::default()
            .with_band(IndexBandRole::Blue, 0.1)
            .with_band(IndexBandRole::Green, 0.4)
            .with_band(IndexBandRole::Red, 0.2)
            .with_band(IndexBandRole::Nir, 0.6)
            .with_band(IndexBandRole::RedEdge, 0.3)
            .with_band(IndexBandRole::Swir1, 0.25)
            .with_band(IndexBandRole::Swir2, 0.15)
    }

    #[test]
    fn full_index_catalog_declares_required_bands() {
        assert_eq!(IndexKind::catalog().len(), 12);
        assert_eq!(
            IndexKind::Ndvi.required_bands(),
            &[IndexBandRole::Red, IndexBandRole::Nir]
        );
        assert_eq!(
            IndexKind::Evi.required_bands(),
            &[IndexBandRole::Blue, IndexBandRole::Red, IndexBandRole::Nir]
        );
        assert_eq!(
            IndexKind::Mndwi.required_bands(),
            &[IndexBandRole::Green, IndexBandRole::Swir1]
        );
        assert_eq!(
            IndexKind::Nbr.required_bands(),
            &[IndexBandRole::Nir, IndexBandRole::Swir2]
        );
    }

    #[test]
    fn full_index_catalog_computes_known_vectors() {
        let values = reflectance_vector();
        let cases = [
            (IndexKind::Ndvi, 0.5),
            (IndexKind::Ndre, 0.33333334),
            (IndexKind::Evi, 0.4878049),
            (IndexKind::Savi, 0.4615385),
            (IndexKind::Vari, 0.4),
            (IndexKind::Gndvi, 0.2),
            (IndexKind::Ndwi, -0.2),
            (IndexKind::Mndwi, 0.23076923),
            (IndexKind::Msavi, 0.4596876),
            (IndexKind::Nbr, 0.6),
            (IndexKind::Ndmi, 0.4117647),
            (IndexKind::Evi2, 0.48076925),
        ];

        for (index, expected) in cases {
            let actual = index.compute_value(&values).unwrap().value().unwrap();
            assert_close(actual, expected);
            let (min, max) = index.expected_value_range();
            assert!(
                actual >= min && actual <= max,
                "{index:?} produced {actual}, outside [{min}, {max}]"
            );
        }
    }

    #[test]
    fn missing_required_band_is_named_by_index_and_role() {
        let values = IndexBandValues::default()
            .with_band(IndexBandRole::Red, 0.2)
            .with_band(IndexBandRole::Nir, 0.6);

        let error = IndexKind::Evi.compute_value(&values).unwrap_err();

        assert_eq!(
            error,
            IndexCatalogError::MissingRequiredBand {
                index: IndexKind::Evi,
                band: IndexBandRole::Blue
            }
        );
    }
}

#[cfg(test)]
mod sensor_preset_tests {
    use super::*;

    #[test]
    fn sensor_presets_supply_default_band_names_by_role() {
        assert_eq!(
            SensorPreset::Sentinel2.default_band_for_role(IndexBandRole::Blue),
            Some("B02")
        );
        assert_eq!(
            SensorPreset::Sentinel2.default_band_for_role(IndexBandRole::Swir2),
            Some("B12")
        );
        assert_eq!(
            SensorPreset::Landsat8.default_band_for_role(IndexBandRole::Swir1),
            Some("B6")
        );
        assert_eq!(
            SensorPreset::DjiMultispectral.default_band_for_role(IndexBandRole::RedEdge),
            Some("RE")
        );
    }

    #[test]
    fn generic_band_override_parses_role_and_band_name() {
        let override_spec: BandOverrideSpec = "nir=AltNIR".parse().unwrap();

        assert_eq!(override_spec.role, IndexBandRole::Nir);
        assert_eq!(override_spec.band_name, "AltNIR");
        assert!("swir3=Missing".parse::<BandOverrideSpec>().is_err());
        assert!("nir=".parse::<BandOverrideSpec>().is_err());
    }
}
