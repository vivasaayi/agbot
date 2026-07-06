//! Local satellite index derivation (satellite pipeline batch 6).
//!
//! End-to-end flow: Earth Search STAC item -> AOI pixel windows in the
//! scene's UTM grid -> ranged band reads via `raster_io::RemoteCogReader`
//! -> Sentinel-2 radiometric calibration + SCL cloud masking + spectral
//! index (`imagery_processor` pure functions) -> windowed GeoTIFF product on
//! the scene's data_root products directory -> L0/L1/L2 registration in the
//! product graph (visible via `/api/stac` and `/browse`).
//!
//! Scope: **Sentinel-2 L2A only.** Earth Search's Landsat C2 L2 asset hrefs
//! point at the requester-pays `s3://usgs-landsat` bucket (verified fixture),
//! so there is no free Landsat band-read path here; Landsat search stays in
//! `landsat.rs` and QA_PIXEL-masked derivation is future work.
//!
//! The module separates pure, unit-tested geometry/pixel math (`project_aoi`,
//! `snap_rect_outward`, `window_from_rect`, `resample_nearest`,
//! `compute_masked_index`) from the async I/O orchestration
//! (`derive_satellite_index`), which takes an injectable [`CogStoreResolver`]
//! so tests run against an in-memory object store.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use imagery_processor::pipeline::calibration::{apply_radiometric_scaling, SensorProfile};
use imagery_processor::pipeline::masks::{scl_kind_masks, SclMaskConfig};
use imagery_processor::{IndexBandRole, IndexBandValues, IndexKind, IndexPixelValue, MaskKind};
use raster_io::object_store::ObjectStore;
use raster_io::{
    write_geotiff_f32, GeoTiffTags, RasterBand, RasterWindow, RemoteCogReader, RemoteFetchMetrics,
};
use sha2::{Digest, Sha256};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::schemas::{GeoBounds, RasterSpatialRef};
use thiserror::Error;

use crate::db::DbPool;
use crate::earth_search::{s2_asset_key, EarthSearchItem, S2_SCL_ASSET_KEY};
use crate::ingest_contract::{commit_ingest, IngestScene, NormalizedIngest};
use crate::utm::{utm_to_wgs84, wgs84_to_utm, UtmError, UtmZone};

/// Nodata value written to derived index GeoTIFFs (matches
/// `imagery_processor`'s GeoTIFF export convention).
pub const INDEX_NODATA: f32 = -9999.0;

const SENTINEL2_COLLECTION: &str = "sentinel-2-l2a";
const ALGORITHM_VERSION: &str = "1.0.0";

#[derive(Debug, Error)]
pub enum DerivationError {
    #[error("collection {0:?} is not derivable locally: only sentinel-2-l2a has free COG assets (Earth Search Landsat bands are requester-pays s3://usgs-landsat)")]
    UnsupportedDataset(Option<String>),
    #[error("unknown index kind {0}; expected one of the imagery_processor index catalog (e.g. ndvi, mndwi, ndmi)")]
    UnknownIndexKind(String),
    #[error("item {item_id} has no {asset_key} asset")]
    MissingAsset { item_id: String, asset_key: String },
    #[error("item {0} has no proj:epsg/proj:code CRS")]
    MissingEpsg(String),
    #[error("item {0} has no properties.datetime")]
    MissingDatetime(String),
    #[error("invalid AOI bbox: {0}")]
    InvalidAoi(String),
    #[error("AOI does not intersect the scene grid of band {band}")]
    AoiOutsideScene { band: String },
    #[error("band {band} has no geotransform")]
    MissingGeotransform { band: String },
    #[error("band {band} CRS EPSG:{band_epsg:?} does not match item CRS EPSG:{item_epsg}")]
    CrsMismatch {
        band: String,
        band_epsg: Option<u32>,
        item_epsg: u32,
    },
    #[error("band {band} decoded as {dtype}; satellite DN bands must be u8/u16")]
    UnsupportedBandDtype { band: String, dtype: &'static str },
    #[error(transparent)]
    Utm(#[from] UtmError),
    #[error("raster I/O failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error("failed to resolve COG store for {href}: {message}")]
    Resolve { href: String, message: String },
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("catalog registration failed: {0}")]
    Ingest(#[from] crate::ingest_contract::IngestError),
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] crate::catalog::CatalogError),
    #[error("index computation failed: {0}")]
    Index(String),
}

impl DerivationError {
    /// True when the failure is a caller problem (bad request), not a server
    /// or upstream fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            DerivationError::UnsupportedDataset(_)
                | DerivationError::UnknownIndexKind(_)
                | DerivationError::MissingAsset { .. }
                | DerivationError::MissingEpsg(_)
                | DerivationError::MissingDatetime(_)
                | DerivationError::InvalidAoi(_)
                | DerivationError::AoiOutsideScene { .. }
                | DerivationError::Utm(_)
        )
    }
}

/// Seam for opening COG object stores from asset hrefs, so tests can serve
/// fixture COGs from `object_store::memory::InMemory`. The production
/// implementation is [`UrlCogResolver`].
pub trait CogStoreResolver: Send + Sync {
    fn resolve(&self, href: &str) -> Result<(Arc<dyn ObjectStore>, String), DerivationError>;
}

