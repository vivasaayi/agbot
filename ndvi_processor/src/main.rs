use anyhow::Result;
use clap::Parser;
use ndvi_processor::{NdviProcessor, Args};
use shared::init_logging;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    
    let args = Args::parse();
    info!("Starting NDVI Processor");

    let processor = NdviProcessor::new().await?;
    processor.process_directory(&args.input_dir, &args.output_dir).await?;

    Ok(())
}
