//! Local index/LST derivation from registered Landsat Collection-2 bands
//! (satellite pipeline batch 35; parity batch 38) — the Landsat parallel of
//! `sen2cor_derive`.
//!
//! The USGS ingest (Track A phase 5b) registers downloaded band GeoTIFFs as
//! L1 `band_*` products with free-form band tokens, so bands are located by
//! candidate kind lists. Band numbering is **instrument-specific** and the
//! same token means different things across sensors — on OLI (Landsat 8/9)
//! `SR_B4` is red, on TM/ETM+ (Landsat 4/5/7) it is NIR — so the candidate
//! tables are selected by the scene id's sensor prefix (`LC08`/`LC09` vs
//! `LT04`/`LT05`/`LE07`). This unlocks the pre-2013 archive: Collection-2
//! Level-2 uses one radiometric scale across all sensors (SR
//! `DN*0.0000275-0.2`, ST `DN*0.00341802+149` K), so TM/ETM+ scenes
//! calibrate through the exact same profiles.
//!
//! Derivable products (batch 38 extended the original ndvi/lst pair):
//! `ndvi`, `ndwi`, `mndwi`, `ndmi`, `nbr`, `evi`, `savi`, and `lst`. When
//! the scene's QA_PIXEL band is registered, cloud/shadow/cirrus/fill pixels
//! are masked first (`qa_applied`, QA product in the lineage). For `lst`,
//! the Collection-2 `ST_QA` band (per-pixel temperature uncertainty in
//! Kelvin, scale 0.01) when registered yields an honest per-product
//! confidence: the fraction of computed pixels whose uncertainty is at or
//! below [`ST_QA_LOW_UNCERTAINTY_K`].
//!
//! The registered `lst` L2s feed the drought TCI path; `mndwi`/`ndwi` feed
//! water extent (+ JRC prior); `nbr` feeds dNBR burn severity — now with
//! Landsat's 1982+ archive depth behind the baselines.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use imagery_processor::pipeline::calibration::{apply_radiometric_scaling, SensorProfile};
use imagery_processor::{IndexBandRole, IndexKind, IndexPixelValue};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::{Deserialize, Serialize};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use thiserror::Error;

use crate::catalog::{self, CatalogError, ProductFilter, RegisteredProduct};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, geotiff_artifact_path, load_raster, DroughtRasterError,
    LoadedRaster,
};
use crate::satellite_derivation::{compute_masked_index, INDEX_NODATA};

/// QA_PIXEL bits rejected for a clear pixel: 0 fill, 1 dilated cloud,
/// 2 cirrus, 3 cloud, 4 cloud shadow. Snow (5), clear (6), and water (7)
/// are valid ground.
pub const QA_PIXEL_REJECT_BITS: u16 = 0b0001_1111;

/// Collection-2 `ST_QA` scale: DN * 0.01 = surface-temperature uncertainty
/// in Kelvin.
pub const ST_QA_SCALE_K: f32 = 0.01;
/// LST pixels with ST_QA uncertainty at or below this (Kelvin) count as
/// low-uncertainty for the product confidence fraction.
pub const ST_QA_LOW_UNCERTAINTY_K: f32 = 2.0;

/// `true` when a Collection-2 QA_PIXEL code marks usable ground.
pub fn qa_pixel_clear(code: u16) -> bool {
    code & QA_PIXEL_REJECT_BITS == 0
}

/// The two Collection-2 band-numbering families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LandsatInstrument {
    /// Landsat 8/9 OLI/TIRS: SR_B2..SR_B7 optical, ST_B10 thermal.
    Oli,
    /// Landsat 4/5 TM and 7 ETM+: SR_B1..SR_B5 + SR_B7 optical, ST_B6.
    TmEtm,
}

/// Instrument from the Collection-2 scene id prefix (`LC08_...`,
/// `LT05_...`). Unknown prefixes default to OLI (the modern family).
pub fn instrument_for_scene(scene_id: &str) -> LandsatInstrument {
    match scene_id.get(..4) {
        Some("LT04") | Some("LT05") | Some("LE07") => LandsatInstrument::TmEtm,
        _ => LandsatInstrument::Oli,
    }
}

