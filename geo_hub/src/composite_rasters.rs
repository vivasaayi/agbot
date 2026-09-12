//! Catalog-driven temporal compositing (satellite pipeline batch 32).
//!
//! Wires the pure `post_processor::temporal_composite` engine to the
//! product catalog: every registered same-kind, same-grid index L2 whose
//! date falls inside the requested window becomes a composite observation,
//! and the per-pixel median/medoid across them registers as a
//! `temporal_composite` L3 GeoTIFF with lineage to every input. Cloud gaps
//! (SCL/Fmask-masked nodata in the inputs) fill from the other
//! observations; pixels no observation covered stay nodata and are counted
//! in `gap_fraction` (confidence = 1 − gap_fraction).
//!
//! Scope notes:
//! - Observations must share the reference grid exactly (the engine refuses
//!   resampling by design); mismatches are skipped with a reason.
//! - `max_ndvi` compositing needs red+NIR band pairs; single-index
//!   composites support `median` (default) and `medoid` only.

use std::path::{Path, PathBuf};

use post_processor::temporal_composite::{
    compose_temporal, composite_l3_draft, CompositeL3Scope, CompositeMethod, CompositeObservation,
    CompositeRequest, CompositeResult, TemporalCompositeError,
};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{ProductArtifact, ProductLevel};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, DroughtRasterError,
    SkippedObservation,
};

fn resample_to_reference(
    source: &crate::drought_rasters::LoadedRaster,
    target: &crate::drought_rasters::LoadedRaster,
) -> Result<(Vec<f32>, Vec<bool>), String> {
    let source_transform = source
        .geo_transform
        .ok_or_else(|| "source has no geotransform".to_string())?;
    let target_transform = target
        .geo_transform
        .ok_or_else(|| "target has no geotransform".to_string())?;
    let source_epsg = source
        .epsg
        .ok_or_else(|| "source has no EPSG".to_string())?;
    let target_epsg = target
        .epsg
        .ok_or_else(|| "target has no EPSG".to_string())?;
    let source_zone = crate::utm::UtmZone::from_epsg(source_epsg).map_err(|e| e.to_string())?;
    let target_zone = crate::utm::UtmZone::from_epsg(target_epsg).map_err(|e| e.to_string())?;
    let determinant =
        source_transform[1] * source_transform[5] - source_transform[2] * source_transform[4];
    if determinant.abs() < 1e-12 {
        return Err("source geotransform is singular".to_string());
    }
    let mut values = vec![INDEX_NODATA; (target.width * target.height) as usize];
    let mut valid = vec![false; values.len()];
    let mut overlap = 0usize;
    for row in 0..target.height {
        for col in 0..target.width {
            let tx = target_transform[0]
                + (f64::from(col) + 0.5) * target_transform[1]
                + (f64::from(row) + 0.5) * target_transform[2];
            let ty = target_transform[3]
                + (f64::from(col) + 0.5) * target_transform[4]
                + (f64::from(row) + 0.5) * target_transform[5];
            let (sx, sy) = if source_epsg == target_epsg {
                (tx, ty)
            } else {
                let (lat, lon) = crate::utm::utm_to_wgs84(tx, ty, target_zone);
                crate::utm::wgs84_to_utm(lat, lon, source_zone).map_err(|e| e.to_string())?
            };
            let dx = sx - source_transform[0];
            let dy = sy - source_transform[3];
            let source_col = (source_transform[5] * dx - source_transform[2] * dy) / determinant;
            let source_row = (-source_transform[4] * dx + source_transform[1] * dy) / determinant;
            let source_col = source_col.floor() as i64;
            let source_row = source_row.floor() as i64;
            if source_col < 0
                || source_row < 0
                || source_col >= i64::from(source.width)
                || source_row >= i64::from(source.height)
            {
                continue;
            }
            overlap += 1;
            let source_index = (source_row as u32 * source.width + source_col as u32) as usize;
            let target_index = (row * target.width + col) as usize;
            if source.valid_mask[source_index] {
                values[target_index] = source.values[source_index];
                valid[target_index] = true;
            }
        }
    }
    if overlap == 0 {
        return Err("source and target grids do not overlap".to_string());
    }
    Ok((values, valid))
}
use crate::satellite_derivation::INDEX_NODATA;

/// Mask rule recorded in evidence: validity comes from each input's own
/// nodata (already SCL/Fmask-masked upstream where applicable).
pub const CATALOG_MASK_RULE: &str = "catalog_l2_nodata_mask";

