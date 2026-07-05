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

use post_processor::landcover_agreement::{
    compare_landcover, AgreementError, AgreementResult, ReferenceClassMap,
};
use post_processor::phenology::{
    classify_land_cover, compute_phenology, landcover_l3_draft, phenology_l3_draft, LandCoverClass,
    LandCoverRuleConfig, PhenologyError, PhenologyL3Scope, PhenologyObservation, PhenologyRequest,
    DEFAULT_MIN_OBSERVATIONS, DEFAULT_SEASON_THRESHOLD_FRACTION,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, observed_on,
    DroughtRasterError, LoadedRaster, SkippedObservation,
};

/// Nodata for land-cover GeoTIFFs (invalid pixels).
pub const LANDCOVER_NODATA: f32 = crate::satellite_derivation::INDEX_NODATA;
/// Catalog kind + source for registered reference maps.
pub const REFERENCE_KIND: &str = "landcover_reference";
pub const WORLDCOVER_SOURCE_ID: &str = "esa-worldcover";
/// WorldCover raster fill value.
pub const WORLDCOVER_NODATA: u8 = 0;

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
    #[error("product {product_id} kind {kind:?} is not {expected:?}")]
    WrongKind {
        product_id: String,
        kind: String,
        expected: &'static str,
    },
    #[error("classification and reference are not on the same grid (no resampling)")]
    ReferenceGridMismatch,
    #[error("agreement computation failed: {0}")]
    Agreement(#[from] AgreementError),
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
            | LandCoverError::Phenology(_)
            | LandCoverError::WrongKind { .. }
            | LandCoverError::ReferenceGridMismatch
            | LandCoverError::Agreement(_) => true,
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

// ---------------------------------------------------------------------------
// Reference maps (ESA WorldCover) + tier-2 agreement validation
// ---------------------------------------------------------------------------

/// (year, version, tile) parsed from an ESA WorldCover tile filename,
/// e.g. `ESA_WorldCover_10m_2021_v200_N09E075_Map.tif`.
pub fn parse_worldcover_filename(name: &str) -> Option<(i32, String, String)> {
    let stem = name
        .strip_suffix(".tif")
        .or_else(|| name.strip_suffix(".tiff"))?;
    let segments: Vec<&str> = stem.split('_').collect();
    // ESA WorldCover 10m <year> <version> <tile> Map
    if segments.len() != 7
        || segments[0] != "ESA"
        || segments[1] != "WorldCover"
        || segments[6] != "Map"
    {
        return None;
    }
    let year: i32 = segments[3].parse().ok()?;
    if !(2000..=2100).contains(&year) {
        return None;
    }
    Some((year, segments[4].to_string(), segments[5].to_string()))
}

/// Build the L2 draft for one WorldCover tile.
pub fn worldcover_draft(path: &Path, year: i32, version: &str, tile: &str) -> ProductRecordDraft {
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: REFERENCE_KIND.to_string(),
        algorithm_id: "worldcover.ingest".to_string(),
        algorithm_version: version.to_string(),
        parameters: serde_json::json!({
            "dataset": "ESA WorldCover 10m",
            "provider": "ESA",
            "year": year,
            "version": version,
            "tile": tile,
            "legend": "worldcover_v200_codes",
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(format!("worldcover-{tile}-{year}")),
            temporal_start: format!("{year}-01-01T00:00:00Z"),
            temporal_end: format!("{year}-12-31T23:59:59Z"),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(10.0),
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
        source_id: Some(WORLDCOVER_SOURCE_ID.to_string()),
    }
}

/// Outcome of a reference directory registration.
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceRegisterOutcome {
    pub registered: Vec<(String, String)>,
    pub skipped: Vec<String>,
}

/// Register every WorldCover tile GeoTIFF in a local directory (files are
/// fetched out-of-band; idempotent; non-matching names skipped).
pub async fn register_worldcover_dir(
    pool: &DbPool,
    dir: &Path,
) -> Result<ReferenceRegisterOutcome, LandCoverError> {
    let mut names: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(dir)
        .map_err(|source| LandCoverError::Store {
            what: "worldcover directory listing",
            source,
        })?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            Some((
                entry.file_name().to_string_lossy().to_string(),
                entry.path(),
            ))
        })
        .collect();
    names.sort();
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut outcome = ReferenceRegisterOutcome {
        registered: Vec::new(),
        skipped: Vec::new(),
    };
    for (name, path) in names {
        match parse_worldcover_filename(&name) {
            Some((year, version, tile)) => {
                let draft = worldcover_draft(&path, year, &version, &tile);
                let product_id = catalog::register_product_with_actor(
                    pool,
                    &draft,
                    &provenance::ActorIdentity::system("geo_hub:worldcover_ingest"),
                    &created_at,
                )
                .await?;
                outcome.registered.push((name, product_id));
            }
            None => outcome.skipped.push(name),
        }
    }
    Ok(outcome)
}

/// A tier-2 validation request.
#[derive(Debug, Clone, Deserialize)]
pub struct LandCoverValidateRequest {
    /// Catalog id of the tier-1 `landcover_rule` L3 product.
    pub landcover_product_id: String,
    /// Catalog id of the `landcover_reference` product on the same grid.
    pub reference_product_id: String,
}

/// Outcome of one validation.
#[derive(Debug, Clone, Serialize)]
pub struct LandCoverValidateOutcome {
    pub agreement_product_id: String,
    pub landcover_product_id: String,
    pub reference_product_id: String,
    pub compared_pixels: u32,
    pub overall_agreement: f64,
    pub kappa: f64,
    pub agreement_artifact: PathBuf,
    pub result: AgreementResult,
}

