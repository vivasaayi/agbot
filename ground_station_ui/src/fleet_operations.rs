use fleet_health::{
    FleetOperationsDashboardFeed, FleetOperationsFeedSourceStatus, FleetOperationsRolloutFeedState,
};
use serde::Serialize;
use shared::schemas::{
    FleetNodeHealthState, FleetNodeInventoryEntry, FleetNodeStatus, FleetVersionInventory,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetOverviewLinkState {
    Online,
    Stale,
    Down,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FleetOverviewAircraftStatus {
    pub node_id: String,
    pub owner_org_id: String,
    pub link_state: FleetOverviewLinkState,
    pub freshness: String,
    pub heartbeat_age_seconds: Option<u64>,
    pub enrolled_status: FleetNodeStatus,
    pub runtime_mode: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FleetStatusOverview {
    pub generated_at: String,
    pub aircraft_count: usize,
    pub stale_or_down_count: usize,
    pub read_only: bool,
    pub fleet_action_routes: Vec<String>,
    pub aircraft: Vec<FleetOverviewAircraftStatus>,
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

pub fn build_fleet_status_overview(
    inventory: &FleetVersionInventory,
    generated_at: impl Into<String>,
) -> FleetStatusOverview {
    let mut aircraft = inventory
        .entries
        .iter()
        .map(fleet_overview_aircraft_status)
        .collect::<Vec<_>>();
    aircraft.sort_by(|left, right| {
        fleet_overview_health_rank(left)
            .cmp(&fleet_overview_health_rank(right))
            .then_with(|| {
                right
                    .heartbeat_age_seconds
                    .unwrap_or(u64::MAX)
                    .cmp(&left.heartbeat_age_seconds.unwrap_or(u64::MAX))
            })
            .then_with(|| left.node_id.cmp(&right.node_id))
    });

    let stale_or_down_count = aircraft
        .iter()
        .filter(|aircraft| {
            matches!(
                aircraft.link_state,
                FleetOverviewLinkState::Stale | FleetOverviewLinkState::Down
            )
        })
        .count();

    FleetStatusOverview {
        generated_at: generated_at.into(),
        aircraft_count: aircraft.len(),
        stale_or_down_count,
        read_only: true,
        fleet_action_routes: Vec::new(),
        aircraft,
    }
}

fn fleet_overview_aircraft_status(entry: &FleetNodeInventoryEntry) -> FleetOverviewAircraftStatus {
    let link_state = match entry.health_state {
        Some(FleetNodeHealthState::Fresh) => FleetOverviewLinkState::Online,
        Some(FleetNodeHealthState::Stale) => FleetOverviewLinkState::Stale,
        Some(FleetNodeHealthState::Down) => FleetOverviewLinkState::Down,
        None => FleetOverviewLinkState::Unknown,
    };
    let freshness = match (entry.health_state, entry.heartbeat_age_seconds) {
        (Some(FleetNodeHealthState::Fresh), Some(age)) => format!("fresh ({age}s)"),
        (Some(FleetNodeHealthState::Stale), Some(age)) => format!("stale ({age}s)"),
        (Some(FleetNodeHealthState::Down), Some(age)) => format!("down ({age}s)"),
        (Some(FleetNodeHealthState::Fresh), None) => "fresh".to_string(),
        (Some(FleetNodeHealthState::Stale), None) => "stale".to_string(),
        (Some(FleetNodeHealthState::Down), None) => "down".to_string(),
        (None, _) => "no heartbeat".to_string(),
    };

    FleetOverviewAircraftStatus {
        node_id: entry.node_id.clone(),
        owner_org_id: entry.owner_org_id.clone(),
        link_state,
        freshness,
        heartbeat_age_seconds: entry.heartbeat_age_seconds,
        enrolled_status: entry.status,
        runtime_mode: entry.runtime_mode.as_str().to_string(),
        version: entry.version.clone(),
    }
}

fn fleet_overview_health_rank(aircraft: &FleetOverviewAircraftStatus) -> u8 {
    match aircraft.link_state {
        FleetOverviewLinkState::Down => 0,
        FleetOverviewLinkState::Stale => 1,
        FleetOverviewLinkState::Unknown => 2,
        FleetOverviewLinkState::Online => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fleet_health::{
        fleet_operations_source_current, fleet_operations_source_unavailable,
        FleetOperationsFeedSource, FleetOperationsRolloutFeedEntry, OtaRolloutStage,
    };
    use shared::schemas::{
        FleetNodeKind, FleetNodeRuntimeMode, FleetNodeStatus, FleetVersionInventory,
    };

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

    #[test]
    fn fleet_status_overview_sorts_aircraft_by_health() {
        let inventory = FleetVersionInventory {
            entries: vec![
                fleet_inventory_entry("drone-healthy", Some(FleetNodeHealthState::Fresh), Some(4)),
                fleet_inventory_entry("drone-stale", Some(FleetNodeHealthState::Stale), Some(31)),
                fleet_inventory_entry("drone-down", Some(FleetNodeHealthState::Down), Some(95)),
            ],
        };

        let overview = build_fleet_status_overview(&inventory, "2026-06-15T00:01:16Z");

        assert_eq!(overview.aircraft_count, 3);
        assert_eq!(overview.stale_or_down_count, 2);
        assert_eq!(
            overview
                .aircraft
                .iter()
                .map(|aircraft| aircraft.node_id.as_str())
                .collect::<Vec<_>>(),
            vec!["drone-down", "drone-stale", "drone-healthy"]
        );
        assert_eq!(
            overview.aircraft[0].link_state,
            FleetOverviewLinkState::Down
        );
        assert_eq!(
            overview.aircraft[1].link_state,
            FleetOverviewLinkState::Stale
        );
        assert_eq!(
            overview.aircraft[2].link_state,
            FleetOverviewLinkState::Online
        );
    }

    #[test]
    fn fleet_status_overview_flags_stale_heartbeat_instead_of_omitting_aircraft() {
        let inventory = FleetVersionInventory {
            entries: vec![fleet_inventory_entry(
                "drone-stale",
                Some(FleetNodeHealthState::Stale),
                Some(45),
            )],
        };

        let overview = build_fleet_status_overview(&inventory, "2026-06-15T00:01:16Z");

        assert_eq!(overview.aircraft_count, 1);
        assert_eq!(overview.stale_or_down_count, 1);
        assert_eq!(overview.aircraft[0].node_id, "drone-stale");
        assert_eq!(
            overview.aircraft[0].link_state,
            FleetOverviewLinkState::Stale
        );
        assert_eq!(overview.aircraft[0].freshness, "stale (45s)");
        assert_ne!(
            overview.aircraft[0].link_state,
            FleetOverviewLinkState::Online
        );
    }

    #[test]
    fn fleet_status_overview_is_read_only_and_exposes_no_fleet_action_routes() {
        let inventory = FleetVersionInventory {
            entries: vec![fleet_inventory_entry(
                "drone-healthy",
                Some(FleetNodeHealthState::Fresh),
                Some(2),
            )],
        };

        let overview = build_fleet_status_overview(&inventory, "2026-06-15T00:01:16Z");

        assert!(overview.read_only);
        assert!(overview.fleet_action_routes.is_empty());
    }

    fn fleet_inventory_entry(
        node_id: &str,
        health_state: Option<FleetNodeHealthState>,
        heartbeat_age_seconds: Option<u64>,
    ) -> FleetNodeInventoryEntry {
        FleetNodeInventoryEntry {
            node_id: node_id.to_string(),
            owner_org_id: "org-a".to_string(),
            kind: FleetNodeKind::Drone,
            runtime_mode: FleetNodeRuntimeMode::Flight,
            status: FleetNodeStatus::Enrolled,
            maintenance: false,
            version: Some("1.8.0".to_string()),
            config_version: Some(7),
            components: Vec::new(),
            capabilities: vec!["multispectral".to_string()],
            health_state,
            heartbeat_age_seconds,
        }
    }
}
