use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const TWIN_CONTRACT_VERSION: &str = "1.0.0";

fn default_contract_version() -> String {
    TWIN_CONTRACT_VERSION.to_string()
}

fn default_payload() -> Value {
    json!({})
}

fn default_ack_timeout_ms() -> u32 {
    1000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwinCommandType {
    Arm,
    Disarm,
    Step,
    SetManualInput,
    SetWind,
    Abort,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct TwinVec3V1 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct TwinManualControlInputV1 {
    pub throttle: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    pub takeoff: bool,
    pub land: bool,
    pub arm: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlightCommandV1 {
    #[serde(default = "default_contract_version")]
    pub contract_version: String,
    pub command_id: String,
    pub command_type: TwinCommandType,
    pub issued_at_unix_ms: u64,
    #[serde(default = "default_payload")]
    pub payload: Value,
    #[serde(default = "default_ack_timeout_ms")]
    pub ack_timeout_ms: u32,
    #[serde(default)]
    pub step_duration_s: f64,
    #[serde(default)]
    pub manual_input: TwinManualControlInputV1,
    #[serde(default)]
    pub wind_mps: TwinVec3V1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryV1 {
    #[serde(default = "default_contract_version")]
    pub contract_version: String,
    pub command_id: String,
    pub time_s: f64,
    pub mode: String,
    pub position: TwinVec3V1,
    pub velocity: TwinVec3V1,
    pub attitude: TwinVec3V1,
    pub battery_percent: f64,
    pub target_waypoint_index: usize,
    pub armed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TwinErrorV1 {
    #[serde(default = "default_contract_version")]
    pub contract_version: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TwinCommandAckV1 {
    #[serde(default = "default_contract_version")]
    pub contract_version: String,
    pub command_id: String,
    pub accepted: bool,
    pub error: Option<TwinErrorV1>,
    pub telemetry: Option<TelemetryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractTypeV1 {
    pub name: String,
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwinContractSchemaV1 {
    pub name: String,
    pub version: String,
    pub types: Vec<ContractTypeV1>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TwinContractError {
    #[error("TwinContractV1 type {type_name} is missing required field(s): {missing:?}")]
    MissingFields {
        type_name: String,
        missing: Vec<String>,
    },
}

impl TwinContractSchemaV1 {
    pub fn has_type(&self, type_name: &str) -> bool {
        self.types.iter().any(|item| item.name == type_name)
    }

    pub fn type_has_field(&self, type_name: &str, field_name: &str) -> bool {
        self.types
            .iter()
            .find(|item| item.name == type_name)
            .map(|item| {
                item.required_fields
                    .iter()
                    .any(|required| required == field_name)
            })
            .unwrap_or(false)
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|item| item == capability)
    }

    pub fn missing_fields(&self, type_name: &str, fields: &[&str]) -> Vec<String> {
        fields
            .iter()
            .filter(|field| !self.type_has_field(type_name, field))
            .map(|field| (*field).to_string())
            .collect()
    }

    pub fn assert_type_fields(
        &self,
        type_name: &str,
        fields: &[&str],
    ) -> Result<(), TwinContractError> {
        let missing = self.missing_fields(type_name, fields);
        if missing.is_empty() {
            Ok(())
        } else {
            Err(TwinContractError::MissingFields {
                type_name: type_name.to_string(),
                missing,
            })
        }
    }
}

pub fn twin_contract_v1_schema() -> TwinContractSchemaV1 {
    TwinContractSchemaV1 {
        name: "TwinContractV1".to_string(),
        version: TWIN_CONTRACT_VERSION.to_string(),
        types: vec![
            ContractTypeV1 {
                name: "FlightCommandV1".to_string(),
                required_fields: [
                    "contract_version",
                    "command_id",
                    "command_type",
                    "issued_at_unix_ms",
                    "payload",
                    "ack_timeout_ms",
                ]
                .into_iter()
                .map(str::to_string)
                .collect(),
            },
            ContractTypeV1 {
                name: "TelemetryV1".to_string(),
                required_fields: [
                    "contract_version",
                    "command_id",
                    "time_s",
                    "mode",
                    "position",
                    "velocity",
                    "attitude",
                    "battery_percent",
                    "target_waypoint_index",
                    "armed",
                ]
                .into_iter()
                .map(str::to_string)
                .collect(),
            },
            ContractTypeV1 {
                name: "TwinErrorV1".to_string(),
                required_fields: ["contract_version", "code", "message", "retryable"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            },
            ContractTypeV1 {
                name: "TwinCommandAckV1".to_string(),
                required_fields: [
                    "contract_version",
                    "command_id",
                    "accepted",
                    "error",
                    "telemetry",
                ]
                .into_iter()
                .map(str::to_string)
                .collect(),
            },
        ],
        capabilities: [
            "shared_command_telemetry_contract",
            "twin_backend_api",
            "deterministic_runner",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHARED_FIXTURE: &str =
        include_str!("../fixtures/twin_contract_v1_command_telemetry.json");

    #[derive(serde::Deserialize)]
    struct SharedContractFixture {
        schema: TwinContractSchemaV1,
        sample_command: FlightCommandV1,
        sample_ack: TwinCommandAckV1,
    }

    fn shared_fixture() -> SharedContractFixture {
        serde_json::from_str(SHARED_FIXTURE).expect("shared twin contract fixture parses")
    }

    #[test]
    fn shared_flight_command_uses_twin_wire_names() {
        let command = FlightCommandV1 {
            contract_version: TWIN_CONTRACT_VERSION.to_string(),
            command_id: "cmd-step-1".to_string(),
            command_type: TwinCommandType::Step,
            issued_at_unix_ms: 1_800_000_000_000,
            payload: json!({"source": "shared"}),
            ack_timeout_ms: 750,
            step_duration_s: 0.25,
            manual_input: TwinManualControlInputV1::default(),
            wind_mps: TwinVec3V1::default(),
        };

        let value = serde_json::to_value(&command).expect("command serializes");

        assert_eq!(value["contract_version"], TWIN_CONTRACT_VERSION);
        assert_eq!(value["command_id"], "cmd-step-1");
        assert_eq!(value["command_type"], "step");
        assert_eq!(value["payload"]["source"], "shared");
        assert_eq!(value["ack_timeout_ms"], 750);
        assert_eq!(value["step_duration_s"], 0.25);
    }

    #[test]
    fn shared_twin_ack_parses_cpp_backend_telemetry_shape() {
        let ack = shared_fixture().sample_ack;

        let telemetry = ack.telemetry.expect("ack contains telemetry");
        assert_eq!(ack.command_id, telemetry.command_id);
        assert_eq!(telemetry.mode, "takeoff");
        assert_eq!(telemetry.position.y, 1.0);
        assert!(telemetry.armed);
    }

    #[test]
    fn shared_fixture_pins_command_and_telemetry_schema() {
        let fixture = shared_fixture();
        let schema = twin_contract_v1_schema();

        assert_eq!(fixture.schema.version, schema.version);
        for expected_type in &fixture.schema.types {
            assert!(schema.has_type(&expected_type.name));
            for required_field in &expected_type.required_fields {
                assert!(
                    schema.type_has_field(&expected_type.name, required_field),
                    "{} missing {}",
                    expected_type.name,
                    required_field
                );
            }
        }
        for capability in &fixture.schema.capabilities {
            assert!(schema.has_capability(capability));
        }

        assert_eq!(fixture.sample_command.command_type, TwinCommandType::Step);
        assert_eq!(fixture.sample_command.command_id, "cmd-step-1");
        assert_eq!(fixture.sample_command.step_duration_s, 0.25);
        assert!(fixture.sample_ack.telemetry.is_some());
    }

    #[test]
    fn contract_schema_detects_drift_in_required_fields() {
        let schema = twin_contract_v1_schema();

        schema
            .assert_type_fields(
                "TelemetryV1",
                &[
                    "contract_version",
                    "command_id",
                    "time_s",
                    "mode",
                    "position",
                    "velocity",
                    "attitude",
                    "battery_percent",
                    "target_waypoint_index",
                    "armed",
                ],
            )
            .expect("TelemetryV1 required fields are pinned");

        let error = schema
            .assert_type_fields("TelemetryV1", &["battery_percent", "missing_field"])
            .expect_err("schema drift is reported");
        assert_eq!(
            error,
            TwinContractError::MissingFields {
                type_name: "TelemetryV1".to_string(),
                missing: vec!["missing_field".to_string()],
            }
        );
    }
}