/// Resolve hrefs with `object_store::parse_url` (HTTPS COGs).
pub struct UrlCogResolver;

impl CogStoreResolver for UrlCogResolver {
    fn resolve(&self, href: &str) -> Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        let resolve_error = |message: String| DerivationError::Resolve {
            href: href.to_string(),
            message,
        };
        let url = url::Url::parse(href).map_err(|err| resolve_error(err.to_string()))?;
        let (store, path) = raster_io::object_store::parse_url(&url)
            .map_err(|err| resolve_error(err.to_string()))?;
        Ok((Arc::from(store), path.to_string()))
    }
}

/// Shareable resolver handle carried as an axum request extension so route
/// tests can inject an in-memory store without touching `AppState`.
#[derive(Clone)]
pub struct SatelliteCogResolver(pub Arc<dyn CogStoreResolver>);

// --- Pure geometry / pixel math ---------------------------------------------

/// Axis-aligned rectangle in projected (UTM meter) coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjRect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// Grid geometry of one band: GDAL geotransform + pixel dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandGrid {
    pub transform: [f64; 6],
    pub width: u32,
    pub height: u32,
}

impl BandGrid {
    pub fn pixel_size_x(&self) -> f64 {
        self.transform[1]
    }

    fn extent(&self) -> ProjRect {
        ProjRect {
            min_x: self.transform[0],
            max_x: self.transform[0] + f64::from(self.width) * self.transform[1],
            max_y: self.transform[3],
            min_y: self.transform[3] + f64::from(self.height) * self.transform[5],
        }
    }
}

/// Project a WGS84 AOI bbox into the scene's UTM zone by projecting the four
/// corners and taking the envelope. Adequate for field/scene-scale AOIs: TM
/// meridian convergence bows the edges by far less than a pixel at this
/// scale.
pub fn project_aoi(aoi: &GeoBounds, zone: UtmZone) -> Result<ProjRect, DerivationError> {
    if !(aoi.min_lon < aoi.max_lon && aoi.min_lat < aoi.max_lat) {
        return Err(DerivationError::InvalidAoi(format!(
            "min corner must be strictly below max corner, got [{}, {}, {}, {}]",
            aoi.min_lon, aoi.min_lat, aoi.max_lon, aoi.max_lat
        )));
    }
    let corners = [
        (aoi.min_lat, aoi.min_lon),
        (aoi.min_lat, aoi.max_lon),
        (aoi.max_lat, aoi.min_lon),
        (aoi.max_lat, aoi.max_lon),
    ];
    let mut projected = corners
        .iter()
        .map(|(lat, lon)| wgs84_to_utm(*lat, *lon, zone));
    let first = projected.next().expect("four corners")?;
    let mut rect = ProjRect {
        min_x: first.0,
        max_x: first.0,
        min_y: first.1,
        max_y: first.1,
    };
    for point in projected {
        let (x, y) = point?;
        rect.min_x = rect.min_x.min(x);
        rect.max_x = rect.max_x.max(x);
        rect.min_y = rect.min_y.min(y);
        rect.max_y = rect.max_y.max(y);
    }
    Ok(rect)
}

/// Tolerance (in pixels) absorbing float noise from the WGS84->UTM roundtrip
/// before flooring/ceiling to grid lines.
const SNAP_EPSILON_PX: f64 = 1e-6;

/// Snap a projected rect **outward** to the pixel grid of `grid` (the
/// coarsest band), then clamp to the grid extent. Guarantees every finer
/// band whose resolution divides the coarse one gets an exactly aligned
/// window. Errors when the AOI misses the scene.
pub fn snap_rect_outward(
    rect: &ProjRect,
    grid: &BandGrid,
    band: &str,
) -> Result<ProjRect, DerivationError> {
    let size_x = grid.transform[1];
    let size_y = -grid.transform[5];
    let origin_x = grid.transform[0];
    let origin_y = grid.transform[3];

    let snapped = ProjRect {
        min_x: origin_x + ((rect.min_x - origin_x) / size_x + SNAP_EPSILON_PX).floor() * size_x,
        max_x: origin_x + ((rect.max_x - origin_x) / size_x - SNAP_EPSILON_PX).ceil() * size_x,
        max_y: origin_y - ((origin_y - rect.max_y) / size_y + SNAP_EPSILON_PX).floor() * size_y,
        min_y: origin_y - ((origin_y - rect.min_y) / size_y - SNAP_EPSILON_PX).ceil() * size_y,
    };
    let extent = grid.extent();
    let clamped = ProjRect {
        min_x: snapped.min_x.max(extent.min_x),
        max_x: snapped.max_x.min(extent.max_x),
        min_y: snapped.min_y.max(extent.min_y),
        max_y: snapped.max_y.min(extent.max_y),
    };
    if clamped.min_x >= clamped.max_x || clamped.min_y >= clamped.max_y {
        return Err(DerivationError::AoiOutsideScene {
            band: band.to_string(),
        });
    }
    Ok(clamped)
}

