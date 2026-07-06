//! Catalog-driven drought raster derivation (satellite pipeline batch 8).
//!
//! Wires the `post_processor` climatology + drought-index engines to the
//! product catalog: the multi-year archive of cataloged L2 index GeoTIFFs on
//! one grid becomes a per-pixel min/max/mean climatology (registered as an
//! `index_climatology` L3 with lineage to every observation), and the
//! requested current-period product is scored against it into a VCI (NDVI)
//! or TCI (LST) raster GeoTIFF, registered as a `drought_index` L3. The
//! drought GeoTIFF web-tiles through the batch-7 catalog tiler and appears
//! in `/api/stac` + `/browse` like any other product.
//!
//! Scope notes:
//! - Observations must share the current product's exact grid (EPSG,
//!   geotransform, dimensions) — the climatology engine refuses resampling
//!   by design. Mismatches are skipped with a reason, not silently dropped.
//! - VHI needs a TCI on the same grid; the satellite derivation path is
//!   Sentinel-2 only (no thermal), so VHI is refused with a reason code
//!   until an LST product path exists.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use post_processor::drought_indices::{
    compute_tci, compute_vci, compute_vhi, drought_l3_draft, drought_result_from_raster,
    DroughtCurrentRaster, DroughtIndexError, DroughtIndexKind, DroughtIndexResult, DroughtL3Scope,
    RehydratedDroughtRaster, SeverityClassCounts, DEFAULT_VHI_ALPHA,
};
use post_processor::index_climatology::{
    build_index_climatology, calendar_period_for, climatology_l3_draft, write_climatology_json,
    ClimatologyError, ClimatologyL3Scope, ClimatologyObservation, ClimatologyPersistError,
    ClimatologyRequest,
};
use post_processor::temporal_composite::CompositeCadence;
use raster_io::{write_geotiff_f32, GeoTiffReader, GeoTiffTags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::product_graph::{ProductArtifact, ProductInputRef, ProductLevel};
use shared::schemas::RasterSpatialRef;
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;

/// Nodata written to drought GeoTIFFs (drought sentinel is NaN in memory;
/// on disk we use the workspace index-nodata convention).
pub const DROUGHT_NODATA: f32 = crate::satellite_derivation::INDEX_NODATA;

/// Default minimum distinct baseline years (design: >= 5, 10+ preferred).
pub const DEFAULT_MIN_YEARS: u32 = 5;

#[derive(Debug, Error)]
pub enum DroughtRasterError {
    #[error("current product {0} is not in the catalog")]
    CurrentNotFound(String),
    #[error("current product {product_id} has no GeoTIFF artifact ({detail})")]
    NotARasterProduct { product_id: String, detail: String },
    #[error("index kind {0:?} has no drought mapping: ndvi -> vci, lst -> tci (vhi needs an LST/TCI path first)")]
    UnsupportedIndexKind(String),
    #[error("series {0:?} is not a known input population (supported: l2, composites)")]
    UnsupportedSeries(String),
    #[error("current product {product_id} is not a single-band temporal_composite (band_names {band_names:?}); the composites series needs one")]
    NotAComposite {
        product_id: String,
        band_names: Option<serde_json::Value>,
    },
    #[error("current product {product_id} temporal_start {value:?} is not an ISO date")]
    BadTemporal {
        product_id: String,
        value: Option<String>,
    },
    #[error("no usable baseline observations share the current product's grid (all {skipped} candidates skipped)")]
    NoUsableObservations { skipped: usize },
    #[error("climatology build failed: {0}")]
    Climatology(#[from] ClimatologyError),
    #[error("climatology has no stats for period {0} (observations never covered it)")]
    MissingPeriod(String),
    #[error("drought index computation failed: {0}")]
    Drought(#[from] DroughtIndexError),
    #[error("raster I/O failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error("climatology artifact persistence failed: {0}")]
    Persist(#[from] ClimatologyPersistError),
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] CatalogError),
    #[error("product {product_id} is not a {expected} drought_index L3 (kind {kind:?}, index_kind {index_kind:?})")]
    NotADroughtComponent {
        product_id: String,
        expected: &'static str,
        kind: String,
        index_kind: Option<String>,
    },
}

impl DroughtRasterError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            DroughtRasterError::CurrentNotFound(_)
                | DroughtRasterError::NotARasterProduct { .. }
                | DroughtRasterError::UnsupportedIndexKind(_)
                | DroughtRasterError::UnsupportedSeries(_)
                | DroughtRasterError::NotAComposite { .. }
                | DroughtRasterError::BadTemporal { .. }
                | DroughtRasterError::NoUsableObservations { .. }
                | DroughtRasterError::MissingPeriod(_)
                | DroughtRasterError::NotADroughtComponent { .. }
                | DroughtRasterError::Drought(
                    DroughtIndexError::InvalidAlpha { .. }
                        | DroughtIndexError::ComponentGridMismatch { .. }
                        | DroughtIndexError::RehydratedValueOutOfRange { .. }
                )
        )
    }
}

