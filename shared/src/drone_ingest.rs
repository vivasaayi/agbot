//! Drone-session ingest manifest (Track A batch 6).
//!
//! `data_collector` serializes a completed capture session (scene + per-capture
//! files with integrity checksums + capture health) into a
//! [`DroneIngestManifest`]; geo_hub receives it at `POST /api/ingest/drone-session`
//! and commits it into the catalog as a scene plus L0 capture products. Living
//! in `shared` keeps the producer (data_collector) from depending on geo_hub.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One captured file in a drone session, carrying its integrity checksum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroneCapture {
    pub capture_id: String,
    /// Catalog product kind, e.g. `raw_capture`, `multispectral_capture`.
    pub kind: String,
    pub file_path: String,
    pub checksum_sha256: String,
    pub size_bytes: u64,
    pub captured_at: String,
}

/// Scene metadata for the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroneIngestScene {
    pub scene_id: String,
    pub sensor: String,
    pub acquired_at: String,
    pub data_path: String,
    #[serde(default = "empty_json_object")]
    pub metadata_json: String,
}

fn empty_json_object() -> String {
    "{}".to_string()
}

/// A complete drone-session ingest: source + scene + captured L0 files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroneIngestManifest {
    pub source_id: String,
    pub session_id: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sensor: Option<String>,
    pub scene: DroneIngestScene,
    pub captures: Vec<DroneCapture>,
    #[serde(default)]
    pub quality: Option<serde_json::Value>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DroneIngestManifestError {
    #[error("manifest is missing {0}")]
    MissingField(&'static str),
    #[error("manifest has no captures")]
    NoCaptures,
    #[error("capture {capture_id} is missing an integrity checksum")]
    MissingChecksum { capture_id: String },
}

impl DroneIngestManifest {
    /// Validate structural integrity before ingest: non-empty ids, at least one
    /// capture, and every capture carries a checksum (the field-integrity
    /// guarantee data_collector promises).
    pub fn validate(&self) -> Result<(), DroneIngestManifestError> {
        if self.source_id.trim().is_empty() {
            return Err(DroneIngestManifestError::MissingField("source_id"));
        }
        if self.session_id.trim().is_empty() {
            return Err(DroneIngestManifestError::MissingField("session_id"));
        }
        if self.scene.scene_id.trim().is_empty() {
            return Err(DroneIngestManifestError::MissingField("scene.scene_id"));
        }
        if self.captures.is_empty() {
            return Err(DroneIngestManifestError::NoCaptures);
        }
        for capture in &self.captures {
            if capture.checksum_sha256.trim().is_empty() {
                return Err(DroneIngestManifestError::MissingChecksum {
                    capture_id: capture.capture_id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> DroneIngestManifest {
        DroneIngestManifest {
            source_id: "drone-1".to_string(),
            session_id: "sess-1".to_string(),
            platform: Some("quad-x".to_string()),
            sensor: Some("multispectral".to_string()),
            scene: DroneIngestScene {
                scene_id: "scene-1".to_string(),
                sensor: "multispectral".to_string(),
                acquired_at: "2026-06-01T00:00:00Z".to_string(),
                data_path: "data/scenes/scene-1".to_string(),
                metadata_json: "{}".to_string(),
            },
            captures: vec![DroneCapture {
                capture_id: "cap-1".to_string(),
                kind: "raw_capture".to_string(),
                file_path: "data/scenes/scene-1/cap-1.tif".to_string(),
                checksum_sha256: "abc123".to_string(),
                size_bytes: 1024,
                captured_at: "2026-06-01T00:00:00Z".to_string(),
            }],
            quality: None,
        }
    }

    #[test]
    fn valid_manifest_passes() {
        assert!(manifest().validate().is_ok());
    }

    #[test]
    fn missing_checksum_is_rejected() {
        let mut m = manifest();
        m.captures[0].checksum_sha256 = "  ".to_string();
        assert_eq!(
            m.validate(),
            Err(DroneIngestManifestError::MissingChecksum {
                capture_id: "cap-1".to_string()
            })
        );
    }

    #[test]
    fn empty_captures_rejected() {
        let mut m = manifest();
        m.captures.clear();
        assert_eq!(m.validate(), Err(DroneIngestManifestError::NoCaptures));
    }

    #[test]
    fn round_trips_through_json() {
        let m = manifest();
        let json = serde_json::to_string(&m).unwrap();
        let back: DroneIngestManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
