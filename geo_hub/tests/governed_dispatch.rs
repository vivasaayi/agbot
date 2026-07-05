//! End-to-end governed-dispatch test (Track E phase E3): an accepted proposal is
//! drafted into a mission, dry-run and approved by a *distinct* operator, then
//! dispatched through `mission_planner::guarded_dispatch`. The dispatch records an
//! `Action` on the ledger so a backward trace closes action -> proposal -> L0.
//!
//! The mission and flight context are real mission_planner domain objects (a
//! validated, armed simulation mission) — not mocks. Only the flight hardware
//! boundary (the MAVLink ack) is simulated, via the ack tracker.

use anyhow::Result;
use chrono::{Duration, TimeZone, Utc};
use geo::{point, polygon};
use geo_hub::proposal_mission::{
    authorize_mission_dispatch, draft_mission_for_proposal, dry_run_mission_dispatch,
    OperatorApproval,
};
use geo_hub::proposal_queue::{self, ProposalCreateRequest, ProposalDecision, ProposalSourceKind};
use geo_hub::state::AppState;
use geo_hub::{catalog, db, proposal_dispatch, provenance_store, server, HubConfig};
use mission_planner::mavlink_integration::{MAVLinkCommandAckTracker, MAV_CMD_NAV_TAKEOFF};
use mission_planner::{
    AbortRecoveryConfig, AbortRecoveryContext, AbortTrigger, DispatchSafetyConfig,
    GuardedDispatchCommand, GuardedDispatchContext, Mission, TelemetryLinkState, Waypoint,
    WaypointType,
};
use serde_json::json;
use shared::product_graph::{ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

const T0: &str = "2026-06-01T00:00:00Z";

async fn pool_ctx(tmp: &TempDir) -> Result<db::DbPool> {
    let config = HubConfig {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            tmp.path().join("dispatch.db").display()
        ),
        data_root: tmp.path().join("data"),
        ..HubConfig::default()
    };
    config.ensure_data_dirs()?;
    let pool = db::connect_pool(&config).await?;
    // Build the router once so all tables (proposals, provenance) are migrated.
    let _ = server::build_router(AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        scene_search_cache: Default::default(),
    });
    Ok(pool)
}

fn draft(level: ProductLevel, kind: &str, inputs: Vec<ProductInputRef>) -> ProductRecordDraft {
    ProductRecordDraft {
        level,
        kind: kind.to_string(),
        algorithm_id: format!("{kind}.compute"),
        algorithm_version: "1.0.0".to_string(),
        parameters: json!({ "scene_id": "scene-1" }),
        inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            scene_id: Some("scene-1".to_string()),
            temporal_start: T0.to_string(),
            temporal_end: T0.to_string(),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some("landsat-9".to_string()),
    }
}

/// Register an L0->L1->L2 chain plus a lineage-tracked finding, returning
/// (l2_product_id, finding_id).
async fn seed_finding(pool: &db::DbPool) -> Result<(String, String)> {
    let l0 = catalog::register_product(pool, &draft(ProductLevel::L0, "raw_capture", vec![]), T0)
        .await?;
    let l1 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L1,
            "band_nir",
            vec![ProductInputRef {
                product_id: l0,
                role: "raw".into(),
            }],
        ),
        T0,
    )
    .await?;
    let l2 = catalog::register_product(
        pool,
        &draft(
            ProductLevel::L2,
            "ndvi",
            vec![ProductInputRef {
                product_id: l1,
                role: "band:nir".into(),
            }],
        ),
        T0,
    )
    .await?;
    // A finding sourced from the L2 product.
    let finding_id = "finding:pest-1".to_string();
    provenance_store::append_lineage(
        pool,
        &provenance::LineageRecord {
            artifact_id: finding_id.clone(),
            kind: provenance::ArtifactKind::Finding,
            inputs: vec![l2.clone()],
            method: "anomaly.detect".to_string(),
            parameters: provenance::ProvenanceParameters::from_json(json!({})),
            operator: "test".to_string(),
            actor: provenance::ActorIdentity::system("test"),
            created_at: T0.to_string(),
        },
    )
    .await?;
    Ok((l2, finding_id))
}

fn armed_mission() -> Mission {
    let area = polygon![
        (x: 0.0, y: 0.0),
        (x: 200.0, y: 0.0),
        (x: 200.0, y: 200.0),
        (x: 0.0, y: 200.0),
        (x: 0.0, y: 0.0),
    ];
    let mut mission = Mission::new(
        "Governed Scout".to_string(),
        "proposal-driven scout mission".to_string(),
        area,
    );
    mission.add_waypoint(Waypoint::new(
        point!(x: 10.0, y: 10.0),
        20.0,
        WaypointType::Takeoff,
    ));
    mission.add_waypoint(Waypoint::new(
        point!(x: 120.0, y: 120.0),
        40.0,
        WaypointType::Survey,
    ));
    mission.add_waypoint(Waypoint::new(
        point!(x: 20.0, y: 20.0),
        0.0,
        WaypointType::Landing,
    ));
    mission.validate().expect("fixture validates");
    mission.arm().expect("fixture arms");
    mission
}

