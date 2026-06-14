use crate::{
    CollectionFailureKind, CollectionFailureRequest, DataPayload, DataType, FlightDataProvenance,
    FlightDataProvenanceError, FlightDataRecord,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultispectralBandCapture {
    pub name: String,
    pub file_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub exposure_time_ms: u32,
    pub gain: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultispectralCaptureManifest {
    pub expected_bands: Vec<String>,
    pub bands: Vec<MultispectralBandCapture>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MultispectralCaptureError {
    #[error("multispectral capture requires at least one expected band")]
    EmptyExpectedBands,
    #[error("multispectral capture includes an empty band name")]
    EmptyBandName,
    #[error("multispectral capture includes duplicate band {band}")]
    DuplicateBand { band: String },
    #[error("multispectral capture is missing required bands: {missing:?}")]
    MissingBands { missing: Vec<String> },
    #[error("multispectral band {band} is missing a file path")]
    MissingFilePath { band: String },
    #[error("multispectral band {band} has invalid dimensions {width}x{height}")]
    InvalidDimensions {
        band: String,
        width: u32,
        height: u32,
    },
    #[error("multispectral band {band} dimensions {actual:?} do not match expected {expected:?}")]
    DimensionMismatch {
        band: String,
        expected: (u32, u32),
        actual: (u32, u32),
    },
    #[error("multispectral band {band} has invalid exposure or gain")]
    InvalidExposure { band: String },
}

#[derive(Debug, thiserror::Error)]
pub enum MultispectralRecordError {
    #[error(transparent)]
    Capture(#[from] MultispectralCaptureError),
    #[error(transparent)]
    Provenance(#[from] FlightDataProvenanceError),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

impl MultispectralCaptureError {
    pub fn missing_bands(&self) -> Vec<String> {
        match self {
            Self::MissingBands { missing } => missing.clone(),
            _ => Vec::new(),
        }
    }

    pub fn into_collection_failure(
        self,
        sensor_id: impl Into<String>,
        occurred_at: DateTime<Utc>,
    ) -> CollectionFailureRequest {
        let kind = if matches!(self, Self::MissingBands { .. }) {
            CollectionFailureKind::MissingBand
        } else {
            CollectionFailureKind::ReaderError
        };

        CollectionFailureRequest {
            occurred_at: Some(occurred_at),
            sensor_id: sensor_id.into(),
            data_type: DataType::MultispectralImage,
            kind,
            message: self.to_string(),
        }
    }
}

pub fn validate_multispectral_capture(
    manifest: &MultispectralCaptureManifest,
) -> Result<(), MultispectralCaptureError> {
    if manifest.expected_bands.is_empty() {
        return Err(MultispectralCaptureError::EmptyExpectedBands);
    }

    let mut expected = Vec::with_capacity(manifest.expected_bands.len());
    let mut expected_seen = HashSet::new();
    for band in &manifest.expected_bands {
        let normalized = normalize_band_name(band)?;
        if !expected_seen.insert(normalized.clone()) {
            return Err(MultispectralCaptureError::DuplicateBand { band: normalized });
        }
        expected.push(normalized);
    }

    let mut captured_seen = HashSet::new();
    let mut reference_dimensions = None;
    for band in &manifest.bands {
        let normalized = normalize_band_name(&band.name)?;
        if !captured_seen.insert(normalized.clone()) {
            return Err(MultispectralCaptureError::DuplicateBand { band: normalized });
        }
        if band.file_path.as_os_str().is_empty() {
            return Err(MultispectralCaptureError::MissingFilePath { band: normalized });
        }
        if band.width == 0 || band.height == 0 {
            return Err(MultispectralCaptureError::InvalidDimensions {
                band: normalized,
                width: band.width,
                height: band.height,
            });
        }
        if band.exposure_time_ms == 0 || !band.gain.is_finite() || band.gain <= 0.0 {
            return Err(MultispectralCaptureError::InvalidExposure { band: normalized });
        }

        let dimensions = (band.width, band.height);
        if let Some(expected_dimensions) = reference_dimensions {
            if dimensions != expected_dimensions {
                return Err(MultispectralCaptureError::DimensionMismatch {
                    band: normalized,
                    expected: expected_dimensions,
                    actual: dimensions,
                });
            }
        } else {
            reference_dimensions = Some(dimensions);
        }
    }

    let missing = expected
        .iter()
        .filter(|band| !captured_seen.contains(*band))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(MultispectralCaptureError::MissingBands { missing });
    }

    Ok(())
}

pub fn multispectral_capture_to_record(
    flight_id: Uuid,
    drone_id: Uuid,
    manifest: &MultispectralCaptureManifest,
    provenance: FlightDataProvenance,
) -> Result<FlightDataRecord, MultispectralRecordError> {
    validate_multispectral_capture(manifest)?;

    let dimensions = manifest.bands.first().map(|band| (band.width, band.height));
    let size_bytes = serde_json::to_vec(manifest)?.len() as u64;
    let calibration_ref = provenance.calibration_ref.clone();
    let mut record = FlightDataRecord::new(
        flight_id,
        drone_id,
        DataType::MultispectralImage,
        DataPayload::MediaFile {
            file_type: "multispectral/tiff-stack".to_string(),
            dimensions,
            duration_seconds: None,
            compression: Some("tiff".to_string()),
        },
        provenance,
        size_bytes,
    )?;

    record
        .metadata
        .insert("bands".to_string(), manifest.expected_bands.join(","));
    record
        .metadata
        .insert("calibration_ref".to_string(), calibration_ref);
    record.metadata.insert(
        "band_files".to_string(),
        serde_json::to_string(
            &manifest
                .bands
                .iter()
                .map(|band| (&band.name, band.file_path.to_string_lossy().to_string()))
                .collect::<Vec<_>>(),
        )?,
    );
    record
        .metadata
        .insert("schema".to_string(), "multispectral/tiff-stack".to_string());
    Ok(record)
}

fn normalize_band_name(band: &str) -> Result<String, MultispectralCaptureError> {
    let normalized = band.trim();
    if normalized.is_empty() {
        return Err(MultispectralCaptureError::EmptyBandName);
    }
    Ok(normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataPayload, DataType, FlightDataProvenance};
    use chrono::{TimeZone, Utc};
    use shared::schemas::GpsCoords;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn band(name: &str) -> MultispectralBandCapture {
        MultispectralBandCapture {
            name: name.to_string(),
            file_path: PathBuf::from(format!("/captures/field-1/{name}.tif")),
            width: 1024,
            height: 768,
            exposure_time_ms: 8,
            gain: 1.2,
        }
    }

    fn manifest() -> MultispectralCaptureManifest {
        MultispectralCaptureManifest {
            expected_bands: vec![
                "Red".to_string(),
                "Green".to_string(),
                "Blue".to_string(),
                "NIR".to_string(),
            ],
            bands: vec![band("Red"), band("Green"), band("Blue"), band("NIR")],
        }
    }

    fn provenance(session_id: Uuid) -> FlightDataProvenance {
        FlightDataProvenance::complete(
            session_id,
            "micasense-rededge".to_string(),
            GpsCoords {
                latitude: 41.0,
                longitude: -96.0,
                altitude: 402.0,
            },
            Utc.timestamp_opt(2000, 0).unwrap(),
            "panel-cal-2026-05-10".to_string(),
        )
    }

    #[test]
    fn multispectral_capture_builds_calibrated_record_when_bands_complete() {
        let session_id = Uuid::new_v4();
        let flight_id = Uuid::new_v4();
        let drone_id = Uuid::new_v4();

        let record = multispectral_capture_to_record(
            flight_id,
            drone_id,
            &manifest(),
            provenance(session_id),
        )
        .expect("complete multispectral manifest should build a record");

        assert_eq!(record.session_id, session_id);
        assert_eq!(record.flight_id, flight_id);
        assert_eq!(record.drone_id, drone_id);
        assert_eq!(record.data_type, DataType::MultispectralImage);
        assert_eq!(record.sensor_id, "micasense-rededge");
        assert_eq!(record.calibration_ref, "panel-cal-2026-05-10");
        assert_eq!(
            record.metadata.get("bands"),
            Some(&"Red,Green,Blue,NIR".to_string())
        );
        assert_eq!(
            record.metadata.get("calibration_ref"),
            Some(&"panel-cal-2026-05-10".to_string())
        );
        match record.payload {
            DataPayload::MediaFile {
                file_type,
                dimensions,
                ..
            } => {
                assert_eq!(file_type, "multispectral/tiff-stack");
                assert_eq!(dimensions, Some((1024, 768)));
            }
            _ => panic!("expected media file payload"),
        }
    }

    #[test]
    fn multispectral_capture_rejects_missing_band_with_failure() {
        let timestamp = Utc.timestamp_opt(2000, 0).unwrap();
        let mut manifest = manifest();
        manifest.bands.retain(|band| band.name != "NIR");

        let error = validate_multispectral_capture(&manifest)
            .expect_err("missing NIR should reject capture");
        assert_eq!(error.missing_bands(), vec!["NIR".to_string()]);

        let failure = error.into_collection_failure("micasense-rededge", timestamp);

        assert_eq!(failure.data_type, DataType::MultispectralImage);
        assert_eq!(failure.kind, crate::CollectionFailureKind::MissingBand);
        assert!(failure.message.contains("NIR"));
    }

    #[test]
    fn multispectral_capture_rejects_dimension_mismatch() {
        let mut manifest = manifest();
        manifest.bands[2].width = 640;

        let error = validate_multispectral_capture(&manifest)
            .expect_err("dimension mismatch should reject capture");

        assert!(matches!(
            error,
            MultispectralCaptureError::DimensionMismatch { .. }
        ));
    }
}