/// A drought raster derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct DroughtRasterRequest {
    /// Catalog id of the current-period L2 index product to score.
    pub current_product_id: String,
    /// Field/season scope stamped on the L3 drafts (drought products are
    /// field-scoped decisions; the pixel data cannot supply this).
    pub field_id: String,
    pub season_id: String,
    #[serde(default = "default_cadence")]
    pub cadence: CompositeCadence,
    #[serde(default = "default_min_years")]
    pub min_years: u32,
    /// Input population (batch 34, mirroring the phenology semantics):
    /// `l2` (default) scores a raw index L2 against raw L2 baselines;
    /// `composites` scores a `temporal_composite` L3 of the index against
    /// composite baselines — never mixed (mixing double-counts scenes).
    #[serde(default = "default_series")]
    pub series: String,
}

fn default_series() -> String {
    "l2".to_string()
}

fn default_cadence() -> CompositeCadence {
    CompositeCadence::Monthly
}

fn default_min_years() -> u32 {
    DEFAULT_MIN_YEARS
}

/// One baseline observation that was considered but not used.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SkippedObservation {
    pub product_id: String,
    /// `no_artifact`, `unreadable`, `grid_mismatch`, `bad_temporal`.
    pub reason: String,
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct DroughtRasterOutcome {
    pub climatology_product_id: String,
    pub drought_product_id: String,
    /// `vci` or `tci`.
    pub drought_index_kind: String,
    /// Calendar period scored, e.g. `m06`.
    pub period: String,
    pub valid_fraction: f32,
    pub severity_counts: SeverityClassCounts,
    pub clamp_count: u32,
    /// Product ids that fed the climatology (current included), date order.
    pub observations_used: Vec<String>,
    pub observations_skipped: Vec<SkippedObservation>,
    pub climatology_artifact: PathBuf,
    pub drought_artifact: PathBuf,
    pub drought_stac_item_href: String,
    pub drought_tiles_href: String,
}

/// One loaded observation raster.
pub(crate) struct LoadedRaster {
    pub(crate) values: Vec<f32>,
    pub(crate) valid_mask: Vec<bool>,
    pub(crate) spatial_ref: RasterSpatialRef,
    pub(crate) epsg: Option<u32>,
    pub(crate) geo_transform: Option<[f64; 6]>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) fn load_raster(path: &Path) -> Result<LoadedRaster, raster_io::RasterIoError> {
    let mut reader = GeoTiffReader::open(path)?;
    let info = reader.info().clone();
    let spatial_ref = reader.spatial_ref()?;
    let values = reader.read_band()?.to_f32();
    let nodata = info.nodata.map(|n| n as f32);
    let valid_mask = values
        .iter()
        .map(|v| v.is_finite() && Some(*v) != nodata)
        .collect();
    Ok(LoadedRaster {
        values,
        valid_mask,
        spatial_ref,
        epsg: info.epsg,
        geo_transform: info.geo_transform,
        width: info.width,
        height: info.height,
    })
}

