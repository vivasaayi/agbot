use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilSensorType {
    ElectricalConductivity,
    MicroWeather,
    SoilMoisture,
    SoilTemperature,
}

impl SoilSensorType {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilSensorType::ElectricalConductivity => "electrical_conductivity",
            SoilSensorType::MicroWeather => "micro_weather",
            SoilSensorType::SoilMoisture => "soil_moisture",
            SoilSensorType::SoilTemperature => "soil_temperature",
        }
    }
}

impl std::str::FromStr for SoilSensorType {
    type Err = SoilIotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "electrical_conductivity" | "ec" => Ok(Self::ElectricalConductivity),
            "micro_weather" | "weather" => Ok(Self::MicroWeather),
            "soil_moisture" => Ok(Self::SoilMoisture),
            "soil_temperature" => Ok(Self::SoilTemperature),
            _ => Err(SoilIotError::UnsupportedSensorType {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoilDeviceStatus {
    Registered,
    Active,
    Maintenance,
    Retired,
}

impl SoilDeviceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SoilDeviceStatus::Registered => "registered",
            SoilDeviceStatus::Active => "active",
            SoilDeviceStatus::Maintenance => "maintenance",
            SoilDeviceStatus::Retired => "retired",
        }
    }
}

impl std::str::FromStr for SoilDeviceStatus {
    type Err = SoilIotError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "registered" => Ok(Self::Registered),
            "active" => Ok(Self::Active),
            "maintenance" => Ok(Self::Maintenance),
            "retired" => Ok(Self::Retired),
            _ => Err(SoilIotError::UnsupportedStatus {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPosition {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub crs: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RegisterSoilDeviceRequest {
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub org_id: String,
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub sensor_type: SoilSensorType,
    pub position: GeoPosition,
    #[serde(default)]
    pub calibration_profile_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilDeviceRecord {
    pub device_id: String,
    pub org_id: String,
    pub field_id: String,
    pub zone_id: Option<String>,
    pub sensor_type: SoilSensorType,
    pub position: GeoPosition,
    pub calibration_profile_ref: String,
    pub status: SoilDeviceStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReadingDeviceCheckRequest {
    #[serde(default)]
    pub device_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingRejectionReason {
    RetiredDevice,
    UnknownDevice,
}

impl ReadingRejectionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            ReadingRejectionReason::RetiredDevice => "retired_device",
            ReadingRejectionReason::UnknownDevice => "unknown_device",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadingRejectionRecord {
    pub rejection_id: String,
    pub device_id: String,
    pub reason: ReadingRejectionReason,
    pub rejected_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SoilIotError {
    #[error("device_id cannot be empty")]
    EmptyDeviceId,
    #[error("rejection_id cannot be empty")]
    EmptyRejectionId,
    #[error("org_id cannot be empty")]
    EmptyOrgId,
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("calibration_profile_ref cannot be empty")]
    EmptyCalibrationProfileRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("soil IoT position CRS cannot be empty")]
    EmptyCrs,
    #[error("unsupported soil IoT position CRS {value}; expected EPSG:4326")]
    UnsupportedCrs { value: String },
    #[error("soil IoT position latitude/longitude is out of bounds")]
    InvalidPosition,
    #[error("unsupported soil sensor type {value}")]
    UnsupportedSensorType { value: String },
    #[error("unsupported soil device status {value}")]
    UnsupportedStatus { value: String },
    #[error("retired device {device_id} cannot leave retired status")]
    RetiredDeviceCannotTransition { device_id: String },
}

pub fn build_soil_device_record(
    request: RegisterSoilDeviceRequest,
    generated_device_id: String,
    created_at: String,
) -> Result<SoilDeviceRecord, SoilIotError> {
    let device_id = match normalize_optional_text(request.device_id) {
        Some(device_id) => device_id,
        None => normalize_required_text(generated_device_id, SoilIotError::EmptyDeviceId)?,
    };
    let position = normalize_position(request.position)?;
    let created_at = normalize_required_text(created_at, SoilIotError::EmptyCreatedAt)?;

    Ok(SoilDeviceRecord {
        device_id,
        org_id: normalize_required_text(request.org_id, SoilIotError::EmptyOrgId)?,
        field_id: normalize_required_text(request.field_id, SoilIotError::EmptyFieldId)?,
        zone_id: normalize_optional_text(request.zone_id),
        sensor_type: request.sensor_type,
        position,
        calibration_profile_ref: normalize_required_text(
            request.calibration_profile_ref,
            SoilIotError::EmptyCalibrationProfileRef,
        )?,
        status: SoilDeviceStatus::Active,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn transition_soil_device_status(
    record: &SoilDeviceRecord,
    status: SoilDeviceStatus,
    updated_at: String,
) -> Result<SoilDeviceRecord, SoilIotError> {
    if record.status == SoilDeviceStatus::Retired && status != SoilDeviceStatus::Retired {
        return Err(SoilIotError::RetiredDeviceCannotTransition {
            device_id: record.device_id.clone(),
        });
    }

    let mut updated = record.clone();
    updated.status = status;
    updated.updated_at = normalize_required_text(updated_at, SoilIotError::EmptyCreatedAt)?;
    Ok(updated)
}

pub fn reading_rejection_for_device(
    device: Option<&SoilDeviceRecord>,
    device_id: String,
    generated_rejection_id: String,
    rejected_at: String,
) -> Result<Option<ReadingRejectionRecord>, SoilIotError> {
    let requested_device_id = normalize_required_text(device_id, SoilIotError::EmptyDeviceId)?;
    let reason = match device {
        None => Some(ReadingRejectionReason::UnknownDevice),
        Some(device) if device.status == SoilDeviceStatus::Retired => {
            Some(ReadingRejectionReason::RetiredDevice)
        }
        Some(_) => None,
    };

    reason
        .map(|reason| {
            Ok(ReadingRejectionRecord {
                rejection_id: normalize_required_text(
                    generated_rejection_id,
                    SoilIotError::EmptyRejectionId,
                )?,
                device_id: requested_device_id,
                reason,
                rejected_at: normalize_required_text(rejected_at, SoilIotError::EmptyCreatedAt)?,
            })
        })
        .transpose()
}

fn normalize_position(position: GeoPosition) -> Result<GeoPosition, SoilIotError> {
    let crs = normalize_required_text(position.crs, SoilIotError::EmptyCrs)?;
    if crs != "EPSG:4326" {
        return Err(SoilIotError::UnsupportedCrs { value: crs });
    }
    if !position.latitude.is_finite()
        || !position.longitude.is_finite()
        || !(-90.0..=90.0).contains(&position.latitude)
        || !(-180.0..=180.0).contains(&position.longitude)
    {
        return Err(SoilIotError::InvalidPosition);
    }

    Ok(GeoPosition {
        latitude: position.latitude,
        longitude: position.longitude,
        crs,
    })
}

fn normalize_required_text(value: String, error: SoilIotError) -> Result<String, SoilIotError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_soil_device_record, reading_rejection_for_device, transition_soil_device_status,
        GeoPosition, ReadingRejectionReason, RegisterSoilDeviceRequest, SoilDeviceStatus,
        SoilIotError, SoilSensorType,
    };

    #[test]
    fn device_registry_builds_active_geolocated_record() {
        let record = build_soil_device_record(
            RegisterSoilDeviceRequest {
                device_id: Some(" soil-probe-001 ".to_string()),
                org_id: " org-001 ".to_string(),
                field_id: " field-001 ".to_string(),
                zone_id: Some(" zone-ne ".to_string()),
                sensor_type: SoilSensorType::SoilMoisture,
                position: GeoPosition {
                    latitude: 38.5816,
                    longitude: -121.4944,
                    crs: " EPSG:4326 ".to_string(),
                },
                calibration_profile_ref: " calibration:soil-probe-001:v1 ".to_string(),
            },
            "generated-device".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("device should be valid");

        assert_eq!(record.device_id, "soil-probe-001");
        assert_eq!(record.org_id, "org-001");
        assert_eq!(record.field_id, "field-001");
        assert_eq!(record.zone_id.as_deref(), Some("zone-ne"));
        assert_eq!(record.status, SoilDeviceStatus::Active);
        assert_eq!(record.position.crs, "EPSG:4326");
    }

    #[test]
    fn device_lifecycle_allows_maintenance_then_retirement() {
        let record = valid_device();
        let maintenance = transition_soil_device_status(
            &record,
            SoilDeviceStatus::Maintenance,
            "2026-06-13T10:00:00Z".to_string(),
        )
        .expect("active device can enter maintenance");
        assert_eq!(maintenance.status, SoilDeviceStatus::Maintenance);

        let retired = transition_soil_device_status(
            &maintenance,
            SoilDeviceStatus::Retired,
            "2026-06-14T10:00:00Z".to_string(),
        )
        .expect("maintenance device can retire");
        let error = transition_soil_device_status(
            &retired,
            SoilDeviceStatus::Active,
            "2026-06-15T10:00:00Z".to_string(),
        )
        .expect_err("retired devices cannot reactivate");

        assert_eq!(
            error,
            SoilIotError::RetiredDeviceCannotTransition {
                device_id: "soil-probe-001".to_string()
            }
        );
    }

    #[test]
    fn reading_from_unknown_or_retired_device_is_rejected() {
        let unknown = reading_rejection_for_device(
            None,
            "soil-probe-missing".to_string(),
            "rejection-001".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("rejection should build")
        .expect("unknown device should reject");
        assert_eq!(unknown.reason, ReadingRejectionReason::UnknownDevice);

        let retired = transition_soil_device_status(
            &valid_device(),
            SoilDeviceStatus::Retired,
            "2026-06-13T10:00:00Z".to_string(),
        )
        .expect("device can retire");
        let rejection = reading_rejection_for_device(
            Some(&retired),
            retired.device_id.clone(),
            "rejection-002".to_string(),
            "2026-06-14T10:00:00Z".to_string(),
        )
        .expect("rejection should build")
        .expect("retired device should reject");
        assert_eq!(rejection.reason, ReadingRejectionReason::RetiredDevice);
    }

    #[test]
    fn device_registry_rejects_invalid_position() {
        let error = build_soil_device_record(
            RegisterSoilDeviceRequest {
                device_id: Some("soil-probe-001".to_string()),
                org_id: "org-001".to_string(),
                field_id: "field-001".to_string(),
                zone_id: None,
                sensor_type: SoilSensorType::SoilMoisture,
                position: GeoPosition {
                    latitude: 120.0,
                    longitude: -121.4944,
                    crs: "EPSG:4326".to_string(),
                },
                calibration_profile_ref: "calibration:soil-probe-001:v1".to_string(),
            },
            "generated-device".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect_err("invalid position should be rejected");

        assert_eq!(error, SoilIotError::InvalidPosition);
    }

    fn valid_device() -> super::SoilDeviceRecord {
        build_soil_device_record(
            RegisterSoilDeviceRequest {
                device_id: Some("soil-probe-001".to_string()),
                org_id: "org-001".to_string(),
                field_id: "field-001".to_string(),
                zone_id: Some("zone-ne".to_string()),
                sensor_type: SoilSensorType::SoilMoisture,
                position: GeoPosition {
                    latitude: 38.5816,
                    longitude: -121.4944,
                    crs: "EPSG:4326".to_string(),
                },
                calibration_profile_ref: "calibration:soil-probe-001:v1".to_string(),
            },
            "generated-device".to_string(),
            "2026-06-12T10:00:00Z".to_string(),
        )
        .expect("device should be valid")
    }
}
