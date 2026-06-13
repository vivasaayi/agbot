use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::{
    error::AgroError,
    schemas::{GeoBounds, RasterResolution, RasterSpatialRef, GEO_EXTENT_ASSERTION_TOLERANCE},
    AgroResult,
};
use std::path::{Path, PathBuf};

use crate::{io::GeoTiffSpatialSidecar, ExportArgs};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductExportStats {
    pub product: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub coverage: f64,
    pub units: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductExportReport {
    pub geotiff_path: PathBuf,
    pub spatial_sidecar_path: PathBuf,
    pub stats_csv_path: PathBuf,
    pub stats: ProductExportStats,
}

pub async fn run_export(args: &ExportArgs) -> AgroResult<ProductExportReport> {
    tokio::fs::create_dir_all(&args.output_dir).await?;
    let metadata_text = tokio::fs::read_to_string(&args.product_metadata).await?;
    let metadata: Value = serde_json::from_str(&metadata_text).map_err(|err| {
        processing_error(format!(
            "failed to parse product metadata {}: {err}",
            args.product_metadata.display()
        ))
    })?;

    let source_path = product_output_path(&metadata)?;
    require_geotiff_source(&source_path)?;
    let sidecar_path = crate::io::geotiff_spatial_sidecar_path(&source_path);
    let sidecar_text = tokio::fs::read_to_string(&sidecar_path)
        .await
        .map_err(|err| {
            processing_error(format!(
                "failed to read GeoTIFF spatial sidecar {}: {err}",
                sidecar_path.display()
            ))
        })?;
    let sidecar: GeoTiffSpatialSidecar = serde_json::from_str(&sidecar_text).map_err(|err| {
        processing_error(format!(
            "failed to parse GeoTIFF spatial sidecar {}: {err}",
            sidecar_path.display()
        ))
    })?;
    validate_sidecar_matches_metadata(&metadata, &sidecar)?;

    let geotiff_path = args.output_dir.join(file_name(&source_path)?);
    if geotiff_path != source_path {
        tokio::fs::copy(&source_path, &geotiff_path).await?;
    }
    let export_sidecar_path = crate::io::geotiff_spatial_sidecar_path(&geotiff_path);
    tokio::fs::write(&export_sidecar_path, serde_json::to_vec_pretty(&sidecar)?).await?;

    let stats = ProductExportStats::from_metadata(&metadata)?;
    stats.validate()?;
    let stats_csv_path = args
        .output_dir
        .join(format!("{}_stats.csv", file_stem(&source_path)?));
    tokio::fs::write(&stats_csv_path, stats.to_csv()).await?;

    Ok(ProductExportReport {
        geotiff_path,
        spatial_sidecar_path: export_sidecar_path,
        stats_csv_path,
        stats,
    })
}

impl ProductExportStats {
    fn from_metadata(metadata: &Value) -> AgroResult<Self> {
        let product = metadata_string(metadata, &["product", "index", "method"])
            .or_else(|| {
                metadata
                    .get("unit")
                    .and_then(Value::as_str)
                    .map(|_| "thermal".to_string())
            })
            .ok_or_else(|| processing_error("product metadata missing product/index/method"))?;
        let units =
            metadata_string(metadata, &["unit", "units"]).unwrap_or_else(|| "index".to_string());
        let min = optional_number(metadata, &["min"], &["/reproducibility/statistics/min"])?;
        let max = optional_number(metadata, &["max"], &["/reproducibility/statistics/max"])?;
        let mean = optional_number(metadata, &["mean"], &["/reproducibility/statistics/mean"])?;
        let coverage = optional_number(
            metadata,
            &["valid_pixel_coverage", "coverage"],
            &["/reproducibility/coverage/valid_pixel_coverage"],
        )?
        .ok_or_else(|| processing_error("product metadata missing valid pixel coverage"))?;

        Ok(Self {
            product,
            min,
            max,
            mean,
            coverage,
            units,
        })
    }

    fn validate(&self) -> AgroResult<()> {
        if self.product.trim().is_empty() {
            return Err(processing_error("stats CSV product must not be empty"));
        }
        if self.units.trim().is_empty() {
            return Err(processing_error("stats CSV units must not be empty"));
        }
        if !(0.0..=1.0).contains(&self.coverage) || !self.coverage.is_finite() {
            return Err(processing_error(format!(
                "stats CSV coverage must be finite and within 0..1, got {}",
                self.coverage
            )));
        }
        for (name, value) in [("min", self.min), ("max", self.max), ("mean", self.mean)] {
            if value.is_some_and(|number| !number.is_finite()) {
                return Err(processing_error(format!(
                    "stats CSV {name} must be finite when present"
                )));
            }
        }
        Ok(())
    }

    fn to_csv(&self) -> String {
        format!(
            "product,min,max,mean,coverage,units\n{},{},{},{},{},{}\n",
            csv_cell(&self.product),
            csv_optional(self.min),
            csv_optional(self.max),
            csv_optional(self.mean),
            csv_number(self.coverage),
            csv_cell(&self.units)
        )
    }
}

fn product_output_path(metadata: &Value) -> AgroResult<PathBuf> {
    metadata
        .get("output_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| processing_error("product metadata missing output_path"))
}

fn require_geotiff_source(source_path: &Path) -> AgroResult<()> {
    let extension = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "tif" || extension == "tiff" {
        return Ok(());
    }

    Err(processing_error(format!(
        "GeoTIFF export requires a completed .tif/.tiff product; got {}",
        source_path.display()
    )))
}

fn validate_sidecar_matches_metadata(
    metadata: &Value,
    sidecar: &GeoTiffSpatialSidecar,
) -> AgroResult<()> {
    let Some(spatial_ref_value) = metadata.get("spatial_ref").filter(|value| !value.is_null())
    else {
        return Ok(());
    };
    let spatial_ref: RasterSpatialRef = serde_json::from_value(spatial_ref_value.clone())
        .map_err(|err| processing_error(format!("invalid product spatial_ref: {err}")))?;
    let expected = GeoTiffSpatialSidecar::from_spatial_ref(&spatial_ref)?;

    if sidecar.crs != expected.crs
        || !bounds_match(&sidecar.bbox, &expected.bbox)
        || !resolution_match(sidecar.resolution, expected.resolution)
        || !transform_match(&sidecar.geo_transform, &expected.geo_transform)
    {
        return Err(processing_error(
            "GeoTIFF spatial sidecar does not match product spatial_ref",
        ));
    }

    Ok(())
}

fn metadata_string(metadata: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn optional_number(metadata: &Value, keys: &[&str], pointers: &[&str]) -> AgroResult<Option<f64>> {
    let value = keys.iter().find_map(|key| metadata.get(*key)).or_else(|| {
        pointers
            .iter()
            .find_map(|pointer| metadata.pointer(pointer))
    });
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|number| number.is_finite())
            .map(Some)
            .ok_or_else(|| processing_error(format!("metadata value {value} is not finite"))),
    }
}

fn bounds_match(left: &GeoBounds, right: &GeoBounds) -> bool {
    (left.min_lon - right.min_lon).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.min_lat - right.min_lat).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lon - right.max_lon).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.max_lat - right.max_lat).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
}

fn resolution_match(left: RasterResolution, right: RasterResolution) -> bool {
    (left.x - right.x).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
        && (left.y - right.y).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE
}

fn transform_match(left: &[f64; 6], right: &[f64; 6]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| (*left - *right).abs() <= GEO_EXTENT_ASSERTION_TOLERANCE)
}

fn file_name(path: &Path) -> AgroResult<&std::ffi::OsStr> {
    path.file_name()
        .ok_or_else(|| processing_error(format!("path has no file name: {}", path.display())))
}

fn file_stem(path: &Path) -> AgroResult<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToString::to_string)
        .ok_or_else(|| processing_error(format!("path has no UTF-8 file stem: {}", path.display())))
}

fn csv_optional(value: Option<f64>) -> String {
    value.map(csv_number).unwrap_or_default()
}

fn csv_number(value: f64) -> String {
    format!("{value}")
}

fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn processing_error(message: impl Into<String>) -> AgroError {
    AgroError::Processing(message.into())
}
