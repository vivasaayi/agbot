use serde::{Deserialize, Serialize};
use shared::schemas::{
    assert_raster_spatial_ref, GeoBounds, RasterResolution, RasterSpatialRef, RasterSpatialRefError,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropModelTask {
    StandCount,
    CanopyCover,
    DiseaseDetection,
    PestDetection,
    WeedMapping,
}

impl CropModelTask {
    pub fn as_str(self) -> &'static str {
        match self {
            CropModelTask::StandCount => "stand_count",
            CropModelTask::CanopyCover => "canopy_cover",
            CropModelTask::DiseaseDetection => "disease_detection",
            CropModelTask::PestDetection => "pest_detection",
            CropModelTask::WeedMapping => "weed_mapping",
        }
    }
}

impl std::str::FromStr for CropModelTask {
    type Err = CropModelRegistryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "stand_count" => Ok(CropModelTask::StandCount),
            "canopy_cover" => Ok(CropModelTask::CanopyCover),
            "disease_detection" => Ok(CropModelTask::DiseaseDetection),
            "pest_detection" => Ok(CropModelTask::PestDetection),
            "weed_mapping" => Ok(CropModelTask::WeedMapping),
            _ => Err(CropModelRegistryError::UnsupportedTask {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelVersionRegistrationRequest {
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub version: String,
    pub task: CropModelTask,
    #[serde(default)]
    pub training_set_ref: String,
    #[serde(default = "default_model_metrics")]
    pub metrics: serde_json::Value,
    #[serde(default)]
    pub provenance_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelVersionRecord {
    pub model_id: String,
    pub version: String,
    pub task: CropModelTask,
    pub training_set_ref: String,
    pub metrics: serde_json::Value,
    pub provenance_ref: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceModelReference {
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelGateResponse {
    pub model_id: String,
    pub version: String,
    pub registered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceRunStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

impl InferenceRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            InferenceRunStatus::Queued => "queued",
            InferenceRunStatus::Running => "running",
            InferenceRunStatus::Completed => "completed",
            InferenceRunStatus::Failed => "failed",
        }
    }
}

impl std::str::FromStr for InferenceRunStatus {
    type Err = InferenceRunError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "queued" => Ok(InferenceRunStatus::Queued),
            "running" => Ok(InferenceRunStatus::Running),
            "completed" => Ok(InferenceRunStatus::Completed),
            "failed" => Ok(InferenceRunStatus::Failed),
            _ => Err(InferenceRunError::UnsupportedStatus {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InferenceRunSubmissionRequest {
    #[serde(default)]
    pub run_id: Option<String>,
    pub mosaic_ref: String,
    pub field_id: String,
    pub season_id: String,
    #[serde(default)]
    pub model: Option<InferenceModelReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceRunRecord {
    pub run_id: String,
    pub mosaic_ref: String,
    pub field_id: String,
    pub season_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub model_version: String,
    pub status: InferenceRunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledInferenceInput {
    pub tile_id: String,
    pub width_px: u32,
    pub height_px: u32,
    pub spatial_ref: RasterSpatialRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledInferenceTile {
    pub tile_id: String,
    pub width_px: u32,
    pub height_px: u32,
    pub spatial_ref: RasterSpatialRef,
    pub footprint: GeoBounds,
    pub resolution: RasterResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TiledInferenceSkipReason {
    InvalidGeoreference,
    CrsMismatch,
    EvaluatorFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TiledInferenceTileSkip {
    pub tile_id: String,
    pub reason: TiledInferenceSkipReason,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledInferenceTileOutput<T> {
    pub tile_id: String,
    pub spatial_ref: RasterSpatialRef,
    pub footprint: GeoBounds,
    pub result: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledInferenceReport<T> {
    pub field_crs: String,
    pub field_extent: GeoBounds,
    pub tiles_total: usize,
    pub tiles_processed: usize,
    pub tiles_skipped: usize,
    pub outputs: Vec<TiledInferenceTileOutput<T>>,
    pub skipped_tiles: Vec<TiledInferenceTileSkip>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlantCountConfig {
    pub min_component_pixels: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantCountTile {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub valid: bool,
    pub width_px: u32,
    pub height_px: u32,
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
    pub crop_mask: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantCountZeroReason {
    InvalidTile,
    NoValidCropPixels,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantLocation {
    pub plant_id: String,
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub crs: String,
    pub x_m: f64,
    pub y_m: f64,
    pub pixel_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilePlantCount {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub plant_count: usize,
    pub tile_area_m2: f64,
    pub density_plants_per_ha: f64,
    #[serde(default)]
    pub zero_reason: Option<PlantCountZeroReason>,
    pub plant_locations: Vec<PlantLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZonePlantCount {
    pub zone_id: String,
    pub plant_count: usize,
    pub area_m2: f64,
    pub density_plants_per_ha: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandCountReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub total_count: usize,
    pub field_area_m2: f64,
    pub field_density_plants_per_ha: f64,
    pub tiles: Vec<TilePlantCount>,
    pub zones: Vec<ZonePlantCount>,
    pub plant_locations: Vec<PlantLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanopyCoverConfig {
    pub vegetation_index_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanopyCoverTile {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub width_px: u32,
    pub height_px: u32,
    pub spatial_ref: RasterSpatialRef,
    pub index_values: Vec<f64>,
    pub valid_mask: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanopyCoverMask {
    pub width_px: u32,
    pub height_px: u32,
    pub vegetation_mask: Vec<bool>,
    pub valid_mask: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileCanopyCover {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub spatial_ref: RasterSpatialRef,
    pub total_pixels: usize,
    pub valid_pixels: usize,
    pub vegetation_pixels: usize,
    pub excluded_pixels: usize,
    pub pixel_area_m2: f64,
    pub valid_area_m2: f64,
    pub excluded_area_m2: f64,
    pub cover_fraction: f64,
    pub mask: CanopyCoverMask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneCanopyCover {
    pub zone_id: String,
    pub valid_pixels: usize,
    pub vegetation_pixels: usize,
    pub excluded_pixels: usize,
    pub valid_area_m2: f64,
    pub excluded_area_m2: f64,
    pub cover_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanopyCoverReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub valid_pixels: usize,
    pub vegetation_pixels: usize,
    pub excluded_pixels: usize,
    pub valid_area_m2: f64,
    pub excluded_area_m2: f64,
    pub cover_fraction: f64,
    pub tiles: Vec<TileCanopyCover>,
    pub zones: Vec<ZoneCanopyCover>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GrowthStageConfig {
    pub emergence_cover_max: f64,
    pub vegetative_cover_min: f64,
    pub reproductive_cover_min: f64,
    pub min_index_observations_for_confidence: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowthIndexObservation {
    pub observed_at: String,
    pub mean_index_value: f64,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowthStage {
    Emergence,
    Vegetative,
    Reproductive,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowthStageConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowthStageEstimate {
    pub field_id: String,
    pub crop: String,
    pub generated_at: String,
    pub stage: GrowthStage,
    pub confidence: GrowthStageConfidence,
    pub cover_fraction: f64,
    pub stand_density_plants_per_ha: f64,
    pub index_observation_count: usize,
    pub index_delta: Option<f64>,
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DiseaseDetectionConfig {
    pub low_confidence_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiseaseLesionCandidate {
    pub tile_id: String,
    pub confidence: f64,
    pub bbox: GeoBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionZoneGeometry {
    pub crs: String,
    pub bbox: GeoBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiseaseLesionDetection {
    pub detection_id: String,
    pub confidence: f64,
    pub low_confidence: bool,
    pub evidence_tile_ref: String,
    pub zone_geometry: DetectionZoneGeometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiseaseDetectionReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub model: ModelGateResponse,
    pub deterministic_cover_valid_pixels: usize,
    pub low_confidence_count: usize,
    pub detections: Vec<DiseaseLesionDetection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PestDetectionConfig {
    pub detection_threshold: f64,
    pub low_confidence_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PestDetectionCandidate {
    pub tile_id: String,
    pub pest_label: String,
    pub confidence: f64,
    pub bbox: GeoBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PestDetection {
    pub detection_id: String,
    pub pest_label: String,
    pub confidence: f64,
    pub low_confidence: bool,
    pub evidence_tile_ref: String,
    pub zone_geometry: DetectionZoneGeometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PestDetectionReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub model: ModelGateResponse,
    pub deterministic_cover_valid_pixels: usize,
    pub detection_threshold: f64,
    pub rejected_candidate_count: usize,
    pub low_confidence_count: usize,
    pub detections: Vec<PestDetection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeedMappingConfig {
    pub low_confidence_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeedZoneCandidate {
    pub tile_id: String,
    pub confidence: f64,
    pub bbox: GeoBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeedMapZone {
    pub zone_id: String,
    pub confidence: f64,
    pub low_confidence: bool,
    pub evidence_tile_ref: String,
    pub area_m2: f64,
    pub geometry: DetectionZoneGeometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeedMapReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub model: ModelGateResponse,
    pub deterministic_cover_valid_pixels: usize,
    pub total_weed_area_m2: f64,
    pub low_confidence_count: usize,
    pub zones: Vec<WeedMapZone>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionVerificationState {
    Unverified,
    Confirmed,
    Rejected,
    Corrected,
}

impl DetectionVerificationState {
    pub fn as_str(self) -> &'static str {
        match self {
            DetectionVerificationState::Unverified => "unverified",
            DetectionVerificationState::Confirmed => "confirmed",
            DetectionVerificationState::Rejected => "rejected",
            DetectionVerificationState::Corrected => "corrected",
        }
    }
}

impl Default for DetectionVerificationState {
    fn default() -> Self {
        Self::Unverified
    }
}

impl std::str::FromStr for DetectionVerificationState {
    type Err = CropDetectionVerificationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "unverified" => Ok(DetectionVerificationState::Unverified),
            "confirmed" => Ok(DetectionVerificationState::Confirmed),
            "rejected" => Ok(DetectionVerificationState::Rejected),
            "corrected" => Ok(DetectionVerificationState::Corrected),
            _ => Err(CropDetectionVerificationError::InvalidVerificationState {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropDetectionVerificationAction {
    Confirmed,
    Rejected,
    Corrected,
}

impl CropDetectionVerificationAction {
    fn verification_state(self) -> DetectionVerificationState {
        match self {
            CropDetectionVerificationAction::Confirmed => DetectionVerificationState::Confirmed,
            CropDetectionVerificationAction::Rejected => DetectionVerificationState::Rejected,
            CropDetectionVerificationAction::Corrected => DetectionVerificationState::Corrected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropDetectionVerificationRequest {
    pub detection_id: String,
    pub task: CropModelTask,
    pub label: String,
    pub confidence: f64,
    #[serde(default)]
    pub evidence_tile_refs: Vec<String>,
    pub zone_geometry: DetectionZoneGeometry,
    pub action: CropDetectionVerificationAction,
    pub actor: String,
    pub verified_at: String,
    #[serde(default)]
    pub corrected_label: Option<String>,
    #[serde(default)]
    pub corrected_geometry: Option<DetectionZoneGeometry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropDetectionCorrectionLabel {
    pub label_id: String,
    pub source_detection_id: String,
    pub task: CropModelTask,
    pub label: String,
    pub geometry: DetectionZoneGeometry,
    pub actor: String,
    pub created_at: String,
    pub evidence_tile_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropDetectionVerificationRecord {
    pub detection_id: String,
    pub task: CropModelTask,
    pub label: String,
    pub confidence: f64,
    pub evidence_tile_refs: Vec<String>,
    pub zone_geometry: DetectionZoneGeometry,
    pub verification_state: DetectionVerificationState,
    pub actor: String,
    pub verified_at: String,
    #[serde(default)]
    pub corrected_label: Option<String>,
    #[serde(default)]
    pub corrected_geometry: Option<DetectionZoneGeometry>,
    #[serde(default)]
    pub correction_label: Option<CropDetectionCorrectionLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingPromotionRequest {
    pub detection_id: String,
    pub verification_state: DetectionVerificationState,
    #[serde(default)]
    pub allow_unverified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingPromotionDecision {
    pub detection_id: String,
    pub verification_state: DetectionVerificationState,
    pub promotion_allowed: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropDetectionFindingRequest {
    pub finding_id: String,
    pub field_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub detection: CropDetectionVerificationRecord,
    pub model: InferenceModelReference,
    pub emitted_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropDetectionFindingRecord {
    pub finding_id: String,
    pub finding_type: CropModelTask,
    pub field_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub detection_id: String,
    pub label: String,
    pub confidence: f64,
    pub evidence_tile_refs: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub model_version: InferenceModelReference,
    pub verification_state: DetectionVerificationState,
    pub zone_geometry: DetectionZoneGeometry,
    pub emitted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CropModelRegistryError {
    #[error("model_id cannot be empty")]
    EmptyModelId,
    #[error("model version cannot be empty")]
    EmptyVersion,
    #[error("training_set_ref cannot be empty")]
    EmptyTrainingSetRef,
    #[error("provenance_ref cannot be empty")]
    EmptyProvenanceRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("metrics must be a non-empty JSON object")]
    InvalidMetrics,
    #[error("unsupported crop model task {value}")]
    UnsupportedTask { value: String },
    #[error("unregistered model {model_id}@{version}")]
    UnregisteredModel { model_id: String, version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InferenceRunError {
    #[error("run_id cannot be empty")]
    EmptyRunId,
    #[error("mosaic_ref cannot be empty")]
    EmptyMosaicRef,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("season_id cannot be empty")]
    EmptySeasonId,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("updated_at cannot be empty")]
    EmptyUpdatedAt,
    #[error("failed inference runs require a failure_reason_code")]
    EmptyFailureReason,
    #[error("non-failed inference runs cannot carry a failure_reason_code")]
    UnexpectedFailureReason,
    #[error("inference run status {value} is invalid")]
    UnsupportedStatus { value: String },
    #[error("invalid inference run transition {from:?} -> {to:?}")]
    InvalidTransition {
        from: InferenceRunStatus,
        to: InferenceRunStatus,
    },
    #[error("inference model gate failed: {source}")]
    ModelGate {
        #[source]
        source: CropModelRegistryError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TiledInferenceError {
    #[error("tiled inference requires at least one tile")]
    EmptyTiles,
    #[error("tiled inference had no valid georeferenced tiles")]
    NoValidTiles,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StandCountError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("CRS cannot be empty")]
    EmptyCrs,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("stand count requires at least one tile")]
    EmptyTiles,
    #[error("min_component_pixels must be greater than zero")]
    InvalidConfig,
    #[error("tile {tile_id} has invalid geometry")]
    InvalidTileGeometry { tile_id: String },
    #[error("tile {tile_id} crop mask length does not match dimensions")]
    CropMaskSizeMismatch { tile_id: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CanopyCoverError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("canopy cover requires at least one tile")]
    EmptyTiles,
    #[error("vegetation_index_threshold must be finite")]
    InvalidThreshold,
    #[error("tile_id cannot be empty")]
    EmptyTileId,
    #[error("tile {tile_id} has invalid spatial reference: {source}")]
    SpatialRefInvalid {
        tile_id: String,
        #[source]
        source: RasterSpatialRefError,
    },
    #[error("tile {tile_id} index values length does not match dimensions")]
    IndexSizeMismatch { tile_id: String },
    #[error("tile {tile_id} valid mask length does not match dimensions")]
    ValidMaskSizeMismatch { tile_id: String },
    #[error("tile {tile_id} CRS {actual} does not match field CRS {expected}")]
    CrsMismatch {
        tile_id: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrowthStageError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("crop cannot be empty")]
    EmptyCrop,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error(
        "stand count field_id {stand_field_id} does not match canopy field_id {canopy_field_id}"
    )]
    FieldMismatch {
        stand_field_id: String,
        canopy_field_id: String,
    },
    #[error("growth stage config is invalid")]
    InvalidConfig,
    #[error("index observation has invalid value or missing evidence")]
    InvalidIndexObservation,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DiseaseDetectionError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("deterministic canopy cover is required before disease detection")]
    DeterministicCoverRequired,
    #[error("disease model gate failed: {source}")]
    ModelGate {
        #[source]
        source: CropModelRegistryError,
    },
    #[error("low_confidence_threshold must be finite and between 0 and 1")]
    InvalidThreshold,
    #[error("tile_id cannot be empty")]
    EmptyTileId,
    #[error("candidate on tile {tile_id} has invalid confidence")]
    InvalidConfidence { tile_id: String },
    #[error("candidate on tile {tile_id} has invalid zone geometry")]
    InvalidZoneGeometry { tile_id: String },
    #[error("candidate references tile {tile_id} without deterministic cover evidence")]
    MissingCoverTile { tile_id: String },
    #[error("cover report field {actual} does not match requested field {expected}")]
    CoverFieldMismatch { expected: String, actual: String },
    #[error("candidate on tile {tile_id} falls outside the deterministic cover tile extent")]
    ZoneOutsideTileExtent { tile_id: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PestDetectionError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("deterministic canopy cover is required before pest detection")]
    DeterministicCoverRequired,
    #[error("pest model gate failed: {source}")]
    ModelGate {
        #[source]
        source: CropModelRegistryError,
    },
    #[error("detection thresholds must be finite, between 0 and 1, and ordered")]
    InvalidThreshold,
    #[error("tile_id cannot be empty")]
    EmptyTileId,
    #[error("pest_label cannot be empty")]
    EmptyPestLabel,
    #[error("candidate on tile {tile_id} has invalid confidence")]
    InvalidConfidence { tile_id: String },
    #[error("candidate on tile {tile_id} has invalid zone geometry")]
    InvalidZoneGeometry { tile_id: String },
    #[error("candidate references tile {tile_id} without deterministic cover evidence")]
    MissingCoverTile { tile_id: String },
    #[error("cover report field {actual} does not match requested field {expected}")]
    CoverFieldMismatch { expected: String, actual: String },
    #[error("candidate on tile {tile_id} falls outside the deterministic cover tile extent")]
    ZoneOutsideTileExtent { tile_id: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WeedMappingError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("deterministic canopy cover is required before weed mapping")]
    DeterministicCoverRequired,
    #[error("weed model gate failed: {source}")]
    ModelGate {
        #[source]
        source: CropModelRegistryError,
    },
    #[error("low_confidence_threshold must be finite and between 0 and 1")]
    InvalidThreshold,
    #[error("tile_id cannot be empty")]
    EmptyTileId,
    #[error("candidate on tile {tile_id} has invalid confidence")]
    InvalidConfidence { tile_id: String },
    #[error("candidate on tile {tile_id} has invalid zone geometry")]
    InvalidZoneGeometry { tile_id: String },
    #[error("candidate references tile {tile_id} without deterministic cover evidence")]
    MissingCoverTile { tile_id: String },
    #[error("cover report field {actual} does not match requested field {expected}")]
    CoverFieldMismatch { expected: String, actual: String },
    #[error("candidate on tile {tile_id} falls outside the deterministic cover tile extent")]
    ZoneOutsideTileExtent { tile_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CropDetectionVerificationError {
    #[error("detection_id cannot be empty")]
    EmptyDetectionId,
    #[error("detection label cannot be empty")]
    EmptyLabel,
    #[error("detection confidence must be finite and between 0 and 1")]
    InvalidConfidence,
    #[error("detection evidence_tile_refs cannot be empty")]
    EmptyEvidence,
    #[error("detection evidence_tile_refs cannot contain empty values")]
    EmptyEvidenceRef,
    #[error("detection geometry CRS cannot be empty")]
    EmptyGeometryCrs,
    #[error("detection geometry bbox is invalid")]
    InvalidGeometry,
    #[error("verification actor cannot be empty")]
    EmptyActor,
    #[error("verification timestamp cannot be empty")]
    EmptyVerifiedAt,
    #[error("corrected label cannot be empty")]
    EmptyCorrectedLabel,
    #[error("corrected geometry CRS cannot be empty")]
    EmptyCorrectedGeometryCrs,
    #[error("corrected geometry bbox is invalid")]
    InvalidCorrectedGeometry,
    #[error("corrected verification requires a corrected label or geometry")]
    MissingCorrection,
    #[error("verification state {value} is invalid")]
    InvalidVerificationState { value: String },
    #[error("task {task:?} is not a pest, disease, or weed detection task")]
    UnsupportedDetectionTask { task: CropModelTask },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FindingPromotionError {
    #[error("detection_id cannot be empty")]
    EmptyDetectionId,
    #[error("unverified detection {detection_id} cannot be promoted without explicit override")]
    UnverifiedDetectionBlocked { detection_id: String },
    #[error("rejected detection {detection_id} cannot be promoted to a finding")]
    RejectedDetectionBlocked { detection_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CropDetectionFindingError {
    #[error("finding_id cannot be empty")]
    EmptyFindingId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("emitted_at cannot be empty")]
    EmptyEmittedAt,
    #[error("model_id cannot be empty")]
    EmptyModelId,
    #[error("model version cannot be empty")]
    EmptyModelVersion,
    #[error("finding evidence_tile_refs cannot be empty")]
    EmptyEvidence,
    #[error("finding evidence_tile_refs cannot contain empty values")]
    EmptyEvidenceRef,
    #[error("detection {detection_id} is unverified and cannot be emitted as a finding")]
    UnverifiedDetection { detection_id: String },
    #[error("detection {detection_id} was rejected and cannot be emitted as a finding")]
    RejectedDetection { detection_id: String },
}

pub fn build_model_version_record(
    request: ModelVersionRegistrationRequest,
    created_at: String,
) -> Result<ModelVersionRecord, CropModelRegistryError> {
    let model_id = normalize_required_text(request.model_id, CropModelRegistryError::EmptyModelId)?;
    let version = normalize_required_text(request.version, CropModelRegistryError::EmptyVersion)?;
    let training_set_ref = normalize_required_text(
        request.training_set_ref,
        CropModelRegistryError::EmptyTrainingSetRef,
    )?;
    let provenance_ref = normalize_required_text(
        request.provenance_ref,
        CropModelRegistryError::EmptyProvenanceRef,
    )?;
    let created_at = normalize_required_text(created_at, CropModelRegistryError::EmptyCreatedAt)?;
    validate_metrics(&request.metrics)?;

    Ok(ModelVersionRecord {
        model_id,
        version,
        task: request.task,
        training_set_ref,
        metrics: request.metrics,
        provenance_ref,
        created_at,
    })
}

pub fn validate_model_reference(
    reference: InferenceModelReference,
    registered: bool,
) -> Result<ModelGateResponse, CropModelRegistryError> {
    let model_id =
        normalize_required_text(reference.model_id, CropModelRegistryError::EmptyModelId)?;
    let version = normalize_required_text(reference.version, CropModelRegistryError::EmptyVersion)?;

    if !registered {
        return Err(CropModelRegistryError::UnregisteredModel { model_id, version });
    }

    Ok(ModelGateResponse {
        model_id,
        version,
        registered,
    })
}

pub fn build_inference_run_record(
    request: InferenceRunSubmissionRequest,
    generated_run_id: String,
    created_at: String,
    model_registered: Option<bool>,
) -> Result<InferenceRunRecord, InferenceRunError> {
    let run_id = normalize_optional_run_text(request.run_id)
        .or_else(|| normalize_run_text(generated_run_id))
        .ok_or(InferenceRunError::EmptyRunId)?;
    let mosaic_ref =
        normalize_run_text(request.mosaic_ref).ok_or(InferenceRunError::EmptyMosaicRef)?;
    let field_id = normalize_run_text(request.field_id).ok_or(InferenceRunError::EmptyFieldId)?;
    let season_id =
        normalize_run_text(request.season_id).ok_or(InferenceRunError::EmptySeasonId)?;
    let created_at = normalize_run_text(created_at).ok_or(InferenceRunError::EmptyCreatedAt)?;
    let (model_id, model_version) = if let Some(model) = request.model {
        let gate = validate_model_reference(model, model_registered.unwrap_or(false))
            .map_err(|source| InferenceRunError::ModelGate { source })?;
        (Some(gate.model_id), gate.version)
    } else {
        (None, "deterministic".to_string())
    };

    Ok(InferenceRunRecord {
        run_id,
        mosaic_ref,
        field_id,
        season_id,
        model_id,
        model_version,
        status: InferenceRunStatus::Queued,
        failure_reason_code: None,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn transition_inference_run_status(
    mut record: InferenceRunRecord,
    next_status: InferenceRunStatus,
    failure_reason_code: Option<String>,
    updated_at: String,
) -> Result<InferenceRunRecord, InferenceRunError> {
    validate_inference_run_transition(record.status, next_status)?;
    let updated_at = normalize_run_text(updated_at).ok_or(InferenceRunError::EmptyUpdatedAt)?;
    let failure_reason_code = normalize_optional_run_text(failure_reason_code);
    if next_status == InferenceRunStatus::Failed && failure_reason_code.is_none() {
        return Err(InferenceRunError::EmptyFailureReason);
    }
    if next_status != InferenceRunStatus::Failed && failure_reason_code.is_some() {
        return Err(InferenceRunError::UnexpectedFailureReason);
    }

    record.status = next_status;
    record.failure_reason_code = failure_reason_code;
    record.updated_at = updated_at;

    Ok(record)
}

pub fn validate_inference_run_transition(
    current: InferenceRunStatus,
    next: InferenceRunStatus,
) -> Result<(), InferenceRunError> {
    match (current, next) {
        (InferenceRunStatus::Queued, InferenceRunStatus::Running)
        | (InferenceRunStatus::Running, InferenceRunStatus::Completed)
        | (InferenceRunStatus::Running, InferenceRunStatus::Failed) => Ok(()),
        (from, to) => Err(InferenceRunError::InvalidTransition { from, to }),
    }
}

pub fn run_tiled_inference_pipeline<T, F>(
    tiles: Vec<TiledInferenceInput>,
    mut evaluator: F,
) -> Result<TiledInferenceReport<T>, TiledInferenceError>
where
    F: FnMut(&TiledInferenceTile) -> Result<T, String>,
{
    if tiles.is_empty() {
        return Err(TiledInferenceError::EmptyTiles);
    }

    let tiles_total = tiles.len();
    let mut field_crs: Option<String> = None;
    let mut field_extent: Option<GeoBounds> = None;
    let mut outputs = Vec::new();
    let mut skipped_tiles = Vec::new();

    for tile in tiles {
        let tile_id = normalize_optional_run_text(Some(tile.tile_id.clone()))
            .unwrap_or_else(|| "unnamed_tile".to_string());
        let spatial_ref =
            match assert_raster_spatial_ref(Some(&tile.spatial_ref), tile.width_px, tile.height_px)
            {
                Ok(spatial_ref) => spatial_ref,
                Err(error) => {
                    skipped_tiles.push(TiledInferenceTileSkip {
                        tile_id,
                        reason: TiledInferenceSkipReason::InvalidGeoreference,
                        detail: error.to_string(),
                    });
                    continue;
                }
            };
        let tile_crs = spatial_ref.crs.clone().unwrap_or_default();
        if let Some(expected_crs) = field_crs.as_ref() {
            if expected_crs != &tile_crs {
                skipped_tiles.push(TiledInferenceTileSkip {
                    tile_id,
                    reason: TiledInferenceSkipReason::CrsMismatch,
                    detail: format!("tile CRS {tile_crs} does not match field CRS {expected_crs}"),
                });
                continue;
            }
        } else {
            field_crs = Some(tile_crs);
        }

        let footprint = spatial_ref
            .bbox
            .clone()
            .expect("assert_raster_spatial_ref returns a bbox");
        let resolution = spatial_ref
            .resolution
            .expect("assert_raster_spatial_ref returns a resolution");
        let prepared_tile = TiledInferenceTile {
            tile_id: tile_id.clone(),
            width_px: tile.width_px,
            height_px: tile.height_px,
            spatial_ref,
            footprint: footprint.clone(),
            resolution,
        };
        let result = match evaluator(&prepared_tile) {
            Ok(result) => result,
            Err(detail) => {
                skipped_tiles.push(TiledInferenceTileSkip {
                    tile_id,
                    reason: TiledInferenceSkipReason::EvaluatorFailed,
                    detail,
                });
                continue;
            }
        };

        field_extent = Some(match field_extent {
            Some(extent) => merge_bounds(&extent, &footprint),
            None => footprint.clone(),
        });
        outputs.push(TiledInferenceTileOutput {
            tile_id,
            spatial_ref: prepared_tile.spatial_ref,
            footprint,
            result,
        });
    }

    let field_crs = field_crs.ok_or(TiledInferenceError::NoValidTiles)?;
    let field_extent = field_extent.ok_or(TiledInferenceError::NoValidTiles)?;
    outputs.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    skipped_tiles.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));

    Ok(TiledInferenceReport {
        field_crs,
        field_extent,
        tiles_total,
        tiles_processed: outputs.len(),
        tiles_skipped: skipped_tiles.len(),
        outputs,
        skipped_tiles,
    })
}

pub fn run_stand_count(
    field_id: String,
    crs: String,
    tiles: Vec<PlantCountTile>,
    config: PlantCountConfig,
    generated_at: String,
) -> Result<StandCountReport, StandCountError> {
    let field_id = normalize_stand_text(field_id, StandCountError::EmptyFieldId)?;
    let crs = normalize_stand_text(crs, StandCountError::EmptyCrs)?;
    let generated_at = normalize_stand_text(generated_at, StandCountError::EmptyGeneratedAt)?;
    if tiles.is_empty() {
        return Err(StandCountError::EmptyTiles);
    }
    if config.min_component_pixels == 0 {
        return Err(StandCountError::InvalidConfig);
    }

    let mut tile_counts = Vec::new();
    let mut plant_locations = Vec::new();
    let mut zone_rollups: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    let mut field_area_m2 = 0.0;

    for tile in tiles {
        let tile_count = count_tile_plants(tile, &crs, config)?;
        field_area_m2 += tile_count.tile_area_m2;
        if let Some(zone_id) = tile_count.zone_id.as_ref() {
            let entry = zone_rollups.entry(zone_id.clone()).or_insert((0, 0.0));
            entry.0 += tile_count.plant_count;
            entry.1 += tile_count.tile_area_m2;
        }
        plant_locations.extend(tile_count.plant_locations.clone());
        tile_counts.push(tile_count);
    }
    tile_counts.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    plant_locations.sort_by(|left, right| left.plant_id.cmp(&right.plant_id));

    let total_count = tile_counts.iter().map(|tile| tile.plant_count).sum();
    let zones = zone_rollups
        .into_iter()
        .map(|(zone_id, (plant_count, area_m2))| ZonePlantCount {
            zone_id,
            plant_count,
            area_m2,
            density_plants_per_ha: density_per_ha(plant_count, area_m2),
        })
        .collect();

    Ok(StandCountReport {
        field_id,
        crs,
        generated_at,
        total_count,
        field_area_m2,
        field_density_plants_per_ha: density_per_ha(total_count, field_area_m2),
        tiles: tile_counts,
        zones,
        plant_locations,
    })
}

pub fn run_canopy_cover(
    field_id: String,
    tiles: Vec<CanopyCoverTile>,
    config: CanopyCoverConfig,
    generated_at: String,
) -> Result<CanopyCoverReport, CanopyCoverError> {
    let field_id = normalize_canopy_text(field_id, CanopyCoverError::EmptyFieldId)?;
    let generated_at = normalize_canopy_text(generated_at, CanopyCoverError::EmptyGeneratedAt)?;
    if tiles.is_empty() {
        return Err(CanopyCoverError::EmptyTiles);
    }
    if !config.vegetation_index_threshold.is_finite() {
        return Err(CanopyCoverError::InvalidThreshold);
    }

    let mut field_crs: Option<String> = None;
    let mut tile_reports = Vec::new();
    let mut zone_rollups: BTreeMap<String, CanopyZoneAccumulator> = BTreeMap::new();

    for tile in tiles {
        let tile_report = evaluate_canopy_tile(tile, config)?;
        let tile_crs = tile_report.spatial_ref.crs.clone().unwrap_or_default();
        match field_crs.as_ref() {
            Some(expected) if expected != &tile_crs => {
                return Err(CanopyCoverError::CrsMismatch {
                    tile_id: tile_report.tile_id,
                    expected: expected.clone(),
                    actual: tile_crs,
                });
            }
            None => field_crs = Some(tile_crs),
            _ => {}
        }
        if let Some(zone_id) = tile_report.zone_id.as_ref() {
            let entry = zone_rollups.entry(zone_id.clone()).or_default();
            entry.valid_pixels += tile_report.valid_pixels;
            entry.vegetation_pixels += tile_report.vegetation_pixels;
            entry.excluded_pixels += tile_report.excluded_pixels;
            entry.valid_area_m2 += tile_report.valid_area_m2;
            entry.excluded_area_m2 += tile_report.excluded_area_m2;
        }
        tile_reports.push(tile_report);
    }

    tile_reports.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    let valid_pixels = tile_reports.iter().map(|tile| tile.valid_pixels).sum();
    let vegetation_pixels = tile_reports.iter().map(|tile| tile.vegetation_pixels).sum();
    let excluded_pixels = tile_reports.iter().map(|tile| tile.excluded_pixels).sum();
    let valid_area_m2 = tile_reports.iter().map(|tile| tile.valid_area_m2).sum();
    let excluded_area_m2 = tile_reports.iter().map(|tile| tile.excluded_area_m2).sum();
    let zones = zone_rollups
        .into_iter()
        .map(|(zone_id, rollup)| ZoneCanopyCover {
            zone_id,
            valid_pixels: rollup.valid_pixels,
            vegetation_pixels: rollup.vegetation_pixels,
            excluded_pixels: rollup.excluded_pixels,
            valid_area_m2: rollup.valid_area_m2,
            excluded_area_m2: rollup.excluded_area_m2,
            cover_fraction: cover_fraction(rollup.vegetation_pixels, rollup.valid_pixels),
        })
        .collect();

    Ok(CanopyCoverReport {
        field_id,
        crs: field_crs.unwrap_or_default(),
        generated_at,
        valid_pixels,
        vegetation_pixels,
        excluded_pixels,
        valid_area_m2,
        excluded_area_m2,
        cover_fraction: cover_fraction(vegetation_pixels, valid_pixels),
        tiles: tile_reports,
        zones,
    })
}

pub fn estimate_growth_stage(
    crop: String,
    stand: &StandCountReport,
    canopy: &CanopyCoverReport,
    mut index_observations: Vec<GrowthIndexObservation>,
    config: GrowthStageConfig,
    generated_at: String,
) -> Result<GrowthStageEstimate, GrowthStageError> {
    let crop = normalize_growth_text(crop, GrowthStageError::EmptyCrop)?;
    let generated_at = normalize_growth_text(generated_at, GrowthStageError::EmptyGeneratedAt)?;
    if stand.field_id != canopy.field_id {
        return Err(GrowthStageError::FieldMismatch {
            stand_field_id: stand.field_id.clone(),
            canopy_field_id: canopy.field_id.clone(),
        });
    }
    normalize_growth_field_id(&stand.field_id)?;
    validate_growth_stage_config(config)?;
    for observation in &index_observations {
        if !observation.mean_index_value.is_finite()
            || observation.evidence_ref.trim().is_empty()
            || observation.observed_at.trim().is_empty()
        {
            return Err(GrowthStageError::InvalidIndexObservation);
        }
    }
    index_observations.sort_by(|left, right| left.observed_at.cmp(&right.observed_at));
    let index_observation_count = index_observations.len();
    let index_delta = match (index_observations.first(), index_observations.last()) {
        (Some(first), Some(last)) if index_observation_count >= 2 => {
            Some(last.mean_index_value - first.mean_index_value)
        }
        _ => None,
    };
    let evidence_refs = growth_stage_evidence_refs(stand, canopy, &index_observations);

    if index_observation_count < config.min_index_observations_for_confidence {
        return Ok(GrowthStageEstimate {
            field_id: stand.field_id.clone(),
            crop,
            generated_at,
            stage: GrowthStage::InsufficientEvidence,
            confidence: GrowthStageConfidence::Low,
            cover_fraction: canopy.cover_fraction,
            stand_density_plants_per_ha: stand.field_density_plants_per_ha,
            index_observation_count,
            index_delta,
            evidence_refs,
            reason_code: Some("insufficient_index_trajectory".to_string()),
        });
    }

    let stage = if canopy.cover_fraction >= config.reproductive_cover_min
        && index_delta.is_some_and(|delta| delta <= 0.0)
    {
        GrowthStage::Reproductive
    } else if canopy.cover_fraction >= config.vegetative_cover_min {
        GrowthStage::Vegetative
    } else {
        GrowthStage::Emergence
    };
    let confidence = if index_observation_count >= config.min_index_observations_for_confidence + 1
        && canopy.valid_pixels > canopy.excluded_pixels
        && stand.total_count > 0
    {
        GrowthStageConfidence::High
    } else {
        GrowthStageConfidence::Medium
    };

    Ok(GrowthStageEstimate {
        field_id: stand.field_id.clone(),
        crop,
        generated_at,
        stage,
        confidence,
        cover_fraction: canopy.cover_fraction,
        stand_density_plants_per_ha: stand.field_density_plants_per_ha,
        index_observation_count,
        index_delta,
        evidence_refs,
        reason_code: None,
    })
}

pub fn run_disease_lesion_detection(
    field_id: String,
    model: InferenceModelReference,
    model_registered: bool,
    deterministic_cover: Option<&CanopyCoverReport>,
    candidates: Vec<DiseaseLesionCandidate>,
    config: DiseaseDetectionConfig,
    generated_at: String,
) -> Result<DiseaseDetectionReport, DiseaseDetectionError> {
    let field_id = normalize_disease_text(field_id, DiseaseDetectionError::EmptyFieldId)?;
    let generated_at =
        normalize_disease_text(generated_at, DiseaseDetectionError::EmptyGeneratedAt)?;
    if !is_unit_fraction(config.low_confidence_threshold) {
        return Err(DiseaseDetectionError::InvalidThreshold);
    }
    let model = validate_model_reference(model, model_registered)
        .map_err(|source| DiseaseDetectionError::ModelGate { source })?;
    let cover = deterministic_cover.ok_or(DiseaseDetectionError::DeterministicCoverRequired)?;
    if cover.field_id != field_id {
        return Err(DiseaseDetectionError::CoverFieldMismatch {
            expected: field_id,
            actual: cover.field_id.clone(),
        });
    }

    let cover_tiles = cover
        .tiles
        .iter()
        .map(|tile| (tile.tile_id.as_str(), tile))
        .collect::<BTreeMap<_, _>>();
    let mut detections = Vec::new();
    let mut tile_detection_counts: BTreeMap<String, usize> = BTreeMap::new();

    for candidate in candidates {
        let tile_id = normalize_disease_text(
            candidate.tile_id.clone(),
            DiseaseDetectionError::EmptyTileId,
        )?;
        if !is_unit_fraction(candidate.confidence) {
            return Err(DiseaseDetectionError::InvalidConfidence { tile_id });
        }
        let cover_tile = cover_tiles.get(tile_id.as_str()).ok_or_else(|| {
            DiseaseDetectionError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        let tile_bbox = cover_tile.spatial_ref.bbox.as_ref().ok_or_else(|| {
            DiseaseDetectionError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        if !valid_bbox(&candidate.bbox) {
            return Err(DiseaseDetectionError::InvalidZoneGeometry { tile_id });
        }
        if !bbox_within(&candidate.bbox, tile_bbox) {
            return Err(DiseaseDetectionError::ZoneOutsideTileExtent { tile_id });
        }

        let sequence = tile_detection_counts.entry(tile_id.clone()).or_insert(0);
        *sequence += 1;
        detections.push(DiseaseLesionDetection {
            detection_id: format!("disease:{tile_id}:{}", *sequence),
            confidence: candidate.confidence,
            low_confidence: candidate.confidence < config.low_confidence_threshold,
            evidence_tile_ref: tile_id,
            zone_geometry: DetectionZoneGeometry {
                crs: cover.crs.clone(),
                bbox: candidate.bbox,
            },
        });
    }

    detections.sort_by(|left, right| left.detection_id.cmp(&right.detection_id));
    let low_confidence_count = detections
        .iter()
        .filter(|detection| detection.low_confidence)
        .count();

    Ok(DiseaseDetectionReport {
        field_id,
        crs: cover.crs.clone(),
        generated_at,
        model,
        deterministic_cover_valid_pixels: cover.valid_pixels,
        low_confidence_count,
        detections,
    })
}

pub fn run_pest_detection(
    field_id: String,
    model: InferenceModelReference,
    model_registered: bool,
    deterministic_cover: Option<&CanopyCoverReport>,
    candidates: Vec<PestDetectionCandidate>,
    config: PestDetectionConfig,
    generated_at: String,
) -> Result<PestDetectionReport, PestDetectionError> {
    let field_id = normalize_pest_text(field_id, PestDetectionError::EmptyFieldId)?;
    let generated_at = normalize_pest_text(generated_at, PestDetectionError::EmptyGeneratedAt)?;
    if !is_unit_fraction(config.detection_threshold)
        || !is_unit_fraction(config.low_confidence_threshold)
        || config.detection_threshold > config.low_confidence_threshold
    {
        return Err(PestDetectionError::InvalidThreshold);
    }
    let model = validate_model_reference(model, model_registered)
        .map_err(|source| PestDetectionError::ModelGate { source })?;
    let cover = deterministic_cover.ok_or(PestDetectionError::DeterministicCoverRequired)?;
    if cover.field_id != field_id {
        return Err(PestDetectionError::CoverFieldMismatch {
            expected: field_id,
            actual: cover.field_id.clone(),
        });
    }

    let cover_tiles = cover
        .tiles
        .iter()
        .map(|tile| (tile.tile_id.as_str(), tile))
        .collect::<BTreeMap<_, _>>();
    let mut detections = Vec::new();
    let mut tile_detection_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut rejected_candidate_count = 0;

    for candidate in candidates {
        let tile_id =
            normalize_pest_text(candidate.tile_id.clone(), PestDetectionError::EmptyTileId)?;
        let pest_label = normalize_pest_text(
            candidate.pest_label.clone(),
            PestDetectionError::EmptyPestLabel,
        )?;
        if !is_unit_fraction(candidate.confidence) {
            return Err(PestDetectionError::InvalidConfidence { tile_id });
        }
        let cover_tile = cover_tiles.get(tile_id.as_str()).ok_or_else(|| {
            PestDetectionError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        let tile_bbox = cover_tile.spatial_ref.bbox.as_ref().ok_or_else(|| {
            PestDetectionError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        if !valid_bbox(&candidate.bbox) {
            return Err(PestDetectionError::InvalidZoneGeometry { tile_id });
        }
        if !bbox_within(&candidate.bbox, tile_bbox) {
            return Err(PestDetectionError::ZoneOutsideTileExtent { tile_id });
        }
        if candidate.confidence < config.detection_threshold {
            rejected_candidate_count += 1;
            continue;
        }

        let sequence = tile_detection_counts.entry(tile_id.clone()).or_insert(0);
        *sequence += 1;
        detections.push(PestDetection {
            detection_id: format!("pest:{tile_id}:{}", *sequence),
            pest_label,
            confidence: candidate.confidence,
            low_confidence: candidate.confidence < config.low_confidence_threshold,
            evidence_tile_ref: tile_id,
            zone_geometry: DetectionZoneGeometry {
                crs: cover.crs.clone(),
                bbox: candidate.bbox,
            },
        });
    }

    detections.sort_by(|left, right| left.detection_id.cmp(&right.detection_id));
    let low_confidence_count = detections
        .iter()
        .filter(|detection| detection.low_confidence)
        .count();

    Ok(PestDetectionReport {
        field_id,
        crs: cover.crs.clone(),
        generated_at,
        model,
        deterministic_cover_valid_pixels: cover.valid_pixels,
        detection_threshold: config.detection_threshold,
        rejected_candidate_count,
        low_confidence_count,
        detections,
    })
}

pub fn run_weed_mapping(
    field_id: String,
    model: InferenceModelReference,
    model_registered: bool,
    deterministic_cover: Option<&CanopyCoverReport>,
    candidates: Vec<WeedZoneCandidate>,
    config: WeedMappingConfig,
    generated_at: String,
) -> Result<WeedMapReport, WeedMappingError> {
    let field_id = normalize_weed_text(field_id, WeedMappingError::EmptyFieldId)?;
    let generated_at = normalize_weed_text(generated_at, WeedMappingError::EmptyGeneratedAt)?;
    if !is_unit_fraction(config.low_confidence_threshold) {
        return Err(WeedMappingError::InvalidThreshold);
    }
    let model = validate_model_reference(model, model_registered)
        .map_err(|source| WeedMappingError::ModelGate { source })?;
    let cover = deterministic_cover.ok_or(WeedMappingError::DeterministicCoverRequired)?;
    if cover.field_id != field_id {
        return Err(WeedMappingError::CoverFieldMismatch {
            expected: field_id,
            actual: cover.field_id.clone(),
        });
    }

    let cover_tiles = cover
        .tiles
        .iter()
        .map(|tile| (tile.tile_id.as_str(), tile))
        .collect::<BTreeMap<_, _>>();
    let mut zones = Vec::new();
    let mut tile_zone_counts: BTreeMap<String, usize> = BTreeMap::new();

    for candidate in candidates {
        let tile_id =
            normalize_weed_text(candidate.tile_id.clone(), WeedMappingError::EmptyTileId)?;
        if !is_unit_fraction(candidate.confidence) {
            return Err(WeedMappingError::InvalidConfidence { tile_id });
        }
        let cover_tile = cover_tiles.get(tile_id.as_str()).ok_or_else(|| {
            WeedMappingError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        let tile_bbox = cover_tile.spatial_ref.bbox.as_ref().ok_or_else(|| {
            WeedMappingError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        if !valid_bbox(&candidate.bbox) {
            return Err(WeedMappingError::InvalidZoneGeometry { tile_id });
        }
        if !bbox_within(&candidate.bbox, tile_bbox) {
            return Err(WeedMappingError::ZoneOutsideTileExtent { tile_id });
        }

        let sequence = tile_zone_counts.entry(tile_id.clone()).or_insert(0);
        *sequence += 1;
        zones.push(WeedMapZone {
            zone_id: format!("weed:{tile_id}:{}", *sequence),
            confidence: candidate.confidence,
            low_confidence: candidate.confidence < config.low_confidence_threshold,
            evidence_tile_ref: tile_id,
            area_m2: bbox_area_m2(&candidate.bbox),
            geometry: DetectionZoneGeometry {
                crs: cover.crs.clone(),
                bbox: candidate.bbox,
            },
        });
    }

    zones.sort_by(|left, right| left.zone_id.cmp(&right.zone_id));
    let total_weed_area_m2 = zones.iter().map(|zone| zone.area_m2).sum();
    let low_confidence_count = zones.iter().filter(|zone| zone.low_confidence).count();

    Ok(WeedMapReport {
        field_id,
        crs: cover.crs.clone(),
        generated_at,
        model,
        deterministic_cover_valid_pixels: cover.valid_pixels,
        total_weed_area_m2,
        low_confidence_count,
        zones,
    })
}

pub fn apply_detection_verification(
    request: CropDetectionVerificationRequest,
) -> Result<CropDetectionVerificationRecord, CropDetectionVerificationError> {
    validate_detection_task(request.task)?;
    let detection_id = normalize_detection_text(
        request.detection_id,
        CropDetectionVerificationError::EmptyDetectionId,
    )?;
    let label =
        normalize_detection_text(request.label, CropDetectionVerificationError::EmptyLabel)?;
    if !is_unit_fraction(request.confidence) {
        return Err(CropDetectionVerificationError::InvalidConfidence);
    }
    let evidence_tile_refs = normalize_evidence_refs(request.evidence_tile_refs)?;
    validate_detection_geometry(
        &request.zone_geometry,
        CropDetectionVerificationError::EmptyGeometryCrs,
        CropDetectionVerificationError::InvalidGeometry,
    )?;
    let actor =
        normalize_detection_text(request.actor, CropDetectionVerificationError::EmptyActor)?;
    let verified_at = normalize_detection_text(
        request.verified_at,
        CropDetectionVerificationError::EmptyVerifiedAt,
    )?;
    let corrected_label = match request.corrected_label {
        Some(value) => Some(normalize_detection_text(
            value,
            CropDetectionVerificationError::EmptyCorrectedLabel,
        )?),
        None => None,
    };
    let corrected_geometry = match request.corrected_geometry {
        Some(geometry) => {
            validate_detection_geometry(
                &geometry,
                CropDetectionVerificationError::EmptyCorrectedGeometryCrs,
                CropDetectionVerificationError::InvalidCorrectedGeometry,
            )?;
            Some(geometry)
        }
        None => None,
    };

    let verification_state = request.action.verification_state();
    let correction_label = if verification_state == DetectionVerificationState::Corrected {
        if corrected_label.is_none() && corrected_geometry.is_none() {
            return Err(CropDetectionVerificationError::MissingCorrection);
        }
        let feedback_label = corrected_label.clone().unwrap_or_else(|| label.clone());
        let feedback_geometry = corrected_geometry
            .clone()
            .unwrap_or_else(|| request.zone_geometry.clone());
        Some(CropDetectionCorrectionLabel {
            label_id: format!("label:correction:{detection_id}"),
            source_detection_id: detection_id.clone(),
            task: request.task,
            label: feedback_label,
            geometry: feedback_geometry,
            actor: actor.clone(),
            created_at: verified_at.clone(),
            evidence_tile_refs: evidence_tile_refs.clone(),
        })
    } else {
        None
    };

    Ok(CropDetectionVerificationRecord {
        detection_id,
        task: request.task,
        label,
        confidence: request.confidence,
        evidence_tile_refs,
        zone_geometry: request.zone_geometry,
        verification_state,
        actor,
        verified_at,
        corrected_label,
        corrected_geometry,
        correction_label,
    })
}

pub fn validate_detection_finding_promotion(
    request: FindingPromotionRequest,
) -> Result<FindingPromotionDecision, FindingPromotionError> {
    let detection_id = normalize_promotion_text(
        request.detection_id,
        FindingPromotionError::EmptyDetectionId,
    )?;
    match request.verification_state {
        DetectionVerificationState::Confirmed | DetectionVerificationState::Corrected => {
            Ok(FindingPromotionDecision {
                detection_id,
                verification_state: request.verification_state,
                promotion_allowed: true,
                reason: None,
            })
        }
        DetectionVerificationState::Rejected => {
            Err(FindingPromotionError::RejectedDetectionBlocked { detection_id })
        }
        DetectionVerificationState::Unverified if request.allow_unverified => {
            Ok(FindingPromotionDecision {
                detection_id,
                verification_state: DetectionVerificationState::Unverified,
                promotion_allowed: true,
                reason: Some("unverified_override".to_string()),
            })
        }
        DetectionVerificationState::Unverified => {
            Err(FindingPromotionError::UnverifiedDetectionBlocked { detection_id })
        }
    }
}

pub fn assemble_detection_finding(
    request: CropDetectionFindingRequest,
) -> Result<CropDetectionFindingRecord, CropDetectionFindingError> {
    let finding_id = normalize_finding_text(
        request.finding_id,
        CropDetectionFindingError::EmptyFindingId,
    )?;
    let field_id =
        normalize_finding_text(request.field_id, CropDetectionFindingError::EmptyFieldId)?;
    let zone_id = request
        .zone_id
        .and_then(|value| normalize_optional_finding_text(value));
    let emitted_at = normalize_finding_text(
        request.emitted_at,
        CropDetectionFindingError::EmptyEmittedAt,
    )?;
    let model_id = normalize_finding_text(
        request.model.model_id,
        CropDetectionFindingError::EmptyModelId,
    )?;
    let version = normalize_finding_text(
        request.model.version,
        CropDetectionFindingError::EmptyModelVersion,
    )?;
    let model_version = InferenceModelReference { model_id, version };

    match request.detection.verification_state {
        DetectionVerificationState::Confirmed | DetectionVerificationState::Corrected => {}
        DetectionVerificationState::Rejected => {
            return Err(CropDetectionFindingError::RejectedDetection {
                detection_id: request.detection.detection_id,
            })
        }
        DetectionVerificationState::Unverified => {
            return Err(CropDetectionFindingError::UnverifiedDetection {
                detection_id: request.detection.detection_id,
            })
        }
    }

    let evidence_tile_refs = normalize_finding_evidence_refs(request.detection.evidence_tile_refs)?;
    let mut evidence_refs = vec![
        format!("detection:{}", request.detection.detection_id),
        format!("model:{}@{}", model_version.model_id, model_version.version),
    ];
    evidence_refs.extend(
        evidence_tile_refs
            .iter()
            .map(|tile_ref| format!("tile:{tile_ref}")),
    );
    evidence_refs.push(format!(
        "verification:{}",
        request.detection.verification_state.as_str()
    ));

    Ok(CropDetectionFindingRecord {
        finding_id,
        finding_type: request.detection.task,
        field_id,
        zone_id,
        detection_id: request.detection.detection_id,
        label: request
            .detection
            .corrected_label
            .unwrap_or(request.detection.label),
        confidence: request.detection.confidence,
        evidence_tile_refs,
        evidence_refs,
        model_version,
        verification_state: request.detection.verification_state,
        zone_geometry: request
            .detection
            .corrected_geometry
            .unwrap_or(request.detection.zone_geometry),
        emitted_at,
    })
}

fn normalize_required_text(
    value: String,
    error: CropModelRegistryError,
) -> Result<String, CropModelRegistryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_run_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn normalize_optional_run_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_run_text)
}

fn validate_metrics(metrics: &serde_json::Value) -> Result<(), CropModelRegistryError> {
    match metrics.as_object() {
        Some(metrics) if !metrics.is_empty() => Ok(()),
        _ => Err(CropModelRegistryError::InvalidMetrics),
    }
}

fn default_model_metrics() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Default)]
struct CanopyZoneAccumulator {
    valid_pixels: usize,
    vegetation_pixels: usize,
    excluded_pixels: usize,
    valid_area_m2: f64,
    excluded_area_m2: f64,
}

fn evaluate_canopy_tile(
    tile: CanopyCoverTile,
    config: CanopyCoverConfig,
) -> Result<TileCanopyCover, CanopyCoverError> {
    let tile_id = normalize_canopy_text(tile.tile_id.clone(), CanopyCoverError::EmptyTileId)?;
    let zone_id = tile
        .zone_id
        .as_ref()
        .and_then(|zone_id| normalize_optional_stand_text(zone_id.clone()));
    let spatial_ref =
        assert_raster_spatial_ref(Some(&tile.spatial_ref), tile.width_px, tile.height_px).map_err(
            |source| CanopyCoverError::SpatialRefInvalid {
                tile_id: tile_id.clone(),
                source,
            },
        )?;
    let pixel_count = tile.width_px as usize * tile.height_px as usize;
    if tile.index_values.len() != pixel_count {
        return Err(CanopyCoverError::IndexSizeMismatch { tile_id });
    }
    if tile.valid_mask.len() != pixel_count {
        return Err(CanopyCoverError::ValidMaskSizeMismatch { tile_id });
    }

    let pixel_area_m2 = spatial_ref
        .resolution
        .map(|resolution| resolution.x * resolution.y)
        .unwrap_or(0.0);
    let mut valid_mask = Vec::with_capacity(pixel_count);
    let mut vegetation_mask = Vec::with_capacity(pixel_count);
    let mut valid_pixels = 0;
    let mut vegetation_pixels = 0;

    for (index_value, qa_valid) in tile.index_values.iter().zip(tile.valid_mask.iter()) {
        let valid = *qa_valid && index_value.is_finite();
        let vegetation = valid && *index_value >= config.vegetation_index_threshold;
        valid_mask.push(valid);
        vegetation_mask.push(vegetation);
        if valid {
            valid_pixels += 1;
        }
        if vegetation {
            vegetation_pixels += 1;
        }
    }

    let excluded_pixels = pixel_count - valid_pixels;
    let valid_area_m2 = valid_pixels as f64 * pixel_area_m2;
    let excluded_area_m2 = excluded_pixels as f64 * pixel_area_m2;

    Ok(TileCanopyCover {
        tile_id,
        zone_id,
        spatial_ref,
        total_pixels: pixel_count,
        valid_pixels,
        vegetation_pixels,
        excluded_pixels,
        pixel_area_m2,
        valid_area_m2,
        excluded_area_m2,
        cover_fraction: cover_fraction(vegetation_pixels, valid_pixels),
        mask: CanopyCoverMask {
            width_px: tile.width_px,
            height_px: tile.height_px,
            vegetation_mask,
            valid_mask,
        },
    })
}

fn count_tile_plants(
    tile: PlantCountTile,
    crs: &str,
    config: PlantCountConfig,
) -> Result<TilePlantCount, StandCountError> {
    let tile_id = normalize_stand_text(
        tile.tile_id.clone(),
        StandCountError::InvalidTileGeometry {
            tile_id: String::new(),
        },
    )?;
    let zone_id = tile
        .zone_id
        .as_ref()
        .and_then(|zone_id| normalize_optional_stand_text(zone_id.clone()));
    validate_tile_geometry(&tile_id, &tile)?;
    let tile_area_m2 = (tile.max_x_m - tile.min_x_m) * (tile.max_y_m - tile.min_y_m);

    if !tile.valid {
        return Ok(TilePlantCount {
            tile_id,
            zone_id,
            plant_count: 0,
            tile_area_m2,
            density_plants_per_ha: 0.0,
            zero_reason: Some(PlantCountZeroReason::InvalidTile),
            plant_locations: Vec::new(),
        });
    }

    let plant_locations = connected_crop_components(&tile_id, zone_id.clone(), crs, &tile, config);
    let plant_count = plant_locations.len();
    let zero_reason = (plant_count == 0).then_some(PlantCountZeroReason::NoValidCropPixels);

    Ok(TilePlantCount {
        tile_id,
        zone_id,
        plant_count,
        tile_area_m2,
        density_plants_per_ha: density_per_ha(plant_count, tile_area_m2),
        zero_reason,
        plant_locations,
    })
}

fn connected_crop_components(
    tile_id: &str,
    zone_id: Option<String>,
    crs: &str,
    tile: &PlantCountTile,
    config: PlantCountConfig,
) -> Vec<PlantLocation> {
    let width = tile.width_px as usize;
    let height = tile.height_px as usize;
    let mut visited = vec![false; tile.crop_mask.len()];
    let mut plants = Vec::new();

    for index in 0..tile.crop_mask.len() {
        if visited[index] || !tile.crop_mask[index] {
            continue;
        }
        let component = flood_fill_component(index, width, height, &tile.crop_mask, &mut visited);
        if component.len() < config.min_component_pixels {
            continue;
        }
        let (x_m, y_m) = component_centroid(&component, width, tile);
        plants.push(PlantLocation {
            plant_id: format!("plant:{tile_id}:{}", plants.len() + 1),
            tile_id: tile_id.to_string(),
            zone_id: zone_id.clone(),
            crs: crs.to_string(),
            x_m,
            y_m,
            pixel_count: component.len(),
        });
    }

    plants
}

fn flood_fill_component(
    start: usize,
    width: usize,
    height: usize,
    crop_mask: &[bool],
    visited: &mut [bool],
) -> Vec<usize> {
    let mut component = Vec::new();
    let mut stack = vec![start];
    while let Some(index) = stack.pop() {
        if visited[index] || !crop_mask[index] {
            continue;
        }
        visited[index] = true;
        component.push(index);
        let row = index / width;
        let col = index % width;
        if col > 0 {
            stack.push(index - 1);
        }
        if col + 1 < width {
            stack.push(index + 1);
        }
        if row > 0 {
            stack.push(index - width);
        }
        if row + 1 < height {
            stack.push(index + width);
        }
    }
    component
}

fn component_centroid(component: &[usize], width: usize, tile: &PlantCountTile) -> (f64, f64) {
    let pixel_width_m = (tile.max_x_m - tile.min_x_m) / tile.width_px as f64;
    let pixel_height_m = (tile.max_y_m - tile.min_y_m) / tile.height_px as f64;
    let mut x_sum = 0.0;
    let mut y_sum = 0.0;
    for index in component {
        let row = index / width;
        let col = index % width;
        x_sum += tile.min_x_m + (col as f64 + 0.5) * pixel_width_m;
        y_sum += tile.max_y_m - (row as f64 + 0.5) * pixel_height_m;
    }
    (
        x_sum / component.len() as f64,
        y_sum / component.len() as f64,
    )
}

fn validate_tile_geometry(tile_id: &str, tile: &PlantCountTile) -> Result<(), StandCountError> {
    let valid = tile.width_px > 0
        && tile.height_px > 0
        && tile.min_x_m.is_finite()
        && tile.min_y_m.is_finite()
        && tile.max_x_m.is_finite()
        && tile.max_y_m.is_finite()
        && tile.max_x_m > tile.min_x_m
        && tile.max_y_m > tile.min_y_m;
    if !valid {
        return Err(StandCountError::InvalidTileGeometry {
            tile_id: tile_id.to_string(),
        });
    }
    if tile.crop_mask.len() != tile.width_px as usize * tile.height_px as usize {
        return Err(StandCountError::CropMaskSizeMismatch {
            tile_id: tile_id.to_string(),
        });
    }
    Ok(())
}

fn normalize_stand_text(value: String, error: StandCountError) -> Result<String, StandCountError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_stand_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn normalize_canopy_text(
    value: String,
    error: CanopyCoverError,
) -> Result<String, CanopyCoverError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_growth_text(
    value: String,
    error: GrowthStageError,
) -> Result<String, GrowthStageError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_growth_field_id(value: &str) -> Result<(), GrowthStageError> {
    if value.trim().is_empty() {
        Err(GrowthStageError::EmptyFieldId)
    } else {
        Ok(())
    }
}

fn validate_growth_stage_config(config: GrowthStageConfig) -> Result<(), GrowthStageError> {
    let valid = is_unit_fraction(config.emergence_cover_max)
        && is_unit_fraction(config.vegetative_cover_min)
        && is_unit_fraction(config.reproductive_cover_min)
        && config.emergence_cover_max <= config.vegetative_cover_min
        && config.vegetative_cover_min <= config.reproductive_cover_min
        && config.min_index_observations_for_confidence >= 2;
    if valid {
        Ok(())
    } else {
        Err(GrowthStageError::InvalidConfig)
    }
}

fn growth_stage_evidence_refs(
    stand: &StandCountReport,
    canopy: &CanopyCoverReport,
    observations: &[GrowthIndexObservation],
) -> Vec<String> {
    let mut refs = BTreeSet::new();
    refs.insert(format!("stand_count:{}", stand.generated_at));
    refs.insert(format!("canopy_cover:{}", canopy.generated_at));
    for observation in observations {
        refs.insert(observation.evidence_ref.trim().to_string());
    }
    refs.into_iter().collect()
}

fn cover_fraction(vegetation_pixels: usize, valid_pixels: usize) -> f64 {
    if valid_pixels > 0 {
        vegetation_pixels as f64 / valid_pixels as f64
    } else {
        0.0
    }
}

fn normalize_disease_text(
    value: String,
    error: DiseaseDetectionError,
) -> Result<String, DiseaseDetectionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_pest_text(
    value: String,
    error: PestDetectionError,
) -> Result<String, PestDetectionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn is_unit_fraction(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn valid_bbox(bbox: &GeoBounds) -> bool {
    bbox.min_lon.is_finite()
        && bbox.min_lat.is_finite()
        && bbox.max_lon.is_finite()
        && bbox.max_lat.is_finite()
        && bbox.max_lon > bbox.min_lon
        && bbox.max_lat > bbox.min_lat
}

fn bbox_within(inner: &GeoBounds, outer: &GeoBounds) -> bool {
    const TOLERANCE: f64 = 1.0e-9;
    inner.min_lon + TOLERANCE >= outer.min_lon
        && inner.min_lat + TOLERANCE >= outer.min_lat
        && inner.max_lon <= outer.max_lon + TOLERANCE
        && inner.max_lat <= outer.max_lat + TOLERANCE
}

fn merge_bounds(left: &GeoBounds, right: &GeoBounds) -> GeoBounds {
    GeoBounds {
        min_lon: left.min_lon.min(right.min_lon),
        min_lat: left.min_lat.min(right.min_lat),
        max_lon: left.max_lon.max(right.max_lon),
        max_lat: left.max_lat.max(right.max_lat),
    }
}

fn normalize_weed_text(value: String, error: WeedMappingError) -> Result<String, WeedMappingError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_detection_task(task: CropModelTask) -> Result<(), CropDetectionVerificationError> {
    match task {
        CropModelTask::DiseaseDetection
        | CropModelTask::PestDetection
        | CropModelTask::WeedMapping => Ok(()),
        _ => Err(CropDetectionVerificationError::UnsupportedDetectionTask { task }),
    }
}

fn normalize_detection_text(
    value: String,
    error: CropDetectionVerificationError,
) -> Result<String, CropDetectionVerificationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_evidence_refs(
    values: Vec<String>,
) -> Result<Vec<String>, CropDetectionVerificationError> {
    let mut refs = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(CropDetectionVerificationError::EmptyEvidenceRef);
        }
        refs.push(trimmed.to_string());
    }
    refs.sort();
    refs.dedup();
    if refs.is_empty() {
        Err(CropDetectionVerificationError::EmptyEvidence)
    } else {
        Ok(refs)
    }
}

fn validate_detection_geometry(
    geometry: &DetectionZoneGeometry,
    empty_crs: CropDetectionVerificationError,
    invalid_geometry: CropDetectionVerificationError,
) -> Result<(), CropDetectionVerificationError> {
    if geometry.crs.trim().is_empty() {
        return Err(empty_crs);
    }
    if !valid_bbox(&geometry.bbox) {
        return Err(invalid_geometry);
    }
    Ok(())
}

fn normalize_promotion_text(
    value: String,
    error: FindingPromotionError,
) -> Result<String, FindingPromotionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_finding_text(
    value: String,
    error: CropDetectionFindingError,
) -> Result<String, CropDetectionFindingError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_finding_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn normalize_finding_evidence_refs(
    values: Vec<String>,
) -> Result<Vec<String>, CropDetectionFindingError> {
    let mut refs = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(CropDetectionFindingError::EmptyEvidenceRef);
        }
        refs.push(trimmed.to_string());
    }
    refs.sort();
    refs.dedup();
    if refs.is_empty() {
        Err(CropDetectionFindingError::EmptyEvidence)
    } else {
        Ok(refs)
    }
}

fn bbox_area_m2(bbox: &GeoBounds) -> f64 {
    (bbox.max_lon - bbox.min_lon) * (bbox.max_lat - bbox.min_lat)
}

fn density_per_ha(count: usize, area_m2: f64) -> f64 {
    if area_m2 > 0.0 {
        count as f64 / (area_m2 / 10_000.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_detection_verification, assemble_detection_finding, build_inference_run_record,
        build_model_version_record, estimate_growth_stage, run_canopy_cover,
        run_disease_lesion_detection, run_pest_detection, run_stand_count,
        run_tiled_inference_pipeline, run_weed_mapping, transition_inference_run_status,
        validate_detection_finding_promotion, validate_model_reference, CanopyCoverConfig,
        CanopyCoverError, CanopyCoverTile, CropDetectionFindingError, CropDetectionFindingRequest,
        CropDetectionVerificationAction, CropDetectionVerificationRequest, CropModelRegistryError,
        CropModelTask, DetectionVerificationState, DetectionZoneGeometry, DiseaseDetectionConfig,
        DiseaseDetectionError, DiseaseLesionCandidate, FindingPromotionError,
        FindingPromotionRequest, GrowthIndexObservation, GrowthStage, GrowthStageConfidence,
        GrowthStageConfig, InferenceModelReference, InferenceRunError, InferenceRunStatus,
        InferenceRunSubmissionRequest, ModelVersionRegistrationRequest, PestDetectionCandidate,
        PestDetectionConfig, PestDetectionError, PlantCountConfig, PlantCountTile,
        PlantCountZeroReason, TiledInferenceInput, TiledInferenceSkipReason, WeedMappingConfig,
        WeedMappingError, WeedZoneCandidate,
    };
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn model_version_record_requires_versioned_provenance() {
        let record = build_model_version_record(
            ModelVersionRegistrationRequest {
                model_id: " lesion-detector ".to_string(),
                version: " 2026.06.1 ".to_string(),
                task: CropModelTask::DiseaseDetection,
                training_set_ref: " dataset:lesion-v3 ".to_string(),
                metrics: serde_json::json!({
                    "precision": 0.91,
                    "recall": 0.87,
                    "iou": 0.73
                }),
                provenance_ref: " provenance:model/lesion-detector/2026.06.1 ".to_string(),
            },
            " 2026-06-12T12:00:00Z ".to_string(),
        )
        .expect("model version should be valid");

        assert_eq!(record.model_id, "lesion-detector");
        assert_eq!(record.version, "2026.06.1");
        assert_eq!(record.task, CropModelTask::DiseaseDetection);
        assert_eq!(record.training_set_ref, "dataset:lesion-v3");
        assert_eq!(
            record
                .metrics
                .get("precision")
                .and_then(|value| value.as_f64()),
            Some(0.91)
        );
        assert_eq!(
            record.provenance_ref,
            "provenance:model/lesion-detector/2026.06.1"
        );
    }

    #[test]
    fn model_version_rejects_missing_metrics() {
        let error = build_model_version_record(
            ModelVersionRegistrationRequest {
                model_id: "lesion-detector".to_string(),
                version: "2026.06.1".to_string(),
                task: CropModelTask::DiseaseDetection,
                training_set_ref: "dataset:lesion-v3".to_string(),
                metrics: serde_json::json!({}),
                provenance_ref: "provenance:model/lesion-detector/2026.06.1".to_string(),
            },
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect_err("empty metrics should be rejected");

        assert_eq!(error, CropModelRegistryError::InvalidMetrics);
    }

    #[test]
    fn unregistered_model_reference_is_rejected() {
        let error = validate_model_reference(
            InferenceModelReference {
                model_id: "unknown-model".to_string(),
                version: "v0".to_string(),
            },
            false,
        )
        .expect_err("unknown model should be rejected");

        assert_eq!(
            error,
            CropModelRegistryError::UnregisteredModel {
                model_id: "unknown-model".to_string(),
                version: "v0".to_string()
            }
        );
    }

    #[test]
    fn deterministic_inference_run_is_queued_and_transitions_in_order() {
        let run = build_inference_run_record(
            InferenceRunSubmissionRequest {
                run_id: Some(" run-001 ".to_string()),
                mosaic_ref: " mosaic:scene-1:orthomosaic ".to_string(),
                field_id: " field-1 ".to_string(),
                season_id: " season-2026 ".to_string(),
                model: None,
            },
            "generated-run".to_string(),
            " 2026-06-13T15:00:00Z ".to_string(),
            None,
        )
        .expect("deterministic run should be queued");

        assert_eq!(run.run_id, "run-001");
        assert_eq!(run.mosaic_ref, "mosaic:scene-1:orthomosaic");
        assert_eq!(run.field_id, "field-1");
        assert_eq!(run.season_id, "season-2026");
        assert_eq!(run.model_id, None);
        assert_eq!(run.model_version, "deterministic");
        assert_eq!(run.status, InferenceRunStatus::Queued);

        let running = transition_inference_run_status(
            run,
            InferenceRunStatus::Running,
            None,
            "2026-06-13T15:01:00Z".to_string(),
        )
        .expect("queued run should start");
        let completed = transition_inference_run_status(
            running,
            InferenceRunStatus::Completed,
            None,
            "2026-06-13T15:05:00Z".to_string(),
        )
        .expect("running run should complete");

        assert_eq!(completed.status, InferenceRunStatus::Completed);
        assert_eq!(completed.failure_reason_code, None);
        assert_eq!(completed.updated_at, "2026-06-13T15:05:00Z");
    }

    #[test]
    fn inference_run_failure_records_reason_code() {
        let run = build_inference_run_record(
            InferenceRunSubmissionRequest {
                run_id: Some("run-002".to_string()),
                mosaic_ref: "mosaic:scene-1:orthomosaic".to_string(),
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                model: None,
            },
            "generated-run".to_string(),
            "2026-06-13T15:00:00Z".to_string(),
            None,
        )
        .expect("run should be queued");
        let running = transition_inference_run_status(
            run,
            InferenceRunStatus::Running,
            None,
            "2026-06-13T15:01:00Z".to_string(),
        )
        .expect("run should start");
        let failed = transition_inference_run_status(
            running,
            InferenceRunStatus::Failed,
            Some("tile_decode_failed".to_string()),
            "2026-06-13T15:02:00Z".to_string(),
        )
        .expect("running run can fail");

        assert_eq!(failed.status, InferenceRunStatus::Failed);
        assert_eq!(
            failed.failure_reason_code.as_deref(),
            Some("tile_decode_failed")
        );
    }

    #[test]
    fn inference_run_rejects_invalid_transition_and_unregistered_model() {
        let run = build_inference_run_record(
            InferenceRunSubmissionRequest {
                run_id: Some("run-003".to_string()),
                mosaic_ref: "mosaic:scene-1:orthomosaic".to_string(),
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                model: None,
            },
            "generated-run".to_string(),
            "2026-06-13T15:00:00Z".to_string(),
            None,
        )
        .expect("run should be queued");
        let transition_error = transition_inference_run_status(
            run,
            InferenceRunStatus::Completed,
            None,
            "2026-06-13T15:01:00Z".to_string(),
        )
        .expect_err("queued run cannot skip running");
        assert_eq!(
            transition_error,
            InferenceRunError::InvalidTransition {
                from: InferenceRunStatus::Queued,
                to: InferenceRunStatus::Completed
            }
        );

        let model_error = build_inference_run_record(
            InferenceRunSubmissionRequest {
                run_id: Some("run-004".to_string()),
                mosaic_ref: "mosaic:scene-1:orthomosaic".to_string(),
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                model: Some(InferenceModelReference {
                    model_id: "unknown".to_string(),
                    version: "v0".to_string(),
                }),
            },
            "generated-run".to_string(),
            "2026-06-13T15:00:00Z".to_string(),
            Some(false),
        )
        .expect_err("unknown model cannot create an inference run");

        assert_eq!(
            model_error,
            InferenceRunError::ModelGate {
                source: CropModelRegistryError::UnregisteredModel {
                    model_id: "unknown".to_string(),
                    version: "v0".to_string()
                }
            }
        );
    }

    #[test]
    fn tiled_inference_pipeline_preserves_crs_extent_and_reassembles_outputs() {
        let report = run_tiled_inference_pipeline(
            vec![
                inference_tile(
                    "tile-west",
                    2,
                    2,
                    GeoBounds {
                        min_lon: 0.0,
                        min_lat: 0.0,
                        max_lon: 20.0,
                        max_lat: 20.0,
                    },
                ),
                inference_tile(
                    "tile-east",
                    2,
                    2,
                    GeoBounds {
                        min_lon: 20.0,
                        min_lat: 0.0,
                        max_lon: 40.0,
                        max_lat: 20.0,
                    },
                ),
            ],
            |tile| Ok(tile.footprint.max_lon - tile.footprint.min_lon),
        )
        .expect("tiled inference should assemble valid tiles");

        assert_eq!(report.field_crs, "EPSG:32614");
        assert_eq!(
            report.field_extent,
            GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 40.0,
                max_lat: 20.0,
            }
        );
        assert_eq!(report.tiles_total, 2);
        assert_eq!(report.tiles_processed, 2);
        assert_eq!(report.tiles_skipped, 0);
        assert_eq!(
            report
                .outputs
                .iter()
                .map(|tile| &tile.tile_id)
                .collect::<Vec<_>>(),
            vec![&"tile-east".to_string(), &"tile-west".to_string()]
        );
        assert!(report
            .outputs
            .iter()
            .all(|tile| tile.spatial_ref.crs.as_deref() == Some("EPSG:32614")));
        assert_eq!(report.outputs[0].result, 20.0);
    }

    #[test]
    fn tiled_inference_pipeline_skips_bad_georeference_without_default_origin() {
        let mut bad_tile = inference_tile(
            "tile-bad",
            2,
            2,
            GeoBounds {
                min_lon: 100.0,
                min_lat: 100.0,
                max_lon: 120.0,
                max_lat: 120.0,
            },
        );
        bad_tile.spatial_ref.georeferenced = false;

        let report = run_tiled_inference_pipeline(
            vec![
                bad_tile,
                inference_tile(
                    "tile-good",
                    2,
                    2,
                    GeoBounds {
                        min_lon: 20.0,
                        min_lat: 0.0,
                        max_lon: 40.0,
                        max_lat: 20.0,
                    },
                ),
            ],
            |tile| Ok(tile.tile_id.clone()),
        )
        .expect("valid tile should still run");

        assert_eq!(report.tiles_total, 2);
        assert_eq!(report.tiles_processed, 1);
        assert_eq!(report.tiles_skipped, 1);
        assert_eq!(report.outputs[0].tile_id, "tile-good");
        assert_eq!(
            report.field_extent,
            GeoBounds {
                min_lon: 20.0,
                min_lat: 0.0,
                max_lon: 40.0,
                max_lat: 20.0,
            }
        );
        assert_eq!(report.skipped_tiles[0].tile_id, "tile-bad");
        assert_eq!(
            report.skipped_tiles[0].reason,
            TiledInferenceSkipReason::InvalidGeoreference
        );
        assert!(!report
            .outputs
            .iter()
            .any(|tile| tile.footprint.min_lon == 0.0 && tile.footprint.min_lat == 0.0));
    }

    #[test]
    fn stand_count_detects_plants_per_field_zone_and_locations() {
        let report = run_stand_count(
            "field-1".to_string(),
            "EPSG:32614".to_string(),
            vec![plant_tile(
                "tile-1",
                Some("zone-a"),
                true,
                vec![
                    true, true, false, false, false, false, false, false, false, false, false,
                    false, false, false, false, true,
                ],
            )],
            PlantCountConfig {
                min_component_pixels: 1,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("stand count should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.total_count, 2);
        assert_eq!(report.tiles[0].plant_count, 2);
        assert_eq!(report.zones[0].zone_id, "zone-a");
        assert_eq!(report.zones[0].plant_count, 2);
        assert_eq!(report.plant_locations.len(), 2);
        assert!(report
            .plant_locations
            .iter()
            .all(|plant| plant.crs == "EPSG:32614"));
        assert!(report.field_density_plants_per_ha > 0.0);
    }

    #[test]
    fn stand_count_invalid_tile_contributes_zero_with_reason() {
        let report = run_stand_count(
            "field-1".to_string(),
            "EPSG:32614".to_string(),
            vec![plant_tile(
                "tile-cloud",
                Some("zone-a"),
                false,
                vec![
                    true, true, true, true, true, true, true, true, true, true, true, true, true,
                    true, true, true,
                ],
            )],
            PlantCountConfig {
                min_component_pixels: 1,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("stand count should run");

        assert_eq!(report.total_count, 0);
        assert!(report.plant_locations.is_empty());
        assert_eq!(
            report.tiles[0].zero_reason,
            Some(PlantCountZeroReason::InvalidTile)
        );
    }

    #[test]
    fn stand_count_bare_tile_contributes_zero_with_reason() {
        let report = run_stand_count(
            "field-1".to_string(),
            "EPSG:32614".to_string(),
            vec![plant_tile(
                "tile-bare",
                Some("zone-a"),
                true,
                vec![false; 16],
            )],
            PlantCountConfig {
                min_component_pixels: 1,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("stand count should run");

        assert_eq!(report.total_count, 0);
        assert!(report.plant_locations.is_empty());
        assert_eq!(
            report.tiles[0].zero_reason,
            Some(PlantCountZeroReason::NoValidCropPixels)
        );
    }

    #[test]
    fn canopy_cover_returns_georeferenced_masks_and_zone_fractions() {
        let report = run_canopy_cover(
            "field-1".to_string(),
            vec![canopy_tile(
                "tile-1",
                Some("zone-a"),
                3,
                2,
                vec![0.7, 0.2, 0.5, 0.1, 0.8, 0.4],
                vec![true; 6],
            )],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("canopy cover should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.valid_pixels, 6);
        assert_eq!(report.vegetation_pixels, 3);
        assert_eq!(report.excluded_pixels, 0);
        assert_eq!(report.cover_fraction, 0.5);
        assert_eq!(report.zones[0].zone_id, "zone-a");
        assert_eq!(report.zones[0].cover_fraction, 0.5);
        assert_eq!(
            report.tiles[0].mask.vegetation_mask,
            vec![true, false, true, false, true, false]
        );
        assert_eq!(report.tiles[0].mask.valid_mask, vec![true; 6]);
        assert_eq!(
            report.tiles[0].spatial_ref.crs.as_deref(),
            Some("EPSG:32614")
        );
        assert_eq!(report.tiles[0].spatial_ref, spatial_ref(3, 2));
    }

    #[test]
    fn canopy_cover_excludes_cloud_nodata_pixels_from_fraction() {
        let report = run_canopy_cover(
            "field-1".to_string(),
            vec![canopy_tile(
                "tile-cloud",
                Some("zone-a"),
                2,
                2,
                vec![0.8, 0.0, 0.1, 0.9],
                vec![true, false, true, false],
            )],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("canopy cover should run");

        assert_eq!(report.valid_pixels, 2);
        assert_eq!(report.vegetation_pixels, 1);
        assert_eq!(report.excluded_pixels, 2);
        assert_eq!(report.cover_fraction, 0.5);
        assert_eq!(
            report.tiles[0].mask.vegetation_mask,
            vec![true, false, false, false]
        );
        assert_eq!(
            report.tiles[0].mask.valid_mask,
            vec![true, false, true, false]
        );
    }

    #[test]
    fn canopy_cover_rejects_bad_spatial_ref() {
        let mut tile = canopy_tile(
            "tile-bad-ref",
            Some("zone-a"),
            2,
            2,
            vec![0.8, 0.0, 0.1, 0.9],
            vec![true; 4],
        );
        tile.spatial_ref.georeferenced = false;

        let error = run_canopy_cover(
            "field-1".to_string(),
            vec![tile],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect_err("bad spatial ref should be rejected");

        assert!(matches!(
            error,
            CanopyCoverError::SpatialRefInvalid { tile_id, .. } if tile_id == "tile-bad-ref"
        ));
    }

    #[test]
    fn growth_stage_estimation_uses_index_cover_and_stand_evidence() {
        let stand = stand_count_fixture();
        let canopy = canopy_cover_fixture(vec![0.72, 0.68, 0.74, 0.7], vec![true; 4]);

        let estimate = estimate_growth_stage(
            "corn".to_string(),
            &stand,
            &canopy,
            vec![
                index_observation("2026-06-01T00:00:00Z", 0.52, "ndvi:2026-06-01"),
                index_observation("2026-06-08T00:00:00Z", 0.66, "ndvi:2026-06-08"),
                index_observation("2026-06-15T00:00:00Z", 0.71, "ndvi:2026-06-15"),
            ],
            growth_config(),
            "2026-06-15T12:00:00Z".to_string(),
        )
        .expect("growth stage should estimate");

        assert_eq!(estimate.field_id, "field-1");
        assert_eq!(estimate.stage, GrowthStage::Vegetative);
        assert_eq!(estimate.confidence, GrowthStageConfidence::High);
        assert_eq!(estimate.index_observation_count, 3);
        assert!((estimate.index_delta.unwrap() - 0.19).abs() < 1e-9);
        assert!(estimate.reason_code.is_none());
        assert!(estimate
            .evidence_refs
            .iter()
            .any(|evidence| evidence == "ndvi:2026-06-15"));
        assert!(estimate
            .evidence_refs
            .iter()
            .any(|evidence| evidence.starts_with("canopy_cover:")));
    }

    #[test]
    fn growth_stage_single_date_returns_low_confidence_insufficient_evidence() {
        let stand = stand_count_fixture();
        let canopy = canopy_cover_fixture(vec![0.72, 0.68, 0.74, 0.7], vec![true; 4]);

        let estimate = estimate_growth_stage(
            "corn".to_string(),
            &stand,
            &canopy,
            vec![index_observation(
                "2026-06-15T00:00:00Z",
                0.71,
                "ndvi:2026-06-15",
            )],
            growth_config(),
            "2026-06-15T12:00:00Z".to_string(),
        )
        .expect("single-date estimate should return a low-confidence result");

        assert_eq!(estimate.stage, GrowthStage::InsufficientEvidence);
        assert_eq!(estimate.confidence, GrowthStageConfidence::Low);
        assert_eq!(estimate.index_delta, None);
        assert_eq!(
            estimate.reason_code.as_deref(),
            Some("insufficient_index_trajectory")
        );
    }

    #[test]
    fn disease_detection_returns_confidence_evidence_and_bounded_zone() {
        let cover = cover_report();
        let report = run_disease_lesion_detection(
            "field-1".to_string(),
            registered_model(),
            true,
            Some(&cover),
            vec![lesion_candidate(
                "tile-1",
                0.82,
                GeoBounds {
                    min_lon: 5.0,
                    min_lat: 5.0,
                    max_lon: 15.0,
                    max_lat: 15.0,
                },
            )],
            DiseaseDetectionConfig {
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:15:00Z".to_string(),
        )
        .expect("disease detection should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.model.model_id, "lesion-detector");
        assert_eq!(report.deterministic_cover_valid_pixels, cover.valid_pixels);
        assert_eq!(report.detections.len(), 1);
        let detection = &report.detections[0];
        assert_eq!(detection.evidence_tile_ref, "tile-1");
        assert_eq!(detection.confidence, 0.82);
        assert!(!detection.low_confidence);
        assert_eq!(detection.zone_geometry.crs, "EPSG:32614");
        assert_eq!(
            detection.zone_geometry.bbox,
            GeoBounds {
                min_lon: 5.0,
                min_lat: 5.0,
                max_lon: 15.0,
                max_lat: 15.0,
            }
        );
    }

    #[test]
    fn disease_detection_marks_low_confidence_without_hiding_detection() {
        let cover = cover_report();
        let report = run_disease_lesion_detection(
            "field-1".to_string(),
            registered_model(),
            true,
            Some(&cover),
            vec![lesion_candidate(
                "tile-1",
                0.42,
                GeoBounds {
                    min_lon: 5.0,
                    min_lat: 5.0,
                    max_lon: 15.0,
                    max_lat: 15.0,
                },
            )],
            DiseaseDetectionConfig {
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:15:00Z".to_string(),
        )
        .expect("low-confidence detection should be retained");

        assert_eq!(report.detections.len(), 1);
        assert_eq!(report.low_confidence_count, 1);
        assert!(report.detections[0].low_confidence);
    }

    #[test]
    fn disease_detection_refuses_to_run_without_deterministic_cover() {
        let error = run_disease_lesion_detection(
            "field-1".to_string(),
            registered_model(),
            true,
            None,
            vec![lesion_candidate(
                "tile-1",
                0.82,
                GeoBounds {
                    min_lon: 5.0,
                    min_lat: 5.0,
                    max_lon: 15.0,
                    max_lat: 15.0,
                },
            )],
            DiseaseDetectionConfig {
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:15:00Z".to_string(),
        )
        .expect_err("deterministic cover is required");

        assert_eq!(error, DiseaseDetectionError::DeterministicCoverRequired);
    }

    #[test]
    fn pest_detection_returns_confidence_evidence_and_bounded_zone() {
        let cover = cover_report();
        let report = run_pest_detection(
            "field-1".to_string(),
            pest_model(),
            true,
            Some(&cover),
            vec![pest_candidate(
                "tile-1",
                "corn_earworm",
                0.88,
                GeoBounds {
                    min_lon: 4.0,
                    min_lat: 6.0,
                    max_lon: 16.0,
                    max_lat: 18.0,
                },
            )],
            PestDetectionConfig {
                detection_threshold: 0.5,
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:18:00Z".to_string(),
        )
        .expect("pest detection should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.model.model_id, "pest-detector");
        assert_eq!(report.deterministic_cover_valid_pixels, cover.valid_pixels);
        assert_eq!(report.detection_threshold, 0.5);
        assert_eq!(report.rejected_candidate_count, 0);
        assert_eq!(report.detections.len(), 1);
        let detection = &report.detections[0];
        assert_eq!(detection.detection_id, "pest:tile-1:1");
        assert_eq!(detection.pest_label, "corn_earworm");
        assert_eq!(detection.evidence_tile_ref, "tile-1");
        assert_eq!(detection.confidence, 0.88);
        assert!(!detection.low_confidence);
        assert_eq!(detection.zone_geometry.crs, "EPSG:32614");
        assert_eq!(
            detection.zone_geometry.bbox,
            GeoBounds {
                min_lon: 4.0,
                min_lat: 6.0,
                max_lon: 16.0,
                max_lat: 18.0,
            }
        );
    }

    #[test]
    fn pest_detection_clear_field_excludes_below_threshold_without_false_zones() {
        let cover = cover_report();
        let report = run_pest_detection(
            "field-1".to_string(),
            pest_model(),
            true,
            Some(&cover),
            vec![pest_candidate(
                "tile-1",
                "aphid",
                0.24,
                GeoBounds {
                    min_lon: 0.0,
                    min_lat: 0.0,
                    max_lon: 8.0,
                    max_lat: 8.0,
                },
            )],
            PestDetectionConfig {
                detection_threshold: 0.5,
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:18:00Z".to_string(),
        )
        .expect("clear-field pest report should run");

        assert!(report.detections.is_empty());
        assert_eq!(report.rejected_candidate_count, 1);
        assert_eq!(report.low_confidence_count, 0);
    }

    #[test]
    fn pest_detection_refuses_to_run_without_deterministic_cover() {
        let error = run_pest_detection(
            "field-1".to_string(),
            pest_model(),
            true,
            None,
            Vec::new(),
            PestDetectionConfig {
                detection_threshold: 0.5,
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:18:00Z".to_string(),
        )
        .expect_err("deterministic cover is required");

        assert_eq!(error, PestDetectionError::DeterministicCoverRequired);
    }

    #[test]
    fn weed_mapping_returns_georeferenced_confidence_zones_and_area() {
        let cover = cover_report();
        let report = run_weed_mapping(
            "field-1".to_string(),
            weed_model(),
            true,
            Some(&cover),
            vec![weed_candidate(
                "tile-1",
                0.76,
                GeoBounds {
                    min_lon: 0.0,
                    min_lat: 0.0,
                    max_lon: 10.0,
                    max_lat: 10.0,
                },
            )],
            WeedMappingConfig {
                low_confidence_threshold: 0.65,
            },
            "2026-06-01T12:20:00Z".to_string(),
        )
        .expect("weed mapping should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.model.model_id, "weed-detector");
        assert_eq!(report.zones.len(), 1);
        assert_eq!(report.total_weed_area_m2, 100.0);
        let zone = &report.zones[0];
        assert_eq!(zone.evidence_tile_ref, "tile-1");
        assert_eq!(zone.confidence, 0.76);
        assert_eq!(zone.area_m2, 100.0);
        assert!(!zone.low_confidence);
        assert_eq!(zone.geometry.crs, "EPSG:32614");
        assert_eq!(
            zone.geometry.bbox,
            GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 10.0,
                max_lat: 10.0,
            }
        );
    }

    #[test]
    fn weed_mapping_returns_no_zones_for_weed_free_field() {
        let cover = cover_report();
        let report = run_weed_mapping(
            "field-1".to_string(),
            weed_model(),
            true,
            Some(&cover),
            Vec::new(),
            WeedMappingConfig {
                low_confidence_threshold: 0.65,
            },
            "2026-06-01T12:20:00Z".to_string(),
        )
        .expect("weed-free mapping should run");

        assert!(report.zones.is_empty());
        assert_eq!(report.total_weed_area_m2, 0.0);
        assert_eq!(report.low_confidence_count, 0);
    }

    #[test]
    fn weed_mapping_refuses_to_run_without_deterministic_cover() {
        let error = run_weed_mapping(
            "field-1".to_string(),
            weed_model(),
            true,
            None,
            Vec::new(),
            WeedMappingConfig {
                low_confidence_threshold: 0.65,
            },
            "2026-06-01T12:20:00Z".to_string(),
        )
        .expect_err("deterministic cover is required");

        assert_eq!(error, WeedMappingError::DeterministicCoverRequired);
    }

    #[test]
    fn human_verification_records_actor_timestamp_and_correction_label() {
        let record = apply_detection_verification(CropDetectionVerificationRequest {
            detection_id: "disease:tile-1:1".to_string(),
            task: CropModelTask::DiseaseDetection,
            label: "northern_leaf_blight".to_string(),
            confidence: 0.82,
            evidence_tile_refs: vec!["tile-1".to_string()],
            zone_geometry: detection_geometry(5.0, 5.0, 15.0, 15.0),
            action: CropDetectionVerificationAction::Corrected,
            actor: "agronomist-7".to_string(),
            verified_at: "2026-06-12T14:00:00Z".to_string(),
            corrected_label: Some("nitrogen_stress".to_string()),
            corrected_geometry: Some(detection_geometry(6.0, 6.0, 16.0, 16.0)),
        })
        .expect("correction should produce an audited verification record");

        assert_eq!(record.detection_id, "disease:tile-1:1");
        assert_eq!(
            record.verification_state,
            DetectionVerificationState::Corrected
        );
        assert_eq!(record.actor, "agronomist-7");
        assert_eq!(record.verified_at, "2026-06-12T14:00:00Z");
        assert_eq!(record.evidence_tile_refs, vec!["tile-1".to_string()]);
        assert_eq!(record.corrected_label.as_deref(), Some("nitrogen_stress"));
        assert_eq!(
            record
                .corrected_geometry
                .as_ref()
                .expect("geometry should be corrected")
                .bbox,
            GeoBounds {
                min_lon: 6.0,
                min_lat: 6.0,
                max_lon: 16.0,
                max_lat: 16.0,
            }
        );
        let feedback = record
            .correction_label
            .expect("correction should feed back as a label");
        assert_eq!(feedback.label_id, "label:correction:disease:tile-1:1");
        assert_eq!(feedback.label, "nitrogen_stress");
        assert_eq!(feedback.actor, "agronomist-7");
        assert_eq!(feedback.source_detection_id, "disease:tile-1:1");
    }

    #[test]
    fn finding_promotion_blocks_unverified_detection_by_default() {
        let error = validate_detection_finding_promotion(FindingPromotionRequest {
            detection_id: "weed:tile-1:1".to_string(),
            verification_state: DetectionVerificationState::Unverified,
            allow_unverified: false,
        })
        .expect_err("unverified detections should not become findings by default");

        assert_eq!(
            error,
            FindingPromotionError::UnverifiedDetectionBlocked {
                detection_id: "weed:tile-1:1".to_string()
            }
        );

        let decision = validate_detection_finding_promotion(FindingPromotionRequest {
            detection_id: "weed:tile-1:1".to_string(),
            verification_state: DetectionVerificationState::Unverified,
            allow_unverified: true,
        })
        .expect("explicit override should be audited and allowed");

        assert!(decision.promotion_allowed);
        assert_eq!(
            decision.verification_state,
            DetectionVerificationState::Unverified
        );
        assert_eq!(decision.reason.as_deref(), Some("unverified_override"));
    }

    #[test]
    fn verified_detection_assembles_evidence_cited_finding() {
        let finding = assemble_detection_finding(CropDetectionFindingRequest {
            finding_id: "finding-1".to_string(),
            field_id: "field-1".to_string(),
            zone_id: Some("zone-a".to_string()),
            detection: confirmed_detection(),
            model: registered_model(),
            emitted_at: "2026-06-12T15:00:00Z".to_string(),
        })
        .expect("verified detection should assemble into an advisor finding");

        assert_eq!(finding.finding_id, "finding-1");
        assert_eq!(finding.finding_type, CropModelTask::DiseaseDetection);
        assert_eq!(finding.field_id, "field-1");
        assert_eq!(finding.zone_id.as_deref(), Some("zone-a"));
        assert_eq!(finding.confidence, 0.82);
        assert_eq!(finding.evidence_tile_refs, vec!["tile-1".to_string()]);
        assert_eq!(finding.model_version.model_id, "lesion-detector");
        assert_eq!(
            finding.verification_state,
            DetectionVerificationState::Confirmed
        );
        assert_eq!(
            finding.evidence_refs,
            vec![
                "detection:disease:tile-1:1".to_string(),
                "model:lesion-detector@2026.06.1".to_string(),
                "tile:tile-1".to_string(),
                "verification:confirmed".to_string(),
            ]
        );
    }

    #[test]
    fn finding_assembly_rejects_uncited_detection() {
        let mut detection = confirmed_detection();
        detection.evidence_tile_refs.clear();

        let error = assemble_detection_finding(CropDetectionFindingRequest {
            finding_id: "finding-uncited".to_string(),
            field_id: "field-1".to_string(),
            zone_id: None,
            detection,
            model: registered_model(),
            emitted_at: "2026-06-12T15:00:00Z".to_string(),
        })
        .expect_err("uncited finding should be rejected");

        assert_eq!(error, CropDetectionFindingError::EmptyEvidence);
    }

    fn plant_tile(
        tile_id: &str,
        zone_id: Option<&str>,
        valid: bool,
        crop_mask: Vec<bool>,
    ) -> PlantCountTile {
        PlantCountTile {
            tile_id: tile_id.to_string(),
            zone_id: zone_id.map(ToOwned::to_owned),
            valid,
            width_px: 4,
            height_px: 4,
            min_x_m: 0.0,
            min_y_m: 0.0,
            max_x_m: 40.0,
            max_y_m: 40.0,
            crop_mask,
        }
    }

    fn inference_tile(
        tile_id: &str,
        width_px: u32,
        height_px: u32,
        bbox: GeoBounds,
    ) -> TiledInferenceInput {
        TiledInferenceInput {
            tile_id: tile_id.to_string(),
            width_px,
            height_px,
            spatial_ref: spatial_ref_with_bbox(width_px, height_px, bbox),
        }
    }

    fn canopy_tile(
        tile_id: &str,
        zone_id: Option<&str>,
        width_px: u32,
        height_px: u32,
        index_values: Vec<f64>,
        valid_mask: Vec<bool>,
    ) -> CanopyCoverTile {
        CanopyCoverTile {
            tile_id: tile_id.to_string(),
            zone_id: zone_id.map(ToOwned::to_owned),
            width_px,
            height_px,
            spatial_ref: spatial_ref(width_px, height_px),
            index_values,
            valid_mask,
        }
    }

    fn cover_report() -> super::CanopyCoverReport {
        run_canopy_cover(
            "field-1".to_string(),
            vec![canopy_tile(
                "tile-1",
                Some("zone-a"),
                3,
                2,
                vec![0.7, 0.2, 0.5, 0.1, 0.8, 0.4],
                vec![true; 6],
            )],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("cover report should be valid")
    }

    fn stand_count_fixture() -> super::StandCountReport {
        run_stand_count(
            "field-1".to_string(),
            "EPSG:32614".to_string(),
            vec![plant_tile(
                "tile-stand",
                Some("zone-a"),
                true,
                vec![
                    true, false, true, false, false, true, false, false, true, false, false, true,
                    false, false, true, false,
                ],
            )],
            PlantCountConfig {
                min_component_pixels: 1,
            },
            "2026-06-01T12:05:00Z".to_string(),
        )
        .expect("stand count fixture should be valid")
    }

    fn canopy_cover_fixture(
        index_values: Vec<f64>,
        valid_mask: Vec<bool>,
    ) -> super::CanopyCoverReport {
        run_canopy_cover(
            "field-1".to_string(),
            vec![canopy_tile(
                "tile-growth",
                Some("zone-a"),
                2,
                2,
                index_values,
                valid_mask,
            )],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("canopy cover fixture should be valid")
    }

    fn growth_config() -> GrowthStageConfig {
        GrowthStageConfig {
            emergence_cover_max: 0.2,
            vegetative_cover_min: 0.35,
            reproductive_cover_min: 0.85,
            min_index_observations_for_confidence: 2,
        }
    }

    fn index_observation(
        observed_at: &str,
        mean_index_value: f64,
        evidence_ref: &str,
    ) -> GrowthIndexObservation {
        GrowthIndexObservation {
            observed_at: observed_at.to_string(),
            mean_index_value,
            evidence_ref: evidence_ref.to_string(),
        }
    }

    fn registered_model() -> InferenceModelReference {
        InferenceModelReference {
            model_id: "lesion-detector".to_string(),
            version: "2026.06.1".to_string(),
        }
    }

    fn confirmed_detection() -> super::CropDetectionVerificationRecord {
        apply_detection_verification(CropDetectionVerificationRequest {
            detection_id: "disease:tile-1:1".to_string(),
            task: CropModelTask::DiseaseDetection,
            label: "northern_leaf_blight".to_string(),
            confidence: 0.82,
            evidence_tile_refs: vec!["tile-1".to_string()],
            zone_geometry: detection_geometry(5.0, 5.0, 15.0, 15.0),
            action: CropDetectionVerificationAction::Confirmed,
            actor: "agronomist-7".to_string(),
            verified_at: "2026-06-12T14:00:00Z".to_string(),
            corrected_label: None,
            corrected_geometry: None,
        })
        .expect("confirmed detection should be valid")
    }

    fn lesion_candidate(tile_id: &str, confidence: f64, bbox: GeoBounds) -> DiseaseLesionCandidate {
        DiseaseLesionCandidate {
            tile_id: tile_id.to_string(),
            confidence,
            bbox,
        }
    }

    fn pest_model() -> InferenceModelReference {
        InferenceModelReference {
            model_id: "pest-detector".to_string(),
            version: "2026.06.1".to_string(),
        }
    }

    fn pest_candidate(
        tile_id: &str,
        pest_label: &str,
        confidence: f64,
        bbox: GeoBounds,
    ) -> PestDetectionCandidate {
        PestDetectionCandidate {
            tile_id: tile_id.to_string(),
            pest_label: pest_label.to_string(),
            confidence,
            bbox,
        }
    }

    fn weed_model() -> InferenceModelReference {
        InferenceModelReference {
            model_id: "weed-detector".to_string(),
            version: "2026.06.1".to_string(),
        }
    }

    fn weed_candidate(tile_id: &str, confidence: f64, bbox: GeoBounds) -> WeedZoneCandidate {
        WeedZoneCandidate {
            tile_id: tile_id.to_string(),
            confidence,
            bbox,
        }
    }

    fn detection_geometry(
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
    ) -> DetectionZoneGeometry {
        DetectionZoneGeometry {
            crs: "EPSG:32614".to_string(),
            bbox: GeoBounds {
                min_lon,
                min_lat,
                max_lon,
                max_lat,
            },
        }
    }

    fn spatial_ref(width_px: u32, height_px: u32) -> RasterSpatialRef {
        let max_x = width_px as f64 * 10.0;
        let max_y = height_px as f64 * 10.0;
        spatial_ref_with_bbox(
            width_px,
            height_px,
            GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: max_x,
                max_lat: max_y,
            },
        )
    }

    fn spatial_ref_with_bbox(width_px: u32, height_px: u32, bbox: GeoBounds) -> RasterSpatialRef {
        let x_resolution = (bbox.max_lon - bbox.min_lon) / width_px as f64;
        let y_resolution = (bbox.max_lat - bbox.min_lat) / height_px as f64;
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            geo_transform: Some([
                bbox.min_lon,
                x_resolution,
                0.0,
                bbox.max_lat,
                0.0,
                -y_resolution,
            ]),
            bbox: Some(bbox),
            resolution: Some(RasterResolution {
                x: x_resolution,
                y: y_resolution,
            }),
        }
    }
}
