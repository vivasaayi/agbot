use serde::{Deserialize, Serialize};
use timeseries::{SeriesPoint, SeriesValue};

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

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DutyAccrualRequest {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub airframe_id: String,
    pub flight_hours: f64,
    #[serde(default)]
    pub cycles: u32,
    pub duty_score: f64,
    #[serde(default)]
    pub ended_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TelemetryHealthIndicatorRequest {
    #[serde(default)]
    pub source_ref: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub samples: Vec<HealthTelemetrySample>,
    #[serde(default)]
    pub telemetry_gaps: Vec<HealthTelemetryGap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthTelemetrySample {
    #[serde(default)]
    pub component_id: String,
    pub component_type: FleetComponentType,
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub battery_open_circuit_voltage_v: Option<f64>,
    #[serde(default)]
    pub battery_voltage_v: Option<f64>,
    #[serde(default)]
    pub battery_current_a: Option<f64>,
    #[serde(default)]
    pub motor_vibration_g: Option<f64>,
    #[serde(default)]
    pub esc_temperature_c: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthTelemetryGap {
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub ended_at: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetHealthIndicator {
    BatteryInternalResistance,
    MotorVibration,
    EscTemperature,
}

impl FleetHealthIndicator {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetHealthIndicator::BatteryInternalResistance => {
                "battery_internal_resistance_milliohm"
            }
            FleetHealthIndicator::MotorVibration => "motor_vibration_g",
            FleetHealthIndicator::EscTemperature => "esc_temperature_c",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            FleetHealthIndicator::BatteryInternalResistance => "milliohm",
            FleetHealthIndicator::MotorVibration => "g",
            FleetHealthIndicator::EscTemperature => "celsius",
        }
    }
}

impl std::str::FromStr for FleetHealthIndicator {
    type Err = FleetHealthError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "battery_internal_resistance_milliohm" | "battery_internal_resistance" => {
                Ok(Self::BatteryInternalResistance)
            }
            "motor_vibration_g" | "motor_vibration" => Ok(Self::MotorVibration),
            "esc_temperature_c" | "esc_temperature" => Ok(Self::EscTemperature),
            _ => Err(FleetHealthError::UnsupportedHealthIndicator {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIndicatorFreshness {
    Fresh,
    Stale,
}

impl HealthIndicatorFreshness {
    pub fn as_str(self) -> &'static str {
        match self {
            HealthIndicatorFreshness::Fresh => "fresh",
            HealthIndicatorFreshness::Stale => "stale",
        }
    }
}

impl std::str::FromStr for HealthIndicatorFreshness {
    type Err = FleetHealthError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fresh" => Ok(Self::Fresh),
            "stale" => Ok(Self::Stale),
            _ => Err(FleetHealthError::UnsupportedHealthFreshness {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetHealthIndicatorSample {
    pub component_id: String,
    pub indicator: FleetHealthIndicator,
    pub value: f64,
    pub ts: String,
    pub source_ref: String,
    pub created_at: String,
    pub freshness: HealthIndicatorFreshness,
}

impl FleetHealthIndicatorSample {
    pub fn to_series_point(&self) -> SeriesPoint {
        SeriesPoint {
            entity_ref: format!("component:{}", self.component_id),
            metric: self.indicator.as_str().to_string(),
            unit: self.indicator.unit().to_string(),
            t: self.ts.clone(),
            value: SeriesValue::Scalar { value: self.value },
            source_ref: self.source_ref.clone(),
            created_at: self.created_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetHealthIndicatorDerivation {
    pub samples: Vec<FleetHealthIndicatorSample>,
    pub gaps: Vec<HealthTelemetryGap>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentHealthVerdictStatus {
    Ok,
    Watch,
    Degraded,
    Critical,
}

impl ComponentHealthVerdictStatus {
    fn severity_rank(self) -> u8 {
        match self {
            ComponentHealthVerdictStatus::Ok => 0,
            ComponentHealthVerdictStatus::Watch => 1,
            ComponentHealthVerdictStatus::Degraded => 2,
            ComponentHealthVerdictStatus::Critical => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthVerdictReasonCode {
    AllIndicatorsWithinThreshold,
    WatchThresholdExceeded,
    DegradedThresholdExceeded,
    CriticalThresholdExceeded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthIndicatorThreshold {
    pub indicator: FleetHealthIndicator,
    pub watch_at: f64,
    pub degraded_at: f64,
    pub critical_at: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ComponentHealthVerdictRequest {
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub evaluated_at: String,
    #[serde(default)]
    pub method_version: String,
    #[serde(default)]
    pub samples: Vec<FleetHealthIndicatorSample>,
    #[serde(default)]
    pub thresholds: Vec<HealthIndicatorThreshold>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthVerdictEvidence {
    pub indicator: FleetHealthIndicator,
    pub value: f64,
    pub threshold: f64,
    pub status: ComponentHealthVerdictStatus,
    pub reason_code: HealthVerdictReasonCode,
    pub sample_ts: String,
    pub source_ref: String,
    pub freshness: HealthIndicatorFreshness,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentHealthVerdict {
    pub component_id: String,
    pub evaluated_at: String,
    pub method_version: String,
    pub status: ComponentHealthVerdictStatus,
    pub reason_code: HealthVerdictReasonCode,
    pub indicator: Option<FleetHealthIndicator>,
    pub threshold: Option<f64>,
    pub value: Option<f64>,
    pub freshness: HealthIndicatorFreshness,
    pub evidence: Vec<HealthVerdictEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentServiceLimit {
    #[serde(default)]
    pub component_id: String,
    #[serde(default)]
    pub max_flight_hours: Option<f64>,
    #[serde(default)]
    pub max_cycles: Option<u32>,
    #[serde(default)]
    pub max_duty_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FleetReadinessRequest {
    #[serde(default)]
    pub airframe_id: String,
    #[serde(default)]
    pub checked_at: String,
    #[serde(default)]
    pub installed_components: Vec<FleetComponentRecord>,
    #[serde(default)]
    pub service_limits: Vec<ComponentServiceLimit>,
    #[serde(default)]
    pub health_verdicts: Vec<ComponentHealthVerdict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetReadinessDecisionStatus {
    Permitted,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetReadinessBlockReason {
    MissingInstalledComponent,
    MissingServiceLimit,
    OverdueServiceHours,
    OverdueServiceCycles,
    OverdueDutyScore,
    MissingHealthData,
    StaleHealthData,
    CriticalHealthVerdict,
    BatteryHealthBelowThreshold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetReadinessBlocker {
    pub reason_code: FleetReadinessBlockReason,
    pub component_ref: Option<String>,
    pub indicator: Option<FleetHealthIndicator>,
    pub observed_value: Option<f64>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetReadinessDecision {
    pub airframe_id: String,
    pub checked_at: String,
    pub status: FleetReadinessDecisionStatus,
    pub blockers: Vec<FleetReadinessBlocker>,
    pub component_count: usize,
    pub verdict_count: usize,
}

impl FleetReadinessDecision {
    pub fn is_clear(&self) -> bool {
        self.status == FleetReadinessDecisionStatus::Permitted && self.blockers.is_empty()
    }
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
    pub flight_hours: f64,
    pub cycles: u32,
    pub duty_score: f64,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDutyAccrualRecord {
    pub session_id: String,
    pub component_id: String,
    pub airframe_id: String,
    pub flight_hours: f64,
    pub cycles: u32,
    pub duty_score: f64,
    pub accrued_at: String,
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
    #[error("session_id cannot be empty")]
    EmptySessionId,
    #[error("flight_hours must be finite and non-negative")]
    InvalidFlightHours,
    #[error("duty_score must be finite and non-negative")]
    InvalidDutyScore,
    #[error("ended_at cannot be empty")]
    EmptyEndedAt,
    #[error("source_ref cannot be empty")]
    EmptySourceRef,
    #[error("telemetry sample timestamp cannot be empty")]
    EmptyTelemetryTimestamp,
    #[error("telemetry sample set cannot be empty")]
    EmptyTelemetrySamples,
    #[error("health indicator sample set cannot be empty")]
    EmptyHealthIndicatorSamples,
    #[error("health threshold set cannot be empty")]
    EmptyHealthThresholds,
    #[error("health threshold method_version cannot be empty")]
    EmptyHealthMethodVersion,
    #[error("telemetry value must be finite")]
    InvalidTelemetryValue,
    #[error(
        "health threshold must be finite, non-negative, and ordered watch <= degraded <= critical"
    )]
    InvalidHealthThreshold { indicator: FleetHealthIndicator },
    #[error("missing health threshold for {indicator:?}")]
    MissingHealthThreshold { indicator: FleetHealthIndicator },
    #[error("indicator sample belongs to component {sample_component_id}, not requested component {component_id}")]
    IndicatorComponentMismatch {
        component_id: String,
        sample_component_id: String,
    },
    #[error("service limit must have a component_id and at least one finite non-negative limit")]
    InvalidServiceLimit { component_id: String },
    #[error("battery current must be finite and non-zero")]
    InvalidBatteryCurrent,
    #[error("telemetry gap timestamp cannot be empty")]
    EmptyTelemetryGapTimestamp,
    #[error("telemetry gap reason cannot be empty")]
    EmptyTelemetryGapReason,
    #[error("telemetry gap started_at must be at or before ended_at")]
    InvalidTelemetryGapRange,
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
    #[error("unsupported fleet health indicator {value}")]
    UnsupportedHealthIndicator { value: String },
    #[error("unsupported health indicator freshness {value}")]
    UnsupportedHealthFreshness { value: String },
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
        flight_hours: 0.0,
        cycles: 0,
        duty_score: 0.0,
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

pub fn build_component_duty_accruals(
    request: DutyAccrualRequest,
    component_ids: &[String],
) -> Result<Vec<ComponentDutyAccrualRecord>, FleetHealthError> {
    let session_id = normalize_required_text(request.session_id, FleetHealthError::EmptySessionId)?;
    let airframe_id =
        normalize_required_text(request.airframe_id, FleetHealthError::EmptyAirframeId)?;
    validate_nonnegative_finite(request.flight_hours, FleetHealthError::InvalidFlightHours)?;
    validate_nonnegative_finite(request.duty_score, FleetHealthError::InvalidDutyScore)?;
    let accrued_at = normalize_required_text(request.ended_at, FleetHealthError::EmptyEndedAt)?;

    component_ids
        .iter()
        .map(|component_id| {
            Ok(ComponentDutyAccrualRecord {
                session_id: session_id.clone(),
                component_id: normalize_required_text(
                    component_id.clone(),
                    FleetHealthError::EmptyComponentId,
                )?,
                airframe_id: airframe_id.clone(),
                flight_hours: request.flight_hours,
                cycles: request.cycles,
                duty_score: request.duty_score,
                accrued_at: accrued_at.clone(),
            })
        })
        .collect()
}

pub fn accrue_component_duty(
    component: &FleetComponentRecord,
    accrual: &ComponentDutyAccrualRecord,
    updated_at: String,
) -> Result<FleetComponentRecord, FleetHealthError> {
    let mut updated = component.clone();
    updated.flight_hours += accrual.flight_hours;
    updated.cycles += accrual.cycles;
    updated.duty_score += accrual.duty_score;
    updated.updated_at = normalize_required_text(updated_at, FleetHealthError::EmptyCreatedAt)?;
    Ok(updated)
}

pub fn derive_health_indicators(
    request: TelemetryHealthIndicatorRequest,
) -> Result<FleetHealthIndicatorDerivation, FleetHealthError> {
    let source_ref = normalize_required_text(request.source_ref, FleetHealthError::EmptySourceRef)?;
    let created_at = normalize_required_text(request.created_at, FleetHealthError::EmptyCreatedAt)?;
    if request.samples.is_empty() {
        return Err(FleetHealthError::EmptyTelemetrySamples);
    }
    let gaps = request
        .telemetry_gaps
        .into_iter()
        .map(normalize_health_telemetry_gap)
        .collect::<Result<Vec<_>, _>>()?;
    let mut samples = Vec::new();

    for sample in request.samples {
        let sample = normalize_health_telemetry_sample(sample)?;
        let freshness = if has_later_gap(&gaps, &sample.component_id, &sample.ts) {
            HealthIndicatorFreshness::Stale
        } else {
            HealthIndicatorFreshness::Fresh
        };

        match sample.component_type {
            FleetComponentType::Battery => {
                if let (Some(open_circuit), Some(loaded), Some(current)) = (
                    sample.battery_open_circuit_voltage_v,
                    sample.battery_voltage_v,
                    sample.battery_current_a,
                ) {
                    validate_finite(open_circuit)?;
                    validate_finite(loaded)?;
                    validate_finite(current)?;
                    if current.abs() <= f64::EPSILON {
                        return Err(FleetHealthError::InvalidBatteryCurrent);
                    }
                    samples.push(FleetHealthIndicatorSample {
                        component_id: sample.component_id,
                        indicator: FleetHealthIndicator::BatteryInternalResistance,
                        value: ((open_circuit - loaded).abs() / current.abs()) * 1000.0,
                        ts: sample.ts,
                        source_ref: source_ref.clone(),
                        created_at: created_at.clone(),
                        freshness,
                    });
                }
            }
            FleetComponentType::Motor => {
                if let Some(value) = sample.motor_vibration_g {
                    validate_finite(value)?;
                    samples.push(FleetHealthIndicatorSample {
                        component_id: sample.component_id,
                        indicator: FleetHealthIndicator::MotorVibration,
                        value,
                        ts: sample.ts,
                        source_ref: source_ref.clone(),
                        created_at: created_at.clone(),
                        freshness,
                    });
                }
            }
            FleetComponentType::Esc => {
                if let Some(value) = sample.esc_temperature_c {
                    validate_finite(value)?;
                    samples.push(FleetHealthIndicatorSample {
                        component_id: sample.component_id,
                        indicator: FleetHealthIndicator::EscTemperature,
                        value,
                        ts: sample.ts,
                        source_ref: source_ref.clone(),
                        created_at: created_at.clone(),
                        freshness,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(FleetHealthIndicatorDerivation { samples, gaps })
}

pub fn evaluate_component_health_verdict(
    request: ComponentHealthVerdictRequest,
) -> Result<ComponentHealthVerdict, FleetHealthError> {
    let component_id =
        normalize_required_text(request.component_id, FleetHealthError::EmptyComponentId)?;
    let evaluated_at =
        normalize_required_text(request.evaluated_at, FleetHealthError::EmptyCreatedAt)?;
    let method_version = normalize_required_text(
        request.method_version,
        FleetHealthError::EmptyHealthMethodVersion,
    )?;
    if request.samples.is_empty() {
        return Err(FleetHealthError::EmptyHealthIndicatorSamples);
    }
    if request.thresholds.is_empty() {
        return Err(FleetHealthError::EmptyHealthThresholds);
    }

    let thresholds = request
        .thresholds
        .into_iter()
        .map(normalize_health_threshold)
        .collect::<Result<Vec<_>, _>>()?;
    let mut evidence = Vec::new();

    for sample in request.samples {
        let sample_component_id =
            normalize_required_text(sample.component_id, FleetHealthError::EmptyComponentId)?;
        if sample_component_id != component_id {
            return Err(FleetHealthError::IndicatorComponentMismatch {
                component_id,
                sample_component_id,
            });
        }
        validate_finite(sample.value)?;
        let sample_ts =
            normalize_required_text(sample.ts, FleetHealthError::EmptyTelemetryTimestamp)?;
        let source_ref =
            normalize_required_text(sample.source_ref, FleetHealthError::EmptySourceRef)?;
        let threshold = thresholds
            .iter()
            .find(|threshold| threshold.indicator == sample.indicator)
            .ok_or(FleetHealthError::MissingHealthThreshold {
                indicator: sample.indicator,
            })?;
        let (status, reason_code, threshold_value) =
            classify_health_indicator(sample.value, threshold);

        evidence.push(HealthVerdictEvidence {
            indicator: sample.indicator,
            value: sample.value,
            threshold: threshold_value,
            status,
            reason_code,
            sample_ts,
            source_ref,
            freshness: sample.freshness,
        });
    }

    let selected = evidence
        .iter()
        .max_by(|left, right| compare_verdict_evidence(left, right))
        .expect("non-empty evidence checked above");
    let freshness = if evidence
        .iter()
        .any(|item| item.freshness == HealthIndicatorFreshness::Stale)
    {
        HealthIndicatorFreshness::Stale
    } else {
        HealthIndicatorFreshness::Fresh
    };

    Ok(ComponentHealthVerdict {
        component_id,
        evaluated_at,
        method_version,
        status: selected.status,
        reason_code: selected.reason_code,
        indicator: Some(selected.indicator),
        threshold: Some(selected.threshold),
        value: Some(selected.value),
        freshness,
        evidence,
    })
}

pub fn evaluate_fleet_readiness(
    request: FleetReadinessRequest,
) -> Result<FleetReadinessDecision, FleetHealthError> {
    let airframe_id =
        normalize_required_text(request.airframe_id, FleetHealthError::EmptyAirframeId)?;
    let checked_at = normalize_required_text(request.checked_at, FleetHealthError::EmptyCreatedAt)?;
    let service_limits = request
        .service_limits
        .into_iter()
        .map(normalize_component_service_limit)
        .collect::<Result<Vec<_>, _>>()?;
    let installed_components = request
        .installed_components
        .into_iter()
        .filter(|component| {
            component.airframe_id.as_deref() == Some(airframe_id.as_str())
                && component.removed_at.is_none()
        })
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();

    if installed_components.is_empty() {
        blockers.push(readiness_blocker(
            FleetReadinessBlockReason::MissingInstalledComponent,
            None,
            None,
            None,
            None,
        ));
    }

    for component in &installed_components {
        match service_limits
            .iter()
            .find(|limit| limit.component_id == component.component_id)
        {
            Some(limit) => append_service_limit_blockers(component, limit, &mut blockers),
            None => blockers.push(readiness_blocker(
                FleetReadinessBlockReason::MissingServiceLimit,
                Some(component.component_id.clone()),
                None,
                None,
                None,
            )),
        }

        let Some(verdict) = request
            .health_verdicts
            .iter()
            .find(|verdict| verdict.component_id.trim() == component.component_id)
        else {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::MissingHealthData,
                Some(component.component_id.clone()),
                None,
                None,
                None,
            ));
            continue;
        };

        if verdict.freshness == HealthIndicatorFreshness::Stale {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::StaleHealthData,
                Some(component.component_id.clone()),
                verdict.indicator,
                verdict.value,
                verdict.threshold,
            ));
            continue;
        }

        if verdict.status == ComponentHealthVerdictStatus::Critical {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::CriticalHealthVerdict,
                Some(component.component_id.clone()),
                verdict.indicator,
                verdict.value,
                verdict.threshold,
            ));
        } else if component.component_type == FleetComponentType::Battery
            && verdict.status == ComponentHealthVerdictStatus::Degraded
        {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::BatteryHealthBelowThreshold,
                Some(component.component_id.clone()),
                verdict.indicator,
                verdict.value,
                verdict.threshold,
            ));
        }
    }

    let status = if blockers.is_empty() {
        FleetReadinessDecisionStatus::Permitted
    } else {
        FleetReadinessDecisionStatus::Blocked
    };

    Ok(FleetReadinessDecision {
        airframe_id,
        checked_at,
        status,
        blockers,
        component_count: installed_components.len(),
        verdict_count: request.health_verdicts.len(),
    })
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

fn normalize_health_telemetry_sample(
    sample: HealthTelemetrySample,
) -> Result<HealthTelemetrySample, FleetHealthError> {
    Ok(HealthTelemetrySample {
        component_id: normalize_required_text(
            sample.component_id,
            FleetHealthError::EmptyComponentId,
        )?,
        component_type: sample.component_type,
        ts: normalize_required_text(sample.ts, FleetHealthError::EmptyTelemetryTimestamp)?,
        battery_open_circuit_voltage_v: sample.battery_open_circuit_voltage_v,
        battery_voltage_v: sample.battery_voltage_v,
        battery_current_a: sample.battery_current_a,
        motor_vibration_g: sample.motor_vibration_g,
        esc_temperature_c: sample.esc_temperature_c,
    })
}

fn normalize_health_telemetry_gap(
    gap: HealthTelemetryGap,
) -> Result<HealthTelemetryGap, FleetHealthError> {
    let component_id =
        normalize_required_text(gap.component_id, FleetHealthError::EmptyComponentId)?;
    let started_at =
        normalize_required_text(gap.started_at, FleetHealthError::EmptyTelemetryGapTimestamp)?;
    let ended_at =
        normalize_required_text(gap.ended_at, FleetHealthError::EmptyTelemetryGapTimestamp)?;
    if started_at > ended_at {
        return Err(FleetHealthError::InvalidTelemetryGapRange);
    }
    Ok(HealthTelemetryGap {
        component_id,
        started_at,
        ended_at,
        reason: normalize_required_text(gap.reason, FleetHealthError::EmptyTelemetryGapReason)?,
    })
}

fn has_later_gap(gaps: &[HealthTelemetryGap], component_id: &str, sample_ts: &str) -> bool {
    gaps.iter()
        .any(|gap| gap.component_id == component_id && gap.started_at.as_str() > sample_ts)
}

fn validate_finite(value: f64) -> Result<(), FleetHealthError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(FleetHealthError::InvalidTelemetryValue)
    }
}

fn normalize_health_threshold(
    threshold: HealthIndicatorThreshold,
) -> Result<HealthIndicatorThreshold, FleetHealthError> {
    let valid = threshold.watch_at.is_finite()
        && threshold.degraded_at.is_finite()
        && threshold.critical_at.is_finite()
        && threshold.watch_at >= 0.0
        && threshold.degraded_at >= threshold.watch_at
        && threshold.critical_at >= threshold.degraded_at;

    if valid {
        Ok(threshold)
    } else {
        Err(FleetHealthError::InvalidHealthThreshold {
            indicator: threshold.indicator,
        })
    }
}

fn classify_health_indicator(
    value: f64,
    threshold: &HealthIndicatorThreshold,
) -> (ComponentHealthVerdictStatus, HealthVerdictReasonCode, f64) {
    if value >= threshold.critical_at {
        (
            ComponentHealthVerdictStatus::Critical,
            HealthVerdictReasonCode::CriticalThresholdExceeded,
            threshold.critical_at,
        )
    } else if value >= threshold.degraded_at {
        (
            ComponentHealthVerdictStatus::Degraded,
            HealthVerdictReasonCode::DegradedThresholdExceeded,
            threshold.degraded_at,
        )
    } else if value >= threshold.watch_at {
        (
            ComponentHealthVerdictStatus::Watch,
            HealthVerdictReasonCode::WatchThresholdExceeded,
            threshold.watch_at,
        )
    } else {
        (
            ComponentHealthVerdictStatus::Ok,
            HealthVerdictReasonCode::AllIndicatorsWithinThreshold,
            threshold.watch_at,
        )
    }
}

fn compare_verdict_evidence(
    left: &HealthVerdictEvidence,
    right: &HealthVerdictEvidence,
) -> std::cmp::Ordering {
    left.status
        .severity_rank()
        .cmp(&right.status.severity_rank())
        .then_with(|| {
            left.value
                .partial_cmp(&right.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn normalize_component_service_limit(
    limit: ComponentServiceLimit,
) -> Result<ComponentServiceLimit, FleetHealthError> {
    let component_id =
        normalize_required_text(limit.component_id, FleetHealthError::EmptyComponentId)?;
    let has_limit = limit.max_flight_hours.is_some()
        || limit.max_cycles.is_some()
        || limit.max_duty_score.is_some();
    let valid = has_limit
        && optional_nonnegative_finite(limit.max_flight_hours)
        && optional_nonnegative_finite(limit.max_duty_score);

    if valid {
        Ok(ComponentServiceLimit {
            component_id,
            max_flight_hours: limit.max_flight_hours,
            max_cycles: limit.max_cycles,
            max_duty_score: limit.max_duty_score,
        })
    } else {
        Err(FleetHealthError::InvalidServiceLimit { component_id })
    }
}

fn optional_nonnegative_finite(value: Option<f64>) -> bool {
    value.is_none_or(|value| value.is_finite() && value >= 0.0)
}

fn append_service_limit_blockers(
    component: &FleetComponentRecord,
    limit: &ComponentServiceLimit,
    blockers: &mut Vec<FleetReadinessBlocker>,
) {
    if let Some(max_flight_hours) = limit.max_flight_hours {
        if component.flight_hours > max_flight_hours {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::OverdueServiceHours,
                Some(component.component_id.clone()),
                None,
                Some(component.flight_hours),
                Some(max_flight_hours),
            ));
        }
    }

    if let Some(max_cycles) = limit.max_cycles {
        if component.cycles > max_cycles {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::OverdueServiceCycles,
                Some(component.component_id.clone()),
                None,
                Some(component.cycles as f64),
                Some(max_cycles as f64),
            ));
        }
    }

    if let Some(max_duty_score) = limit.max_duty_score {
        if component.duty_score > max_duty_score {
            blockers.push(readiness_blocker(
                FleetReadinessBlockReason::OverdueDutyScore,
                Some(component.component_id.clone()),
                None,
                Some(component.duty_score),
                Some(max_duty_score),
            ));
        }
    }
}

fn readiness_blocker(
    reason_code: FleetReadinessBlockReason,
    component_ref: Option<String>,
    indicator: Option<FleetHealthIndicator>,
    observed_value: Option<f64>,
    threshold: Option<f64>,
) -> FleetReadinessBlocker {
    FleetReadinessBlocker {
        reason_code,
        component_ref,
        indicator,
        observed_value,
        threshold,
    }
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

fn validate_nonnegative_finite(
    value: f64,
    error: FleetHealthError,
) -> Result<(), FleetHealthError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        accrue_component_duty, build_component_duty_accruals, build_component_record,
        component_event, derive_health_indicators, evaluate_component_health_verdict,
        evaluate_fleet_readiness, install_component, ComponentHealthVerdict,
        ComponentHealthVerdictRequest, ComponentHealthVerdictStatus, ComponentServiceLimit,
        DutyAccrualRequest, FleetComponentRecord, FleetComponentType, FleetHealthError,
        FleetHealthIndicator, FleetReadinessBlockReason, FleetReadinessDecisionStatus,
        FleetReadinessRequest, HealthIndicatorFreshness, HealthIndicatorThreshold,
        HealthTelemetryGap, HealthTelemetrySample, HealthVerdictEvidence, HealthVerdictReasonCode,
        InstallComponentRequest, RegisterComponentRequest, ServiceHistoryEntry,
        TelemetryHealthIndicatorRequest,
    };
    use timeseries::SeriesValue;

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

    #[test]
    fn duty_accrual_builds_per_component_records_and_updates_totals() {
        let component = build_component_record(
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

        let accruals = build_component_duty_accruals(
            DutyAccrualRequest {
                session_id: " session-001 ".to_string(),
                airframe_id: " airframe-1 ".to_string(),
                flight_hours: 1.25,
                cycles: 1,
                duty_score: 0.8,
                ended_at: " 2026-06-03T12:15:00Z ".to_string(),
            },
            &[component.component_id.clone()],
        )
        .expect("accrual should be valid");

        assert_eq!(accruals.len(), 1);
        assert_eq!(accruals[0].session_id, "session-001");
        assert_eq!(accruals[0].component_id, "battery-pack-001");

        let updated =
            accrue_component_duty(&component, &accruals[0], "2026-06-03T12:15:00Z".to_string())
                .expect("totals should update");
        assert_eq!(updated.flight_hours, 1.25);
        assert_eq!(updated.cycles, 1);
        assert_eq!(updated.duty_score, 0.8);
    }

    #[test]
    fn duty_accrual_rejects_invalid_hours() {
        let error = build_component_duty_accruals(
            DutyAccrualRequest {
                session_id: "session-001".to_string(),
                airframe_id: "airframe-1".to_string(),
                flight_hours: -1.0,
                cycles: 1,
                duty_score: 0.8,
                ended_at: "2026-06-03T12:15:00Z".to_string(),
            },
            &["battery-pack-001".to_string()],
        )
        .expect_err("negative hours should be rejected");

        assert_eq!(error, FleetHealthError::InvalidFlightHours);
    }

    #[test]
    fn telemetry_health_indicators_derive_scalar_series_points() {
        let derived = derive_health_indicators(TelemetryHealthIndicatorRequest {
            source_ref: "telemetry:session-001".to_string(),
            created_at: "2026-06-12T12:20:00Z".to_string(),
            samples: vec![
                HealthTelemetrySample {
                    component_id: "battery-pack-001".to_string(),
                    component_type: FleetComponentType::Battery,
                    ts: "2026-06-12T12:00:00Z".to_string(),
                    battery_open_circuit_voltage_v: Some(16.8),
                    battery_voltage_v: Some(15.96),
                    battery_current_a: Some(28.0),
                    motor_vibration_g: None,
                    esc_temperature_c: None,
                },
                HealthTelemetrySample {
                    component_id: "motor-front-left".to_string(),
                    component_type: FleetComponentType::Motor,
                    ts: "2026-06-12T12:00:00Z".to_string(),
                    battery_open_circuit_voltage_v: None,
                    battery_voltage_v: None,
                    battery_current_a: None,
                    motor_vibration_g: Some(0.42),
                    esc_temperature_c: None,
                },
                HealthTelemetrySample {
                    component_id: "esc-front-left".to_string(),
                    component_type: FleetComponentType::Esc,
                    ts: "2026-06-12T12:00:00Z".to_string(),
                    battery_open_circuit_voltage_v: None,
                    battery_voltage_v: None,
                    battery_current_a: None,
                    motor_vibration_g: None,
                    esc_temperature_c: Some(54.5),
                },
            ],
            telemetry_gaps: vec![],
        })
        .expect("health indicators should derive");

        assert_eq!(derived.samples.len(), 3);
        let resistance = derived
            .samples
            .iter()
            .find(|sample| sample.indicator == FleetHealthIndicator::BatteryInternalResistance)
            .expect("resistance sample should exist");
        assert_eq!(resistance.component_id, "battery-pack-001");
        assert!((resistance.value - 30.0).abs() < 1e-9);
        assert_eq!(resistance.freshness, HealthIndicatorFreshness::Fresh);

        let point = resistance.to_series_point();
        assert_eq!(point.entity_ref, "component:battery-pack-001");
        assert_eq!(point.metric, "battery_internal_resistance_milliohm");
        assert_eq!(point.unit, "milliohm");
        assert_eq!(point.t, "2026-06-12T12:00:00Z");
        match point.value {
            SeriesValue::Scalar { value } => assert!((value - 30.0).abs() < 1e-9),
            SeriesValue::Raster(_) => panic!("health indicator should be scalar"),
        }
    }

    #[test]
    fn telemetry_dropout_records_gap_and_marks_last_indicator_stale_without_backfill() {
        let derived = derive_health_indicators(TelemetryHealthIndicatorRequest {
            source_ref: "telemetry:session-002".to_string(),
            created_at: "2026-06-12T12:20:00Z".to_string(),
            samples: vec![HealthTelemetrySample {
                component_id: "battery-pack-001".to_string(),
                component_type: FleetComponentType::Battery,
                ts: "2026-06-12T12:00:00Z".to_string(),
                battery_open_circuit_voltage_v: Some(16.8),
                battery_voltage_v: Some(16.24),
                battery_current_a: Some(28.0),
                motor_vibration_g: None,
                esc_temperature_c: None,
            }],
            telemetry_gaps: vec![HealthTelemetryGap {
                component_id: "battery-pack-001".to_string(),
                started_at: "2026-06-12T12:01:00Z".to_string(),
                ended_at: "2026-06-12T12:05:00Z".to_string(),
                reason: "mavlink-radio-dropout".to_string(),
            }],
        })
        .expect("health indicators should derive with gap");

        assert_eq!(derived.gaps.len(), 1);
        assert_eq!(derived.gaps[0].reason, "mavlink-radio-dropout");
        assert_eq!(derived.samples.len(), 1);
        assert_eq!(
            derived.samples[0].freshness,
            HealthIndicatorFreshness::Stale
        );
        assert_ne!(derived.samples[0].ts, "2026-06-12T12:01:00Z");
    }

    #[test]
    fn component_verdict_is_ok_when_indicators_are_within_thresholds() {
        let verdict = evaluate_component_health_verdict(ComponentHealthVerdictRequest {
            component_id: "battery-pack-001".to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            samples: vec![indicator_sample(
                "battery-pack-001",
                FleetHealthIndicator::BatteryInternalResistance,
                31.0,
            )],
            thresholds: vec![threshold(
                FleetHealthIndicator::BatteryInternalResistance,
                60.0,
                85.0,
                110.0,
            )],
        })
        .expect("verdict should evaluate");

        assert_eq!(verdict.status, ComponentHealthVerdictStatus::Ok);
        assert_eq!(
            verdict.reason_code,
            HealthVerdictReasonCode::AllIndicatorsWithinThreshold
        );
        assert_eq!(
            verdict.indicator,
            Some(FleetHealthIndicator::BatteryInternalResistance)
        );
        assert_eq!(verdict.threshold, Some(60.0));
        assert_eq!(verdict.value, Some(31.0));
        assert_eq!(verdict.evidence.len(), 1);
    }

    #[test]
    fn critical_indicator_sets_component_verdict_with_threshold_evidence() {
        let verdict = evaluate_component_health_verdict(ComponentHealthVerdictRequest {
            component_id: "motor-front-left".to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            samples: vec![
                indicator_sample(
                    "motor-front-left",
                    FleetHealthIndicator::MotorVibration,
                    1.8,
                ),
                indicator_sample(
                    "motor-front-left",
                    FleetHealthIndicator::EscTemperature,
                    72.0,
                ),
            ],
            thresholds: vec![
                threshold(FleetHealthIndicator::MotorVibration, 0.6, 1.0, 1.5),
                threshold(FleetHealthIndicator::EscTemperature, 70.0, 85.0, 100.0),
            ],
        })
        .expect("verdict should evaluate");

        assert_eq!(verdict.status, ComponentHealthVerdictStatus::Critical);
        assert_eq!(
            verdict.reason_code,
            HealthVerdictReasonCode::CriticalThresholdExceeded
        );
        assert_eq!(
            verdict.indicator,
            Some(FleetHealthIndicator::MotorVibration)
        );
        assert_eq!(verdict.threshold, Some(1.5));
        assert_eq!(verdict.value, Some(1.8));
        assert_eq!(verdict.evidence[0].source_ref, "telemetry:session-001");
    }

    #[test]
    fn verdict_refuses_indicator_without_configured_threshold() {
        let error = evaluate_component_health_verdict(ComponentHealthVerdictRequest {
            component_id: "motor-front-left".to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            samples: vec![indicator_sample(
                "motor-front-left",
                FleetHealthIndicator::MotorVibration,
                0.7,
            )],
            thresholds: vec![threshold(
                FleetHealthIndicator::BatteryInternalResistance,
                60.0,
                85.0,
                110.0,
            )],
        })
        .expect_err("missing threshold should be rejected");

        assert_eq!(
            error,
            FleetHealthError::MissingHealthThreshold {
                indicator: FleetHealthIndicator::MotorVibration
            }
        );
    }

    #[test]
    fn readiness_allows_airframe_with_fresh_verdicts_and_service_in_limits() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![
                component_for_readiness(
                    "battery-pack-001",
                    FleetComponentType::Battery,
                    15.0,
                    20,
                    10.0,
                ),
                component_for_readiness(
                    "motor-front-left",
                    FleetComponentType::Motor,
                    15.0,
                    20,
                    10.0,
                ),
            ],
            vec![
                service_limit("battery-pack-001", Some(100.0), Some(200), Some(100.0)),
                service_limit("motor-front-left", Some(100.0), Some(200), Some(100.0)),
            ],
            vec![
                component_verdict(
                    "battery-pack-001",
                    ComponentHealthVerdictStatus::Ok,
                    HealthIndicatorFreshness::Fresh,
                ),
                component_verdict(
                    "motor-front-left",
                    ComponentHealthVerdictStatus::Watch,
                    HealthIndicatorFreshness::Fresh,
                ),
            ],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Permitted);
        assert!(decision.blockers.is_empty());
        assert_eq!(decision.component_count, 2);
    }

    #[test]
    fn readiness_hard_blocks_overdue_service_interval() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "battery-pack-001",
                FleetComponentType::Battery,
                121.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "battery-pack-001",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "battery-pack-001",
                ComponentHealthVerdictStatus::Ok,
                HealthIndicatorFreshness::Fresh,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            decision.blockers[0].reason_code,
            FleetReadinessBlockReason::OverdueServiceHours
        );
        assert_eq!(
            decision.blockers[0].component_ref.as_deref(),
            Some("battery-pack-001")
        );
        assert_eq!(decision.blockers[0].observed_value, Some(121.0));
        assert_eq!(decision.blockers[0].threshold, Some(100.0));
    }

    #[test]
    fn readiness_hard_blocks_active_critical_health_verdict() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "motor-front-left",
                FleetComponentType::Motor,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "motor-front-left",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "motor-front-left",
                ComponentHealthVerdictStatus::Critical,
                HealthIndicatorFreshness::Fresh,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            decision.blockers[0].reason_code,
            FleetReadinessBlockReason::CriticalHealthVerdict
        );
        assert_eq!(
            decision.blockers[0].component_ref.as_deref(),
            Some("motor-front-left")
        );
    }

    #[test]
    fn readiness_hard_blocks_degraded_battery_health() {
        let decision = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "battery-pack-001",
                FleetComponentType::Battery,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "battery-pack-001",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "battery-pack-001",
                ComponentHealthVerdictStatus::Degraded,
                HealthIndicatorFreshness::Fresh,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(decision.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            decision.blockers[0].reason_code,
            FleetReadinessBlockReason::BatteryHealthBelowThreshold
        );
        assert_eq!(
            decision.blockers[0].component_ref.as_deref(),
            Some("battery-pack-001")
        );
    }

    #[test]
    fn readiness_denies_missing_or_stale_health_data_by_default() {
        let stale = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "battery-pack-001",
                FleetComponentType::Battery,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "battery-pack-001",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![component_verdict(
                "battery-pack-001",
                ComponentHealthVerdictStatus::Ok,
                HealthIndicatorFreshness::Stale,
            )],
        ))
        .expect("readiness should evaluate");

        assert_eq!(stale.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            stale.blockers[0].reason_code,
            FleetReadinessBlockReason::StaleHealthData
        );

        let missing = evaluate_fleet_readiness(readiness_request(
            vec![component_for_readiness(
                "motor-front-left",
                FleetComponentType::Motor,
                15.0,
                20,
                10.0,
            )],
            vec![service_limit(
                "motor-front-left",
                Some(100.0),
                Some(200),
                Some(100.0),
            )],
            vec![],
        ))
        .expect("readiness should evaluate");

        assert_eq!(missing.status, FleetReadinessDecisionStatus::Blocked);
        assert_eq!(
            missing.blockers[0].reason_code,
            FleetReadinessBlockReason::MissingHealthData
        );
    }

    fn indicator_sample(
        component_id: &str,
        indicator: FleetHealthIndicator,
        value: f64,
    ) -> super::FleetHealthIndicatorSample {
        super::FleetHealthIndicatorSample {
            component_id: component_id.to_string(),
            indicator,
            value,
            ts: "2026-06-12T12:00:00Z".to_string(),
            source_ref: "telemetry:session-001".to_string(),
            created_at: "2026-06-12T12:20:00Z".to_string(),
            freshness: HealthIndicatorFreshness::Fresh,
        }
    }

    fn threshold(
        indicator: FleetHealthIndicator,
        watch_at: f64,
        degraded_at: f64,
        critical_at: f64,
    ) -> HealthIndicatorThreshold {
        HealthIndicatorThreshold {
            indicator,
            watch_at,
            degraded_at,
            critical_at,
        }
    }

    fn readiness_request(
        installed_components: Vec<FleetComponentRecord>,
        service_limits: Vec<ComponentServiceLimit>,
        health_verdicts: Vec<ComponentHealthVerdict>,
    ) -> FleetReadinessRequest {
        FleetReadinessRequest {
            airframe_id: "airframe-1".to_string(),
            checked_at: "2026-06-12T12:45:00Z".to_string(),
            installed_components,
            service_limits,
            health_verdicts,
        }
    }

    fn component_for_readiness(
        component_id: &str,
        component_type: FleetComponentType,
        flight_hours: f64,
        cycles: u32,
        duty_score: f64,
    ) -> FleetComponentRecord {
        FleetComponentRecord {
            component_id: component_id.to_string(),
            component_type,
            serial: format!("{component_id}-serial"),
            airframe_id: Some("airframe-1".to_string()),
            installed_at: Some("2026-06-01T10:00:00Z".to_string()),
            removed_at: None,
            service_history: vec![],
            flight_hours,
            cycles,
            duty_score,
            created_at: "2026-06-01T10:05:00Z".to_string(),
            updated_at: "2026-06-12T12:00:00Z".to_string(),
        }
    }

    fn service_limit(
        component_id: &str,
        max_flight_hours: Option<f64>,
        max_cycles: Option<u32>,
        max_duty_score: Option<f64>,
    ) -> ComponentServiceLimit {
        ComponentServiceLimit {
            component_id: component_id.to_string(),
            max_flight_hours,
            max_cycles,
            max_duty_score,
        }
    }

    fn component_verdict(
        component_id: &str,
        status: ComponentHealthVerdictStatus,
        freshness: HealthIndicatorFreshness,
    ) -> ComponentHealthVerdict {
        let (reason_code, value, threshold) = match status {
            ComponentHealthVerdictStatus::Ok => (
                HealthVerdictReasonCode::AllIndicatorsWithinThreshold,
                30.0,
                60.0,
            ),
            ComponentHealthVerdictStatus::Watch => {
                (HealthVerdictReasonCode::WatchThresholdExceeded, 0.7, 0.6)
            }
            ComponentHealthVerdictStatus::Degraded => {
                (HealthVerdictReasonCode::DegradedThresholdExceeded, 1.2, 1.0)
            }
            ComponentHealthVerdictStatus::Critical => {
                (HealthVerdictReasonCode::CriticalThresholdExceeded, 1.8, 1.5)
            }
        };

        ComponentHealthVerdict {
            component_id: component_id.to_string(),
            evaluated_at: "2026-06-12T12:30:00Z".to_string(),
            method_version: "fleet-health-thresholds-v1".to_string(),
            status,
            reason_code,
            indicator: Some(FleetHealthIndicator::MotorVibration),
            threshold: Some(threshold),
            value: Some(value),
            freshness,
            evidence: vec![HealthVerdictEvidence {
                indicator: FleetHealthIndicator::MotorVibration,
                value,
                threshold,
                status,
                reason_code,
                sample_ts: "2026-06-12T12:00:00Z".to_string(),
                source_ref: "telemetry:session-001".to_string(),
                freshness,
            }],
        }
    }
}
