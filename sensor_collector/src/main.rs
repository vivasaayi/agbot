use anyhow::Result;
use clap::Parser;
use sensor_collector::{SensorCollectorService, Args};
use shared::init_logging;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    
    let _args = Args::parse();
    info!("Starting Sensor Collector Service");

    let service = SensorCollectorService::new().await?;
    service.run().await?;

    Ok(())
}
