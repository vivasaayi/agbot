//! HLS (Harmonized Landsat Sentinel-2) v2.0 ingestion (satellite pipeline
//! batch 19).
//!
//! HLS is NASA's cross-sensor harmonization: HLSL30 (Landsat 8/9) and HLSS30
//! (Sentinel-2) surface reflectance are BRDF/atmosphere/geometry corrected
//! onto one common 30 m tile grid with a shared processing chain, giving a
//! ~2-3 day revisit — the densest ready-made analysis-ready time series for
//! phenology (batch 10) and climatology (batch 8). Because both instruments
//! land on the same grid, their NDVI products merge into one field series
//! that the existing phenology/land-cover derivation gathers automatically.
//!
//! This registers pre-downloaded HLS band GeoTIFFs (CMR-STAC / Earthdata
//! download is out-of-band, credentialed) as harmonized `ndvi` L2 products:
//! for each granule with both required bands, NDVI is computed through the
//! canonical `imagery_processor` index math (reason-coded per pixel) and
//! written as a GeoTIFF on the granule's grid.
//!
//! Band roles are instrument-specific — this is exactly the harmonization
//! the module encapsulates: HLSS30 red=B04 / NIR=B08, HLSL30 red=B04 /
//! NIR=B05. Real HLS is **Int16 scaled DN** (× 1e-4, signed -9999 fill);
//! `load_hls_reflectance` reads it via `raster_io`'s Int16 support (batch
//! 21) and scales integer bands to reflectance, while f32 bands (already
//! reflectance) pass through — so pre-downloaded HLS GeoTIFFs feed the local
//! pipeline directly. When a granule includes its Fmask QA band (batch 22),
//! cloud/cirrus/adjacent/shadow pixels are masked before the index; without
//! Fmask, masking falls back to band fill values only (`fmask_applied` in
//! the product records which happened).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use imagery_processor::{IndexBandRole, IndexKind, IndexPixelValue};
use raster_io::{write_geotiff_f32, GeoTiffTags};
use serde::Serialize;
use shared::product_graph::{ProductArtifact, ProductLevel, ProductRecordDraft, ProductScope};
use thiserror::Error;

use crate::catalog::{self, CatalogError};
use crate::db::DbPool;
use crate::drought_rasters::{
    artifact_file_component, file_checksum, load_raster, DroughtRasterError, LoadedRaster,
};
use crate::satellite_derivation::{compute_masked_index, INDEX_NODATA};

/// Source id stamped on HLS registrations.
pub const HLS_SOURCE_ID: &str = "hls-v2.0";
/// HLS v2.0 surface-reflectance scale factor: Int16 DN * 1e-4 = reflectance.
pub const HLS_REFLECTANCE_SCALE: f32 = 1.0e-4;

#[derive(Debug, Error)]
pub enum HlsError {
    #[error("granule {granule} red and NIR bands are not on the same grid")]
    GridMismatch { granule: String },
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

/// A parsed HLS v2.0 band filename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlsBandFile {
    /// `S30` (Sentinel-2) or `L30` (Landsat).
    pub instrument: String,
    /// MGRS tile without the leading `T`, e.g. `43PFN`.
    pub tile: String,
    /// Acquisition date (from the `YYYYDDD` day-of-year stamp).
    pub acquired_on: chrono::NaiveDate,
    /// Raw acquisition stamp `YYYYDDDThhmmss` (granule identity).
    pub stamp: String,
    /// Band token, e.g. `B04`, `B08`, `Fmask`.
    pub band: String,
}

impl HlsBandFile {
    /// Stable granule id shared by every band of one acquisition.
    pub fn granule_id(&self) -> String {
        format!("HLS.{}.T{}.{}", self.instrument, self.tile, self.stamp)
    }
}

/// Parse an HLS v2.0 band filename, e.g.
/// `HLS.S30.T43PFN.2024152T051651.v2.0.B04.tif`.
pub fn parse_hls_filename(name: &str) -> Option<HlsBandFile> {
    let stem = name
        .strip_suffix(".tif")
        .or_else(|| name.strip_suffix(".tiff"))?;
    // HLS . {inst} . T{tile} . {stamp} . v{maj} . {min} . {band}
    let segments: Vec<&str> = stem.split('.').collect();
    if segments.len() != 7 || segments[0] != "HLS" || !segments[4].starts_with('v') {
        return None;
    }
    let instrument = segments[1];
    if instrument != "S30" && instrument != "L30" {
        return None;
    }
    let tile = segments[2].strip_prefix('T')?;
    let stamp = segments[3];
    // stamp = YYYYDDDThhmmss
    if stamp.len() < 8 || stamp.as_bytes()[7] != b'T' {
        return None;
    }
    let year: i32 = stamp.get(0..4)?.parse().ok()?;
    let doy: u32 = stamp.get(4..7)?.parse().ok()?;
    let acquired_on = chrono::NaiveDate::from_yo_opt(year, doy)?;
    Some(HlsBandFile {
        instrument: instrument.to_string(),
        tile: tile.to_string(),
        acquired_on,
        stamp: stamp.to_string(),
        band: segments[6].to_string(),
    })
}