/// Pixel window of a grid-aligned projected rect within one band. The rect
/// must already be snapped to a grid whose pixel size is an integer multiple
/// of this band's (values are rounded to the nearest pixel edge).
pub fn window_from_rect(grid: &BandGrid, rect: &ProjRect) -> Result<RasterWindow, DerivationError> {
    let size_x = grid.transform[1];
    let size_y = -grid.transform[5];
    let x = ((rect.min_x - grid.transform[0]) / size_x).round().max(0.0) as u32;
    let y = ((grid.transform[3] - rect.max_y) / size_y).round().max(0.0) as u32;
    let right = ((rect.max_x - grid.transform[0]) / size_x).round() as u32;
    let bottom = ((grid.transform[3] - rect.min_y) / size_y).round() as u32;
    let right = right.min(grid.width);
    let bottom = bottom.min(grid.height);
    if right <= x || bottom <= y {
        return Err(DerivationError::AoiOutsideScene {
            band: "window".to_string(),
        });
    }
    Ok(RasterWindow {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

/// Geotransform of a window cut from a band grid.
pub fn output_transform(grid: &BandGrid, window: RasterWindow) -> [f64; 6] {
    let mut transform = grid.transform;
    transform[0] += f64::from(window.x) * grid.transform[1];
    transform[3] += f64::from(window.y) * grid.transform[5];
    transform
}

/// Nearest-neighbor resample (row-major). With grid-aligned windows whose
/// resolutions are integer multiples this is exact block replication.
pub fn resample_nearest<T: Copy>(
    src: &[T],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
) -> Vec<T> {
    let mut out = Vec::with_capacity(dst_width as usize * dst_height as usize);
    for y in 0..dst_height {
        let src_y = (u64::from(y) * u64::from(src_height) / u64::from(dst_height)) as u32;
        for x in 0..dst_width {
            let src_x = (u64::from(x) * u64::from(src_width) / u64::from(dst_width)) as u32;
            out.push(src[(src_y * src_width + src_x) as usize]);
        }
    }
    out
}

/// Result of the pure per-pixel index computation.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexGrid {
    /// Row-major index values; invalid pixels hold [`INDEX_NODATA`].
    pub values: Vec<f32>,
    pub valid_pixels: usize,
    pub invalid_pixels: usize,
    /// Per-reason invalid counts (`masked`, `fill`, ...).
    pub reason_counts: BTreeMap<String, usize>,
}

/// Compute a spectral index over calibrated bands under a clear-sky mask.
/// Masked pixels and pixels with any invalid band sample become nodata with
/// a reason count; everything else delegates to
/// `IndexKind::compute_value`.
pub fn compute_masked_index(
    index: IndexKind,
    bands: &BTreeMap<IndexBandRole, Vec<IndexPixelValue>>,
    clear_mask: &[bool],
) -> Result<IndexGrid, DerivationError> {
    let pixel_count = clear_mask.len();
    for (role, pixels) in bands {
        if pixels.len() != pixel_count {
            return Err(DerivationError::Index(format!(
                "band {role:?} has {} pixels, mask has {pixel_count}",
                pixels.len()
            )));
        }
    }
    let mut values = Vec::with_capacity(pixel_count);
    let mut valid_pixels = 0usize;
    let mut reason_counts: BTreeMap<String, usize> = BTreeMap::new();
    let invalid = |reason: &str, counts: &mut BTreeMap<String, usize>| {
        *counts.entry(reason.to_string()).or_insert(0) += 1;
        INDEX_NODATA
    };

    for pixel in 0..pixel_count {
        if !clear_mask[pixel] {
            values.push(invalid("masked", &mut reason_counts));
            continue;
        }
        let mut band_values = IndexBandValues::default();
        let mut bad_reason: Option<&'static str> = None;
        for (role, pixels) in bands {
            match pixels[pixel] {
                IndexPixelValue::Valid(value) => band_values.insert(*role, value),
                IndexPixelValue::Invalid { reason } => {
                    bad_reason = Some(reason);
                    break;
                }
            }
        }
        if let Some(reason) = bad_reason {
            values.push(invalid(reason, &mut reason_counts));
            continue;
        }
        match index
            .compute_value(&band_values)
            .map_err(|err| DerivationError::Index(err.to_string()))?
        {
            IndexPixelValue::Valid(value) if value.is_finite() => {
                valid_pixels += 1;
                values.push(value);
            }
            IndexPixelValue::Valid(_) => {
                values.push(invalid("non_finite", &mut reason_counts));
            }
            IndexPixelValue::Invalid { reason } => {
                values.push(invalid(reason, &mut reason_counts));
            }
        }
    }

    Ok(IndexGrid {
        valid_pixels,
        invalid_pixels: pixel_count - valid_pixels,
        values,
        reason_counts,
    })
}

/// Canonical lowercase key for an index kind (clap value-enum name).
pub fn index_kind_key(index: IndexKind) -> String {
    use clap::ValueEnum;
    index
        .to_possible_value()
        .expect("index kinds have no skipped variants")
        .get_name()
        .to_string()
}

/// Parse an index kind from its lowercase key.
pub fn index_kind_from_key(key: &str) -> Result<IndexKind, DerivationError> {
    <IndexKind as clap::ValueEnum>::from_str(key.trim(), true)
        .map_err(|_| DerivationError::UnknownIndexKind(key.to_string()))
}

// --- Orchestration -----------------------------------------------------------

/// A satellite index derivation request.
#[derive(Debug, Clone)]
pub struct DeriveRequest {
    pub item: EarthSearchItem,
    /// WGS84 lon/lat AOI.
    pub aoi: GeoBounds,
    pub index: IndexKind,
    /// Field this derivation is scoped to; threaded into every registered
    /// product's [`ProductScope`] so the per-field time series can find it.
    pub field_id: Option<String>,
    /// Season the field observation belongs to.
    pub season_id: Option<String>,
}

/// Outcome of a completed derivation, with registration references.
#[derive(Debug, Clone)]
pub struct DerivationOutcome {
    pub product_id: String,
    pub scene_id: String,
    pub collection: String,
    pub index_kind: String,
    pub product_path: PathBuf,
    pub stac_item_href: String,
    pub width: u32,
    pub height: u32,
    pub valid_pixels: usize,
    pub invalid_pixels: usize,
    pub evidence: serde_json::Value,
}

struct BandRead {
    asset_key: String,
    href: String,
    grid: BandGrid,
    window: RasterWindow,
    dns: Vec<u16>,
    metrics: RemoteFetchMetrics,
}

fn sanitize_scene_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "satellite_scene".to_string()
    } else {
        trimmed.to_string()
    }
}

