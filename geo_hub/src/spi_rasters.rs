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
//! Cadence note: SPI derivation is monthly (with SPI-N accumulation
//! windows). CHIRPS dekad files register as `precipitation` L2s with dekad
//! temporal bounds for downstream use, but are deterministically excluded
//! from monthly SPI windows (they do not span a full calendar month);
//! dekad-cadence SPI is future work.

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
/// Algorithm ids distinguishing CHIRPS cadences in the catalog.
pub const CHIRPS_MONTHLY_ALGORITHM: &str = "chirps.ingest.monthly";
pub const CHIRPS_DEKAD_ALGORITHM: &str = "chirps.ingest.dekad";
/// Default CHIRPS v2.0 HTTPS root (plain directory listing, no auth).
pub const CHIRPS_BASE_URL: &str = "https://data.chc.ucsb.edu/products/CHIRPS-2.0";
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
    #[error("window_months must be in 1..=12, got {0}")]
    InvalidWindowMonths(u32),
    #[error("the current {window_months}-month window is incomplete: no monthly precipitation product for {year}-{month:02}")]
    MissingWindowMonth {
        window_months: u32,
        year: i32,
        month: u32,
    },
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
    #[error("chirps fetch request invalid: {0}")]
    InvalidFetchRequest(String),
}

impl SpiRasterError {
    pub fn is_client_error(&self) -> bool {
        match self {
            SpiRasterError::CurrentNotFound(_)
            | SpiRasterError::NotPrecipitation { .. }
            | SpiRasterError::NoUsableRecord { .. }
            | SpiRasterError::InvalidWindowMonths(_)
            | SpiRasterError::MissingWindowMonth { .. }
            | SpiRasterError::InvalidFetchRequest(_) => true,
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

/// (year, month, dekad) parsed from a CHIRPS v2.0 dekad filename
/// (`chirps-v2.0.YYYY.MM.D.tif`, D in 1..=3).
pub fn parse_chirps_dekad_filename(name: &str) -> Option<(i32, u32, u8)> {
    let lower = name.to_ascii_lowercase();
    let stem = lower
        .strip_suffix(".tif")
        .or_else(|| lower.strip_suffix(".tiff"))?;
    let rest = stem.strip_prefix("chirps-v2.0.")?;
    let mut parts = rest.split('.');
    let (year, month, dekad) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || dekad.len() != 1 {
        return None;
    }
    let year: i32 = year.parse().ok()?;
    let month: u32 = month.parse().ok()?;
    let dekad: u8 = dekad.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=3).contains(&dekad) {
        return None;
    }
    Some((year, month, dekad))
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
        algorithm_id: CHIRPS_MONTHLY_ALGORITHM.to_string(),
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

/// Build the L2 draft for one CHIRPS dekad GeoTIFF (dekad 1 = days 1-10,
/// 2 = 11-20, 3 = 21-end of month).
pub fn chirps_dekad_draft(path: &Path, year: i32, month: u32, dekad: u8) -> ProductRecordDraft {
    let (first_day, last_day) = match dekad {
        1 => (1, 10),
        2 => (11, 20),
        _ => (21, last_day_of_month(year, month)),
    };
    let mut draft = chirps_monthly_draft(path, year, month);
    draft.algorithm_id = CHIRPS_DEKAD_ALGORITHM.to_string();
    draft.parameters = serde_json::json!({
        "dataset": "CHIRPS-2.0 dekad",
        "provider": "UCSB Climate Hazards Center",
        "year": year,
        "month": month,
        "dekad": dekad,
        "units": "mm",
    });
    draft.scope.temporal_start = format!("{year}-{month:02}-{first_day:02}T00:00:00Z");
    draft.scope.temporal_end = format!("{year}-{month:02}-{last_day:02}T23:59:59Z");
    draft
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
        let draft = if let Some((year, month)) = parse_chirps_monthly_filename(&name) {
            chirps_monthly_draft(&path, year, month)
        } else if let Some((year, month, dekad)) = parse_chirps_dekad_filename(&name) {
            chirps_dekad_draft(&path, year, month, dekad)
        } else {
            skipped.push(name);
            continue;
        };
        let product_id = catalog::register_product_with_actor(
            pool,
            &draft,
            &provenance::ActorIdentity::system("geo_hub:chirps_ingest"),
            &created_at,
        )
        .await?;
        registered.push((name, product_id));
    }
    Ok(ChirpsRegisterOutcome {
        registered,
        skipped,
    })
}

// ---------------------------------------------------------------------------
// CHIRPS HTTPS fetcher
// ---------------------------------------------------------------------------

/// Ceiling on files per fetch request (a 40-year dekad archive is 1440;
/// anything larger is almost certainly a malformed request).
pub const MAX_FETCH_FILES: usize = 1500;

type FetchFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send + 'a>>;

/// Seam for downloading CHIRPS files so tests run network-free (mirrors the
/// satellite `CogStoreResolver` pattern). Production is [`HttpChirpsFetcher`].
pub trait ChirpsFetcher: Send + Sync {
    fn fetch<'a>(&'a self, url: &'a str) -> FetchFuture<'a>;
}

/// Plain reqwest GET; non-2xx statuses are errors.
#[derive(Default)]
pub struct HttpChirpsFetcher {
    client: reqwest::Client,
}

impl ChirpsFetcher for HttpChirpsFetcher {
    fn fetch<'a>(&'a self, url: &'a str) -> FetchFuture<'a> {
        Box::pin(async move {
            let response = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|err| err.to_string())?;
            if !response.status().is_success() {
                return Err(format!("HTTP {}", response.status()));
            }
            response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|err| err.to_string())
        })
    }
}