fn dispatch_context(mission: &Mission) -> GuardedDispatchContext {
    let sent_at = Utc.timestamp_opt(1_800_000_100, 0).unwrap();
    GuardedDispatchContext {
        current_position: Some(point!(x: 20.0, y: 20.0)),
        no_fly_zones: Vec::new(),
        dispatch_safety: DispatchSafetyConfig {
            altitude_ceiling_m: 120.0,
        },
        battery_percentage: 80,
        minimum_battery_percentage: 30,
        weather: None,
        airspace_constraints: Vec::new(),
        link_state: TelemetryLinkState::Fresh,
        sent_at,
        simulated_ack_latency: Duration::milliseconds(120),
        abort_context: Some(AbortRecoveryContext {
            current_position: point!(x: 20.0, y: 20.0),
            home_position: point!(x: 0.0, y: 0.0),
            battery_percentage: 80,
            emergency_landing_sites: vec![point!(x: 30.0, y: 30.0)],
            trigger: AbortTrigger::GeofenceViolation,
            triggered_at: sent_at,
        }),
        abort_config: AbortRecoveryConfig::default(),
        mission_id: mission.id,
    }
}

#[tokio::test]
async fn accepted_proposal_dispatches_and_records_action_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool_ctx(&tmp).await?;
    let (l2, finding_id) = seed_finding(&pool).await?;

    // Raise + accept a scout proposal from the finding.
    let proposal = proposal_queue::create_proposal(
        &pool,
        &ProposalCreateRequest {
            source_kind: ProposalSourceKind::Finding,
            source_id: finding_id.clone(),
            field_id: Some("field-1".to_string()),
            title: "Scout pest hotspot".to_string(),
            action_category: "scout".to_string(),
            priority: "high".to_string(),
            rationale: None,
        },
        T0,
    )
    .await?;
    let accepted = proposal_queue::decide(
        &pool,
        &proposal.proposal_id,
        ProposalDecision::Accept,
        "agronomist-1",
        T0,
    )
    .await?;

    // Draft -> dry-run -> distinct operator approves.
    let mission_draft = draft_mission_for_proposal(&accepted)?;
    let dry_run = dry_run_mission_dispatch(&mission_draft);
    let approval = authorize_mission_dispatch(
        &dry_run,
        &OperatorApproval {
            operator_id: "operator-9".to_string(),
            approved: true,
            approved_at: T0.to_string(),
        },
    )?;
    assert!(approval.dispatch_authorized);

    // Governed dispatch invokes guarded_dispatch on a real armed mission.
    let mission = armed_mission();
    let context = dispatch_context(&mission);
    let command = GuardedDispatchCommand {
        correlation_id: Uuid::new_v4(),
        mavlink_command: MAV_CMD_NAV_TAKEOFF,
        label: "takeoff".to_string(),
    };
    let mut tracker = MAVLinkCommandAckTracker::default();
    let outcome = proposal_dispatch::governed_dispatch(
        &pool,
        &approval,
        &mission,
        command,
        context,
        &mut tracker,
        T0,
    )
    .await?;
    assert!(!outcome.audit.is_empty(), "guarded dispatch produced audit");

    // The dispatch recorded an Action that traces back to the proposal and L0.
    let action_id = proposal_dispatch::action_id_for(&approval);
    let trace = provenance_store::trace_backward(&pool, &action_id).await?;
    assert!(trace.gaps.is_empty(), "action->L0 gap-free: {:?}", trace.gaps);
    assert!(
        trace
            .records
            .iter()
            .any(|r| r.artifact_id == proposal.proposal_id),
        "action traces through the proposal"
    );
    assert!(
        trace.records.iter().any(|r| r.artifact_id == l2),
        "action traces down to the L2 product"
    );
    Ok(())
}

#[tokio::test]
async fn unauthorized_approval_never_dispatches() -> Result<()> {
    let tmp = TempDir::new()?;
    let pool = pool_ctx(&tmp).await?;
    let (_l2, finding_id) = seed_finding(&pool).await?;

    let proposal = proposal_queue::create_proposal(
        &pool,
        &ProposalCreateRequest {
            source_kind: ProposalSourceKind::Finding,
            source_id: finding_id,
            field_id: Some("field-1".to_string()),
            title: "Scout pest hotspot".to_string(),
            action_category: "scout".to_string(),
            priority: "high".to_string(),
            rationale: None,
        },
        T0,
    )
    .await?;
    let accepted = proposal_queue::decide(
        &pool,
        &proposal.proposal_id,
        ProposalDecision::Accept,
        "agronomist-1",
        T0,
    )
    .await?;
    let mission_draft = draft_mission_for_proposal(&accepted)?;
    let dry_run = dry_run_mission_dispatch(&mission_draft);
    // The reviewer's own (rejected) sign-off never authorizes.
    let approval = authorize_mission_dispatch(
        &dry_run,
        &OperatorApproval {
            operator_id: "operator-9".to_string(),
            approved: false,
            approved_at: T0.to_string(),
        },
    )?;
    assert!(!approval.dispatch_authorized);

    let mission = armed_mission();
    let context = dispatch_context(&mission);
    let command = GuardedDispatchCommand {
        correlation_id: Uuid::new_v4(),
        mavlink_command: MAV_CMD_NAV_TAKEOFF,
        label: "takeoff".to_string(),
    };
    let mut tracker = MAVLinkCommandAckTracker::default();
    let result = proposal_dispatch::governed_dispatch(
        &pool,
        &approval,
        &mission,
        command,
        context,
        &mut tracker,
        T0,
    )
    .await;
    assert!(result.is_err(), "unauthorized approval must not dispatch");

    // And no Action was written to the ledger.
    let action_id = proposal_dispatch::action_id_for(&approval);
    assert!(provenance_store::get_lineage(&pool, &action_id)
        .await?
        .is_none());
    Ok(())
}