#[derive(Debug, Error)]
pub enum CompositeRasterError {
    #[error("window start {0:?} is not an ISO date (YYYY-MM-DD)")]
    BadStart(String),
    #[error("window end {0:?} is not an ISO date (YYYY-MM-DD)")]
    BadEnd(String),
    #[error("method {0:?} is not composable here (single-index composites support median and medoid; max_ndvi needs red+nir band pairs)")]
    UnsupportedMethod(String),
    #[error("no usable {kind} observations in {start}..{end} (all {skipped} candidates skipped)")]
    NoUsableObservations {
        kind: String,
        start: String,
        end: String,
        skipped: usize,
    },
    #[error("composite computation failed: {0}")]
    Engine(#[from] TemporalCompositeError),
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

impl CompositeRasterError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            CompositeRasterError::BadStart(_)
                | CompositeRasterError::BadEnd(_)
                | CompositeRasterError::UnsupportedMethod(_)
                | CompositeRasterError::NoUsableObservations { .. }
                | CompositeRasterError::Engine(_)
        )
    }
}

/// One composite derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct CompositeDeriveRequest {
    /// Index kind to composite (e.g. `ndvi`, `mndwi`).
    pub kind: String,
    /// Inclusive ISO date window selecting the L2 series.
    pub start: String,
    pub end: String,
    /// `median` (default) or `medoid`.
    #[serde(default = "default_method")]
    pub method: String,
    pub field_id: String,
    pub season_id: String,
}

