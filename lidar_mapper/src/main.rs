use anyhow::Result;
use clap::Parser;
use lidar_mapper::{LidarMapper, Args};
use shared::init_logging;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    
    let args = Args::parse();
    info!("Starting LiDAR Mapper");

    let mapper = LidarMapper::new().await?;
    mapper.process_directory(&args.input_dir, &args.output_dir).await?;

    Ok(())
}
