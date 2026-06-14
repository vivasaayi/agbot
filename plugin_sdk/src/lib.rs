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
    pub status: PluginLifecycleStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginLifecycleStatus {
    Registered,
    Enabled,
    Disabled,
}

impl PluginLifecycleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }
}

impl FromStr for PluginLifecycleStatus {
    type Err = PluginLifecycleStatusParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "registered" => Ok(Self::Registered),
            "enabled" => Ok(Self::Enabled),
            "disabled" => Ok(Self::Disabled),
            _ => Err(PluginLifecycleStatusParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("unknown plugin lifecycle status: {value}")]
pub struct PluginLifecycleStatusParseError {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLifecycleTransitionRequest {
    pub status: PluginLifecycleStatus,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginLifecycleAuditRecord {
    pub audit_id: String,
    pub plugin_id: String,
    pub previous_status: PluginLifecycleStatus,
    pub new_status: PluginLifecycleStatus,
    pub actor_id: String,
    pub occurred_at: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDecision {
    Permitted,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityViolationReason {
    UnknownPlugin,
    MalformedCapability,
    UndeclaredCapability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityAuditEntry {
    pub audit_id: String,
    pub plugin_id: String,
    pub required_capability: String,
    pub decision: CapabilityDecision,
    pub reason: Option<CapabilityViolationReason>,
    pub attempted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginExecutionLimits {
    pub max_runtime_ms: u64,
    pub max_memory_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginExecutionPlan {
    pub plugin_id: String,
    pub required_capabilities: Vec<String>,
    pub estimated_runtime_ms: u64,
    pub estimated_memory_mb: u64,
    pub result: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxExecutionStatus {
    Completed,
    Terminated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxTerminationReason {
    CapabilityViolation,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    UnknownPlugin,
    PluginNotEnabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionOutcome {
    pub plugin_id: String,
    pub status: SandboxExecutionStatus,
    pub termination_reason: Option<SandboxTerminationReason>,
    pub result: Option<String>,
    pub estimated_runtime_ms: u64,
    pub estimated_memory_mb: u64,
    pub capability_audit: Vec<CapabilityAuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostApiVersionRange {
    pub supported_min: String,
    pub supported_max: String,
}

impl Default for HostApiVersionRange {
    fn default() -> Self {
        Self {
            supported_min: "2026.1".to_string(),
            supported_max: "2026.1".to_string(),
        }
    }
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
    #[error(
        "plugin {plugin_id} host_api_version {host_api_version} is outside supported range {supported_min}..={supported_max}"
    )]
    UnsupportedHostApiVersion {
        plugin_id: String,
        host_api_version: String,
        supported_min: String,
        supported_max: String,
    },
    #[error("invalid host_api_version {host_api_version}")]
    InvalidHostApiVersion { host_api_version: String },
    #[error("invalid host api version range {supported_min}..={supported_max}")]
    InvalidHostApiVersionRange {
        supported_min: String,
        supported_max: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginLifecycleError {
    #[error("unknown plugin: {plugin_id}")]
    UnknownPlugin { plugin_id: String },
    #[error("actor_id cannot be empty")]
    EmptyActorId,
    #[error("occurred_at cannot be empty")]
    EmptyOccurredAt,
    #[error("audit_id cannot be empty")]
    EmptyAuditId,
    #[error("plugin {plugin_id} cannot transition from {from_status:?} to {to_status:?}")]
    InvalidTransition {
        plugin_id: String,
        from_status: PluginLifecycleStatus,
        to_status: PluginLifecycleStatus,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginHost {
    registrations: BTreeMap<String, PluginRegistrationRecord>,
    capability_audit: Vec<CapabilityAuditEntry>,
    host_api_versions: HostApiVersionRange,
}

impl PluginHost {
    pub fn with_supported_host_api_range(
        supported_min: &str,
        supported_max: &str,
    ) -> Result<Self, PluginRegistrationError> {
        let host_api_versions = HostApiVersionRange {
            supported_min: normalize_optional_text(supported_min.to_string()).ok_or_else(|| {
                PluginRegistrationError::InvalidHostApiVersion {
                    host_api_version: supported_min.to_string(),
                }
            })?,
            supported_max: normalize_optional_text(supported_max.to_string()).ok_or_else(|| {
                PluginRegistrationError::InvalidHostApiVersion {
                    host_api_version: supported_max.to_string(),
                }
            })?,
        };
        validate_host_api_range(&host_api_versions)?;
        Ok(Self {
            host_api_versions,
            ..Self::default()
        })
    }

    pub fn with_registration_records(
        records: Vec<PluginRegistrationRecord>,
    ) -> Result<Self, PluginRegistrationError> {
        let mut registrations = BTreeMap::new();
        for record in records {
            if registrations.contains_key(&record.plugin_id) {
                return Err(PluginRegistrationError::DuplicatePluginId {
                    plugin_id: record.plugin_id,
                });
            }
            registrations.insert(record.plugin_id.clone(), record);
        }

        Ok(Self {
            registrations,
            ..Self::default()
        })
    }

    pub fn list_extension_points(&self) -> Vec<ExtensionPointContract> {
        extension_point_taxonomy()
    }

    pub fn register_plugin(
        &mut self,
        manifest: RawPluginManifest,
    ) -> Result<PluginRegistrationRecord, PluginRegistrationError> {
        let manifest = validate_manifest(manifest)?;
        enforce_host_api_compatibility(&manifest, &self.host_api_versions)?;
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
            status: PluginLifecycleStatus::Registered,
        };
        self.registrations
            .insert(record.plugin_id.clone(), record.clone());
        Ok(record)
    }

    pub fn list_plugins(&self) -> Vec<PluginRegistrationRecord> {
        self.registrations.values().cloned().collect()
    }

    pub fn transition_plugin_status(
        &mut self,
        plugin_id: &str,
        request: PluginLifecycleTransitionRequest,
        audit_id: String,
    ) -> Result<(PluginRegistrationRecord, PluginLifecycleAuditRecord), PluginLifecycleError> {
        let plugin_id = normalize_optional_text(plugin_id.to_string()).unwrap_or_default();
        let actor_id =
            normalize_optional_text(request.actor_id).ok_or(PluginLifecycleError::EmptyActorId)?;
        let occurred_at = normalize_optional_text(request.occurred_at)
            .ok_or(PluginLifecycleError::EmptyOccurredAt)?;
        let audit_id =
            normalize_optional_text(audit_id).ok_or(PluginLifecycleError::EmptyAuditId)?;
        let registration = self.registrations.get_mut(&plugin_id).ok_or_else(|| {
            PluginLifecycleError::UnknownPlugin {
                plugin_id: plugin_id.clone(),
            }
        })?;
        let previous_status = registration.status;
        if !is_allowed_lifecycle_transition(previous_status, request.status) {
            return Err(PluginLifecycleError::InvalidTransition {
                plugin_id,
                from_status: previous_status,
                to_status: request.status,
            });
        }

        registration.status = request.status;
        let updated = registration.clone();
        let audit = PluginLifecycleAuditRecord {
            audit_id,
            plugin_id: updated.plugin_id.clone(),
            previous_status,
            new_status: request.status,
            actor_id,
            occurred_at,
        };
        Ok((updated, audit))
    }

    pub fn check_capability(
        &mut self,
        plugin_id: &str,
        required_capability: &str,
        attempted_at: &str,
    ) -> CapabilityAuditEntry {
        let plugin_id = normalize_optional_text(plugin_id.to_string()).unwrap_or_default();
        let required_capability =
            normalize_optional_text(required_capability.to_string()).unwrap_or_default();
        let attempted_at = normalize_optional_text(attempted_at.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let reason =
            capability_violation_reason(self.registrations.get(&plugin_id), &required_capability);
        let decision = if reason.is_some() {
            CapabilityDecision::Denied
        } else {
            CapabilityDecision::Permitted
        };
        let entry = CapabilityAuditEntry {
            audit_id: format!(
                "capability-audit-{number:06}",
                number = self.capability_audit.len() + 1
            ),
            plugin_id,
            required_capability,
            decision,
            reason,
            attempted_at,
        };
        self.capability_audit.push(entry.clone());
        entry
    }

    pub fn capability_audit_entries(&self) -> Vec<CapabilityAuditEntry> {
        self.capability_audit.clone()
    }

    pub fn execute_sandboxed(
        &mut self,
        plan: PluginExecutionPlan,
        limits: PluginExecutionLimits,
        attempted_at: &str,
    ) -> SandboxExecutionOutcome {
        let plugin_id = normalize_optional_text(plan.plugin_id).unwrap_or_default();
        let Some(registration) = self.registrations.get(&plugin_id) else {
            return SandboxExecutionOutcome {
                plugin_id,
                status: SandboxExecutionStatus::Terminated,
                termination_reason: Some(SandboxTerminationReason::UnknownPlugin),
                result: None,
                estimated_runtime_ms: plan.estimated_runtime_ms,
                estimated_memory_mb: plan.estimated_memory_mb,
                capability_audit: Vec::new(),
            };
        };
        if registration.status != PluginLifecycleStatus::Enabled {
            return SandboxExecutionOutcome {
                plugin_id,
                status: SandboxExecutionStatus::Terminated,
                termination_reason: Some(SandboxTerminationReason::PluginNotEnabled),
                result: None,
                estimated_runtime_ms: plan.estimated_runtime_ms,
                estimated_memory_mb: plan.estimated_memory_mb,
                capability_audit: Vec::new(),
            };
        }

        let mut capability_audit = Vec::new();
        for capability in plan.required_capabilities {
            let entry = self.check_capability(&plugin_id, &capability, attempted_at);
            let denied = entry.decision == CapabilityDecision::Denied;
            capability_audit.push(entry);
            if denied {
                return SandboxExecutionOutcome {
                    plugin_id,
                    status: SandboxExecutionStatus::Terminated,
                    termination_reason: Some(SandboxTerminationReason::CapabilityViolation),
                    result: None,
                    estimated_runtime_ms: plan.estimated_runtime_ms,
                    estimated_memory_mb: plan.estimated_memory_mb,
                    capability_audit,
                };
            }
        }

        if plan.estimated_runtime_ms > limits.max_runtime_ms {
            return SandboxExecutionOutcome {
                plugin_id,
                status: SandboxExecutionStatus::Terminated,
                termination_reason: Some(SandboxTerminationReason::TimeLimitExceeded),
                result: None,
                estimated_runtime_ms: plan.estimated_runtime_ms,
                estimated_memory_mb: plan.estimated_memory_mb,
                capability_audit,
            };
        }

        if plan.estimated_memory_mb > limits.max_memory_mb {
            return SandboxExecutionOutcome {
                plugin_id,
                status: SandboxExecutionStatus::Terminated,
                termination_reason: Some(SandboxTerminationReason::MemoryLimitExceeded),
                result: None,
                estimated_runtime_ms: plan.estimated_runtime_ms,
                estimated_memory_mb: plan.estimated_memory_mb,
                capability_audit,
            };
        }

        SandboxExecutionOutcome {
            plugin_id,
            status: SandboxExecutionStatus::Completed,
            termination_reason: None,
            result: Some(plan.result),
            estimated_runtime_ms: plan.estimated_runtime_ms,
            estimated_memory_mb: plan.estimated_memory_mb,
            capability_audit,
        }
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

fn capability_violation_reason(
    registration: Option<&PluginRegistrationRecord>,
    required_capability: &str,
) -> Option<CapabilityViolationReason> {
    if !is_well_formed_capability(required_capability) {
        return Some(CapabilityViolationReason::MalformedCapability);
    }
    let Some(registration) = registration else {
        return Some(CapabilityViolationReason::UnknownPlugin);
    };
    if registration
        .capabilities
        .iter()
        .any(|capability| capability == required_capability)
    {
        None
    } else {
        Some(CapabilityViolationReason::UndeclaredCapability)
    }
}

fn is_allowed_lifecycle_transition(
    from_status: PluginLifecycleStatus,
    to_status: PluginLifecycleStatus,
) -> bool {
    matches!(
        (from_status, to_status),
        (
            PluginLifecycleStatus::Registered,
            PluginLifecycleStatus::Enabled
        ) | (
            PluginLifecycleStatus::Enabled,
            PluginLifecycleStatus::Disabled
        ) | (
            PluginLifecycleStatus::Disabled,
            PluginLifecycleStatus::Enabled
        )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ParsedHostApiVersion {
    major: u32,
    minor: u32,
}

fn enforce_host_api_compatibility(
    manifest: &PluginManifest,
    range: &HostApiVersionRange,
) -> Result<(), PluginRegistrationError> {
    validate_host_api_range(range)?;
    let plugin_version = parse_host_api_version(&manifest.host_api_version)?;
    let min_version = parse_host_api_version(&range.supported_min)?;
    let max_version = parse_host_api_version(&range.supported_max)?;
    if plugin_version < min_version || plugin_version > max_version {
        return Err(PluginRegistrationError::UnsupportedHostApiVersion {
            plugin_id: manifest.plugin_id.clone(),
            host_api_version: manifest.host_api_version.clone(),
            supported_min: range.supported_min.clone(),
            supported_max: range.supported_max.clone(),
        });
    }

    Ok(())
}

fn validate_host_api_range(range: &HostApiVersionRange) -> Result<(), PluginRegistrationError> {
    let min_version = parse_host_api_version(&range.supported_min)?;
    let max_version = parse_host_api_version(&range.supported_max)?;
    if min_version > max_version {
        return Err(PluginRegistrationError::InvalidHostApiVersionRange {
            supported_min: range.supported_min.clone(),
            supported_max: range.supported_max.clone(),
        });
    }

    Ok(())
}

fn parse_host_api_version(value: &str) -> Result<ParsedHostApiVersion, PluginRegistrationError> {
    let normalized = normalize_optional_text(value.to_string()).ok_or_else(|| {
        PluginRegistrationError::InvalidHostApiVersion {
            host_api_version: value.to_string(),
        }
    })?;
    let Some((major, minor)) = normalized.split_once('.') else {
        return Err(PluginRegistrationError::InvalidHostApiVersion {
            host_api_version: normalized,
        });
    };
    let Ok(major) = major.parse::<u32>() else {
        return Err(PluginRegistrationError::InvalidHostApiVersion {
            host_api_version: normalized,
        });
    };
    let Ok(minor) = minor.parse::<u32>() else {
        return Err(PluginRegistrationError::InvalidHostApiVersion {
            host_api_version: normalized,
        });
    };
    Ok(ParsedHostApiVersion { major, minor })
}

fn normalize_optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        CapabilityDecision, CapabilityViolationReason, ManifestField, ManifestRejectionReason,
        PluginExecutionLimits, PluginExecutionPlan, PluginHost, PluginLifecycleStatus,
        PluginLifecycleTransitionRequest, PluginRegistrationError, RawPluginManifest,
        SandboxExecutionStatus, SandboxTerminationReason,
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
        assert_eq!(record.status, PluginLifecycleStatus::Registered);
        assert_eq!(
            record.capabilities,
            vec!["read:scene".to_string(), "write:product".to_string()]
        );
        assert_eq!(host.list_plugins(), vec![record]);
    }

    #[test]
    fn lifecycle_enable_disable_transitions_are_audited() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");

        let (enabled, enable_audit) = host
            .transition_plugin_status(
                "plugin.scene_reader",
                lifecycle_request(
                    PluginLifecycleStatus::Enabled,
                    "platform-admin-1",
                    "2026-06-12T12:05:00Z",
                ),
                "plugin-audit-000001".to_string(),
            )
            .expect("registered plugin should enable");

        assert_eq!(enabled.status, PluginLifecycleStatus::Enabled);
        assert_eq!(
            enable_audit.previous_status,
            PluginLifecycleStatus::Registered
        );
        assert_eq!(enable_audit.new_status, PluginLifecycleStatus::Enabled);
        assert_eq!(enable_audit.actor_id, "platform-admin-1");
        assert_eq!(enable_audit.occurred_at, "2026-06-12T12:05:00Z");

        let (disabled, disable_audit) = host
            .transition_plugin_status(
                "plugin.scene_reader",
                lifecycle_request(
                    PluginLifecycleStatus::Disabled,
                    "platform-admin-1",
                    "2026-06-12T12:06:00Z",
                ),
                "plugin-audit-000002".to_string(),
            )
            .expect("enabled plugin should disable");

        assert_eq!(disabled.status, PluginLifecycleStatus::Disabled);
        assert_eq!(
            disable_audit.previous_status,
            PluginLifecycleStatus::Enabled
        );
        assert_eq!(disable_audit.new_status, PluginLifecycleStatus::Disabled);
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

    #[test]
    fn declared_capability_permits_plugin_call() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");

        let decision =
            host.check_capability("plugin.scene_reader", "read:scene", "2026-06-12T12:00:00Z");

        assert_eq!(decision.decision, CapabilityDecision::Permitted);
        assert_eq!(decision.reason, None);
        assert_eq!(decision.required_capability, "read:scene");
        assert_eq!(host.capability_audit_entries(), vec![decision]);
    }

    #[test]
    fn undeclared_capability_is_denied_and_audited() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");

        let decision =
            host.check_capability("plugin.scene_reader", "write:field", "2026-06-12T12:01:00Z");

        assert_eq!(decision.decision, CapabilityDecision::Denied);
        assert_eq!(
            decision.reason,
            Some(CapabilityViolationReason::UndeclaredCapability)
        );
        assert_eq!(decision.audit_id, "capability-audit-000001");
        assert_eq!(host.capability_audit_entries(), vec![decision]);
    }

    #[test]
    fn network_capability_without_declaration_is_denied_and_audited() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");

        let decision =
            host.check_capability("plugin.scene_reader", "net:http", "2026-06-12T12:02:00Z");

        assert_eq!(decision.decision, CapabilityDecision::Denied);
        assert_eq!(
            decision.reason,
            Some(CapabilityViolationReason::UndeclaredCapability)
        );
        assert_eq!(decision.required_capability, "net:http");
        assert_eq!(host.capability_audit_entries().len(), 1);
    }

    #[test]
    fn sandbox_executes_well_behaved_plugin_within_limits() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");
        enable_scene_reader(&mut host);

        let outcome = host.execute_sandboxed(
            sandbox_plan("plugin.scene_reader", vec!["read:scene"], 25, 64),
            sandbox_limits(),
            "2026-06-12T12:10:00Z",
        );

        assert_eq!(outcome.status, SandboxExecutionStatus::Completed);
        assert_eq!(outcome.termination_reason, None);
        assert_eq!(outcome.result, Some("scene stats complete".to_string()));
        assert_eq!(
            outcome.capability_audit[0].decision,
            CapabilityDecision::Permitted
        );
    }

    #[test]
    fn sandbox_terminates_resource_limit_breach_and_host_survives() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");
        enable_scene_reader(&mut host);

        let outcome = host.execute_sandboxed(
            sandbox_plan("plugin.scene_reader", vec!["read:scene"], 250, 64),
            sandbox_limits(),
            "2026-06-12T12:11:00Z",
        );

        assert_eq!(outcome.status, SandboxExecutionStatus::Terminated);
        assert_eq!(
            outcome.termination_reason,
            Some(SandboxTerminationReason::TimeLimitExceeded)
        );
        assert_eq!(host.list_plugins().len(), 1);
        assert_eq!(host.capability_audit_entries().len(), 1);
    }

    #[test]
    fn sandbox_terminates_undeclared_capability_before_execution() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");
        enable_scene_reader(&mut host);

        let outcome = host.execute_sandboxed(
            sandbox_plan("plugin.scene_reader", vec!["write:field"], 25, 64),
            sandbox_limits(),
            "2026-06-12T12:12:00Z",
        );

        assert_eq!(outcome.status, SandboxExecutionStatus::Terminated);
        assert_eq!(
            outcome.termination_reason,
            Some(SandboxTerminationReason::CapabilityViolation)
        );
        assert_eq!(
            outcome.capability_audit[0].reason,
            Some(CapabilityViolationReason::UndeclaredCapability)
        );
        assert_eq!(outcome.result, None);
    }

    #[test]
    fn sandbox_refuses_disabled_plugin_before_capability_checks() {
        let mut host = PluginHost::default();
        host.register_plugin(read_scene_manifest())
            .expect("plugin should register");
        enable_scene_reader(&mut host);
        host.transition_plugin_status(
            "plugin.scene_reader",
            lifecycle_request(
                PluginLifecycleStatus::Disabled,
                "platform-admin-1",
                "2026-06-12T12:20:00Z",
            ),
            "plugin-audit-000010".to_string(),
        )
        .expect("enabled plugin should disable");

        let outcome = host.execute_sandboxed(
            sandbox_plan("plugin.scene_reader", vec!["read:scene"], 25, 64),
            sandbox_limits(),
            "2026-06-12T12:21:00Z",
        );

        assert_eq!(outcome.status, SandboxExecutionStatus::Terminated);
        assert_eq!(
            outcome.termination_reason,
            Some(SandboxTerminationReason::PluginNotEnabled)
        );
        assert!(outcome.capability_audit.is_empty());
        assert_eq!(outcome.result, None);
    }

    #[test]
    fn compatible_host_api_version_registers_within_supported_range() {
        let mut host = PluginHost::with_supported_host_api_range("2026.1", "2026.3")
            .expect("range should be valid");
        let mut manifest = read_scene_manifest();
        manifest.host_api_version = "2026.3".to_string();

        let record = host
            .register_plugin(manifest)
            .expect("compatible plugin should register");

        assert_eq!(record.host_api_version, "2026.3");
        assert_eq!(host.list_plugins().len(), 1);
    }

    #[test]
    fn unsupported_host_api_version_is_refused_before_registration() {
        let mut host = PluginHost::with_supported_host_api_range("2026.1", "2026.3")
            .expect("range should be valid");
        let mut manifest = read_scene_manifest();
        manifest.host_api_version = "2025.9".to_string();

        let error = host
            .register_plugin(manifest)
            .expect_err("unsupported host api version should be refused");

        assert_eq!(
            error,
            PluginRegistrationError::UnsupportedHostApiVersion {
                plugin_id: "plugin.scene_reader".to_string(),
                host_api_version: "2025.9".to_string(),
                supported_min: "2026.1".to_string(),
                supported_max: "2026.3".to_string(),
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

    fn read_scene_manifest() -> RawPluginManifest {
        RawPluginManifest {
            plugin_id: "plugin.scene_reader".to_string(),
            name: "Scene Reader".to_string(),
            version: "1.0.0".to_string(),
            kind: "processor".to_string(),
            host_api_version: "2026.1".to_string(),
            capabilities: vec!["read:scene".to_string()],
            entrypoint: "scene_reader::run".to_string(),
        }
    }

    fn sandbox_limits() -> PluginExecutionLimits {
        PluginExecutionLimits {
            max_runtime_ms: 100,
            max_memory_mb: 128,
        }
    }

    fn enable_scene_reader(host: &mut PluginHost) {
        host.transition_plugin_status(
            "plugin.scene_reader",
            lifecycle_request(
                PluginLifecycleStatus::Enabled,
                "platform-admin-1",
                "2026-06-12T12:04:00Z",
            ),
            "plugin-audit-enable-scene-reader".to_string(),
        )
        .expect("plugin should enable");
    }

    fn lifecycle_request(
        status: PluginLifecycleStatus,
        actor_id: &str,
        occurred_at: &str,
    ) -> PluginLifecycleTransitionRequest {
        PluginLifecycleTransitionRequest {
            status,
            actor_id: actor_id.to_string(),
            occurred_at: occurred_at.to_string(),
        }
    }

    fn sandbox_plan(
        plugin_id: &str,
        required_capabilities: Vec<&str>,
        estimated_runtime_ms: u64,
        estimated_memory_mb: u64,
    ) -> PluginExecutionPlan {
        PluginExecutionPlan {
            plugin_id: plugin_id.to_string(),
            required_capabilities: required_capabilities
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            estimated_runtime_ms,
            estimated_memory_mb,
            result: "scene stats complete".to_string(),
        }
    }
}