fn band_to_u16(band: RasterBand, name: &str) -> Result<Vec<u16>, DerivationError> {
    match band {
        RasterBand::U16(values) => Ok(values),
        RasterBand::U8(values) => Ok(values.into_iter().map(u16::from).collect()),
        // Landsat C2 / Sentinel-2 Int16 SR: negative values are the -9999
        // fill (nonphysical reflectance), clamped to 0 — the nodata sentinel
        // the SCL masking + calibration downstream already treats as fill.
        RasterBand::I16(values) => Ok(values.into_iter().map(|v| v.max(0) as u16).collect()),
        RasterBand::F32(_) => Err(DerivationError::UnsupportedBandDtype {
            band: name.to_string(),
            dtype: "f32",
        }),
    }
}

async fn read_band_window(
    resolver: &dyn CogStoreResolver,
    item: &EarthSearchItem,
    asset_key: &str,
    item_epsg: u32,
    aoi_rect: &ProjRect,
    snapped: Option<&ProjRect>,
) -> Result<(BandRead, ProjRect), DerivationError> {
    let asset = item
        .asset(asset_key)
        .ok_or_else(|| DerivationError::MissingAsset {
            item_id: item.id.clone(),
            asset_key: asset_key.to_string(),
        })?;
    let (store, location) = resolver.resolve(&asset.href)?;
    let reader = RemoteCogReader::open(store, &location).await?;
    let info = reader.info();
    if info.epsg != Some(item_epsg) {
        return Err(DerivationError::CrsMismatch {
            band: asset_key.to_string(),
            band_epsg: info.epsg,
            item_epsg,
        });
    }
    let grid = BandGrid {
        transform: info
            .geo_transform
            .ok_or_else(|| DerivationError::MissingGeotransform {
                band: asset_key.to_string(),
            })?,
        width: info.width,
        height: info.height,
    };
    // The first (coarsest) band snaps the rect; later bands reuse it.
    let snapped_rect = match snapped {
        Some(rect) => *rect,
        None => snap_rect_outward(aoi_rect, &grid, asset_key)?,
    };
    let window = window_from_rect(&grid, &snapped_rect)?;
    let dns = band_to_u16(reader.read_window(window).await?, asset_key)?;
    Ok((
        BandRead {
            asset_key: asset_key.to_string(),
            href: asset.href.clone(),
            grid,
            window,
            dns,
            metrics: reader.fetch_metrics(),
        },
        snapped_rect,
    ))
}

fn s2_sensor_profile(item: &EarthSearchItem) -> (SensorProfile, serde_json::Value) {
    match item.s2_processing_baseline() {
        Some(baseline) if baseline < 4.0 => (
            SensorProfile::Sentinel2L2ALegacy,
            serde_json::json!({ "profile": "sentinel2_l2a_legacy", "baseline": baseline }),
        ),
        Some(baseline) => (
            SensorProfile::Sentinel2L2ABaseline0400,
            serde_json::json!({ "profile": "sentinel2_l2a_baseline_0400", "baseline": baseline }),
        ),
        None => (
            SensorProfile::Sentinel2L2ABaseline0400,
            serde_json::json!({
                "profile": "sentinel2_l2a_baseline_0400",
                "baseline": null,
                "note": "s2:processing_baseline missing; assumed current (>= 04.00) offset",
            }),
        ),
    }
}

fn item_wgs84_spatial_ref(item: &EarthSearchItem) -> Option<RasterSpatialRef> {
    let bbox = item.bbox.as_ref().filter(|bbox| bbox.len() == 4)?;
    Some(RasterSpatialRef {
        georeferenced: true,
        crs: Some("EPSG:4326".to_string()),
        bbox: Some(GeoBounds {
            min_lon: bbox[0],
            min_lat: bbox[1],
            max_lon: bbox[2],
            max_lat: bbox[3],
        }),
        geo_transform: None,
        resolution: None,
    })
}

