use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
#[derive(Parser)]
#[command(about = "Map FlightSim world returns with an explicit mission-home origin")]
struct Args {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long)]
    origin_latitude: f64,
    #[arg(long)]
    origin_longitude: f64,
    #[arg(long, default_value_t = 1.0)]
    resolution: f64,
}
fn main() -> Result<()> {
    let a = Args::parse();
    println!(
        "{}",
        lidar_mapper::sim_georef::export(
            &a.input,
            &a.output_dir,
            a.origin_latitude,
            a.origin_longitude,
            a.resolution
        )?
    );
    Ok(())
}