/// Shareable fetcher handle carried as an axum request extension so route
/// tests can inject an in-memory fetcher.
#[derive(Clone)]
pub struct ChirpsFetcherHandle(pub std::sync::Arc<dyn ChirpsFetcher>);

/// A CHIRPS archive fetch request. Files land in `<data_root>/chirps/` and
/// register through the same idempotent path as directory registration.
#[derive(Debug, Clone, Deserialize)]
pub struct ChirpsFetchRequest {
    pub start_year: i32,
    pub end_year: i32,
    /// Calendar months to fetch (default: all twelve).
    #[serde(default)]
    pub months: Option<Vec<u32>>,
    /// `monthly` (default) or `dekad`.
    #[serde(default = "default_cadence")]
    pub cadence: String,
    /// Override the CHIRPS root (tests point at their fake fetcher's URLs).
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

fn default_cadence() -> String {
    "monthly".to_string()
}

fn default_base_url() -> String {
    CHIRPS_BASE_URL.to_string()
}

/// Outcome of one archive fetch.
#[derive(Debug, Clone, Serialize)]
pub struct ChirpsFetchOutcome {
    /// (filename, product_id) for files downloaded this call.
    pub fetched: Vec<(String, String)>,
    /// (filename, product_id) for files already on disk (registered anyway,
    /// idempotently — a resumed fetch converges).
    pub already_present: Vec<(String, String)>,
    /// (filename, error) for downloads that failed; the fetch continues.
    pub failed: Vec<(String, String)>,
}

/// Gunzip if the payload has the gzip magic, else pass through.
fn maybe_gunzip(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        use std::io::Read;
        let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .map_err(|err| format!("gunzip failed: {err}"))?;
        Ok(out)
    } else {
        Ok(bytes)
    }
}

