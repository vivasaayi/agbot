use anyhow::Result;
use clap::Parser;
use std::{fs::File, io::BufReader, path::PathBuf};

#[derive(Parser)]
#[command(about = "Convert FlightSim LiDAR JSONL into mapper scan files")]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output_dir: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let summary = data_collector::sim_lidar_bridge::export_sim_lidar_scans(
        BufReader::new(File::open(args.input)?),
        &args.output_dir,
    )?;
    println!(
        "Exported {} scans; {} scans had no hits",
        summary.exported, summary.no_hits
    );
    Ok(())
}
