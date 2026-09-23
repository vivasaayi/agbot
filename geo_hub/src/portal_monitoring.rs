//! Farmer-facing monitoring state derived from durable queue evidence.
use serde::Serialize;

#[derive(Debug, Default)]
pub struct QueueEvidence {
    pub running: i64,
    pub queued: i64,
    pub retrying: i64,
    pub failed: i64,
    pub completed: i64,
    pub active_subscriptions: i64,
    pub next_attempt_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitoringStatus {
    pub status: &'static str,
    pub message: &'static str,
    pub next_attempt_at: Option<String>,
    pub failed_jobs: i64,
}

pub fn monitoring_status(
    enabled: bool,
    has_imagery: bool,
    queue: QueueEvidence,
) -> MonitoringStatus {
    let (status, message) = if !enabled {
        ("worker_disabled", "Automatic monitoring is stopped. Start AGBot with monitoring enabled to process imagery.")
    } else if queue.running > 0 {
        (
            "running",
            "Checking satellite imagery and processing your field. You can keep using the app.",
        )
    } else if queue.retrying > 0 {
        (
            "retrying",
            "An imagery request could not finish. AGBot will retry automatically.",
        )
    } else if queue.queued > 0 {
        (
            "queued",
            "Your field is waiting for satellite processing. Refresh to check progress.",
        )
    } else if queue.failed > 0 {
        ("failed", "Some monitoring jobs could not finish. Available imagery remains visible; processing needs attention.")
    } else if has_imagery {
        ("ready", "Usable imagery is available. Check its acquisition date and coverage before making decisions.")
    } else if queue.completed > 0 {
        ("no_usable_imagery", "No usable imagery is available yet. Clouds, coverage, and quality checks can limit results. This does not establish field health.")
    } else if queue.active_subscriptions > 0 {
        (
            "waiting",
            "Monitoring is enabled. Waiting for the first satellite check.",
        )
    } else {
        (
            "not_subscribed",
            "Automatic satellite monitoring has not been set up for this field.",
        )
    };
    MonitoringStatus {
        status,
        message,
        next_attempt_at: queue.next_attempt_at,
        failed_jobs: queue.failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waiting_states_do_not_imply_healthy_field() {
        assert_eq!(
            monitoring_status(
                true,
                false,
                QueueEvidence {
                    completed: 1,
                    ..Default::default()
                }
            )
            .status,
            "no_usable_imagery"
        );
        assert_eq!(
            monitoring_status(false, true, QueueEvidence::default()).status,
            "worker_disabled"
        );
        assert_eq!(
            monitoring_status(
                true,
                true,
                QueueEvidence {
                    queued: 1,
                    retrying: 1,
                    ..Default::default()
                }
            )
            .status,
            "retrying"
        );
        assert_eq!(
            monitoring_status(
                true,
                false,
                QueueEvidence {
                    running: 1,
                    ..Default::default()
                }
            )
            .status,
            "running"
        );
        assert_eq!(
            monitoring_status(
                true,
                true,
                QueueEvidence {
                    failed: 1,
                    ..Default::default()
                }
            )
            .status,
            "failed"
        );
        assert_eq!(
            monitoring_status(true, true, QueueEvidence::default()).status,
            "ready"
        );
    }
}
