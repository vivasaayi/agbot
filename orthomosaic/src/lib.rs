use provenance::{EvidenceObject, EvidenceStore};
use serde::{Deserialize, Serialize};
use shared::schemas::{
    assert_raster_spatial_ref, GeoBounds, GpsCoords, RasterResolution, RasterSpatialRef,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraImuPose {
    pub roll_deg: f64,
    pub pitch_deg: f64,
    pub yaw_deg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraExif {
    pub camera_model: String,
    #[serde(default)]
    pub focal_length_mm: Option<f64>,
    #[serde(default)]
    pub image_width_px: Option<u32>,
    #[serde(default)]
    pub image_height_px: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FrameIngestRequest {
    #[serde(default)]
    pub frame_id: String,
    #[serde(default)]
    pub gps: Option<GpsCoords>,
    #[serde(default)]
    pub imu: Option<CameraImuPose>,
    #[serde(default)]
    pub exif: Option<CameraExif>,
    #[serde(default)]
    pub capture_ts: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FramePoseRecord {
    pub frame_id: String,
    #[serde(default)]
    pub gps: Option<GpsCoords>,
    #[serde(default)]
    pub imu: Option<CameraImuPose>,
    #[serde(default)]
    pub exif: Option<CameraExif>,
    pub capture_ts: String,
}

impl FramePoseRecord {
    pub fn has_camera_pose(&self) -> bool {
        self.gps.is_some() || self.imu.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FrameSetIngestRequest {
    #[serde(default)]
    pub frame_set_id: Option<String>,
    #[serde(default)]
    pub scene_id: String,
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub season_id: String,
    #[serde(default)]
    pub frames: Vec<FrameIngestRequest>,
    #[serde(default)]
    pub crs_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameSetRecord {
    pub frame_set_id: String,
    pub scene_id: String,
    pub field_id: String,
    pub season_id: String,
    pub frames: Vec<FramePoseRecord>,
    #[serde(default)]
    pub crs_hint: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionStatus {
    Queued,
    Reconstructing,
    Orthorectifying,
    Completed,
    Failed,
}

impl ReconstructionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ReconstructionStatus::Queued => "queued",
            ReconstructionStatus::Reconstructing => "reconstructing",
            ReconstructionStatus::Orthorectifying => "orthorectifying",
            ReconstructionStatus::Completed => "completed",
            ReconstructionStatus::Failed => "failed",
        }
    }
}

impl std::str::FromStr for ReconstructionStatus {
    type Err = ReconstructionJobError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "queued" => Ok(ReconstructionStatus::Queued),
            "reconstructing" => Ok(ReconstructionStatus::Reconstructing),
            "orthorectifying" => Ok(ReconstructionStatus::Orthorectifying),
            "completed" => Ok(ReconstructionStatus::Completed),
            "failed" => Ok(ReconstructionStatus::Failed),
            _ => Err(ReconstructionJobError::UnsupportedStatus {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReconstructionJobRequest {
    #[serde(default)]
    pub recon_id: Option<String>,
    #[serde(default)]
    pub frame_set_id: String,
    #[serde(default = "default_reconstruction_params")]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconstructionJobRecord {
    pub recon_id: String,
    pub frame_set_id: String,
    pub params: serde_json::Value,
    pub status: ReconstructionStatus,
    #[serde(default)]
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionProgressStage {
    FeatureMatching,
    CameraRegistration,
    DenseReconstruction,
    Orthorectification,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconstructionProgressEvent {
    pub recon_id: String,
    pub stage: ReconstructionProgressStage,
    pub matched_frames: usize,
    pub registered_cameras: usize,
    pub dense_points: usize,
    pub coverage_fraction: f64,
    pub at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconstructionStallReasonCode {
    NoProgressWithinWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconstructionStallEvent {
    pub recon_id: String,
    pub stage: ReconstructionProgressStage,
    pub last_progress_at: chrono::DateTime<chrono::Utc>,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub stalled_for_seconds: u64,
    pub reason_code: ReconstructionStallReasonCode,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReconstructionJobError {
    #[error("recon_id cannot be empty")]
    EmptyReconId,
    #[error("frame_set_id cannot be empty")]
    EmptyFrameSetId,
    #[error("timestamp cannot be empty")]
    EmptyTimestamp,
    #[error("failure reason cannot be empty")]
    EmptyFailureReason,
    #[error("reconstruction progress coverage_fraction must be finite within 0..=1")]
    InvalidProgressCoverage,
    #[error("unsupported reconstruction status {value}")]
    UnsupportedStatus { value: String },
    #[error("invalid reconstruction status transition {from:?} -> {to:?}")]
    InvalidStatusTransition {
        from: ReconstructionStatus,
        to: ReconstructionStatus,
    },
}

pub fn build_reconstruction_job(
    request: ReconstructionJobRequest,
    issued_recon_id: String,
    created_at: String,
) -> Result<ReconstructionJobRecord, ReconstructionJobError> {
    let recon_id = normalize_optional_text(request.recon_id)
        .or_else(|| normalize_optional_text(Some(issued_recon_id)))
        .ok_or(ReconstructionJobError::EmptyReconId)?;
    let frame_set_id = normalize_required_recon_text(
        request.frame_set_id,
        ReconstructionJobError::EmptyFrameSetId,
    )?;
    let created_at =
        normalize_required_recon_text(created_at, ReconstructionJobError::EmptyTimestamp)?;

    Ok(ReconstructionJobRecord {
        recon_id,
        frame_set_id,
        params: request.params,
        status: ReconstructionStatus::Queued,
        failure_reason: None,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn transition_reconstruction_status(
    mut record: ReconstructionJobRecord,
    next_status: ReconstructionStatus,
    failure_reason: Option<String>,
    updated_at: String,
) -> Result<ReconstructionJobRecord, ReconstructionJobError> {
    validate_reconstruction_transition(record.status, next_status)?;
    let updated_at =
        normalize_required_recon_text(updated_at, ReconstructionJobError::EmptyTimestamp)?;
    let failure_reason = if next_status == ReconstructionStatus::Failed {
        Some(
            normalize_optional_text(failure_reason)
                .ok_or(ReconstructionJobError::EmptyFailureReason)?,
        )
    } else {
        None
    };

    record.status = next_status;
    record.failure_reason = failure_reason;
    record.updated_at = updated_at;
    Ok(record)
}

pub fn build_reconstruction_progress_event(
    recon_id: String,
    stage: ReconstructionProgressStage,
    matched_frames: usize,
    registered_cameras: usize,
    dense_points: usize,
    coverage_fraction: f64,
    at: chrono::DateTime<chrono::Utc>,
) -> Result<ReconstructionProgressEvent, ReconstructionJobError> {
    let recon_id = normalize_required_recon_text(recon_id, ReconstructionJobError::EmptyReconId)?;
    if !coverage_fraction.is_finite() || !(0.0..=1.0).contains(&coverage_fraction) {
        return Err(ReconstructionJobError::InvalidProgressCoverage);
    }

    Ok(ReconstructionProgressEvent {
        recon_id,
        stage,
        matched_frames,
        registered_cameras,
        dense_points,
        coverage_fraction,
        at,
    })
}

pub fn reconstruction_progress_stream(
    mut events: Vec<ReconstructionProgressEvent>,
) -> Vec<ReconstructionProgressEvent> {
    events.sort_by(|left, right| {
        left.at
            .cmp(&right.at)
            .then(left.recon_id.cmp(&right.recon_id))
            .then((left.stage as u8).cmp(&(right.stage as u8)))
    });
    events
}

pub fn detect_reconstruction_stall(
    events: &[ReconstructionProgressEvent],
    detected_at: chrono::DateTime<chrono::Utc>,
    stall_after: std::time::Duration,
) -> Option<ReconstructionStallEvent> {
    let latest = events.iter().max_by(|left, right| {
        left.at
            .cmp(&right.at)
            .then(left.recon_id.cmp(&right.recon_id))
            .then((left.stage as u8).cmp(&(right.stage as u8)))
    })?;
    let stalled_for_seconds = detected_at
        .signed_duration_since(latest.at)
        .num_seconds()
        .max(0) as u64;

    (stalled_for_seconds > stall_after.as_secs()).then(|| ReconstructionStallEvent {
        recon_id: latest.recon_id.clone(),
        stage: latest.stage,
        last_progress_at: latest.at,
        detected_at,
        stalled_for_seconds,
        reason_code: ReconstructionStallReasonCode::NoProgressWithinWindow,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameSetQaConfig {
    pub sensor_width_mm: f64,
    pub sensor_height_mm: f64,
    pub min_forward_overlap_fraction: f64,
    pub min_coverage_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldCoverageExtent {
    pub field_id: String,
    pub origin_latitude: f64,
    pub origin_longitude: f64,
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameQaRecord {
    pub frame_id: String,
    pub gsd_m_per_px: f64,
    pub ground_width_m: f64,
    pub ground_height_m: f64,
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameOverlapQaRecord {
    pub frame_a_id: String,
    pub frame_b_id: String,
    pub overlap_fraction: f64,
    pub passes_threshold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameQaReasonCode {
    InsufficientOverlap,
    InsufficientCoverage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameSetQaGapRegion {
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
    pub reason_code: FrameQaReasonCode,
    #[serde(default)]
    pub frame_a_id: Option<String>,
    #[serde(default)]
    pub frame_b_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameSetQaReport {
    pub frame_set_id: String,
    pub field_id: String,
    pub generated_at: String,
    pub frames: Vec<FrameQaRecord>,
    pub overlaps: Vec<FrameOverlapQaRecord>,
    pub mean_gsd_m_per_px: f64,
    pub coverage_fraction: f64,
    pub gap_regions: Vec<FrameSetQaGapRegion>,
    pub passes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FeatureMatchingConfig {
    pub keypoint_spacing_m: f64,
    pub min_pair_overlap_fraction: f64,
    pub min_inlier_matches: usize,
    pub max_keypoints_per_frame: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedKeypoint {
    pub keypoint_id: String,
    pub ground_cell_id: String,
    pub ground_x_m: f64,
    pub ground_y_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameFeatureSet {
    pub frame_id: String,
    pub keypoints: Vec<DetectedKeypoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FramePairMatchReport {
    pub frame_a_id: String,
    pub frame_b_id: String,
    pub overlap_fraction: f64,
    pub candidate_matches: usize,
    pub inlier_matches: usize,
    pub inlier_ratio: f64,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureMatchReport {
    pub frame_set_id: String,
    pub generated_at: String,
    pub features: Vec<FrameFeatureSet>,
    pub pairs: Vec<FramePairMatchReport>,
    pub graph_connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SparseSfmConfig {
    pub max_reprojection_error_px: f64,
    pub min_observations_per_point: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraPoseEstimate {
    pub frame_id: String,
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
    pub yaw_deg: f64,
    pub reprojection_error_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparsePointRecord {
    pub point_id: String,
    pub ground_x_m: f64,
    pub ground_y_m: f64,
    pub elevation_m: f64,
    pub observations: usize,
    pub reprojection_error_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparseSfmReport {
    pub frame_set_id: String,
    pub generated_at: String,
    pub cameras: Vec<CameraPoseEstimate>,
    pub sparse_points: Vec<SparsePointRecord>,
    pub overall_rms_reprojection_error_px: f64,
    pub max_reprojection_error_px: f64,
    pub passes_reprojection_threshold: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrthomosaicConfig {
    pub output_crs: String,
    pub resolution_m_per_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrthorectifiedFrameRecord {
    pub frame_id: String,
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrthomosaicRaster {
    pub frame_set_id: String,
    pub generated_at: String,
    pub width_px: u32,
    pub height_px: u32,
    pub spatial_ref: RasterSpatialRef,
    pub contributing_frames: Vec<OrthorectifiedFrameRecord>,
    pub extent_round_trips: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DenseReconstructionConfig {
    pub output_crs: String,
    pub sample_spacing_m: f64,
    pub samples_per_sparse_point: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DensePoint {
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DensePointCloud {
    pub frame_set_id: String,
    pub generated_at: String,
    pub crs: String,
    pub extent: GeoBounds,
    pub points: Vec<DensePoint>,
    pub point_count: usize,
    pub density_points_per_square_m: f64,
    pub extent_round_trips: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DsmConfig {
    pub output_crs: String,
    pub resolution_m_per_px: f64,
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
    pub nodata_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DsmRaster {
    pub frame_set_id: String,
    pub generated_at: String,
    pub width_px: u32,
    pub height_px: u32,
    pub spatial_ref: RasterSpatialRef,
    pub elevation_m: Vec<f64>,
    pub point_support_counts: Vec<u32>,
    pub nodata_mask: Vec<bool>,
    pub extent_round_trips: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReprojectionReportConfig {
    pub max_overall_rms_error_px: f64,
    pub max_camera_error_px: f64,
    pub max_point_error_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraReprojectionErrorRecord {
    pub frame_id: String,
    pub reprojection_error_px: f64,
    pub threshold_px: f64,
    pub passes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointReprojectionErrorRecord {
    pub point_id: String,
    pub reprojection_error_px: f64,
    pub threshold_px: f64,
    pub observations: usize,
    pub passes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReprojectionErrorReport {
    pub frame_set_id: String,
    pub generated_at: String,
    pub cameras: Vec<CameraReprojectionErrorRecord>,
    pub points: Vec<PointReprojectionErrorRecord>,
    pub overall_rms_error_px: f64,
    pub max_overall_rms_error_px: f64,
    pub passes: bool,
    pub failing_camera_ids: Vec<String>,
    pub failing_point_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcpMarkedImagePoint {
    pub frame_id: String,
    pub image_x_px: f64,
    pub image_y_px: f64,
    pub estimated_x_m: f64,
    pub estimated_y_m: f64,
    pub estimated_z_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcpSurveyedCoordinate {
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
    pub crs: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundControlPoint {
    pub gcp_id: String,
    pub marked_image_points: Vec<GcpMarkedImagePoint>,
    pub surveyed_coord: GcpSurveyedCoordinate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcpRegistrationRequest {
    pub frame_set_id: String,
    pub project_crs: String,
    pub generated_at: String,
    pub gcps: Vec<GroundControlPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcpResidualRecord {
    pub gcp_id: String,
    pub marked_point_count: usize,
    pub estimated_x_m: f64,
    pub estimated_y_m: f64,
    pub estimated_z_m: f64,
    pub surveyed_x_m: f64,
    pub surveyed_y_m: f64,
    pub surveyed_z_m: f64,
    pub horizontal_residual_m: f64,
    pub vertical_residual_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcpAccuracyReport {
    pub frame_set_id: String,
    pub project_crs: String,
    pub generated_at: String,
    pub residuals: Vec<GcpResidualRecord>,
    pub horizontal_rmse_m: f64,
    pub vertical_rmse_m: f64,
    pub overall_rmse_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledRasterProductRequest {
    pub uri: String,
    pub width_px: u32,
    pub height_px: u32,
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
    pub gsd_m_per_px: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledOutputHandoffRequest {
    pub scene_id: String,
    pub recon_id: String,
    pub generated_at: String,
    pub source_image_ids: Vec<String>,
    pub tile_size_px: u32,
    pub mosaic: TiledRasterProductRequest,
    pub dsm: TiledRasterProductRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilePyramidLevel {
    pub z: u8,
    pub tile_columns: u32,
    pub tile_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledOutputLayer {
    pub product_kind: String,
    pub uri: String,
    pub width_px: u32,
    pub height_px: u32,
    pub spatial_ref: RasterSpatialRef,
    pub gsd_m_per_px: f64,
    pub tile_url_template: String,
    pub tile_pyramid: Vec<TilePyramidLevel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledOutputHandoff {
    pub scene_id: String,
    pub recon_id: String,
    pub generated_at: String,
    pub source_image_ids: Vec<String>,
    pub tile_size_px: u32,
    pub layers: Vec<TiledOutputLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MosaicQualityVerdict {
    Publishable,
    NotPublishable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MosaicPublishStatus {
    Published,
    Blocked,
}

impl MosaicPublishStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MosaicPublishStatus::Published => "published",
            MosaicPublishStatus::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MosaicProvenanceRecord {
    pub frames: Vec<String>,
    pub camera_model: String,
    #[serde(default)]
    pub gcps: Vec<String>,
    pub params: serde_json::Value,
    pub software_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MosaicPublishGateRequest {
    pub scene_id: String,
    pub product_kind: String,
    pub requested_at: String,
    pub qa_report_ref: String,
    pub quality_verdict: MosaicQualityVerdict,
    pub provenance: MosaicProvenanceRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MosaicPublishGateDecision {
    pub scene_id: String,
    pub product_kind: String,
    pub status: MosaicPublishStatus,
    pub quality_verdict: MosaicQualityVerdict,
    pub qa_report_ref: String,
    pub provenance_hash: String,
    pub downstream_consumers: Vec<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    pub provenance: MosaicProvenanceRecord,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FeatureMatchingError {
    #[error("frame set must include at least one frame")]
    EmptyFrameSet,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("feature matching config field {field} must be finite and positive")]
    InvalidConfig { field: &'static str },
    #[error("feature matching config fraction {field} must be within 0..=1")]
    InvalidConfigFraction { field: &'static str },
    #[error("QA report frame_set_id {qa_frame_set_id} does not match frame set {frame_set_id}")]
    FrameSetMismatch {
        frame_set_id: String,
        qa_frame_set_id: String,
    },
    #[error("QA report is missing frame {frame_id}")]
    MissingQaFrame { frame_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SparseSfmFailureReason {
    CouldNotSolve,
    ReprojectionThresholdExceeded,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SparseSfmError {
    #[error("frame set must include at least one frame")]
    EmptyFrameSet,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("sparse SfM config field {field} must be finite and positive")]
    InvalidConfig { field: &'static str },
    #[error("QA report frame_set_id {qa_frame_set_id} does not match frame set {frame_set_id}")]
    FrameSetMismatch {
        frame_set_id: String,
        qa_frame_set_id: String,
    },
    #[error(
        "match report frame_set_id {match_frame_set_id} does not match frame set {frame_set_id}"
    )]
    MatchFrameSetMismatch {
        frame_set_id: String,
        match_frame_set_id: String,
    },
    #[error("QA report is missing frame {frame_id}")]
    MissingQaFrame { frame_id: String },
    #[error("sparse SfM could not solve: {detail}")]
    CouldNotSolve {
        reason_code: SparseSfmFailureReason,
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OrthomosaicError {
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("orthomosaic config output_crs cannot be empty")]
    EmptyOutputCrs,
    #[error("orthomosaic config resolution_m_per_px must be finite and positive")]
    InvalidResolution,
    #[error("QA report frame_set_id {qa_frame_set_id} does not match frame set {frame_set_id}")]
    FrameSetMismatch {
        frame_set_id: String,
        qa_frame_set_id: String,
    },
    #[error("sparse SfM frame_set_id {sfm_frame_set_id} does not match frame set {frame_set_id}")]
    SparseSfmFrameSetMismatch {
        frame_set_id: String,
        sfm_frame_set_id: String,
    },
    #[error("QA report is missing frame {frame_id}")]
    MissingQaFrame { frame_id: String },
    #[error("georeferencing-error: {reason}")]
    GeoreferencingError { reason: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DenseReconstructionError {
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("dense reconstruction config output_crs cannot be empty")]
    EmptyOutputCrs,
    #[error("dense reconstruction sample_spacing_m must be finite and positive")]
    InvalidSampleSpacing,
    #[error("dense reconstruction samples_per_sparse_point must be positive")]
    InvalidSamplesPerSparsePoint,
    #[error("QA report frame_set_id {qa_frame_set_id} does not match frame set {frame_set_id}")]
    FrameSetMismatch {
        frame_set_id: String,
        qa_frame_set_id: String,
    },
    #[error("sparse SfM frame_set_id {sfm_frame_set_id} does not match frame set {frame_set_id}")]
    SparseSfmFrameSetMismatch {
        frame_set_id: String,
        sfm_frame_set_id: String,
    },
    #[error("dense reconstruction refused: {reason}")]
    Refused { reason: String },
    #[error("georeferencing-error: {reason}")]
    GeoreferencingError { reason: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReprojectionReportError {
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("reprojection report config field {field} must be finite and non-negative")]
    InvalidConfig { field: &'static str },
    #[error("sparse SfM report has no camera estimates")]
    EmptyCameras,
    #[error("sparse SfM report has no sparse points")]
    EmptyPoints,
    #[error("sparse SfM report contains a non-finite reprojection error")]
    NonFiniteResidual,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GcpRegistrationError {
    #[error("frame_set_id cannot be empty")]
    EmptyFrameSetId,
    #[error("project_crs cannot be empty")]
    EmptyProjectCrs,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("at least one GCP is required")]
    EmptyGcps,
    #[error("gcp_id cannot be empty")]
    EmptyGcpId,
    #[error("GCP {gcp_id} must include at least one marked image point")]
    EmptyMarkedImagePoints { gcp_id: String },
    #[error("GCP {gcp_id} surveyed CRS {actual_crs} does not match project CRS {expected_crs}")]
    CrsMismatch {
        gcp_id: String,
        expected_crs: String,
        actual_crs: String,
    },
    #[error("GCP {gcp_id} contains a non-finite coordinate")]
    NonFiniteCoordinate { gcp_id: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TiledOutputHandoffError {
    #[error("scene_id cannot be empty")]
    EmptySceneId,
    #[error("recon_id cannot be empty")]
    EmptyReconId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("source_image_ids must include at least one frame id")]
    EmptySourceImageIds,
    #[error("tile_size_px must be positive")]
    InvalidTileSize,
    #[error("product {product_kind} uri cannot be empty")]
    EmptyUri { product_kind: String },
    #[error("product {product_kind} raster dimensions must be positive")]
    InvalidDimensions { product_kind: String },
    #[error("product {product_kind} gsd_m_per_px must be finite and positive")]
    InvalidGsd { product_kind: String },
    #[error("product {product_kind} spatial_ref is invalid: {reason}")]
    InvalidSpatialRef {
        product_kind: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MosaicPublishGateError {
    #[error("scene_id cannot be empty")]
    EmptySceneId,
    #[error("product_kind cannot be empty")]
    EmptyProductKind,
    #[error("requested_at cannot be empty")]
    EmptyRequestedAt,
    #[error("qa_report_ref cannot be empty")]
    EmptyQaReportRef,
    #[error("provenance must include at least one frame")]
    EmptyFrames,
    #[error("provenance frame id cannot be empty")]
    EmptyFrameId,
    #[error("provenance camera_model cannot be empty")]
    EmptyCameraModel,
    #[error("provenance software_version cannot be empty")]
    EmptySoftwareVersion,
    #[error("provenance GCP id cannot be empty")]
    EmptyGcpId,
    #[error("provenance hash failed: {reason}")]
    ProvenanceHashFailed { reason: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DsmError {
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("dense point cloud must include at least one point")]
    EmptyPointCloud,
    #[error("DSM config output_crs cannot be empty")]
    EmptyOutputCrs,
    #[error("DSM config field {field} must be finite")]
    NonFiniteConfig { field: &'static str },
    #[error("DSM config resolution_m_per_px must be finite and positive")]
    InvalidResolution,
    #[error("DSM config extent is invalid")]
    InvalidExtent,
    #[error("dense point contains a non-finite coordinate")]
    NonFinitePoint,
    #[error("georeferencing-error: {reason}")]
    GeoreferencingError { reason: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FrameSetQaError {
    #[error("frame set must include at least one frame")]
    EmptyFrameSet,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("QA config field {field} must be finite and positive")]
    InvalidConfig { field: &'static str },
    #[error("QA config fraction {field} must be within 0..=1")]
    InvalidConfigFraction { field: &'static str },
    #[error("field extent is invalid")]
    InvalidFieldExtent,
    #[error("frame {frame_id} requires GPS for QA")]
    MissingGps { frame_id: String },
    #[error("frame {frame_id} requires EXIF for QA")]
    MissingExif { frame_id: String },
    #[error("frame {frame_id} requires focal_length_mm for QA")]
    MissingFocalLength { frame_id: String },
    #[error("frame {frame_id} requires image dimensions for QA")]
    MissingImageDimensions { frame_id: String },
    #[error("frame {frame_id} has invalid camera intrinsics")]
    InvalidCameraIntrinsics { frame_id: String },
}

pub fn run_frame_set_qa(
    frame_set: &FrameSetRecord,
    field_extent: FieldCoverageExtent,
    config: FrameSetQaConfig,
    generated_at: String,
) -> Result<FrameSetQaReport, FrameSetQaError> {
    validate_qa_config(config)?;
    validate_field_extent(&field_extent)?;
    let generated_at =
        normalize_optional_text(Some(generated_at)).ok_or(FrameSetQaError::EmptyGeneratedAt)?;
    if frame_set.frames.is_empty() {
        return Err(FrameSetQaError::EmptyFrameSet);
    }

    let mut frames = frame_set
        .frames
        .iter()
        .map(|frame| build_frame_qa(frame, &field_extent, config))
        .collect::<Result<Vec<_>, _>>()?;
    frames.sort_by(|left, right| left.frame_id.cmp(&right.frame_id));

    let mut ordered_frames = frame_set.frames.iter().collect::<Vec<_>>();
    ordered_frames.sort_by(|left, right| {
        left.capture_ts
            .cmp(&right.capture_ts)
            .then_with(|| left.frame_id.cmp(&right.frame_id))
    });
    let mut overlaps = Vec::new();
    let mut gap_regions = Vec::new();
    for pair in ordered_frames.windows(2) {
        let frame_a = frames
            .iter()
            .find(|frame| frame.frame_id == pair[0].frame_id)
            .expect("QA frame exists");
        let frame_b = frames
            .iter()
            .find(|frame| frame.frame_id == pair[1].frame_id)
            .expect("QA frame exists");
        let overlap_fraction = overlap_fraction(frame_a, frame_b);
        let passes_threshold = overlap_fraction >= config.min_forward_overlap_fraction;
        overlaps.push(FrameOverlapQaRecord {
            frame_a_id: frame_a.frame_id.clone(),
            frame_b_id: frame_b.frame_id.clone(),
            overlap_fraction,
            passes_threshold,
        });
        if !passes_threshold {
            gap_regions.push(gap_between_frames(
                frame_a,
                frame_b,
                FrameQaReasonCode::InsufficientOverlap,
            ));
        }
    }

    let field_rect = Rect {
        min_x: field_extent.min_x_m,
        min_y: field_extent.min_y_m,
        max_x: field_extent.max_x_m,
        max_y: field_extent.max_y_m,
    };
    let clipped_footprints = frames
        .iter()
        .filter_map(|frame| frame.rect().intersection(&field_rect))
        .collect::<Vec<_>>();
    let coverage_area_m2 = union_area(&clipped_footprints);
    let coverage_fraction = (coverage_area_m2 / field_rect.area()).clamp(0.0, 1.0);
    if coverage_fraction < config.min_coverage_fraction && gap_regions.is_empty() {
        gap_regions.push(FrameSetQaGapRegion {
            min_x_m: field_extent.min_x_m,
            min_y_m: field_extent.min_y_m,
            max_x_m: field_extent.max_x_m,
            max_y_m: field_extent.max_y_m,
            reason_code: FrameQaReasonCode::InsufficientCoverage,
            frame_a_id: None,
            frame_b_id: None,
        });
    }

    let mean_gsd_m_per_px =
        frames.iter().map(|frame| frame.gsd_m_per_px).sum::<f64>() / frames.len() as f64;
    let passes = overlaps.iter().all(|overlap| overlap.passes_threshold)
        && coverage_fraction >= config.min_coverage_fraction;

    Ok(FrameSetQaReport {
        frame_set_id: frame_set.frame_set_id.clone(),
        field_id: field_extent.field_id,
        generated_at,
        frames,
        overlaps,
        mean_gsd_m_per_px,
        coverage_fraction,
        gap_regions,
        passes,
    })
}

pub fn run_feature_matching(
    frame_set: &FrameSetRecord,
    qa_report: &FrameSetQaReport,
    config: FeatureMatchingConfig,
    generated_at: String,
) -> Result<FeatureMatchReport, FeatureMatchingError> {
    validate_feature_matching_config(config)?;
    let generated_at = normalize_optional_text(Some(generated_at))
        .ok_or(FeatureMatchingError::EmptyGeneratedAt)?;
    if frame_set.frames.is_empty() {
        return Err(FeatureMatchingError::EmptyFrameSet);
    }
    if qa_report.frame_set_id != frame_set.frame_set_id {
        return Err(FeatureMatchingError::FrameSetMismatch {
            frame_set_id: frame_set.frame_set_id.clone(),
            qa_frame_set_id: qa_report.frame_set_id.clone(),
        });
    }

    let qa_frames = qa_report
        .frames
        .iter()
        .map(|frame| (frame.frame_id.as_str(), frame))
        .collect::<BTreeMap<_, _>>();
    let mut features = Vec::new();
    for frame in &frame_set.frames {
        let qa_frame = qa_frames.get(frame.frame_id.as_str()).ok_or_else(|| {
            FeatureMatchingError::MissingQaFrame {
                frame_id: frame.frame_id.clone(),
            }
        })?;
        features.push(detect_keypoints_from_footprint(qa_frame, config));
    }
    features.sort_by(|left, right| left.frame_id.cmp(&right.frame_id));

    let feature_cells = features
        .iter()
        .map(|feature_set| {
            (
                feature_set.frame_id.as_str(),
                feature_set
                    .keypoints
                    .iter()
                    .map(|keypoint| keypoint.ground_cell_id.as_str())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut pairs = Vec::new();
    for overlap in &qa_report.overlaps {
        let left = feature_cells
            .get(overlap.frame_a_id.as_str())
            .ok_or_else(|| FeatureMatchingError::MissingQaFrame {
                frame_id: overlap.frame_a_id.clone(),
            })?;
        let right = feature_cells
            .get(overlap.frame_b_id.as_str())
            .ok_or_else(|| FeatureMatchingError::MissingQaFrame {
                frame_id: overlap.frame_b_id.clone(),
            })?;
        let candidate_matches = left.intersection(right).count();
        let overlap_passes = overlap.overlap_fraction >= config.min_pair_overlap_fraction;
        let inlier_matches = if overlap_passes { candidate_matches } else { 0 };
        let inlier_ratio = if candidate_matches == 0 {
            0.0
        } else {
            inlier_matches as f64 / candidate_matches as f64
        };
        let connected = overlap_passes && inlier_matches >= config.min_inlier_matches;

        pairs.push(FramePairMatchReport {
            frame_a_id: overlap.frame_a_id.clone(),
            frame_b_id: overlap.frame_b_id.clone(),
            overlap_fraction: overlap.overlap_fraction,
            candidate_matches,
            inlier_matches,
            inlier_ratio,
            connected,
        });
    }
    pairs.sort_by(|left, right| {
        left.frame_a_id
            .cmp(&right.frame_a_id)
            .then_with(|| left.frame_b_id.cmp(&right.frame_b_id))
    });

    let frame_ids = frame_set
        .frames
        .iter()
        .map(|frame| frame.frame_id.as_str())
        .collect::<Vec<_>>();
    let graph_connected = feature_match_graph_connected(&frame_ids, &pairs);

    Ok(FeatureMatchReport {
        frame_set_id: frame_set.frame_set_id.clone(),
        generated_at,
        features,
        pairs,
        graph_connected,
    })
}

pub fn run_sparse_sfm(
    frame_set: &FrameSetRecord,
    qa_report: &FrameSetQaReport,
    match_report: &FeatureMatchReport,
    config: SparseSfmConfig,
    generated_at: String,
) -> Result<SparseSfmReport, SparseSfmError> {
    validate_sparse_sfm_config(config)?;
    let generated_at =
        normalize_optional_text(Some(generated_at)).ok_or(SparseSfmError::EmptyGeneratedAt)?;
    if frame_set.frames.is_empty() {
        return Err(SparseSfmError::EmptyFrameSet);
    }
    if qa_report.frame_set_id != frame_set.frame_set_id {
        return Err(SparseSfmError::FrameSetMismatch {
            frame_set_id: frame_set.frame_set_id.clone(),
            qa_frame_set_id: qa_report.frame_set_id.clone(),
        });
    }
    if match_report.frame_set_id != frame_set.frame_set_id {
        return Err(SparseSfmError::MatchFrameSetMismatch {
            frame_set_id: frame_set.frame_set_id.clone(),
            match_frame_set_id: match_report.frame_set_id.clone(),
        });
    }
    if !match_report.graph_connected {
        return Err(SparseSfmError::CouldNotSolve {
            reason_code: SparseSfmFailureReason::CouldNotSolve,
            detail: "match graph is disconnected".to_string(),
        });
    }

    let qa_frames = qa_report
        .frames
        .iter()
        .map(|frame| (frame.frame_id.as_str(), frame))
        .collect::<BTreeMap<_, _>>();
    let feature_sets = match_report
        .features
        .iter()
        .map(|features| (features.frame_id.as_str(), features))
        .collect::<BTreeMap<_, _>>();

    let mut tie_point_observations: BTreeMap<&str, Vec<&DetectedKeypoint>> = BTreeMap::new();
    for features in &match_report.features {
        for keypoint in &features.keypoints {
            tie_point_observations
                .entry(keypoint.ground_cell_id.as_str())
                .or_default()
                .push(keypoint);
        }
    }

    let mut sparse_points = tie_point_observations
        .into_iter()
        .filter_map(|(cell_id, observations)| {
            (observations.len() >= config.min_observations_per_point).then(|| {
                let ground_x_m = observations
                    .iter()
                    .map(|keypoint| keypoint.ground_x_m)
                    .sum::<f64>()
                    / observations.len() as f64;
                let ground_y_m = observations
                    .iter()
                    .map(|keypoint| keypoint.ground_y_m)
                    .sum::<f64>()
                    / observations.len() as f64;
                SparsePointRecord {
                    point_id: format!("sparse-point:{cell_id}"),
                    ground_x_m,
                    ground_y_m,
                    elevation_m: 0.0,
                    observations: observations.len(),
                    reprojection_error_px: 0.0,
                }
            })
        })
        .collect::<Vec<_>>();
    sparse_points.sort_by(|left, right| left.point_id.cmp(&right.point_id));
    if sparse_points.is_empty() {
        return Err(SparseSfmError::CouldNotSolve {
            reason_code: SparseSfmFailureReason::CouldNotSolve,
            detail: "insufficient tie points".to_string(),
        });
    }

    let sparse_point_cells = sparse_points
        .iter()
        .map(|point| point.point_id.trim_start_matches("sparse-point:"))
        .collect::<BTreeSet<_>>();
    let mut cameras = Vec::new();
    for frame in &frame_set.frames {
        let qa_frame = qa_frames.get(frame.frame_id.as_str()).ok_or_else(|| {
            SparseSfmError::MissingQaFrame {
                frame_id: frame.frame_id.clone(),
            }
        })?;
        let feature_set = feature_sets.get(frame.frame_id.as_str()).ok_or_else(|| {
            SparseSfmError::CouldNotSolve {
                reason_code: SparseSfmFailureReason::CouldNotSolve,
                detail: format!("missing feature set for frame {}", frame.frame_id),
            }
        })?;
        let retained_observations = feature_set
            .keypoints
            .iter()
            .filter(|keypoint| sparse_point_cells.contains(keypoint.ground_cell_id.as_str()))
            .count();
        if retained_observations == 0 {
            return Err(SparseSfmError::CouldNotSolve {
                reason_code: SparseSfmFailureReason::CouldNotSolve,
                detail: format!("frame {} has no retained tie points", frame.frame_id),
            });
        }

        cameras.push(camera_pose_estimate(frame, qa_frame, 0.0));
    }
    cameras.sort_by(|left, right| left.frame_id.cmp(&right.frame_id));

    let point_error_sum = sparse_points
        .iter()
        .map(|point| point.reprojection_error_px.powi(2))
        .sum::<f64>();
    let camera_error_sum = cameras
        .iter()
        .map(|camera| camera.reprojection_error_px.powi(2))
        .sum::<f64>();
    let sample_count = sparse_points.len() + cameras.len();
    let overall_rms_reprojection_error_px =
        ((point_error_sum + camera_error_sum) / sample_count as f64).sqrt();
    let passes_reprojection_threshold = overall_rms_reprojection_error_px
        <= config.max_reprojection_error_px
        && cameras
            .iter()
            .all(|camera| camera.reprojection_error_px <= config.max_reprojection_error_px)
        && sparse_points
            .iter()
            .all(|point| point.reprojection_error_px <= config.max_reprojection_error_px);
    if !passes_reprojection_threshold {
        return Err(SparseSfmError::CouldNotSolve {
            reason_code: SparseSfmFailureReason::ReprojectionThresholdExceeded,
            detail: "reprojection error exceeds threshold".to_string(),
        });
    }

    Ok(SparseSfmReport {
        frame_set_id: frame_set.frame_set_id.clone(),
        generated_at,
        cameras,
        sparse_points,
        overall_rms_reprojection_error_px,
        max_reprojection_error_px: config.max_reprojection_error_px,
        passes_reprojection_threshold,
    })
}

pub fn densify_sparse_reconstruction(
    frame_set: &FrameSetRecord,
    qa_report: &FrameSetQaReport,
    sfm_report: &SparseSfmReport,
    config: DenseReconstructionConfig,
    generated_at: String,
) -> Result<DensePointCloud, DenseReconstructionError> {
    let output_crs = normalize_optional_text(Some(config.output_crs.clone()))
        .ok_or(DenseReconstructionError::EmptyOutputCrs)?;
    validate_dense_reconstruction_config(&config)?;
    let generated_at = normalize_optional_text(Some(generated_at))
        .ok_or(DenseReconstructionError::EmptyGeneratedAt)?;
    if qa_report.frame_set_id != frame_set.frame_set_id {
        return Err(DenseReconstructionError::FrameSetMismatch {
            frame_set_id: frame_set.frame_set_id.clone(),
            qa_frame_set_id: qa_report.frame_set_id.clone(),
        });
    }
    if sfm_report.frame_set_id != frame_set.frame_set_id {
        return Err(DenseReconstructionError::SparseSfmFrameSetMismatch {
            frame_set_id: frame_set.frame_set_id.clone(),
            sfm_frame_set_id: sfm_report.frame_set_id.clone(),
        });
    }
    assert_dense_pose_set(frame_set, sfm_report)?;

    let frame_extents = qa_report
        .frames
        .iter()
        .map(|frame| OrthorectifiedFrameRecord {
            frame_id: frame.frame_id.clone(),
            min_x_m: frame.min_x_m,
            min_y_m: frame.min_y_m,
            max_x_m: frame.max_x_m,
            max_y_m: frame.max_y_m,
        })
        .collect::<Vec<_>>();
    let extent = mosaic_extent(&frame_extents).ok_or_else(|| {
        DenseReconstructionError::GeoreferencingError {
            reason: "empty_dense_extent".to_string(),
        }
    })?;
    let area_square_m = rect_area_square_m(extent);
    if area_square_m <= 0.0 {
        return Err(DenseReconstructionError::GeoreferencingError {
            reason: "non_positive_dense_extent".to_string(),
        });
    }

    let mut points = Vec::new();
    for sparse_point in &sfm_report.sparse_points {
        for sample_index in 0..config.samples_per_sparse_point {
            let offset = dense_sample_offset(
                sample_index,
                config.samples_per_sparse_point,
                config.sample_spacing_m,
            );
            let x_m = clamp_f64(
                sparse_point.ground_x_m + offset.0,
                extent.min_x,
                extent.max_x,
            );
            let y_m = clamp_f64(
                sparse_point.ground_y_m + offset.1,
                extent.min_y,
                extent.max_y,
            );
            points.push(DensePoint {
                x_m,
                y_m,
                z_m: sparse_point.elevation_m,
            });
        }
    }
    points.sort_by(|left, right| {
        left.x_m
            .partial_cmp(&right.x_m)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.y_m
                    .partial_cmp(&right.y_m)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                left.z_m
                    .partial_cmp(&right.z_m)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let point_count = points.len();
    if point_count == 0 {
        return Err(DenseReconstructionError::Refused {
            reason: "empty_dense_point_cloud".to_string(),
        });
    }

    Ok(DensePointCloud {
        frame_set_id: frame_set.frame_set_id.clone(),
        generated_at,
        crs: output_crs,
        extent: GeoBounds {
            min_lat: extent.min_y,
            min_lon: extent.min_x,
            max_lat: extent.max_y,
            max_lon: extent.max_x,
        },
        points,
        point_count,
        density_points_per_square_m: point_count as f64 / area_square_m,
        extent_round_trips: true,
    })
}

pub fn run_orthorectified_mosaic(
    frame_set: &FrameSetRecord,
    qa_report: &FrameSetQaReport,
    sfm_report: &SparseSfmReport,
    config: OrthomosaicConfig,
    generated_at: String,
) -> Result<OrthomosaicRaster, OrthomosaicError> {
    let output_crs =
        normalize_optional_text(Some(config.output_crs)).ok_or(OrthomosaicError::EmptyOutputCrs)?;
    validate_mosaic_resolution(config.resolution_m_per_px)?;
    let generated_at =
        normalize_optional_text(Some(generated_at)).ok_or(OrthomosaicError::EmptyGeneratedAt)?;
    if qa_report.frame_set_id != frame_set.frame_set_id {
        return Err(OrthomosaicError::FrameSetMismatch {
            frame_set_id: frame_set.frame_set_id.clone(),
            qa_frame_set_id: qa_report.frame_set_id.clone(),
        });
    }
    if sfm_report.frame_set_id != frame_set.frame_set_id {
        return Err(OrthomosaicError::SparseSfmFrameSetMismatch {
            frame_set_id: frame_set.frame_set_id.clone(),
            sfm_frame_set_id: sfm_report.frame_set_id.clone(),
        });
    }
    assert_solved_pose_set(frame_set, sfm_report)?;

    let qa_frames = qa_report
        .frames
        .iter()
        .map(|frame| (frame.frame_id.as_str(), frame))
        .collect::<BTreeMap<_, _>>();
    let mut contributing_frames = Vec::new();
    for frame in &frame_set.frames {
        let qa_frame = qa_frames.get(frame.frame_id.as_str()).ok_or_else(|| {
            OrthomosaicError::MissingQaFrame {
                frame_id: frame.frame_id.clone(),
            }
        })?;
        contributing_frames.push(OrthorectifiedFrameRecord {
            frame_id: frame.frame_id.clone(),
            min_x_m: qa_frame.min_x_m,
            min_y_m: qa_frame.min_y_m,
            max_x_m: qa_frame.max_x_m,
            max_y_m: qa_frame.max_y_m,
        });
    }
    contributing_frames.sort_by(|left, right| left.frame_id.cmp(&right.frame_id));

    let extent = mosaic_extent(&contributing_frames).ok_or_else(|| {
        OrthomosaicError::GeoreferencingError {
            reason: "empty_mosaic_extent".to_string(),
        }
    })?;
    let width_px = ((extent.max_x - extent.min_x) / config.resolution_m_per_px).ceil() as u32;
    let height_px = ((extent.max_y - extent.min_y) / config.resolution_m_per_px).ceil() as u32;
    if width_px == 0 || height_px == 0 {
        return Err(OrthomosaicError::GeoreferencingError {
            reason: "non_positive_mosaic_dimensions".to_string(),
        });
    }

    let adjusted_min_y = extent.max_y - height_px as f64 * config.resolution_m_per_px;
    let adjusted_max_x = extent.min_x + width_px as f64 * config.resolution_m_per_px;
    let spatial_ref = RasterSpatialRef {
        georeferenced: true,
        crs: Some(output_crs),
        bbox: Some(GeoBounds {
            min_lat: adjusted_min_y,
            min_lon: extent.min_x,
            max_lat: extent.max_y,
            max_lon: adjusted_max_x,
        }),
        geo_transform: Some([
            extent.min_x,
            config.resolution_m_per_px,
            0.0,
            extent.max_y,
            0.0,
            -config.resolution_m_per_px,
        ]),
        resolution: Some(RasterResolution {
            x: config.resolution_m_per_px,
            y: config.resolution_m_per_px,
        }),
    };
    let spatial_ref =
        assert_raster_spatial_ref(Some(&spatial_ref), width_px, height_px).map_err(|error| {
            OrthomosaicError::GeoreferencingError {
                reason: error.to_string(),
            }
        })?;

    Ok(OrthomosaicRaster {
        frame_set_id: frame_set.frame_set_id.clone(),
        generated_at,
        width_px,
        height_px,
        spatial_ref,
        contributing_frames,
        extent_round_trips: true,
    })
}

pub fn generate_dsm(
    cloud: &DensePointCloud,
    config: DsmConfig,
    generated_at: String,
) -> Result<DsmRaster, DsmError> {
    let output_crs =
        normalize_optional_text(Some(config.output_crs.clone())).ok_or(DsmError::EmptyOutputCrs)?;
    validate_dsm_config(&config)?;
    let generated_at =
        normalize_optional_text(Some(generated_at)).ok_or(DsmError::EmptyGeneratedAt)?;
    if cloud.points.is_empty() {
        return Err(DsmError::EmptyPointCloud);
    }
    if cloud
        .points
        .iter()
        .any(|point| !point.x_m.is_finite() || !point.y_m.is_finite() || !point.z_m.is_finite())
    {
        return Err(DsmError::NonFinitePoint);
    }

    let width_px = ((config.max_x_m - config.min_x_m) / config.resolution_m_per_px).ceil() as u32;
    let height_px = ((config.max_y_m - config.min_y_m) / config.resolution_m_per_px).ceil() as u32;
    if width_px == 0 || height_px == 0 {
        return Err(DsmError::InvalidExtent);
    }

    let cell_count = width_px as usize * height_px as usize;
    let mut elevation_m = vec![config.nodata_value; cell_count];
    let mut point_support_counts = vec![0u32; cell_count];
    let mut nodata_mask = vec![true; cell_count];

    for point in &cloud.points {
        let Some(index) = dsm_cell_index(point, &config, width_px, height_px) else {
            continue;
        };
        point_support_counts[index] += 1;
        if nodata_mask[index] || point.z_m > elevation_m[index] {
            elevation_m[index] = point.z_m;
        }
        nodata_mask[index] = false;
    }

    let adjusted_max_x = config.min_x_m + width_px as f64 * config.resolution_m_per_px;
    let adjusted_min_y = config.max_y_m - height_px as f64 * config.resolution_m_per_px;
    let spatial_ref = RasterSpatialRef {
        georeferenced: true,
        crs: Some(output_crs),
        bbox: Some(GeoBounds {
            min_lat: adjusted_min_y,
            min_lon: config.min_x_m,
            max_lat: config.max_y_m,
            max_lon: adjusted_max_x,
        }),
        geo_transform: Some([
            config.min_x_m,
            config.resolution_m_per_px,
            0.0,
            config.max_y_m,
            0.0,
            -config.resolution_m_per_px,
        ]),
        resolution: Some(RasterResolution {
            x: config.resolution_m_per_px,
            y: config.resolution_m_per_px,
        }),
    };
    let spatial_ref =
        assert_raster_spatial_ref(Some(&spatial_ref), width_px, height_px).map_err(|error| {
            DsmError::GeoreferencingError {
                reason: error.to_string(),
            }
        })?;

    Ok(DsmRaster {
        frame_set_id: cloud.frame_set_id.clone(),
        generated_at,
        width_px,
        height_px,
        spatial_ref,
        elevation_m,
        point_support_counts,
        nodata_mask,
        extent_round_trips: true,
    })
}

pub fn build_reprojection_error_report(
    sfm_report: &SparseSfmReport,
    config: ReprojectionReportConfig,
    generated_at: String,
) -> Result<ReprojectionErrorReport, ReprojectionReportError> {
    validate_reprojection_report_config(config)?;
    let generated_at = normalize_optional_text(Some(generated_at))
        .ok_or(ReprojectionReportError::EmptyGeneratedAt)?;
    if sfm_report.cameras.is_empty() {
        return Err(ReprojectionReportError::EmptyCameras);
    }
    if sfm_report.sparse_points.is_empty() {
        return Err(ReprojectionReportError::EmptyPoints);
    }
    if sfm_report
        .cameras
        .iter()
        .any(|camera| !camera.reprojection_error_px.is_finite())
        || sfm_report
            .sparse_points
            .iter()
            .any(|point| !point.reprojection_error_px.is_finite())
    {
        return Err(ReprojectionReportError::NonFiniteResidual);
    }

    let mut cameras = sfm_report
        .cameras
        .iter()
        .map(|camera| CameraReprojectionErrorRecord {
            frame_id: camera.frame_id.clone(),
            reprojection_error_px: camera.reprojection_error_px,
            threshold_px: config.max_camera_error_px,
            passes: camera.reprojection_error_px <= config.max_camera_error_px,
        })
        .collect::<Vec<_>>();
    cameras.sort_by(|left, right| left.frame_id.cmp(&right.frame_id));

    let mut points = sfm_report
        .sparse_points
        .iter()
        .map(|point| PointReprojectionErrorRecord {
            point_id: point.point_id.clone(),
            reprojection_error_px: point.reprojection_error_px,
            threshold_px: config.max_point_error_px,
            observations: point.observations,
            passes: point.reprojection_error_px <= config.max_point_error_px,
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left.point_id.cmp(&right.point_id));

    let residual_sum = cameras
        .iter()
        .map(|camera| camera.reprojection_error_px.powi(2))
        .chain(
            points
                .iter()
                .map(|point| point.reprojection_error_px.powi(2)),
        )
        .sum::<f64>();
    let residual_count = cameras.len() + points.len();
    let overall_rms_error_px = (residual_sum / residual_count as f64).sqrt();
    let failing_camera_ids = cameras
        .iter()
        .filter(|camera| !camera.passes)
        .map(|camera| camera.frame_id.clone())
        .collect::<Vec<_>>();
    let failing_point_ids = points
        .iter()
        .filter(|point| !point.passes)
        .map(|point| point.point_id.clone())
        .collect::<Vec<_>>();
    let passes = overall_rms_error_px <= config.max_overall_rms_error_px
        && failing_camera_ids.is_empty()
        && failing_point_ids.is_empty();

    Ok(ReprojectionErrorReport {
        frame_set_id: sfm_report.frame_set_id.clone(),
        generated_at,
        cameras,
        points,
        overall_rms_error_px,
        max_overall_rms_error_px: config.max_overall_rms_error_px,
        passes,
        failing_camera_ids,
        failing_point_ids,
    })
}

pub fn register_ground_control_points(
    request: GcpRegistrationRequest,
) -> Result<GcpAccuracyReport, GcpRegistrationError> {
    let frame_set_id = normalize_optional_text(Some(request.frame_set_id))
        .ok_or(GcpRegistrationError::EmptyFrameSetId)?;
    let project_crs = normalize_optional_text(Some(request.project_crs))
        .ok_or(GcpRegistrationError::EmptyProjectCrs)?;
    let generated_at = normalize_optional_text(Some(request.generated_at))
        .ok_or(GcpRegistrationError::EmptyGeneratedAt)?;
    if request.gcps.is_empty() {
        return Err(GcpRegistrationError::EmptyGcps);
    }

    let mut residuals = request
        .gcps
        .into_iter()
        .map(|gcp| gcp_residual(gcp, &project_crs))
        .collect::<Result<Vec<_>, _>>()?;
    residuals.sort_by(|left, right| left.gcp_id.cmp(&right.gcp_id));

    let residual_count = residuals.len() as f64;
    let horizontal_squared_sum = residuals
        .iter()
        .map(|residual| residual.horizontal_residual_m.powi(2))
        .sum::<f64>();
    let vertical_squared_sum = residuals
        .iter()
        .map(|residual| residual.vertical_residual_m.powi(2))
        .sum::<f64>();

    Ok(GcpAccuracyReport {
        frame_set_id,
        project_crs,
        generated_at,
        residuals,
        horizontal_rmse_m: (horizontal_squared_sum / residual_count).sqrt(),
        vertical_rmse_m: (vertical_squared_sum / residual_count).sqrt(),
        overall_rmse_m: ((horizontal_squared_sum + vertical_squared_sum) / residual_count).sqrt(),
    })
}

pub fn build_tiled_output_handoff(
    request: TiledOutputHandoffRequest,
) -> Result<TiledOutputHandoff, TiledOutputHandoffError> {
    let scene_id = normalize_optional_text(Some(request.scene_id))
        .ok_or(TiledOutputHandoffError::EmptySceneId)?;
    let recon_id = normalize_optional_text(Some(request.recon_id))
        .ok_or(TiledOutputHandoffError::EmptyReconId)?;
    let generated_at = normalize_optional_text(Some(request.generated_at))
        .ok_or(TiledOutputHandoffError::EmptyGeneratedAt)?;
    if request.tile_size_px == 0 {
        return Err(TiledOutputHandoffError::InvalidTileSize);
    }
    let source_image_ids = request
        .source_image_ids
        .into_iter()
        .filter_map(|source_id| normalize_optional_text(Some(source_id)))
        .collect::<Vec<_>>();
    if source_image_ids.is_empty() {
        return Err(TiledOutputHandoffError::EmptySourceImageIds);
    }

    let layers = vec![
        tiled_output_layer(
            "orthomosaic",
            &scene_id,
            request.tile_size_px,
            request.mosaic,
        )?,
        tiled_output_layer("dsm", &scene_id, request.tile_size_px, request.dsm)?,
    ];

    Ok(TiledOutputHandoff {
        scene_id,
        recon_id,
        generated_at,
        source_image_ids,
        tile_size_px: request.tile_size_px,
        layers,
    })
}

pub fn evaluate_mosaic_publish_gate(
    request: MosaicPublishGateRequest,
) -> Result<MosaicPublishGateDecision, MosaicPublishGateError> {
    let scene_id = normalize_optional_text(Some(request.scene_id))
        .ok_or(MosaicPublishGateError::EmptySceneId)?;
    let product_kind = normalize_optional_text(Some(request.product_kind))
        .map(|value| value.to_ascii_lowercase())
        .ok_or(MosaicPublishGateError::EmptyProductKind)?;
    let _requested_at = normalize_optional_text(Some(request.requested_at))
        .ok_or(MosaicPublishGateError::EmptyRequestedAt)?;
    let qa_report_ref = normalize_optional_text(Some(request.qa_report_ref))
        .ok_or(MosaicPublishGateError::EmptyQaReportRef)?;
    let provenance = normalize_mosaic_provenance(request.provenance)?;
    let provenance_hash = mosaic_provenance_hash(
        &scene_id,
        &product_kind,
        &qa_report_ref,
        request.quality_verdict,
        &provenance,
    )?;

    let (status, downstream_consumers, blocked_reason) = match request.quality_verdict {
        MosaicQualityVerdict::Publishable => (
            MosaicPublishStatus::Published,
            vec!["imagery_processor".to_string(), "lidar_mapper".to_string()],
            None,
        ),
        MosaicQualityVerdict::NotPublishable => (
            MosaicPublishStatus::Blocked,
            Vec::new(),
            Some("quality_report_not_publishable".to_string()),
        ),
    };

    Ok(MosaicPublishGateDecision {
        scene_id,
        product_kind,
        status,
        quality_verdict: request.quality_verdict,
        qa_report_ref,
        provenance_hash,
        downstream_consumers,
        blocked_reason,
        provenance,
    })
}

impl FrameQaRecord {
    fn rect(&self) -> Rect {
        Rect {
            min_x: self.min_x_m,
            min_y: self.min_y_m,
            max_x: self.max_x_m,
            max_y: self.max_y_m,
        }
    }
}

fn detect_keypoints_from_footprint(
    frame: &FrameQaRecord,
    config: FeatureMatchingConfig,
) -> FrameFeatureSet {
    let rect = frame.rect();
    let spacing = config.keypoint_spacing_m;
    let start_x = (rect.min_x / spacing).ceil() as i64;
    let end_x = (rect.max_x / spacing).floor() as i64;
    let start_y = (rect.min_y / spacing).ceil() as i64;
    let end_y = (rect.max_y / spacing).floor() as i64;

    let mut keypoints = Vec::new();
    if start_x <= end_x && start_y <= end_y {
        for iy in start_y..=end_y {
            for ix in start_x..=end_x {
                let ground_x_m = ix as f64 * spacing;
                let ground_y_m = iy as f64 * spacing;
                let ground_cell_id = format!("{ix}:{iy}");
                keypoints.push(DetectedKeypoint {
                    keypoint_id: format!("{}:{ground_cell_id}", frame.frame_id),
                    ground_cell_id,
                    ground_x_m,
                    ground_y_m,
                });
            }
        }
    }
    keypoints.truncate(config.max_keypoints_per_frame);

    FrameFeatureSet {
        frame_id: frame.frame_id.clone(),
        keypoints,
    }
}

fn camera_pose_estimate(
    frame: &FramePoseRecord,
    qa_frame: &FrameQaRecord,
    reprojection_error_px: f64,
) -> CameraPoseEstimate {
    let rect = qa_frame.rect();
    CameraPoseEstimate {
        frame_id: frame.frame_id.clone(),
        x_m: (rect.min_x + rect.max_x) / 2.0,
        y_m: (rect.min_y + rect.max_y) / 2.0,
        z_m: frame.gps.as_ref().map(|gps| gps.altitude).unwrap_or(0.0),
        yaw_deg: frame.imu.as_ref().map(|imu| imu.yaw_deg).unwrap_or(0.0),
        reprojection_error_px,
    }
}

fn feature_match_graph_connected(frame_ids: &[&str], pairs: &[FramePairMatchReport]) -> bool {
    if frame_ids.len() <= 1 {
        return true;
    }

    let mut adjacency = frame_ids
        .iter()
        .map(|frame_id| (*frame_id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for pair in pairs.iter().filter(|pair| pair.connected) {
        if let Some(neighbors) = adjacency.get_mut(pair.frame_a_id.as_str()) {
            neighbors.insert(pair.frame_b_id.as_str());
        }
        if let Some(neighbors) = adjacency.get_mut(pair.frame_b_id.as_str()) {
            neighbors.insert(pair.frame_a_id.as_str());
        }
    }

    let mut visited = BTreeSet::new();
    let mut stack = vec![frame_ids[0]];
    while let Some(frame_id) = stack.pop() {
        if !visited.insert(frame_id) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(frame_id) {
            for neighbor in neighbors {
                stack.push(*neighbor);
            }
        }
    }

    visited.len() == frame_ids.len()
}

fn build_frame_qa(
    frame: &FramePoseRecord,
    field_extent: &FieldCoverageExtent,
    config: FrameSetQaConfig,
) -> Result<FrameQaRecord, FrameSetQaError> {
    let gps = frame
        .gps
        .as_ref()
        .ok_or_else(|| FrameSetQaError::MissingGps {
            frame_id: frame.frame_id.clone(),
        })?;
    let exif = frame
        .exif
        .as_ref()
        .ok_or_else(|| FrameSetQaError::MissingExif {
            frame_id: frame.frame_id.clone(),
        })?;
    let focal_length_mm = finite_positive(
        exif.focal_length_mm,
        FrameSetQaError::MissingFocalLength {
            frame_id: frame.frame_id.clone(),
        },
    )?;
    let image_width_px = nonzero_dimension(exif.image_width_px, &frame.frame_id)?;
    let image_height_px = nonzero_dimension(exif.image_height_px, &frame.frame_id)?;
    if gps.altitude <= 0.0 || !gps.altitude.is_finite() {
        return Err(FrameSetQaError::InvalidCameraIntrinsics {
            frame_id: frame.frame_id.clone(),
        });
    }

    let ground_width_m = gps.altitude * config.sensor_width_mm / focal_length_mm;
    let ground_height_m = gps.altitude * config.sensor_height_mm / focal_length_mm;
    if !ground_width_m.is_finite() || !ground_height_m.is_finite() {
        return Err(FrameSetQaError::InvalidCameraIntrinsics {
            frame_id: frame.frame_id.clone(),
        });
    }

    let center_x_m = (gps.longitude - field_extent.origin_longitude)
        * meters_per_degree_lon(field_extent.origin_latitude);
    let center_y_m = (gps.latitude - field_extent.origin_latitude) * METERS_PER_DEGREE_LAT;
    let gsd_x_m_per_px = ground_width_m / image_width_px as f64;
    let gsd_y_m_per_px = ground_height_m / image_height_px as f64;

    Ok(FrameQaRecord {
        frame_id: frame.frame_id.clone(),
        gsd_m_per_px: (gsd_x_m_per_px + gsd_y_m_per_px) / 2.0,
        ground_width_m,
        ground_height_m,
        min_x_m: center_x_m - ground_width_m / 2.0,
        min_y_m: center_y_m - ground_height_m / 2.0,
        max_x_m: center_x_m + ground_width_m / 2.0,
        max_y_m: center_y_m + ground_height_m / 2.0,
    })
}

fn validate_qa_config(config: FrameSetQaConfig) -> Result<(), FrameSetQaError> {
    require_positive("sensor_width_mm", config.sensor_width_mm)?;
    require_positive("sensor_height_mm", config.sensor_height_mm)?;
    require_fraction(
        "min_forward_overlap_fraction",
        config.min_forward_overlap_fraction,
    )?;
    require_fraction("min_coverage_fraction", config.min_coverage_fraction)?;
    Ok(())
}

fn validate_feature_matching_config(
    config: FeatureMatchingConfig,
) -> Result<(), FeatureMatchingError> {
    if !config.keypoint_spacing_m.is_finite() || config.keypoint_spacing_m <= 0.0 {
        return Err(FeatureMatchingError::InvalidConfig {
            field: "keypoint_spacing_m",
        });
    }
    if !config.min_pair_overlap_fraction.is_finite()
        || !(0.0..=1.0).contains(&config.min_pair_overlap_fraction)
    {
        return Err(FeatureMatchingError::InvalidConfigFraction {
            field: "min_pair_overlap_fraction",
        });
    }
    if config.min_inlier_matches == 0 {
        return Err(FeatureMatchingError::InvalidConfig {
            field: "min_inlier_matches",
        });
    }
    if config.max_keypoints_per_frame == 0 {
        return Err(FeatureMatchingError::InvalidConfig {
            field: "max_keypoints_per_frame",
        });
    }

    Ok(())
}

fn validate_sparse_sfm_config(config: SparseSfmConfig) -> Result<(), SparseSfmError> {
    if !config.max_reprojection_error_px.is_finite() || config.max_reprojection_error_px < 0.0 {
        return Err(SparseSfmError::InvalidConfig {
            field: "max_reprojection_error_px",
        });
    }
    if config.min_observations_per_point == 0 {
        return Err(SparseSfmError::InvalidConfig {
            field: "min_observations_per_point",
        });
    }

    Ok(())
}

fn validate_dense_reconstruction_config(
    config: &DenseReconstructionConfig,
) -> Result<(), DenseReconstructionError> {
    if !config.sample_spacing_m.is_finite() || config.sample_spacing_m <= 0.0 {
        return Err(DenseReconstructionError::InvalidSampleSpacing);
    }
    if config.samples_per_sparse_point == 0 {
        return Err(DenseReconstructionError::InvalidSamplesPerSparsePoint);
    }

    Ok(())
}

fn validate_mosaic_resolution(resolution_m_per_px: f64) -> Result<(), OrthomosaicError> {
    if resolution_m_per_px.is_finite() && resolution_m_per_px > 0.0 {
        Ok(())
    } else {
        Err(OrthomosaicError::InvalidResolution)
    }
}

fn assert_dense_pose_set(
    frame_set: &FrameSetRecord,
    sfm_report: &SparseSfmReport,
) -> Result<(), DenseReconstructionError> {
    if !sfm_report.passes_reprojection_threshold
        || sfm_report.cameras.len() != frame_set.frames.len()
        || sfm_report.sparse_points.is_empty()
    {
        return Err(DenseReconstructionError::Refused {
            reason: "unsolved_pose_set".to_string(),
        });
    }

    let solved_frames = sfm_report
        .cameras
        .iter()
        .map(|camera| camera.frame_id.as_str())
        .collect::<BTreeSet<_>>();
    if frame_set
        .frames
        .iter()
        .all(|frame| solved_frames.contains(frame.frame_id.as_str()))
    {
        Ok(())
    } else {
        Err(DenseReconstructionError::Refused {
            reason: "unsolved_pose_set".to_string(),
        })
    }
}

fn assert_solved_pose_set(
    frame_set: &FrameSetRecord,
    sfm_report: &SparseSfmReport,
) -> Result<(), OrthomosaicError> {
    if !sfm_report.passes_reprojection_threshold
        || sfm_report.cameras.len() != frame_set.frames.len()
        || sfm_report.sparse_points.is_empty()
    {
        return Err(OrthomosaicError::GeoreferencingError {
            reason: "unsolved_pose_set".to_string(),
        });
    }

    let solved_frames = sfm_report
        .cameras
        .iter()
        .map(|camera| camera.frame_id.as_str())
        .collect::<BTreeSet<_>>();
    let all_frames_solved = frame_set
        .frames
        .iter()
        .all(|frame| solved_frames.contains(frame.frame_id.as_str()));
    if all_frames_solved {
        Ok(())
    } else {
        Err(OrthomosaicError::GeoreferencingError {
            reason: "unsolved_pose_set".to_string(),
        })
    }
}

fn mosaic_extent(frames: &[OrthorectifiedFrameRecord]) -> Option<Rect> {
    let mut iter = frames.iter();
    let first = iter.next()?;
    let mut extent = Rect {
        min_x: first.min_x_m,
        min_y: first.min_y_m,
        max_x: first.max_x_m,
        max_y: first.max_y_m,
    };
    for frame in iter {
        extent.min_x = extent.min_x.min(frame.min_x_m);
        extent.min_y = extent.min_y.min(frame.min_y_m);
        extent.max_x = extent.max_x.max(frame.max_x_m);
        extent.max_y = extent.max_y.max(frame.max_y_m);
    }
    (extent.area() > 0.0).then_some(extent)
}

fn rect_area_square_m(rect: Rect) -> f64 {
    (rect.max_x - rect.min_x).max(0.0) * (rect.max_y - rect.min_y).max(0.0)
}

fn dense_sample_offset(index: usize, sample_count: usize, spacing_m: f64) -> (f64, f64) {
    if sample_count == 1 {
        return (0.0, 0.0);
    }
    let centered = index as f64 - (sample_count - 1) as f64 / 2.0;
    let axis = if index % 2 == 0 { 1.0 } else { -1.0 };
    (
        centered * spacing_m,
        axis * centered.abs() * spacing_m * 0.5,
    )
}

fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

fn validate_dsm_config(config: &DsmConfig) -> Result<(), DsmError> {
    if !config.resolution_m_per_px.is_finite() || config.resolution_m_per_px <= 0.0 {
        return Err(DsmError::InvalidResolution);
    }
    for (field, value) in [
        ("min_x_m", config.min_x_m),
        ("min_y_m", config.min_y_m),
        ("max_x_m", config.max_x_m),
        ("max_y_m", config.max_y_m),
        ("nodata_value", config.nodata_value),
    ] {
        if !value.is_finite() {
            return Err(DsmError::NonFiniteConfig { field });
        }
    }
    if config.max_x_m <= config.min_x_m || config.max_y_m <= config.min_y_m {
        return Err(DsmError::InvalidExtent);
    }
    Ok(())
}

fn validate_reprojection_report_config(
    config: ReprojectionReportConfig,
) -> Result<(), ReprojectionReportError> {
    for (field, value) in [
        ("max_overall_rms_error_px", config.max_overall_rms_error_px),
        ("max_camera_error_px", config.max_camera_error_px),
        ("max_point_error_px", config.max_point_error_px),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(ReprojectionReportError::InvalidConfig { field });
        }
    }
    Ok(())
}

fn gcp_residual(
    gcp: GroundControlPoint,
    project_crs: &str,
) -> Result<GcpResidualRecord, GcpRegistrationError> {
    let gcp_id =
        normalize_optional_text(Some(gcp.gcp_id)).ok_or(GcpRegistrationError::EmptyGcpId)?;
    let surveyed_crs = normalize_optional_text(Some(gcp.surveyed_coord.crs.clone()))
        .ok_or(GcpRegistrationError::EmptyProjectCrs)?;
    if surveyed_crs != project_crs {
        return Err(GcpRegistrationError::CrsMismatch {
            gcp_id,
            expected_crs: project_crs.to_string(),
            actual_crs: surveyed_crs,
        });
    }
    if gcp.marked_image_points.is_empty() {
        return Err(GcpRegistrationError::EmptyMarkedImagePoints { gcp_id });
    }
    if !gcp.surveyed_coord.x_m.is_finite()
        || !gcp.surveyed_coord.y_m.is_finite()
        || !gcp.surveyed_coord.z_m.is_finite()
        || gcp.marked_image_points.iter().any(|point| {
            !point.image_x_px.is_finite()
                || !point.image_y_px.is_finite()
                || !point.estimated_x_m.is_finite()
                || !point.estimated_y_m.is_finite()
                || !point.estimated_z_m.is_finite()
        })
    {
        return Err(GcpRegistrationError::NonFiniteCoordinate { gcp_id });
    }

    let marked_point_count = gcp.marked_image_points.len();
    let count = marked_point_count as f64;
    let estimated_x_m = gcp
        .marked_image_points
        .iter()
        .map(|point| point.estimated_x_m)
        .sum::<f64>()
        / count;
    let estimated_y_m = gcp
        .marked_image_points
        .iter()
        .map(|point| point.estimated_y_m)
        .sum::<f64>()
        / count;
    let estimated_z_m = gcp
        .marked_image_points
        .iter()
        .map(|point| point.estimated_z_m)
        .sum::<f64>()
        / count;
    let dx_m = estimated_x_m - gcp.surveyed_coord.x_m;
    let dy_m = estimated_y_m - gcp.surveyed_coord.y_m;
    let dz_m = estimated_z_m - gcp.surveyed_coord.z_m;

    Ok(GcpResidualRecord {
        gcp_id,
        marked_point_count,
        estimated_x_m,
        estimated_y_m,
        estimated_z_m,
        surveyed_x_m: gcp.surveyed_coord.x_m,
        surveyed_y_m: gcp.surveyed_coord.y_m,
        surveyed_z_m: gcp.surveyed_coord.z_m,
        horizontal_residual_m: (dx_m.powi(2) + dy_m.powi(2)).sqrt(),
        vertical_residual_m: dz_m.abs(),
    })
}

fn tiled_output_layer(
    product_kind: &str,
    scene_id: &str,
    tile_size_px: u32,
    product: TiledRasterProductRequest,
) -> Result<TiledOutputLayer, TiledOutputHandoffError> {
    let product_kind = product_kind.to_string();
    let uri = normalize_optional_text(Some(product.uri)).ok_or_else(|| {
        TiledOutputHandoffError::EmptyUri {
            product_kind: product_kind.clone(),
        }
    })?;
    if product.width_px == 0 || product.height_px == 0 {
        return Err(TiledOutputHandoffError::InvalidDimensions { product_kind });
    }
    if !product.gsd_m_per_px.is_finite() || product.gsd_m_per_px <= 0.0 {
        return Err(TiledOutputHandoffError::InvalidGsd { product_kind });
    }
    let spatial_ref = assert_raster_spatial_ref(
        product.spatial_ref.as_ref(),
        product.width_px,
        product.height_px,
    )
    .map_err(|error| TiledOutputHandoffError::InvalidSpatialRef {
        product_kind: product_kind.clone(),
        reason: error.to_string(),
    })?;
    let tile_url_template =
        format!("/api/scenes/{scene_id}/products/{product_kind}/tiles/{{z}}/{{x}}/{{y}}.png");
    let tile_pyramid = tile_pyramid(product.width_px, product.height_px, tile_size_px);

    Ok(TiledOutputLayer {
        product_kind,
        uri,
        width_px: product.width_px,
        height_px: product.height_px,
        spatial_ref,
        gsd_m_per_px: product.gsd_m_per_px,
        tile_url_template,
        tile_pyramid,
    })
}

fn tile_pyramid(width_px: u32, height_px: u32, tile_size_px: u32) -> Vec<TilePyramidLevel> {
    let full_columns = width_px.div_ceil(tile_size_px).max(1);
    let full_rows = height_px.div_ceil(tile_size_px).max(1);
    let max_axis = full_columns.max(full_rows);
    let mut levels = Vec::new();
    let mut z = 0_u8;
    let mut tiles_per_axis = 1_u32;
    loop {
        levels.push(TilePyramidLevel {
            z,
            tile_columns: tiles_per_axis.min(full_columns),
            tile_rows: tiles_per_axis.min(full_rows),
        });
        if tiles_per_axis >= max_axis {
            break;
        }
        let Some(next) = tiles_per_axis.checked_mul(2) else {
            break;
        };
        let Some(next_z) = z.checked_add(1) else {
            break;
        };
        tiles_per_axis = next;
        z = next_z;
    }
    levels
}

fn normalize_mosaic_provenance(
    provenance: MosaicProvenanceRecord,
) -> Result<MosaicProvenanceRecord, MosaicPublishGateError> {
    if provenance.frames.is_empty() {
        return Err(MosaicPublishGateError::EmptyFrames);
    }
    let frames = provenance
        .frames
        .into_iter()
        .map(|frame_id| {
            normalize_optional_text(Some(frame_id)).ok_or(MosaicPublishGateError::EmptyFrameId)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let camera_model = normalize_optional_text(Some(provenance.camera_model))
        .ok_or(MosaicPublishGateError::EmptyCameraModel)?;
    let gcps = provenance
        .gcps
        .into_iter()
        .map(|gcp_id| {
            normalize_optional_text(Some(gcp_id)).ok_or(MosaicPublishGateError::EmptyGcpId)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let software_version = normalize_optional_text(Some(provenance.software_version))
        .ok_or(MosaicPublishGateError::EmptySoftwareVersion)?;

    Ok(MosaicProvenanceRecord {
        frames,
        camera_model,
        gcps,
        params: provenance.params,
        software_version,
    })
}

fn mosaic_provenance_hash(
    scene_id: &str,
    product_kind: &str,
    qa_report_ref: &str,
    quality_verdict: MosaicQualityVerdict,
    provenance: &MosaicProvenanceRecord,
) -> Result<String, MosaicPublishGateError> {
    let payload = serde_json::json!({
        "scene_id": scene_id,
        "product_kind": product_kind,
        "qa_report_ref": qa_report_ref,
        "quality_verdict": quality_verdict,
        "provenance": provenance,
    });
    let evidence = EvidenceObject {
        evidence_kind: "orthomosaic_publish_provenance".to_string(),
        payload,
    };
    EvidenceStore::default()
        .store_evidence(evidence)
        .map(|stored| stored.digest)
        .map_err(|error| MosaicPublishGateError::ProvenanceHashFailed {
            reason: error.to_string(),
        })
}

fn dsm_cell_index(
    point: &DensePoint,
    config: &DsmConfig,
    width_px: u32,
    height_px: u32,
) -> Option<usize> {
    if point.x_m < config.min_x_m
        || point.x_m >= config.max_x_m
        || point.y_m < config.min_y_m
        || point.y_m >= config.max_y_m
    {
        return None;
    }
    let col = ((point.x_m - config.min_x_m) / config.resolution_m_per_px).floor() as u32;
    let row = ((config.max_y_m - point.y_m) / config.resolution_m_per_px).floor() as u32;
    if col < width_px && row < height_px {
        Some(row as usize * width_px as usize + col as usize)
    } else {
        None
    }
}

fn validate_field_extent(field_extent: &FieldCoverageExtent) -> Result<(), FrameSetQaError> {
    let valid = field_extent.origin_latitude.is_finite()
        && field_extent.origin_longitude.is_finite()
        && field_extent.min_x_m.is_finite()
        && field_extent.min_y_m.is_finite()
        && field_extent.max_x_m.is_finite()
        && field_extent.max_y_m.is_finite()
        && field_extent.max_x_m > field_extent.min_x_m
        && field_extent.max_y_m > field_extent.min_y_m
        && !field_extent.field_id.trim().is_empty();
    if valid {
        Ok(())
    } else {
        Err(FrameSetQaError::InvalidFieldExtent)
    }
}

fn require_positive(field: &'static str, value: f64) -> Result<(), FrameSetQaError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(FrameSetQaError::InvalidConfig { field })
    }
}

fn require_fraction(field: &'static str, value: f64) -> Result<(), FrameSetQaError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(FrameSetQaError::InvalidConfigFraction { field })
    }
}

fn finite_positive(value: Option<f64>, error: FrameSetQaError) -> Result<f64, FrameSetQaError> {
    let value = value.ok_or_else(|| error.clone())?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(error)
    }
}

fn nonzero_dimension(value: Option<u32>, frame_id: &str) -> Result<u32, FrameSetQaError> {
    match value {
        Some(value) if value > 0 => Ok(value),
        _ => Err(FrameSetQaError::MissingImageDimensions {
            frame_id: frame_id.to_string(),
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Rect {
    fn area(self) -> f64 {
        ((self.max_x - self.min_x).max(0.0)) * ((self.max_y - self.min_y).max(0.0))
    }

    fn intersection(self, other: &Rect) -> Option<Rect> {
        let rect = Rect {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
        };
        (rect.area() > 0.0).then_some(rect)
    }
}

fn overlap_fraction(frame_a: &FrameQaRecord, frame_b: &FrameQaRecord) -> f64 {
    let rect_a = frame_a.rect();
    let rect_b = frame_b.rect();
    let Some(intersection) = rect_a.intersection(&rect_b) else {
        return 0.0;
    };
    let denominator = rect_a.area().min(rect_b.area());
    if denominator <= 0.0 {
        0.0
    } else {
        intersection.area() / denominator
    }
}

fn gap_between_frames(
    frame_a: &FrameQaRecord,
    frame_b: &FrameQaRecord,
    reason_code: FrameQaReasonCode,
) -> FrameSetQaGapRegion {
    let rect_a = frame_a.rect();
    let rect_b = frame_b.rect();
    let (min_x, max_x) = if rect_a.max_x <= rect_b.min_x {
        (rect_a.max_x, rect_b.min_x)
    } else if rect_b.max_x <= rect_a.min_x {
        (rect_b.max_x, rect_a.min_x)
    } else {
        (
            rect_a.min_x.max(rect_b.min_x),
            rect_a.max_x.min(rect_b.max_x),
        )
    };
    let (min_y, max_y) = if rect_a.max_y <= rect_b.min_y {
        (rect_a.max_y, rect_b.min_y)
    } else if rect_b.max_y <= rect_a.min_y {
        (rect_b.max_y, rect_a.min_y)
    } else {
        (
            rect_a.min_y.max(rect_b.min_y),
            rect_a.max_y.min(rect_b.max_y),
        )
    };

    FrameSetQaGapRegion {
        min_x_m: min_x.min(max_x),
        min_y_m: min_y.min(max_y),
        max_x_m: max_x.max(min_x),
        max_y_m: max_y.max(min_y),
        reason_code,
        frame_a_id: Some(frame_a.frame_id.clone()),
        frame_b_id: Some(frame_b.frame_id.clone()),
    }
}

fn union_area(rects: &[Rect]) -> f64 {
    if rects.is_empty() {
        return 0.0;
    }

    let mut xs = rects
        .iter()
        .flat_map(|rect| [rect.min_x, rect.max_x])
        .collect::<Vec<_>>();
    xs.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    xs.dedup_by(|left, right| (*left - *right).abs() <= f64::EPSILON);

    let mut area = 0.0;
    for slab in xs.windows(2) {
        let x0 = slab[0];
        let x1 = slab[1];
        if x1 <= x0 {
            continue;
        }
        let mut intervals = rects
            .iter()
            .filter(|rect| rect.min_x < x1 && rect.max_x > x0)
            .map(|rect| (rect.min_y, rect.max_y))
            .collect::<Vec<_>>();
        intervals.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        area += (x1 - x0) * merged_interval_length(&intervals);
    }
    area
}

fn merged_interval_length(intervals: &[(f64, f64)]) -> f64 {
    let mut total = 0.0;
    let mut current: Option<(f64, f64)> = None;
    for &(start, end) in intervals {
        if end <= start {
            continue;
        }
        match current {
            Some((current_start, current_end)) if start <= current_end => {
                current = Some((current_start, current_end.max(end)));
            }
            Some((current_start, current_end)) => {
                total += current_end - current_start;
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((start, end)) = current {
        total += end - start;
    }
    total
}

const METERS_PER_DEGREE_LAT: f64 = 111_320.0;

fn meters_per_degree_lon(latitude: f64) -> f64 {
    METERS_PER_DEGREE_LAT * latitude.to_radians().cos()
}

fn validate_reconstruction_transition(
    current: ReconstructionStatus,
    next: ReconstructionStatus,
) -> Result<(), ReconstructionJobError> {
    let valid = matches!(
        (current, next),
        (
            ReconstructionStatus::Queued,
            ReconstructionStatus::Reconstructing
        ) | (
            ReconstructionStatus::Reconstructing,
            ReconstructionStatus::Orthorectifying
        ) | (
            ReconstructionStatus::Orthorectifying,
            ReconstructionStatus::Completed
        ) | (ReconstructionStatus::Queued, ReconstructionStatus::Failed)
            | (
                ReconstructionStatus::Reconstructing,
                ReconstructionStatus::Failed
            )
            | (
                ReconstructionStatus::Orthorectifying,
                ReconstructionStatus::Failed
            )
    );

    if valid {
        Ok(())
    } else {
        Err(ReconstructionJobError::InvalidStatusTransition {
            from: current,
            to: next,
        })
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FrameSetIngestError {
    #[error("frame_set_id cannot be empty")]
    EmptyFrameSetId,
    #[error("scene_id cannot be empty")]
    EmptySceneId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("season_id cannot be empty")]
    EmptySeasonId,
    #[error("frame set must include at least one frame")]
    EmptyFrames,
    #[error("frame_id cannot be empty")]
    EmptyFrameId,
    #[error("frame {frame_id} capture_ts cannot be empty")]
    EmptyCaptureTimestamp { frame_id: String },
    #[error("frame {frame_id} has invalid GPS coordinates")]
    InvalidGps { frame_id: String },
    #[error("frame {frame_id} has invalid IMU pose")]
    InvalidImu { frame_id: String },
    #[error("frame {frame_id} has no camera pose")]
    NoCameraPose { frame_id: String },
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
}

pub fn build_frame_set_record(
    request: FrameSetIngestRequest,
    issued_frame_set_id: String,
    created_at: String,
) -> Result<FrameSetRecord, FrameSetIngestError> {
    let frame_set_id = normalize_optional_text(request.frame_set_id)
        .or_else(|| normalize_optional_text(Some(issued_frame_set_id)))
        .ok_or(FrameSetIngestError::EmptyFrameSetId)?;
    let scene_id = normalize_required_text(request.scene_id, FrameSetIngestError::EmptySceneId)?;
    let field_id = normalize_required_text(request.field_id, FrameSetIngestError::EmptyFieldId)?;
    let season_id = normalize_required_text(request.season_id, FrameSetIngestError::EmptySeasonId)?;
    let created_at = normalize_required_text(created_at, FrameSetIngestError::EmptyCreatedAt)?;
    let crs_hint = normalize_optional_text(request.crs_hint);
    if request.frames.is_empty() {
        return Err(FrameSetIngestError::EmptyFrames);
    }

    let frames = request
        .frames
        .into_iter()
        .map(normalize_frame)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(FrameSetRecord {
        frame_set_id,
        scene_id,
        field_id,
        season_id,
        frames,
        crs_hint,
        created_at,
    })
}

fn normalize_frame(frame: FrameIngestRequest) -> Result<FramePoseRecord, FrameSetIngestError> {
    let frame_id = normalize_required_text(frame.frame_id, FrameSetIngestError::EmptyFrameId)?;
    let capture_ts = normalize_required_text(
        frame.capture_ts,
        FrameSetIngestError::EmptyCaptureTimestamp {
            frame_id: frame_id.clone(),
        },
    )?;
    if let Some(gps) = frame.gps.as_ref() {
        validate_gps(gps).map_err(|_| FrameSetIngestError::InvalidGps {
            frame_id: frame_id.clone(),
        })?;
    }
    if let Some(imu) = frame.imu.as_ref() {
        validate_imu(imu).map_err(|_| FrameSetIngestError::InvalidImu {
            frame_id: frame_id.clone(),
        })?;
    }

    let record = FramePoseRecord {
        frame_id,
        gps: frame.gps,
        imu: frame.imu,
        exif: frame.exif,
        capture_ts,
    };
    if !record.has_camera_pose() {
        return Err(FrameSetIngestError::NoCameraPose {
            frame_id: record.frame_id,
        });
    }

    Ok(record)
}

fn normalize_required_text(
    value: String,
    error: FrameSetIngestError,
) -> Result<String, FrameSetIngestError> {
    normalize_optional_text(Some(value)).ok_or(error)
}

fn normalize_required_recon_text(
    value: String,
    error: ReconstructionJobError,
) -> Result<String, ReconstructionJobError> {
    normalize_optional_text(Some(value)).ok_or(error)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn default_reconstruction_params() -> serde_json::Value {
    serde_json::json!({})
}

fn validate_gps(gps: &GpsCoords) -> Result<(), ()> {
    if gps.latitude.is_finite()
        && gps.longitude.is_finite()
        && gps.altitude.is_finite()
        && (-90.0..=90.0).contains(&gps.latitude)
        && (-180.0..=180.0).contains(&gps.longitude)
    {
        Ok(())
    } else {
        Err(())
    }
}

fn validate_imu(imu: &CameraImuPose) -> Result<(), ()> {
    if imu.roll_deg.is_finite() && imu.pitch_deg.is_finite() && imu.yaw_deg.is_finite() {
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_frame_set_record, build_reconstruction_job, build_reconstruction_progress_event,
        build_reprojection_error_report, build_tiled_output_handoff, densify_sparse_reconstruction,
        detect_reconstruction_stall, generate_dsm, reconstruction_progress_stream,
        run_feature_matching, run_frame_set_qa, run_orthorectified_mosaic, run_sparse_sfm,
        transition_reconstruction_status, CameraExif, CameraImuPose, DensePoint, DensePointCloud,
        DenseReconstructionConfig, DenseReconstructionError, DsmConfig, FeatureMatchingConfig,
        FieldCoverageExtent, FrameIngestRequest, FrameQaReasonCode, FrameSetIngestError,
        FrameSetIngestRequest, FrameSetQaConfig, GcpMarkedImagePoint, GcpRegistrationError,
        GcpRegistrationRequest, GcpSurveyedCoordinate, GroundControlPoint, MosaicProvenanceRecord,
        MosaicPublishGateRequest, MosaicPublishStatus, MosaicQualityVerdict, OrthomosaicConfig,
        OrthomosaicError, ReconstructionJobError, ReconstructionJobRequest,
        ReconstructionProgressStage, ReconstructionStallReasonCode, ReconstructionStatus,
        ReprojectionReportConfig, SparseSfmConfig, SparseSfmError, SparseSfmFailureReason,
        TiledOutputHandoffError, TiledOutputHandoffRequest, TiledRasterProductRequest,
    };
    use shared::schemas::{
        assert_raster_spatial_ref, GeoBounds, GpsCoords, RasterResolution, RasterSpatialRef,
    };

    #[test]
    fn frame_metadata_input_parses_exif_gps_imu_pose() {
        let frame: FrameIngestRequest = serde_json::from_value(serde_json::json!({
            "frame_id": "frame-001",
            "capture_ts": "2026-06-01T12:00:00Z",
            "gps": {
                "latitude": 41.10,
                "longitude": -96.70,
                "altitude": 120.0
            },
            "imu": {
                "roll_deg": 1.2,
                "pitch_deg": -0.4,
                "yaw_deg": 87.0
            },
            "exif": {
                "camera_model": "MicaSense RedEdge",
                "focal_length_mm": 5.4,
                "image_width_px": 1280,
                "image_height_px": 960
            }
        }))
        .expect("frame metadata should parse");

        assert_eq!(frame.frame_id, "frame-001");
        assert_eq!(frame.gps.as_ref().map(|gps| gps.latitude), Some(41.10));
        assert_eq!(frame.imu.as_ref().map(|imu| imu.yaw_deg), Some(87.0));
        assert_eq!(
            frame.exif.as_ref().map(|exif| exif.camera_model.as_str()),
            Some("MicaSense RedEdge")
        );
    }

    #[test]
    fn frame_set_ingest_builds_traceable_record_with_pose() {
        let record = build_frame_set_record(
            FrameSetIngestRequest {
                frame_set_id: Some(" frame-set-001 ".to_string()),
                scene_id: " scene-1 ".to_string(),
                field_id: " field-1 ".to_string(),
                season_id: " season-2026 ".to_string(),
                frames: vec![FrameIngestRequest {
                    frame_id: " frame-001 ".to_string(),
                    gps: Some(GpsCoords {
                        latitude: 41.10,
                        longitude: -96.70,
                        altitude: 120.0,
                    }),
                    imu: Some(CameraImuPose {
                        roll_deg: 1.2,
                        pitch_deg: -0.4,
                        yaw_deg: 87.0,
                    }),
                    exif: None,
                    capture_ts: " 2026-06-01T12:00:00Z ".to_string(),
                }],
                crs_hint: Some(" EPSG:4326 ".to_string()),
            },
            "generated-frame-set".to_string(),
            " 2026-06-01T12:05:00Z ".to_string(),
        )
        .expect("frame set should build");

        assert_eq!(record.frame_set_id, "frame-set-001");
        assert_eq!(record.scene_id, "scene-1");
        assert_eq!(record.field_id, "field-1");
        assert_eq!(record.season_id, "season-2026");
        assert_eq!(record.crs_hint.as_deref(), Some("EPSG:4326"));
        assert_eq!(record.frames.len(), 1);
        assert!(record.frames[0].has_camera_pose());
    }

    #[test]
    fn frame_set_ingest_rejects_frame_without_camera_pose() {
        let error = build_frame_set_record(
            FrameSetIngestRequest {
                frame_set_id: Some("frame-set-001".to_string()),
                scene_id: "scene-1".to_string(),
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                frames: vec![FrameIngestRequest {
                    frame_id: "frame-001".to_string(),
                    gps: None,
                    imu: None,
                    exif: None,
                    capture_ts: "2026-06-01T12:00:00Z".to_string(),
                }],
                crs_hint: None,
            },
            "generated-frame-set".to_string(),
            "2026-06-01T12:05:00Z".to_string(),
        )
        .expect_err("no-pose frames should be rejected");

        assert_eq!(
            error,
            FrameSetIngestError::NoCameraPose {
                frame_id: "frame-001".to_string()
            }
        );
    }

    #[test]
    fn reconstruction_job_creation_starts_queued_with_parameters() {
        let job = build_reconstruction_job(
            ReconstructionJobRequest {
                recon_id: Some(" recon-001 ".to_string()),
                frame_set_id: " frame-set-001 ".to_string(),
                params: serde_json::json!({
                    "feature_detector": "orb",
                    "max_features": 4000
                }),
            },
            "generated-recon".to_string(),
            " 2026-06-01T12:10:00Z ".to_string(),
        )
        .expect("job should be created");

        assert_eq!(job.recon_id, "recon-001");
        assert_eq!(job.frame_set_id, "frame-set-001");
        assert_eq!(job.status, ReconstructionStatus::Queued);
        assert_eq!(job.failure_reason, None);
        assert_eq!(
            job.params
                .get("feature_detector")
                .and_then(|value| value.as_str()),
            Some("orb")
        );
    }

    #[test]
    fn reconstruction_job_failure_records_reason() {
        let job = build_reconstruction_job(
            ReconstructionJobRequest {
                recon_id: Some("recon-001".to_string()),
                frame_set_id: "frame-set-001".to_string(),
                params: serde_json::json!({}),
            },
            "generated-recon".to_string(),
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("job should be created");

        let failed = transition_reconstruction_status(
            job,
            ReconstructionStatus::Failed,
            Some(" feature-match-insufficient-overlap ".to_string()),
            "2026-06-01T12:11:00Z".to_string(),
        )
        .expect("job should fail with reason");

        assert_eq!(failed.status, ReconstructionStatus::Failed);
        assert_eq!(
            failed.failure_reason.as_deref(),
            Some("feature-match-insufficient-overlap")
        );
        assert_eq!(failed.updated_at, "2026-06-01T12:11:00Z");
    }

    #[test]
    fn reconstruction_job_rejects_invalid_lifecycle_jump() {
        let job = build_reconstruction_job(
            ReconstructionJobRequest {
                recon_id: Some("recon-001".to_string()),
                frame_set_id: "frame-set-001".to_string(),
                params: serde_json::json!({}),
            },
            "generated-recon".to_string(),
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("job should be created");

        let error = transition_reconstruction_status(
            job,
            ReconstructionStatus::Completed,
            None,
            "2026-06-01T12:11:00Z".to_string(),
        )
        .expect_err("queued cannot jump straight to completed");

        assert_eq!(
            error,
            ReconstructionJobError::InvalidStatusTransition {
                from: ReconstructionStatus::Queued,
                to: ReconstructionStatus::Completed
            }
        );
    }

    #[test]
    fn reconstruction_progress_stream_orders_stage_counts_with_coverage() {
        let later = build_reconstruction_progress_event(
            " recon-001 ".to_string(),
            ReconstructionProgressStage::CameraRegistration,
            12,
            8,
            0,
            0.42,
            dt("2026-06-01T12:10:10Z"),
        )
        .expect("later progress event builds");
        let earlier = build_reconstruction_progress_event(
            "recon-001".to_string(),
            ReconstructionProgressStage::FeatureMatching,
            12,
            0,
            0,
            0.18,
            dt("2026-06-01T12:10:00Z"),
        )
        .expect("earlier progress event builds");

        let stream = reconstruction_progress_stream(vec![later, earlier]);

        assert_eq!(
            stream[0].stage,
            ReconstructionProgressStage::FeatureMatching
        );
        assert_eq!(stream[0].matched_frames, 12);
        assert_eq!(stream[0].coverage_fraction, 0.18);
        assert_eq!(
            stream[1].stage,
            ReconstructionProgressStage::CameraRegistration
        );
        assert_eq!(stream[1].registered_cameras, 8);
        assert_eq!(stream[1].coverage_fraction, 0.42);
    }

    #[test]
    fn reconstruction_progress_stall_is_flagged_after_window() {
        let event = build_reconstruction_progress_event(
            "recon-001".to_string(),
            ReconstructionProgressStage::DenseReconstruction,
            12,
            12,
            40_000,
            0.70,
            dt("2026-06-01T12:10:00Z"),
        )
        .expect("progress event builds");

        let healthy = detect_reconstruction_stall(
            std::slice::from_ref(&event),
            dt("2026-06-01T12:10:20Z"),
            std::time::Duration::from_secs(30),
        );
        assert_eq!(healthy, None);

        let stalled = detect_reconstruction_stall(
            &[event],
            dt("2026-06-01T12:10:45Z"),
            std::time::Duration::from_secs(30),
        )
        .expect("stalled event should be flagged");

        assert_eq!(stalled.recon_id, "recon-001");
        assert_eq!(
            stalled.stage,
            ReconstructionProgressStage::DenseReconstruction
        );
        assert_eq!(stalled.stalled_for_seconds, 45);
        assert_eq!(
            stalled.reason_code,
            ReconstructionStallReasonCode::NoProgressWithinWindow
        );
    }

    #[test]
    fn frame_set_qa_reports_gsd_overlap_and_full_field_coverage() {
        let frame_set = qa_frame_set(vec![
            qa_frame("frame-001", 0.0, "2026-06-01T12:00:00Z"),
            qa_frame("frame-002", 60.0, "2026-06-01T12:00:05Z"),
        ]);
        let qa = run_frame_set_qa(
            &frame_set,
            field_extent(-75.0, -50.0, 135.0, 50.0),
            qa_config(),
            "2026-06-01T12:01:00Z".to_string(),
        )
        .expect("QA should run");

        assert_eq!(qa.frame_set_id, "frame-set-qa");
        assert_eq!(qa.field_id, "field-1");
        assert_eq!(qa.frames.len(), 2);
        assert_close(qa.frames[0].gsd_m_per_px, 0.1);
        assert_close(qa.mean_gsd_m_per_px, 0.1);
        assert_eq!(qa.overlaps.len(), 1);
        assert_close(qa.overlaps[0].overlap_fraction, 0.6);
        assert!(qa.overlaps[0].passes_threshold);
        assert_close(qa.coverage_fraction, 1.0);
        assert!(qa.gap_regions.is_empty());
        assert!(qa.passes);
    }

    #[test]
    fn frame_set_qa_flags_gap_with_reason_code_when_overlap_is_insufficient() {
        let frame_set = qa_frame_set(vec![
            qa_frame("frame-001", 0.0, "2026-06-01T12:00:00Z"),
            qa_frame("frame-002", 170.0, "2026-06-01T12:00:05Z"),
        ]);

        let qa = run_frame_set_qa(
            &frame_set,
            field_extent(-75.0, -50.0, 245.0, 50.0),
            FrameSetQaConfig {
                min_forward_overlap_fraction: 0.3,
                min_coverage_fraction: 0.95,
                ..qa_config()
            },
            "2026-06-01T12:01:00Z".to_string(),
        )
        .expect("QA should run with explicit gap");

        assert!(!qa.passes);
        assert_eq!(qa.overlaps.len(), 1);
        assert_close(qa.overlaps[0].overlap_fraction, 0.0);
        assert!(!qa.overlaps[0].passes_threshold);
        assert_eq!(qa.gap_regions.len(), 1);
        assert_eq!(
            qa.gap_regions[0].reason_code,
            FrameQaReasonCode::InsufficientOverlap
        );
        assert!(qa.gap_regions[0].max_x_m > qa.gap_regions[0].min_x_m);
        assert!(qa.coverage_fraction < 0.95);
    }

    #[test]
    fn feature_matching_connects_overlapping_frame_set_with_inlier_evidence() {
        let frame_set = qa_frame_set(vec![
            qa_frame("frame-001", 0.0, "2026-06-01T12:00:00Z"),
            qa_frame("frame-002", 60.0, "2026-06-01T12:00:05Z"),
        ]);
        let qa = run_frame_set_qa(
            &frame_set,
            field_extent(-75.0, -50.0, 135.0, 50.0),
            qa_config(),
            "2026-06-01T12:01:00Z".to_string(),
        )
        .expect("QA should run");

        let report = run_feature_matching(
            &frame_set,
            &qa,
            feature_config(),
            "2026-06-01T12:02:00Z".to_string(),
        )
        .expect("feature matching should run");
        let repeated = run_feature_matching(
            &frame_set,
            &qa,
            feature_config(),
            "2026-06-01T12:02:00Z".to_string(),
        )
        .expect("feature matching should be deterministic");

        assert_eq!(report, repeated);
        assert_eq!(report.frame_set_id, "frame-set-qa");
        assert!(report.graph_connected);
        assert_eq!(report.pairs.len(), 1);
        assert!(report
            .features
            .iter()
            .all(|features| !features.keypoints.is_empty()));
        assert!(report.pairs[0].connected);
        assert!(report.pairs[0].inlier_matches >= 4);
        assert!(report.pairs[0].inlier_ratio >= 0.9);
    }

    #[test]
    fn feature_matching_does_not_fabricate_links_for_non_overlapping_frames() {
        let frame_set = qa_frame_set(vec![
            qa_frame("frame-001", 0.0, "2026-06-01T12:00:00Z"),
            qa_frame("frame-002", 300.0, "2026-06-01T12:00:05Z"),
        ]);
        let qa = run_frame_set_qa(
            &frame_set,
            field_extent(-75.0, -50.0, 375.0, 50.0),
            qa_config(),
            "2026-06-01T12:01:00Z".to_string(),
        )
        .expect("QA should run");

        let report = run_feature_matching(
            &frame_set,
            &qa,
            feature_config(),
            "2026-06-01T12:02:00Z".to_string(),
        )
        .expect("feature matching should run");

        assert_eq!(report.pairs.len(), 1);
        assert_eq!(report.pairs[0].candidate_matches, 0);
        assert_eq!(report.pairs[0].inlier_matches, 0);
        assert_eq!(report.pairs[0].inlier_ratio, 0.0);
        assert!(!report.pairs[0].connected);
        assert!(!report.graph_connected);
    }

    #[test]
    fn sparse_sfm_solves_connected_match_graph_with_reprojection_evidence() {
        let frame_set = qa_frame_set(vec![
            qa_frame("frame-001", 0.0, "2026-06-01T12:00:00Z"),
            qa_frame("frame-002", 60.0, "2026-06-01T12:00:05Z"),
        ]);
        let qa = run_frame_set_qa(
            &frame_set,
            field_extent(-75.0, -50.0, 135.0, 50.0),
            qa_config(),
            "2026-06-01T12:01:00Z".to_string(),
        )
        .expect("QA should run");
        let matches = run_feature_matching(
            &frame_set,
            &qa,
            feature_config(),
            "2026-06-01T12:02:00Z".to_string(),
        )
        .expect("feature matching should run");

        let sfm = run_sparse_sfm(
            &frame_set,
            &qa,
            &matches,
            sfm_config(),
            "2026-06-01T12:03:00Z".to_string(),
        )
        .expect("connected match graph should solve");

        assert_eq!(sfm.frame_set_id, "frame-set-qa");
        assert_eq!(sfm.cameras.len(), 2);
        assert!(!sfm.sparse_points.is_empty());
        assert!(sfm.passes_reprojection_threshold);
        assert!(sfm
            .cameras
            .iter()
            .all(|camera| camera.reprojection_error_px <= 0.5));
        assert!(sfm.sparse_points.iter().all(|point| point.observations >= 2
            && point.reprojection_error_px <= sfm_config().max_reprojection_error_px));
    }

    #[test]
    fn sparse_sfm_fails_cleanly_for_disconnected_match_graph() {
        let frame_set = qa_frame_set(vec![
            qa_frame("frame-001", 0.0, "2026-06-01T12:00:00Z"),
            qa_frame("frame-002", 300.0, "2026-06-01T12:00:05Z"),
        ]);
        let qa = run_frame_set_qa(
            &frame_set,
            field_extent(-75.0, -50.0, 375.0, 50.0),
            qa_config(),
            "2026-06-01T12:01:00Z".to_string(),
        )
        .expect("QA should run");
        let matches = run_feature_matching(
            &frame_set,
            &qa,
            feature_config(),
            "2026-06-01T12:02:00Z".to_string(),
        )
        .expect("feature matching should run");

        let error = run_sparse_sfm(
            &frame_set,
            &qa,
            &matches,
            sfm_config(),
            "2026-06-01T12:03:00Z".to_string(),
        )
        .expect_err("disconnected graph should not solve");

        assert_eq!(
            error,
            SparseSfmError::CouldNotSolve {
                reason_code: SparseSfmFailureReason::CouldNotSolve,
                detail: "match graph is disconnected".to_string()
            }
        );
    }

    #[test]
    fn dense_reconstruction_produces_point_cloud_with_crs_extent_and_density() {
        let (frame_set, qa, sfm) = solved_sfm_fixture();

        let cloud = densify_sparse_reconstruction(
            &frame_set,
            &qa,
            &sfm,
            dense_config(),
            "2026-06-01T12:04:00Z".to_string(),
        )
        .expect("solved sparse SfM should densify");

        assert_eq!(cloud.frame_set_id, "frame-set-qa");
        assert_eq!(cloud.generated_at, "2026-06-01T12:04:00Z");
        assert_eq!(cloud.crs, "EPSG:32614");
        assert_eq!(
            cloud.point_count,
            sfm.sparse_points.len() * dense_config().samples_per_sparse_point
        );
        assert_eq!(cloud.point_count, cloud.points.len());
        assert!(cloud.density_points_per_square_m > 0.0);
        assert!(cloud.extent_round_trips);
        assert_close(cloud.extent.min_lon, -75.0);
        assert_close(cloud.extent.max_lon, 135.0);
        assert!(cloud.points.iter().all(|point| {
            point.x_m >= cloud.extent.min_lon
                && point.x_m <= cloud.extent.max_lon
                && point.y_m >= cloud.extent.min_lat
                && point.y_m <= cloud.extent.max_lat
        }));
    }

    #[test]
    fn dense_reconstruction_refuses_unconverged_sparse_reconstruction() {
        let (frame_set, qa, mut sfm) = solved_sfm_fixture();
        sfm.passes_reprojection_threshold = false;

        let error = densify_sparse_reconstruction(
            &frame_set,
            &qa,
            &sfm,
            dense_config(),
            "2026-06-01T12:04:00Z".to_string(),
        )
        .expect_err("unconverged sparse SfM must not densify");

        assert_eq!(
            error,
            DenseReconstructionError::Refused {
                reason: "unsolved_pose_set".to_string()
            }
        );
    }

    #[test]
    fn orthorectified_mosaic_round_trips_georeferenced_extent() {
        let (frame_set, qa, sfm) = solved_sfm_fixture();

        let mosaic = run_orthorectified_mosaic(
            &frame_set,
            &qa,
            &sfm,
            mosaic_config(),
            "2026-06-01T12:04:00Z".to_string(),
        )
        .expect("solved poses should produce a mosaic");

        assert_eq!(mosaic.frame_set_id, "frame-set-qa");
        assert_eq!(mosaic.contributing_frames.len(), 2);
        assert!(mosaic.width_px > 0);
        assert!(mosaic.height_px > 0);
        assert!(mosaic.extent_round_trips);
        let asserted =
            assert_raster_spatial_ref(Some(&mosaic.spatial_ref), mosaic.width_px, mosaic.height_px)
                .expect("mosaic spatial ref should round-trip");
        assert_eq!(asserted.crs.as_deref(), Some("EPSG:32614"));
        assert_close(asserted.resolution.unwrap().x, 5.0);
        assert_close(asserted.resolution.unwrap().y, 5.0);
    }

    #[test]
    fn orthorectified_mosaic_refuses_unsolved_pose_set() {
        let (frame_set, qa, mut sfm) = solved_sfm_fixture();
        sfm.passes_reprojection_threshold = false;

        let error = run_orthorectified_mosaic(
            &frame_set,
            &qa,
            &sfm,
            mosaic_config(),
            "2026-06-01T12:04:00Z".to_string(),
        )
        .expect_err("unsolved poses must not publish a mosaic");

        assert_eq!(
            error,
            OrthomosaicError::GeoreferencingError {
                reason: "unsolved_pose_set".to_string()
            }
        );
    }

    #[test]
    fn dsm_generation_rasterizes_dense_points_with_geospatial_round_trip() {
        let dsm = generate_dsm(
            &dense_cloud_fixture(),
            dsm_config(),
            "2026-06-01T12:05:00Z".to_string(),
        )
        .expect("DSM should generate");

        assert_eq!(dsm.width_px, 2);
        assert_eq!(dsm.height_px, 2);
        assert_eq!(dsm.point_support_counts[0], 2);
        assert_close(dsm.elevation_m[0], 102.0);
        assert_eq!(dsm.point_support_counts[3], 1);
        assert_close(dsm.elevation_m[3], 90.0);
        let asserted =
            assert_raster_spatial_ref(Some(&dsm.spatial_ref), dsm.width_px, dsm.height_px)
                .expect("DSM spatial ref should round-trip");
        assert_eq!(asserted.crs.as_deref(), Some("EPSG:32614"));
        assert_close(asserted.resolution.unwrap().x, 10.0);
    }

    #[test]
    fn dsm_generation_marks_unsupported_cells_nodata_without_interpolation() {
        let dsm = generate_dsm(
            &dense_cloud_fixture(),
            dsm_config(),
            "2026-06-01T12:05:00Z".to_string(),
        )
        .expect("DSM should generate");

        assert!(dsm.nodata_mask[1]);
        assert!(dsm.nodata_mask[2]);
        assert_eq!(dsm.point_support_counts[1], 0);
        assert_eq!(dsm.point_support_counts[2], 0);
        assert_eq!(dsm.elevation_m[1], dsm_config().nodata_value);
        assert_eq!(dsm.elevation_m[2], dsm_config().nodata_value);
    }

    #[test]
    fn reprojection_report_passes_known_residual_scene() {
        let (_, _, sfm) = solved_sfm_fixture();

        let report = build_reprojection_error_report(
            &sfm,
            reprojection_config(),
            "2026-06-01T12:06:00Z".to_string(),
        )
        .expect("reprojection report should build");

        assert_eq!(report.frame_set_id, "frame-set-qa");
        assert_eq!(report.cameras.len(), 2);
        assert_eq!(report.points.len(), sfm.sparse_points.len());
        assert!(report.passes);
        assert_close(report.overall_rms_error_px, 0.0);
        assert!(report.cameras.iter().all(|camera| camera.passes));
        assert!(report.points.iter().all(|point| point.passes));
    }

    #[test]
    fn reprojection_report_flags_over_threshold_reconstruction() {
        let (_, _, mut sfm) = solved_sfm_fixture();
        sfm.cameras[0].reprojection_error_px = 3.0;

        let report = build_reprojection_error_report(
            &sfm,
            reprojection_config(),
            "2026-06-01T12:06:00Z".to_string(),
        )
        .expect("reprojection report should build");

        assert!(!report.passes);
        assert!(report.overall_rms_error_px > reprojection_config().max_overall_rms_error_px);
        assert_eq!(
            report.failing_camera_ids,
            vec![sfm.cameras[0].frame_id.clone()]
        );
        assert!(report.failing_point_ids.is_empty());
    }

    #[test]
    fn gcp_registration_reports_residuals_and_overall_accuracy() {
        let report = super::register_ground_control_points(gcp_request(vec![
            gcp(
                " GCP-1 ",
                "EPSG:32614",
                100.0,
                200.0,
                10.0,
                vec![
                    marked("frame-001", 101.0, 202.0, 12.0),
                    marked("frame-002", 99.0, 198.0, 8.0),
                ],
            ),
            gcp(
                "GCP-2",
                "EPSG:32614",
                50.0,
                50.0,
                5.0,
                vec![marked("frame-001", 53.0, 54.0, 7.0)],
            ),
        ]))
        .expect("GCP registration should report residuals");

        assert_eq!(report.frame_set_id, "frame-set-qa");
        assert_eq!(report.project_crs, "EPSG:32614");
        assert_eq!(report.generated_at, "2026-06-01T12:07:00Z");
        assert_eq!(report.residuals.len(), 2);
        assert_eq!(report.residuals[0].gcp_id, "GCP-1");
        assert_eq!(report.residuals[0].marked_point_count, 2);
        assert_close(report.residuals[0].horizontal_residual_m, 0.0);
        assert_close(report.residuals[0].vertical_residual_m, 0.0);
        assert_eq!(report.residuals[1].gcp_id, "GCP-2");
        assert_eq!(report.residuals[1].marked_point_count, 1);
        assert_close(report.residuals[1].horizontal_residual_m, 5.0);
        assert_close(report.residuals[1].vertical_residual_m, 2.0);
        assert_close(report.horizontal_rmse_m, 3.5355339059327378);
        assert_close(report.vertical_rmse_m, 1.4142135623730951);
        assert_close(report.overall_rmse_m, 3.8078865529319543);
    }

    #[test]
    fn gcp_registration_refuses_surveyed_crs_mismatch() {
        let error = super::register_ground_control_points(gcp_request(vec![gcp(
            "GCP-1",
            "EPSG:4326",
            100.0,
            200.0,
            10.0,
            vec![marked("frame-001", 100.0, 200.0, 10.0)],
        )]))
        .expect_err("control points must match the project CRS exactly");

        assert_eq!(
            error,
            GcpRegistrationError::CrsMismatch {
                gcp_id: "GCP-1".to_string(),
                expected_crs: "EPSG:32614".to_string(),
                actual_crs: "EPSG:4326".to_string()
            }
        );
    }

    #[test]
    fn tiled_output_handoff_reports_pyramid_metadata_for_mosaic_and_dsm() {
        let handoff = build_tiled_output_handoff(tiled_handoff_request(
            Some(tile_spatial_ref()),
            Some(tile_spatial_ref()),
        ))
        .expect("handoff should build for asserted rasters");

        assert_eq!(handoff.scene_id, "ortho-scene-1");
        assert_eq!(handoff.recon_id, "recon-ortho-1");
        assert_eq!(handoff.source_image_ids, vec!["frame-001", "frame-002"]);
        assert_eq!(handoff.tile_size_px, 1);
        assert_eq!(handoff.layers.len(), 2);
        assert_eq!(handoff.layers[0].product_kind, "orthomosaic");
        assert_eq!(
            handoff.layers[0].spatial_ref.crs.as_deref(),
            Some("EPSG:4326")
        );
        assert_close(handoff.layers[0].gsd_m_per_px, 0.05);
        assert_eq!(handoff.layers[0].tile_pyramid.len(), 2);
        assert_eq!(handoff.layers[0].tile_pyramid[1].z, 1);
        assert_eq!(handoff.layers[0].tile_pyramid[1].tile_columns, 2);
        assert_eq!(handoff.layers[1].product_kind, "dsm");
    }

    #[test]
    fn tiled_output_handoff_refuses_missing_raster_crs() {
        let mut missing_crs = tile_spatial_ref();
        missing_crs.crs = None;

        let error = build_tiled_output_handoff(tiled_handoff_request(
            Some(missing_crs),
            Some(tile_spatial_ref()),
        ))
        .expect_err("handoff must refuse untraceable rasters");

        assert_eq!(
            error,
            TiledOutputHandoffError::InvalidSpatialRef {
                product_kind: "orthomosaic".to_string(),
                reason: "georeferencing missing CRS".to_string()
            }
        );
    }

    #[test]
    fn mosaic_publish_gate_marks_publishable_with_deterministic_provenance() {
        let decision = super::evaluate_mosaic_publish_gate(mosaic_publish_request(
            MosaicQualityVerdict::Publishable,
        ))
        .expect("publishable QA should pass gate");
        let repeated = super::evaluate_mosaic_publish_gate(mosaic_publish_request(
            MosaicQualityVerdict::Publishable,
        ))
        .expect("same provenance should hash deterministically");

        assert_eq!(decision.status, MosaicPublishStatus::Published);
        assert_eq!(decision.qa_report_ref, "qa-report-001");
        assert_eq!(
            decision.downstream_consumers,
            vec!["imagery_processor", "lidar_mapper"]
        );
        assert!(decision.blocked_reason.is_none());
        assert_eq!(decision.provenance_hash, repeated.provenance_hash);
        assert!(decision.provenance_hash.starts_with("sha256:"));
        assert_eq!(decision.provenance.frames, vec!["frame-001", "frame-002"]);
        assert_eq!(decision.provenance.gcps, vec!["GCP-1"]);
    }

    #[test]
    fn mosaic_publish_gate_blocks_not_publishable_quality_report() {
        let decision = super::evaluate_mosaic_publish_gate(mosaic_publish_request(
            MosaicQualityVerdict::NotPublishable,
        ))
        .expect("failing QA should produce a blocked decision");

        assert_eq!(decision.status, MosaicPublishStatus::Blocked);
        assert_eq!(
            decision.quality_verdict,
            MosaicQualityVerdict::NotPublishable
        );
        assert_eq!(
            decision.blocked_reason.as_deref(),
            Some("quality_report_not_publishable")
        );
        assert!(decision.downstream_consumers.is_empty());
        assert!(decision.provenance_hash.starts_with("sha256:"));
    }

    fn qa_frame_set(frames: Vec<FrameIngestRequest>) -> super::FrameSetRecord {
        build_frame_set_record(
            FrameSetIngestRequest {
                frame_set_id: Some("frame-set-qa".to_string()),
                scene_id: "scene-1".to_string(),
                field_id: "field-1".to_string(),
                season_id: "season-2026".to_string(),
                frames,
                crs_hint: Some("EPSG:32614".to_string()),
            },
            "generated".to_string(),
            "2026-06-01T12:00:30Z".to_string(),
        )
        .expect("frame set should build")
    }

    fn qa_frame(frame_id: &str, x_m: f64, capture_ts: &str) -> FrameIngestRequest {
        const ORIGIN_LAT: f64 = 41.0;
        const ORIGIN_LON: f64 = -96.0;
        let lon = ORIGIN_LON + x_m / meters_per_degree_lon(ORIGIN_LAT);
        FrameIngestRequest {
            frame_id: frame_id.to_string(),
            gps: Some(GpsCoords {
                latitude: ORIGIN_LAT,
                longitude: lon,
                altitude: 100.0,
            }),
            imu: Some(CameraImuPose {
                roll_deg: 0.0,
                pitch_deg: 0.0,
                yaw_deg: 90.0,
            }),
            exif: Some(CameraExif {
                camera_model: "QA Cam".to_string(),
                focal_length_mm: Some(8.8),
                image_width_px: Some(1500),
                image_height_px: Some(1000),
            }),
            capture_ts: capture_ts.to_string(),
        }
    }

    fn field_extent(min_x_m: f64, min_y_m: f64, max_x_m: f64, max_y_m: f64) -> FieldCoverageExtent {
        FieldCoverageExtent {
            field_id: "field-1".to_string(),
            origin_latitude: 41.0,
            origin_longitude: -96.0,
            min_x_m,
            min_y_m,
            max_x_m,
            max_y_m,
        }
    }

    fn qa_config() -> FrameSetQaConfig {
        FrameSetQaConfig {
            sensor_width_mm: 13.2,
            sensor_height_mm: 8.8,
            min_forward_overlap_fraction: 0.3,
            min_coverage_fraction: 0.9,
        }
    }

    fn feature_config() -> FeatureMatchingConfig {
        FeatureMatchingConfig {
            keypoint_spacing_m: 20.0,
            min_pair_overlap_fraction: 0.3,
            min_inlier_matches: 4,
            max_keypoints_per_frame: 128,
        }
    }

    fn sfm_config() -> SparseSfmConfig {
        SparseSfmConfig {
            max_reprojection_error_px: 0.5,
            min_observations_per_point: 2,
        }
    }

    fn mosaic_config() -> OrthomosaicConfig {
        OrthomosaicConfig {
            output_crs: "EPSG:32614".to_string(),
            resolution_m_per_px: 5.0,
        }
    }

    fn dense_config() -> DenseReconstructionConfig {
        DenseReconstructionConfig {
            output_crs: "EPSG:32614".to_string(),
            sample_spacing_m: 2.0,
            samples_per_sparse_point: 3,
        }
    }

    fn solved_sfm_fixture() -> (
        super::FrameSetRecord,
        super::FrameSetQaReport,
        super::SparseSfmReport,
    ) {
        let frame_set = qa_frame_set(vec![
            qa_frame("frame-001", 0.0, "2026-06-01T12:00:00Z"),
            qa_frame("frame-002", 60.0, "2026-06-01T12:00:05Z"),
        ]);
        let qa = run_frame_set_qa(
            &frame_set,
            field_extent(-75.0, -50.0, 135.0, 50.0),
            qa_config(),
            "2026-06-01T12:01:00Z".to_string(),
        )
        .expect("QA should run");
        let matches = run_feature_matching(
            &frame_set,
            &qa,
            feature_config(),
            "2026-06-01T12:02:00Z".to_string(),
        )
        .expect("feature matching should run");
        let sfm = run_sparse_sfm(
            &frame_set,
            &qa,
            &matches,
            sfm_config(),
            "2026-06-01T12:03:00Z".to_string(),
        )
        .expect("sparse SfM should solve");

        (frame_set, qa, sfm)
    }

    fn dense_cloud_fixture() -> DensePointCloud {
        let points = vec![
            DensePoint {
                x_m: 5.0,
                y_m: 15.0,
                z_m: 100.0,
            },
            DensePoint {
                x_m: 6.0,
                y_m: 16.0,
                z_m: 102.0,
            },
            DensePoint {
                x_m: 15.0,
                y_m: 5.0,
                z_m: 90.0,
            },
        ];
        DensePointCloud {
            frame_set_id: "frame-set-qa".to_string(),
            generated_at: "2026-06-01T12:04:00Z".to_string(),
            crs: "EPSG:32614".to_string(),
            extent: GeoBounds {
                min_lat: 0.0,
                min_lon: 0.0,
                max_lat: 20.0,
                max_lon: 20.0,
            },
            point_count: points.len(),
            density_points_per_square_m: points.len() as f64 / 400.0,
            extent_round_trips: true,
            points,
        }
    }

    fn dsm_config() -> DsmConfig {
        DsmConfig {
            output_crs: "EPSG:32614".to_string(),
            resolution_m_per_px: 10.0,
            min_x_m: 0.0,
            min_y_m: 0.0,
            max_x_m: 20.0,
            max_y_m: 20.0,
            nodata_value: -9999.0,
        }
    }

    fn reprojection_config() -> ReprojectionReportConfig {
        ReprojectionReportConfig {
            max_overall_rms_error_px: 0.5,
            max_camera_error_px: 0.5,
            max_point_error_px: 0.5,
        }
    }

    fn gcp_request(gcps: Vec<GroundControlPoint>) -> GcpRegistrationRequest {
        GcpRegistrationRequest {
            frame_set_id: " frame-set-qa ".to_string(),
            project_crs: " EPSG:32614 ".to_string(),
            generated_at: " 2026-06-01T12:07:00Z ".to_string(),
            gcps,
        }
    }

    fn gcp(
        gcp_id: &str,
        crs: &str,
        surveyed_x_m: f64,
        surveyed_y_m: f64,
        surveyed_z_m: f64,
        marked_image_points: Vec<GcpMarkedImagePoint>,
    ) -> GroundControlPoint {
        GroundControlPoint {
            gcp_id: gcp_id.to_string(),
            marked_image_points,
            surveyed_coord: GcpSurveyedCoordinate {
                x_m: surveyed_x_m,
                y_m: surveyed_y_m,
                z_m: surveyed_z_m,
                crs: crs.to_string(),
            },
        }
    }

    fn marked(
        frame_id: &str,
        estimated_x_m: f64,
        estimated_y_m: f64,
        estimated_z_m: f64,
    ) -> GcpMarkedImagePoint {
        GcpMarkedImagePoint {
            frame_id: frame_id.to_string(),
            image_x_px: 120.0,
            image_y_px: 240.0,
            estimated_x_m,
            estimated_y_m,
            estimated_z_m,
        }
    }

    fn tiled_handoff_request(
        mosaic_spatial_ref: Option<RasterSpatialRef>,
        dsm_spatial_ref: Option<RasterSpatialRef>,
    ) -> TiledOutputHandoffRequest {
        TiledOutputHandoffRequest {
            scene_id: " ortho-scene-1 ".to_string(),
            recon_id: " recon-ortho-1 ".to_string(),
            generated_at: " 2026-06-01T12:08:00Z ".to_string(),
            source_image_ids: vec![" frame-001 ".to_string(), " frame-002 ".to_string()],
            tile_size_px: 1,
            mosaic: TiledRasterProductRequest {
                uri: " /data/ortho-scene-1/products/orthomosaic/orthomosaic.png ".to_string(),
                width_px: 2,
                height_px: 2,
                spatial_ref: mosaic_spatial_ref,
                gsd_m_per_px: 0.05,
            },
            dsm: TiledRasterProductRequest {
                uri: " /data/ortho-scene-1/products/dsm/dsm.png ".to_string(),
                width_px: 2,
                height_px: 2,
                spatial_ref: dsm_spatial_ref,
                gsd_m_per_px: 0.05,
            },
        }
    }

    fn tile_spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -96.7,
                min_lat: 41.1,
                max_lon: -96.6,
                max_lat: 41.2,
            }),
            geo_transform: Some([-96.7, 0.05, 0.0, 41.2, 0.0, -0.05]),
            resolution: Some(RasterResolution { x: 0.05, y: 0.05 }),
        }
    }

    fn mosaic_publish_request(quality_verdict: MosaicQualityVerdict) -> MosaicPublishGateRequest {
        MosaicPublishGateRequest {
            scene_id: " ortho-scene-1 ".to_string(),
            product_kind: " orthomosaic ".to_string(),
            requested_at: " 2026-06-01T12:09:00Z ".to_string(),
            qa_report_ref: " qa-report-001 ".to_string(),
            quality_verdict,
            provenance: MosaicProvenanceRecord {
                frames: vec![" frame-001 ".to_string(), " frame-002 ".to_string()],
                camera_model: " MicaSense RedEdge ".to_string(),
                gcps: vec![" GCP-1 ".to_string()],
                params: serde_json::json!({
                    "feature_detector": "orb",
                    "resolution_m_per_px": 0.05
                }),
                software_version: " agbot-orthomosaic 0.1.0 ".to_string(),
            },
        }
    }

    fn dt(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn meters_per_degree_lon(latitude: f64) -> f64 {
        111_320.0 * latitude.to_radians().cos()
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-6,
            "expected {expected}, got {actual}"
        );
    }
}
