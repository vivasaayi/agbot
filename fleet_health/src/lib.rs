use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetComponentType {
    Airframe,
    Battery,
    Controller,
    Esc,
    Motor,
    Propeller,
    Sensor,
}

impl FleetComponentType {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetComponentType::Airframe => "airframe",
            FleetComponentType::Battery => "battery",
            FleetComponentType::Controller => "controller",
            FleetComponentType::Esc => "esc",
            FleetComponentType::Motor => "motor",
            FleetComponentType::Propeller => "propeller",
            FleetComponentType::Sensor => "sensor",
        }
    }
}

impl std::str::FromStr for FleetComponentType {
    type Err = FleetHealthError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "airframe" => Ok(Self::Airframe),
            "battery" => Ok(Self::Battery),
            "controller" => Ok(Self::Controller),
            "esc" => Ok(Self::Esc),
            "motor" => Ok(Self::Motor),
            "propeller" => Ok(Self::Propeller),
            "sensor" => Ok(Self::Sensor),
            _ => Err(FleetHealthError::UnsupportedComponentType {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceHistoryEntry {
    #[serde(default)]
    pub service_id: String,
    #[serde(default)]
    pub performed_at: String,
    #[serde(default)]
    pub technician: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RegisterComponentRequest {
    #[serde(default)]
    pub component_id: Option<String>,
    pub component_type: FleetComponentType,
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub airframe_id: Option<String>,
    #[serde(default)]
    pub installed_at: Option<String>,
    #[serde(default)]
    pub removed_at: Option<String>,
    #[serde(default)]
    pub service_history: Vec<ServiceHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InstallComponentRequest {
    #[serde(default)]
    pub airframe_id: String,
    #[serde(default)]
    pub installed_at: String,
    #[serde(default)]
    pub actor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetComponentRecord {
    pub component_id: String,
    pub component_type: FleetComponentType,
    pub serial: String,
    pub airframe_id: Option<String>,
    pub installed_at: Option<String>,
    pub removed_at: Option<String>,
    pub service_history: Vec<ServiceHistoryEntry>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetComponentEventRecord {
    pub component_id: String,
    pub event_type: String,
    pub airframe_id: Option<String>,
    pub event_at: String,
    pub actor: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FleetHealthError {
    #[error("component_id cannot be empty")]
    EmptyComponentId,
    #[error("component serial cannot be empty")]
    EmptySerial,
    #[error("airframe_id cannot be empty")]
    EmptyAirframeId,
    #[error("installed_at cannot be empty")]
    EmptyInstalledAt,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("service_id cannot be empty")]
    EmptyServiceId,
    #[error("service performed_at cannot be empty")]
    EmptyServicePerformedAt,
    #[error("service technician cannot be empty")]
    EmptyServiceTechnician,
    #[error("service action cannot be empty")]
    EmptyServiceAction,
    #[error("unsupported fleet component type {value}")]
    UnsupportedComponentType { value: String },
    #[error("component {component_id} is already installed on airframe {airframe_id}")]
    AlreadyInstalled {
        component_id: String,
        airframe_id: String,
    },
}

pub fn build_component_record(
    request: RegisterComponentRequest,
    generated_component_id: String,
    created_at: String,
) -> Result<FleetComponentRecord, FleetHealthError> {
    let component_id = match normalize_optional_text(request.component_id) {
        Some(component_id) => component_id,
        None => {
            normalize_required_text(generated_component_id, FleetHealthError::EmptyComponentId)?
        }
    };
    let airframe_id = normalize_optional_text(request.airframe_id);
    let installed_at = normalize_optional_text(request.installed_at);
    if airframe_id.is_some() && installed_at.is_none() {
        return Err(FleetHealthError::EmptyInstalledAt);
    }
    if installed_at.is_some() && airframe_id.is_none() {
        return Err(FleetHealthError::EmptyAirframeId);
    }

    let service_history = request
        .service_history
        .into_iter()
        .map(normalize_service_history_entry)
        .collect::<Result<Vec<_>, _>>()?;
    let created_at = normalize_required_text(created_at, FleetHealthError::EmptyCreatedAt)?;

    Ok(FleetComponentRecord {
        component_id,
        component_type: request.component_type,
        serial: normalize_required_text(request.serial, FleetHealthError::EmptySerial)?,
        airframe_id,
        installed_at,
        removed_at: normalize_optional_text(request.removed_at),
        service_history,
        created_at: created_at.clone(),
        updated_at: created_at,
    })
}

pub fn install_component(
    component: &FleetComponentRecord,
    request: InstallComponentRequest,
    updated_at: String,
) -> Result<FleetComponentRecord, FleetHealthError> {
    let airframe_id =
        normalize_required_text(request.airframe_id, FleetHealthError::EmptyAirframeId)?;
    let installed_at =
        normalize_required_text(request.installed_at, FleetHealthError::EmptyInstalledAt)?;

    if component.removed_at.is_none() {
        if let Some(current_airframe) = &component.airframe_id {
            if current_airframe != &airframe_id {
                return Err(FleetHealthError::AlreadyInstalled {
                    component_id: component.component_id.clone(),
                    airframe_id: current_airframe.clone(),
                });
            }
        }
    }

    let mut updated = component.clone();
    updated.airframe_id = Some(airframe_id);
    updated.installed_at = Some(installed_at);
    updated.removed_at = None;
    updated.updated_at = normalize_required_text(updated_at, FleetHealthError::EmptyCreatedAt)?;
    Ok(updated)
}

pub fn component_event(
    component_id: &str,
    event_type: &str,
    airframe_id: Option<String>,
    event_at: String,
    actor: Option<String>,
    details: Option<String>,
) -> Result<FleetComponentEventRecord, FleetHealthError> {
    Ok(FleetComponentEventRecord {
        component_id: normalize_required_text(
            component_id.to_string(),
            FleetHealthError::EmptyComponentId,
        )?,
        event_type: normalize_required_text(
            event_type.to_string(),
            FleetHealthError::EmptyServiceAction,
        )?,
        airframe_id: normalize_optional_text(airframe_id),
        event_at: normalize_required_text(event_at, FleetHealthError::EmptyCreatedAt)?,
        actor: normalize_optional_text(actor),
        details: normalize_optional_text(details),
    })
}

fn normalize_service_history_entry(
    entry: ServiceHistoryEntry,
) -> Result<ServiceHistoryEntry, FleetHealthError> {
    Ok(ServiceHistoryEntry {
        service_id: normalize_required_text(entry.service_id, FleetHealthError::EmptyServiceId)?,
        performed_at: normalize_required_text(
            entry.performed_at,
            FleetHealthError::EmptyServicePerformedAt,
        )?,
        technician: normalize_required_text(
            entry.technician,
            FleetHealthError::EmptyServiceTechnician,
        )?,
        action: normalize_required_text(entry.action, FleetHealthError::EmptyServiceAction)?,
        notes: normalize_optional_text(entry.notes),
    })
}

fn normalize_required_text(
    value: String,
    error: FleetHealthError,
) -> Result<String, FleetHealthError> {
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
        build_component_record, component_event, install_component, FleetComponentType,
        FleetHealthError, InstallComponentRequest, RegisterComponentRequest, ServiceHistoryEntry,
    };

    #[test]
    fn component_record_normalizes_install_and_service_history() {
        let record = build_component_record(
            RegisterComponentRequest {
                component_id: Some(" battery-pack-001 ".to_string()),
                component_type: FleetComponentType::Battery,
                serial: " BAT-2026-001 ".to_string(),
                airframe_id: Some(" airframe-1 ".to_string()),
                installed_at: Some(" 2026-06-01T10:00:00Z ".to_string()),
                removed_at: None,
                service_history: vec![ServiceHistoryEntry {
                    service_id: " svc-001 ".to_string(),
                    performed_at: " 2026-06-01T09:30:00Z ".to_string(),
                    technician: " tech-1 ".to_string(),
                    action: " incoming_inspection ".to_string(),
                    notes: Some(" capacity check passed ".to_string()),
                }],
            },
            "generated-component".to_string(),
            " 2026-06-01T10:05:00Z ".to_string(),
        )
        .expect("component should be valid");

        assert_eq!(record.component_id, "battery-pack-001");
        assert_eq!(record.component_type, FleetComponentType::Battery);
        assert_eq!(record.serial, "BAT-2026-001");
        assert_eq!(record.airframe_id.as_deref(), Some("airframe-1"));
        assert_eq!(record.installed_at.as_deref(), Some("2026-06-01T10:00:00Z"));
        assert_eq!(record.service_history[0].service_id, "svc-001");
        assert_eq!(record.service_history[0].technician, "tech-1");
    }

    #[test]
    fn component_cannot_install_on_two_airframes_at_once() {
        let record = build_component_record(
            RegisterComponentRequest {
                component_id: Some("battery-pack-001".to_string()),
                component_type: FleetComponentType::Battery,
                serial: "BAT-2026-001".to_string(),
                airframe_id: Some("airframe-1".to_string()),
                installed_at: Some("2026-06-01T10:00:00Z".to_string()),
                removed_at: None,
                service_history: vec![],
            },
            "generated-component".to_string(),
            "2026-06-01T10:05:00Z".to_string(),
        )
        .expect("component should be valid");

        let error = install_component(
            &record,
            InstallComponentRequest {
                airframe_id: "airframe-2".to_string(),
                installed_at: "2026-06-02T10:00:00Z".to_string(),
                actor: Some("tech-2".to_string()),
            },
            "2026-06-02T10:00:00Z".to_string(),
        )
        .expect_err("double install should be rejected");

        assert_eq!(
            error,
            FleetHealthError::AlreadyInstalled {
                component_id: "battery-pack-001".to_string(),
                airframe_id: "airframe-1".to_string()
            }
        );
    }

    #[test]
    fn invalid_service_history_is_rejected() {
        let error = build_component_record(
            RegisterComponentRequest {
                component_id: Some("battery-pack-001".to_string()),
                component_type: FleetComponentType::Battery,
                serial: "BAT-2026-001".to_string(),
                airframe_id: None,
                installed_at: None,
                removed_at: None,
                service_history: vec![ServiceHistoryEntry {
                    service_id: "svc-001".to_string(),
                    performed_at: "2026-06-01T09:30:00Z".to_string(),
                    technician: "tech-1".to_string(),
                    action: " ".to_string(),
                    notes: None,
                }],
            },
            "generated-component".to_string(),
            "2026-06-01T10:05:00Z".to_string(),
        )
        .expect_err("empty service action should be rejected");

        assert_eq!(error, FleetHealthError::EmptyServiceAction);
    }

    #[test]
    fn component_events_are_normalized() {
        let event = component_event(
            " battery-pack-001 ",
            " installed ",
            Some(" airframe-1 ".to_string()),
            " 2026-06-01T10:00:00Z ".to_string(),
            Some(" tech-1 ".to_string()),
            Some(" initial install ".to_string()),
        )
        .expect("event should be valid");

        assert_eq!(event.component_id, "battery-pack-001");
        assert_eq!(event.event_type, "installed");
        assert_eq!(event.airframe_id.as_deref(), Some("airframe-1"));
        assert_eq!(event.actor.as_deref(), Some("tech-1"));
    }
}
