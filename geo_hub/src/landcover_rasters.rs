//! Catalog-driven phenology + tier-1 land-cover derivation (satellite
//! pipeline batch 10).
//!
//! `POST /api/landcover/derive` gathers a field's cataloged NDVI L2 GeoTIFF
//! series inside a date window (same grid, mismatches skipped with reason),
//! computes per-pixel phenology metrics (`post_processor::phenology`),
//! registers them as a `phenology` L3 (self-describing JSON artifact, lineage
//! to every observation), then classifies each pixel with the deterministic
//! tier-1 rules — optionally fed by the mean of same-window, same-grid MNDWI
//! products for the water rule — and registers the class raster as a
//! `landcover_rule` L3 GeoTIFF (codes 1..=6, invalid = nodata). The class
//! raster web-tiles through the catalog tiler's categorical land-cover
//! colormap and appears in `/api/stac` + `/browse`.

use std::path::{Path, PathBuf};

use post_processor::phenology::{
    classify_land_cover, compute_phenology, landcover_l3_draft, phenology_l3_draft,
    LandCoverRuleConfig, PhenologyError, PhenologyL3Scope, PhenologyObservation, PhenologyRequest,
    DEFAULT_MIN_OBSERVATIONS, DEFAULT_SEASON_THRESHOLD_FRACTION,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{ProductArtifact, ProductInputRef, ProductLevel};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, observed_on,
    DroughtRasterError, LoadedRaster, SkippedObservation,
};

/// Nodata for land-cover GeoTIFFs (invalid pixels).
pub const LANDCOVER_NODATA: f32 = crate::satellite_derivation::INDEX_NODATA;

#[derive(Debug, Error)]
pub enum LandCoverError {
    #[error("window start {start} is not before end {end} (ISO dates required)")]
    BadWindow { start: String, end: String },
    #[error("no cataloged {kind} products fall inside {start}..{end}")]
    NoSeries {
        kind: String,
        start: String,
        end: String,
    },
    #[error("no usable NDVI observations share one grid inside the window (all {skipped} candidates skipped)")]
    NoUsableSeries { skipped: usize },
    #[error("phenology computation failed: {0}")]
    Phenology(#[from] PhenologyError),
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

impl LandCoverError {
    pub fn is_client_error(&self) -> bool {
        match self {
            LandCoverError::BadWindow { .. }
            | LandCoverError::NoSeries { .. }
            | LandCoverError::NoUsableSeries { .. }
            | LandCoverError::Phenology(_) => true,
            LandCoverError::Shared(shared) => shared.is_client_error(),
            _ => false,
        }
    }
}

/// A land-cover derivation request over one season window.
#[derive(Debug, Clone, Deserialize)]
pub struct LandCoverDeriveRequest {
    pub field_id: String,
    pub season_id: String,
    /// Inclusive ISO date window selecting the NDVI/MNDWI series.
    pub start: String,
    pub end: String,
    #[serde(default = "default_min_observations")]
    pub min_observations: u32,
    #[serde(default = "default_threshold_fraction")]
    pub season_threshold_fraction: f32,
}

fn default_min_observations() -> u32 {
    DEFAULT_MIN_OBSERVATIONS
}

fn default_threshold_fraction() -> f32 {
    DEFAULT_SEASON_THRESHOLD_FRACTION
}

/// Outcome of one derivation.
#[derive(Debug, Clone, Serialize)]
pub struct LandCoverOutcome {
    pub phenology_product_id: String,
    pub landcover_product_id: String,
    pub ndvi_observations_used: Vec<String>,
    pub water_observations_used: Vec<String>,
    pub observations_skipped: Vec<SkippedObservation>,
    pub phenology_valid_fraction: f32,
    pub landcover_valid_fraction: f32,
    /// (class, pixel count) for every class present.
    pub class_counts: Vec<(String, u32)>,
    pub phenology_artifact: PathBuf,
    pub landcover_artifact: PathBuf,
    pub landcover_stac_item_href: String,
    pub landcover_tiles_href: String,
}

/// Same-grid check against the reference raster.
fn grid_matches(raster: &LoadedRaster, reference: &LoadedRaster) -> bool {
    raster.epsg == reference.epsg
        && raster.geo_transform == reference.geo_transform
        && (raster.width, raster.height) == (reference.width, reference.height)
}

/// Load the window's series of one kind, skipping grid mismatches. The first
/// loadable product (earliest temporal order is not guaranteed by
/// `list_products`, so the reference is the loaded raster of the earliest
/// *dated* candidate) anchors the grid when `reference` is `None`.
async fn load_series(
    pool: &DbPool,
    kind: &str,
    start: &str,
    end: &str,
    reference: Option<&LoadedRaster>,
    skipped: &mut Vec<SkippedObservation>,
) -> Result<Vec<(RegisteredProduct, LoadedRaster)>, LandCoverError> {
    let mut candidates = catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some(kind.to_string()),
            level: Some(ProductLevel::L2),
            status: Some("registered".to_string()),
            temporal_start: Some(start.to_string()),
            temporal_end: Some(end.to_string()),
            ..ProductFilter::default()
        },
    )
    .await?;
    // Deterministic date order; the earliest dated product anchors the grid.
    candidates.sort_by_key(|product| {
        (
            product.temporal_start.clone().unwrap_or_default(),
            product.product_id.clone(),
        )
    });

