use anyhow::Result;
use clap::Parser;
use mission_control::{MissionControlService, Args};
use shared::init_logging;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    
    let _args = Args::parse();
    info!("Starting Mission Control Service");

    let service = MissionControlService::new().await?;
    service.run().await?;

    Ok(())
}
