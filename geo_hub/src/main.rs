use anyhow::Context;
use geo_hub::{db, serve, HubConfig};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Load configuration (from GEO_HUB_* env vars or geo_hub.{toml})
    let config = HubConfig::load().context("failed to load hub config")?;
    config
        .ensure_data_dirs()
        .context("failed to create data directories")?;

    // Connect to database pool
    let pool = db::connect_pool(&config)
        .await
        .context("failed to connect database")?;

    info!(
        bind = %config.bind_address,
        runtime_mode = %config.runtime_mode,
        landsat_source = %config.landsat.source,
        "starting geo_hub"
    );
    serve(config, pool).await
}
