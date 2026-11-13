//! Geographic elevation from FlightSim world-space returns (x east, y up, z north).
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::schemas::{RasterResolution, RasterSpatialRef};
use std::{fs, path::Path};

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
#[derive(Deserialize)]
struct Scan {
    status: String,
    timestamp: String,
    points: Vec<WorldPoint>,
}

/// Uses the same small-area geographic conversion as MissionLoader.cpp.
/// The origin must be the mission home; it is recorded in product identity.
pub fn export(
    input: &Path,
    output: &Path,
    latitude: f64,
    longitude: f64,
    resolution: f64,
) -> Result<String> {
    ensure!(
        latitude.is_finite()
            && latitude.abs() < 85.0
            && longitude.is_finite()
            && longitude.abs() <= 180.0,
        "invalid or unsupported geographic origin"
    );
    ensure!(
        resolution.is_finite() && resolution > 0.0,
        "resolution must be positive"
    );
    let bytes = fs::read(input)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let mut points = Vec::new();
    let mut timestamp = None;
    let mut end_timestamp = None;
    for (i, line) in std::str::from_utf8(&bytes)?.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let scan: Scan =
            serde_json::from_str(line).with_context(|| format!("scan line {}", i + 1))?;
        let parsed = chrono::DateTime::parse_from_rfc3339(&scan.timestamp)?;
        end_timestamp = Some(
            end_timestamp.map_or(parsed, |prior: chrono::DateTime<chrono::FixedOffset>| {
                prior.max(parsed)
            }),
        );
        timestamp = Some(
            timestamp.map_or(parsed, |prior: chrono::DateTime<chrono::FixedOffset>| {
                prior.min(parsed)
            }),
        );
        if scan.status == "no_hits" && scan.points.is_empty() {
            continue;
        }
        ensure!(
            scan.status == "ok" && !scan.points.is_empty(),
            "sensor status {} on line {}",
            scan.status,
            i + 1
        );
        for p in scan.points {
            ensure!(
                p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                "nonfinite world point"
            );
            points.push(p);
        }
    }
    ensure!(!points.is_empty(), "no world-space returns");
    let min_x = points
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, f64::min)
        .div_euclid(resolution)
        * resolution;
    let max_z = points.iter().map(|p| p.z).fold(f64::NEG_INFINITY, f64::max);
    let top = (max_z / resolution).ceil() * resolution + resolution;
    let width = ((points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max) - min_x)
        / resolution)
        .floor()
        + 1.0;
    let height = ((top - points.iter().map(|p| p.z).fold(f64::INFINITY, f64::min)) / resolution)
        .floor()
        + 1.0;
    ensure!(
        width >= 1.0 && height >= 1.0 && width * height <= 16_000_000.0,
        "raster exceeds 16 million cell budget"
    );
    let (w, h) = (width as usize, height as usize);
    let mut sums = vec![0.0; w * h];
    let mut counts = vec![0u64; w * h];
    for p in &points {
        let x = ((p.x - min_x) / resolution).floor() as usize;
        let y = ((top - p.z) / resolution).floor() as usize;
        ensure!(x < w && y < h, "point outside raster");
        sums[y * w + x] += p.y;
        counts[y * w + x] += 1;
    }
    let pixels: Vec<f32> = sums
        .iter()
        .zip(&counts)
        .map(|(s, c)| {
            if *c == 0 {
                -9999.0
            } else {
                (s / *c as f64) as f32
            }
        })
        .collect();
    if pixels.iter().any(|v| !v.is_finite()) {
        bail!("elevation outside float range");
    }
    let lon_scale = 111_320.0 * latitude.to_radians().cos().abs().max(0.01);
    let transform = [
        longitude + min_x / lon_scale,
        resolution / lon_scale,
        0.0,
        latitude + top / 111_320.0,
        0.0,
        -resolution / 111_320.0,
    ];
    ensure!(
        transform[0] >= -180.0
            && transform[0] + width * transform[1] <= 180.0
            && transform[3] <= 90.0
            && transform[3] + height * transform[5] >= -90.0,
        "raster crosses geographic limits"
    );
    fs::create_dir_all(output)?;
    let raster = output.join("sim_elevation.tif");
    raster_io::write_geotiff_f32(
        &raster,
        w as u32,
        h as u32,
        &pixels,
        &raster_io::GeoTiffTags {
            epsg: Some(4326),
            geo_transform: Some(transform),
            nodata: Some(-9999.0),
        },
    )?;
    let time = timestamp.context("missing timestamp")?.to_rfc3339();
    let mut draft = ProductRecordDraft {
        level: ProductLevel::L0,
        kind: "sim_lidar_world".into(),
        algorithm_id: "sim.lidar.capture".into(),
        algorithm_version: "1".into(),
        parameters: serde_json::json!({"checksum_sha256":hash}),
        inputs: vec![],
        scope: ProductScope {
            temporal_start: time.clone(),
            temporal_end: end_timestamp.context("missing end timestamp")?.to_rfc3339(),
            ..Default::default()
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: Some(ProductArtifact {
            path: fs::canonicalize(input)?.to_string_lossy().into_owned(),
            format: "jsonl".into(),
            checksum_sha256: Some(hash),
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: vec![],
        source_id: None,
    };
    fs::write(
        output.join("world_capture.product_record.json"),
        serde_json::to_vec_pretty(&draft)?,
    )?;
    let parent = draft.product_id();
    draft.level = ProductLevel::L2;
    draft.kind = "sim_elevation".into();
    draft.algorithm_id = "sim.lidar.world_mean_elevation".into();
    draft.parameters = serde_json::json!({"origin_latitude":latitude,"origin_longitude":longitude,"resolution_m":resolution,"coordinate_convention":"x_east_y_up_z_north","simulation":true});
    draft.inputs = vec![ProductInputRef {
        product_id: parent,
        role: "world_returns".into(),
    }];
    draft.spatial_ref = Some(RasterSpatialRef {
        georeferenced: true,
        crs: Some("EPSG:4326".into()),
        geo_transform: Some(transform),
        bbox: Some(shared::schemas::GeoBounds {
            min_lon: transform[0],
            max_lon: transform[0] + width * transform[1],
            min_lat: transform[3] + height * transform[5],
            max_lat: transform[3],
        }),
        resolution: Some(RasterResolution {
            x: transform[1],
            y: -transform[5],
        }),
    });
    draft.gsd_m_per_px = Some(resolution);
    draft.artifact = Some(ProductArtifact {
        path: fs::canonicalize(&raster)?.to_string_lossy().into_owned(),
        format: "tif".into(),
        checksum_sha256: Some(format!("{:x}", Sha256::digest(fs::read(&raster)?))),
    });
    draft.quality_summary = Some(
        serde_json::json!({"simulation":true,"returns":points.len(),"observed_cells":counts.iter().filter(|c|**c>0).count(),"vertical_reference":"simulator local y; no absolute vertical datum asserted"}),
    );
    fs::write(
        output.join("sim_elevation.product_record.json"),
        serde_json::to_vec_pretty(&draft)?,
    )?;
    Ok(draft.product_id())
}