pub(crate) fn geotiff_artifact_path(
    product: &RegisteredProduct,
) -> Result<&str, DroughtRasterError> {
    let path = product
        .path
        .as_deref()
        .ok_or_else(|| DroughtRasterError::NotARasterProduct {
            product_id: product.product_id.clone(),
            detail: "no artifact path".to_string(),
        })?;
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".tif") || lower.ends_with(".tiff")) {
        return Err(DroughtRasterError::NotARasterProduct {
            product_id: product.product_id.clone(),
            detail: format!("artifact format {:?}", product.format),
        });
    }
    Ok(path)
}

/// ISO date from a product's temporal_start (`YYYY-MM-DD` prefix).
pub(crate) fn observed_on(product: &RegisteredProduct) -> Option<NaiveDate> {
    let start = product.temporal_start.as_deref()?;
    NaiveDate::parse_from_str(start.get(..10)?, "%Y-%m-%d").ok()
}

/// Filesystem-safe artifact file component for a product id (mirrors the
/// tile-cache convention: sanitized prefix + short digest, collision-free).
pub(crate) fn artifact_file_component(product_id: &str) -> String {
    let sanitized: String = product_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let digest = format!("{:x}", Sha256::digest(product_id.as_bytes()));
    format!("{}-{}", sanitized.trim_matches('_'), &digest[..12])
}

pub(crate) fn file_checksum(path: &Path, what: &'static str) -> Result<String, DroughtRasterError> {
    let bytes = std::fs::read(path).map_err(|source| DroughtRasterError::Store { what, source })?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// Map an L2 index kind to the drought index it scores.
fn drought_kind_for(index_kind: &str) -> Result<&'static str, DroughtRasterError> {
    match index_kind.trim().to_ascii_lowercase().as_str() {
        "ndvi" => Ok("vci"),
        "lst" | "thermal_lst" => Ok("tci"),
        other => Err(DroughtRasterError::UnsupportedIndexKind(other.to_string())),
    }
}

