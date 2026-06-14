use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing::{Level, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const PRIMARY_NODE_ID_ENV: &str = "AGBOT_NODE_ID";
const SECONDARY_NODE_ID_ENV: &str = "NODE_ID";
const HOSTNAME_ENV: &str = "HOSTNAME";
const COMPUTERNAME_ENV: &str = "COMPUTERNAME";
const FALLBACK_NODE_ID: &str = "agbot-node-local";

static ACTIVE_LOGGING_CONTEXT: OnceLock<LoggingContext> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggingContext {
    pub node_id: String,
    pub node_id_source: LoggingNodeIdSource,
}

impl LoggingContext {
    pub fn resolve(configured_node_id: Option<&str>, host_identity: Option<&str>) -> Self {
        if let Some(node_id) = normalize_configured_node_id(configured_node_id) {
            return Self {
                node_id,
                node_id_source: LoggingNodeIdSource::Configured,
            };
        }

        Self {
            node_id: derive_fallback_node_id(host_identity),
            node_id_source: LoggingNodeIdSource::Derived,
        }
    }

    pub fn from_env() -> Self {
        let configured_node_id = std::env::var(PRIMARY_NODE_ID_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var(SECONDARY_NODE_ID_ENV)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            });
        let host_identity = std::env::var(HOSTNAME_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var(COMPUTERNAME_ENV)
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            });

        Self::resolve(configured_node_id.as_deref(), host_identity.as_deref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoggingNodeIdSource {
    Configured,
    Derived,
}

/// Initialize logging for the application.
pub fn init_logging() -> Result<()> {
    dotenvy::dotenv().ok();
    init_logging_with_context(LoggingContext::from_env())
}

pub fn init_logging_with_context(context: LoggingContext) -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()?;

    let context = ACTIVE_LOGGING_CONTEXT.get_or_init(|| context).clone();

    if context.node_id_source == LoggingNodeIdSource::Derived {
        tracing::warn!(
            node_id = %context.node_id,
            "node_id not configured; using stable derived logging node id"
        );
    }

    Ok(())
}

pub fn active_logging_context() -> LoggingContext {
    ACTIVE_LOGGING_CONTEXT
        .get()
        .cloned()
        .unwrap_or_else(LoggingContext::from_env)
}

pub fn current_operation_span(correlation_id: Option<&str>) -> Span {
    logging_operation_span(&active_logging_context(), correlation_id)
}

pub fn logging_operation_span(context: &LoggingContext, correlation_id: Option<&str>) -> Span {
    let correlation_id = normalize_correlation_id(correlation_id);
    tracing::span!(
        Level::INFO,
        "agbot_operation",
        node_id = %context.node_id,
        correlation_id = %correlation_id
    )
}

pub fn with_correlation_id<T>(correlation_id: Option<&str>, operation: impl FnOnce() -> T) -> T {
    let span = current_operation_span(correlation_id);
    let _entered = span.enter();
    operation()
}

fn normalize_configured_node_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_correlation_id(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

fn derive_fallback_node_id(host_identity: Option<&str>) -> String {
    let normalized = host_identity
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_host_identity)
        .filter(|value| !value.is_empty());

    normalized
        .map(|host| format!("agbot-{host}"))
        .unwrap_or_else(|| FALLBACK_NODE_ID.to_string())
}

fn normalize_host_identity(host_identity: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_dash = false;

    for ch in host_identity.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !normalized.is_empty() {
            normalized.push('-');
            last_was_dash = true;
        }
    }

    normalized.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    use super::{logging_operation_span, LoggingContext, LoggingNodeIdSource};

    #[test]
    fn logging_context_uses_configured_node_and_correlation_span_fields() {
        let context = LoggingContext::resolve(Some(" edge-node-7 "), Some("fallback-host"));
        assert_eq!(context.node_id, "edge-node-7");
        assert_eq!(context.node_id_source, LoggingNodeIdSource::Configured);

        let writer = CapturedWriter::default();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .without_time()
                .with_writer(writer.clone()),
        );

        let _guard = subscriber.set_default();
        let span = logging_operation_span(&context, Some("corr-42"));
        let _entered = span.enter();
        tracing::info!("log context probe");

        let output = writer.output();
        assert!(output.contains("edge-node-7"), "{output}");
        assert!(output.contains("corr-42"), "{output}");
        assert!(output.contains("log context probe"), "{output}");
    }

    #[test]
    fn logging_context_derives_stable_fallback_when_node_id_missing() {
        let context = LoggingContext::resolve(None, Some(" Lab Host 01 "));

        assert_eq!(context.node_id, "agbot-lab-host-01");
        assert_eq!(context.node_id_source, LoggingNodeIdSource::Derived);
        assert_eq!(
            LoggingContext::resolve(Some("  "), Some(" Lab Host 01 ")).node_id,
            "agbot-lab-host-01"
        );
    }

    #[derive(Clone, Default)]
    struct CapturedWriter {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl CapturedWriter {
        fn output(&self) -> String {
            String::from_utf8(self.bytes.lock().expect("capture lock").clone())
                .expect("capture should be UTF-8")
        }
    }

    impl<'a> MakeWriter<'a> for CapturedWriter {
        type Writer = CapturedWrite;

        fn make_writer(&'a self) -> Self::Writer {
            CapturedWrite {
                bytes: Arc::clone(&self.bytes),
            }
        }
    }

    struct CapturedWrite {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for CapturedWrite {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes
                .lock()
                .expect("capture lock")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
