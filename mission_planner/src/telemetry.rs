use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::Telemetry;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionTelemetrySample {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub telemetry: Telemetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryFreshnessConfig {
    pub stale_after: Duration,
    pub gap_after: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryLinkState {
    Fresh,
    Stale,
    NoSamples,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFreshness {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub state: TelemetryLinkState,
    pub latest_timestamp: Option<DateTime<Utc>>,
    pub checked_at: DateTime<Utc>,
    pub age_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryGapEvent {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub last_sample_timestamp: DateTime<Utc>,
    pub detected_at: DateTime<Utc>,
    pub duration_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinkHealthConfig {
    pub warning_rssi_dbm: f32,
    pub warning_packet_loss_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkHealthState {
    Healthy,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkHealthWarning {
    LowRssi,
    HighPacketLoss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionLinkHealthSample {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub timestamp: DateTime<Utc>,
    pub rssi_dbm: f32,
    pub packet_loss_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionLinkHealth {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub timestamp: DateTime<Utc>,
    pub rssi_dbm: f32,
    pub packet_loss_rate: f32,
    pub state: LinkHealthState,
    pub warnings: Vec<LinkHealthWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkHealthTransition {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub from: LinkHealthState,
    pub to: LinkHealthState,
    pub previous_warnings: Vec<LinkHealthWarning>,
    pub current_warnings: Vec<LinkHealthWarning>,
    pub timestamp: DateTime<Utc>,
    pub rssi_dbm: f32,
    pub packet_loss_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MavlinkFailsafeFlag {
    Battery,
    Ekf,
    Geofence,
    Gps,
    RadioLoss,
    RcLoss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionFailsafeSample {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub timestamp: DateTime<Utc>,
    pub flags: Vec<MavlinkFailsafeFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionFailsafeState {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub timestamp: DateTime<Utc>,
    pub active: bool,
    pub flags: Vec<MavlinkFailsafeFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailsafeTransition {
    pub mission_id: Uuid,
    pub drone_id: String,
    pub timestamp: DateTime<Utc>,
    pub previous_flags: Vec<MavlinkFailsafeFlag>,
    pub current_flags: Vec<MavlinkFailsafeFlag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryRecordErrorCode {
    OutOfOrderTimestamp,
    InvalidLinkHealthSample,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryRecordError {
    pub code: TelemetryRecordErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TelemetryStreamKey {
    mission_id: Uuid,
    drone_id: String,
}

#[derive(Debug, Clone)]
pub struct TelemetryHistory {
    config: TelemetryFreshnessConfig,
    link_health_config: LinkHealthConfig,
    samples_by_mission: HashMap<Uuid, Vec<MissionTelemetrySample>>,
    latest_by_stream: HashMap<TelemetryStreamKey, MissionTelemetrySample>,
    gap_events: Vec<TelemetryGapEvent>,
    latest_link_health: HashMap<TelemetryStreamKey, MissionLinkHealth>,
    link_health_transitions: Vec<LinkHealthTransition>,
    latest_failsafe_state: HashMap<TelemetryStreamKey, MissionFailsafeState>,
    failsafe_transitions: Vec<FailsafeTransition>,
}

impl Default for TelemetryFreshnessConfig {
    fn default() -> Self {
        Self {
            stale_after: Duration::seconds(5),
            gap_after: Duration::seconds(5),
        }
    }
}

impl Default for LinkHealthConfig {
    fn default() -> Self {
        Self {
            warning_rssi_dbm: -85.0,
            warning_packet_loss_rate: 0.10,
        }
    }
}

impl Default for TelemetryHistory {
    fn default() -> Self {
        Self::new(TelemetryFreshnessConfig::default())
    }
}

impl TelemetryHistory {
    pub fn new(config: TelemetryFreshnessConfig) -> Self {
        Self {
            config,
            link_health_config: LinkHealthConfig::default(),
            samples_by_mission: HashMap::new(),
            latest_by_stream: HashMap::new(),
            gap_events: Vec::new(),
            latest_link_health: HashMap::new(),
            link_health_transitions: Vec::new(),
            latest_failsafe_state: HashMap::new(),
            failsafe_transitions: Vec::new(),
        }
    }

    pub fn with_link_health_config(
        config: TelemetryFreshnessConfig,
        link_health_config: LinkHealthConfig,
    ) -> Self {
        Self {
            link_health_config,
            ..Self::new(config)
        }
    }

    pub fn record_sample(
        &mut self,
        sample: MissionTelemetrySample,
    ) -> Result<(), TelemetryRecordError> {
        let key = TelemetryStreamKey::from_sample(&sample);
        if let Some(previous) = self.latest_by_stream.get(&key) {
            let previous_timestamp = previous.telemetry.timestamp;
            let timestamp = sample.telemetry.timestamp;
            if timestamp <= previous_timestamp {
                return Err(TelemetryRecordError {
                    code: TelemetryRecordErrorCode::OutOfOrderTimestamp,
                    message: format!(
                        "telemetry timestamp {} must be later than previous sample {}",
                        timestamp, previous_timestamp
                    ),
                });
            }
            let delta = timestamp - previous_timestamp;
            if delta > self.config.gap_after {
                self.record_gap_event(&key, previous_timestamp, timestamp, delta.num_seconds());
            }
        }

        self.samples_by_mission
            .entry(sample.mission_id)
            .or_default()
            .push(sample.clone());
        self.latest_by_stream.insert(key, sample);
        Ok(())
    }

    pub fn replay_mission(&self, mission_id: Uuid) -> Vec<MissionTelemetrySample> {
        let mut samples = self
            .samples_by_mission
            .get(&mission_id)
            .cloned()
            .unwrap_or_default();
        samples.sort_by(|left, right| {
            left.telemetry
                .timestamp
                .cmp(&right.telemetry.timestamp)
                .then_with(|| left.drone_id.cmp(&right.drone_id))
        });
        samples
    }

    pub fn latest_for_mission(&self, mission_id: Uuid) -> Vec<MissionTelemetrySample> {
        let mut samples = self
            .latest_by_stream
            .iter()
            .filter(|(key, _)| key.mission_id == mission_id)
            .map(|(_, sample)| sample.clone())
            .collect::<Vec<_>>();
        samples.sort_by(|left, right| left.drone_id.cmp(&right.drone_id));
        samples
    }

    pub fn evaluate_freshness(
        &mut self,
        mission_id: Uuid,
        drone_id: &str,
        checked_at: DateTime<Utc>,
    ) -> TelemetryFreshness {
        let key = TelemetryStreamKey {
            mission_id,
            drone_id: drone_id.to_string(),
        };
        let Some(latest) = self.latest_by_stream.get(&key) else {
            return TelemetryFreshness {
                mission_id,
                drone_id: drone_id.to_string(),
                state: TelemetryLinkState::NoSamples,
                latest_timestamp: None,
                checked_at,
                age_seconds: None,
            };
        };

        let latest_timestamp = latest.telemetry.timestamp;
        let age = checked_at - latest_timestamp;
        let age_seconds = age.num_seconds().max(0);
        let state = if age > self.config.stale_after {
            self.record_stale_gap_once(&key, latest_timestamp, checked_at, age_seconds);
            TelemetryLinkState::Stale
        } else {
            TelemetryLinkState::Fresh
        };

        TelemetryFreshness {
            mission_id,
            drone_id: drone_id.to_string(),
            state,
            latest_timestamp: Some(latest_timestamp),
            checked_at,
            age_seconds: Some(age_seconds),
        }
    }

    pub fn gap_events_for(&self, mission_id: Uuid, drone_id: &str) -> Vec<TelemetryGapEvent> {
        self.gap_events
            .iter()
            .filter(|event| event.mission_id == mission_id && event.drone_id == drone_id)
            .cloned()
            .collect()
    }

    pub fn record_link_health(
        &mut self,
        sample: MissionLinkHealthSample,
    ) -> Result<MissionLinkHealth, TelemetryRecordError> {
        if !sample.rssi_dbm.is_finite()
            || !sample.packet_loss_rate.is_finite()
            || !(0.0..=1.0).contains(&sample.packet_loss_rate)
        {
            return Err(TelemetryRecordError {
                code: TelemetryRecordErrorCode::InvalidLinkHealthSample,
                message: "link health sample requires finite RSSI and packet loss in 0.0..=1.0"
                    .to_string(),
            });
        }

        let key = TelemetryStreamKey {
            mission_id: sample.mission_id,
            drone_id: sample.drone_id.clone(),
        };
        if let Some(previous) = self.latest_link_health.get(&key) {
            if sample.timestamp <= previous.timestamp {
                return Err(TelemetryRecordError {
                    code: TelemetryRecordErrorCode::OutOfOrderTimestamp,
                    message: format!(
                        "link health timestamp {} must be later than previous sample {}",
                        sample.timestamp, previous.timestamp
                    ),
                });
            }
        }

        let warnings = self.link_health_warnings(sample.rssi_dbm, sample.packet_loss_rate);
        let state = if warnings.is_empty() {
            LinkHealthState::Healthy
        } else {
            LinkHealthState::Warning
        };
        let health = MissionLinkHealth {
            mission_id: sample.mission_id,
            drone_id: sample.drone_id,
            timestamp: sample.timestamp,
            rssi_dbm: sample.rssi_dbm,
            packet_loss_rate: sample.packet_loss_rate,
            state,
            warnings,
        };

        if let Some(previous) = self.latest_link_health.get(&key) {
            if previous.state != health.state || previous.warnings != health.warnings {
                self.link_health_transitions.push(LinkHealthTransition {
                    mission_id: health.mission_id,
                    drone_id: health.drone_id.clone(),
                    from: previous.state,
                    to: health.state,
                    previous_warnings: previous.warnings.clone(),
                    current_warnings: health.warnings.clone(),
                    timestamp: health.timestamp,
                    rssi_dbm: health.rssi_dbm,
                    packet_loss_rate: health.packet_loss_rate,
                });
            }
        }

        self.latest_link_health.insert(key, health.clone());
        Ok(health)
    }

    pub fn latest_link_health(
        &self,
        mission_id: Uuid,
        drone_id: &str,
    ) -> Option<MissionLinkHealth> {
        self.latest_link_health
            .get(&TelemetryStreamKey {
                mission_id,
                drone_id: drone_id.to_string(),
            })
            .cloned()
    }

    pub fn link_health_transitions_for(
        &self,
        mission_id: Uuid,
        drone_id: &str,
    ) -> Vec<LinkHealthTransition> {
        self.link_health_transitions
            .iter()
            .filter(|transition| {
                transition.mission_id == mission_id && transition.drone_id == drone_id
            })
            .cloned()
            .collect()
    }

    pub fn record_failsafe_state(
        &mut self,
        sample: MissionFailsafeSample,
    ) -> Result<Vec<FailsafeTransition>, TelemetryRecordError> {
        let key = TelemetryStreamKey {
            mission_id: sample.mission_id,
            drone_id: sample.drone_id.clone(),
        };
        if let Some(previous) = self.latest_failsafe_state.get(&key) {
            if sample.timestamp <= previous.timestamp {
                return Err(TelemetryRecordError {
                    code: TelemetryRecordErrorCode::OutOfOrderTimestamp,
                    message: format!(
                        "failsafe timestamp {} must be later than previous sample {}",
                        sample.timestamp, previous.timestamp
                    ),
                });
            }
        }

        let flags = normalize_failsafe_flags(sample.flags);
        let previous_flags = self
            .latest_failsafe_state
            .get(&key)
            .map(|state| state.flags.clone())
            .unwrap_or_default();
        let state = MissionFailsafeState {
            mission_id: sample.mission_id,
            drone_id: sample.drone_id.clone(),
            timestamp: sample.timestamp,
            active: !flags.is_empty(),
            flags: flags.clone(),
        };
        self.latest_failsafe_state.insert(key, state);

        if previous_flags == flags {
            return Ok(Vec::new());
        }

        let transition = FailsafeTransition {
            mission_id: sample.mission_id,
            drone_id: sample.drone_id,
            timestamp: sample.timestamp,
            previous_flags,
            current_flags: flags,
        };
        self.failsafe_transitions.push(transition.clone());
        Ok(vec![transition])
    }

    pub fn latest_failsafe_state(
        &self,
        mission_id: Uuid,
        drone_id: &str,
    ) -> Option<MissionFailsafeState> {
        self.latest_failsafe_state
            .get(&TelemetryStreamKey {
                mission_id,
                drone_id: drone_id.to_string(),
            })
            .cloned()
    }

    pub fn failsafe_transitions_for(
        &self,
        mission_id: Uuid,
        drone_id: &str,
    ) -> Vec<FailsafeTransition> {
        self.failsafe_transitions
            .iter()
            .filter(|transition| {
                transition.mission_id == mission_id && transition.drone_id == drone_id
            })
            .cloned()
            .collect()
    }

    fn link_health_warnings(&self, rssi_dbm: f32, packet_loss_rate: f32) -> Vec<LinkHealthWarning> {
        let mut warnings = Vec::new();
        if rssi_dbm <= self.link_health_config.warning_rssi_dbm {
            warnings.push(LinkHealthWarning::LowRssi);
        }
        if packet_loss_rate >= self.link_health_config.warning_packet_loss_rate {
            warnings.push(LinkHealthWarning::HighPacketLoss);
        }
        warnings
    }

    fn record_stale_gap_once(
        &mut self,
        key: &TelemetryStreamKey,
        last_sample_timestamp: DateTime<Utc>,
        detected_at: DateTime<Utc>,
        duration_seconds: i64,
    ) {
        if self.gap_events.iter().any(|event| {
            event.mission_id == key.mission_id
                && event.drone_id == key.drone_id
                && event.last_sample_timestamp == last_sample_timestamp
        }) {
            return;
        }
        self.record_gap_event(key, last_sample_timestamp, detected_at, duration_seconds);
    }

    fn record_gap_event(
        &mut self,
        key: &TelemetryStreamKey,
        last_sample_timestamp: DateTime<Utc>,
        detected_at: DateTime<Utc>,
        duration_seconds: i64,
    ) {
        self.gap_events.push(TelemetryGapEvent {
            mission_id: key.mission_id,
            drone_id: key.drone_id.clone(),
            last_sample_timestamp,
            detected_at,
            duration_seconds,
        });
    }
}

fn normalize_failsafe_flags(flags: Vec<MavlinkFailsafeFlag>) -> Vec<MavlinkFailsafeFlag> {
    flags
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

impl TelemetryStreamKey {
    fn from_sample(sample: &MissionTelemetrySample) -> Self {
        Self {
            mission_id: sample.mission_id,
            drone_id: sample.drone_id.clone(),
        }
    }
}

impl fmt::Display for TelemetryRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for TelemetryRecordError {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};
    use shared::schemas::{GpsCoords, Telemetry};
    use uuid::Uuid;

    fn sample_at(timestamp_seconds: i64) -> Telemetry {
        Telemetry {
            timestamp: Utc.timestamp_opt(timestamp_seconds, 0).unwrap(),
            position: GpsCoords {
                latitude: 41.0,
                longitude: -96.0,
                altitude: 400.0,
            },
            battery_voltage: 15.8,
            battery_percentage: 82,
            armed: true,
            mode: "AUTO".to_string(),
            ground_speed: 6.0,
            air_speed: 6.5,
            heading: 90.0,
            altitude_relative: 40.0,
        }
    }

    fn mission_sample(
        mission_id: Uuid,
        drone_id: &str,
        timestamp_seconds: i64,
    ) -> MissionTelemetrySample {
        MissionTelemetrySample {
            mission_id,
            drone_id: drone_id.to_string(),
            telemetry: sample_at(timestamp_seconds),
        }
    }

    #[test]
    fn telemetry_history_replays_samples_without_gaps() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::new(TelemetryFreshnessConfig {
            stale_after: Duration::seconds(5),
            gap_after: Duration::seconds(5),
        });

        history
            .record_sample(mission_sample(mission_id, "drone-1", 100))
            .expect("first sample records");
        history
            .record_sample(mission_sample(mission_id, "drone-1", 102))
            .expect("monotonic sample records");

        let replay = history.replay_mission(mission_id);
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].telemetry.timestamp, sample_at(100).timestamp);
        assert_eq!(replay[1].telemetry.timestamp, sample_at(102).timestamp);
        assert_eq!(
            history
                .latest_for_mission(mission_id)
                .first()
                .map(|sample| sample.telemetry.timestamp),
            Some(sample_at(102).timestamp)
        );
        assert!(history.gap_events_for(mission_id, "drone-1").is_empty());
    }

    #[test]
    fn telemetry_history_rejects_out_of_order_samples() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::default();
        history
            .record_sample(mission_sample(mission_id, "drone-1", 100))
            .expect("first sample records");

        let error = history
            .record_sample(mission_sample(mission_id, "drone-1", 99))
            .expect_err("older sample should be rejected");

        assert_eq!(error.code, TelemetryRecordErrorCode::OutOfOrderTimestamp);
        assert_eq!(history.replay_mission(mission_id).len(), 1);
    }

    #[test]
    fn telemetry_freshness_marks_stale_and_records_gap_event() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::new(TelemetryFreshnessConfig {
            stale_after: Duration::seconds(5),
            gap_after: Duration::seconds(5),
        });
        history
            .record_sample(mission_sample(mission_id, "drone-1", 100))
            .expect("sample records");

        let freshness =
            history.evaluate_freshness(mission_id, "drone-1", Utc.timestamp_opt(107, 0).unwrap());

        assert_eq!(freshness.state, TelemetryLinkState::Stale);
        assert_eq!(freshness.age_seconds, Some(7));
        let gaps = history.gap_events_for(mission_id, "drone-1");
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].duration_seconds, 7);
    }

    #[test]
    fn telemetry_freshness_is_fresh_for_steady_stream() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::new(TelemetryFreshnessConfig {
            stale_after: Duration::seconds(5),
            gap_after: Duration::seconds(5),
        });
        history
            .record_sample(mission_sample(mission_id, "drone-1", 100))
            .expect("sample records");

        let freshness =
            history.evaluate_freshness(mission_id, "drone-1", Utc.timestamp_opt(103, 0).unwrap());

        assert_eq!(freshness.state, TelemetryLinkState::Fresh);
        assert_eq!(freshness.age_seconds, Some(3));
        assert!(history.gap_events_for(mission_id, "drone-1").is_empty());
    }

    #[test]
    fn link_health_warning_records_transition_for_degrading_rssi_and_loss() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::default();

        let healthy = history
            .record_link_health(MissionLinkHealthSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(100, 0).unwrap(),
                rssi_dbm: -68.0,
                packet_loss_rate: 0.02,
            })
            .expect("healthy link sample records");
        assert_eq!(healthy.state, LinkHealthState::Healthy);
        assert!(healthy.warnings.is_empty());

        let warning = history
            .record_link_health(MissionLinkHealthSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(101, 0).unwrap(),
                rssi_dbm: -92.0,
                packet_loss_rate: 0.18,
            })
            .expect("degrading link sample records");
        assert_eq!(warning.state, LinkHealthState::Warning);
        assert_eq!(
            warning.warnings,
            vec![
                LinkHealthWarning::LowRssi,
                LinkHealthWarning::HighPacketLoss
            ]
        );

        let transitions = history.link_health_transitions_for(mission_id, "drone-1");
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].from, LinkHealthState::Healthy);
        assert_eq!(transitions[0].to, LinkHealthState::Warning);
        assert!(transitions[0].previous_warnings.is_empty());
        assert_eq!(
            transitions[0].current_warnings,
            vec![
                LinkHealthWarning::LowRssi,
                LinkHealthWarning::HighPacketLoss
            ]
        );
        assert_eq!(transitions[0].timestamp, Utc.timestamp_opt(101, 0).unwrap());
    }

    #[test]
    fn link_health_records_warning_detail_transition_without_state_change() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::default();

        history
            .record_link_health(MissionLinkHealthSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(100, 0).unwrap(),
                rssi_dbm: -92.0,
                packet_loss_rate: 0.02,
            })
            .expect("first warning sample records");
        history
            .record_link_health(MissionLinkHealthSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(101, 0).unwrap(),
                rssi_dbm: -92.0,
                packet_loss_rate: 0.18,
            })
            .expect("second warning sample records");

        let transitions = history.link_health_transitions_for(mission_id, "drone-1");
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].from, LinkHealthState::Warning);
        assert_eq!(transitions[0].to, LinkHealthState::Warning);
        assert_eq!(
            transitions[0].previous_warnings,
            vec![LinkHealthWarning::LowRssi]
        );
        assert_eq!(
            transitions[0].current_warnings,
            vec![
                LinkHealthWarning::LowRssi,
                LinkHealthWarning::HighPacketLoss
            ]
        );
    }

    #[test]
    fn mavlink_failsafe_flags_are_persisted_and_surfaced_on_transition() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::default();

        let transitions = history
            .record_failsafe_state(MissionFailsafeSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(200, 0).unwrap(),
                flags: vec![MavlinkFailsafeFlag::RadioLoss, MavlinkFailsafeFlag::Battery],
            })
            .expect("failsafe transition should record");

        assert_eq!(transitions.len(), 1);
        assert!(transitions[0].previous_flags.is_empty());
        assert_eq!(
            transitions[0].current_flags,
            vec![MavlinkFailsafeFlag::Battery, MavlinkFailsafeFlag::RadioLoss]
        );

        let state = history
            .latest_failsafe_state(mission_id, "drone-1")
            .expect("failsafe state should be retained");
        assert!(state.active);
        assert_eq!(
            state.flags,
            vec![MavlinkFailsafeFlag::Battery, MavlinkFailsafeFlag::RadioLoss]
        );

        let duplicate = history
            .record_failsafe_state(MissionFailsafeSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(201, 0).unwrap(),
                flags: vec![MavlinkFailsafeFlag::RadioLoss, MavlinkFailsafeFlag::Battery],
            })
            .expect("duplicate failsafe state should record without transition");
        assert!(duplicate.is_empty());
        assert_eq!(
            history
                .failsafe_transitions_for(mission_id, "drone-1")
                .len(),
            1
        );
    }

    #[test]
    fn mavlink_failsafe_rejects_out_of_order_samples_without_clobbering_latest() {
        let mission_id = Uuid::new_v4();
        let mut history = TelemetryHistory::default();

        history
            .record_failsafe_state(MissionFailsafeSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(300, 0).unwrap(),
                flags: vec![MavlinkFailsafeFlag::RadioLoss],
            })
            .expect("newest failsafe sample records");

        let error = history
            .record_failsafe_state(MissionFailsafeSample {
                mission_id,
                drone_id: "drone-1".to_string(),
                timestamp: Utc.timestamp_opt(299, 0).unwrap(),
                flags: vec![MavlinkFailsafeFlag::Battery],
            })
            .expect_err("older failsafe sample should be rejected");

        assert_eq!(error.code, TelemetryRecordErrorCode::OutOfOrderTimestamp);
        let latest = history
            .latest_failsafe_state(mission_id, "drone-1")
            .expect("latest state should remain");
        assert_eq!(latest.timestamp, Utc.timestamp_opt(300, 0).unwrap());
        assert_eq!(latest.flags, vec![MavlinkFailsafeFlag::RadioLoss]);
        assert_eq!(
            history
                .failsafe_transitions_for(mission_id, "drone-1")
                .len(),
            1
        );
    }
}
