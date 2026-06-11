use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared::schemas::Telemetry;
use std::collections::HashMap;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TelemetryRecordErrorCode {
    OutOfOrderTimestamp,
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
    samples_by_mission: HashMap<Uuid, Vec<MissionTelemetrySample>>,
    latest_by_stream: HashMap<TelemetryStreamKey, MissionTelemetrySample>,
    gap_events: Vec<TelemetryGapEvent>,
}

impl Default for TelemetryFreshnessConfig {
    fn default() -> Self {
        Self {
            stale_after: Duration::seconds(5),
            gap_after: Duration::seconds(5),
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
            samples_by_mission: HashMap::new(),
            latest_by_stream: HashMap::new(),
            gap_events: Vec::new(),
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
}