/// Derive a drought-index raster for `request.current_product_id`:
/// build/refresh the grid's index climatology from every cataloged
/// same-kind, same-grid L2 product, register it as an L3, score the current
/// product (VCI for NDVI, TCI for LST), write the drought GeoTIFF, and
/// register it as an L3 with lineage to the current product, every baseline
/// observation, and the climatology product. Idempotent: identical inputs
/// re-register the same content-addressed product ids.
pub async fn derive_drought_raster(
    pool: &DbPool,
    data_root: &Path,
    request: &DroughtRasterRequest,
) -> Result<DroughtRasterOutcome, DroughtRasterError> {
    let current = catalog::get_product(pool, &request.current_product_id)
        .await?
        .ok_or_else(|| DroughtRasterError::CurrentNotFound(request.current_product_id.clone()))?;
    // Resolve the index kind: raw L2s carry it as their catalog kind;
    // composites carry it in their identity-bearing band_names.
    let composites = match request.series.trim().to_ascii_lowercase().as_str() {
        "l2" => false,
        "composites" => true,
        other => return Err(DroughtRasterError::UnsupportedSeries(other.to_string())),
    };
    let index_kind = if composites {
        match current
            .parameters
            .get("band_names")
            .and_then(|b| b.as_array())
        {
            Some(bands) if bands.len() == 1 && bands[0].is_string() => {
                bands[0].as_str().expect("checked").to_string()
            }
            _ => {
                return Err(DroughtRasterError::NotAComposite {
                    product_id: current.product_id.clone(),
                    band_names: current.parameters.get("band_names").cloned(),
                })
            }
        }
    } else {
        current.kind.clone()
    };
    let drought_kind = drought_kind_for(&index_kind)?;
    let current_date = observed_on(&current).ok_or_else(|| DroughtRasterError::BadTemporal {
        product_id: current.product_id.clone(),
        value: current.temporal_start.clone(),
    })?;
    let current_raster = load_raster(Path::new(geotiff_artifact_path(&current)?))?;

    // Gather baseline candidates from the requested population: raw
    // same-kind L2s, or same-index temporal composites (never mixed).
    let (candidate_kind, candidate_level) = if composites {
        ("temporal_composite".to_string(), ProductLevel::L3)
    } else {
        (index_kind.clone(), ProductLevel::L2)
    };
    let mut candidates = catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some(candidate_kind),
            level: Some(candidate_level),
            status: Some("registered".to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    if composites {
        candidates.retain(|candidate| {
            candidate.parameters.get("band_names") == Some(&serde_json::json!([index_kind]))
        });
    }

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
        let raster = if is_current {
            // Already loaded and by definition on the reference grid.
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
        let raster = raster.as_ref().unwrap_or(&current_raster);
        observations.push(ClimatologyObservation {
            product_id: candidate.product_id.clone(),
            observed_on: date,
            values: raster.values.clone(),
            valid_mask: raster.valid_mask.clone(),
            // Grids are verified numerically identical above; using the
            // reference spatial_ref keeps the engine's exact-equality check
            // honest.
            spatial_ref: current_raster.spatial_ref.clone(),
        });
        used_ids.push(candidate.product_id.clone());
    }
    if observations.is_empty() {
        return Err(DroughtRasterError::NoUsableObservations {
            skipped: skipped.len(),
        });
    }

    // --- Climatology build + L3 registration.
    let climatology = build_index_climatology(&ClimatologyRequest {
        index_kind: index_kind.clone(),
        cadence: request.cadence,
        width: current_raster.width,
        height: current_raster.height,
        spatial_ref: current_raster.spatial_ref.clone(),
        min_years: request.min_years,
        observations,
    })?;

    let actor = provenance::ActorIdentity::system("geo_hub:drought_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    #[allow(clippy::items_after_statements)]
    fn stamp_series(draft: &mut shared::product_graph::ProductRecordDraft, series: &str) {
        if let Some(params) = draft.parameters.as_object_mut() {
            params.insert("series".to_string(), serde_json::json!(series));
        }
    }
    let mut climatology_draft = climatology_l3_draft(
        &climatology,
        &ClimatologyL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            source_id: current.source_id.clone(),
        },
    );
    stamp_series(&mut climatology_draft, &request.series);
    let climatology_dir = data_root.join("derived").join("climatology");
    std::fs::create_dir_all(&climatology_dir).map_err(|source| DroughtRasterError::Store {
        what: "climatology directory",
        source,
    })?;
    let climatology_path = climatology_dir.join(format!(
        "{}.json",
        artifact_file_component(&climatology_draft.product_id())
    ));
    write_climatology_json(&climatology, &climatology_path)?;
    climatology_draft.spatial_ref = Some(current_raster.spatial_ref.clone());
    climatology_draft.artifact = Some(ProductArtifact {
        format: "json".to_string(),
        path: climatology_path.to_string_lossy().to_string(),
        checksum_sha256: Some(file_checksum(&climatology_path, "climatology readback")?),
    });
    let climatology_product_id =
        catalog::register_product_with_actor(pool, &climatology_draft, &actor, &created_at).await?;

    // --- Score the current period.
    let period = calendar_period_for(current_date, request.cadence);
    let stats = climatology
        .period_stats(period)
        .ok_or_else(|| DroughtRasterError::MissingPeriod(period.label()))?;
    let current_input = DroughtCurrentRaster {
        product_id: current.product_id.clone(),
        width: current_raster.width,
        height: current_raster.height,
        spatial_ref: current_raster.spatial_ref.clone(),
        values: current_raster.values.clone(),
        valid_mask: current_raster.valid_mask.clone(),
    };
    let result: DroughtIndexResult = match drought_kind {
        "vci" => compute_vci(&current_input, &climatology, stats)?,
        _ => compute_tci(&current_input, &climatology, stats)?,
    };

    // --- Drought GeoTIFF + L3 registration.
    let mut drought_draft = drought_l3_draft(
        &result,
        &DroughtL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            temporal_start: current
                .temporal_start
                .clone()
                .unwrap_or_else(|| current_date.format("%Y-%m-%dT00:00:00Z").to_string()),
            temporal_end: current
                .temporal_end
                .clone()
                .unwrap_or_else(|| current_date.format("%Y-%m-%dT23:59:59Z").to_string()),
            source_id: current.source_id.clone(),
        },
    );
    stamp_series(&mut drought_draft, &request.series);
    let drought_dir = data_root.join("derived").join("drought");
    std::fs::create_dir_all(&drought_dir).map_err(|source| DroughtRasterError::Store {
        what: "drought directory",
        source,
    })?;
    let drought_path = drought_dir.join(format!(
        "{}.tif",
        artifact_file_component(&drought_draft.product_id())
    ));
    let disk_values: Vec<f32> = result
        .values
        .iter()
        .map(|v| if v.is_finite() { *v } else { DROUGHT_NODATA })
        .collect();
    write_geotiff_f32(
        &drought_path,
        result.width,
        result.height,
        &disk_values,
        &GeoTiffTags {
            epsg: current_raster.epsg,
            geo_transform: current_raster.geo_transform,
            nodata: Some(f64::from(DROUGHT_NODATA)),
        },
    )?;
    let drought_checksum = file_checksum(&drought_path, "drought raster readback")?;
    drought_draft.spatial_ref = Some(current_raster.spatial_ref.clone());
    drought_draft.gsd_m_per_px = current.gsd_m_per_px;
    drought_draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: drought_path.to_string_lossy().to_string(),
        checksum_sha256: Some(drought_checksum.clone()),
    });
    drought_draft.evidence_digests.push(drought_checksum);
    // Lineage to the registered climatology L3 on top of the engine-derived
    // L2 edges (identity is unaffected: inputs are not part of it).
    drought_draft.inputs.push(ProductInputRef {
        product_id: climatology_product_id.clone(),
        role: "climatology".to_string(),
    });
    let drought_product_id =
        catalog::register_product_with_actor(pool, &drought_draft, &actor, &created_at).await?;

    Ok(DroughtRasterOutcome {
        drought_stac_item_href: format!(
            "/api/stac/collections/drought_index/items/{drought_product_id}"
        ),
        drought_tiles_href: format!(
            "/api/catalog/products/{drought_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
        climatology_product_id,
        drought_product_id,
        drought_index_kind: drought_kind.to_string(),
        period: period.label(),
        valid_fraction: result.valid_fraction,
        severity_counts: result.severity_counts,
        clamp_count: result.clamp_count,
        observations_used: used_ids,
        observations_skipped: skipped,
        climatology_artifact: climatology_path,
        drought_artifact: drought_path,
    })
}

