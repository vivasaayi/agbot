//! CHIRPS precipitation registration + SPI raster derivation (satellite
//! pipeline batch 9).
//!
//! Two responsibilities, both catalog-driven:
//! - **CHIRPS ingest**: register local CHIRPS v2.0 monthly GeoTIFFs
//!   (`chirps-v2.0.YYYY.MM.tif`, EPSG:4326, mm accumulations) as L2
//!   `precipitation` products. Files are fetched out-of-band (plain HTTPS
//!   directory, no auth) — registration is offline and idempotent.
//! - **SPI derive**: score a current-month precipitation product against the
//!   multi-year record of same-month, same-grid products via the pure
//!   `post_processor::spi` engine, write the SPI GeoTIFF, and register it as
//!   an L3 `spi` product with lineage to the whole record. SPI rasters
//!   web-tile through the catalog tiler (geographic-grid support) and appear
//!   in `/api/stac` + `/browse`.
//!
//! Cadence note: monthly only — CHIRPS dekad files and multi-month
//! accumulation windows (SPI-3/6/12) are future work; the engine itself is
//! window-agnostic (it scores whatever accumulations it is given).

use std::path::{Path, PathBuf};

use chrono::Datelike;
use post_processor::spi::{
    compute_spi, spi_l3_draft, SpiClassCounts, SpiCurrentRaster, SpiError, SpiL3Scope,
    SpiObservation, SpiRequest, DEFAULT_SPI_MIN_YEARS,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, observed_on,
    DroughtRasterError, SkippedObservation,
};

/// Catalog kind for precipitation accumulations.
pub const PRECIPITATION_KIND: &str = "precipitation";
/// Source id stamped on CHIRPS registrations.
pub const CHIRPS_SOURCE_ID: &str = "chirps-v2.0";
/// Nodata for SPI GeoTIFFs (workspace index-nodata convention).
pub const SPI_NODATA: f32 = crate::satellite_derivation::INDEX_NODATA;

