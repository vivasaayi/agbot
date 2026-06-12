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
    #[error("edge resource snapshot field {field} must be finite")]
    NonFiniteSnapshotValue { field: &'static str },
    #[error(transparent)]
    Observability(#[from] ObservabilityError),
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
        evaluate_edge_resource_budget, EdgeResourceAction, EdgeResourceBudget, EdgeResourceKind,
        EdgeResourceSnapshot,
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

    fn dt(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&chrono::Utc)
    }
}
