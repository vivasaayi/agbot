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

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
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
        build_frame_set_record, CameraImuPose, FrameIngestRequest, FrameSetIngestError,
        FrameSetIngestRequest,
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
}