fn default_method() -> String {
    "median".to_string()
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct CompositeDeriveOutcome {
    pub composite_product_id: String,
    pub kind: String,
    pub method: String,
    /// Product ids composited, date order.
    pub observations_used: Vec<String>,
    pub observations_skipped: Vec<SkippedObservation>,
    pub gap_fraction: f32,
    pub period_start: String,
    pub period_end: String,
    pub composite_artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

fn parse_method(method: &str) -> Result<CompositeMethod, CompositeRasterError> {
    match method.trim().to_ascii_lowercase().as_str() {
        "median" => Ok(CompositeMethod::Median),
        "medoid" => Ok(CompositeMethod::Medoid),
        other => Err(CompositeRasterError::UnsupportedMethod(other.to_string())),
    }
}

/// Composite every cataloged same-kind, same-grid L2 in the window into a
/// `temporal_composite` L3 GeoTIFF with lineage to each input. Idempotent:
/// identical inputs re-register the same content-addressed id.
pub async fn derive_composite(
    pool: &DbPool,
    data_root: &Path,
    request: &CompositeDeriveRequest,
) -> Result<CompositeDeriveOutcome, CompositeRasterError> {
    let start = chrono::NaiveDate::parse_from_str(request.start.trim(), "%Y-%m-%d")
        .map_err(|_| CompositeRasterError::BadStart(request.start.clone()))?;
    let end = chrono::NaiveDate::parse_from_str(request.end.trim(), "%Y-%m-%d")
        .map_err(|_| CompositeRasterError::BadEnd(request.end.clone()))?;
    let method = parse_method(&request.method)?;

    // Candidates: registered same-kind L2s whose temporal range intersects
    // the window (the filter is inclusive on both ends).
    let candidates = catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some(request.kind.clone()),
            level: Some(ProductLevel::L2),
            status: Some("registered".to_string()),
            field_id: Some(request.field_id.clone()),
            season_id: Some(request.season_id.clone()),
            temporal_start: Some(format!("{start}T00:00:00Z")),
            temporal_end: Some(format!("{end}T23:59:59Z")),
            ..ProductFilter::default()
        },
    )
    .await?;

    let mut skipped = Vec::new();
    let skip = |product_id: &str, reason: &str, list: &mut Vec<SkippedObservation>| {
        list.push(SkippedObservation {
            product_id: product_id.to_string(),
            reason: reason.to_string(),
        });
    };

    // Date-sorted usable observations on the reference grid (the first
    // usable candidate defines it).
    let mut dated: Vec<(chrono::NaiveDate, &RegisteredProduct)> = Vec::new();
    for candidate in &candidates {
        match crate::drought_rasters::observed_on(candidate) {
            Some(date) => dated.push((date, candidate)),
            None => skip(&candidate.product_id, "bad_temporal", &mut skipped),
        }
    }
    dated.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.product_id.cmp(&b.1.product_id)));

    let mut reference: Option<crate::drought_rasters::LoadedRaster> = None;
    let mut observations = Vec::new();
    let mut used_ids = Vec::new();
    for (date, product) in dated {
        let Ok(path) = geotiff_artifact_path(product) else {
            skip(&product.product_id, "no_artifact", &mut skipped);
            continue;
        };
        let raster = match load_raster(Path::new(path)) {
            Ok(raster) => raster,
            Err(_) => {
                skip(&product.product_id, "unreadable", &mut skipped);
                continue;
            }
        };
        let (values, valid_mask, spatial_ref) = if let Some(reference) = &reference {
            if raster.epsg != reference.epsg
                || raster.geo_transform != reference.geo_transform
                || (raster.width, raster.height) != (reference.width, reference.height)
            {
                match resample_to_reference(&raster, reference) {
                    Ok((values, valid)) => (values, valid, reference.spatial_ref.clone()),
                    Err(_) => {
                        skip(&product.product_id, "grid_mismatch", &mut skipped);
                        continue;
                    }
                }
            } else {
                (
                    raster.values.clone(),
                    raster.valid_mask.clone(),
                    reference.spatial_ref.clone(),
                )
            }
        } else {
            (
                raster.values.clone(),
                raster.valid_mask.clone(),
                raster.spatial_ref.clone(),
            )
        };
        observations.push(CompositeObservation {
            product_id: product.product_id.clone(),
            scene_id: product.scene_id.clone().unwrap_or_default(),
            observed_on: date,
            bands: vec![values],
            valid_mask,
            spatial_ref,
        });
        used_ids.push(product.product_id.clone());
        if reference.is_none() {
            reference = Some(raster);
        }
    }
    let Some(reference) = reference else {
        return Err(CompositeRasterError::NoUsableObservations {
            kind: request.kind.clone(),
            start: request.start.clone(),
            end: request.end.clone(),
            skipped: skipped.len(),
        });
    };

    let result: CompositeResult = compose_temporal(&CompositeRequest {
        width: reference.width,
        height: reference.height,
        band_names: vec![request.kind.clone()],
        spatial_ref: reference.spatial_ref.clone(),
        observations,
        method,
        mask_rule: CATALOG_MASK_RULE.to_string(),
    })?;

    // --- Composite GeoTIFF + L3 registration.
    let mut draft = composite_l3_draft(
        &result,
        &CompositeL3Scope {
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: None,
            source_id: None,
        },
    );
    let composite_dir = data_root.join("derived").join("composite");
    std::fs::create_dir_all(&composite_dir).map_err(|source| CompositeRasterError::Store {
        what: "composite directory",
        source,
    })?;
    let composite_path = composite_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    let disk_values: Vec<f32> = result.bands[0]
        .iter()
        .map(|v| if v.is_finite() { *v } else { INDEX_NODATA })
        .collect();
    write_geotiff_f32(
        &composite_path,
        result.width,
        result.height,
        &disk_values,
        &GeoTiffTags {
            epsg: reference.epsg,
            geo_transform: reference.geo_transform,
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let checksum = file_checksum(&composite_path, "composite readback")?;
    draft.spatial_ref = Some(reference.spatial_ref.clone());
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: composite_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    draft.quality_summary = Some(serde_json::json!({
        "gap_fraction": result.gap_fraction,
        "selection_counts": result.selection_counts,
        "observation_count": used_ids.len(),
        "skipped": skipped,
    }));

    let actor = provenance::ActorIdentity::system("geo_hub:composite_rasters");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let composite_product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(CompositeDeriveOutcome {
        stac_item_href: format!(
            "/api/stac/collections/temporal_composite/items/{composite_product_id}"
        ),
        tiles_href: format!(
            "/api/catalog/products/{composite_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
        composite_product_id,
        kind: request.kind.clone(),
        method: result.method.label().to_string(),
        observations_used: used_ids,
        observations_skipped: skipped,
        gap_fraction: result.gap_fraction,
        period_start: result.period_start.to_string(),
        period_end: result.period_end.to_string(),
        composite_artifact: composite_path,
    })
}

/// List registered temporal-composite L3 products, optionally by field.
pub async fn list_composite_products(
    pool: &DbPool,
    field_id: Option<String>,
) -> Result<Vec<RegisteredProduct>, CatalogError> {
    catalog::list_products(
        pool,
        &ProductFilter {
            kind: Some("temporal_composite".to_string()),
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
    use shared::schemas::RasterSpatialRef;

    fn raster(values: Vec<f32>, transform: [f64; 6]) -> crate::drought_rasters::LoadedRaster {
        crate::drought_rasters::LoadedRaster {
            valid_mask: vec![true; values.len()],
            values,
            spatial_ref: RasterSpatialRef {
                georeferenced: true,
                crs: Some("EPSG:32643".to_string()),
                bbox: None,
                geo_transform: Some(transform),
                resolution: None,
            },
            epsg: Some(32643),
            geo_transform: Some(transform),
            width: 2,
            height: 2,
        }
    }

    #[test]
    fn shifted_observation_resamples_onto_reference_grid() {
        let source = raster(
            vec![1.0, 2.0, 3.0, 4.0],
            [10.0, 10.0, 0.0, 20.0, 0.0, -10.0],
        );
        let target = raster(vec![0.0; 4], [0.0, 10.0, 0.0, 20.0, 0.0, -10.0]);
        let (values, valid) = resample_to_reference(&source, &target).unwrap();
        assert_eq!(valid, vec![false, true, false, true]);
        assert_eq!(values[1], 1.0);
        assert_eq!(values[3], 3.0);
    }
}