    let mut series: Vec<(RegisteredProduct, LoadedRaster)> = Vec::new();
    for candidate in candidates {
        if observed_on(&candidate).is_none() {
            skipped.push(SkippedObservation {
                product_id: candidate.product_id.clone(),
                reason: "bad_temporal".to_string(),
            });
            continue;
        }
        let Ok(path) = geotiff_artifact_path(&candidate) else {
            skipped.push(SkippedObservation {
                product_id: candidate.product_id.clone(),
                reason: "no_artifact".to_string(),
            });
            continue;
        };
        let raster = match load_raster(Path::new(path)) {
            Ok(raster) => raster,
            Err(_) => {
                skipped.push(SkippedObservation {
                    product_id: candidate.product_id.clone(),
                    reason: "unreadable".to_string(),
                });
                continue;
            }
        };
        let anchor = reference.or(series.first().map(|(_, raster)| raster));
        if let Some(anchor) = anchor {
            if !grid_matches(&raster, anchor) {
                skipped.push(SkippedObservation {
                    product_id: candidate.product_id.clone(),
                    reason: "grid_mismatch".to_string(),
                });
                continue;
            }
        }
        series.push((candidate, raster));
    }
    Ok(series)
}

/// Derive phenology + land cover for a field/season window. Idempotent on
/// identical inputs (content-addressed identities).
pub async fn derive_landcover(
    pool: &DbPool,
    data_root: &Path,
    request: &LandCoverDeriveRequest,
) -> Result<LandCoverOutcome, LandCoverError> {
    if request.start >= request.end {
        return Err(LandCoverError::BadWindow {
            start: request.start.clone(),
            end: request.end.clone(),
        });
    }
    let mut skipped = Vec::new();
    let ndvi_series = load_series(
        pool,
        "ndvi",
        &request.start,
        &request.end,
        None,
        &mut skipped,
    )
    .await?;
    if ndvi_series.is_empty() {
        return if skipped.is_empty() {
            Err(LandCoverError::NoSeries {
                kind: "ndvi".to_string(),
                start: request.start.clone(),
                end: request.end.clone(),
            })
        } else {
            Err(LandCoverError::NoUsableSeries {
                skipped: skipped.len(),
            })
        };
    }
    let reference = &ndvi_series[0].1;

    // Optional water-index series on the same grid: per-pixel mean of valid
    // MNDWI samples feeds the tier-1 water rule.
    let mndwi_series = load_series(
        pool,
        "mndwi",
        &request.start,
        &request.end,
        Some(reference),
        &mut skipped,
    )
    .await?;
    let pixel_count = reference.width as usize * reference.height as usize;
    let water_index_mean: Option<Vec<f32>> = if mndwi_series.is_empty() {
        None
    } else {
        let mut sums = vec![0.0f64; pixel_count];
        let mut counts = vec![0u32; pixel_count];
        for (_, raster) in &mndwi_series {
            for pixel in 0..pixel_count {
                if raster.valid_mask[pixel] && raster.values[pixel].is_finite() {
                    sums[pixel] += f64::from(raster.values[pixel]);
                    counts[pixel] += 1;
                }
            }
        }
        Some(
            (0..pixel_count)
                .map(|pixel| {
                    if counts[pixel] > 0 {
                        (sums[pixel] / f64::from(counts[pixel])) as f32
                    } else {
                        f32::NAN
                    }
                })
                .collect(),
        )
    };

    // --- Phenology.
    let observations: Vec<PhenologyObservation> = ndvi_series
        .iter()
        .map(|(product, raster)| PhenologyObservation {
            product_id: product.product_id.clone(),
            observed_on: observed_on(product).expect("filtered above"),
            values: raster.values.clone(),
            valid_mask: raster.valid_mask.clone(),
            spatial_ref: reference.spatial_ref.clone(),
        })
        .collect();
    let phenology = compute_phenology(&PhenologyRequest {
        width: reference.width,
        height: reference.height,
        spatial_ref: reference.spatial_ref.clone(),
        observations,
        min_observations: request.min_observations,
        season_threshold_fraction: request.season_threshold_fraction,
    })?;

    let scope = PhenologyL3Scope {
        field_id: request.field_id.clone(),
        season_id: request.season_id.clone(),
        scene_id: None,
        temporal_start: format!("{}T00:00:00Z", request.start),
        temporal_end: format!("{}T23:59:59Z", request.end),
        source_id: ndvi_series[0].0.source_id.clone(),
    };
    let actor = provenance::ActorIdentity::system("geo_hub:landcover_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let mut phenology_draft = phenology_l3_draft(&phenology, &scope);
    let derived_dir = data_root.join("derived").join("landcover");
    std::fs::create_dir_all(&derived_dir).map_err(|source| LandCoverError::Store {
        what: "landcover directory",
        source,
    })?;
    let phenology_path = derived_dir.join(format!(
        "{}.phenology.json",
        artifact_file_component(&phenology_draft.product_id())
    ));
    let phenology_json = serde_json::to_vec(&phenology).expect("phenology result serializes");
    std::fs::write(&phenology_path, &phenology_json).map_err(|source| LandCoverError::Store {
        what: "phenology artifact",
        source,
    })?;
    phenology_draft.spatial_ref = Some(reference.spatial_ref.clone());
    phenology_draft.artifact = Some(ProductArtifact {
        format: "json".to_string(),
        path: phenology_path.to_string_lossy().to_string(),
        checksum_sha256: Some(file_checksum(&phenology_path, "phenology readback")?),
    });
    let phenology_product_id =
        catalog::register_product_with_actor(pool, &phenology_draft, &actor, &created_at).await?;

    // --- Classification.
    let classification = classify_land_cover(
        &phenology,
        water_index_mean.as_deref(),
        &LandCoverRuleConfig::default(),
    )?;
    let mut landcover_inputs = vec![phenology_product_id.clone()];
    for (product, _) in &mndwi_series {
        landcover_inputs.push(product.product_id.clone());
    }
    let mut landcover_draft = landcover_l3_draft(&classification, landcover_inputs, &scope);
    let landcover_path = derived_dir.join(format!(
        "{}.landcover.tif",
        artifact_file_component(&landcover_draft.product_id())
    ));
    let class_values: Vec<f32> = classification
        .classes
        .iter()
        .map(|class| {
            class
                .class_code()
                .map(f32::from)
                .unwrap_or(LANDCOVER_NODATA)
        })
        .collect();
    write_geotiff_f32(
        &landcover_path,
        classification.width,
        classification.height,
        &class_values,
        &GeoTiffTags {
            epsg: reference.epsg,
            geo_transform: reference.geo_transform,
            nodata: Some(f64::from(LANDCOVER_NODATA)),
        },
    )?;
    let checksum = file_checksum(&landcover_path, "landcover readback")?;
    landcover_draft.spatial_ref = Some(reference.spatial_ref.clone());
    landcover_draft.gsd_m_per_px = ndvi_series[0].0.gsd_m_per_px;
    landcover_draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: landcover_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    landcover_draft.evidence_digests.push(checksum);
    // The NDVI series feeds the classification through the phenology
    // product; keep the direct edges too so a flat trace reaches L2 without
    // resolving the intermediate artifact.
    for (product, _) in &ndvi_series {
        landcover_draft.inputs.push(ProductInputRef {
            product_id: product.product_id.clone(),
            role: "ndvi_series".to_string(),
        });
    }
    let landcover_product_id =
        catalog::register_product_with_actor(pool, &landcover_draft, &actor, &created_at).await?;

    Ok(LandCoverOutcome {
        landcover_stac_item_href: format!(
            "/api/stac/collections/landcover_rule/items/{landcover_product_id}"
        ),
        landcover_tiles_href: format!(
            "/api/catalog/products/{landcover_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
        phenology_product_id,
        landcover_product_id,
        ndvi_observations_used: ndvi_series
            .iter()
            .map(|(product, _)| product.product_id.clone())
            .collect(),
        water_observations_used: mndwi_series
            .iter()
            .map(|(product, _)| product.product_id.clone())
            .collect(),
        observations_skipped: skipped,
        phenology_valid_fraction: phenology.valid_fraction,
        landcover_valid_fraction: classification.valid_fraction,
        class_counts: classification
            .class_counts
            .iter()
            .map(|(class, count)| {
                (
                    serde_json::to_value(class)
                        .expect("class serializes")
                        .as_str()
                        .expect("class is a string")
                        .to_string(),
                    *count,
                )
            })
            .collect(),
        phenology_artifact: phenology_path,
        landcover_artifact: landcover_path,
    })
}

/// List registered phenology + land-cover L3 products, optionally by field.
pub async fn list_landcover_products(
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
    let phenology = catalog::list_products(pool, &filter("phenology")).await?;
    let landcover = catalog::list_products(pool, &filter("landcover_rule")).await?;
    Ok((phenology, landcover))
}
