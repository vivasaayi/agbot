use serde::{Deserialize, Serialize};
use shared::schemas::GpsCoords;

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
        build_frame_set_record, build_reconstruction_job, transition_reconstruction_status,
        CameraImuPose, FrameIngestRequest, FrameSetIngestError, FrameSetIngestRequest,
        ReconstructionJobError, ReconstructionJobRequest, ReconstructionStatus,
    };
    use shared::schemas::GpsCoords;

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
}
