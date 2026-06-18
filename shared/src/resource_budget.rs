use crate::fleet_alerts::{
    FleetAlertComparator, FleetAlertEvidence, FleetAlertKind, FleetAlertRecord, FleetAlertRoute,
    FleetAlertSeverity,
};
use crate::observability::{FleetMetricSample, ObservabilityContext, ObservabilityError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeResourceKind {
    Cpu,
    Memory,
    Disk,
}

impl EdgeResourceKind {
    fn as_str(self) -> &'static str {
        match self {
            EdgeResourceKind::Cpu => "cpu",
            EdgeResourceKind::Memory => "memory",
            EdgeResourceKind::Disk => "disk",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeResourceAction {
    Admit,
    Throttle,
    Shed,
    BackpressureWrites,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeResourceBudget {
    pub node_id: String,
    pub max_cpu_percent: f64,
    pub max_memory_mb: u64,
    pub min_disk_free_gb: f64,
    pub throttle_at_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeResourceSnapshot {
    pub node_id: String,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub disk_free_gb: f64,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeResourceControl {
    pub resource: EdgeResourceKind,
    pub action: EdgeResourceAction,
    pub metric_name: String,
    pub observed_value: f64,
    pub threshold_value: f64,
    pub severity: Option<FleetAlertSeverity>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeResourceEvaluation {
    pub node_id: String,
    pub at: DateTime<Utc>,
    pub controls: Vec<EdgeResourceControl>,
    pub metrics: Vec<FleetMetricSample>,
    pub alerts: Vec<FleetAlertRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeThermalEnergyKind {
    Thermal,
    Energy,
}

impl EdgeThermalEnergyKind {
    fn as_str(self) -> &'static str {
        match self {
            EdgeThermalEnergyKind::Thermal => "thermal",
            EdgeThermalEnergyKind::Energy => "energy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeThermalEnergyBudget {
    pub node_id: String,
    pub max_temperature_c: f64,
    pub min_battery_percent: f64,
    pub throttle_at_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeThermalEnergySnapshot {
    pub node_id: String,
    #[serde(default)]
    pub temperature_c: Option<f64>,
    #[serde(default)]
    pub battery_remaining_percent: Option<f64>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeThermalEnergyControl {
    pub kind: EdgeThermalEnergyKind,
    pub action: EdgeResourceAction,
    pub metric_name: String,
    pub observed_value: Option<f64>,
    pub threshold_value: Option<f64>,
    pub severity: Option<FleetAlertSeverity>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeThermalEnergyHeartbeatState {
    pub thermal_state: EdgeResourceAction,
    pub energy_state: EdgeResourceAction,
    pub thermal_available: bool,
    pub energy_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeThermalEnergyEvaluation {
    pub node_id: String,
    pub at: DateTime<Utc>,
    pub controls: Vec<EdgeThermalEnergyControl>,
    pub heartbeat_state: EdgeThermalEnergyHeartbeatState,
    pub metrics: Vec<FleetMetricSample>,
    pub alerts: Vec<FleetAlertRecord>,
}

impl EdgeThermalEnergyEvaluation {
    pub fn control_for(&self, kind: EdgeThermalEnergyKind) -> &EdgeThermalEnergyControl {
        self.controls
            .iter()
            .find(|control| control.kind == kind)
            .expect("thermal energy evaluation should include every control")
    }
}

impl EdgeResourceEvaluation {
    pub fn control_for(&self, resource: EdgeResourceKind) -> &EdgeResourceControl {
        self.controls
            .iter()
            .find(|control| control.resource == resource)
            .expect("edge resource evaluation should include every resource control")
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EdgeResourceBudgetError {
    #[error("edge resource budget node_id cannot be empty")]
    EmptyNodeId,
    #[error("edge resource snapshot node_id {actual} does not match budget node {expected}")]
    NodeIdMismatch { expected: String, actual: String },
    #[error("edge resource budget max_cpu_percent must be positive and finite")]
    InvalidCpuLimit,
    #[error("edge resource budget max_memory_mb must be positive")]
    InvalidMemoryLimit,
    #[error("edge resource budget min_disk_free_gb must be positive and finite")]
    InvalidDiskLimit,
    #[error("edge resource budget throttle_at_fraction must be > 0 and <= 1")]
    InvalidThrottleFraction,
    #[error("edge thermal budget max_temperature_c must be positive and finite")]
    InvalidThermalLimit,
    #[error("edge energy budget min_battery_percent must be finite in the range 0..=100")]
    InvalidBatteryReserve,
    #[error("edge resource snapshot field {field} must be finite")]
    NonFiniteSnapshotValue { field: &'static str },
    #[error(transparent)]
    Observability(#[from] ObservabilityError),
}

pub fn evaluate_edge_thermal_energy_budget(
    budget: &EdgeThermalEnergyBudget,
    snapshot: &EdgeThermalEnergySnapshot,
) -> Result<EdgeThermalEnergyEvaluation, EdgeResourceBudgetError> {
    let node_id = normalize_node_id(&budget.node_id)?;
    let snapshot_node_id = normalize_node_id(&snapshot.node_id)?;
    if snapshot_node_id != node_id {
        return Err(EdgeResourceBudgetError::NodeIdMismatch {
            expected: node_id,
            actual: snapshot_node_id,
        });
    }
    validate_thermal_energy_budget(budget)?;
    validate_thermal_energy_snapshot(snapshot)?;

    let thermal_control = evaluate_optional_upper_limit(
        EdgeThermalEnergyKind::Thermal,
        "temperature_c",
        snapshot.temperature_c,
        budget.max_temperature_c,
        budget.throttle_at_fraction,
        "thermal budget unavailable",
        "temperature is approaching the configured thermal budget; throttle workload",
        "temperature exceeds the configured thermal budget; shed lower-priority workload",
    );
    let energy_control = evaluate_optional_lower_limit(
        EdgeThermalEnergyKind::Energy,
        "battery_remaining_percent",
        snapshot.battery_remaining_percent,
        budget.min_battery_percent,
        budget.throttle_at_fraction,
    );
    let controls = vec![thermal_control, energy_control];

    let context = ObservabilityContext::new(&node_id, None)?;
    let metrics = controls
        .iter()
        .filter_map(|control| {
            control.observed_value.map(|observed| {
                thermal_energy_metric(
                    &context,
                    &control.metric_name,
                    observed,
                    snapshot.at,
                    control.kind,
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let alerts = controls
        .iter()
        .filter_map(|control| build_thermal_energy_alert(&node_id, control, snapshot.at))
        .collect();
    let heartbeat_state = EdgeThermalEnergyHeartbeatState {
        thermal_state: controls
            .iter()
            .find(|control| control.kind == EdgeThermalEnergyKind::Thermal)
            .map(|control| control.action)
            .unwrap_or(EdgeResourceAction::Unavailable),
        energy_state: controls
            .iter()
            .find(|control| control.kind == EdgeThermalEnergyKind::Energy)
            .map(|control| control.action)
            .unwrap_or(EdgeResourceAction::Unavailable),
        thermal_available: snapshot.temperature_c.is_some(),
        energy_available: snapshot.battery_remaining_percent.is_some(),
    };

    Ok(EdgeThermalEnergyEvaluation {
        node_id,
        at: snapshot.at,
        controls,
        heartbeat_state,
        metrics,
        alerts,
    })
}

pub fn evaluate_edge_resource_budget(
    budget: &EdgeResourceBudget,
    snapshot: &EdgeResourceSnapshot,
) -> Result<EdgeResourceEvaluation, EdgeResourceBudgetError> {
    let node_id = normalize_node_id(&budget.node_id)?;
    let snapshot_node_id = normalize_node_id(&snapshot.node_id)?;
    if snapshot_node_id != node_id {
        return Err(EdgeResourceBudgetError::NodeIdMismatch {
            expected: node_id,
            actual: snapshot_node_id,
        });
    }
    validate_budget(budget)?;
    validate_snapshot(snapshot)?;

    let cpu_control = evaluate_upper_limit(
        EdgeResourceKind::Cpu,
        "cpu_usage_percent",
        snapshot.cpu_usage_percent,
        budget.max_cpu_percent,
        budget.throttle_at_fraction,
        "CPU usage is approaching the configured edge budget; throttle new work",
        "CPU usage exceeds the configured edge budget; shed lower-priority work",
    );
    let memory_control = evaluate_upper_limit(
        EdgeResourceKind::Memory,
        "memory_usage_mb",
        snapshot.memory_usage_mb as f64,
        budget.max_memory_mb as f64,
        budget.throttle_at_fraction,
        "memory usage is approaching the configured edge budget; throttle new work",
        "memory usage exceeds the configured edge budget; shed lower-priority work",
    );
    let disk_control = evaluate_disk_free_budget(
        snapshot.disk_free_gb,
        budget.min_disk_free_gb,
        budget.throttle_at_fraction,
    );
    let controls = vec![cpu_control, memory_control, disk_control];

    let context = ObservabilityContext::new(&node_id, None)?;
    let metrics = vec![
        budget_metric(
            &context,
            "edge_cpu_budget_pressure_ratio",
            snapshot.cpu_usage_percent / budget.max_cpu_percent,
            "ratio",
            snapshot.at,
            EdgeResourceKind::Cpu,
        )?,
        budget_metric(
            &context,
            "edge_memory_budget_pressure_ratio",
            snapshot.memory_usage_mb as f64 / budget.max_memory_mb as f64,
            "ratio",
            snapshot.at,
            EdgeResourceKind::Memory,
        )?,
        budget_metric(
            &context,
            "edge_disk_budget_pressure_ratio",
            disk_pressure_ratio(snapshot.disk_free_gb, budget.min_disk_free_gb),
            "ratio",
            snapshot.at,
            EdgeResourceKind::Disk,
        )?,
    ];
    let alerts = controls
        .iter()
        .filter_map(|control| build_budget_alert(&node_id, control, snapshot.at))
        .collect();

    Ok(EdgeResourceEvaluation {
        node_id,
        at: snapshot.at,
        controls,
        metrics,
        alerts,
    })
}

fn evaluate_upper_limit(
    resource: EdgeResourceKind,
    metric_name: &str,
    observed_value: f64,
    max_value: f64,
    throttle_at_fraction: f64,
    throttle_reason: &str,
    shed_reason: &str,
) -> EdgeResourceControl {
    let throttle_threshold = max_value * throttle_at_fraction;
    if observed_value >= max_value {
        return EdgeResourceControl {
            resource,
            action: EdgeResourceAction::Shed,
            metric_name: metric_name.to_string(),
            observed_value,
            threshold_value: max_value,
            severity: Some(FleetAlertSeverity::Critical),
            reason: shed_reason.to_string(),
        };
    }

    if observed_value >= throttle_threshold {
        return EdgeResourceControl {
            resource,
            action: EdgeResourceAction::Throttle,
            metric_name: metric_name.to_string(),
            observed_value,
            threshold_value: throttle_threshold,
            severity: Some(FleetAlertSeverity::Warning),
            reason: throttle_reason.to_string(),
        };
    }

    EdgeResourceControl {
        resource,
        action: EdgeResourceAction::Admit,
        metric_name: metric_name.to_string(),
        observed_value,
        threshold_value: throttle_threshold,
        severity: None,
        reason: "resource is within configured budget".to_string(),
    }
}

fn evaluate_disk_free_budget(
    disk_free_gb: f64,
    min_disk_free_gb: f64,
    throttle_at_fraction: f64,
) -> EdgeResourceControl {
    let warning_threshold = min_disk_free_gb / throttle_at_fraction;
    if disk_free_gb <= min_disk_free_gb {
        return EdgeResourceControl {
            resource: EdgeResourceKind::Disk,
            action: EdgeResourceAction::BackpressureWrites,
            metric_name: "disk_free_gb".to_string(),
            observed_value: disk_free_gb,
            threshold_value: min_disk_free_gb,
            severity: Some(FleetAlertSeverity::Critical),
            reason: "disk free capacity is below the configured reserve; backpressure writes"
                .to_string(),
        };
    }

    if disk_free_gb <= warning_threshold {
        return EdgeResourceControl {
            resource: EdgeResourceKind::Disk,
            action: EdgeResourceAction::BackpressureWrites,
            metric_name: "disk_free_gb".to_string(),
            observed_value: disk_free_gb,
            threshold_value: warning_threshold,
            severity: Some(FleetAlertSeverity::Warning),
            reason: "disk free capacity is approaching the configured reserve; backpressure writes"
                .to_string(),
        };
    }

    EdgeResourceControl {
        resource: EdgeResourceKind::Disk,
        action: EdgeResourceAction::Admit,
        metric_name: "disk_free_gb".to_string(),
        observed_value: disk_free_gb,
        threshold_value: warning_threshold,
        severity: None,
        reason: "disk free capacity is within configured budget".to_string(),
    }
}

fn evaluate_optional_upper_limit(
    kind: EdgeThermalEnergyKind,
    metric_name: &str,
    observed_value: Option<f64>,
    max_value: f64,
    throttle_at_fraction: f64,
    unavailable_reason: &str,
    throttle_reason: &str,
    shed_reason: &str,
) -> EdgeThermalEnergyControl {
    let Some(observed_value) = observed_value else {
        return unavailable_control(kind, metric_name, unavailable_reason);
    };
    let throttle_threshold = max_value * throttle_at_fraction;
    if observed_value >= max_value {
        return EdgeThermalEnergyControl {
            kind,
            action: EdgeResourceAction::Shed,
            metric_name: metric_name.to_string(),
            observed_value: Some(observed_value),
            threshold_value: Some(max_value),
            severity: Some(FleetAlertSeverity::Critical),
            reason: shed_reason.to_string(),
        };
    }
    if observed_value >= throttle_threshold {
        return EdgeThermalEnergyControl {
            kind,
            action: EdgeResourceAction::Throttle,
            metric_name: metric_name.to_string(),
            observed_value: Some(observed_value),
            threshold_value: Some(throttle_threshold),
            severity: Some(FleetAlertSeverity::Warning),
            reason: throttle_reason.to_string(),
        };
    }
    EdgeThermalEnergyControl {
        kind,
        action: EdgeResourceAction::Admit,
        metric_name: metric_name.to_string(),
        observed_value: Some(observed_value),
        threshold_value: Some(throttle_threshold),
        severity: None,
        reason: "thermal signal is within configured budget".to_string(),
    }
}

fn evaluate_optional_lower_limit(
    kind: EdgeThermalEnergyKind,
    metric_name: &str,
    observed_value: Option<f64>,
    min_value: f64,
    throttle_at_fraction: f64,
) -> EdgeThermalEnergyControl {
    let Some(observed_value) = observed_value else {
        return unavailable_control(kind, metric_name, "energy budget unavailable");
    };
    let warning_threshold = min_value / throttle_at_fraction;
    if observed_value <= min_value {
        return EdgeThermalEnergyControl {
            kind,
            action: EdgeResourceAction::Shed,
            metric_name: metric_name.to_string(),
            observed_value: Some(observed_value),
            threshold_value: Some(min_value),
            severity: Some(FleetAlertSeverity::Critical),
            reason: "battery reserve is below the configured energy budget; shed lower-priority workload"
                .to_string(),
        };
    }
    if observed_value <= warning_threshold {
        return EdgeThermalEnergyControl {
            kind,
            action: EdgeResourceAction::Throttle,
            metric_name: metric_name.to_string(),
            observed_value: Some(observed_value),
            threshold_value: Some(warning_threshold),
            severity: Some(FleetAlertSeverity::Warning),
            reason:
                "battery reserve is approaching the configured energy budget; throttle workload"
                    .to_string(),
        };
    }
    EdgeThermalEnergyControl {
        kind,
        action: EdgeResourceAction::Admit,
        metric_name: metric_name.to_string(),
        observed_value: Some(observed_value),
        threshold_value: Some(warning_threshold),
        severity: None,
        reason: "energy signal is within configured budget".to_string(),
    }
}

fn unavailable_control(
    kind: EdgeThermalEnergyKind,
    metric_name: &str,
    reason: &str,
) -> EdgeThermalEnergyControl {
    EdgeThermalEnergyControl {
        kind,
        action: EdgeResourceAction::Unavailable,
        metric_name: metric_name.to_string(),
        observed_value: None,
        threshold_value: None,
        severity: None,
        reason: reason.to_string(),
    }
}

fn build_budget_alert(
    node_id: &str,
    control: &EdgeResourceControl,
    evaluated_at: DateTime<Utc>,
) -> Option<FleetAlertRecord> {
    let severity = control.severity?;
    let comparator = match control.resource {
        EdgeResourceKind::Disk => FleetAlertComparator::LessThanOrEqual,
        EdgeResourceKind::Cpu | EdgeResourceKind::Memory => {
            FleetAlertComparator::GreaterThanOrEqual
        }
    };

    Some(FleetAlertRecord {
        alert_id: format!(
            "fleet-alert:{node_id}:resource_budget:{}",
            control.metric_name
        ),
        node_id: node_id.to_string(),
        correlation_id: None,
        kind: FleetAlertKind::ResourceBudget,
        severity,
        route: FleetAlertRoute::OperatorConsole,
        evidence: FleetAlertEvidence {
            metric_name: control.metric_name.clone(),
            observed_value: control.observed_value,
            threshold_value: control.threshold_value,
            comparator,
        },
        message: control.reason.clone(),
        evaluated_at,
    })
}

fn build_thermal_energy_alert(
    node_id: &str,
    control: &EdgeThermalEnergyControl,
    evaluated_at: DateTime<Utc>,
) -> Option<FleetAlertRecord> {
    let severity = control.severity?;
    let observed_value = control.observed_value?;
    let threshold_value = control.threshold_value?;
    let comparator = match control.kind {
        EdgeThermalEnergyKind::Thermal => FleetAlertComparator::GreaterThanOrEqual,
        EdgeThermalEnergyKind::Energy => FleetAlertComparator::LessThanOrEqual,
    };

    Some(FleetAlertRecord {
        alert_id: format!(
            "fleet-alert:{node_id}:thermal_energy_budget:{}",
            control.metric_name
        ),
        node_id: node_id.to_string(),
        correlation_id: None,
        kind: FleetAlertKind::ResourceBudget,
        severity,
        route: FleetAlertRoute::OperatorConsole,
        evidence: FleetAlertEvidence {
            metric_name: control.metric_name.clone(),
            observed_value,
            threshold_value,
            comparator,
        },
        message: control.reason.clone(),
        evaluated_at,
    })
}

fn budget_metric(
    context: &ObservabilityContext,
    name: &str,
    value: f64,
    unit: &str,
    at: DateTime<Utc>,
    resource: EdgeResourceKind,
) -> Result<FleetMetricSample, ObservabilityError> {
    FleetMetricSample::new(context, name, value, unit, at)?
        .with_label("category", "resource_budget")?
        .with_label("resource", resource.as_str())
}

fn thermal_energy_metric(
    context: &ObservabilityContext,
    name: &str,
    value: f64,
    at: DateTime<Utc>,
    kind: EdgeThermalEnergyKind,
) -> Result<FleetMetricSample, ObservabilityError> {
    let unit = match kind {
        EdgeThermalEnergyKind::Thermal => "celsius",
        EdgeThermalEnergyKind::Energy => "percent",
    };
    FleetMetricSample::new(context, name, value, unit, at)?
        .with_label("category", "thermal_energy_budget")?
        .with_label("resource", kind.as_str())
}

fn validate_budget(budget: &EdgeResourceBudget) -> Result<(), EdgeResourceBudgetError> {
    if !budget.max_cpu_percent.is_finite() || budget.max_cpu_percent <= 0.0 {
        return Err(EdgeResourceBudgetError::InvalidCpuLimit);
    }
    if budget.max_memory_mb == 0 {
        return Err(EdgeResourceBudgetError::InvalidMemoryLimit);
    }
    if !budget.min_disk_free_gb.is_finite() || budget.min_disk_free_gb <= 0.0 {
        return Err(EdgeResourceBudgetError::InvalidDiskLimit);
    }
    if !budget.throttle_at_fraction.is_finite()
        || budget.throttle_at_fraction <= 0.0
        || budget.throttle_at_fraction > 1.0
    {
        return Err(EdgeResourceBudgetError::InvalidThrottleFraction);
    }

    Ok(())
}

fn validate_thermal_energy_budget(
    budget: &EdgeThermalEnergyBudget,
) -> Result<(), EdgeResourceBudgetError> {
    if !budget.max_temperature_c.is_finite() || budget.max_temperature_c <= 0.0 {
        return Err(EdgeResourceBudgetError::InvalidThermalLimit);
    }
    if !budget.min_battery_percent.is_finite()
        || budget.min_battery_percent < 0.0
        || budget.min_battery_percent > 100.0
    {
        return Err(EdgeResourceBudgetError::InvalidBatteryReserve);
    }
    if !budget.throttle_at_fraction.is_finite()
        || budget.throttle_at_fraction <= 0.0
        || budget.throttle_at_fraction > 1.0
    {
        return Err(EdgeResourceBudgetError::InvalidThrottleFraction);
    }
    Ok(())
}

fn validate_snapshot(snapshot: &EdgeResourceSnapshot) -> Result<(), EdgeResourceBudgetError> {
    if !snapshot.cpu_usage_percent.is_finite() {
        return Err(EdgeResourceBudgetError::NonFiniteSnapshotValue {
            field: "cpu_usage_percent",
        });
    }
    if !snapshot.disk_free_gb.is_finite() {
        return Err(EdgeResourceBudgetError::NonFiniteSnapshotValue {
            field: "disk_free_gb",
        });
    }
    Ok(())
}

fn validate_thermal_energy_snapshot(
    snapshot: &EdgeThermalEnergySnapshot,
) -> Result<(), EdgeResourceBudgetError> {
    if snapshot
        .temperature_c
        .is_some_and(|value| !value.is_finite())
    {
        return Err(EdgeResourceBudgetError::NonFiniteSnapshotValue {
            field: "temperature_c",
        });
    }
    if snapshot
        .battery_remaining_percent
        .is_some_and(|value| !value.is_finite())
    {
        return Err(EdgeResourceBudgetError::NonFiniteSnapshotValue {
            field: "battery_remaining_percent",
        });
    }
    Ok(())
}

fn normalize_node_id(node_id: &str) -> Result<String, EdgeResourceBudgetError> {
    let node_id = node_id.trim();
    if node_id.is_empty() {
        Err(EdgeResourceBudgetError::EmptyNodeId)
    } else {
        Ok(node_id.to_string())
    }
}

fn disk_pressure_ratio(disk_free_gb: f64, min_disk_free_gb: f64) -> f64 {
    if disk_free_gb <= 0.0 {
        f64::MAX
    } else {
        min_disk_free_gb / disk_free_gb
    }
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate_edge_resource_budget, evaluate_edge_thermal_energy_budget, EdgeResourceAction,
        EdgeResourceBudget, EdgeResourceKind, EdgeResourceSnapshot, EdgeThermalEnergyBudget,
        EdgeThermalEnergyKind, EdgeThermalEnergySnapshot,
    };
    use crate::fleet_alerts::{FleetAlertKind, FleetAlertSeverity};

    #[test]
    fn edge_resource_budget_throttles_memory_and_emits_alert() {
        let budget = EdgeResourceBudget {
            node_id: "edge-jetson-1".to_string(),
            max_cpu_percent: 80.0,
            max_memory_mb: 4096,
            min_disk_free_gb: 8.0,
            throttle_at_fraction: 0.85,
        };
        let snapshot = EdgeResourceSnapshot {
            node_id: "edge-jetson-1".to_string(),
            cpu_usage_percent: 52.0,
            memory_usage_mb: 3600,
            disk_free_gb: 42.0,
            at: dt("2026-06-12T12:10:00Z"),
        };

        let evaluation = evaluate_edge_resource_budget(&budget, &snapshot)
            .expect("resource budget should evaluate");

        assert_eq!(evaluation.node_id, "edge-jetson-1");
        assert_eq!(
            evaluation.control_for(EdgeResourceKind::Memory).action,
            EdgeResourceAction::Throttle
        );
        assert_eq!(
            evaluation.control_for(EdgeResourceKind::Cpu).action,
            EdgeResourceAction::Admit
        );
        assert_eq!(evaluation.alerts.len(), 1);
        assert_eq!(evaluation.alerts[0].kind, FleetAlertKind::ResourceBudget);
        assert_eq!(evaluation.alerts[0].severity, FleetAlertSeverity::Warning);
        assert_eq!(evaluation.alerts[0].evidence.metric_name, "memory_usage_mb");
        assert!(evaluation.metrics.iter().any(|metric| metric.name
            == "edge_memory_budget_pressure_ratio"
            && metric.value > 0.85));
    }

    #[test]
    fn edge_resource_budget_backpressures_disk_writes_before_fill() {
        let budget = EdgeResourceBudget {
            node_id: "edge-pi-2".to_string(),
            max_cpu_percent: 75.0,
            max_memory_mb: 2048,
            min_disk_free_gb: 6.0,
            throttle_at_fraction: 0.80,
        };
        let snapshot = EdgeResourceSnapshot {
            node_id: "edge-pi-2".to_string(),
            cpu_usage_percent: 35.0,
            memory_usage_mb: 1024,
            disk_free_gb: 5.5,
            at: dt("2026-06-12T12:12:00Z"),
        };

        let evaluation = evaluate_edge_resource_budget(&budget, &snapshot)
            .expect("resource budget should evaluate");

        let disk_control = evaluation.control_for(EdgeResourceKind::Disk);
        assert_eq!(disk_control.action, EdgeResourceAction::BackpressureWrites);
        assert_eq!(disk_control.metric_name, "disk_free_gb");
        assert_eq!(evaluation.alerts.len(), 1);
        assert_eq!(evaluation.alerts[0].severity, FleetAlertSeverity::Critical);
        assert_eq!(evaluation.alerts[0].evidence.observed_value, 5.5);
        assert_eq!(evaluation.alerts[0].evidence.threshold_value, 6.0);
    }

    #[test]
    fn edge_thermal_energy_budget_throttles_on_thermal_signal_and_reports_heartbeat_state() {
        let budget = EdgeThermalEnergyBudget {
            node_id: "edge-jetson-thermal".to_string(),
            max_temperature_c: 80.0,
            min_battery_percent: 25.0,
            throttle_at_fraction: 0.90,
        };
        let snapshot = EdgeThermalEnergySnapshot {
            node_id: "edge-jetson-thermal".to_string(),
            temperature_c: Some(73.0),
            battery_remaining_percent: Some(64.0),
            at: dt("2026-06-15T00:15:00Z"),
        };

        let evaluation = evaluate_edge_thermal_energy_budget(&budget, &snapshot)
            .expect("thermal energy budget should evaluate");

        let thermal = evaluation.control_for(EdgeThermalEnergyKind::Thermal);
        assert_eq!(thermal.action, EdgeResourceAction::Throttle);
        assert_eq!(thermal.metric_name, "temperature_c");
        assert_eq!(thermal.observed_value, Some(73.0));
        assert_eq!(thermal.threshold_value, Some(72.0));
        assert_eq!(thermal.severity, Some(FleetAlertSeverity::Warning));
        assert_eq!(
            evaluation.control_for(EdgeThermalEnergyKind::Energy).action,
            EdgeResourceAction::Admit
        );
        assert_eq!(
            evaluation.heartbeat_state.thermal_state,
            EdgeResourceAction::Throttle
        );
        assert!(evaluation.heartbeat_state.thermal_available);
        assert_eq!(evaluation.alerts.len(), 1);
        assert_eq!(evaluation.alerts[0].kind, FleetAlertKind::ResourceBudget);
        assert_eq!(evaluation.alerts[0].evidence.metric_name, "temperature_c");
        assert!(evaluation
            .metrics
            .iter()
            .any(|metric| metric.name == "temperature_c"
                && metric.unit == "celsius"
                && metric
                    .labels
                    .iter()
                    .any(|label| label.0 == "category" && label.1 == "thermal_energy_budget")));
    }

    #[test]
    fn edge_thermal_energy_budget_reports_missing_thermal_sensor_unavailable_not_faked() {
        let budget = EdgeThermalEnergyBudget {
            node_id: "edge-pi-no-thermal".to_string(),
            max_temperature_c: 78.0,
            min_battery_percent: 20.0,
            throttle_at_fraction: 0.85,
        };
        let snapshot = EdgeThermalEnergySnapshot {
            node_id: "edge-pi-no-thermal".to_string(),
            temperature_c: None,
            battery_remaining_percent: Some(80.0),
            at: dt("2026-06-15T00:16:00Z"),
        };

        let evaluation = evaluate_edge_thermal_energy_budget(&budget, &snapshot)
            .expect("missing thermal sensor should be represented");

        let thermal = evaluation.control_for(EdgeThermalEnergyKind::Thermal);
        assert_eq!(thermal.action, EdgeResourceAction::Unavailable);
        assert_eq!(thermal.reason, "thermal budget unavailable");
        assert_eq!(thermal.observed_value, None);
        assert_eq!(thermal.threshold_value, None);
        assert!(!evaluation.heartbeat_state.thermal_available);
        assert_eq!(
            evaluation.heartbeat_state.thermal_state,
            EdgeResourceAction::Unavailable
        );
        assert!(!evaluation
            .metrics
            .iter()
            .any(|metric| metric.name == "temperature_c"));
        assert!(evaluation.alerts.is_empty());
    }

    fn dt(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&chrono::Utc)
    }
}