/// Fetch a CHIRPS archive slice over HTTPS and register every file. Files
/// already on disk are not re-downloaded. Individual download failures are
/// reported per file, not fatal.
pub async fn fetch_chirps(
    pool: &DbPool,
    data_root: &Path,
    fetcher: &dyn ChirpsFetcher,
    request: &ChirpsFetchRequest,
) -> Result<ChirpsFetchOutcome, SpiRasterError> {
    if request.start_year > request.end_year {
        return Err(SpiRasterError::InvalidFetchRequest(format!(
            "start_year {} is after end_year {}",
            request.start_year, request.end_year
        )));
    }
    let months = match &request.months {
        Some(months) => {
            if months.is_empty() || months.iter().any(|m| !(1..=12).contains(m)) {
                return Err(SpiRasterError::InvalidFetchRequest(format!(
                    "months must be nonempty values in 1..=12, got {months:?}"
                )));
            }
            months.clone()
        }
        None => (1..=12).collect(),
    };
    let dekads: &[Option<u8>] = match request.cadence.as_str() {
        "monthly" => &[None],
        "dekad" => &[Some(1), Some(2), Some(3)],
        other => {
            return Err(SpiRasterError::InvalidFetchRequest(format!(
                "cadence must be \"monthly\" or \"dekad\", got {other:?}"
            )))
        }
    };
    let year_count = (request.end_year - request.start_year + 1) as usize;
    let file_count = year_count * months.len() * dekads.len();
    if file_count > MAX_FETCH_FILES {
        return Err(SpiRasterError::InvalidFetchRequest(format!(
            "{file_count} files requested exceeds the {MAX_FETCH_FILES} per-request ceiling"
        )));
    }

    let chirps_dir = data_root.join("chirps");
    std::fs::create_dir_all(&chirps_dir).map_err(|source| SpiRasterError::Store {
        what: "chirps directory",
        source,
    })?;
    let base = request.base_url.trim_end_matches('/');
    let subdir = match request.cadence.as_str() {
        "dekad" => "global_dekad",
        _ => "global_monthly",
    };

    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let actor = provenance::ActorIdentity::system("geo_hub:chirps_fetch");
    let mut outcome = ChirpsFetchOutcome {
        fetched: Vec::new(),
        already_present: Vec::new(),
        failed: Vec::new(),
    };
    for year in request.start_year..=request.end_year {
        for month in &months {
            for dekad in dekads {
                let (filename, draft_builder): (String, _) = match dekad {
                    None => (format!("chirps-v2.0.{year}.{month:02}.tif"), None),
                    Some(dekad) => (
                        format!("chirps-v2.0.{year}.{month:02}.{dekad}.tif"),
                        Some(*dekad),
                    ),
                };
                let target = chirps_dir.join(&filename);
                let downloaded = if target.exists() {
                    false
                } else {
                    // CHIRPS serves gzipped tifs in the global directories.
                    let url = format!("{base}/{subdir}/tifs/{filename}.gz");
                    match fetcher.fetch(&url).await.and_then(maybe_gunzip) {
                        Ok(bytes) => {
                            if let Err(source) = std::fs::write(&target, bytes) {
                                return Err(SpiRasterError::Store {
                                    what: "fetched chirps file",
                                    source,
                                });
                            }
                            true
                        }
                        Err(error) => {
                            outcome.failed.push((filename, error));
                            continue;
                        }
                    }
                };
                let draft = match draft_builder {
                    None => chirps_monthly_draft(&target, year, *month),
                    Some(dekad) => chirps_dekad_draft(&target, year, *month, dekad),
                };
                let product_id =
                    catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;
                if downloaded {
                    outcome.fetched.push((filename, product_id));
                } else {
                    outcome.already_present.push((filename, product_id));
                }
            }
        }
    }
    Ok(outcome)
}

// ---------------------------------------------------------------------------
// SPI derivation
// ---------------------------------------------------------------------------

/// The (year, month) that is `back` calendar months before (year, month).
fn months_back(year: i32, month: u32, back: u32) -> (i32, u32) {
    let total = year as i64 * 12 + i64::from(month) - 1 - i64::from(back);
    (
        (total.div_euclid(12)) as i32,
        (total.rem_euclid(12) + 1) as u32,
    )
}

/// (year, month) of a product covering exactly one full calendar month
/// (temporal_start on day 1, temporal_end on that month's last day) — the
/// contract CHIRPS monthly registration writes. Anything else (dekads,
/// arbitrary spans) is not usable for monthly accumulation windows.
fn monthly_span(product: &RegisteredProduct) -> Option<(i32, u32)> {
    let start = observed_on(product)?;
    if start.day() != 1 {
        return None;
    }
    let end_text = product.temporal_end.as_deref()?;
    let end = chrono::NaiveDate::parse_from_str(end_text.get(..10)?, "%Y-%m-%d").ok()?;
    let (year, month) = (start.year(), start.month());
    if end.year() != year || end.month() != month || end.day() != last_day_of_month(year, month) {
        return None;
    }
    Some((year, month))
}

/// An SPI raster derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct SpiDeriveRequest {
    /// Catalog id of the precipitation product for the window's END month.
    pub current_product_id: String,
    pub field_id: String,
    pub season_id: String,
    #[serde(default = "default_min_years")]
    pub min_years: u32,
    /// Accumulation window in calendar months ending at the current
    /// product's month: 1 = classic monthly SPI, 3/6/12 = SPI-3/6/12.
    #[serde(default = "default_window_months")]
    pub window_months: u32,
}

fn default_min_years() -> u32 {
    DEFAULT_SPI_MIN_YEARS
}

fn default_window_months() -> u32 {
    1
}

/// Outcome of one SPI derivation.
#[derive(Debug, Clone, Serialize)]
pub struct SpiDeriveOutcome {
    pub spi_product_id: String,
    /// Calendar month the window ends in (1..=12).
    pub month: u32,
    pub window_months: u32,
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

