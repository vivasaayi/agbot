use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubRuntimeMode {
    #[default]
    Local,
    #[serde(alias = "sim")]
    Simulation,
    Live,
}

impl HubRuntimeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Simulation => "simulation",
            Self::Live => "live",
        }
    }

    fn default_credential_source(self) -> LandsatCredentialSource {
        match self {
            Self::Live => LandsatCredentialSource::Environment,
            Self::Local | Self::Simulation => LandsatCredentialSource::None,
        }
    }
}

impl fmt::Display for HubRuntimeMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandsatCredentialSource {
    #[default]
    None,
    #[serde(alias = "env")]
    Environment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LandsatConfig {
    pub source: String,
    pub credential_source: LandsatCredentialSource,
}

impl Default for LandsatConfig {
    fn default() -> Self {
        Self {
            source: "sample".to_string(),
            credential_source: LandsatCredentialSource::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineConfig {
    /// Master switch for the background satellite pipeline worker.
    pub enabled: bool,
    /// How often the worker polls the job queue for ready work.
    pub poll_interval_ms: u64,
    /// Minimum delay between successive upstream provider requests.
    pub provider_min_delay_ms: u64,
    /// Hard wall-clock cap on a single job's execution. A handler that hangs
    /// (e.g. an upstream read with no timeout of its own) would otherwise block
    /// the serial worker forever; on timeout the job is failed transiently and
    /// retried with backoff, so the worker keeps draining. `0` disables the cap.
    pub job_timeout_secs: u64,
    /// Minimum product confidence the derive QA gate accepts. A derived product
    /// scoring below this is quarantined (excluded from serving and downstream
    /// L3/app fan-out) instead of silently entering the graph. `0` disables the
    /// gate (every product with a defined confidence passes).
    pub qa_min_confidence: f64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_ms: 1000,
            provider_min_delay_ms: 250,
            // 30 minutes: far above any legitimate derive/composite, low enough
            // to bound a wedged worker. Always on by default.
            job_timeout_secs: 1800,
            // Off by default so existing behavior (accept all) is preserved;
            // the appliance can raise it to quarantine low-confidence products.
            qa_min_confidence: 0.0,
        }
    }
}

/// Default request body cap (64 MiB) — generous enough for shapefile / GeoTIFF
/// imports while still bounding memory per request.
pub const DEFAULT_MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// Bearer token required to call the admin API (portal access-code
    /// mint / list / revoke). Minting an access code can forge a portal
    /// session for any account, so these routes are gated.
    ///
    /// When `None` (or empty) the admin API is **disabled** and returns 403 —
    /// fail-closed, never open. Set `GEO_HUB__SECURITY__ADMIN_TOKEN` in any
    /// exposed deployment; first-run login still works via the bootstrap
    /// access code, which is seeded directly into the database.
    pub admin_token: Option<String>,
    /// When true, every non-public `/api/*` route requires a valid portal
    /// session (`Authorization: Bearer <token>`); anonymous callers get 401.
    /// Public paths (health, login, static shell, token-based shares) are
    /// always allowed. Defaults to **false** so local/dev/test runs are
    /// unauthenticated; turn it on (`GEO_HUB__SECURITY__REQUIRE_SESSION=true`)
    /// in any exposed deployment.
    pub require_session: bool,
    /// Maximum accepted request body size in bytes. Requests larger than this
    /// are rejected with 413 before the handler runs.
    pub max_body_bytes: usize,
    /// Per-client-IP request cap over a rolling 60-second window. `0` disables
    /// rate limiting (the default). When exceeded, requests get 429 with a
    /// `Retry-After` header. A single fixed window per IP — coarse, but enough
    /// to blunt login brute-force and runaway clients; front with a reverse
    /// proxy for anything finer.
    pub rate_limit_per_min: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            admin_token: None,
            require_session: false,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            rate_limit_per_min: 0,
        }
    }
}

