//! Secret indirection: resolve a config secret from a reference rather than an
//! inline literal, so credentials stay out of config files, images, and the
//! process environment listing.
//!
//! Supported reference forms (anything else is returned as a literal):
//! - `env:NAME`  — the value of environment variable `NAME`.
//! - `file:PATH` — the trimmed contents of `PATH`.
//! - `/run/secrets/NAME` — a bare Docker / systemd secret-mount path, read as a
//!   file (matches the convention `scripts/verify-secrets.sh` already allows).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret environment variable {0} is not set")]
    MissingEnv(String),
    #[error("failed to read secret file {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
}

/// Resolves possibly-indirected secret values. A unit type today; kept as a
/// struct so future backends (e.g. a managed secret store) can be added without
/// changing call sites.
#[derive(Debug, Clone, Copy, Default)]
pub struct SecretResolver;

impl SecretResolver {
    pub fn new() -> Self {
        Self
    }

    /// Resolve `value` to its concrete secret. Literals (no recognized prefix)
    /// pass through unchanged after trimming.
    pub fn resolve(&self, value: &str) -> Result<String, SecretError> {
        let trimmed = value.trim();
        if let Some(name) = trimmed.strip_prefix("env:") {
            return std::env::var(name).map_err(|_| SecretError::MissingEnv(name.to_string()));
        }
        if let Some(path) = trimmed.strip_prefix("file:") {
            return read_secret_file(path);
        }
        if trimmed.starts_with("/run/secrets/") {
            return read_secret_file(trimmed);
        }
        Ok(trimmed.to_string())
    }
}

fn read_secret_file(path: &str) -> Result<String, SecretError> {
    let contents = std::fs::read_to_string(path).map_err(|source| SecretError::ReadFile {
        path: path.to_string(),
        source,
    })?;
    Ok(contents.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_values_pass_through_trimmed() {
        let r = SecretResolver::new();
        assert_eq!(r.resolve("  s3cret  ").unwrap(), "s3cret");
        assert_eq!(r.resolve("plain").unwrap(), "plain");
    }

    #[test]
    fn file_reference_reads_and_trims_contents() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("token");
        std::fs::write(&path, "  file-secret\n").unwrap();
        let r = SecretResolver::new();
        assert_eq!(
            r.resolve(&format!("file:{}", path.display())).unwrap(),
            "file-secret"
        );
    }

    #[test]
    fn missing_file_reference_errors() {
        let r = SecretResolver::new();
        let err = r.resolve("file:/no/such/secret/path").unwrap_err();
        assert!(matches!(err, SecretError::ReadFile { .. }));
    }

    #[test]
    fn env_reference_reads_variable() {
        // Uniquely-named var to avoid cross-test interference.
        let var = "GEO_HUB_TEST_SECRET_ENV_XYZ";
        std::env::set_var(var, "env-secret");
        let r = SecretResolver::new();
        assert_eq!(r.resolve(&format!("env:{var}")).unwrap(), "env-secret");
        std::env::remove_var(var);

        let err = r
            .resolve("env:GEO_HUB_DEFINITELY_UNSET_VAR_QQ")
            .unwrap_err();
        assert!(matches!(err, SecretError::MissingEnv(_)));
    }
}