/// The (red, NIR) band tokens for an HLS instrument. This is the
/// cross-sensor harmonization: the SAME NDVI is computed from
/// instrument-specific NIR bands (Sentinel B08 vs Landsat B05).
pub fn ndvi_bands(instrument: &str) -> Option<(&'static str, &'static str)> {
    match instrument {
        "S30" => Some(("B04", "B08")),
        "L30" => Some(("B04", "B05")),
        _ => None,
    }
}

/// Outcome of an HLS directory registration.
#[derive(Debug, Clone, Serialize)]
pub struct HlsRegisterOutcome {
    /// (granule id, ndvi product id) for each granule with a full band set.
    pub registered: Vec<(String, String)>,
    /// (granule id, reason) for granules missing a required band.
    pub incomplete: Vec<(String, String)>,
    /// Filenames that did not parse as HLS bands.
    pub skipped: Vec<String>,
}

/// One granule's collected band paths.
#[derive(Default)]
struct GranuleBands {
    instrument: String,
    tile: String,
    acquired_on: Option<chrono::NaiveDate>,
    /// band token -> path.
    bands: BTreeMap<String, PathBuf>,
}

/// Load an HLS band as surface reflectance. Real HLS is Int16 scaled DN
/// (× 1e-4); the fill value is a negative -9999 masked by the GeoTIFF
/// nodata tag. Integer dtypes (Int16/U16) are scaled to reflectance; f32
/// bands are already reflectance and pass through. `valid_mask` reflects the
/// original DN before scaling, so the fill never leaks into the index math.
fn load_hls_reflectance(path: &Path) -> Result<LoadedRaster, raster_io::RasterIoError> {
    let mut reader = raster_io::GeoTiffReader::open(path)?;
    let info = reader.info().clone();
    let spatial_ref = reader.spatial_ref()?;
    let band = reader.read_band()?;
    let is_integer = band.dtype().is_integer();
    let raw = band.to_f32();
    let nodata = info.nodata.map(|n| n as f32);
    let valid_mask: Vec<bool> = raw
        .iter()
        .map(|v| v.is_finite() && Some(*v) != nodata)
        .collect();
    let scale = if is_integer {
        HLS_REFLECTANCE_SCALE
    } else {
        1.0
    };
    let values = raw.iter().map(|v| v * scale).collect();
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

/// HLS v2.0 Fmask QA bits rejected for a clear-sky NDVI pixel: bit 0 cirrus,
/// bit 1 cloud, bit 2 adjacent-to-cloud/shadow, bit 3 cloud shadow. (Water
/// bit 5 and snow bit 4 are kept — they are valid ground, not obscuration.)
pub const FMASK_REJECT_BITS: u8 = 0b0000_1111;
/// Fmask fill value.
pub const FMASK_FILL: u8 = 255;

/// Build a per-pixel clear-sky mask from an HLS Fmask band (`true` = keep):
/// reject any pixel with a cloud/cirrus/adjacent/shadow bit set, or fill.
fn fmask_clear(fmask: &LoadedRaster) -> Vec<bool> {
    fmask
        .values
        .iter()
        .zip(&fmask.valid_mask)
        .map(|(value, valid)| {
            if !*valid || !value.is_finite() {
                return false;
            }
            let code = value.round() as i64;
            if code == i64::from(FMASK_FILL) || !(0..=255).contains(&code) {
                return false;
            }
            (code as u8) & FMASK_REJECT_BITS == 0
        })
        .collect()
}

fn to_index_pixels(raster: &LoadedRaster) -> Vec<IndexPixelValue> {
    raster
        .values
        .iter()
        .zip(&raster.valid_mask)
        .map(|(value, valid)| {
            if *valid && value.is_finite() {
                IndexPixelValue::Valid(*value)
            } else {
                IndexPixelValue::Invalid { reason: "fill" }
            }
        })
        .collect()
}

fn same_grid(a: &LoadedRaster, b: &LoadedRaster) -> bool {
    a.epsg == b.epsg
        && a.geo_transform == b.geo_transform
        && (a.width, a.height) == (b.width, b.height)
}

/// Register every complete HLS granule in a local directory as a harmonized
/// `ndvi` L2 product. Granules missing a required band are reported (not
/// errors); non-HLS files are skipped. Idempotent (content-addressed ids).
pub async fn register_hls_dir(pool: &DbPool, dir: &Path) -> Result<HlsRegisterOutcome, HlsError> {
    let mut entries: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .map_err(|source| HlsError::Store {
            what: "hls directory listing",
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
    entries.sort();

    let mut granules: BTreeMap<String, GranuleBands> = BTreeMap::new();
    let mut skipped = Vec::new();
    for (name, path) in entries {
        match parse_hls_filename(&name) {
            Some(parsed) => {
                let granule = granules.entry(parsed.granule_id()).or_default();
                granule.instrument = parsed.instrument.clone();
                granule.tile = parsed.tile.clone();
                granule.acquired_on = Some(parsed.acquired_on);
                granule.bands.insert(parsed.band.clone(), path);
            }
            None => skipped.push(name),
        }
    }

    let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let actor = provenance::ActorIdentity::system("geo_hub:hls_ingest");
    let mut registered = Vec::new();
    let mut incomplete = Vec::new();

    for (granule_id, granule) in granules {
        let Some((red_band, nir_band)) = ndvi_bands(&granule.instrument) else {
            incomplete.push((
                granule_id,
                format!("unknown instrument {}", granule.instrument),
            ));
            continue;
        };
        let (Some(red_path), Some(nir_path)) =
            (granule.bands.get(red_band), granule.bands.get(nir_band))
        else {
            incomplete.push((
                granule_id,
                format!("missing band ({red_band} and/or {nir_band})"),
            ));
            continue;
        };

        let red = load_hls_reflectance(red_path)?;
        let nir = load_hls_reflectance(nir_path)?;
        if !same_grid(&red, &nir) {
            return Err(HlsError::GridMismatch {
                granule: granule_id,
            });
        }

        let bands = BTreeMap::from([
            (IndexBandRole::Red, to_index_pixels(&red)),
            (IndexBandRole::Nir, to_index_pixels(&nir)),
        ]);
        // Clear-sky mask from the granule's Fmask band when present (raw u8
        // QA, not scaled reflectance); otherwise keep everything.
        let (clear_mask, fmask_applied) = match granule.bands.get("Fmask") {
            Some(fmask_path) => {
                let fmask = load_raster(fmask_path)?;
                if !same_grid(&fmask, &red) {
                    return Err(HlsError::GridMismatch {
                        granule: granule_id,
                    });
                }
                (fmask_clear(&fmask), true)
            }
            None => (vec![true; red.values.len()], false),
        };
        let index = compute_masked_index(IndexKind::Ndvi, &bands, &clear_mask)
            .map_err(|err| HlsError::Index(err.to_string()))?;

        let acquired_on = granule.acquired_on.expect("granule has a date");
        let draft_seed = ndvi_draft(&granule_id, &granule.instrument, &granule.tile, acquired_on);

        // Write the NDVI GeoTIFF next to the source granule's derived output.
        let out_dir = red_path
            .parent()
            .map(|p| p.join("ndvi"))
            .unwrap_or_else(|| PathBuf::from("ndvi"));
        std::fs::create_dir_all(&out_dir).map_err(|source| HlsError::Store {
            what: "hls ndvi directory",
            source,
        })?;
        let out_path = out_dir.join(format!(
            "{}.ndvi.tif",
            artifact_file_component(&draft_seed.product_id())
        ));
        write_geotiff_f32(
            &out_path,
            red.width,
            red.height,
            &index.values,
            &GeoTiffTags {
                epsg: red.epsg,
                geo_transform: red.geo_transform,
                nodata: Some(f64::from(INDEX_NODATA)),
            },
        )?;
        let checksum = file_checksum(&out_path, "hls ndvi readback")?;

        let mut draft = draft_seed;
        draft.spatial_ref = Some(red.spatial_ref.clone());
        draft.artifact = Some(ProductArtifact {
            format: "tif".to_string(),
            path: out_path.to_string_lossy().to_string(),
            checksum_sha256: Some(checksum.clone()),
        });
        draft.evidence_digests.push(checksum);
        draft.quality_summary = Some(serde_json::json!({
            "valid_pixels": index.valid_pixels,
            "invalid_pixels": index.invalid_pixels,
            "reasons": index.reason_counts,
            "fmask_applied": fmask_applied,
        }));
        if let Some(params) = draft.parameters.as_object_mut() {
            params.insert(
                "fmask_applied".to_string(),
                serde_json::json!(fmask_applied),
            );
        }

        let product_id =
            catalog::register_product_with_actor(pool, &draft, &actor, &created_at).await?;
        registered.push((granule_id, product_id));
    }

    Ok(HlsRegisterOutcome {
        registered,
        incomplete,
        skipped,
    })
}

fn ndvi_draft(
    scene_id: &str,
    instrument: &str,
    tile: &str,
    acquired_on: chrono::NaiveDate,
) -> ProductRecordDraft {
    let day = acquired_on.format("%Y-%m-%d");
    ProductRecordDraft {
        level: ProductLevel::L2,
        kind: "ndvi".to_string(),
        algorithm_id: "hls.ndvi".to_string(),
        algorithm_version: "2.0".to_string(),
        parameters: serde_json::json!({
            "dataset": "HLS v2.0 harmonized surface reflectance",
            "provider": "NASA LP DAAC",
            "instrument": instrument,
            "tile": tile,
            "acquired_on": acquired_on.format("%Y-%m-%d").to_string(),
            "index": "ndvi",
        }),
        inputs: Vec::new(),
        scope: ProductScope {
            farm_id: None,
            field_id: None,
            season_id: None,
            scene_id: Some(scene_id.to_string()),
            temporal_start: format!("{day}T00:00:00Z"),
            temporal_end: format!("{day}T23:59:59Z"),
        },
        spatial_ref: None,
        gsd_m_per_px: Some(30.0),
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: Vec::new(),
        source_id: Some(HLS_SOURCE_ID.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hls_filename_parsing_is_pinned() {
        let parsed = parse_hls_filename("HLS.S30.T43PFN.2024152T051651.v2.0.B04.tif").unwrap();
        assert_eq!(parsed.instrument, "S30");
        assert_eq!(parsed.tile, "43PFN");
        assert_eq!(parsed.band, "B04");
        // 2024 is a leap year; day-of-year 152 = May 31 (Feb has 29 days).
        assert_eq!(
            parsed.acquired_on,
            chrono::NaiveDate::from_ymd_opt(2024, 5, 31).unwrap()
        );
        assert_eq!(parsed.granule_id(), "HLS.S30.T43PFN.2024152T051651");

        let l30 = parse_hls_filename("HLS.L30.T43PFN.2024153T052015.v2.0.B05.tif").unwrap();
        assert_eq!(l30.instrument, "L30");
        assert_eq!(l30.band, "B05");

        for bad in [
            "HLS.X30.T43PFN.2024152T051651.v2.0.B04.tif", // bad instrument
            "HLS.S30.T43PFN.2024152T051651.B04.tif",      // no version segment
            "S2A_MSIL2A_20240601.tif",                    // not HLS
            "notes.txt",
        ] {
            assert!(parse_hls_filename(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn ndvi_bands_are_instrument_specific() {
        assert_eq!(ndvi_bands("S30"), Some(("B04", "B08")));
        assert_eq!(ndvi_bands("L30"), Some(("B04", "B05")));
        assert_eq!(ndvi_bands("X30"), None);
    }

    #[test]
    fn fmask_clear_rejects_cloud_bits_keeps_water_and_snow() {
        use shared::schemas::RasterSpatialRef;
        let raster = |codes: Vec<f32>| LoadedRaster {
            width: codes.len() as u32,
            height: 1,
            valid_mask: codes.iter().map(|c| *c != f32::from(FMASK_FILL)).collect(),
            values: codes,
            spatial_ref: RasterSpatialRef::default(),
            epsg: Some(32643),
            geo_transform: None,
        };
        // 0 clear, 2 cloud(bit1), 8 shadow(bit3), 1 cirrus(bit0),
        // 32 water(bit5, kept), 16 snow(bit4, kept), 255 fill.
        let clear = fmask_clear(&raster(vec![0.0, 2.0, 8.0, 1.0, 32.0, 16.0, 255.0]));
        assert_eq!(clear, vec![true, false, false, false, true, true, false]);
    }
}