    let window = request.window_months;
    if !(1..=12).contains(&window) {
        return Err(SpiRasterError::InvalidWindowMonths(window));
    }

    // Index every same-grid full-calendar-month precipitation raster by
    // (year, month). Deterministic: candidates sorted by product id; a
    // duplicate month keeps the first and reports the rest.
    let mut candidates = catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some(PRECIPITATION_KIND.to_string()),
            level: Some(ProductLevel::L2),
            status: Some("registered".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    candidates.sort_by(|a, b| a.product_id.cmp(&b.product_id));

    let mut skipped = Vec::new();
    let skip = |product_id: &str, reason: &str, list: &mut Vec<SkippedObservation>| {
        list.push(SkippedObservation {
            product_id: product_id.to_string(),
            reason: reason.to_string(),
        });
    };
    struct MonthRaster {
        product_id: String,
        values: Vec<f32>,
        valid_mask: Vec<bool>,
    }
    let mut months: std::collections::BTreeMap<(i32, u32), MonthRaster> =
        std::collections::BTreeMap::new();
    for candidate in &candidates {
        if candidate.algorithm_id == CHIRPS_DEKAD_ALGORITHM {
            // Dekads are a different cadence, not record noise: excluded
            // without a per-product skip entry (an archive can hold
            // thousands of them).
            continue;
        }
        let Some(key) = monthly_span(candidate) else {
            skip(
                &candidate.product_id,
                "not_a_full_calendar_month",
                &mut skipped,
            );
            continue;
        };
        if months.contains_key(&key) {
            skip(&candidate.product_id, "duplicate_month", &mut skipped);
            continue;
        }
        let is_current = candidate.product_id == current.product_id;
        let (values, valid_mask) = if is_current {
            (current_raster.values.clone(), current_mask.clone())
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
                    let mask: Vec<bool> = raster
                        .valid_mask
                        .iter()
                        .zip(&raster.values)
                        .map(|(valid, value)| *valid && *value >= 0.0)
                        .collect();
                    (raster.values, mask)
                }
                Err(_) => {
                    skip(&candidate.product_id, "unreadable", &mut skipped);
                    continue;
                }
            }
        };
        months.insert(
            key,
            MonthRaster {
                product_id: candidate.product_id.clone(),
                values,
                valid_mask,
            },
        );
    }

    let pixel_count = current_raster.values.len();
    let end_month = current_date.month();
    // Accumulate one window ending at (year, end_month): per-pixel sum with
    // an all-months-valid mask; None when any member month is missing.
    let window_of = |year: i32,
                     months: &std::collections::BTreeMap<(i32, u32), MonthRaster>|
     -> Option<(Vec<f32>, Vec<bool>, Vec<String>)> {
        let mut sum = vec![0.0f32; pixel_count];
        let mut mask = vec![true; pixel_count];
        let mut member_ids = Vec::with_capacity(window as usize);
        for back in 0..window {
            let member = months.get(&months_back(year, end_month, back))?;
            for pixel in 0..pixel_count {
                sum[pixel] += member.values[pixel];
                mask[pixel] &= member.valid_mask[pixel];
            }
            member_ids.push(member.product_id.clone());
        }
        Some((sum, mask, member_ids))
    };

    // The current window must be complete.
    let current_year = current_date.year();
    for back in 0..window {
        let (year, month) = months_back(current_year, end_month, back);
        if !months.contains_key(&(year, month)) {
            return Err(SpiRasterError::MissingWindowMonth {
                window_months: window,
                year,
                month,
            });
        }
    }
    let (current_values, current_window_mask, current_member_ids) =
        window_of(current_year, &months).expect("checked complete above");

    // Record: one accumulated observation per year whose window is complete
    // (current year included — the fit uses the full record).
    let mut observations = Vec::new();
    let mut used_ids = Vec::new();
    let mut member_lineage: Vec<String> = current_member_ids.clone();
    let record_year_candidates: std::collections::BTreeSet<i32> = months
        .keys()
        .filter(|(_, month)| *month == end_month)
        .map(|(year, _)| *year)
        .collect();
    for year in record_year_candidates {
        match window_of(year, &months) {
            Some((values, mask, member_ids)) => {
                let ending = months
                    .get(&(year, end_month))
                    .expect("window complete implies ending month");
                observations.push(SpiObservation {
                    product_id: ending.product_id.clone(),
                    observed_on: chrono::NaiveDate::from_ymd_opt(year, end_month, 1)
                        .expect("valid month start"),
                    values,
                    valid_mask: mask,
                    spatial_ref: current_raster.spatial_ref.clone(),
                });
                used_ids.push(ending.product_id.clone());
                for member in member_ids {
                    if !member_lineage.contains(&member) {
                        member_lineage.push(member);
                    }
                }
            }
            None => skip(
                &format!("window:{year}-{end_month:02}"),
                "incomplete_window",
                &mut skipped,
            ),
        }
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
            values: current_values,
            valid_mask: current_window_mask,
        },
        observations,
        min_years: request.min_years,
    })?;

    // --- SPI GeoTIFF + L3 registration. The scope spans the whole
    // accumulation window; the window length is stamped into the parameters
    // so SPI-1 and SPI-3 for the same end month get distinct identities.
    let (start_year, start_month) = months_back(current_year, end_month, window - 1);
    let mut draft = spi_l3_draft(
        &result,
        &SpiL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            temporal_start: format!("{start_year}-{start_month:02}-01T00:00:00Z"),
            temporal_end: current
                .temporal_end
                .clone()
                .unwrap_or_else(|| format!("{current_date}T23:59:59Z")),
            source_id: current.source_id.clone(),
        },
    );
    draft
        .parameters
        .as_object_mut()
        .expect("spi parameters are an object")
        .insert(
            "accumulation".to_string(),
            serde_json::json!({ "window_months": window, "cadence": "monthly" }),
        );
    // Lineage: spi_l3_draft covers the window-ending products; add every
    // other window member so the trace reaches all consumed months.
    for member in &member_lineage {
        if !draft.inputs.iter().any(|edge| &edge.product_id == member) {
            draft.inputs.push(shared::product_graph::ProductInputRef {
                product_id: member.clone(),
                role: "accumulation_member".to_string(),
            });
        }
    }
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
        month: end_month,
        window_months: window,
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
    fn chirps_dekad_filename_parsing_is_pinned() {
        assert_eq!(
            parse_chirps_dekad_filename("chirps-v2.0.2024.06.1.tif"),
            Some((2024, 6, 1))
        );
        assert_eq!(
            parse_chirps_dekad_filename("CHIRPS-v2.0.1981.12.3.TIFF"),
            Some((1981, 12, 3))
        );
        for bad in [
            "chirps-v2.0.2024.06.tif",   // monthly, not dekad
            "chirps-v2.0.2024.06.4.tif", // dekad out of range
            "chirps-v2.0.2024.06.1.2.tif",
            "chirps-v2.0.2024.13.1.tif",
        ] {
            assert_eq!(parse_chirps_dekad_filename(bad), None, "{bad}");
        }
    }

    #[test]
    fn dekad_draft_spans_its_dekad_and_has_distinct_identity() {
        let d1 = chirps_dekad_draft(Path::new("/data/chirps-v2.0.2024.02.1.tif"), 2024, 2, 1);
        assert_eq!(d1.algorithm_id, CHIRPS_DEKAD_ALGORITHM);
        assert_eq!(d1.scope.temporal_start, "2024-02-01T00:00:00Z");
        assert_eq!(d1.scope.temporal_end, "2024-02-10T23:59:59Z");
        let d3 = chirps_dekad_draft(Path::new("/data/chirps-v2.0.2024.02.3.tif"), 2024, 2, 3);
        // Leap February: third dekad runs to the 29th.
        assert_eq!(d3.scope.temporal_end, "2024-02-29T23:59:59Z");
        // Dekads are distinct from each other and from the month product.
        let monthly = chirps_monthly_draft(Path::new("/data/chirps-v2.0.2024.02.tif"), 2024, 2);
        assert_ne!(d1.product_id(), d3.product_id());
        assert_ne!(d1.product_id(), monthly.product_id());
    }

    #[test]
    fn maybe_gunzip_inflates_gzip_and_passes_plain_bytes() {
        use std::io::Write;
        let payload = b"agbot chirps fixture".to_vec();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&payload).unwrap();
        let gzipped = encoder.finish().unwrap();
        assert_eq!(maybe_gunzip(gzipped).unwrap(), payload);
        assert_eq!(maybe_gunzip(payload.clone()).unwrap(), payload);
        // Corrupt gzip is an error, not silence.
        assert!(maybe_gunzip(vec![0x1f, 0x8b, 0x00]).is_err());
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
