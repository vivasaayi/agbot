use fleet_health::{
    FleetOperationsDashboardFeed, FleetOperationsFeedSourceStatus, FleetOperationsRolloutFeedState,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FleetOperationsConsoleSummary {
    pub generated_at: String,
    pub node_count: usize,
    pub alert_count: usize,
    pub active_rollout_count: usize,
    pub source_gap_count: usize,
    pub stale_or_unavailable: bool,
    pub lines: Vec<String>,
}

pub fn summarize_fleet_operations_feed(
    feed: &FleetOperationsDashboardFeed,
) -> FleetOperationsConsoleSummary {
    let source_gap_count = feed.source_gaps.len();
    let active_rollout_count = feed
        .rollouts
        .iter()
        .filter(|rollout| {
            matches!(
                rollout.state,
                FleetOperationsRolloutFeedState::Advancing
                    | FleetOperationsRolloutFeedState::Started
                    | FleetOperationsRolloutFeedState::Paused
            )
        })
        .count();
    let stale_or_unavailable = feed
        .sources
        .iter()
        .any(|source| source.status == FleetOperationsFeedSourceStatus::Unavailable);
    let mut lines = vec![
        "Fleet Operations:".to_string(),
        format!("  Nodes: {}", feed.inventory.entries.len()),
        format!("  Alerts: {}", feed.alerts.len()),
        format!("  Active rollouts: {active_rollout_count}"),
    ];

    if source_gap_count > 0 {
        lines.push(format!("  Source gaps: {source_gap_count}"));
        for gap in &feed.source_gaps {
            lines.push(format!(
                "  Gap {:?}: {}",
                gap.source,
                gap.message
                    .as_deref()
                    .unwrap_or("source unavailable without detail")
            ));
        }
    }

    FleetOperationsConsoleSummary {
        generated_at: feed.generated_at.clone(),
        node_count: feed.inventory.entries.len(),
        alert_count: feed.alerts.len(),
        active_rollout_count,
        source_gap_count,
        stale_or_unavailable,
        lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fleet_health::{
        fleet_operations_source_current, fleet_operations_source_unavailable,
        FleetOperationsFeedSource, FleetOperationsRolloutFeedEntry, OtaRolloutStage,
    };
    use shared::schemas::FleetVersionInventory;

    #[test]
    fn console_summary_reflects_feed_counts_without_mutation_controls() {
        let feed = FleetOperationsDashboardFeed {
            generated_at: "2026-06-12T13:05:00Z".to_string(),
            inventory: FleetVersionInventory { entries: vec![] },
            alerts: vec![],
            rollouts: vec![FleetOperationsRolloutFeedEntry {
                rollout_id: "rollout-2026-06-12".to_string(),
                stage: OtaRolloutStage::Canary,
                version: Some("2.0.0".to_string()),
                state: FleetOperationsRolloutFeedState::Started,
                reason_code: "started_by_operator".to_string(),
                updated_at: "2026-06-12T13:04:00Z".to_string(),
                evaluated_node_count: 0,
            }],
            sources: vec![fleet_operations_source_current(
                FleetOperationsFeedSource::Inventory,
                "2026-06-12T13:04:59Z",
            )],
            source_gaps: vec![],
        };

        let summary = summarize_fleet_operations_feed(&feed);

        assert_eq!(summary.node_count, 0);
        assert_eq!(summary.alert_count, 0);
        assert_eq!(summary.active_rollout_count, 1);
        assert!(!summary.stale_or_unavailable);
        assert!(summary
            .lines
            .iter()
            .any(|line| line == "  Active rollouts: 1"));
    }

    #[test]
    fn console_summary_surfaces_feed_source_gap() {
        let gap = fleet_operations_source_unavailable(
            FleetOperationsFeedSource::Alerts,
            "2026-06-12T13:06:00Z",
            "alert store unavailable",
        );
        let feed = FleetOperationsDashboardFeed {
            generated_at: "2026-06-12T13:06:01Z".to_string(),
            inventory: FleetVersionInventory { entries: vec![] },
            alerts: vec![],
            rollouts: vec![],
            sources: vec![gap.clone()],
            source_gaps: vec![gap],
        };

        let summary = summarize_fleet_operations_feed(&feed);

        assert!(summary.stale_or_unavailable);
        assert_eq!(summary.source_gap_count, 1);
        assert!(summary
            .lines
            .iter()
            .any(|line| line.contains("alert store unavailable")));
    }
}
