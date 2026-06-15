use crate::operator_actions::OperatorActionKind;
use serde::{Deserialize, Serialize};
use shared::schemas::Telemetry;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorAssistThresholds {
    pub low_battery_percentage: u8,
    pub geofence_edge_distance_m: f64,
}

impl Default for OperatorAssistThresholds {
    fn default() -> Self {
        Self {
            low_battery_percentage: 25,
            geofence_edge_distance_m: 20.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeofenceProximitySignal {
    pub mission_id: Uuid,
    pub distance_to_edge_m: f64,
    pub outside: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorAssistAdvisory {
    pub advisory_id: String,
    pub mission_id: Uuid,
    pub suggested_action: OperatorActionKind,
    pub evidence_refs: Vec<String>,
    pub requires_operator_confirmation: bool,
    pub auto_executed: bool,
    pub summary: String,
}

pub fn evaluate_operator_assist_advisory(
    telemetry: &Telemetry,
    geofence: Option<&GeofenceProximitySignal>,
    thresholds: &OperatorAssistThresholds,
) -> Option<OperatorAssistAdvisory> {
    let geofence = geofence?;
    let low_battery = telemetry.battery_percentage <= thresholds.low_battery_percentage;
    let near_or_outside_geofence =
        geofence.outside || geofence.distance_to_edge_m <= thresholds.geofence_edge_distance_m;
    if !low_battery || !near_or_outside_geofence {
        return None;
    }

    Some(OperatorAssistAdvisory {
        advisory_id: format!("operator-assist:rth:{}", geofence.mission_id),
        mission_id: geofence.mission_id,
        suggested_action: OperatorActionKind::ReturnToHome,
        evidence_refs: vec![
            format!(
                "telemetry:battery_percentage:{}<=threshold:{}",
                telemetry.battery_percentage, thresholds.low_battery_percentage
            ),
            if geofence.outside {
                "geofence:outside:true".to_string()
            } else {
                format!(
                    "geofence:distance_to_edge_m:{:.1}<=threshold:{:.1}",
                    geofence.distance_to_edge_m, thresholds.geofence_edge_distance_m
                )
            },
        ],
        requires_operator_confirmation: true,
        auto_executed: false,
        summary: "Low battery near geofence edge; suggest return-to-home".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::schemas::GpsCoords;

    #[test]
    fn low_battery_near_geofence_suggests_rth_with_evidence() {
        let mission_id = Uuid::new_v4();
        let advisory = evaluate_operator_assist_advisory(
            &telemetry_with_battery(18),
            Some(&GeofenceProximitySignal {
                mission_id,
                distance_to_edge_m: 8.5,
                outside: false,
            }),
            &OperatorAssistThresholds::default(),
        )
        .expect("low battery near geofence should suggest action");

        assert_eq!(advisory.mission_id, mission_id);
        assert_eq!(advisory.suggested_action, OperatorActionKind::ReturnToHome);
        assert!(advisory
            .evidence_refs
            .iter()
            .any(|evidence| evidence.contains("battery_percentage:18")));
        assert!(advisory
            .evidence_refs
            .iter()
            .any(|evidence| evidence.contains("distance_to_edge_m:8.5")));
    }

    #[test]
    fn nominal_telemetry_does_not_raise_advisory() {
        let advisory = evaluate_operator_assist_advisory(
            &telemetry_with_battery(72),
            Some(&GeofenceProximitySignal {
                mission_id: Uuid::new_v4(),
                distance_to_edge_m: 120.0,
                outside: false,
            }),
            &OperatorAssistThresholds::default(),
        );

        assert!(advisory.is_none());
    }

    #[test]
    fn advisory_is_gated_and_never_auto_executes() {
        let advisory = evaluate_operator_assist_advisory(
            &telemetry_with_battery(20),
            Some(&GeofenceProximitySignal {
                mission_id: Uuid::new_v4(),
                distance_to_edge_m: 2.0,
                outside: false,
            }),
            &OperatorAssistThresholds::default(),
        )
        .expect("advisory should be raised");

        assert!(advisory.requires_operator_confirmation);
        assert!(!advisory.auto_executed);
    }

    fn telemetry_with_battery(battery_percentage: u8) -> Telemetry {
        Telemetry {
            timestamp: Utc::now(),
            position: GpsCoords {
                latitude: 41.25,
                longitude: -96.45,
                altitude: 320.0,
            },
            battery_voltage: 14.8,
            battery_percentage,
            armed: true,
            mode: "AUTO".to_string(),
            ground_speed: 8.0,
            air_speed: 9.0,
            heading: 180.0,
            altitude_relative: 120.0,
        }
    }
}