impl SecurityConfig {
    /// The configured admin token, if a non-empty one is set.
    pub fn admin_token(&self) -> Option<&str> {
        self.admin_token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    /// Human-readable single-line records (default; good for a terminal).
    #[default]
    Text,
    /// One JSON object per record, for log aggregators / structured search.
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ObservabilityConfig {
    /// Log output format. `RUST_LOG` still controls levels in both formats.
    /// Set `GEO_HUB__OBSERVABILITY__LOG_FORMAT=json` on the appliance so logs
    /// are machine-parseable.
    pub log_format: LogFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StorageConfig {
    /// When true, a product's on-disk artifact is deleted as it is superseded,
    /// so regenerated rasters (monthly composites, climatologies, phenology)
    /// don't accumulate unbounded and exhaust the data volume. Off by default
    /// so dev/test runs keep every artifact; enable it
    /// (`GEO_HUB__STORAGE__DELETE_SUPERSEDED_ARTIFACTS=true`) on the appliance.
    /// Deletion is best-effort and safety-guarded: only files under
    /// `data_root` that no other product still references are removed.
    pub delete_superseded_artifacts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BootstrapConfig {
    /// When true, a fresh server (no marketplace accounts yet) is seeded with a
    /// default org + account + portal access code so the farmer PWA works
    /// out-of-the-box. Seeding is skipped entirely once any account exists, so
    /// it never touches a real deployment.
    pub enabled: bool,
    /// Org id assigned to the seeded account and demo farm/field.
    pub org_id: String,
    /// Account id the seeded access code authenticates as.
    pub account_id: String,
    /// Plaintext access code the operator logs in with on a fresh server. Only
    /// its sha256 hash is stored. Override in any exposed deployment.
    pub access_code: String,
    /// When true, also seed one demo farm + field so the portal is not empty.
    pub seed_demo: bool,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            org_id: "org-local".to_string(),
            account_id: "acct-local".to_string(),
            access_code: "agb-demo".to_string(),
            seed_demo: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HubConfig {
    pub runtime_mode: HubRuntimeMode,
    pub bind_address: String,
    pub database_url: String,
    pub data_root: PathBuf,
    /// Directory holding the static web workspace served at `/workspace`.
    /// Relative paths are resolved against the current working directory
    /// first, then against the cargo workspace root (see
    /// [`HubConfig::workspace_web_dir`]). Override with
    /// `GEO_HUB__WORKSPACE_WEB_ROOT`.
    pub workspace_web_root: PathBuf,
    pub landsat: LandsatConfig,
    pub pipeline: PipelineConfig,
    pub bootstrap: BootstrapConfig,
    pub security: SecurityConfig,
    pub storage: StorageConfig,
    pub observability: ObservabilityConfig,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            runtime_mode: HubRuntimeMode::Local,
            bind_address: "0.0.0.0:8080".to_string(),
            database_url: "sqlite://geo_hub.db".to_string(),
            data_root: PathBuf::from("data/geo_hub"),
            workspace_web_root: PathBuf::from("geo_hub/web"),
            landsat: LandsatConfig::default(),
            pipeline: PipelineConfig::default(),
            bootstrap: BootstrapConfig::default(),
            security: SecurityConfig::default(),
            storage: StorageConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }
}

impl HubConfig {
    pub fn load() -> Result<Self> {
        Self::load_with_path(None::<&Path>)
    }

    pub fn load_with_path(path: Option<&Path>) -> Result<Self> {
        let mut builder = config::Config::builder()
            .add_source(config::File::with_name("geo_hub").required(false))
            .add_source(config::Environment::with_prefix("GEO_HUB").separator("__"));

        if let Some(path) = path {
            builder = builder.add_source(config::File::from(path).required(true));
        }

        let explicit_path = path.is_some();
        let cfg = builder.build()?;
        if explicit_path {
            Self::validate_required_file_fields(&cfg)?;
        }
        let has_landsat_credential_source = cfg.get_string("landsat.credential_source").is_ok();
        let mut config: HubConfig = cfg.try_deserialize()?;
        if !has_landsat_credential_source {
            config.landsat.credential_source = config.runtime_mode.default_credential_source();
        }
        if config.database_url.starts_with("sqlite://") && !config.database_url.contains('?') {
            // Enable WAL mode for better concurrency
            config.database_url.push_str("?mode=rwc");
        }
        config.resolve_secrets()?;
        Ok(config)
    }

    /// Resolve secret-bearing config fields through [`SecretResolver`] so a
    /// value like `file:/run/secrets/admin_token` or `env:ADMIN_TOKEN` is
    /// dereferenced once, at load, keeping the secret out of config and the
    /// process environment listing. A misconfigured reference (missing file /
    /// env var) fails startup loudly rather than silently disabling auth.
    fn resolve_secrets(&mut self) -> Result<()> {
        let resolver = crate::secrets::SecretResolver::new();
        if let Some(raw) = self.security.admin_token.take() {
            let resolved = resolver
                .resolve(&raw)
                .context("resolving security.admin_token")?;
            self.security.admin_token = Some(resolved);
        }
        Ok(())
    }

    fn validate_required_file_fields(cfg: &config::Config) -> Result<()> {
        for field in [
            "runtime_mode",
            "bind_address",
            "database_url",
            "data_root",
            "landsat.source",
        ] {
            if cfg.get_string(field).is_err() {
                return Err(anyhow!("missing required hub config field `{field}`"));
            }
        }
        Ok(())
    }

    pub fn ensure_data_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_root)?;
        std::fs::create_dir_all(self.data_root.join("scenes"))?;
        if let Some(parent) = self.sqlite_db_parent() {
            std::fs::create_dir_all(&parent)?;
        }
        Ok(())
    }

    /// The on-disk path of a file-backed `sqlite://` database, or `None` for
    /// in-memory databases and non-sqlite URLs. Strips any `?mode=rwc`-style
    /// query suffix. Used by backup/restore and the parent-dir helper.
    pub fn database_file_path(&self) -> Option<PathBuf> {
        let rest = self.database_url.strip_prefix("sqlite://")?;
        let path_part = rest.split('?').next().unwrap_or(rest);
        if path_part.is_empty() || path_part == ":memory:" {
            return None;
        }
        Some(PathBuf::from(path_part))
    }

    /// For a `sqlite://` database URL backed by a file, return the parent
    /// directory of that file so it can be created ahead of connecting. This
    /// keeps a relocated appliance (e.g. `sqlite:///opt/agbot/db/geo_hub.db`)
    /// working even when the enclosing directory does not yet exist. Returns
    /// `None` for in-memory databases and non-sqlite URLs.
    fn sqlite_db_parent(&self) -> Option<PathBuf> {
        self.database_file_path()?
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
    }

    /// Resolve the directory of the static web workspace served at
    /// `/workspace`. Absolute paths are used as-is. A relative path is used
    /// relative to the current working directory when it exists there (the
    /// normal case when running from the cargo workspace root); otherwise it
    /// falls back to the cargo workspace root derived from this crate's
    /// manifest directory, which keeps tests and crate-local runs working.
    pub fn workspace_web_dir(&self) -> PathBuf {
        if self.workspace_web_root.is_absolute() || self.workspace_web_root.exists() {
            return self.workspace_web_root.clone();
        }
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap_or(manifest_dir);
        workspace_root.join(&self.workspace_web_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_config(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("geo_hub.toml");
        fs::write(&path, contents).unwrap();
        (tmp, path)
    }

    #[test]
    fn hub_config_loads_runtime_mode_and_landsat_settings() {
        let (_tmp, path) = write_config(
            r#"
runtime_mode = "simulation"
bind_address = "127.0.0.1:8787"
database_url = "sqlite://geo_hub_test.db"
data_root = "tmp/geo_hub"

[landsat]
source = "sample"
"#,
        );

        let config = HubConfig::load_with_path(Some(&path)).unwrap();

        assert_eq!(config.runtime_mode, HubRuntimeMode::Simulation);
        assert_eq!(config.bind_address, "127.0.0.1:8787");
        assert_eq!(config.database_url, "sqlite://geo_hub_test.db?mode=rwc");
        assert_eq!(config.data_root, PathBuf::from("tmp/geo_hub"));
        assert_eq!(config.landsat.source, "sample");
        assert_eq!(
            config.landsat.credential_source,
            LandsatCredentialSource::None
        );
    }

    #[test]
    fn hub_config_live_mode_defaults_to_environment_credentials() {
        let (_tmp, path) = write_config(
            r#"
runtime_mode = "live"
bind_address = "127.0.0.1:8787"
database_url = "sqlite://geo_hub_live.db"
data_root = "tmp/geo_hub_live"

[landsat]
source = "landsat"
"#,
        );

        let config = HubConfig::load_with_path(Some(&path)).unwrap();

        assert_eq!(config.runtime_mode, HubRuntimeMode::Live);
        assert_eq!(
            config.landsat.credential_source,
            LandsatCredentialSource::Environment
        );
    }

    #[test]
    fn hub_config_default_workspace_web_dir_resolves_to_crate_web_directory() {
        let config = HubConfig::default();

        assert_eq!(config.workspace_web_root, PathBuf::from("geo_hub/web"));

        let resolved = config.workspace_web_dir();
        assert!(resolved.is_absolute() || resolved.exists());
        assert!(resolved.ends_with("geo_hub/web"));
    }

    #[test]
    fn hub_config_absolute_workspace_web_root_is_used_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        let config = HubConfig {
            workspace_web_root: tmp.path().to_path_buf(),
            ..HubConfig::default()
        };

        assert_eq!(config.workspace_web_dir(), tmp.path());
    }

    #[test]
    fn hub_config_pipeline_defaults_off_and_loads_from_file() {
        let default_config = HubConfig::default();
        assert!(!default_config.pipeline.enabled);
        assert_eq!(default_config.pipeline.poll_interval_ms, 1000);
        assert_eq!(default_config.pipeline.provider_min_delay_ms, 250);

        let (_tmp, path) = write_config(
            r#"
runtime_mode = "simulation"
bind_address = "127.0.0.1:8787"
database_url = "sqlite://geo_hub_test.db"
data_root = "tmp/geo_hub"

[landsat]
source = "sample"

[pipeline]
enabled = true
poll_interval_ms = 2500
provider_min_delay_ms = 500
"#,
        );

        let config = HubConfig::load_with_path(Some(&path)).unwrap();
        assert!(config.pipeline.enabled);
        assert_eq!(config.pipeline.poll_interval_ms, 2500);
        assert_eq!(config.pipeline.provider_min_delay_ms, 500);
    }

    #[test]
    fn hub_config_security_admin_token_defaults_off_and_loads_from_file() {
        let default_config = HubConfig::default();
        assert_eq!(default_config.security.admin_token(), None);

        let (_tmp, path) = write_config(
            r#"
runtime_mode = "simulation"
bind_address = "127.0.0.1:8787"
database_url = "sqlite://geo_hub_test.db"
data_root = "tmp/geo_hub"

[landsat]
source = "sample"

[security]
admin_token = "  s3cret-admin  "
"#,
        );

        let config = HubConfig::load_with_path(Some(&path)).unwrap();
        // The accessor trims surrounding whitespace and rejects empties.
        assert_eq!(config.security.admin_token(), Some("s3cret-admin"));
    }

    #[test]
    fn hub_config_observability_log_format_defaults_text_and_parses_json() {
        assert_eq!(
            HubConfig::default().observability.log_format,
            LogFormat::Text
        );

        let (_tmp, path) = write_config(
            r#"
runtime_mode = "local"
bind_address = "127.0.0.1:8787"
database_url = "sqlite://geo_hub_test.db"
data_root = "tmp/geo_hub"

[landsat]
source = "sample"

[observability]
log_format = "json"
"#,
        );
        let config = HubConfig::load_with_path(Some(&path)).unwrap();
        assert_eq!(config.observability.log_format, LogFormat::Json);
    }

    #[test]
    fn hub_config_resolves_admin_token_from_a_file_reference() {
        let tmp = tempfile::tempdir().unwrap();
        let secret_path = tmp.path().join("admin_token");
        fs::write(&secret_path, "  resolved-admin-secret\n").unwrap();

        let (_cfgtmp, path) = write_config(&format!(
            r#"
runtime_mode = "local"
bind_address = "127.0.0.1:8787"
database_url = "sqlite://geo_hub_test.db"
data_root = "tmp/geo_hub"

[landsat]
source = "sample"

[security]
admin_token = "file:{}"
"#,
            secret_path.display()
        ));

        let config = HubConfig::load_with_path(Some(&path)).unwrap();
        // The `file:` reference is dereferenced (and trimmed) at load.
        assert_eq!(config.security.admin_token(), Some("resolved-admin-secret"));
    }

    #[test]
    fn hub_config_fails_when_admin_token_file_is_missing() {
        let (_tmp, path) = write_config(
            r#"
runtime_mode = "local"
bind_address = "127.0.0.1:8787"
database_url = "sqlite://geo_hub_test.db"
data_root = "tmp/geo_hub"

[landsat]
source = "sample"

[security]
admin_token = "file:/no/such/secret/file"
"#,
        );
        let err = HubConfig::load_with_path(Some(&path)).unwrap_err();
        assert!(err.to_string().contains("admin_token"), "{err}");
    }

    #[test]
    fn security_admin_token_accessor_rejects_blank() {
        let blank = SecurityConfig {
            admin_token: Some("   ".to_string()),
            ..SecurityConfig::default()
        };
        assert_eq!(blank.admin_token(), None);
    }

    #[test]
    fn ensure_data_dirs_creates_absolute_sqlite_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let db_dir = tmp.path().join("db");
        let data_dir = tmp.path().join("data");
        let config = HubConfig {
            database_url: format!("sqlite://{}/geo_hub.db?mode=rwc", db_dir.display()),
            data_root: data_dir.clone(),
            ..HubConfig::default()
        };

        config.ensure_data_dirs().unwrap();

        assert!(db_dir.is_dir());
        assert!(data_dir.join("scenes").is_dir());
    }

    #[test]
    fn sqlite_db_parent_ignores_relative_and_memory_urls() {
        let relative = HubConfig {
            database_url: "sqlite://geo_hub.db?mode=rwc".to_string(),
            ..HubConfig::default()
        };
        assert_eq!(relative.sqlite_db_parent(), None);

        let memory = HubConfig {
            database_url: "sqlite://:memory:".to_string(),
            ..HubConfig::default()
        };
        assert_eq!(memory.sqlite_db_parent(), None);
    }

    #[test]
    fn hub_config_missing_required_file_field_fails_fast() {
        let (_tmp, path) = write_config(
            r#"
runtime_mode = "local"
bind_address = "127.0.0.1:8787"
data_root = "tmp/geo_hub"

[landsat]
source = "sample"
"#,
        );

        let error = HubConfig::load_with_path(Some(&path)).unwrap_err();

        assert!(error.to_string().contains("database_url"));
    }
}
