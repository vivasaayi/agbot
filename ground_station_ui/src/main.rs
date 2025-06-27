use anyhow::Result;
use clap::Parser;
use ground_station_ui::{GroundStationUI, Args};
use shared::init_logging;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    
    let _args = Args::parse();
    info!("Starting Ground Station UI");

    let ui = GroundStationUI::new().await?;
    ui.run().await?;

    Ok(())
}
