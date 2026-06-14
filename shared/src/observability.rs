use crate::logging::LoggingContext;
use crate::types::PerformanceMetrics;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservabilityContext {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl ObservabilityContext {
    pub fn new(node_id: &str, correlation_id: Option<&str>) -> Result<Self, ObservabilityError> {
        let node_id = normalize_required_text(node_id, ObservabilityError::EmptyNodeId)?;
        let correlation_id = normalize_optional_text(correlation_id);

        Ok(Self {
            node_id,
            correlation_id,
        })
    }

    pub fn from_logging_context(
        logging_context: &LoggingContext,
        correlation_id: Option<&str>,
    ) -> Self {
        Self {
            node_id: logging_context.node_id.clone(),
            correlation_id: normalize_optional_text(correlation_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetMetricSample {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub at: DateTime<Utc>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

impl FleetMetricSample {
    pub fn new(
        context: &ObservabilityContext,
        name: &str,
        value: f64,
        unit: &str,
        at: DateTime<Utc>,
    ) -> Result<Self, ObservabilityError> {
        if !value.is_finite() {
            return Err(ObservabilityError::NonFiniteMetricValue {
                name: name.trim().to_string(),
            });
        }

        Ok(Self {
            node_id: context.node_id.clone(),
            correlation_id: context.correlation_id.clone(),
            name: normalize_required_text(name, ObservabilityError::EmptyMetricName)?,
            value,
            unit: normalize_required_text(unit, ObservabilityError::EmptyMetricUnit)?,
            at,
            labels: BTreeMap::new(),
        })
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Result<Self, ObservabilityError> {
        let key = normalize_required_text(key, ObservabilityError::EmptyMetricLabel)?;
        let value = normalize_required_text(value, ObservabilityError::EmptyMetricLabel)?;
        self.labels.insert(key, value);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRecord {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub name: String,
    pub at: DateTime<Utc>,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

impl TraceRecord {
    pub fn new(
        context: &ObservabilityContext,
        name: &str,
        at: DateTime<Utc>,
    ) -> Result<Self, ObservabilityError> {
        Ok(Self {
            node_id: context.node_id.clone(),
            correlation_id: context.correlation_id.clone(),
            name: normalize_required_text(name, ObservabilityError::EmptyTraceName)?,
            at,
            fields: BTreeMap::new(),
        })
    }

    pub fn with_field(mut self, key: &str, value: &str) -> Result<Self, ObservabilityError> {
        let key = normalize_required_text(key, ObservabilityError::EmptyTraceField)?;
        let value = normalize_required_text(value, ObservabilityError::EmptyTraceField)?;
        self.fields.insert(key, value);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservabilityEvent {
    Metric(FleetMetricSample),
    Trace(TraceRecord),
}

pub trait ObservabilitySink {
    fn ingest(&mut self, event: ObservabilityEvent) -> Result<(), ObservabilityExportError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BestEffortObservabilityExporter {
    max_buffered: usize,
    buffer: VecDeque<ObservabilityEvent>,
    dropped_count: u64,
}

impl BestEffortObservabilityExporter {
    pub fn new(max_buffered: usize) -> Self {
        Self {
            max_buffered,
            buffer: VecDeque::new(),
            dropped_count: 0,
        }
    }

    pub fn export(
        &mut self,
        event: ObservabilityEvent,
        sink: &mut impl ObservabilitySink,
    ) -> ExportOutcome {
        match sink.ingest(event.clone()) {
            Ok(()) => ExportOutcome::Delivered,
            Err(error) => self.buffer_after_failure(event, error),
        }
    }

    pub fn flush_buffer(&mut self, sink: &mut impl ObservabilitySink) -> FlushSummary {
        let mut delivered = 0usize;
        let mut last_error = None;

        while let Some(event) = self.buffer.front().cloned() {
            match sink.ingest(event) {
                Ok(()) => {
                    self.buffer.pop_front();
                    delivered += 1;
                }
                Err(error) => {
                    last_error = Some(error);
                    break;
                }
            }
        }

        FlushSummary {
            delivered,
            remaining: self.buffer.len(),
            dropped_total: self.dropped_count,
            last_error,
        }
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }

    fn buffer_after_failure(
        &mut self,
        event: ObservabilityEvent,
        error: ObservabilityExportError,
    ) -> ExportOutcome {
        if self.max_buffered == 0 {
            self.dropped_count += 1;
            return ExportOutcome::Dropped {
                dropped_total: self.dropped_count,
                reason: error,
            };
        }

        let dropped_oldest = if self.buffer.len() == self.max_buffered {
            self.buffer.pop_front();
            self.dropped_count += 1;
            true
        } else {
            false
        };
        self.buffer.push_back(event);

        if dropped_oldest {
            ExportOutcome::DroppedOldest {
                buffered: self.buffer.len(),
                dropped_total: self.dropped_count,
                reason: error,
            }
        } else {
            ExportOutcome::Buffered {
                buffered: self.buffer.len(),
                reason: error,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportOutcome {
    Delivered,
    Buffered {
        buffered: usize,
        reason: ObservabilityExportError,
    },
    DroppedOldest {
        buffered: usize,
        dropped_total: u64,
        reason: ObservabilityExportError,
    },
    Dropped {
        dropped_total: u64,
        reason: ObservabilityExportError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushSummary {
    pub delivered: usize,
    pub remaining: usize,
    pub dropped_total: u64,
    pub last_error: Option<ObservabilityExportError>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InMemoryObservabilityCollector {
    events: Vec<ObservabilityEvent>,
    unavailable_reason: Option<String>,
}

impl InMemoryObservabilityCollector {
    pub fn unavailable(reason: &str) -> Self {
        Self {
            events: Vec::new(),
            unavailable_reason: Some(
                normalize_optional_text(Some(reason)).unwrap_or_else(|| "unavailable".to_string()),
            ),
        }
    }

    pub fn events(&self) -> &[ObservabilityEvent] {
        &self.events
    }

    pub fn metrics_for_node(&self, node_id: &str) -> Vec<&FleetMetricSample> {
        self.events
            .iter()
            .filter_map(|event| match event {
                ObservabilityEvent::Metric(metric) if metric.node_id == node_id => Some(metric),
                _ => None,
            })
            .collect()
    }

    pub fn trace_names_for_node(&self, node_id: &str) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|event| match event {
                ObservabilityEvent::Trace(trace) if trace.node_id == node_id => {
                    Some(trace.name.clone())
                }
                _ => None,
            })
            .collect()
    }
}

impl ObservabilitySink for InMemoryObservabilityCollector {
    fn ingest(&mut self, event: ObservabilityEvent) -> Result<(), ObservabilityExportError> {
        if let Some(reason) = &self.unavailable_reason {
            return Err(ObservabilityExportError::CollectorUnavailable {
                reason: reason.clone(),
            });
        }

        self.events.push(event);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ObservabilityExportError {
    #[error("observability collector unavailable: {reason}")]
    CollectorUnavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ObservabilityError {
    #[error("observability node_id cannot be empty")]
    EmptyNodeId,
    #[error("observability metric name cannot be empty")]
    EmptyMetricName,
    #[error("observability metric unit cannot be empty")]
    EmptyMetricUnit,
    #[error("observability metric label cannot be empty")]
    EmptyMetricLabel,
    #[error("observability metric {name} value must be finite")]
    NonFiniteMetricValue { name: String },
    #[error("observability trace name cannot be empty")]
    EmptyTraceName,
    #[error("observability trace field cannot be empty")]
    EmptyTraceField,
}

pub fn metric_samples_from_performance_metrics(
    context: &ObservabilityContext,
    metrics: PerformanceMetrics,
    error_rate_per_minute: f64,
) -> Result<Vec<FleetMetricSample>, ObservabilityError> {
    let at = metrics.timestamp;
    let mut samples = vec![
        FleetMetricSample::new(
            context,
            "cpu_usage_percent",
            metrics.cpu_usage_percent as f64,
            "percent",
            at,
        )?
        .with_label("category", "resource")?,
        FleetMetricSample::new(
            context,
            "memory_usage_mb",
            metrics.memory_usage_mb as f64,
            "MiB",
            at,
        )?
        .with_label("category", "resource")?,
        FleetMetricSample::new(context, "disk_usage_gb", metrics.disk_usage_gb, "GiB", at)?
            .with_label("category", "resource")?,
        FleetMetricSample::new(
            context,
            "network_throughput_mbps",
            metrics.network_throughput_mbps as f64,
            "Mbps",
            at,
        )?
        .with_label("category", "throughput")?,
        FleetMetricSample::new(
            context,
            "active_connections",
            metrics.active_connections as f64,
            "count",
            at,
        )?
        .with_label("category", "capacity")?,
        FleetMetricSample::new(
            context,
            "processed_messages_per_second",
            metrics.processed_messages_per_second as f64,
            "messages/s",
            at,
        )?
        .with_label("category", "throughput")?,
    ];
    samples.push(
        FleetMetricSample::new(
            context,
            "error_rate_per_minute",
            error_rate_per_minute,
            "errors/min",
            at,
        )?
        .with_label("category", "error_rate")?,
    );

    Ok(samples)
}

fn normalize_required_text(
    value: &str,
    error: ObservabilityError,
) -> Result<String, ObservabilityError> {
    let value = value.trim();
    if value.is_empty() {
        Err(error)
    } else {
        Ok(value.to_string())
    }
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use crate::logging::LoggingContext;
    use crate::types::PerformanceMetrics;

    use super::{
        metric_samples_from_performance_metrics, BestEffortObservabilityExporter,
        InMemoryObservabilityCollector, ObservabilityContext, ObservabilityEvent, TraceRecord,
    };

    #[test]
    fn observability_metrics_are_tagged_and_queryable_by_node() {
        let context = ObservabilityContext::from_logging_context(
            &LoggingContext::resolve(Some("node-7"), None),
            Some("corr-7"),
        );
        let metrics = metric_samples_from_performance_metrics(
            &context,
            PerformanceMetrics {
                cpu_usage_percent: 41.5,
                memory_usage_mb: 2048,
                disk_usage_gb: 128.25,
                network_throughput_mbps: 18.5,
                active_connections: 12,
                processed_messages_per_second: 93.0,
                timestamp: dt("2026-06-12T12:00:00Z"),
            },
            0.05,
        )
        .expect("performance metrics should convert to samples");

        assert!(metrics.iter().all(|metric| metric.node_id == "node-7"));
        assert!(metrics
            .iter()
            .all(|metric| metric.correlation_id.as_deref() == Some("corr-7")));

        let mut collector = InMemoryObservabilityCollector::default();
        let mut exporter = BestEffortObservabilityExporter::new(8);
        for metric in metrics {
            exporter.export(ObservabilityEvent::Metric(metric), &mut collector);
        }

        let node_metrics = collector.metrics_for_node("node-7");
        let metric_names = node_metrics
            .iter()
            .map(|metric| metric.name.as_str())
            .collect::<Vec<_>>();
        assert!(metric_names.contains(&"cpu_usage_percent"));
        assert!(metric_names.contains(&"processed_messages_per_second"));
        assert!(metric_names.contains(&"error_rate_per_minute"));
    }

    #[test]
    fn exporter_buffers_and_drops_bounded_when_collector_is_down() {
        let context =
            ObservabilityContext::new("node-9", Some("corr-9")).expect("context should normalize");
        let mut down = InMemoryObservabilityCollector::unavailable("collector offline");
        let mut exporter = BestEffortObservabilityExporter::new(2);

        exporter.export(
            ObservabilityEvent::Trace(
                TraceRecord::new(&context, "first", dt("2026-06-12T12:00:00Z")).unwrap(),
            ),
            &mut down,
        );
        exporter.export(
            ObservabilityEvent::Trace(
                TraceRecord::new(&context, "second", dt("2026-06-12T12:00:01Z")).unwrap(),
            ),
            &mut down,
        );
        exporter.export(
            ObservabilityEvent::Trace(
                TraceRecord::new(&context, "third", dt("2026-06-12T12:00:02Z")).unwrap(),
            ),
            &mut down,
        );

        assert_eq!(exporter.buffered_len(), 2);
        assert_eq!(exporter.dropped_count(), 1);

        let mut recovered = InMemoryObservabilityCollector::default();
        let summary = exporter.flush_buffer(&mut recovered);
        assert_eq!(summary.delivered, 2);
        assert_eq!(exporter.buffered_len(), 0);
        assert_eq!(
            recovered.trace_names_for_node("node-9"),
            vec!["second".to_string(), "third".to_string()]
        );
    }

    fn dt(value: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&chrono::Utc)
    }
}