#[derive(Debug, Error)]
pub enum SpiRasterError {
    #[error("current product {0} is not in the catalog")]
    CurrentNotFound(String),
    #[error("current product {product_id} kind {kind:?} is not {PRECIPITATION_KIND:?}")]
    NotPrecipitation { product_id: String, kind: String },
    #[error("no usable record observations share the current product's month and grid (all {skipped} candidates skipped)")]
    NoUsableRecord { skipped: usize },
    #[error("SPI computation failed: {0}")]
    Spi(#[from] SpiError),
    #[error(transparent)]
    Shared(#[from] DroughtRasterError),
    #[error("raster I/O failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] CatalogError),
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl SpiRasterError {
    pub fn is_client_error(&self) -> bool {
        match self {
            SpiRasterError::CurrentNotFound(_)
            | SpiRasterError::NotPrecipitation { .. }
            | SpiRasterError::NoUsableRecord { .. } => true,
            SpiRasterError::Shared(shared) => shared.is_client_error(),
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// CHIRPS registration
// ---------------------------------------------------------------------------

/// Year/month parsed from a CHIRPS v2.0 monthly filename
/// (`chirps-v2.0.YYYY.MM.tif`, case-insensitive, `.tiff` accepted).
pub fn parse_chirps_monthly_filename(name: &str) -> Option<(i32, u32)> {
    let lower = name.to_ascii_lowercase();
    let stem = lower
        .strip_suffix(".tif")
        .or_else(|| lower.strip_suffix(".tiff"))?;
    let rest = stem.strip_prefix("chirps-v2.0.")?;
    let (year, month) = rest.split_once('.')?;
    if year.len() != 4 || month.len() != 2 {
        return None;
    }
    let year: i32 = year.parse().ok()?;
    let month: u32 = month.parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some((year, month))
}

/// Last day of a month (proleptic Gregorian).
fn last_day_of_month(year: i32, month: u32) -> u32 {
    for day in (28..=31).rev() {
        if chrono::NaiveDate::from_ymd_opt(year, month, day).is_some() {
            return day;
        }
    }
    28
}

/// Build the L2 draft for one CHIRPS monthly GeoTIFF.
pub fn chirps_monthly_draft(path: &Path, year: i32, month: u32) -> ProductRecordDraft {
    let last_day = last_day_of_month(year, month);
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: PRECIPITATION_KIND.to_string(),
        algorithm_id: "chirps.ingest.monthly".to_string(),
        algorithm_version: "2.0".to_string(),
        parameters: serde_json::json!({
            "dataset": "CHIRPS-2.0 monthly",
            "provider": "UCSB Climate Hazards Center",
            "year": year,
            "month": month,
            "units": "mm",
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: None,
            temporal_start: format!("{year}-{month:02}-01T00:00:00Z"),
            temporal_end: format!("{year}-{month:02}-{last_day:02}T23:59:59Z"),
        },
        spatial_ref: None,
        gsd_m_per_px: None,
        artifact: Some(ProductArtifact {
            format: "tif".to_string(),
            path: path.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(CHIRPS_SOURCE_ID.to_string()),
    }
}

/// Outcome of a CHIRPS directory registration.
#[derive(Debug, Clone, Serialize)]
pub struct ChirpsRegisterOutcome {
    /// (filename, product_id) for every registered file, name order.
    pub registered: Vec<(String, String)>,
    /// Filenames that did not match the CHIRPS monthly pattern.
    pub skipped: Vec<String>,
}

/// Register every CHIRPS monthly GeoTIFF in a local directory. Idempotent
/// (identity is content-addressed on year/month parameters); non-matching
/// files are listed as skipped, never errors.
pub async fn register_chirps_dir(
    pool: &DbPool,
    dir: &Path,
) -> Result<ChirpsRegisterOutcome, SpiRasterError> {
    let mut names: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .map_err(|source| SpiRasterError::Store {
            what: "chirps directory listing",
            source,
        })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            Some((name, entry.path()))
        })
        .collect();
    names.sort();

    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut registered = Vec::new();
    let mut skipped = Vec::new();
    for (name, path) in names {
        match parse_chirps_monthly_filename(&name) {
            Some((year, month)) => {
                let draft = chirps_monthly_draft(&path, year, month);
                let product_id = catalog::register_product_with_actor(
                    pool,
                    &draft,
                    &provenance::ActorIdentity::system("geo_hub:chirps_ingest"),
                    &created_at,
                )
                .await?;
                registered.push((name, product_id));
            }
            None => skipped.push(name),
        }
    }
    Ok(ChirpsRegisterOutcome {
        registered,
        skipped,
    })
}

// ---------------------------------------------------------------------------
// SPI derivation
// ---------------------------------------------------------------------------

/// An SPI raster derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct SpiDeriveRequest {
    /// Catalog id of the current-month precipitation product to score.
    pub current_product_id: String,
    pub field_id: String,
    pub season_id: String,
    #[serde(default = "default_min_years")]
    pub min_years: u32,
}

fn default_min_years() -> u32 {
    DEFAULT_SPI_MIN_YEARS
}

/// Outcome of one SPI derivation.
#[derive(Debug, Clone, Serialize)]
pub struct SpiDeriveOutcome {
    pub spi_product_id: String,
    /// Calendar month scored (1..=12).
    pub month: u32,
    pub record_years: Vec<i32>,
    pub valid_fraction: f32,
    pub class_counts: SpiClassCounts,
    pub clamp_count: u32,
    pub observations_used: Vec<String>,
    pub observations_skipped: Vec<SkippedObservation>,
    pub spi_artifact: PathBuf,
    pub spi_stac_item_href: String,
    pub spi_tiles_href: String,
}

/// Derive an SPI raster: gather every registered same-month, same-grid
/// precipitation product as the record (current included), run the pure SPI
/// engine, write the GeoTIFF, and register the `spi` L3 with lineage.
/// Idempotent on identical inputs.
pub async fn derive_spi_raster(
    pool: &DbPool,
    data_root: &Path,
    request: &SpiDeriveRequest,
) -> Result<SpiDeriveOutcome, SpiRasterError> {
    let current = catalog::get_product(pool, &request.current_product_id)
        .await?
        .ok_or_else(|| SpiRasterError::CurrentNotFound(request.current_product_id.clone()))?;
    if current.kind != PRECIPITATION_KIND {
        return Err(SpiRasterError::NotPrecipitation {
            product_id: current.product_id.clone(),
            kind: current.kind.clone(),
        });
    }
    let current_date = observed_on(&current).ok_or_else(|| DroughtRasterError::BadTemporal {
        product_id: current.product_id.clone(),
        value: current.temporal_start.clone(),
    })?;
    let current_raster = load_raster(Path::new(geotiff_artifact_path(&current)?))?;
    // Negative accumulations are physically invalid (CHIRPS uses -9999
    // nodata, already masked; belt-and-braces for other sources).
    let current_mask: Vec<bool> = current_raster
        .valid_mask
        .iter()
        .zip(&current_raster.values)
        .map(|(valid, value)| *valid && *value >= 0.0)
        .collect();

    let candidates = catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some(PRECIPITATION_KIND.to_string()),
            level: Some(ProductLevel::L2),
            status: Some("registered".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;

    let mut observations = Vec::new();
    let mut used_ids = Vec::new();
    let mut skipped = Vec::new();
    let skip = |product_id: &str, reason: &str, list: &mut Vec<SkippedObservation>| {
        list.push(SkippedObservation {
            product_id: product_id.to_string(),
            reason: reason.to_string(),
        });
    };
    for candidate in &candidates {
        let is_current = candidate.product_id == current.product_id;
        let Some(date) = observed_on(candidate) else {
            skip(&candidate.product_id, "bad_temporal", &mut skipped);
            continue;
        };
        if date.month() != current_date.month() {
            // Different calendar month: not part of this SPI record, and
            // not worth reporting as skipped noise.
            continue;
        }
        let raster = if is_current {
            None
        } else {
            let Ok(path) = geotiff_artifact_path(candidate) else {
                skip(&candidate.product_id, "no_artifact", &mut skipped);
                continue;
            };
            match load_raster(Path::new(path)) {
                Ok(raster) => {
                    if raster.epsg != current_raster.epsg
                        || raster.geo_transform != current_raster.geo_transform
                        || (raster.width, raster.height)
                            != (current_raster.width, current_raster.height)
                    {
                        skip(&candidate.product_id, "grid_mismatch", &mut skipped);
                        continue;
                    }
                    Some(raster)
                }
                Err(_) => {
                    skip(&candidate.product_id, "unreadable", &mut skipped);
                    continue;
                }
            }
        };
        let (values, mask) = match &raster {
            Some(raster) => (raster.values.clone(), raster.valid_mask.clone()),
            None => (current_raster.values.clone(), current_mask.clone()),
        };
        observations.push(SpiObservation {
            product_id: candidate.product_id.clone(),
            observed_on: date,
            values,
            valid_mask: mask,
            spatial_ref: current_raster.spatial_ref.clone(),
        });
        used_ids.push(candidate.product_id.clone());
    }
    if observations.is_empty() {
        return Err(SpiRasterError::NoUsableRecord {
            skipped: skipped.len(),
        });
    }

    let result = compute_spi(&SpiRequest {
        current: SpiCurrentRaster {
            product_id: current.product_id.clone(),
            width: current_raster.width,
            height: current_raster.height,
            spatial_ref: current_raster.spatial_ref.clone(),
            values: current_raster.values.clone(),
            valid_mask: current_mask,
        },
        observations,
        min_years: request.min_years,
    })?;

    // --- SPI GeoTIFF + L3 registration.
    let mut draft = spi_l3_draft(
        &result,
        &SpiL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            temporal_start: current
                .temporal_start
                .clone()
                .unwrap_or_else(|| format!("{current_date}T00:00:00Z")),
            temporal_end: current
                .temporal_end
                .clone()
                .unwrap_or_else(|| format!("{current_date}T23:59:59Z")),
            source_id: current.source_id.clone(),
        },
    );
    let spi_dir = data_root.join("derived").join("spi");
    std::fs::create_dir_all(&spi_dir).map_err(|source| SpiRasterError::Store {
        what: "spi directory",
        source,
    })?;
    let spi_path = spi_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    let disk_values: Vec<f32> = result
        .values
        .iter()
        .map(|v| if v.is_finite() { *v } else { SPI_NODATA })
        .collect();
    write_geotiff_f32(
        &spi_path,
        result.width,
        result.height,
        &disk_values,
        &GeoTiffTags {
            epsg: current_raster.epsg,
            geo_transform: current_raster.geo_transform,
            nodata: Some(f64::from(SPI_NODATA)),
        },
    )?;
    let checksum = file_checksum(&spi_path, "spi raster readback")?;
    draft.spatial_ref = Some(current_raster.spatial_ref.clone());
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: spi_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    let actor = provenance::ActorIdentity::system("geo_hub:spi_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let spi_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(SpiDeriveOutcome {
        spi_stac_item_href: format!("/api/stac/collections/spi/items/{spi_product_id}"),
        spi_tiles_href: format!(
            "/api/catalog/products/{spi_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
        spi_product_id,
        month: current_date.month(),
        record_years: result.evidence.record_years.clone(),
        valid_fraction: result.valid_fraction,
        class_counts: result.class_counts.clone(),
        clamp_count: result.clamp_count,
        observations_used: used_ids,
        observations_skipped: skipped,
        spi_artifact: spi_path,
    })
}

/// List registered SPI L3 products, optionally scoped to a field.
pub async fn list_spi_products(
    pool: &DbPool,
    field_id: Option<String>,
) -> Result<Vec<RegisteredProduct>, CatalogError> {
    catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some("spi".to_string()),
            level: Some(ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id,
            ..ProductFilter::default()
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chirps_filename_parsing_is_pinned() {
        assert_eq!(
            parse_chirps_monthly_filename("chirps-v2.0.2024.06.tif"),
            Some((2024, 6))
        );
        assert_eq!(
            parse_chirps_monthly_filename("CHIRPS-v2.0.1981.01.TIFF"),
            Some((1981, 1))
        );
        for bad in [
            "chirps-v2.0.2024.13.tif",    // month out of range
            "chirps-v2.0.2024.6.tif",     // month not zero-padded
            "chirps-v2.0.2024.06.nc",     // wrong extension
            "chirps-v3.0.2024.06.tif",    // wrong version prefix
            "chirps-v2.0.2024.06.05.tif", // daily file
            "notes.txt",
        ] {
            assert_eq!(parse_chirps_monthly_filename(bad), None, "{bad}");
        }
    }

    #[test]
    fn chirps_draft_spans_the_calendar_month() {
        let draft = chirps_monthly_draft(Path::new("/data/chirps-v2.0.2024.02.tif"), 2024, 2);
        assert_eq!(draft.kind, PRECIPITATION_KIND);
        assert_eq!(draft.level, ProductLevel::L2);
        assert_eq!(draft.scope.temporal_start, "2024-02-01T00:00:00Z");
        // 2024 is a leap year.
        assert_eq!(draft.scope.temporal_end, "2024-02-29T23:59:59Z");
        assert_eq!(draft.source_id.as_deref(), Some(CHIRPS_SOURCE_ID));
        // Identity distinguishes months.
        let other = chirps_monthly_draft(Path::new("/data/chirps-v2.0.2024.03.tif"), 2024, 3);
        assert_ne!(draft.product_id(), other.product_id());
    }
}
