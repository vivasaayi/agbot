//! Producer contract for the product catalog.
//!
//! A `ProductRecordDraft` is what a processing crate (imagery, LiDAR,
//! post-processing) hands to the catalog: enough to derive a deterministic
//! product identity, reproduce the computation, and emit provenance lineage.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;

use crate::schemas::RasterSpatialRef;

/// Processing level of a catalog product, ordered L0 < L1 < L2 < L3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductLevel {
    L0,
    L1,
    L2,
    L3,
}

impl ProductLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProductLevel::L0 => "l0",
            ProductLevel::L1 => "l1",
            ProductLevel::L2 => "l2",
            ProductLevel::L3 => "l3",
        }
    }
}

impl fmt::Display for ProductLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid product level: {value} (expected l0, l1, l2, or l3)")]
pub struct ProductLevelParseError {
    pub value: String,
}

impl FromStr for ProductLevel {
    type Err = ProductLevelParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "l0" => Ok(ProductLevel::L0),
            "l1" => Ok(ProductLevel::L1),
            "l2" => Ok(ProductLevel::L2),
            "l3" => Ok(ProductLevel::L3),
            other => Err(ProductLevelParseError {
                value: other.to_string(),
            }),
        }
    }
}

/// Spatial/temporal scope a product was derived over. Timestamps are RFC3339
/// strings, consistent with the rest of the `shared` schemas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductScope {
    #[serde(default)]
    pub farm_id: Option<String>,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
    #[serde(default)]
    pub scene_id: Option<String>,
    pub temporal_start: String,
    pub temporal_end: String,
}

/// Reference to an upstream product consumed as an input, tagged with the
/// role it plays (e.g. "band:nir", "mask", "prior_epoch", "dem",
/// "external:landsat_asset").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductInputRef {
    pub product_id: String,
    pub role: String,
}

/// Physical artifact backing a product record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductArtifact {
    pub path: String,
    pub format: String,
    #[serde(default)]
    pub checksum_sha256: Option<String>,
}

/// Draft of a product record as emitted by a producer, before catalog
/// registration assigns storage metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductRecordDraft {
    pub level: ProductLevel,
    pub kind: String,
    pub algorithm_id: String,
    pub algorithm_version: String,
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub inputs: Vec<ProductInputRef>,
    pub scope: ProductScope,
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
    #[serde(default)]
    pub gsd_m_per_px: Option<f64>,
    #[serde(default)]
    pub artifact: Option<ProductArtifact>,
    #[serde(default)]
    pub quality_mask: Option<ProductInputRef>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub confidence_method: Option<String>,
    #[serde(default)]
    pub quality_summary: Option<serde_json::Value>,
    #[serde(default)]
    pub evidence_digests: Vec<String>,
    #[serde(default)]
    pub source_id: Option<String>,
}

impl ProductRecordDraft {
    /// SHA256 hex digest over a canonical serialization of the computation
    /// identity: `algorithm_id`, `algorithm_version`, `parameters` (object
    /// keys sorted recursively, so JSON key insertion order does not matter),
    /// and `inputs` sorted by `(role, product_id)` (so input order does not
    /// matter).
    pub fn parameters_hash(&self) -> String {
        let mut inputs: Vec<&ProductInputRef> = self.inputs.iter().collect();
        inputs.sort_by(|a, b| {
            a.role
                .cmp(&b.role)
                .then_with(|| a.product_id.cmp(&b.product_id))
        });

        let mut canonical = String::new();
        canonical.push_str("{\"algorithm_id\":");
        push_json_string(&mut canonical, &self.algorithm_id);
        canonical.push_str(",\"algorithm_version\":");
        push_json_string(&mut canonical, &self.algorithm_version);
        canonical.push_str(",\"inputs\":[");
        for (index, input) in inputs.iter().enumerate() {
            if index > 0 {
                canonical.push(',');
            }
            canonical.push_str("{\"product_id\":");
            push_json_string(&mut canonical, &input.product_id);
            canonical.push_str(",\"role\":");
            push_json_string(&mut canonical, &input.role);
            canonical.push('}');
        }
        canonical.push_str("],\"parameters\":");
        push_canonical_json(&mut canonical, &self.parameters);
        canonical.push('}');

        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let digest = hasher.finalize();
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Deterministic product identity: `{scope_token}:{kind}:{hash12}` where
    /// `hash12` is the first 12 hex characters of [`Self::parameters_hash`]
    /// and `scope_token` is resolved in priority order:
    ///
    /// 1. `scope.scene_id` when present;
    /// 2. otherwise `{field_id}@{temporal_start}` when `scope.field_id` is
    ///    present;
    /// 3. otherwise the literal `"global"`.
    pub fn product_id(&self) -> String {
        let scope_token = if let Some(scene_id) = &self.scope.scene_id {
            scene_id.clone()
        } else if let Some(field_id) = &self.scope.field_id {
            format!("{field_id}@{}", self.scope.temporal_start)
        } else {
            "global".to_string()
        };
        let hash = self.parameters_hash();
        format!("{scope_token}:{}:{}", self.kind, &hash[..12])
    }
}

/// Append `value` to `out` as canonical JSON: object keys sorted recursively,
/// no whitespace, `serde_json` number formatting.
fn push_canonical_json(out: &mut String, value: &serde_json::Value) {
    match value {
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::Bool(flag) => {
            out.push_str(if *flag { "true" } else { "false" });
        }
        serde_json::Value::Number(number) => out.push_str(&number.to_string()),
        serde_json::Value::String(text) => push_json_string(out, text),
        serde_json::Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_canonical_json(out, item);
            }
            out.push(']');
        }
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_json_string(out, key);
                out.push(':');
                push_canonical_json(out, &map[*key]);
            }
            out.push('}');
        }
    }
}

fn push_json_string(out: &mut String, text: &str) {
    // serde_json string serialization is deterministic and infallible.
    out.push_str(&serde_json::Value::String(text.to_string()).to_string());
}