/// A VHI blend request over two registered drought-index L3 products.
#[derive(Debug, Clone, Deserialize)]
pub struct VhiDeriveRequest {
    /// Catalog id of a registered `drought_index` L3 with index_kind `vci`.
    pub vci_product_id: String,
    /// Catalog id of a registered `drought_index` L3 with index_kind `tci`,
    /// on the same grid.
    pub tci_product_id: String,
    /// Vegetation weight `α` in [0, 1] (Kogan default 0.5).
    #[serde(default = "default_vhi_alpha")]
    pub alpha: f64,
    pub field_id: String,
    pub season_id: String,
}

fn default_vhi_alpha() -> f64 {
    DEFAULT_VHI_ALPHA
}

/// Outcome of one VHI derivation.
#[derive(Debug, Clone, Serialize)]
pub struct VhiDeriveOutcome {
    pub vhi_product_id: String,
    pub alpha: f64,
    pub valid_fraction: f32,
    pub severity_counts: SeverityClassCounts,
    pub vhi_artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

/// Load a registered drought-index L3 component, verifying it really is the
/// expected VCI/TCI, and rehydrate it for the blend.
async fn load_drought_component(
    pool: &DbPool,
    product_id: &str,
    expected: &'static str,
    kind: DroughtIndexKind,
) -> Result<(RegisteredProduct, LoadedRaster, DroughtIndexResult), DroughtRasterError> {
    let product = catalog::get_product(pool, product_id)
        .await?
        .ok_or_else(|| DroughtRasterError::CurrentNotFound(product_id.to_string()))?;
    let index_kind = product
        .parameters
        .get("index_kind")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    if product.kind != "drought_index" || index_kind.as_deref() != Some(expected) {
        return Err(DroughtRasterError::NotADroughtComponent {
            product_id: product_id.to_string(),
            expected,
            kind: product.kind.clone(),
            index_kind,
        });
    }
    let raster = load_raster(Path::new(geotiff_artifact_path(&product)?))?;
    let result = drought_result_from_raster(&RehydratedDroughtRaster {
        kind,
        product_id: product_id.to_string(),
        width: raster.width,
        height: raster.height,
        spatial_ref: raster.spatial_ref.clone(),
        values: raster.values.clone(),
        valid_mask: raster.valid_mask.clone(),
    })?;
    Ok((product, raster, result))
}

/// Blend two registered same-grid VCI + TCI drought products into a VHI
/// (`α·VCI + (1−α)·TCI`) and register it as a `drought_index` L3 with
/// lineage to both components. Idempotent (content-addressed ids).
pub async fn derive_vhi_raster(
    pool: &DbPool,
    data_root: &Path,
    request: &VhiDeriveRequest,
) -> Result<VhiDeriveOutcome, DroughtRasterError> {
    let (vci_product, vci_raster, vci) =
        load_drought_component(pool, &request.vci_product_id, "vci", DroughtIndexKind::Vci).await?;
    let (tci_product, _, tci) =
        load_drought_component(pool, &request.tci_product_id, "tci", DroughtIndexKind::Tci).await?;

    let result = compute_vhi(&vci, &tci, request.alpha)?;

    let temporal_start = vci_product
        .temporal_start
        .clone()
        .or_else(|| tci_product.temporal_start.clone())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    let temporal_end = vci_product
        .temporal_end
        .clone()
        .or_else(|| tci_product.temporal_end.clone())
        .unwrap_or_else(|| temporal_start.clone());
    let mut draft = drought_l3_draft(
        &result,
        &DroughtL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            temporal_start,
            temporal_end,
            source_id: vci_product.source_id.clone().or(tci_product.source_id),
        },
    );

