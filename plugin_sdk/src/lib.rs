use serde::{Deserialize, Serialize};
use shared::plugin_extensions::{
    extension_point_taxonomy, ExtensionPointContract, ExtensionPointKind,
};
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPluginManifest {
    #[serde(default)]
    pub plugin_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub host_api_version: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub entrypoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub kind: ExtensionPointKind,
    pub host_api_version: String,
    pub capabilities: Vec<String>,
    pub entrypoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRegistrationRecord {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub kind: ExtensionPointKind,
    pub host_api_version: String,
    pub capabilities: Vec<String>,
    pub entrypoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestField {
    PluginId,
    Name,
    Version,
    Kind,
    HostApiVersion,
    Capabilities,
    Entrypoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestRejectionReason {
    EmptyRequiredField,
    EmptyCapabilityList,
    MalformedCapability,
    UnknownExtensionPointKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestViolation {
    pub field: ManifestField,
    pub reason: ManifestRejectionReason,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginRegistrationError {
    #[error("plugin manifest validation failed")]
    InvalidManifest { violations: Vec<ManifestViolation> },
    #[error("plugin already registered: {plugin_id}")]
    DuplicatePluginId { plugin_id: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginHost {
    registrations: BTreeMap<String, PluginRegistrationRecord>,
}

impl PluginHost {
    pub fn list_extension_points(&self) -> Vec<ExtensionPointContract> {
        extension_point_taxonomy()
    }

    pub fn register_plugin(
        &mut self,
        manifest: RawPluginManifest,
    ) -> Result<PluginRegistrationRecord, PluginRegistrationError> {
        let manifest = validate_manifest(manifest)?;
        if self.registrations.contains_key(&manifest.plugin_id) {
            return Err(PluginRegistrationError::DuplicatePluginId {
                plugin_id: manifest.plugin_id,
            });
        }

        let record = PluginRegistrationRecord {
            plugin_id: manifest.plugin_id,
            name: manifest.name,
            version: manifest.version,
            kind: manifest.kind,
            host_api_version: manifest.host_api_version,
            capabilities: manifest.capabilities,
            entrypoint: manifest.entrypoint,
        };
        self.registrations
            .insert(record.plugin_id.clone(), record.clone());
        Ok(record)
    }

    pub fn list_plugins(&self) -> Vec<PluginRegistrationRecord> {
        self.registrations.values().cloned().collect()
    }
}

pub fn validate_manifest(
    manifest: RawPluginManifest,
) -> Result<PluginManifest, PluginRegistrationError> {
    let mut violations = Vec::new();

    let plugin_id = required_field(manifest.plugin_id, ManifestField::PluginId, &mut violations);
    let name = required_field(manifest.name, ManifestField::Name, &mut violations);
    let version = required_field(manifest.version, ManifestField::Version, &mut violations);
    let kind_text = normalize_optional_text(manifest.kind);
    let kind = match kind_text.as_deref() {
        Some(value) => match ExtensionPointKind::from_str(value) {
            Ok(kind) => Some(kind),
            Err(_) => {
                violations.push(ManifestViolation {
                    field: ManifestField::Kind,
                    reason: ManifestRejectionReason::UnknownExtensionPointKind,
                    value: Some(value.to_string()),
                });
                None
            }
        },
        None => {
            violations.push(ManifestViolation {
                field: ManifestField::Kind,
                reason: ManifestRejectionReason::EmptyRequiredField,
                value: None,
            });
            None
        }
    };
    let host_api_version = required_field(
        manifest.host_api_version,
        ManifestField::HostApiVersion,
        &mut violations,
    );
    let capabilities = validate_capabilities(manifest.capabilities, &mut violations);
    let entrypoint = required_field(
        manifest.entrypoint,
        ManifestField::Entrypoint,
        &mut violations,
    );

    if !violations.is_empty() {
        return Err(PluginRegistrationError::InvalidManifest { violations });
    }

    Ok(PluginManifest {
        plugin_id: plugin_id.expect("plugin_id is present after validation"),
        name: name.expect("name is present after validation"),
        version: version.expect("version is present after validation"),
        kind: kind.expect("kind is present after validation"),
        host_api_version: host_api_version.expect("host_api_version is present after validation"),
        capabilities,
        entrypoint: entrypoint.expect("entrypoint is present after validation"),
    })
}

fn required_field(
    value: String,
    field: ManifestField,
    violations: &mut Vec<ManifestViolation>,
) -> Option<String> {
    match normalize_optional_text(value) {
        Some(value) => Some(value),
        None => {
            violations.push(ManifestViolation {
                field,
                reason: ManifestRejectionReason::EmptyRequiredField,
                value: None,
            });
            None
        }
    }
}

fn validate_capabilities(
    capabilities: Vec<String>,
    violations: &mut Vec<ManifestViolation>,
) -> Vec<String> {
    if capabilities.is_empty() {
        violations.push(ManifestViolation {
            field: ManifestField::Capabilities,
            reason: ManifestRejectionReason::EmptyCapabilityList,
            value: None,
        });
        return Vec::new();
    }

    let mut normalized = Vec::new();
    for capability in capabilities {
        match normalize_optional_text(capability) {
            Some(value) if is_well_formed_capability(&value) => normalized.push(value),
            Some(value) => violations.push(ManifestViolation {
                field: ManifestField::Capabilities,
                reason: ManifestRejectionReason::MalformedCapability,
                value: Some(value),
            }),
            None => violations.push(ManifestViolation {
                field: ManifestField::Capabilities,
                reason: ManifestRejectionReason::MalformedCapability,
                value: None,
            }),
        }
    }

    normalized
}

fn is_well_formed_capability(value: &str) -> bool {
    let Some((scope, resource)) = value.split_once(':') else {
        return false;
    };
    !scope.trim().is_empty() && !resource.trim().is_empty()
}

fn normalize_optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ManifestField, ManifestRejectionReason, PluginHost, PluginRegistrationError,
        RawPluginManifest,
    };
    use shared::plugin_extensions::ExtensionPointKind;

    #[test]
    fn host_lists_exact_extension_point_taxonomy() {
        let host = PluginHost::default();
        let contracts = host.list_extension_points();

        assert_eq!(contracts.len(), 6);
        assert_eq!(contracts[0].kind, ExtensionPointKind::Index);
        assert_eq!(contracts[5].kind, ExtensionPointKind::ImportExportAdapter);
        assert!(contracts
            .iter()
            .all(|contract| !contract.contract_signature.trim().is_empty()));
    }

    #[test]
    fn well_formed_manifest_registers_and_lists_plugin() {
        let mut host = PluginHost::default();

        let record = host
            .register_plugin(custom_index_manifest())
            .expect("manifest should register");

        assert_eq!(record.plugin_id, "plugin.custom_ndvi");
        assert_eq!(record.kind, ExtensionPointKind::Index);
        assert_eq!(record.version, "1.2.3");
        assert_eq!(
            record.capabilities,
            vec!["read:scene".to_string(), "write:product".to_string()]
        );
        assert_eq!(host.list_plugins(), vec![record]);
    }

    #[test]
    fn unknown_extension_point_kind_is_rejected_without_registration() {
        let mut host = PluginHost::default();
        let mut manifest = custom_index_manifest();
        manifest.kind = "telepathy".to_string();

        let error = host
            .register_plugin(manifest)
            .expect_err("unknown kind should be rejected");

        assert_eq!(
            error,
            PluginRegistrationError::InvalidManifest {
                violations: vec![super::ManifestViolation {
                    field: ManifestField::Kind,
                    reason: ManifestRejectionReason::UnknownExtensionPointKind,
                    value: Some("telepathy".to_string()),
                }]
            }
        );
        assert!(host.list_plugins().is_empty());
    }

    #[test]
    fn malformed_manifest_reports_field_level_reasons_without_partial_registration() {
        let mut host = PluginHost::default();

        let error = host
            .register_plugin(RawPluginManifest {
                plugin_id: " ".to_string(),
                name: String::new(),
                version: "0.1.0".to_string(),
                kind: "index".to_string(),
                host_api_version: " ".to_string(),
                capabilities: vec!["readscene".to_string(), " ".to_string()],
                entrypoint: String::new(),
            })
            .expect_err("malformed manifest should be rejected");

        assert_eq!(
            error,
            PluginRegistrationError::InvalidManifest {
                violations: vec![
                    super::ManifestViolation {
                        field: ManifestField::PluginId,
                        reason: ManifestRejectionReason::EmptyRequiredField,
                        value: None,
                    },
                    super::ManifestViolation {
                        field: ManifestField::Name,
                        reason: ManifestRejectionReason::EmptyRequiredField,
                        value: None,
                    },
                    super::ManifestViolation {
                        field: ManifestField::HostApiVersion,
                        reason: ManifestRejectionReason::EmptyRequiredField,
                        value: None,
                    },
                    super::ManifestViolation {
                        field: ManifestField::Capabilities,
                        reason: ManifestRejectionReason::MalformedCapability,
                        value: Some("readscene".to_string()),
                    },
                    super::ManifestViolation {
                        field: ManifestField::Capabilities,
                        reason: ManifestRejectionReason::MalformedCapability,
                        value: None,
                    },
                    super::ManifestViolation {
                        field: ManifestField::Entrypoint,
                        reason: ManifestRejectionReason::EmptyRequiredField,
                        value: None,
                    },
                ]
            }
        );
        assert!(host.list_plugins().is_empty());
    }

    fn custom_index_manifest() -> RawPluginManifest {
        RawPluginManifest {
            plugin_id: "plugin.custom_ndvi".to_string(),
            name: "Custom NDVI".to_string(),
            version: "1.2.3".to_string(),
            kind: "index".to_string(),
            host_api_version: "2026.1".to_string(),
            capabilities: vec!["read:scene".to_string(), "write:product".to_string()],
            entrypoint: "custom_ndvi::run".to_string(),
        }
    }
}
