use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedSecretRef {
    pub name: String,
    pub value_env: String,
    pub file_env: String,
}

impl ManagedSecretRef {
    pub fn new(name: &str, value_env: &str, file_env: &str) -> Self {
        Self {
            name: name.trim().to_string(),
            value_env: value_env.trim().to_string(),
            file_env: file_env.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretSource {
    Environment,
    FileMount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretMaterial {
    value: String,
    pub source: SecretSource,
}

impl SecretMaterial {
    pub fn new(value: &str, source: SecretSource) -> Self {
        Self {
            value: value.trim().to_string(),
            source,
        }
    }

    pub fn expose(&self) -> &str {
        &self.value
    }

    pub fn redacted(&self) -> &'static str {
        "********"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretResolver {
    env: BTreeMap<String, String>,
}

impl SecretResolver {
    pub fn from_env() -> Self {
        Self::from_map(std::env::vars().collect())
    }

    pub fn from_map(env: BTreeMap<String, String>) -> Self {
        Self { env }
    }

    pub fn resolve(&self, reference: &ManagedSecretRef) -> Result<SecretMaterial, SecretError> {
        validate_ref(reference)?;

        if let Some(path) = self.env_value(&reference.file_env) {
            let value = fs::read_to_string(&path).map_err(|source| SecretError::ReadFailed {
                name: reference.name.clone(),
                path: path.clone().into(),
                details: source.to_string(),
            })?;
            return material_from_value(&reference.name, value, SecretSource::FileMount);
        }

        if let Some(value) = self.env_value(&reference.value_env) {
            return material_from_value(&reference.name, value, SecretSource::Environment);
        }

        Err(SecretError::Missing {
            name: reference.name.clone(),
            value_env: reference.value_env.clone(),
            file_env: reference.file_env.clone(),
        })
    }

    fn env_value(&self, key: &str) -> Option<String> {
        self.env
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    #[error("secret reference name cannot be empty")]
    EmptyName,
    #[error("secret reference environment key cannot be empty for {name}")]
    EmptyEnvironmentKey { name: String },
    #[error("secret {name} is missing; set {value_env} or {file_env}")]
    Missing {
        name: String,
        value_env: String,
        file_env: String,
    },
    #[error("secret {name} is empty")]
    EmptyValue { name: String },
    #[error("failed to read secret {name} from {path:?}: {details}")]
    ReadFailed {
        name: String,
        path: PathBuf,
        details: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaintextSecretRule {
    pub key_markers: Vec<String>,
}

impl Default for PlaintextSecretRule {
    fn default() -> Self {
        Self {
            key_markers: vec![
                "PASSWORD".to_string(),
                "TOKEN".to_string(),
                "SECRET".to_string(),
                "API_KEY".to_string(),
                "DATABASE_URL".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaintextSecretFinding {
    pub path: String,
    pub line: usize,
    pub key: String,
    pub reason: String,
}

pub fn scan_plaintext_secrets(
    path: &str,
    content: &str,
    rules: &[PlaintextSecretRule],
) -> Vec<PlaintextSecretFinding> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let (key, value) = parse_assignment(line)?;
            let key_upper = key.to_ascii_uppercase();
            if key_upper.ends_with("_FILE") {
                return None;
            }
            if !rules.iter().any(|rule| {
                rule.key_markers
                    .iter()
                    .any(|marker| key_upper.contains(marker))
            }) {
                return None;
            }
            if is_managed_secret_reference(value) {
                return None;
            }

            Some(PlaintextSecretFinding {
                path: path.to_string(),
                line: index + 1,
                key: key.to_string(),
                reason: "secret-like key has a committed plaintext value".to_string(),
            })
        })
        .collect()
}

fn validate_ref(reference: &ManagedSecretRef) -> Result<(), SecretError> {
    if reference.name.is_empty() {
        return Err(SecretError::EmptyName);
    }
    if reference.value_env.is_empty() || reference.file_env.is_empty() {
        return Err(SecretError::EmptyEnvironmentKey {
            name: reference.name.clone(),
        });
    }
    Ok(())
}

fn material_from_value(
    name: &str,
    value: String,
    source: SecretSource,
) -> Result<SecretMaterial, SecretError> {
    let value = value.trim();
    if value.is_empty() {
        Err(SecretError::EmptyValue {
            name: name.to_string(),
        })
    } else {
        Ok(SecretMaterial::new(value, source))
    }
}

fn parse_assignment(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let line = line.strip_prefix("- ").unwrap_or(line);
    let line = line.strip_prefix("ENV ").unwrap_or(line);

    if let Some((key, value)) = line.split_once('=') {
        return parse_assignment_parts(key, value);
    }
    if let Some((key, value)) = line.split_once(':') {
        return parse_assignment_parts(key, value);
    }
    None
}

fn parse_assignment_parts<'a>(key: &'a str, value: &'a str) -> Option<(&'a str, &'a str)> {
    let key = key.trim().trim_matches('"').trim_matches('\'');
    let value = value.trim().trim_matches('"').trim_matches('\'');
    (!key.is_empty() && !value.is_empty()).then_some((key, value))
}

fn is_managed_secret_reference(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("${")
        || value.starts_with("$(")
        || value.starts_with('$')
        || value.starts_with("/run/secrets/")
        || value.eq_ignore_ascii_case("null")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::{
        scan_plaintext_secrets, ManagedSecretRef, PlaintextSecretRule, SecretMaterial,
        SecretResolver, SecretSource,
    };

    #[test]
    fn secret_resolver_prefers_file_mount_and_redacts_material() {
        let file_var = unique_name("AGBOT_TEST_SECRET_FILE");
        let value_var = unique_name("AGBOT_TEST_SECRET");
        let secret_path = std::env::temp_dir().join(format!("{}.secret", unique_name("agbot")));
        fs::write(&secret_path, " rotated-db-password \n").expect("write secret file");

        let mut env = BTreeMap::new();
        env.insert(value_var.clone(), "plaintext-fallback".to_string());
        env.insert(file_var.clone(), secret_path.display().to_string());
        let resolver = SecretResolver::from_map(env);

        let secret = resolver
            .resolve(&ManagedSecretRef::new("db_password", &value_var, &file_var))
            .expect("file-mounted secret should resolve");

        assert_eq!(secret.source, SecretSource::FileMount);
        assert_eq!(secret.expose(), "rotated-db-password");
        assert_eq!(secret.redacted(), "********");
        assert_eq!(
            SecretMaterial::new("x", SecretSource::Environment).redacted(),
            "********"
        );

        let _ = fs::remove_file(secret_path);
    }

    #[test]
    fn plaintext_secret_scan_reports_compose_password_value() {
        let findings = scan_plaintext_secrets(
            "docker-compose.yml",
            "services:\n  db:\n    environment:\n      POSTGRES_PASSWORD: password\n",
            &[PlaintextSecretRule::default()],
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "docker-compose.yml");
        assert_eq!(findings[0].line, 4);
        assert_eq!(findings[0].key, "POSTGRES_PASSWORD");
    }

    #[test]
    fn plaintext_secret_scan_allows_managed_references_and_file_keys() {
        let findings = scan_plaintext_secrets(
            "docker-compose.yml",
            "POSTGRES_PASSWORD_FILE: /run/secrets/agbot_postgres_password\nDATABASE_URL: ${DATABASE_URL:?set managed secret}\n",
            &[PlaintextSecretRule::default()],
        );

        assert!(findings.is_empty());
    }

    fn unique_name(prefix: &str) -> String {
        format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
    }
}
