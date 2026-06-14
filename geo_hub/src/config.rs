use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubRuntimeMode {
    Local,
    #[serde(alias = "sim")]
    Simulation,
    Live,
}

impl Default for HubRuntimeMode {
    fn default() -> Self {
        Self::Local
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandsatCredentialSource {
    None,
    #[serde(alias = "env")]
    Environment,
}

impl Default for LandsatCredentialSource {
    fn default() -> Self {
        Self::None
    }
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

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HubConfig {
    pub runtime_mode: HubRuntimeMode,
    pub bind_address: String,
    pub database_url: String,
    pub data_root: PathBuf,
    pub landsat: LandsatConfig,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            runtime_mode: HubRuntimeMode::Local,
            bind_address: "0.0.0.0:8080".to_string(),
            database_url: "sqlite://geo_hub.db".to_string(),
            data_root: PathBuf::from("data/geo_hub"),
            landsat: LandsatConfig::default(),
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
        Ok(config)
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
        Ok(())
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