/// WGS84 envelope of a projected rect (corners inverse-projected).
fn rect_to_wgs84(rect: &ProjRect, zone: UtmZone) -> GeoBounds {
    let corners = [
        utm_to_wgs84(rect.min_x, rect.min_y, zone),
        utm_to_wgs84(rect.max_x, rect.min_y, zone),
        utm_to_wgs84(rect.min_x, rect.max_y, zone),
        utm_to_wgs84(rect.max_x, rect.max_y, zone),
    ];
    GeoBounds {
        min_lon: corners.iter().map(|(_, lon)| *lon).fold(f64::MAX, f64::min),
        max_lon: corners.iter().map(|(_, lon)| *lon).fold(f64::MIN, f64::max),
        min_lat: corners.iter().map(|(lat, _)| *lat).fold(f64::MAX, f64::min),
        max_lat: corners.iter().map(|(lat, _)| *lat).fold(f64::MIN, f64::max),
    }
}

fn window_json(window: RasterWindow) -> serde_json::Value {
    serde_json::json!({
        "x": window.x, "y": window.y,
        "width": window.width, "height": window.height,
    })
}

/// Run one satellite index derivation synchronously: fetch band windows,
/// calibrate, mask, compute, write the GeoTIFF, and register L0 + L1 + L2
/// products with lineage and evidence. Idempotent on identical inputs
/// (catalog identity is content-addressed).
pub async fn derive_satellite_index(
    pool: &DbPool,
    data_root: &Path,
    resolver: &dyn CogStoreResolver,
    request: &DeriveRequest,
) -> Result<DerivationOutcome, DerivationError> {
    let item = &request.item;
    let collection = item.collection.clone();
    if collection.as_deref() != Some(SENTINEL2_COLLECTION) {
        return Err(DerivationError::UnsupportedDataset(collection));
    }
    let collection = SENTINEL2_COLLECTION.to_string();
    let item_epsg = item
        .epsg()
        .ok_or_else(|| DerivationError::MissingEpsg(item.id.clone()))?;
    let acquired_at = item
        .datetime()
        .ok_or_else(|| DerivationError::MissingDatetime(item.id.clone()))?
        .to_string();
    let zone = UtmZone::from_epsg(item_epsg)?;
    let aoi_rect = project_aoi(&request.aoi, zone)?;

    // SCL (20 m) is the coarsest grid in play for the supported indices, so
    // it snaps the shared projected rect; 10 m band windows then align 2:1.
    let (scl_read, snapped) =
        read_band_window(resolver, item, S2_SCL_ASSET_KEY, item_epsg, &aoi_rect, None).await?;

    let roles = request.index.required_bands();
    let mut band_reads: Vec<(IndexBandRole, BandRead)> = Vec::with_capacity(roles.len());
    for role in roles {
        let (read, _) = read_band_window(
            resolver,
            item,
            s2_asset_key(*role),
            item_epsg,
            &aoi_rect,
            Some(&snapped),
        )
        .await?;
        band_reads.push((*role, read));
    }

    // Target grid: the finest band feeding the index.
    let (target_grid, target_window) = band_reads
        .iter()
        .map(|(_, read)| (read.grid, read.window))
        .min_by(|(left, _), (right, _)| {
            left.pixel_size_x()
                .partial_cmp(&right.pixel_size_x())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("every index requires at least one band");
    let (out_width, out_height) = (target_window.width, target_window.height);
    let pixel_count = out_width as usize * out_height as usize;

    // Clear-sky mask on the SCL native window grid (dilation at SCL
    // resolution, per the design masking rule), then nearest-resampled to
    // the target grid.
    let scl_config = SclMaskConfig::default();
    let scl_masks = scl_kind_masks(
        &scl_read.dns,
        scl_read.window.width,
        scl_read.window.height,
        &scl_config,
    );
    let clear_native = scl_masks
        .get(&MaskKind::Clear)
        .expect("scl_kind_masks always emits Clear");
    let clear_mask = resample_nearest(
        clear_native,
        scl_read.window.width,
        scl_read.window.height,
        out_width,
        out_height,
    );

    // Calibrate each band (resampling DNs to the target grid first).
    let (profile, calibration_json) = s2_sensor_profile(item);
    let mut calibrated: BTreeMap<IndexBandRole, Vec<IndexPixelValue>> = BTreeMap::new();
    let mut band_evidence = Vec::new();
    for (role, read) in &band_reads {
        let dns = if (read.window.width, read.window.height) == (out_width, out_height) {
            read.dns.clone()
        } else {
            resample_nearest(
                &read.dns,
                read.window.width,
                read.window.height,
                out_width,
                out_height,
            )
        };
        let scaled = apply_radiometric_scaling(profile, &dns);
        band_evidence.push(serde_json::json!({
            "role": format!("{role:?}").to_lowercase(),
            "asset_key": read.asset_key,
            "href": read.href,
            "window": window_json(read.window),
            "gsd_m_per_px": read.grid.pixel_size_x(),
            "fill_pixels": scaled.fill_pixel_count,
            "clamped_pixels": scaled.clamped_pixel_count,
            "fetch": {
                "range_requests": read.metrics.range_requests,
                "bytes_fetched": read.metrics.bytes_fetched,
            },
        }));
        calibrated.insert(*role, scaled.pixels);
    }

    let index_grid = compute_masked_index(request.index, &calibrated, &clear_mask)?;
    let index_key = index_kind_key(request.index);

    // Write the windowed GeoTIFF product into the scene products directory.
    let scene_component = sanitize_scene_component(&item.id);
    let scene_dir = data_root.join("scenes").join(&scene_component);
    let products_dir = scene_dir.join("products");
    std::fs::create_dir_all(&products_dir).map_err(|source| DerivationError::Store {
        what: "scene products directory",
        source,
    })?;
    let product_path = products_dir.join(format!("{index_key}.tif"));
    let out_transform = output_transform(&target_grid, target_window);
    write_geotiff_f32(
        &product_path,
        out_width,
        out_height,
        &index_grid.values,
        &GeoTiffTags {
            epsg: Some(item_epsg),
            geo_transform: Some(out_transform),
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let artifact_bytes = std::fs::read(&product_path).map_err(|source| DerivationError::Store {
        what: "derived product readback",
        source,
    })?;
    let checksum = format!("{:x}", Sha256::digest(&artifact_bytes));

    // Persist the STAC item JSON as the scene's L0 evidence artifact.
    let item_json_path = scene_dir.join("earth_search_item.json");
    let item_json = serde_json::to_vec_pretty(item).expect("item serializes");
    std::fs::write(&item_json_path, &item_json).map_err(|source| DerivationError::Store {
        what: "earth search item json",
        source,
    })?;

    // --- Registration: L0 scene + L1 remote bands via the ingest contract.
    let source_id = format!("earth-search:{collection}");
    let scene_spatial_ref = item_wgs84_spatial_ref(item);
    let l0 = ProductRecordDraft {
        level: ProductLevel::L0,
        kind: "raw_scene".to_string(),
        algorithm_id: "earth_search.stac.ingest".to_string(),
        algorithm_version: ALGORITHM_VERSION.to_string(),
        parameters: serde_json::json!({
            "item_id": item.id,
            "collection": collection,
            "provider": "Earth Search (Element84)",
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: Some(item.id.clone()),
            temporal_start: acquired_at.clone(),
            temporal_end: acquired_at.clone(),
        },
        spatial_ref: scene_spatial_ref.clone(),
        gsd_m_per_px: None,
        artifact: Some(ProductArtifact {
            format: "json".to_string(),
            path: item_json_path.to_string_lossy().to_string(),
            checksum_sha256: None,
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(source_id.clone()),
    };
    let l0_id = l0.product_id();

    let mut l1_products = Vec::new();
    let mut l2_inputs = vec![];
    let all_reads = band_reads
        .iter()
        .map(|(role, read)| (format!("band:{}", format!("{role:?}").to_lowercase()), read))
        .chain(std::iter::once(("mask:scl".to_string(), &scl_read)));
    for (input_role, read) in all_reads {
        let draft = ProductRecordDraft {
            level: ProductLevel::L1,
            kind: format!("band_{}", read.asset_key),
            algorithm_id: "earth_search.remote_band".to_string(),
            algorithm_version: ALGORITHM_VERSION.to_string(),
            parameters: serde_json::json!({
                "item_id": item.id,
                "band": read.asset_key,
                "href": read.href,
            }),
            inputs: vec![ProductInputRef {
                product_id: l0_id.clone(),
                role: "raw_scene".to_string(),
            }],
            scope: ProductScope {
                farm_id: None,
                field_id: request.field_id.clone(),
                season_id: request.season_id.clone(),
                scene_id: Some(item.id.clone()),
                temporal_start: acquired_at.clone(),
                temporal_end: acquired_at.clone(),
            },
            spatial_ref: scene_spatial_ref.clone(),
            gsd_m_per_px: Some(read.grid.pixel_size_x()),
            artifact: Some(ProductArtifact {
                format: "tif".to_string(),
                path: read.href.clone(),
                checksum_sha256: None,
            }),
            quality_mask: None,
            confidence: None,
            confidence_method: None,
            quality_summary: None,
            evidence_digests: Vec::new(),
            source_id: Some(source_id.clone()),
        };
        l2_inputs.push(ProductInputRef {
            product_id: draft.product_id(),
            role: input_role,
        });
        l1_products.push(draft);
    }

    let ingest = NormalizedIngest {
        source_id: source_id.clone(),
        source_kind: "satellite".to_string(),
        platform: Some(collection.clone()),
        sensor: Some("Sentinel-2 MSI".to_string()),
        source_config: None,
        scene: Some(IngestScene {
            scene_id: item.id.clone(),
            owner: None,
            sensor: collection.clone(),
            acquired_at: acquired_at.clone(),
            data_path: item_json_path.to_string_lossy().to_string(),
            metadata_json: String::from_utf8_lossy(&item_json).to_string(),
            cloud_cover: item.cloud_cover(),
        }),
        l0_products: vec![l0],
        l1_products,
        quality: item
            .cloud_cover()
            .map(|cover| serde_json::json!({ "cloud_cover": cover })),
    };
    let actor = provenance::ActorIdentity::system("geo_hub:satellite_derivation");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    commit_ingest(pool, &ingest, &actor, &created_at).await?;

    // --- L2 index product with full evidence.
    let out_bbox_wgs84 = rect_to_wgs84(&snapped, zone);
    let evidence = serde_json::json!({
        "item_id": item.id,
        "collection": collection,
        "index": index_key,
        "aoi_wgs84": [request.aoi.min_lon, request.aoi.min_lat, request.aoi.max_lon, request.aoi.max_lat],
        "crs": format!("EPSG:{item_epsg}"),
        "window": window_json(target_window),
        "geo_transform": out_transform,
        "calibration": calibration_json,
        "mask": {
            "scheme": "scl",
            "keep_classes": scl_config.keep_classes.iter().collect::<Vec<_>>(),
            "dilate_radius_px": scl_config.dilate_radius,
            "scl_window": window_json(scl_read.window),
        },
        "bands": band_evidence,
        "scl_fetch": {
            "range_requests": scl_read.metrics.range_requests,
            "bytes_fetched": scl_read.metrics.bytes_fetched,
        },
        "pixels": {
            "total": pixel_count,
            "valid": index_grid.valid_pixels,
            "invalid": index_grid.invalid_pixels,
            "reasons": index_grid.reason_counts,
        },
        "nodata": INDEX_NODATA,
    });
    let l2 = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: index_key.clone(),
        algorithm_id: format!("satellite.index.{index_key}"),
        algorithm_version: ALGORITHM_VERSION.to_string(),
        parameters: evidence.clone(),
        inputs: l2_inputs,
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: Some(item.id.clone()),
            temporal_start: acquired_at.clone(),
            temporal_end: acquired_at,
        },
        // The artifact GeoTIFF carries the exact UTM georeferencing; the
        // catalog spatial_ref is the WGS84 envelope of the derived window so
        // the STAC/browse layers can place the item on a map.
        spatial_ref: Some(RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(out_bbox_wgs84),
            geo_transform: None,
            resolution: None,
        }),
        gsd_m_per_px: Some(target_grid.pixel_size_x()),
        artifact: Some(ProductArtifact {
            format: "tif".to_string(),
            path: product_path.to_string_lossy().to_string(),
            checksum_sha256: Some(checksum.clone()),
        }),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: Some(serde_json::json!({
            "valid_pixels": index_grid.valid_pixels,
            "invalid_pixels": index_grid.invalid_pixels,
            "reasons": index_grid.reason_counts,
        })),
        evidence_digests: vec![checksum],
        source_id: Some(source_id),
    };
    let product_id =
        crate::catalog::register_product_with_actor(pool, &l2, &actor, &created_at).await?;

    Ok(DerivationOutcome {
        stac_item_href: format!("/api/stac/collections/{index_key}/items/{product_id}"),
        product_id,
        scene_id: item.id.clone(),
        collection,
        index_kind: index_key,
        product_path,
        width: out_width,
        height: out_height,
        valid_pixels: index_grid.valid_pixels,
        invalid_pixels: index_grid.invalid_pixels,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sentinel-2 tile 43PFN 10 m grid (matches the captured fixture item).
    fn grid_10m() -> BandGrid {
        BandGrid {
            transform: [600_000.0, 10.0, 0.0, 1_300_020.0, 0.0, -10.0],
            width: 10_980,
            height: 10_980,
        }
    }

    fn grid_20m() -> BandGrid {
        BandGrid {
            transform: [600_000.0, 20.0, 0.0, 1_300_020.0, 0.0, -20.0],
            width: 5_490,
            height: 5_490,
        }
    }

    #[test]
    fn snapped_rect_yields_aligned_windows_across_resolutions() {
        // A projected rect strictly inside pixel boundaries: snapping on the
        // 20 m grid must expand outward to 20 m lines, and both windows must
        // cover the same ground at exactly 2:1.
        let rect = ProjRect {
            min_x: 600_105.0,
            max_x: 600_195.0,
            min_y: 1_299_825.0,
            max_y: 1_299_915.0,
        };
        let snapped = snap_rect_outward(&rect, &grid_20m(), "scl").unwrap();
        assert_eq!(
            snapped,
            ProjRect {
                min_x: 600_100.0,
                max_x: 600_200.0,
                min_y: 1_299_820.0,
                max_y: 1_299_920.0,
            }
        );

        let window_20 = window_from_rect(&grid_20m(), &snapped).unwrap();
        assert_eq!(
            window_20,
            RasterWindow {
                x: 5,
                y: 5,
                width: 5,
                height: 5
            }
        );
        let window_10 = window_from_rect(&grid_10m(), &snapped).unwrap();
        assert_eq!(
            window_10,
            RasterWindow {
                x: 10,
                y: 10,
                width: 10,
                height: 10
            }
        );

        // Output transform anchors at the window origin.
        assert_eq!(
            output_transform(&grid_10m(), window_10),
            [600_100.0, 10.0, 0.0, 1_299_920.0, 0.0, -10.0]
        );
    }

    #[test]
    fn snap_is_stable_on_exact_grid_lines() {
        // Corners already on 20 m lines must not grow by another pixel.
        let rect = ProjRect {
            min_x: 600_100.0,
            max_x: 600_200.0,
            min_y: 1_299_820.0,
            max_y: 1_299_920.0,
        };
        let snapped = snap_rect_outward(&rect, &grid_20m(), "scl").unwrap();
        assert_eq!(snapped, rect);
    }

    #[test]
    fn aoi_fully_outside_scene_is_reason_coded() {
        let rect = ProjRect {
            min_x: 100_000.0,
            max_x: 100_100.0,
            min_y: 0.0,
            max_y: 100.0,
        };
        assert!(matches!(
            snap_rect_outward(&rect, &grid_20m(), "scl"),
            Err(DerivationError::AoiOutsideScene { .. })
        ));
    }

    #[test]
    fn aoi_overlapping_scene_edge_is_clamped() {
        // Extends west and north beyond the tile: clamps to the tile origin.
        let rect = ProjRect {
            min_x: 599_950.0,
            max_x: 600_045.0,
            min_y: 1_299_975.0,
            max_y: 1_300_100.0,
        };
        let snapped = snap_rect_outward(&rect, &grid_20m(), "scl").unwrap();
        assert_eq!(snapped.min_x, 600_000.0);
        assert_eq!(snapped.max_y, 1_300_020.0);
        let window = window_from_rect(&grid_20m(), &snapped).unwrap();
        assert_eq!((window.x, window.y), (0, 0));
        assert_eq!((window.width, window.height), (3, 3));
    }

    #[test]
    fn project_aoi_matches_hand_projected_utm_corners() {
        // Corners hand-verified through the UTM reference tests: an AOI in
        // zone 43N (75 E central meridian). Envelope must contain the
        // projected corners with min/max ordering.
        let zone = UtmZone {
            zone: 43,
            north: true,
        };
        let aoi = GeoBounds {
            min_lon: 76.90,
            min_lat: 11.74,
            max_lon: 76.92,
            max_lat: 11.76,
        };
        let rect = project_aoi(&aoi, zone).unwrap();
        let corners = [
            wgs84_to_utm(11.74, 76.90, zone).unwrap(),
            wgs84_to_utm(11.74, 76.92, zone).unwrap(),
            wgs84_to_utm(11.76, 76.90, zone).unwrap(),
            wgs84_to_utm(11.76, 76.92, zone).unwrap(),
        ];
        let min_x = corners.iter().map(|(x, _)| *x).fold(f64::MAX, f64::min);
        let max_x = corners.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
        let min_y = corners.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
        let max_y = corners.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        assert_eq!(
            (rect.min_x, rect.min_y, rect.max_x, rect.max_y),
            (min_x, min_y, max_x, max_y),
            "envelope is the min/max over all four projected corners"
        );
        // East of the 75 E central meridian the grid converges northward, so
        // the west edge at max_lat is the true min_x — the envelope must not
        // naively use the (min_lat, min_lon) corner.
        assert!(corners[2].0 < corners[0].0);
        assert!(rect.min_x < rect.max_x && rect.min_y < rect.max_y);

        // Degenerate AOI is rejected.
        let empty = GeoBounds {
            min_lon: 76.92,
            min_lat: 11.74,
            max_lon: 76.90,
            max_lat: 11.76,
        };
        assert!(matches!(
            project_aoi(&empty, zone),
            Err(DerivationError::InvalidAoi(_))
        ));
    }

    #[test]
    fn resample_nearest_is_exact_block_replication_for_2x() {
        // 2x2 -> 4x4: each source pixel becomes a 2x2 block.
        let src = [1u16, 2, 3, 4];
        let out = resample_nearest(&src, 2, 2, 4, 4);
        assert_eq!(out, vec![1, 1, 2, 2, 1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 4, 4]);
        // Identity when dimensions match.
        assert_eq!(resample_nearest(&src, 2, 2, 2, 2), src.to_vec());
    }

    #[test]
    fn compute_masked_index_applies_mask_fill_and_math() {
        // 2x2 grid: pixel 0 masked, pixel 1 fill in nir, pixels 2-3 valid.
        let red = vec![
            IndexPixelValue::Valid(0.1),
            IndexPixelValue::Valid(0.1),
            IndexPixelValue::Valid(0.1),
            IndexPixelValue::Valid(0.2),
        ];
        let nir = vec![
            IndexPixelValue::Valid(0.5),
            IndexPixelValue::Invalid { reason: "fill" },
            IndexPixelValue::Valid(0.5),
            IndexPixelValue::Valid(0.6),
        ];
        let bands = BTreeMap::from([(IndexBandRole::Red, red), (IndexBandRole::Nir, nir)]);
        let clear = vec![false, true, true, true];

        let grid = compute_masked_index(IndexKind::Ndvi, &bands, &clear).unwrap();

        assert_eq!(grid.values[0], INDEX_NODATA);
        assert_eq!(grid.values[1], INDEX_NODATA);
        // (0.5 - 0.1) / (0.5 + 0.1) = 0.6666667
        assert!((grid.values[2] - 0.666_666_7).abs() < 1e-6);
        // (0.6 - 0.2) / (0.6 + 0.2) = 0.5
        assert!((grid.values[3] - 0.5).abs() < 1e-6);
        assert_eq!(grid.valid_pixels, 2);
        assert_eq!(grid.invalid_pixels, 2);
        assert_eq!(grid.reason_counts.get("masked"), Some(&1));
        assert_eq!(grid.reason_counts.get("fill"), Some(&1));
    }

    #[test]
    fn index_kind_keys_roundtrip() {
        for (key, kind) in [
            ("ndvi", IndexKind::Ndvi),
            ("mndwi", IndexKind::Mndwi),
            ("ndmi", IndexKind::Ndmi),
        ] {
            assert_eq!(index_kind_key(kind), key);
            assert_eq!(index_kind_from_key(key).unwrap(), kind);
        }
        assert!(matches!(
            index_kind_from_key("not-an-index"),
            Err(DerivationError::UnknownIndexKind(_))
        ));
    }
}
