use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisEvidence {
    pub layer_ref: String,
    pub method: String,
    pub parameters: BTreeMap<String, Value>,
    pub input_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AnalysisEvidenceError {
    #[error("failed to serialize evidence input: {message}")]
    Serialization { message: String },
}

impl From<serde_json::Error> for AnalysisEvidenceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization {
            message: error.to_string(),
        }
    }
}

pub fn make_analysis_evidence<T: Serialize>(
    layer_ref: impl Into<String>,
    method: impl Into<String>,
    parameters: BTreeMap<String, Value>,
    input: &T,
) -> Result<AnalysisEvidence, AnalysisEvidenceError> {
    let input_hash = deterministic_fingerprint(input)?;
    Ok(AnalysisEvidence {
        layer_ref: layer_ref.into(),
        method: method.into(),
        parameters,
        input_hash,
    })
}

pub fn deterministic_fingerprint<T: Serialize>(input: &T) -> Result<String, AnalysisEvidenceError> {
    let serialized = serde_json::to_vec(&canonical_value(serde_json::to_value(input)?))?;
    Ok(format!("{:016x}", fnv1a64(&serialized)))
}

fn canonical_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries = map.into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            let mut ordered = Map::new();
            for (key, value) in entries {
                ordered.insert(key, canonical_value(value));
            }
            Value::Object(ordered)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_value).collect()),
        Value::Number(number) => Value::Number(number),
        Value::String(value) => Value::String(value),
        Value::Bool(value) => Value::Bool(value),
        Value::Null => Value::Null,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn evidence_parameters(values: &[(&str, Value)]) -> BTreeMap<String, Value> {
    let mut parameters = BTreeMap::new();
    for (key, value) in values {
        parameters.insert((*key).to_string(), value.clone());
    }
    parameters
}

pub fn evidence_reason(reason_code: impl AsRef<str>) -> Value {
    json!(reason_code.as_ref())
}
