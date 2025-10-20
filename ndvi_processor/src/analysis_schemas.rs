use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Analysis type enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AnalysisType {
    // Vegetation indices
    Ndvi,
    Evi,
    Savi,
    Arvi,
    Msavi,
    Cvi,
    Lai,
    FCover,
    
    // Water indices
    Ndwi,
    Mndwi,
    Awei,
    
    // Drought/Stress indices
    Vhi,
    Tci,
    Vci,
    Pdi,
    
    // Burn indices
    Nbr,
    Dnbr,
    Rdnbr,
    Bai,
    
    // Urban indices
    Ndbi,
    Ui,
    Ibi,
    
    // Snow indices
    Ndsi,
    Ndsii,
    S3,
    
    // Soil indices
    Bsi,
    Si,
    Ri,
    
    // Atmospheric indices
    Haze,
    Ci,
    
    // Combined analyses
    LandCover,
    ChangeDetection,
    Composite,
}

/// Health classification for vegetation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HealthStatus {
    Excellent,
    Good,
    Moderate,
    Poor,
    Critical,
    NoData,
}

/// Land cover classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LandCoverType {
    Water,
    Forest,
    Grassland,
    Cropland,
    Urban,
    BareSoil,
    Snow,
    Cloud,
    Shadow,
    Unknown,
}

/// Generic analysis result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub analysis_id: Uuid,
    pub analysis_type: AnalysisType,
    pub timestamp: DateTime<Utc>,
    pub source_images: Vec<Uuid>,
    pub output_path: String,
    pub statistics: IndexStatistics,
    pub metadata: AnalysisMetadata,
}

/// Statistical measures for any index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub median: f32,
    pub std_dev: f32,
    pub percentile_25: f32,
    pub percentile_75: f32,
    pub valid_pixels: u64,
    pub total_pixels: u64,
    pub coverage_percentage: f32,
}

/// Additional metadata for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    pub processing_time_ms: u64,
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
    pub quality_flags: Vec<QualityFlag>,
    pub coordinate_system: String,
    pub spatial_resolution: f64,
    pub bands_used: Vec<String>,
}

/// Quality assessment flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityFlag {
    CloudContamination(f32),
    ShadowContamination(f32),
    AtmosphericCorrection(bool),
    SensorCalibration(bool),
    GeometricCorrection(bool),
    TopographicCorrection(bool),
}

/// Vegetation analysis comprehensive result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VegetationAnalysisResult {
    pub analysis_result: AnalysisResult,
    pub health_classification: HealthClassification,
    pub biomass_estimate: BiomassEstimate,
    pub phenology: PhenologyMetrics,
    pub stress_indicators: StressIndicators,
}

/// Health classification breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthClassification {
    pub overall_health: HealthStatus,
    pub health_distribution: std::collections::HashMap<HealthStatus, f32>,
    pub degraded_areas: Vec<DegradedArea>,
}

/// Degraded area identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradedArea {
    pub area_id: String,
    pub coordinates: Vec<[f64; 2]>, // Polygon coordinates
    pub area_hectares: f64,
    pub severity: HealthStatus,
    pub likely_cause: Option<String>,
}

/// Biomass estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomassEstimate {
    pub total_biomass_tons: f64,
    pub biomass_density_tons_per_hectare: f64,
    pub carbon_stock_tons: f64,
    pub confidence_interval: (f64, f64),
    pub estimation_method: String,
}

/// Phenology metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhenologyMetrics {
    pub growth_stage: GrowthStage,
    pub days_since_planting: Option<u32>,
    pub days_to_harvest: Option<u32>,
    pub peak_green_date: Option<DateTime<Utc>>,
    pub senescence_start: Option<DateTime<Utc>>,
}

/// Growth stage classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GrowthStage {
    Germination,
    Emergence,
    Vegetative,
    Flowering,
    Fruiting,
    Maturity,
    Senescence,
    Dormant,
    Unknown,
}

/// Stress indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressIndicators {
    pub water_stress: StressLevel,
    pub nutrient_stress: StressLevel,
    pub disease_pressure: StressLevel,
    pub heat_stress: StressLevel,
    pub recommendations: Vec<String>,
}

/// Stress level classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StressLevel {
    None,
    Low,
    Moderate,
    High,
    Severe,
}

/// Water body analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterAnalysisResult {
    pub analysis_result: AnalysisResult,
    pub water_bodies: Vec<WaterBody>,
    pub total_water_area_hectares: f64,
    pub water_quality: WaterQuality,
    pub temporal_change: Option<TemporalChange>,
}

/// Individual water body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterBody {
    pub id: String,
    pub coordinates: Vec<[f64; 2]>, // Polygon coordinates
    pub area_hectares: f64,
    pub perimeter_meters: f64,
    pub water_type: WaterType,
    pub turbidity_level: TurbidityLevel,
}

