use crate::{swarm::generate_formation_slots, Formation};
use anyhow::{ensure, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use shared::GeoCoordinate;
use std::collections::HashMap;
use uuid::Uuid;

const EARTH_RADIUS_M: f64 = 6_371_000.0;
const GEOMETRY_EPSILON: f64 = 1e-9;

/// Multi-drone coordination system
pub struct CoordinationEngine {
    active_drones: HashMap<Uuid, DroneState>,
    coordination_rules: Vec<CoordinationRule>,
    coordination_rule_audit_log: Vec<CoordinationRuleAuditEvent>,
    communication_range: f64, // meters
    update_interval: std::time::Duration,
    telemetry_freshness_timeout: ChronoDuration,
    heartbeat_timeout: ChronoDuration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneState {
    pub id: Uuid,
    pub position: GeoCoordinate,
    pub velocity: (f32, f32, f32),
    pub heading: f32,
    pub battery_level: f32,
    pub status: DroneOperationStatus,
    pub current_mission: Option<Uuid>,
    pub last_update: DateTime<Utc>,
    pub communication_quality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DroneOperationStatus {
    Idle,
    InTransit,
    ExecutingMission,
    Returning,
    Emergency,
    Maintenance,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DroneTelemetryFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SwarmTelemetryStatus {
    NoDrones,
    Fresh,
    Degraded,
    Stale,
}

impl Default for SwarmTelemetryStatus {
    fn default() -> Self {
        Self::NoDrones
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneTelemetrySnapshot {
    pub drone_id: Uuid,
    pub position: GeoCoordinate,
    pub battery_level: f32,
    pub operation_status: DroneOperationStatus,
    pub current_mission: Option<Uuid>,
    pub last_update: DateTime<Utc>,
    pub age_seconds: i64,
    pub communication_quality: f32,
    pub freshness: DroneTelemetryFreshness,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTelemetryReport {
    pub generated_at: DateTime<Utc>,
    pub total_drones: usize,
    pub fresh_drones: usize,
    pub stale_drones: usize,
    pub status: SwarmTelemetryStatus,
    pub drones: Vec<DroneTelemetrySnapshot>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DroneLinkHealth {
    Healthy,
    Degraded,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneLinkStatus {
    pub drone_id: Uuid,
    pub last_heartbeat: DateTime<Utc>,
    pub heartbeat_age_seconds: i64,
    pub link_quality: f32,
    pub health: DroneLinkHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationRuleAuditEvent {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub drone_id: Option<Uuid>,
    pub triggered_at: DateTime<Utc>,
    pub condition: String,
    pub action: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkQualityReport {
    pub generated_at: DateTime<Utc>,
    pub total_links: usize,
    pub healthy_links: usize,
    pub degraded_links: usize,
    pub timed_out_links: usize,
    pub links: Vec<DroneLinkStatus>,
    pub audit_events: Vec<CoordinationRuleAuditEvent>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FormationOptimizationConfig {
    pub minimum_separation_m: f64,
}

impl Default for FormationOptimizationConfig {
    fn default() -> Self {
        Self {
            minimum_separation_m: 25.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationAssignment {
    pub drone_id: Uuid,
    pub slot_index: usize,
    pub start_position: GeoCoordinate,
    pub target_position: GeoCoordinate,
    pub travel_distance_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationOptimizationReport {
    pub assignments: Vec<FormationAssignment>,
    pub total_travel_m: f64,
    pub minimum_path_separation_m: f64,
    pub rejected_assignment_count: usize,
    pub validated: bool,
}

#[derive(Debug, Clone, Copy)]
struct LocalPoint {
    east_m: f64,
    north_m: f64,
    altitude_m: f64,
}

#[derive(Debug, Clone)]
struct FormationSlotTarget {
    slot_index: usize,
    local: LocalPoint,
    geo: GeoCoordinate,
}

#[derive(Debug, Clone)]
struct AssignmentCandidate {
    drone_id: Uuid,
    slot_index: usize,
    start_local: LocalPoint,
    target_local: LocalPoint,
    start_position: GeoCoordinate,
    target_position: GeoCoordinate,
    travel_distance_m: f64,
}

#[derive(Debug, Clone)]
struct AssignmentSearchResult {
    assignments: Vec<AssignmentCandidate>,
    total_travel_m: f64,
}

pub struct CoordinationRule {
    pub id: Uuid,
    pub name: String,
    pub priority: u8,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub enabled: bool,
}

impl Clone for CoordinationRule {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            priority: self.priority,
            condition: self.condition.clone(),
            action: self.action.clone(),
            enabled: self.enabled,
        }
    }
}

impl std::fmt::Debug for CoordinationRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoordinationRule")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("priority", &self.priority)
            .field("condition", &format_args!("RuleCondition"))
            .field("action", &format_args!("RuleAction"))
            .field("enabled", &self.enabled)
            .finish()
    }
}

pub enum RuleCondition {
    ProximityAlert { distance_threshold: f64 },
    BatteryLow { threshold: f32 },
    CommunicationLoss { timeout_seconds: u32 },
    WeatherCondition { condition: String },
    Custom(Box<dyn Fn(&[DroneState]) -> bool + Send + Sync>),
}

impl Clone for RuleCondition {
    fn clone(&self) -> Self {
        match self {
            RuleCondition::ProximityAlert { distance_threshold } => RuleCondition::ProximityAlert {
                distance_threshold: *distance_threshold,
            },
            RuleCondition::BatteryLow { threshold } => RuleCondition::BatteryLow {
                threshold: *threshold,
            },
            RuleCondition::CommunicationLoss { timeout_seconds } => {
                RuleCondition::CommunicationLoss {
                    timeout_seconds: *timeout_seconds,
                }
            }
            RuleCondition::WeatherCondition { condition } => RuleCondition::WeatherCondition {
                condition: condition.clone(),
            },
            RuleCondition::Custom(_) => {
                // Cannot clone functions, so create a dummy one
                RuleCondition::Custom(Box::new(|_| false))
            }
        }
    }
}

impl std::fmt::Debug for RuleCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleCondition::ProximityAlert { distance_threshold } => f
                .debug_struct("ProximityAlert")
                .field("distance_threshold", distance_threshold)
                .finish(),
            RuleCondition::BatteryLow { threshold } => f
                .debug_struct("BatteryLow")
                .field("threshold", threshold)
                .finish(),
            RuleCondition::CommunicationLoss { timeout_seconds } => f
                .debug_struct("CommunicationLoss")
                .field("timeout_seconds", timeout_seconds)
                .finish(),
            RuleCondition::WeatherCondition { condition } => f
                .debug_struct("WeatherCondition")
                .field("condition", condition)
                .finish(),
            RuleCondition::Custom(_) => f
                .debug_struct("Custom")
                .field("function", &"<closure>")
                .finish(),
        }
    }
}

pub enum RuleAction {
    ChangeSpeed { factor: f32 },
    ChangeAltitude { delta: f32 },
    ReturnToBase,
    LandImmediate,
    FormFormation { formation_type: String },
    SendAlert { message: String },
    Custom(Box<dyn Fn(&mut DroneState) + Send + Sync>),
}

impl Clone for RuleAction {
    fn clone(&self) -> Self {
        match self {
            RuleAction::ChangeSpeed { factor } => RuleAction::ChangeSpeed { factor: *factor },
            RuleAction::ChangeAltitude { delta } => RuleAction::ChangeAltitude { delta: *delta },
            RuleAction::ReturnToBase => RuleAction::ReturnToBase,
            RuleAction::LandImmediate => RuleAction::LandImmediate,
            RuleAction::FormFormation { formation_type } => RuleAction::FormFormation {
                formation_type: formation_type.clone(),
            },
            RuleAction::SendAlert { message } => RuleAction::SendAlert {
                message: message.clone(),
            },
            RuleAction::Custom(_) => {
                // Cannot clone functions, so create a dummy one
                RuleAction::Custom(Box::new(|_| {}))
            }
        }
    }
}

impl std::fmt::Debug for RuleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleAction::ChangeSpeed { factor } => f
                .debug_struct("ChangeSpeed")
                .field("factor", factor)
                .finish(),
            RuleAction::ChangeAltitude { delta } => f
                .debug_struct("ChangeAltitude")
                .field("delta", delta)
                .finish(),
            RuleAction::ReturnToBase => f.debug_struct("ReturnToBase").finish(),
            RuleAction::LandImmediate => f.debug_struct("LandImmediate").finish(),
            RuleAction::FormFormation { formation_type } => f
                .debug_struct("FormFormation")
                .field("formation_type", formation_type)
                .finish(),
            RuleAction::SendAlert { message } => f
                .debug_struct("SendAlert")
                .field("message", message)
                .finish(),
            RuleAction::Custom(_) => f
                .debug_struct("Custom")
                .field("function", &"<closure>")
                .finish(),
        }
    }
}

impl CoordinationEngine {
    pub fn new() -> Self {
        Self {
            active_drones: HashMap::new(),
            coordination_rules: Self::default_rules(),
            coordination_rule_audit_log: Vec::new(),
            communication_range: 1000.0, // 1km
            update_interval: std::time::Duration::from_millis(500),
            telemetry_freshness_timeout: ChronoDuration::seconds(5),
            heartbeat_timeout: ChronoDuration::seconds(30),
        }
    }

    fn default_rules() -> Vec<CoordinationRule> {
        vec![
            CoordinationRule {
                id: Uuid::new_v4(),
                name: "Collision Avoidance".to_string(),
                priority: 1,
                condition: RuleCondition::ProximityAlert {
                    distance_threshold: 50.0,
                },
                action: RuleAction::ChangeAltitude { delta: 10.0 },
                enabled: true,
            },
            CoordinationRule {
                id: Uuid::new_v4(),
                name: "Low Battery Return".to_string(),
                priority: 2,
                condition: RuleCondition::BatteryLow { threshold: 0.2 },
                action: RuleAction::ReturnToBase,
                enabled: true,
            },
            CoordinationRule {
                id: Uuid::new_v4(),
                name: "Communication Loss".to_string(),
                priority: 1,
                condition: RuleCondition::CommunicationLoss {
                    timeout_seconds: 30,
                },
                action: RuleAction::LandImmediate,
                enabled: true,
            },
        ]
    }

    pub async fn register_drone(
        &mut self,
        drone_id: Uuid,
        initial_state: DroneState,
    ) -> Result<()> {
        self.active_drones.insert(drone_id, initial_state);
        tracing::info!("Registered drone {} for coordination", drone_id);
        Ok(())
    }

    pub async fn unregister_drone(&mut self, drone_id: Uuid) -> Result<()> {
        self.active_drones.remove(&drone_id);
        tracing::info!("Unregistered drone {} from coordination", drone_id);
        Ok(())
    }

    pub async fn update_drone_state(&mut self, drone_id: Uuid, state: DroneState) -> Result<()> {
        if let Some(existing_state) = self.active_drones.get_mut(&drone_id) {
            *existing_state = state;

            // Check coordination rules
            self.evaluate_rules(drone_id).await?;
        } else {
            return Err(anyhow::anyhow!("Drone not registered: {}", drone_id));
        }
        Ok(())
    }

    pub fn set_telemetry_freshness_timeout(&mut self, timeout: ChronoDuration) -> Result<()> {
        ensure!(
            timeout > ChronoDuration::zero(),
            "telemetry freshness timeout must be positive"
        );
        self.telemetry_freshness_timeout = timeout;
        Ok(())
    }

    pub fn set_heartbeat_timeout(&mut self, timeout: ChronoDuration) -> Result<()> {
        ensure!(
            timeout > ChronoDuration::zero(),
            "heartbeat timeout must be positive"
        );
        self.heartbeat_timeout = timeout;
        for rule in &mut self.coordination_rules {
            if let RuleCondition::CommunicationLoss { timeout_seconds } = &mut rule.condition {
                *timeout_seconds = timeout.num_seconds().max(1) as u32;
            }
        }
        Ok(())
    }

    pub async fn record_heartbeat(
        &mut self,
        drone_id: Uuid,
        received_at: DateTime<Utc>,
        link_quality: f32,
    ) -> Result<DroneLinkStatus> {
        ensure!(
            link_quality.is_finite() && (0.0..=1.0).contains(&link_quality),
            "link quality must be finite and between 0.0 and 1.0"
        );
        let snapshot = {
            let state = self
                .active_drones
                .get_mut(&drone_id)
                .ok_or_else(|| anyhow::anyhow!("Drone not registered: {}", drone_id))?;
            state.last_update = received_at;
            state.communication_quality = link_quality;
            state.clone()
        };
        Ok(self.link_status_for(&snapshot, received_at))
    }

    pub fn coordination_rule_audit_log(&self) -> &[CoordinationRuleAuditEvent] {
        &self.coordination_rule_audit_log
    }

    pub async fn swarm_telemetry_report_at(
        &self,
        checked_at: DateTime<Utc>,
    ) -> SwarmTelemetryReport {
        self.build_swarm_telemetry_report(checked_at)
    }

    fn build_swarm_telemetry_report(&self, checked_at: DateTime<Utc>) -> SwarmTelemetryReport {
        let mut drones = self
            .active_drones
            .values()
            .map(|state| {
                let age_seconds = Self::telemetry_age_seconds(state.last_update, checked_at);
                let freshness = self.telemetry_freshness_for(state, checked_at);
                DroneTelemetrySnapshot {
                    drone_id: state.id,
                    position: state.position.clone(),
                    battery_level: state.battery_level,
                    operation_status: state.status.clone(),
                    current_mission: state.current_mission,
                    last_update: state.last_update,
                    age_seconds,
                    communication_quality: state.communication_quality,
                    freshness,
                }
            })
            .collect::<Vec<_>>();
        drones.sort_by_key(|drone| drone.drone_id);

        let total_drones = drones.len();
        let fresh_drones = drones
            .iter()
            .filter(|drone| drone.freshness == DroneTelemetryFreshness::Fresh)
            .count();
        let stale_drones = total_drones - fresh_drones;

        SwarmTelemetryReport {
            generated_at: checked_at,
            total_drones,
            fresh_drones,
            stale_drones,
            status: Self::aggregate_telemetry_status(total_drones, fresh_drones, stale_drones),
            drones,
        }
    }

    fn telemetry_freshness_for(
        &self,
        state: &DroneState,
        checked_at: DateTime<Utc>,
    ) -> DroneTelemetryFreshness {
        let age = checked_at.signed_duration_since(state.last_update);
        if age <= self.telemetry_freshness_timeout {
            DroneTelemetryFreshness::Fresh
        } else {
            DroneTelemetryFreshness::Stale
        }
    }

    fn telemetry_age_seconds(last_update: DateTime<Utc>, checked_at: DateTime<Utc>) -> i64 {
        checked_at
            .signed_duration_since(last_update)
            .num_seconds()
            .max(0)
    }

    fn aggregate_telemetry_status(
        total_drones: usize,
        fresh_drones: usize,
        stale_drones: usize,
    ) -> SwarmTelemetryStatus {
        match (total_drones, fresh_drones, stale_drones) {
            (0, _, _) => SwarmTelemetryStatus::NoDrones,
            (_, _, 0) => SwarmTelemetryStatus::Fresh,
            (_, 0, _) => SwarmTelemetryStatus::Stale,
            _ => SwarmTelemetryStatus::Degraded,
        }
    }

    pub async fn evaluate_link_rules_at(
        &mut self,
        checked_at: DateTime<Utc>,
    ) -> Result<LinkQualityReport> {
        let mut report = self.build_link_quality_report(checked_at);
        let mut audit_events = Vec::new();

        for rule in self.coordination_rules.iter().filter(|rule| {
            rule.enabled && matches!(rule.condition, RuleCondition::CommunicationLoss { .. })
        }) {
            for link in report
                .links
                .iter()
                .filter(|link| link.health == DroneLinkHealth::TimedOut)
            {
                audit_events.push(CoordinationRuleAuditEvent {
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    drone_id: Some(link.drone_id),
                    triggered_at: checked_at,
                    condition: "communication_loss".to_string(),
                    action: Self::rule_action_label(&rule.action).to_string(),
                    message: format!(
                        "Drone {} heartbeat timed out after {}s",
                        link.drone_id, link.heartbeat_age_seconds
                    ),
                });
            }
        }

        self.coordination_rule_audit_log
            .extend(audit_events.iter().cloned());
        report.audit_events = audit_events;
        Ok(report)
    }

    fn build_link_quality_report(&self, checked_at: DateTime<Utc>) -> LinkQualityReport {
        let mut links = self
            .active_drones
            .values()
            .map(|state| self.link_status_for(state, checked_at))
            .collect::<Vec<_>>();
        links.sort_by_key(|link| link.drone_id);

        let healthy_links = links
            .iter()
            .filter(|link| link.health == DroneLinkHealth::Healthy)
            .count();
        let degraded_links = links
            .iter()
            .filter(|link| link.health == DroneLinkHealth::Degraded)
            .count();
        let timed_out_links = links
            .iter()
            .filter(|link| link.health == DroneLinkHealth::TimedOut)
            .count();

        LinkQualityReport {
            generated_at: checked_at,
            total_links: links.len(),
            healthy_links,
            degraded_links,
            timed_out_links,
            links,
            audit_events: Vec::new(),
        }
    }

    fn link_status_for(&self, state: &DroneState, checked_at: DateTime<Utc>) -> DroneLinkStatus {
        let heartbeat_age_seconds = Self::telemetry_age_seconds(state.last_update, checked_at);
        let health = if checked_at.signed_duration_since(state.last_update) > self.heartbeat_timeout
        {
            DroneLinkHealth::TimedOut
        } else if state.communication_quality >= 0.7 {
            DroneLinkHealth::Healthy
        } else {
            DroneLinkHealth::Degraded
        };

        DroneLinkStatus {
            drone_id: state.id,
            last_heartbeat: state.last_update,
            heartbeat_age_seconds,
            link_quality: state.communication_quality,
            health,
        }
    }

    fn rule_action_label(action: &RuleAction) -> &'static str {
        match action {
            RuleAction::ChangeSpeed { .. } => "ChangeSpeed",
            RuleAction::ChangeAltitude { .. } => "ChangeAltitude",
            RuleAction::ReturnToBase => "ReturnToBase",
            RuleAction::LandImmediate => "LandImmediate",
            RuleAction::FormFormation { .. } => "FormFormation",
            RuleAction::SendAlert { .. } => "SendAlert",
            RuleAction::Custom(_) => "Custom",
        }
    }

    pub async fn evaluate_rules(&self, _drone_id: Uuid) -> Result<()> {
        let states: Vec<&DroneState> = self.active_drones.values().collect();

        for rule in &self.coordination_rules {
            if !rule.enabled {
                continue;
            }

            let should_trigger = match &rule.condition {
                RuleCondition::ProximityAlert { distance_threshold } => {
                    self.check_proximity_violations(*distance_threshold, &states)
                }
                RuleCondition::BatteryLow { threshold } => {
                    states.iter().any(|s| s.battery_level < *threshold)
                }
                RuleCondition::CommunicationLoss { timeout_seconds } => {
                    let timeout = chrono::Duration::seconds(*timeout_seconds as i64);
                    let cutoff = Utc::now() - timeout;
                    states.iter().any(|s| s.last_update < cutoff)
                }
                RuleCondition::WeatherCondition { condition: _ } => {
                    // TODO: Integrate with weather data
                    false
                }
                RuleCondition::Custom(_) => {
                    // TODO: Implement custom rule evaluation
                    false
                }
            };

            if should_trigger {
                tracing::warn!("Coordination rule triggered: {}", rule.name);
                // TODO: Execute rule action
            }
        }

        Ok(())
    }

    fn check_proximity_violations(&self, threshold: f64, states: &[&DroneState]) -> bool {
        for (i, state1) in states.iter().enumerate() {
            for state2 in states.iter().skip(i + 1) {
                let distance = self.calculate_distance(&state1.position, &state2.position);
                if distance < threshold {
                    return true;
                }
            }
        }
        false
    }

    fn calculate_distance(&self, pos1: &GeoCoordinate, pos2: &GeoCoordinate) -> f64 {
        let lat1 = pos1.latitude.to_radians();
        let lat2 = pos2.latitude.to_radians();
        let delta_lat = (pos2.latitude - pos1.latitude).to_radians();
        let delta_lon = (pos2.longitude - pos1.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0_f64 * a.sqrt().atan2((1.0 - a).sqrt());

        6371000.0 * c // Earth radius in meters
    }

    pub async fn get_coordination_status(&self) -> CoordinationStatus {
        self.get_coordination_status_at(Utc::now()).await
    }

    pub async fn get_coordination_status_at(
        &self,
        checked_at: DateTime<Utc>,
    ) -> CoordinationStatus {
        let total_drones = self.active_drones.len();
        let active_drones = self
            .active_drones
            .values()
            .filter(|s| !matches!(s.status, DroneOperationStatus::Maintenance))
            .count();

        let coordination_quality = if total_drones > 0 {
            self.active_drones
                .values()
                .map(|s| s.communication_quality)
                .sum::<f32>()
                / total_drones as f32
        } else {
            1.0
        };
        let telemetry_report = self.build_swarm_telemetry_report(checked_at);

        CoordinationStatus {
            total_drones,
            active_drones,
            fresh_drones: telemetry_report.fresh_drones,
            stale_drones: telemetry_report.stale_drones,
            telemetry_status: telemetry_report.status,
            coordination_quality,
            active_rules: self.coordination_rules.iter().filter(|r| r.enabled).count(),
            last_update: checked_at,
        }
    }

    pub async fn optimize_formations(&mut self) -> Result<FormationOptimizationReport> {
        if self.active_drones.is_empty() {
            tracing::info!("Formation optimization skipped because no drones are active");
            return Ok(FormationOptimizationReport {
                assignments: Vec::new(),
                total_travel_m: 0.0,
                minimum_path_separation_m: 0.0,
                rejected_assignment_count: 0,
                validated: true,
            });
        }

        let mut drones = self.active_drones.values().collect::<Vec<_>>();
        drones.sort_by_key(|state| state.id);
        let leader_position = drones[0].position.clone();
        let drone_count = drones.len();
        let cols = (drone_count as f64).sqrt().ceil() as u32;
        let rows = ((drone_count as f64) / f64::from(cols)).ceil() as u32;
        let formation = Formation::Grid {
            rows,
            cols,
            spacing_m: FormationOptimizationConfig::default().minimum_separation_m as f32,
        };

        let report = self
            .optimize_formation_slots(
                &formation,
                leader_position,
                FormationOptimizationConfig::default(),
            )
            .await?;
        tracing::info!(
            assignments = report.assignments.len(),
            total_travel_m = report.total_travel_m,
            minimum_path_separation_m = report.minimum_path_separation_m,
            "Optimized active formation slot assignment"
        );
        Ok(report)
    }

    pub async fn optimize_formation_slots(
        &self,
        formation: &Formation,
        leader_position: GeoCoordinate,
        config: FormationOptimizationConfig,
    ) -> Result<FormationOptimizationReport> {
        ensure!(
            config.minimum_separation_m.is_finite() && config.minimum_separation_m > 0.0,
            "minimum formation separation must be finite and positive"
        );
        ensure!(
            leader_position.latitude.is_finite()
                && leader_position.longitude.is_finite()
                && leader_position.altitude_m.is_finite(),
            "leader position must be finite"
        );
        ensure!(
            !self.active_drones.is_empty(),
            "formation optimization requires at least one active drone"
        );

        let mut drones = self.active_drones.values().collect::<Vec<_>>();
        drones.sort_by_key(|state| state.id);
        let slots = generate_formation_slots(formation, drones.len(), config.minimum_separation_m)?;
        let mut slot_targets = slots
            .into_iter()
            .map(|slot| {
                let local = LocalPoint {
                    east_m: slot.offset_m.0,
                    north_m: slot.offset_m.1,
                    altitude_m: f64::from(slot.offset_m.2),
                };
                FormationSlotTarget {
                    slot_index: slot.slot_index,
                    local,
                    geo: local_to_geo(&leader_position, local),
                }
            })
            .collect::<Vec<_>>();
        slot_targets.sort_by_key(|slot| slot.slot_index);

        let mut used_slots = vec![false; slot_targets.len()];
        let mut current = Vec::with_capacity(drones.len());
        let mut rejected_assignment_count = 0;
        let mut best = None;
        Self::search_assignments(
            &drones,
            &slot_targets,
            &leader_position,
            config.minimum_separation_m,
            0,
            0.0,
            &mut used_slots,
            &mut current,
            &mut best,
            &mut rejected_assignment_count,
        );

        let best = best.ok_or_else(|| {
            anyhow::anyhow!("no separation-respecting formation assignment found")
        })?;
        let minimum_path_separation_m = Self::minimum_assignment_separation(&best.assignments);
        let validated =
            best.assignments.len() < 2 || minimum_path_separation_m >= config.minimum_separation_m;
        let assignments = best
            .assignments
            .into_iter()
            .map(|assignment| FormationAssignment {
                drone_id: assignment.drone_id,
                slot_index: assignment.slot_index,
                start_position: assignment.start_position,
                target_position: assignment.target_position,
                travel_distance_m: assignment.travel_distance_m,
            })
            .collect::<Vec<_>>();

        Ok(FormationOptimizationReport {
            assignments,
            total_travel_m: best.total_travel_m,
            minimum_path_separation_m,
            rejected_assignment_count,
            validated,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn search_assignments(
        drones: &[&DroneState],
        slot_targets: &[FormationSlotTarget],
        leader_position: &GeoCoordinate,
        minimum_separation_m: f64,
        drone_index: usize,
        current_cost: f64,
        used_slots: &mut [bool],
        current: &mut Vec<AssignmentCandidate>,
        best: &mut Option<AssignmentSearchResult>,
        rejected_assignment_count: &mut usize,
    ) {
        if let Some(best) = best {
            if current_cost >= best.total_travel_m {
                return;
            }
        }

        if drone_index == drones.len() {
            *best = Some(AssignmentSearchResult {
                assignments: current.clone(),
                total_travel_m: current_cost,
            });
            return;
        }

        let drone = drones[drone_index];
        let start_local = geo_to_local(leader_position, &drone.position);
        for slot_index in 0..slot_targets.len() {
            if used_slots[slot_index] {
                continue;
            }
            let slot = &slot_targets[slot_index];
            let candidate = AssignmentCandidate {
                drone_id: drone.id,
                slot_index: slot.slot_index,
                start_local,
                target_local: slot.local,
                start_position: drone.position.clone(),
                target_position: slot.geo.clone(),
                travel_distance_m: local_distance(start_local, slot.local),
            };
            if !Self::candidate_is_safe(current, &candidate, minimum_separation_m) {
                *rejected_assignment_count += 1;
                continue;
            }

            used_slots[slot_index] = true;
            current.push(candidate);
            Self::search_assignments(
                drones,
                slot_targets,
                leader_position,
                minimum_separation_m,
                drone_index + 1,
                current_cost
                    + current
                        .last()
                        .expect("candidate was pushed")
                        .travel_distance_m,
                used_slots,
                current,
                best,
                rejected_assignment_count,
            );
            current.pop();
            used_slots[slot_index] = false;
        }
    }

    fn candidate_is_safe(
        current: &[AssignmentCandidate],
        candidate: &AssignmentCandidate,
        minimum_separation_m: f64,
    ) -> bool {
        current.iter().all(|existing| {
            assignment_pair_minimum_distance(existing, candidate) + GEOMETRY_EPSILON
                >= minimum_separation_m
        })
    }

    fn minimum_assignment_separation(assignments: &[AssignmentCandidate]) -> f64 {
        if assignments.len() < 2 {
            return 0.0;
        }
        let mut minimum = f64::INFINITY;
        for left_index in 0..assignments.len() {
            for right_index in (left_index + 1)..assignments.len() {
                minimum = minimum.min(assignment_pair_minimum_distance(
                    &assignments[left_index],
                    &assignments[right_index],
                ));
            }
        }
        minimum
    }

    pub async fn handle_emergency(
        &mut self,
        drone_id: Uuid,
        emergency_type: EmergencyType,
    ) -> Result<()> {
        if let Some(state) = self.active_drones.get_mut(&drone_id) {
            state.status = DroneOperationStatus::Emergency;

            match emergency_type {
                EmergencyType::BatteryDepleted => {
                    tracing::error!("Battery depleted for drone {}", drone_id);
                }
                EmergencyType::SystemFailure => {
                    tracing::error!("System failure for drone {}", drone_id);
                }
                EmergencyType::WeatherHazard => {
                    tracing::error!("Weather hazard affecting drone {}", drone_id);
                }
                EmergencyType::CollisionRisk => {
                    tracing::error!("Collision risk for drone {}", drone_id);
                }
            }
        }

        Ok(())
    }

    pub async fn execute_action(&mut self, _swarm_id: Uuid, _action: String) -> Result<()> {
        // TODO: Implement action execution logic
        tracing::info!("Action execution not yet implemented");
        Ok(())
    }
}

fn geo_to_local(origin: &GeoCoordinate, position: &GeoCoordinate) -> LocalPoint {
    let origin_lat_rad = origin.latitude.to_radians();
    LocalPoint {
        east_m: (position.longitude - origin.longitude).to_radians()
            * EARTH_RADIUS_M
            * origin_lat_rad.cos(),
        north_m: (position.latitude - origin.latitude).to_radians() * EARTH_RADIUS_M,
        altitude_m: f64::from(position.altitude_m - origin.altitude_m),
    }
}

fn local_to_geo(origin: &GeoCoordinate, local: LocalPoint) -> GeoCoordinate {
    let origin_lat_rad = origin.latitude.to_radians();
    GeoCoordinate {
        latitude: origin.latitude + (local.north_m / EARTH_RADIUS_M).to_degrees(),
        longitude: origin.longitude
            + (local.east_m / (EARTH_RADIUS_M * origin_lat_rad.cos())).to_degrees(),
        altitude_m: origin.altitude_m + local.altitude_m as f32,
    }
}

fn local_distance(left: LocalPoint, right: LocalPoint) -> f64 {
    ((left.east_m - right.east_m).powi(2)
        + (left.north_m - right.north_m).powi(2)
        + (left.altitude_m - right.altitude_m).powi(2))
    .sqrt()
}

fn assignment_pair_minimum_distance(
    left: &AssignmentCandidate,
    right: &AssignmentCandidate,
) -> f64 {
    let synchronous_distance = minimum_synchronous_path_distance(
        left.start_local,
        left.target_local,
        right.start_local,
        right.target_local,
    );
    if segments_intersect_2d(
        left.start_local,
        left.target_local,
        right.start_local,
        right.target_local,
    ) {
        synchronous_distance.min(0.0)
    } else {
        synchronous_distance
    }
}

fn minimum_synchronous_path_distance(
    left_start: LocalPoint,
    left_target: LocalPoint,
    right_start: LocalPoint,
    right_target: LocalPoint,
) -> f64 {
    let relative_start = LocalPoint {
        east_m: left_start.east_m - right_start.east_m,
        north_m: left_start.north_m - right_start.north_m,
        altitude_m: left_start.altitude_m - right_start.altitude_m,
    };
    let relative_velocity = LocalPoint {
        east_m: (left_target.east_m - left_start.east_m)
            - (right_target.east_m - right_start.east_m),
        north_m: (left_target.north_m - left_start.north_m)
            - (right_target.north_m - right_start.north_m),
        altitude_m: (left_target.altitude_m - left_start.altitude_m)
            - (right_target.altitude_m - right_start.altitude_m),
    };
    let velocity_norm = relative_velocity.east_m.powi(2)
        + relative_velocity.north_m.powi(2)
        + relative_velocity.altitude_m.powi(2);
    let closest_t = if velocity_norm <= GEOMETRY_EPSILON {
        0.0
    } else {
        -((relative_start.east_m * relative_velocity.east_m)
            + (relative_start.north_m * relative_velocity.north_m)
            + (relative_start.altitude_m * relative_velocity.altitude_m))
            / velocity_norm
    }
    .clamp(0.0, 1.0);

    let closest = LocalPoint {
        east_m: relative_start.east_m + relative_velocity.east_m * closest_t,
        north_m: relative_start.north_m + relative_velocity.north_m * closest_t,
        altitude_m: relative_start.altitude_m + relative_velocity.altitude_m * closest_t,
    };
    local_distance(
        closest,
        LocalPoint {
            east_m: 0.0,
            north_m: 0.0,
            altitude_m: 0.0,
        },
    )
}

fn segments_intersect_2d(
    left_start: LocalPoint,
    left_target: LocalPoint,
    right_start: LocalPoint,
    right_target: LocalPoint,
) -> bool {
    if point_distance_2d(left_start, left_target) <= GEOMETRY_EPSILON
        && point_distance_2d(right_start, right_target) <= GEOMETRY_EPSILON
    {
        return point_distance_2d(left_start, right_start) <= GEOMETRY_EPSILON;
    }

    let o1 = orientation_2d(left_start, left_target, right_start);
    let o2 = orientation_2d(left_start, left_target, right_target);
    let o3 = orientation_2d(right_start, right_target, left_start);
    let o4 = orientation_2d(right_start, right_target, left_target);

    if o1.abs() <= GEOMETRY_EPSILON && on_segment_2d(left_start, right_start, left_target) {
        return true;
    }
    if o2.abs() <= GEOMETRY_EPSILON && on_segment_2d(left_start, right_target, left_target) {
        return true;
    }
    if o3.abs() <= GEOMETRY_EPSILON && on_segment_2d(right_start, left_start, right_target) {
        return true;
    }
    if o4.abs() <= GEOMETRY_EPSILON && on_segment_2d(right_start, left_target, right_target) {
        return true;
    }

    ((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0))
        && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0))
}

fn orientation_2d(a: LocalPoint, b: LocalPoint, c: LocalPoint) -> f64 {
    (b.east_m - a.east_m) * (c.north_m - a.north_m)
        - (b.north_m - a.north_m) * (c.east_m - a.east_m)
}

fn on_segment_2d(a: LocalPoint, b: LocalPoint, c: LocalPoint) -> bool {
    b.east_m >= a.east_m.min(c.east_m) - GEOMETRY_EPSILON
        && b.east_m <= a.east_m.max(c.east_m) + GEOMETRY_EPSILON
        && b.north_m >= a.north_m.min(c.north_m) - GEOMETRY_EPSILON
        && b.north_m <= a.north_m.max(c.north_m) + GEOMETRY_EPSILON
}

fn point_distance_2d(left: LocalPoint, right: LocalPoint) -> f64 {
    ((left.east_m - right.east_m).powi(2) + (left.north_m - right.north_m).powi(2)).sqrt()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationStatus {
    pub total_drones: usize,
    pub active_drones: usize,
    #[serde(default)]
    pub fresh_drones: usize,
    #[serde(default)]
    pub stale_drones: usize,
    #[serde(default)]
    pub telemetry_status: SwarmTelemetryStatus,
    pub coordination_quality: f32,
    pub active_rules: usize,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmergencyType {
    BatteryDepleted,
    SystemFailure,
    WeatherHazard,
    CollisionRisk,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-13T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn test_state(drone_id: Uuid, last_update: DateTime<Utc>, battery_level: f32) -> DroneState {
        DroneState {
            id: drone_id,
            position: GeoCoordinate {
                latitude: 40.7128,
                longitude: -74.0060,
                altitude_m: 100.0,
            },
            velocity: (5.0, 0.0, 0.0),
            heading: 0.0,
            battery_level,
            status: DroneOperationStatus::ExecutingMission,
            current_mission: None,
            last_update,
            communication_quality: 0.95,
        }
    }

    fn test_state_at(
        drone_id: Uuid,
        position: GeoCoordinate,
        last_update: DateTime<Utc>,
    ) -> DroneState {
        DroneState {
            id: drone_id,
            position,
            velocity: (0.0, 0.0, 0.0),
            heading: 0.0,
            battery_level: 0.8,
            status: DroneOperationStatus::Idle,
            current_mission: None,
            last_update,
            communication_quality: 0.95,
        }
    }

    fn geo_offset(origin: &GeoCoordinate, east_m: f64, north_m: f64) -> GeoCoordinate {
        const EARTH_RADIUS_M: f64 = 6_371_000.0;
        let latitude = origin.latitude + (north_m / EARTH_RADIUS_M).to_degrees();
        let longitude = origin.longitude
            + (east_m / (EARTH_RADIUS_M * origin.latitude.to_radians().cos())).to_degrees();
        GeoCoordinate {
            latitude,
            longitude,
            altitude_m: origin.altitude_m,
        }
    }

    #[tokio::test]
    async fn test_coordination_engine() {
        let mut engine = CoordinationEngine::new();

        let drone_id = Uuid::new_v4();
        let state = DroneState {
            id: drone_id,
            position: GeoCoordinate {
                latitude: 40.7128,
                longitude: -74.0060,
                altitude_m: 100.0,
            },
            velocity: (5.0, 0.0, 0.0),
            heading: 0.0,
            battery_level: 0.8,
            status: DroneOperationStatus::Idle,
            current_mission: None,
            last_update: Utc::now(),
            communication_quality: 0.95,
        };

        engine.register_drone(drone_id, state).await.unwrap();

        let status = engine.get_coordination_status().await;
        assert_eq!(status.total_drones, 1);
        assert_eq!(status.active_drones, 1);
    }

    #[tokio::test]
    async fn per_drone_freshness_reports_fresh_and_aggregates_steady_swarm() {
        let mut engine = CoordinationEngine::new();
        engine
            .set_telemetry_freshness_timeout(chrono::Duration::seconds(5))
            .unwrap();
        let checked_at = fixed_time();
        let left_id = Uuid::from_u128(11);
        let right_id = Uuid::from_u128(12);

        engine
            .register_drone(
                left_id,
                test_state(left_id, checked_at - chrono::Duration::seconds(1), 0.82),
            )
            .await
            .unwrap();
        engine
            .register_drone(
                right_id,
                test_state(right_id, checked_at - chrono::Duration::seconds(2), 0.76),
            )
            .await
            .unwrap();

        let report = engine.swarm_telemetry_report_at(checked_at).await;
        let status = engine.get_coordination_status_at(checked_at).await;

        assert_eq!(report.status, SwarmTelemetryStatus::Fresh);
        assert_eq!(report.total_drones, 2);
        assert_eq!(report.fresh_drones, 2);
        assert_eq!(report.stale_drones, 0);
        assert_eq!(report.drones.len(), 2);
        assert_eq!(report.drones[0].drone_id, left_id);
        assert_eq!(report.drones[0].freshness, DroneTelemetryFreshness::Fresh);
        assert_eq!(report.drones[0].battery_level, 0.82);
        assert_eq!(status.telemetry_status, SwarmTelemetryStatus::Fresh);
        assert_eq!(status.fresh_drones, 2);
        assert_eq!(status.stale_drones, 0);
    }

    #[tokio::test]
    async fn stale_drone_degrades_swarm_telemetry_status() {
        let mut engine = CoordinationEngine::new();
        engine
            .set_telemetry_freshness_timeout(chrono::Duration::seconds(5))
            .unwrap();
        let checked_at = fixed_time();
        let fresh_id = Uuid::from_u128(21);
        let stale_id = Uuid::from_u128(22);

        engine
            .register_drone(
                fresh_id,
                test_state(fresh_id, checked_at - chrono::Duration::seconds(1), 0.91),
            )
            .await
            .unwrap();
        engine
            .register_drone(
                stale_id,
                test_state(stale_id, checked_at - chrono::Duration::seconds(30), 0.63),
            )
            .await
            .unwrap();

        let report = engine.swarm_telemetry_report_at(checked_at).await;
        let status = engine.get_coordination_status_at(checked_at).await;
        let json = serde_json::to_string(&report).unwrap();

        assert_eq!(report.status, SwarmTelemetryStatus::Degraded);
        assert_eq!(report.total_drones, 2);
        assert_eq!(report.fresh_drones, 1);
        assert_eq!(report.stale_drones, 1);
        assert_eq!(report.drones[1].drone_id, stale_id);
        assert_eq!(report.drones[1].freshness, DroneTelemetryFreshness::Stale);
        assert_eq!(report.drones[1].age_seconds, 30);
        assert_eq!(status.telemetry_status, SwarmTelemetryStatus::Degraded);
        assert_eq!(status.fresh_drones, 1);
        assert_eq!(status.stale_drones, 1);
        assert!(json.contains("\"freshness\":\"Stale\""));
        assert!(json.contains("\"status\":\"Degraded\""));
    }

    #[tokio::test]
    async fn regular_heartbeats_keep_link_healthy_without_rule_audit() {
        let mut engine = CoordinationEngine::new();
        engine
            .set_heartbeat_timeout(chrono::Duration::seconds(10))
            .unwrap();
        let checked_at = fixed_time();
        let drone_id = Uuid::from_u128(31);

        engine
            .register_drone(
                drone_id,
                test_state(drone_id, checked_at - chrono::Duration::seconds(30), 0.84),
            )
            .await
            .unwrap();
        engine
            .record_heartbeat(drone_id, checked_at - chrono::Duration::seconds(2), 0.93)
            .await
            .unwrap();

        let report = engine.evaluate_link_rules_at(checked_at).await.unwrap();

        assert_eq!(report.healthy_links, 1);
        assert_eq!(report.timed_out_links, 0);
        assert_eq!(report.links[0].health, DroneLinkHealth::Healthy);
        assert_eq!(report.links[0].link_quality, 0.93);
        assert!(report.audit_events.is_empty());
        assert!(engine.coordination_rule_audit_log().is_empty());
    }

    #[tokio::test]
    async fn heartbeat_timeout_fires_comm_loss_rule_and_is_audited() {
        let mut engine = CoordinationEngine::new();
        engine
            .set_heartbeat_timeout(chrono::Duration::seconds(10))
            .unwrap();
        let checked_at = fixed_time();
        let stale_id = Uuid::from_u128(32);

        engine
            .register_drone(
                stale_id,
                test_state(stale_id, checked_at - chrono::Duration::seconds(45), 0.59),
            )
            .await
            .unwrap();

        let report = engine.evaluate_link_rules_at(checked_at).await.unwrap();
        let json = serde_json::to_string(&report).unwrap();

        assert_eq!(report.healthy_links, 0);
        assert_eq!(report.timed_out_links, 1);
        assert_eq!(report.links[0].drone_id, stale_id);
        assert_eq!(report.links[0].health, DroneLinkHealth::TimedOut);
        assert_eq!(report.links[0].heartbeat_age_seconds, 45);
        assert_eq!(report.audit_events.len(), 1);
        assert_eq!(report.audit_events[0].rule_name, "Communication Loss");
        assert_eq!(report.audit_events[0].drone_id, Some(stale_id));
        assert_eq!(report.audit_events[0].triggered_at, checked_at);
        assert_eq!(engine.coordination_rule_audit_log().len(), 1);
        assert!(json.contains("\"health\":\"TimedOut\""));
        assert!(json.contains("\"condition\":\"communication_loss\""));
    }

    #[tokio::test]
    async fn formation_optimizer_assigns_grid_slots_deterministically() {
        let mut engine = CoordinationEngine::new();
        let checked_at = fixed_time();
        let leader = GeoCoordinate {
            latitude: 40.0,
            longitude: -96.0,
            altitude_m: 100.0,
        };
        let drone_ids = [
            Uuid::from_u128(101),
            Uuid::from_u128(102),
            Uuid::from_u128(103),
            Uuid::from_u128(104),
        ];
        for (drone_id, east_m, north_m) in [
            (drone_ids[0], 1.0, 1.0),
            (drone_ids[1], 29.0, 1.0),
            (drone_ids[2], 1.0, 29.0),
            (drone_ids[3], 29.0, 29.0),
        ] {
            engine
                .register_drone(
                    drone_id,
                    test_state_at(drone_id, geo_offset(&leader, east_m, north_m), checked_at),
                )
                .await
                .unwrap();
        }

        let report = engine
            .optimize_formation_slots(
                &crate::Formation::Grid {
                    rows: 2,
                    cols: 2,
                    spacing_m: 30.0,
                },
                leader,
                FormationOptimizationConfig {
                    minimum_separation_m: 25.0,
                },
            )
            .await
            .unwrap();

        let assignments = report
            .assignments
            .iter()
            .map(|assignment| (assignment.drone_id, assignment.slot_index))
            .collect::<Vec<_>>();
        assert_eq!(
            assignments,
            vec![
                (drone_ids[0], 0),
                (drone_ids[1], 1),
                (drone_ids[2], 2),
                (drone_ids[3], 3),
            ]
        );
        assert!(report.validated);
        assert!(report.minimum_path_separation_m >= 25.0);
        assert!(report.total_travel_m < 8.0);
    }

    #[tokio::test]
    async fn formation_optimizer_rejects_crossing_slot_swap_and_resolves_safe_assignment() {
        let mut engine = CoordinationEngine::new();
        let checked_at = fixed_time();
        let leader = GeoCoordinate {
            latitude: 40.0,
            longitude: -96.0,
            altitude_m: 100.0,
        };
        let left_id = Uuid::from_u128(201);
        let right_id = Uuid::from_u128(202);
        engine
            .register_drone(
                left_id,
                test_state_at(left_id, geo_offset(&leader, 30.0, 0.0), checked_at),
            )
            .await
            .unwrap();
        engine
            .register_drone(
                right_id,
                test_state_at(right_id, geo_offset(&leader, 0.0, 0.0), checked_at),
            )
            .await
            .unwrap();

        let report = engine
            .optimize_formation_slots(
                &crate::Formation::Line {
                    spacing_m: 30.0,
                    heading_deg: 90.0,
                },
                leader,
                FormationOptimizationConfig {
                    minimum_separation_m: 25.0,
                },
            )
            .await
            .unwrap();

        let assignments = report
            .assignments
            .iter()
            .map(|assignment| (assignment.drone_id, assignment.slot_index))
            .collect::<Vec<_>>();
        assert_eq!(assignments, vec![(left_id, 1), (right_id, 0)]);
        assert!(report.rejected_assignment_count > 0);
        assert!(report.validated);
        assert!(report.minimum_path_separation_m >= 25.0);
    }

    #[test]
    fn test_distance_calculation() {
        let engine = CoordinationEngine::new();

        let pos1 = GeoCoordinate {
            latitude: 40.7128,
            longitude: -74.0060,
            altitude_m: 100.0,
        };

        let pos2 = GeoCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude_m: 100.0,
        };

        let distance = engine.calculate_distance(&pos1, &pos2);
        assert!(distance > 0.0);
        assert!(distance < 200.0); // Should be less than 200m for this small difference
    }
}