/// Band roles the derive resolves against the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BandRole {
    Blue,
    Green,
    Red,
    Nir,
    Swir1,
    Swir2,
    SurfaceTemperature,
    QaPixel,
    StQa,
}

impl BandRole {
    fn label(self) -> &'static str {
        match self {
            BandRole::Blue => "blue",
            BandRole::Green => "green",
            BandRole::Red => "red",
            BandRole::Nir => "nir",
            BandRole::Swir1 => "swir1",
            BandRole::Swir2 => "swir2",
            BandRole::SurfaceTemperature => "surface_temperature",
            BandRole::QaPixel => "qa_pixel",
            BandRole::StQa => "st_qa",
        }
    }
}

/// Candidate catalog kinds for a role on an instrument. Generic role-named
/// tokens (`band_red`, …) are instrument-safe and appear in both tables;
/// numbered tokens are instrument-specific — this is exactly the OLI/TM
/// divergence (SR_B4 = red on OLI, NIR on TM/ETM+).
fn candidates(instrument: LandsatInstrument, role: BandRole) -> &'static [&'static str] {
    use BandRole::*;
    use LandsatInstrument::*;
    match (instrument, role) {
        (Oli, Blue) => &["band_sr_b2", "band_b2", "band_blue"],
        (Oli, Green) => &["band_sr_b3", "band_b3", "band_green"],
        (Oli, Red) => &["band_sr_b4", "band_b4", "band_red"],
        (Oli, Nir) => &["band_sr_b5", "band_b5", "band_nir08", "band_nir"],
        (Oli, Swir1) => &["band_sr_b6", "band_b6", "band_swir16", "band_swir1"],
        (Oli, Swir2) => &["band_sr_b7", "band_b7", "band_swir22", "band_swir2"],
        (Oli, SurfaceTemperature) => &["band_st_b10", "band_b10", "band_lwir11"],
        (TmEtm, Blue) => &["band_sr_b1", "band_blue"],
        (TmEtm, Green) => &["band_sr_b2", "band_green"],
        (TmEtm, Red) => &["band_sr_b3", "band_red"],
        (TmEtm, Nir) => &["band_sr_b4", "band_nir"],
        (TmEtm, Swir1) => &["band_sr_b5", "band_swir1"],
        (TmEtm, Swir2) => &["band_sr_b7", "band_swir2"],
        (TmEtm, SurfaceTemperature) => &["band_st_b6", "band_lwir"],
        (_, QaPixel) => &["band_qa_pixel", "band_qa"],
        (_, StQa) => &["band_st_qa"],
    }
}

/// The optical index products the spec table derives: index kind + the
/// band roles it consumes (variable length — EVI needs three).
fn index_spec(product: &str) -> Option<(IndexKind, &'static [(IndexBandRole, BandRole)])> {
    match product {
        "ndvi" => Some((
            IndexKind::Ndvi,
            &[
                (IndexBandRole::Red, BandRole::Red),
                (IndexBandRole::Nir, BandRole::Nir),
            ],
        )),
        "ndwi" => Some((
            IndexKind::Ndwi,
            &[
                (IndexBandRole::Green, BandRole::Green),
                (IndexBandRole::Nir, BandRole::Nir),
            ],
        )),
        "mndwi" => Some((
            IndexKind::Mndwi,
            &[
                (IndexBandRole::Green, BandRole::Green),
                (IndexBandRole::Swir1, BandRole::Swir1),
            ],
        )),
        "ndmi" => Some((
            IndexKind::Ndmi,
            &[
                (IndexBandRole::Nir, BandRole::Nir),
                (IndexBandRole::Swir1, BandRole::Swir1),
            ],
        )),
        "nbr" => Some((
            IndexKind::Nbr,
            &[
                (IndexBandRole::Nir, BandRole::Nir),
                (IndexBandRole::Swir2, BandRole::Swir2),
            ],
        )),
        "evi" => Some((
            IndexKind::Evi,
            &[
                (IndexBandRole::Blue, BandRole::Blue),
                (IndexBandRole::Red, BandRole::Red),
                (IndexBandRole::Nir, BandRole::Nir),
            ],
        )),
        "savi" => Some((
            IndexKind::Savi,
            &[
                (IndexBandRole::Red, BandRole::Red),
                (IndexBandRole::Nir, BandRole::Nir),
            ],
        )),
        _ => None,
    }
}