/// Water body type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WaterType {
    River,
    Lake,
    Pond,
    Reservoir,
    Wetland,
    FloodPlain,
    Irrigation,
    Unknown,
}

/// Water turbidity classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurbidityLevel {
    Clear,
    SlightlyTurbid,
    Turbid,
    HighlyTurbid,
}

/// Water quality assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterQuality {
    pub overall_quality: WaterQualityLevel,
    pub chlorophyll_concentration: Option<f32>,
    pub turbidity_ntu: Option<f32>,
    pub algae_presence: AlgaeLevel,
    pub pollution_indicators: Vec<PollutionIndicator>,
}

/// Water quality levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WaterQualityLevel {
    Excellent,
    Good,
    Moderate,
    Poor,
    VeryPoor,
}

/// Algae presence levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AlgaeLevel {
    None,
    Low,
    Moderate,
    High,
    Bloom,
}

/// Pollution indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollutionIndicator {
    pub indicator_type: String,
    pub level: f32,
    pub threshold_exceeded: bool,
    pub source_likely: Option<String>,
}

/// Temporal change analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalChange {
    pub change_percentage: f32,
    pub change_type: ChangeType,
    pub trend: Trend,
    pub seasonality: bool,
    pub anomaly_detected: bool,
}

/// Change type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Increase,
    Decrease,
    Stable,
    Fluctuating,
}

/// Trend classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Trend {
    Increasing,
    Decreasing,
    Stable,
    Cyclic,
    Unknown,
}

/// Drought analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroughtAnalysisResult {
    pub analysis_result: AnalysisResult,
    pub drought_severity: DroughtSeverity,
    pub affected_area_hectares: f64,
    pub drought_duration_days: Option<u32>,
    pub recovery_probability: f32,
    pub impact_assessment: ImpactAssessment,
}

/// Drought severity classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DroughtSeverity {
    None,
    Mild,
    Moderate,
    Severe,
    Extreme,
}

/// Impact assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub crop_yield_impact: f32, // Percentage reduction
    pub economic_loss_estimate: Option<f64>, // Currency units
    pub water_resources_impact: f32,
    pub ecosystem_impact: f32,
    pub affected_population: Option<u32>,
}

/// Burn analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnAnalysisResult {
    pub analysis_result: AnalysisResult,
    pub burn_severity: BurnSeverity,
    pub burned_area_hectares: f64,
    pub recovery_stage: RecoveryStage,
    pub fire_progression: Option<FireProgression>,
}

/// Burn severity classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BurnSeverity {
    Unburned,
    Low,
    Moderate,
    High,
    HighPostFire,
}

/// Recovery stage classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryStage {
    Immediate,
    ShortTerm,
    MediumTerm,
    LongTerm,
    Recovered,
}

/// Fire progression tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireProgression {
    pub progression_rate: f32, // hectares per day
    pub direction: f32, // degrees
    pub intensity: f32,
    pub containment_probability: f32,
}

/// Land cover analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandCoverResult {
    pub analysis_result: AnalysisResult,
    pub land_cover_map: String, // Path to classified raster
    pub class_distribution: std::collections::HashMap<LandCoverType, f32>,
    pub confidence_map: String, // Path to confidence raster
    pub change_detection: Option<LandCoverChange>,
}

/// Land cover change analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandCoverChange {
    pub change_matrix: std::collections::HashMap<(LandCoverType, LandCoverType), f32>,
    pub net_change: std::collections::HashMap<LandCoverType, f32>,
    pub change_hotspots: Vec<ChangeHotspot>,
}

/// Change hotspot identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeHotspot {
    pub location: [f64; 2], // Center coordinates
    pub radius_meters: f64,
    pub change_type: (LandCoverType, LandCoverType),
    pub confidence: f32,
    pub area_affected: f64,
}

/// Multi-temporal analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTemporalResult {
    pub time_series: Vec<AnalysisResult>,
    pub trend_analysis: TrendAnalysis,
    pub anomaly_detection: AnomalyDetection,
    pub forecasting: Option<Forecast>,
}

/// Trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub overall_trend: Trend,
    pub trend_strength: f32,
    pub seasonal_component: bool,
    pub breakpoints: Vec<DateTime<Utc>>,
}

/// Anomaly detection results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetection {
    pub anomalies: Vec<Anomaly>,
    pub threshold: f32,
    pub detection_method: String,
}

/// Individual anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub timestamp: DateTime<Utc>,
    pub severity: f32,
    pub spatial_extent: Option<Vec<[f64; 2]>>,
    pub probable_cause: Option<String>,
}

/// Forecasting results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forecast {
    pub predictions: Vec<PredictionPoint>,
    pub confidence_intervals: Vec<(f32, f32)>,
    pub model_accuracy: f32,
    pub forecast_horizon_days: u32,
}

/// Individual prediction point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionPoint {
    pub timestamp: DateTime<Utc>,
    pub predicted_value: f32,
    pub confidence: f32,
}
