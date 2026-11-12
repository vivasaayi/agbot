//! Convert FlightSim LiDAR JSONL into the mapper's per-scan JSON format.
use crate::{flight_sim_cpp_lidar_observation_from_json, SimulatedSensorObservation};
use anyhow::{bail, Context, Result};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExportSummary {
    pub exported: usize,
    pub no_hits: usize,
}

pub fn export_sim_lidar_scans(input: impl BufRead, output: &Path) -> Result<ExportSummary> {
    fs::create_dir_all(output)?;
    let mut summary = ExportSummary::default();
    for (index, line) in input.lines().enumerate() {
        let line = line.context("reading LiDAR input")?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("invalid JSON on line {}", index + 1))?;
        let observation = flight_sim_cpp_lidar_observation_from_json(&line, "flight_sim_cpp")
            .with_context(|| format!("invalid scan on line {}", index + 1))?;
        match observation {
            SimulatedSensorObservation::Lidar { mut scan, .. } => {
                // The collector adapter uses metres; the mapper's scan-file
                // contract currently expects millimetres.
                for point in &mut scan.points {
                    point.distance *= 1000.0;
                    if !point.distance.is_finite()
                        || point.distance <= 0.0
                        || !point.angle.is_finite()
                    {
                        bail!("line {}: invalid LiDAR range or angle", index + 1);
                    }
                }
                let path = output.join(format!("scan_{}.json", scan.scan_id));
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .with_context(|| format!("creating {}", path.display()))?;
                serde_json::to_writer(&mut file, &scan)?;
                file.write_all(b"\n")?;
                summary.exported += 1;
            }
            SimulatedSensorObservation::Failure { .. }
                if value["status"] == "no_hits"
                    && value["points"].as_array().is_some_and(|p| p.is_empty()) =>
            {
                summary.no_hits += 1;
            }
            SimulatedSensorObservation::Failure { message, .. } => {
                bail!("line {}: {}", index + 1, message)
            }
            _ => bail!("line {}: unexpected sensor observation", index + 1),
        }
    }
    if summary.exported == 0 {
        bail!("no LiDAR scans with hits were exported");
    }
    Ok(summary)
}