#[derive(Debug, Error)]
pub enum LandsatDeriveError {
    #[error("product {0:?} is not derivable from Landsat C2 bands (supported: ndvi, ndwi, mndwi, ndmi, nbr, evi, savi, lst)")]
    UnsupportedProduct(String),
    #[error("scene {scene_id} has no registered band for {role} (looked for {kinds:?})")]
    BandNotFound {
        scene_id: String,
        role: &'static str,
        kinds: &'static [&'static str],
    },
    #[error("bands are not on the same grid ({reference:?} vs {other:?})")]
    GridMismatch {
        reference: (u32, u32),
        other: (u32, u32),
    },
    #[error("index computation failed: {0}")]
    Index(String),
    #[error("raster I/O failed: {0}")]
    Raster(#[from] raster_io::RasterIoError),
    #[error(transparent)]
    Shared(#[from] DroughtRasterError),
    #[error("catalog registration failed: {0}")]
    Catalog(#[from] CatalogError),
    #[error("failed to store {what}: {source}")]
    Store {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
}

impl LandsatDeriveError {
    /// True when the failure is a caller problem, not a server fault.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            LandsatDeriveError::UnsupportedProduct(_)
                | LandsatDeriveError::BandNotFound { .. }
                | LandsatDeriveError::GridMismatch { .. }
        )
    }
}

/// One Landsat local derivation request.
#[derive(Debug, Clone, Deserialize)]
pub struct LandsatDeriveRequest {
    /// Scene id of a registered USGS Landsat scene (the L1 bands' scene).
    pub scene_id: String,
    /// `ndvi` (default), `ndwi`, `mndwi`, `ndmi`, `nbr`, `evi`, `savi`, or
    /// `lst`.
    #[serde(default = "default_product")]
    pub product: String,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
}

fn default_product() -> String {
    "ndvi".to_string()
}

/// Outcome of one derivation, with registration references.
#[derive(Debug, Clone, Serialize)]
pub struct LandsatDeriveOutcome {
    pub product_id: String,
    /// The derived kind — also the catalog kind.
    pub product: String,
    pub scene_id: String,
    /// Which band-numbering family resolved the bands.
    pub instrument: LandsatInstrument,
    pub valid_pixels: usize,
    pub invalid_pixels: usize,
    /// Whether the scene's QA_PIXEL band masked clouds first.
    pub qa_applied: bool,
    /// (`lst` only) fraction of computed pixels with ST_QA uncertainty
    /// <= [`ST_QA_LOW_UNCERTAINTY_K`], when the scene has an ST_QA band.
    pub st_qa_low_uncertainty_fraction: Option<f32>,
    pub artifact: PathBuf,
    pub stac_item_href: String,
    pub tiles_href: String,
}

/// Find the scene's registered L1 band product for a role, trying each
/// candidate kind in order.
async fn find_band(
    pool: &DbPool,
    scene_id: &str,
    kinds: &'static [&'static str],
) -> Result<Option<RegisteredProduct>, LandsatDeriveError> {
    for kind in kinds {
        let mut products = catalog::list_products(
            pool,
            &ProductFilter {
                scene_id: Some(scene_id.to_string()),
                kind: Some((*kind).to_string()),
                level: Some(ProductLevel::L1),
                status: Some("registered".to_string()),
                ..ProductFilter::default()
            },
        )
        .await?;
        if let Some(product) = products.pop() {
            return Ok(Some(product));
        }
    }
    Ok(None)
}

async fn require_band(
    pool: &DbPool,
    scene_id: &str,
    instrument: LandsatInstrument,
    role: BandRole,
) -> Result<RegisteredProduct, LandsatDeriveError> {
    let kinds = candidates(instrument, role);
    find_band(pool, scene_id, kinds)
        .await?
        .ok_or(LandsatDeriveError::BandNotFound {
            scene_id: scene_id.to_string(),
            role: role.label(),
            kinds,
        })
}

fn load_band(product: &RegisteredProduct) -> Result<LoadedRaster, LandsatDeriveError> {
    Ok(load_raster(Path::new(geotiff_artifact_path(product)?))?)
}