    let drought_dir = data_root.join("derived").join("drought");
    std::fs::create_dir_all(&drought_dir).map_err(|source| DroughtRasterError::Store {
        what: "drought directory",
        source,
    })?;
    let vhi_path = drought_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    let disk_values: Vec<f32> = result
        .values
        .iter()
        .map(|v| if v.is_finite() { *v } else { DROUGHT_NODATA })
        .collect();
    write_geotiff_f32(
        &vhi_path,
        result.width,
        result.height,
        &disk_values,
        &GeoTiffTags {
            epsg: vci_raster.epsg,
            geo_transform: vci_raster.geo_transform,
            nodata: Some(f64::from(DROUGHT_NODATA)),
        },
    )?;
    let checksum = file_checksum(&vhi_path, "vhi raster readback")?;
    draft.spatial_ref = Some(vci_raster.spatial_ref.clone());
    draft.gsd_m_per_px = vci_product.gsd_m_per_px;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: vhi_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);

    let actor = provenance::ActorIdentity::system("geo_hub:drought_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let vhi_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(VhiDeriveOutcome {
        stac_item_href: format!("/api/stac/collections/drought_index/items/{vhi_product_id}"),
        tiles_href: format!("/api/catalog/products/{vhi_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"),
        vhi_product_id,
        alpha: request.alpha,
        valid_fraction: result.valid_fraction,
        severity_counts: result.severity_counts,
        vhi_artifact: vhi_path,
    })
}

/// A standalone index-climatology derivation request (batch S-12). Unlike
/// [`derive_drought_raster`], which builds a climatology as a byproduct of
/// scoring one current product, this registers *only* the climatology so the
/// pipeline can materialize it as its own L3 before any drought scoring runs.
#[derive(Debug, Clone)]
pub struct ClimatologyDeriveRequest {
    /// Index kind to build a climatology for (e.g. `ndvi`).
    pub index_kind: String,
    pub field_id: String,
    pub season_id: String,
    pub cadence: CompositeCadence,
    /// Minimum distinct baseline years per pixel/period.
    pub min_years: u32,
}

/// Outcome of a standalone climatology derivation.
#[derive(Debug, Clone, Serialize)]
pub struct ClimatologyDeriveOutcome {
    pub climatology_product_id: String,
    /// Composite product ids that fed the climatology, date order.
    pub observations_used: Vec<String>,
    pub observations_skipped: Vec<SkippedObservation>,
    pub climatology_artifact: PathBuf,
}