fn expect_kind(product: &RegisteredProduct, expected: &'static str) -> Result<(), LandCoverError> {
    if product.kind != expected {
        return Err(LandCoverError::WrongKind {
            product_id: product.product_id.clone(),
            kind: product.kind.clone(),
            expected,
        });
    }
    Ok(())
}

/// Decode a landcover_rule GeoTIFF band back into classes (codes 1..=6;
/// anything else, incl. nodata, is Invalid).
fn classes_from_codes(values: &[f32]) -> Vec<LandCoverClass> {
    values
        .iter()
        .map(|value| match value.round() as i64 {
            1 => LandCoverClass::Water,
            2 => LandCoverClass::BareOrSparse,
            3 => LandCoverClass::AnnualCrop,
            4 => LandCoverClass::TreeOrPerennial,
            5 => LandCoverClass::Grassland,
            6 => LandCoverClass::Unknown,
            _ => LandCoverClass::Invalid,
        })
        .collect()
}

/// Validate a tier-1 classification against a same-grid reference map and
/// register the agreement report as a `landcover_agreement` L3 (JSON
/// artifact, lineage to both inputs). Idempotent.
pub async fn validate_landcover(
    pool: &DbPool,
    data_root: &Path,
    request: &LandCoverValidateRequest,
) -> Result<LandCoverValidateOutcome, LandCoverError> {
    let landcover = catalog::get_product(pool, &request.landcover_product_id)
        .await?
        .ok_or_else(|| {
            LandCoverError::Shared(DroughtRasterError::CurrentNotFound(
                request.landcover_product_id.clone(),
            ))
        })?;
    expect_kind(&landcover, "landcover_rule")?;
    let reference = catalog::get_product(pool, &request.reference_product_id)
        .await?
        .ok_or_else(|| {
            LandCoverError::Shared(DroughtRasterError::CurrentNotFound(
                request.reference_product_id.clone(),
            ))
        })?;
    expect_kind(&reference, REFERENCE_KIND)?;

    let landcover_raster = load_raster(Path::new(geotiff_artifact_path(&landcover)?))?;
    let reference_raster = load_raster(Path::new(geotiff_artifact_path(&reference)?))?;
    if !grid_matches(&reference_raster, &landcover_raster) {
        return Err(LandCoverError::ReferenceGridMismatch);
    }

    let ours = classes_from_codes(&landcover_raster.values);
    let reference_codes: Vec<u8> = reference_raster
        .values
        .iter()
        .zip(&reference_raster.valid_mask)
        .map(|(value, valid)| {
            if *valid && value.is_finite() && (0.0..=255.0).contains(value) {
                value.round() as u8
            } else {
                WORLDCOVER_NODATA
            }
        })
        .collect();
    let class_map = ReferenceClassMap::default();
    let result = compare_landcover(&ours, &reference_codes, WORLDCOVER_NODATA, &class_map)?;

    // --- Agreement L3 (JSON artifact) with lineage to both inputs.
    let draft = ProductRecordDraft {
        level: ProductLevel::L3,
        kind: "landcover_agreement".to_string(),
        algorithm_id: "landcover.tier2_agreement".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "landcover_product_id": landcover.product_id,
            "reference_product_id": reference.product_id,
            "compared_pixels": result.compared_pixels,
            "overall_agreement": result.overall_agreement,
            "kappa": result.kappa,
            "excluded": result.excluded,
        }),
        inputs: vec![
            ProductInputRef {
                product_id: landcover.product_id.clone(),
                role: "classification".to_string(),
            },
            ProductInputRef {
                product_id: reference.product_id.clone(),
                role: "reference".to_string(),
            },
        ],
        scope: ProductScope {
            farm_id: None,
            field_id: landcover.field_id.clone(),
            season_id: landcover.season_id.clone(),
            scene_id: None,
            temporal_start: landcover.temporal_start.clone().unwrap_or_default(),
            temporal_end: landcover.temporal_end.clone().unwrap_or_default(),
        },
        spatial_ref: Some(landcover_raster.spatial_ref.clone()),
        gsd_m_per_px: landcover.gsd_m_per_px,
        artifact: None,
        quality_mask: None,
        confidence: Some(result.overall_agreement),
        confidence_method: Some("reference_overall_agreement".to_string()),
        quality_summary: None,
        evidence_digests: vec![result.input_hash.clone()],
        source_id: reference.source_id.clone(),
    };
    let agreement_dir = data_root.join("derived").join("landcover");
    std::fs::create_dir_all(&agreement_dir).map_err(|source| LandCoverError::Store {
        what: "landcover directory",
        source,
    })?;
    let agreement_path = agreement_dir.join(format!(
        "{}.agreement.json",
        artifact_file_component(&draft.product_id())
    ));
    std::fs::write(
        &agreement_path,
        serde_json::to_vec(&result).expect("agreement serializes"),
    )
    .map_err(|source| LandCoverError::Store {
        what: "agreement artifact",
        source,
    })?;
    let mut draft = draft;
    draft.artifact = Some(ProductArtifact {
        format: "json".to_string(),
        path: agreement_path.to_string_lossy().to_string(),
        checksum_sha256: Some(file_checksum(&agreement_path, "agreement readback")?),
    });
    let actor = provenance::ActorIdentity::system("geo_hub:landcover_validation");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let agreement_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(LandCoverValidateOutcome {
        agreement_product_id,
        landcover_product_id: landcover.product_id,
        reference_product_id: reference.product_id,
        compared_pixels: result.compared_pixels,
        overall_agreement: result.overall_agreement,
        kappa: result.kappa,
        agreement_artifact: agreement_path,
        result,
    })
}