/// Raster f32 values back to the u16 DN the C2 calibration expects
/// (masked/non-finite/fill map to DN 0, the profile's fill value).
fn band_to_dn(raster: &LoadedRaster) -> Vec<u16> {
    raster
        .values
        .iter()
        .zip(&raster.valid_mask)
        .map(|(value, valid)| {
            if *valid && value.is_finite() && (0.0..=65535.0).contains(value) {
                value.round() as u16
            } else {
                0
            }
        })
        .collect()
}

fn same_grid(a: &LoadedRaster, b: &LoadedRaster) -> bool {
    a.epsg == b.epsg
        && a.geo_transform == b.geo_transform
        && (a.width, a.height) == (b.width, b.height)
}

fn ensure_same_grid(
    reference: &LoadedRaster,
    other: &LoadedRaster,
) -> Result<(), LandsatDeriveError> {
    if !same_grid(reference, other) {
        return Err(LandsatDeriveError::GridMismatch {
            reference: (reference.width, reference.height),
            other: (other.width, other.height),
        });
    }
    Ok(())
}

/// Derive an optical index (surface reflectance) or LST (surface
/// temperature, Kelvin) locally from a registered Landsat C2 scene's band
/// products, QA-masked when the scene has a QA_PIXEL band. TM/ETM+ scenes
/// resolve through their own band numbering, extending baselines to the
/// 1982+ archive. Idempotent (content-addressed ids).
pub async fn derive_landsat_product(
    pool: &DbPool,
    data_root: &Path,
    request: &LandsatDeriveRequest,
) -> Result<LandsatDeriveOutcome, LandsatDeriveError> {
    let product_key = request.product.trim().to_ascii_lowercase();
    let optical_spec = index_spec(&product_key);
    if optical_spec.is_none() && product_key != "lst" {
        return Err(LandsatDeriveError::UnsupportedProduct(
            request.product.clone(),
        ));
    }
    let instrument = instrument_for_scene(&request.scene_id);

    // Resolve + load the role bands; the first defines the reference grid.
    let roles: Vec<(IndexBandRole, BandRole)> = match &optical_spec {
        Some((_, bands)) => bands.to_vec(),
        // LST consumes the thermal band; the index role slot is unused.
        None => vec![(IndexBandRole::Nir, BandRole::SurfaceTemperature)],
    };
    let mut band_products = Vec::new();
    let mut rasters: Vec<LoadedRaster> = Vec::new();
    for (index_role, band_role) in &roles {
        let product = require_band(pool, &request.scene_id, instrument, *band_role).await?;
        let raster = load_band(&product)?;
        if let Some(reference) = rasters.first() {
            ensure_same_grid(reference, &raster)?;
        }
        band_products.push((*index_role, *band_role, product));
        rasters.push(raster);
    }
    let reference = &rasters[0];
    let pixel_count = reference.width as usize * reference.height as usize;

    // Clear mask from the scene's QA_PIXEL band when registered.
    let qa_product = find_band(
        pool,
        &request.scene_id,
        candidates(instrument, BandRole::QaPixel),
    )
    .await?;
    let (clear, qa_applied, qa_input) = match &qa_product {
        Some(product) => {
            let qa = load_band(product)?;
            ensure_same_grid(reference, &qa)?;
            let codes = band_to_dn(&qa);
            (
                codes.iter().map(|code| qa_pixel_clear(*code)).collect(),
                true,
                Some(ProductInputRef {
                    product_id: product.product_id.clone(),
                    role: BandRole::QaPixel.label().to_string(),
                }),
            )
        }
        None => (vec![true; pixel_count], false, None),
    };

    // Calibrate + compute.
    let mut st_qa_input = None;
    let mut st_qa_fraction = None;
    let (values, valid_pixels, invalid_pixels, reasons, unit) = if let Some((kind, _)) =
        optical_spec
    {
        let mut bands = BTreeMap::new();
        for ((index_role, _, _), raster) in band_products.iter().zip(&rasters) {
            let scaled =
                apply_radiometric_scaling(SensorProfile::LandsatC2L2Sr, &band_to_dn(raster));
            bands.insert(*index_role, scaled.pixels);
        }
        let index = compute_masked_index(kind, &bands, &clear)
            .map_err(|err| LandsatDeriveError::Index(err.to_string()))?;
        (
            index.values,
            index.valid_pixels,
            index.invalid_pixels,
            serde_json::json!(index.reason_counts),
            "ratio",
        )
    } else {
        let scaled =
            apply_radiometric_scaling(SensorProfile::LandsatC2L2St, &band_to_dn(&rasters[0]));
        let mut values = vec![INDEX_NODATA; pixel_count];
        let mut valid = 0usize;
        for (pixel, (sample, keep)) in scaled.pixels.iter().zip(&clear).enumerate() {
            if let (IndexPixelValue::Valid(kelvin), true) = (sample, keep) {
                values[pixel] = *kelvin;
                valid += 1;
            }
        }
        // Per-pixel temperature uncertainty (ST_QA, Kelvin*100) -> honest
        // product confidence: the low-uncertainty fraction of computed
        // pixels.
        if let Some(product) = find_band(
            pool,
            &request.scene_id,
            candidates(instrument, BandRole::StQa),
        )
        .await?
        {
            let st_qa = load_band(&product)?;
            ensure_same_grid(reference, &st_qa)?;
            let uncertainties = band_to_dn(&st_qa);
            let mut low = 0usize;
            for (pixel, value) in values.iter().enumerate() {
                if *value != INDEX_NODATA
                    && f32::from(uncertainties[pixel]) * ST_QA_SCALE_K <= ST_QA_LOW_UNCERTAINTY_K
                {
                    low += 1;
                }
            }
            st_qa_fraction = Some(if valid == 0 {
                0.0
            } else {
                low as f32 / valid as f32
            });
            st_qa_input = Some(ProductInputRef {
                product_id: product.product_id.clone(),
                role: BandRole::StQa.label().to_string(),
            });
        }
        (
            values,
            valid,
            pixel_count - valid,
            serde_json::json!({ "fill_or_masked": pixel_count - valid }),
            "kelvin",
        )
    };

    let scene_product = &band_products[0].2;
    let mut draft = ProductRecordDraft {
        level: ProductLevel::L2,
        kind: product_key.clone(),
        algorithm_id: format!("landsat_c2.{product_key}"),
        algorithm_version: "1.1.0".to_string(),
        parameters: serde_json::json!({
            "scene_id": request.scene_id,
            "product": product_key,
            "instrument": instrument,
            "bands": band_products
                .iter()
                .map(|(_, _, product)| product.kind.clone())
                .collect::<Vec<_>>(),
            "sensor_profile": if product_key == "lst" {
                "landsat_c2l2_st"
            } else {
                "landsat_c2l2_sr"
            },
            "qa_applied": qa_applied,
            "st_qa_low_uncertainty_k": st_qa_fraction.map(|_| ST_QA_LOW_UNCERTAINTY_K),
            "unit": unit,
        }),
        inputs: band_products
            .iter()
            .map(|(_, band_role, product)| ProductInputRef {
                product_id: product.product_id.clone(),
                role: band_role.label().to_string(),
            })
            .chain(qa_input)
            .chain(st_qa_input)
            .collect(),
        scope: ProductScope {
            farm_id: None,
            field_id: request.field_id.clone(),
            season_id: request.season_id.clone(),
            scene_id: Some(request.scene_id.clone()),
            temporal_start: scene_product
                .temporal_start
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
            temporal_end: scene_product
                .temporal_end
                .clone()
                .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string()),
        },
        spatial_ref: Some(reference.spatial_ref.clone()),
        gsd_m_per_px: reference.geo_transform.map(|t| t[1].abs()),
        artifact: None,
        quality_mask: None,
        confidence: st_qa_fraction.map(f64::from),
        confidence_method: st_qa_fraction.map(|_| "st_qa_low_uncertainty_fraction".to_string()),
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: scene_product.source_id.clone(),
    };

    let derived_dir = data_root.join("derived").join("landsat_index");
    std::fs::create_dir_all(&derived_dir).map_err(|source| LandsatDeriveError::Store {
        what: "landsat index directory",
        source,
    })?;
    let out_path = derived_dir.join(format!(
        "{}.tif",
        artifact_file_component(&draft.product_id())
    ));
    write_geotiff_f32(
        &out_path,
        reference.width,
        reference.height,
        &values,
        &GeoTiffTags {
            epsg: reference.epsg,
            geo_transform: reference.geo_transform,
            nodata: Some(f64::from(INDEX_NODATA)),
        },
    )?;
    let checksum = file_checksum(&out_path, "landsat derive readback")?;
    draft.artifact = Some(ProductArtifact {
        format: "tif".to_string(),
        path: out_path.to_string_lossy().to_string(),
        checksum_sha256: Some(checksum.clone()),
    });
    draft.evidence_digests.push(checksum);
    draft.quality_summary = Some(serde_json::json!({
        "valid_pixels": valid_pixels,
        "invalid_pixels": invalid_pixels,
        "reasons": reasons,
        "qa_applied": qa_applied,
        "st_qa_low_uncertainty_fraction": st_qa_fraction,
    }));

    let actor = provenance::ActorIdentity::system("geo_hub:landsat_derive");
    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let product_id =
        catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;

    Ok(LandsatDeriveOutcome {
        stac_item_href: format!("/api/stac/collections/{product_key}/items/{product_id}"),
        tiles_href: format!("/api/catalog/products/{product_id}/tiles/{{z}}/{{x}}/{{y}}.png"),
        product_id,
        product: product_key,
        scene_id: request.scene_id.clone(),
        instrument,
        valid_pixels,
        invalid_pixels,
        qa_applied,
        st_qa_low_uncertainty_fraction: st_qa_fraction,
        artifact: out_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qa_pixel_bits_are_pinned() {
        // fill(1), dilated cloud(2), cirrus(4), cloud(8), shadow(16) reject;
        // snow(32), clear(64), water(128) keep.
        for rejected in [1u16, 2, 4, 8, 16, 8 | 64, 21824 | 8] {
            assert!(!qa_pixel_clear(rejected), "{rejected}");
        }
        for kept in [0u16, 32, 64, 128, 64 | 128, 32 | 64] {
            assert!(qa_pixel_clear(kept), "{kept}");
        }
    }

    #[test]
    fn instrument_resolution_is_scene_prefix_driven() {
        for (scene, instrument) in [
            ("LC08_L2SP_144051_20240601_02_T1", LandsatInstrument::Oli),
            ("LC09_L2SP_144051_20240601_02_T1", LandsatInstrument::Oli),
            ("LT05_L2SP_144051_19950601_02_T1", LandsatInstrument::TmEtm),
            ("LT04_L2SP_144051_19890601_02_T1", LandsatInstrument::TmEtm),
            ("LE07_L2SP_144051_20050601_02_T1", LandsatInstrument::TmEtm),
            ("unknown-scene", LandsatInstrument::Oli),
        ] {
            assert_eq!(instrument_for_scene(scene), instrument, "{scene}");
        }
    }

    #[test]
    fn band_numbering_diverges_between_instruments() {
        // The heart of the archive unlock: SR_B4 is red on OLI but NIR on
        // TM/ETM+, and the thermal band moves from ST_B10 to ST_B6.
        assert!(candidates(LandsatInstrument::Oli, BandRole::Red).contains(&"band_sr_b4"));
        assert!(candidates(LandsatInstrument::TmEtm, BandRole::Nir).contains(&"band_sr_b4"));
        assert!(!candidates(LandsatInstrument::TmEtm, BandRole::Red).contains(&"band_sr_b4"));
        assert!(
            candidates(LandsatInstrument::TmEtm, BandRole::SurfaceTemperature)
                .contains(&"band_st_b6")
        );
        assert!(
            candidates(LandsatInstrument::Oli, BandRole::SurfaceTemperature)
                .contains(&"band_st_b10")
        );
    }

    #[test]
    fn index_specs_cover_the_documented_products() {
        for product in ["ndvi", "ndwi", "mndwi", "ndmi", "nbr", "evi", "savi"] {
            assert!(index_spec(product).is_some(), "{product}");
        }
        assert!(index_spec("lst").is_none(), "lst is the thermal path");
        assert!(index_spec("evi9").is_none());
        // EVI is the three-band spec.
        assert_eq!(index_spec("evi").unwrap().1.len(), 3);
    }
}