/// Build and register an index climatology from a field's monthly
/// `temporal_composite` L3s of `index_kind` (batch S-12). The composites are
/// the pipeline's per-month rollups; two same-calendar-month composites in
/// different years give the calendar period a 2-year baseline. All composites
/// must share one grid (the first usable one anchors it; mismatches are
/// skipped with a reason). Idempotent: identical inputs re-register the same
/// content-addressed id.
///
/// Returns [`DroughtRasterError::NoUsableObservations`] when the field has no
/// usable composite; the caller (pipeline worker) treats that as a no-op.
pub async fn derive_index_climatology(
    pool: &DbPool,
    data_root: &Path,
    request: &ClimatologyDeriveRequest,
) -> Result<ClimatologyDeriveOutcome, DroughtRasterError> {
    // Field-scoped single-band composites of this index (band_names is
    // identity-bearing on the composite draft).
    let mut candidates = catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some("temporal_composite".to_string()),
            level: Some(ProductLevel::L3),
            status: Some("registered".to_string()),
            field_id: Some(request.field_id.clone()),
            ..ProductFilter::default()
        },
    )
    .await?;
    candidates.retain(|candidate| {
        candidate.parameters.get("band_names") == Some(&serde_json::json!([request.index_kind]))
    });
    candidates.sort_by(|a, b| {
        a.temporal_start
            .cmp(&b.temporal_start)
            .then(a.product_id.cmp(&b.product_id))
    });

    let mut skipped = Vec::new();
    let skip = |product_id: &str, reason: &str, list: &mut Vec<SkippedObservation>| {
        list.push(SkippedObservation {
            product_id: product_id.to_string(),
            reason: reason.to_string(),
        });
    };

    let mut reference: Option<LoadedRaster> = None;
    let mut observations = Vec::new();
    let mut used_ids = Vec::new();
    for candidate in &candidates {
        let Some(date) = observed_on(candidate) else {
            skip(&candidate.product_id, "bad_temporal", &mut skipped);
            continue;
        };
        let Ok(path) = geotiff_artifact_path(candidate) else {
            skip(&candidate.product_id, "no_artifact", &mut skipped);
            continue;
        };
        let raster = match load_raster(Path::new(path)) {
            Ok(raster) => raster,
            Err(_) => {
                skip(&candidate.product_id, "unreadable", &mut skipped);
                continue;
            }
        };
        if let Some(reference) = &reference {
            if raster.epsg != reference.epsg
                || raster.geo_transform != reference.geo_transform
                || (raster.width, raster.height) != (reference.width, reference.height)
            {
                skip(&candidate.product_id, "grid_mismatch", &mut skipped);
                continue;
            }
        }
        let spatial_ref = reference
            .as_ref()
            .map(|reference| reference.spatial_ref.clone())
            .unwrap_or_else(|| raster.spatial_ref.clone());
        observations.push(ClimatologyObservation {
            product_id: candidate.product_id.clone(),
            observed_on: date,
            values: raster.values.clone(),
            valid_mask: raster.valid_mask.clone(),
            spatial_ref,
        });
        used_ids.push(candidate.product_id.clone());
        if reference.is_none() {
            reference = Some(raster);
        }
    }
    let Some(reference) = reference else {
        return Err(DroughtRasterError::NoUsableObservations {
            skipped: skipped.len(),
        });
    };

    let climatology = build_index_climatology(&ClimatologyRequest {
        index_kind: request.index_kind.clone(),
        cadence: request.cadence,
        width: reference.width,
        height: reference.height,
        spatial_ref: reference.spatial_ref.clone(),
        min_years: request.min_years,
        observations,
    })?;

    let actor = provenance::ActorIdentity::system("geo_hub:drought_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut climatology_draft = climatology_l3_draft(
        &climatology,
        &ClimatologyL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            source_id: reference_source_id(&candidates, &request.index_kind),
        },
    );
    if let Some(params) = climatology_draft.parameters.as_object_mut() {
        params.insert("series".to_string(), serde_json::json!("composites"));
    }
    let climatology_dir = data_root.join("derived").join("climatology");
    std::fs::create_dir_all(&climatology_dir).map_err(|source| DroughtRasterError::Store {
        what: "climatology directory",
        source,
    })?;
    let climatology_path = climatology_dir.join(format!(
        "{}.json",
        artifact_file_component(&climatology_draft.product_id())
    ));
    write_climatology_json(&climatology, &climatology_path)?;
    climatology_draft.spatial_ref = Some(reference.spatial_ref.clone());
    climatology_draft.artifact = Some(ProductArtifact {
        format: "json".to_string(),
        path: climatology_path.to_string_lossy().to_string(),
        checksum_sha256: Some(file_checksum(&climatology_path, "climatology readback")?),
    });
    let climatology_product_id =
        catalog::register_product_with_actor(pool, &climatology_draft, &actor, &created_at).await?;

    Ok(ClimatologyDeriveOutcome {
        climatology_product_id,
        observations_used: used_ids,
        observations_skipped: skipped,
        climatology_artifact: climatology_path,
    })
}

