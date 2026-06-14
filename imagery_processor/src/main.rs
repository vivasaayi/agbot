use anyhow::Result;
use clap::Parser;
use imagery_processor::{Cli, Commands, Processor};
use shared::init_logging;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;

    let cli = Cli::parse();
    let proc = Processor::new().await?;

    match cli.command {
        Commands::Indices(args) => {
            info!("Running indices: {:?}", args.index);
            proc.run_indices(&args).await?;
        }
        Commands::Thermal(args) => {
            info!("Running thermal processing");
            proc.run_thermal(&args).await?;
        }
        Commands::Classify(args) => {
            info!("Running classification");
            proc.run_classify(&args).await?;
        }
        Commands::Masks(args) => {
            info!("Running masks processing");
            proc.run_masks(&args).await?;
        }
        Commands::Export(args) => {
            info!("Running product export");
            proc.run_export(&args).await?;
        }
    }

    Ok(())
}
