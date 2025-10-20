use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HubConfig {
    pub bind_address: String,
    pub database_url: String,
    pub data_root: PathBuf,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:8080".to_string(),
            database_url: "sqlite://geo_hub.db".to_string(),
            data_root: PathBuf::from("data/geo_hub"),
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

        let cfg = builder.build()?;
        let mut config: HubConfig = cfg.try_deserialize()?;
        if config.database_url.starts_with("sqlite://") && !config.database_url.contains('?') {
            // Enable WAL mode for better concurrency
            config.database_url.push_str("?mode=rwc");
        }
        Ok(config)
    }

    pub fn ensure_data_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_root)?;
        std::fs::create_dir_all(self.data_root.join("scenes"))?;
        Ok(())
    }
}