/// The source id to stamp on a climatology draft: the first candidate's, so
/// the trace keeps a provider reference when the composites carry one.
fn reference_source_id(candidates: &[RegisteredProduct], _index_kind: &str) -> Option<String> {
    candidates.iter().find_map(|c| c.source_id.clone())
}

/// List registered drought-raster products (climatologies + drought indices),
/// optionally scoped to a field.
pub async fn list_drought_raster_products(
    pool: &DbPool,
    field_id: Option<String>,
) -> Result<(Vec<RegisteredProduct>, Vec<RegisteredProduct>), CatalogError> {
    let filter = |kind: &str| ProductFilter {
        kind: Some(kind.to_string()),
        level: Some(ProductLevel::L3),
        status: Some("registered".to_string()),
        field_id: field_id.clone(),
        ..ProductFilter::default()
    };
    let climatologies = catalog::list_products(pool, &filter("index_climatology")).await?;
    let droughts = catalog::list_products(pool, &filter("drought_index")).await?;
    Ok((climatologies, droughts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drought_kind_mapping_is_pinned() {
        assert_eq!(drought_kind_for("ndvi").unwrap(), "vci");
        assert_eq!(drought_kind_for(" NDVI ").unwrap(), "vci");
        assert_eq!(drought_kind_for("lst").unwrap(), "tci");
        assert!(matches!(
            drought_kind_for("mndwi"),
            Err(DroughtRasterError::UnsupportedIndexKind(_))
        ));
    }

    #[test]
    fn artifact_file_component_is_safe_and_collision_resistant() {
        let a = artifact_file_component("scene:ndvi:abc123");
        let b = artifact_file_component("scene/ndvi/abc123");
        assert!(a
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'));
        // Same sanitized prefix, different digests.
        assert_ne!(a, b);
    }

    #[test]
    fn observed_on_parses_iso_prefix_only() {
        let mut product = sample_product();
        product.temporal_start = Some("2026-06-14T10:30:00Z".to_string());
        assert_eq!(observed_on(&product), NaiveDate::from_ymd_opt(2026, 6, 14));
        product.temporal_start = Some("June 2026".to_string());
        assert_eq!(observed_on(&product), None);
        product.temporal_start = None;
        assert_eq!(observed_on(&product), None);
    }

    fn sample_product() -> RegisteredProduct {
        RegisteredProduct {
            product_id: "p".to_string(),
            level: ProductLevel::L2,
            kind: "ndvi".to_string(),
            algorithm_id: "a".to_string(),
            algorithm_version: "1".to_string(),
            parameters: serde_json::json!({}),
            parameters_hash: "h".to_string(),
            path: None,
            format: None,
            checksum_sha256: None,
            crs: None,
            bbox: None,
            gsd_m_per_px: None,
            temporal_start: None,
            temporal_end: None,
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: None,
            source_id: None,
            quality_mask_product_id: None,
            confidence: None,
            confidence_method: None,
            status: "active".to_string(),
            superseded_by: None,
            provenance_id: None,
            created_at: "2026-07-05T00:00:00Z".to_string(),
        }
    }
}
