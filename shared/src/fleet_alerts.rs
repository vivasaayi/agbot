use crate::observability::FleetMetricSample;
use crate::schemas::{FleetNodeHealthSnapshot, FleetNodeHealthState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetAlertKind {
    NodeDown,
    LowDisk,
    LowFleetBattery,
    ProcessingStall,
    ResourceBudget,
}

impl FleetAlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FleetAlertKind::NodeDown => "node_down",
            FleetAlertKind::LowDisk => "low_disk",
            FleetAlertKind::LowFleetBattery => "low_fleet_battery",
            FleetAlertKind::ProcessingStall => "processing_stall",
            FleetAlertKind::ResourceBudget => "resource_budget",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetAlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetAlertRoute {
    OperatorConsole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetAlertComparator {
    LessThanOrEqual,
    GreaterThanOrEqual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetAlertEvidence {
    pub metric_name: String,
    pub observed_value: f64,
    pub threshold_value: f64,
    pub comparator: FleetAlertComparator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetAlertRecord {
    pub alert_id: String,
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub kind: FleetAlertKind,
    pub severity: FleetAlertSeverity,
    pub route: FleetAlertRoute,
    pub evidence: FleetAlertEvidence,
    pub message: String,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetAlertRuleConfig {
    pub node_down_after_seconds: u64,
    pub min_disk_free_gb: f64,
    pub min_fleet_battery_percent: f64,
    pub min_processing_messages_per_second: f64,
}

impl Default for FleetAlertRuleConfig {
    fn default() -> Self {
        Self {
            node_down_after_seconds: 60,
            min_disk_free_gb: 10.0,
            min_fleet_battery_percent: 20.0,
            min_processing_messages_per_second: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OperatorConsoleAlertFeed {
    alerts: Vec<FleetAlertRecord>,
}

impl OperatorConsoleAlertFeed {
    pub fn route(&mut self, alerts: Vec<FleetAlertRecord>) {
        self.alerts.extend(
            alerts
                .into_iter()
                .filter(|alert| alert.route == FleetAlertRoute::OperatorConsole),
        );
    }

    pub fn alerts(&self) -> &[FleetAlertRecord] {
        &self.alerts
    }

    pub fn alerts_for_node(&self, node_id: &str) -> Vec<&FleetAlertRecord> {
        self.alerts
            .iter()
            .filter(|alert| alert.node_id == node_id)
            .collect()
    }
}

pub fn evaluate_fleet_alerts(
    health: Option<&FleetNodeHealthSnapshot>,
    metrics: &[FleetMetricSample],
    config: &FleetAlertRuleConfig,
    evaluated_at: DateTime<Utc>,
) -> Vec<FleetAlertRecord> {
    let mut alerts = Vec::new();

    if let Some(health) = health {
        if health.state == FleetNodeHealthState::Down
            || health.heartbeat_age_seconds >= config.node_down_after_seconds
        {
            alerts.push(build_alert(
                &health.node_id,
                None,
                FleetAlertKind::NodeDown,
                FleetAlertSeverity::Critical,
                FleetAlertEvidence {
                    metric_name: "heartbeat_age_seconds".to_string(),
                    observed_value: health.heartbeat_age_seconds as f64,
                    threshold_value: config.node_down_after_seconds as f64,
                    comparator: FleetAlertComparator::GreaterThanOrEqual,
                },
                "fleet node is down",
                evaluated_at,
            ));
        }
    }

    if let Some(metric) = latest_metric(metrics, &["disk_free_gb"]) {
        if metric.value <= config.min_disk_free_gb {
            alerts.push(build_metric_alert(
                metric,
                FleetAlertKind::LowDisk,
                FleetAlertSeverity::Warning,
                config.min_disk_free_gb,
                FleetAlertComparator::LessThanOrEqual,
                "disk free capacity is below threshold",
                evaluated_at,
            ));
        }
    }

    if let Some(metric) = latest_metric(metrics, &["fleet_battery_percent", "battery_percentage"]) {
        if metric.value <= config.min_fleet_battery_percent {
            alerts.push(build_metric_alert(
                metric,
                FleetAlertKind::LowFleetBattery,
                if metric.value <= config.min_fleet_battery_percent / 2.0 {
                    FleetAlertSeverity::Critical
                } else {
                    FleetAlertSeverity::Warning
                },
                config.min_fleet_battery_percent,
                FleetAlertComparator::LessThanOrEqual,
                "fleet battery level is below threshold",
                evaluated_at,
            ));
        }
    }

    if let Some(metric) = latest_metric(metrics, &["processed_messages_per_second"]) {
        if metric.value <= config.min_processing_messages_per_second {
            alerts.push(build_metric_alert(
                metric,
                FleetAlertKind::ProcessingStall,
                if metric.value == 0.0 {
                    FleetAlertSeverity::Critical
                } else {
                    FleetAlertSeverity::Warning
                },
                config.min_processing_messages_per_second,
                FleetAlertComparator::LessThanOrEqual,
                "processing throughput is below threshold",
                evaluated_at,
            ));
        }
    }

    alerts.sort_by(|left, right| {
        alert_rank(left.kind)
            .cmp(&alert_rank(right.kind))
            .then_with(|| left.node_id.cmp(&right.node_id))
            .then_with(|| left.evidence.metric_name.cmp(&right.evidence.metric_name))
    });
    alerts
}

fn latest_metric<'a>(
    metrics: &'a [FleetMetricSample],
    names: &[&str],
) -> Option<&'a FleetMetricSample> {
    metrics
        .iter()
        .filter(|metric| names.iter().any(|name| metric.name == *name))
        .max_by(|left, right| {
            left.at
                .cmp(&right.at)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.node_id.cmp(&right.node_id))
        })
}

fn build_metric_alert(
    metric: &FleetMetricSample,
    kind: FleetAlertKind,
    severity: FleetAlertSeverity,
    threshold_value: f64,
    comparator: FleetAlertComparator,
    message: &str,
    evaluated_at: DateTime<Utc>,
) -> FleetAlertRecord {
    build_alert(
        &metric.node_id,
        metric.correlation_id.clone(),
        kind,
        severity,
        FleetAlertEvidence {
            metric_name: metric.name.clone(),
            observed_value: metric.value,
            threshold_value,
            comparator,
        },
        message,
        evaluated_at,
    )
}

fn build_alert(
    node_id: &str,
    correlation_id: Option<String>,
    kind: FleetAlertKind,
    severity: FleetAlertSeverity,
    evidence: FleetAlertEvidence,
    message: &str,
    evaluated_at: DateTime<Utc>,
) -> FleetAlertRecord {
    FleetAlertRecord {
        alert_id: format!(
            "fleet-alert:{node_id}:{}:{}",
            kind.as_str(),
            evidence.metric_name
        ),
        node_id: node_id.to_string(),
        correlation_id,
        kind,
        severity,
        route: FleetAlertRoute::OperatorConsole,
        evidence,
        message: message.to_string(),
        evaluated_at,
    }
}

fn alert_rank(kind: FleetAlertKind) -> u8 {
    match kind {
        FleetAlertKind::NodeDown => 0,
        FleetAlertKind::LowDisk => 1,
        FleetAlertKind::LowFleetBattery => 2,
        FleetAlertKind::ProcessingStall => 3,
        FleetAlertKind::ResourceBudget => 4,
    }
}

#[cfg(test)]
mod tests {
    use crate::observability::{FleetMetricSample, ObservabilityContext};
    use crate::schemas::{FleetNodeHealthSnapshot, FleetNodeHealthState, FleetNodeRuntimeMode};

    use super::{
        evaluate_fleet_alerts, FleetAlertKind, FleetAlertRoute, FleetAlertRuleConfig,
        FleetAlertSeverity, OperatorConsoleAlertFeed,
    };

    #[test]
    fn fleet_alert_rules_fire_low_disk_console_alert_with_threshold_evidence() {
        let context = ObservabilityContext::new("node-low-disk", Some("corr-disk")).unwrap();
        let metrics = vec![metric(&context, "disk_free_gb", 4.5)];
        let config = FleetAlertRuleConfig {
            min_disk_free_gb: 10.0,
            ..FleetAlertRuleConfig::default()
        };

        let alerts = evaluate_fleet_alerts(None, &metrics, &config, dt("2026-06-12T12:05:00Z"));

        assert_eq!(alerts.len(), 1);
        let alert = &alerts[0];
        assert_eq!(alert.kind, FleetAlertKind::LowDisk);
        assert_eq!(alert.severity, FleetAlertSeverity::Warning);
        assert_eq!(alert.route, FleetAlertRoute::OperatorConsole);
        assert_eq!(alert.node_id, "node-low-disk");
        assert_eq!(alert.evidence.metric_name, "disk_free_gb");
        assert_eq!(alert.evidence.observed_value, 4.5);
        assert_eq!(alert.evidence.threshold_value, 10.0);
        assert_eq!(alert.correlation_id.as_deref(), Some("corr-disk"));

        let mut feed = OperatorConsoleAlertFeed::default();
        feed.route(alerts);
        assert_eq!(feed.alerts_for_node("node-low-disk").len(), 1);
    }

    #[test]
    fn fleet_alert_rules_do_not_fire_on_baseline_metrics() {
        let context = ObservabilityContext::new("node-healthy", None).unwrap();
        let metrics = vec![
            metric(&context, "disk_free_gb", 48.0),
            metric(&context, "fleet_battery_percent", 78.0),
            metric(&context, "processed_messages_per_second", 14.0),
        ];
        let health = health_snapshot("node-healthy", FleetNodeHealthState::Fresh, 5);

        let alerts = evaluate_fleet_alerts(
            Some(&health),
            &metrics,
            &FleetAlertRuleConfig::default(),
            dt("2026-06-12T12:05:00Z"),
        );

        assert!(alerts.is_empty());
    }

    #[test]
    fn fleet_alert_rules_fire_node_down_battery_and_stall_alerts() {
        let context = ObservabilityContext::new("node-compounded", None).unwrap();
        let metrics = vec![
            metric(&context, "fleet_battery_percent", 9.0),
            metric(&context, "processed_messages_per_second", 0.0),
        ];
        let health = health_snapshot("node-compounded", FleetNodeHealthState::Down, 91);
        let config = FleetAlertRuleConfig {
            node_down_after_seconds: 60,
            min_fleet_battery_percent: 20.0,
            min_processing_messages_per_second: 1.0,
            ..FleetAlertRuleConfig::default()
        };

        let alerts =
            evaluate_fleet_alerts(Some(&health), &metrics, &config, dt("2026-06-12T12:05:00Z"));

        let kinds = alerts.iter().map(|alert| alert.kind).collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                FleetAlertKind::NodeDown,
                FleetAlertKind::LowFleetBattery,
                FleetAlertKind::ProcessingStall
            ]
        );
        assert!(alerts
            .iter()
            .all(|alert| alert.route == FleetAlertRoute::OperatorConsole));
        assert!(alerts
            .iter()
            .any(|alert| alert.severity == FleetAlertSeverity::Critical));
    }

    fn metric(context: &ObservabilityContext, name: &str, value: f64) -> FleetMetricSample {
        FleetMetricSample::new(context, name, value, "unit", dt("2026-06-12T12:00:00Z")).unwrap()
    }

    fn health_snapshot(
        node_id: &str,
        state: FleetNodeHealthState,
        heartbeat_age_seconds: u64,
    ) -> FleetNodeHealthSnapshot {
        FleetNodeHealthSnapshot {
            node_id: node_id.to_string(),
            version: "agbot-node 1.4.0".to_string(),
            config_version: 7,
            components: Vec::new(),
            capabilities: vec!["multispectral".to_string()],
            runtime_mode: FleetNodeRuntimeMode::Flight,
            uptime_seconds: 7200,
            last_heartbeat_at: dt("2026-06-12T12:00:00Z"),
            heartbeat_age_seconds,
            state,
        }
    }

    fn dt(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&chrono::Utc)
    }
}
