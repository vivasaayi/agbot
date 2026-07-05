//! GeoTIFF band ingestion for the indices pipeline.
//!
//! When a `metadata_*.json` points its band `file_paths` at `.tif`/`.tiff`
//! rasters, bands are read with `raster_io` (pure Rust, no GDAL), the
//! georeferencing (CRS + geotransform) is captured from the GeoTIFF tags into
//! the ingest evidence, all bands are asserted to agree on
//! CRS/geotransform/dimensions (typed error, no silent resampling), the
//! `GDAL_NODATA` value is honored as fill, and an optional batch-1
//! `SensorProfile` scales u16 DNs to reflectance before index math.

use crate::io::{
    BandGridEvidence, BandIngestEvidence, CalibrationStatus, ImageIngestQualityEvidence,
    IngestCoverageStatus, RadiometricCalibrationEvidence, DEFAULT_VALID_PIXEL_FLOOR,
};
use crate::pipeline::calibration::{apply_radiometric_scaling, SensorProfile};
use crate::SensorPreset;
use chrono::Utc;
use raster_io::{GeoTiffReader, RasterBand};
use shared::schemas::{MultispectralImage, RasterSpatialRef};
use std::collections::BTreeMap;

pub fn is_geotiff_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".tif") || lower.ends_with(".tiff")
}

#[derive(Debug, thiserror::Error)]
pub enum GeoTiffIngestError {
    #[error("required band '{band_name}' is missing from metadata")]
    MissingRequiredBand { band_name: String },
    #[error("failed to read GeoTIFF band '{band_name}': {source}")]
    Read {
        band_name: String,
        source: raster_io::RasterIoError,
    },
    #[error(
        "band '{band_name}' dimensions {actual_width}x{actual_height} do not match expected {expected_width}x{expected_height}; resampling is not supported"
    )]
    DimensionMismatch {
        band_name: String,
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
    #[error(
        "band '{band_name}' CRS {actual:?} does not match {expected:?}; reprojection is not supported"
    )]
    CrsMismatch {
        band_name: String,
        expected: Option<String>,
        actual: Option<String>,
    },
    #[error(
        "band '{band_name}' geotransform {actual:?} does not match {expected:?}; resampling is not supported"
    )]
    GeoTransformMismatch {
        band_name: String,
        expected: Option<Box<[f64; 6]>>,
        actual: Option<Box<[f64; 6]>>,
    },
    #[error(
        "sensor profile radiometric scaling requires u16 DN bands, but band '{band_name}' is {dtype}"
    )]
    ProfileDtypeUnsupported { band_name: String, dtype: String },
}

/// A GeoTIFF band decoded to the pipeline's working representation:
/// per-pixel f32 values plus a validity mask (nodata/fill pixels invalid).
#[derive(Debug, Clone)]
pub struct GeoTiffLoadedBand {
    pub values: Vec<f32>,
    pub valid: Vec<bool>,
}

#[derive(Debug)]
pub struct GeoTiffIndexIngest {
    pub evidence: BandIngestEvidence,
    /// Loaded bands keyed by band name (a band may serve multiple roles).
    pub bands: BTreeMap<String, GeoTiffLoadedBand>,
}

pub fn ingest_geotiff_index_bands(
    image: &MultispectralImage,
    sensor: Option<SensorPreset>,
    resolved_bands: BTreeMap<String, String>,
    profile: Option<SensorProfile>,
) -> Result<GeoTiffIndexIngest, GeoTiffIngestError> {
    let expected_width = image.metadata.width;
    let expected_height = image.metadata.height;

    let mut band_names: Vec<String> = resolved_bands.values().cloned().collect();
    band_names.sort();
    band_names.dedup();

    let mut reference: Option<(String, RasterSpatialRef)> = None;
    let mut band_grids = BTreeMap::new();
    let mut bands = BTreeMap::new();

    for band_name in &band_names {
        let band_path = image.file_paths.get(band_name).ok_or_else(|| {
            GeoTiffIngestError::MissingRequiredBand {
                band_name: band_name.clone(),
            }
        })?;
        let read_error = |source: raster_io::RasterIoError| GeoTiffIngestError::Read {
            band_name: band_name.clone(),
            source,
        };
        let mut reader = GeoTiffReader::open(band_path).map_err(read_error)?;
        let info = reader.info().clone();

        if (info.width, info.height) != (expected_width, expected_height) {
            return Err(GeoTiffIngestError::DimensionMismatch {
                band_name: band_name.clone(),
                expected_width,
                expected_height,
                actual_width: info.width,
                actual_height: info.height,
            });
        }

        let spatial_ref = reader.spatial_ref().map_err(read_error)?;
        match &reference {
            None => reference = Some((band_name.clone(), spatial_ref.clone())),
            Some((_, expected)) => {
                if expected.crs != spatial_ref.crs {
                    return Err(GeoTiffIngestError::CrsMismatch {
                        band_name: band_name.clone(),
                        expected: expected.crs.clone(),
                        actual: spatial_ref.crs.clone(),
                    });
                }
                if expected.geo_transform != spatial_ref.geo_transform {
                    return Err(GeoTiffIngestError::GeoTransformMismatch {
                        band_name: band_name.clone(),
                        expected: expected.geo_transform.map(Box::new),
                        actual: spatial_ref.geo_transform.map(Box::new),
                    });
                }
            }
        }

        let raster = reader.read_band().map_err(read_error)?;
        let loaded = load_band_values(band_name, &raster, info.nodata, profile)?;

        band_grids.insert(
            band_name.clone(),
            BandGridEvidence {
                width: info.width,
                height: info.height,
                dtype: raster.dtype().name().to_string(),
                nodata: info.nodata.map(|nodata| format!("{nodata}")),
            },
        );
        bands.insert(band_name.clone(), loaded);
    }

    let (_, spatial_ref) = reference.ok_or_else(|| GeoTiffIngestError::MissingRequiredBand {
        band_name: "<none resolved>".to_string(),
    })?;

    let radiometric_calibration = match profile {
        Some(profile) => profile.calibration_evidence(&band_names),
        None => RadiometricCalibrationEvidence {
            status: CalibrationStatus::UncalibratedDn,
            coefficients: BTreeMap::new(),
        },
    };

    let mut band_index_to_name: BTreeMap<usize, String> = image
        .metadata
        .bands
        .iter()
        .enumerate()
        .map(|(index, name)| (index, name.clone()))
        .collect();
    if band_index_to_name.is_empty() {
        let mut all_names = image.file_paths.keys().cloned().collect::<Vec<_>>();
        all_names.sort();
        for (index, name) in all_names.into_iter().enumerate() {
            band_index_to_name.insert(index, name);
        }
    }

    let ingest_quality = build_quality_evidence(image, &bands, expected_width, expected_height);

    Ok(GeoTiffIndexIngest {
        evidence: BandIngestEvidence {
            image_id: image.image_id,
            sensor: sensor.map(crate::io::sensor_name).map(str::to_string),
            band_index_to_name,
            resolved_bands,
            band_grids,
            radiometric_calibration,
            spatial_ref,
            width: expected_width,
            height: expected_height,
            ingest_quality,
        },
        bands,
    })
}

fn load_band_values(
    band_name: &str,
    raster: &RasterBand,
    nodata: Option<f64>,
    profile: Option<SensorProfile>,
) -> Result<GeoTiffLoadedBand, GeoTiffIngestError> {
    match (profile, raster) {
        (Some(profile), RasterBand::U16(dns)) => {
            let scaled = apply_radiometric_scaling(profile, dns);
            let mut values = Vec::with_capacity(dns.len());
            let mut valid = Vec::with_capacity(dns.len());
            for (dn, pixel) in dns.iter().zip(&scaled.pixels) {
                let is_nodata = nodata.is_some_and(|nodata| f64::from(*dn) == nodata);
                match pixel.value() {
                    Some(value) if !is_nodata => {
                        values.push(value);
                        valid.push(true);
                    }
                    _ => {
                        values.push(0.0);
                        valid.push(false);
                    }
                }
            }
            Ok(GeoTiffLoadedBand { values, valid })
        }
        (Some(_), other) => Err(GeoTiffIngestError::ProfileDtypeUnsupported {
            band_name: band_name.to_string(),
            dtype: other.dtype().name().to_string(),
        }),
        (None, raster) => {
            let values = raster.to_f32();
            let valid = (0..values.len())
                .map(|index| nodata.is_none_or(|nodata| raster.value_as_f64(index) != Some(nodata)))
                .collect();
            Ok(GeoTiffLoadedBand { values, valid })
        }
    }
}

fn build_quality_evidence(
    image: &MultispectralImage,
    bands: &BTreeMap<String, GeoTiffLoadedBand>,
    width: u32,
    height: u32,
) -> ImageIngestQualityEvidence {
    let total_pixel_count = (width * height) as usize;
    let mut valid_pixels = vec![false; total_pixel_count];
    for band in bands.values() {
        for (index, is_valid) in band.valid.iter().enumerate() {
            if *is_valid {
                valid_pixels[index] = true;
            }
        }
    }
    let valid_pixel_count = valid_pixels.iter().filter(|is_valid| **is_valid).count();
    let valid_pixel_fraction = if total_pixel_count == 0 {
        0.0
    } else {
        valid_pixel_count as f32 / total_pixel_count as f32
    };
    let coverage_status = if valid_pixel_fraction >= DEFAULT_VALID_PIXEL_FLOOR {
        IngestCoverageStatus::Pass
    } else {
        IngestCoverageStatus::LowCoverage
    };
    let ingested_at = Utc::now();
    ImageIngestQualityEvidence {
        capture_time: image.metadata.timestamp,
        ingested_at,
        capture_age_seconds: ingested_at
            .signed_duration_since(image.metadata.timestamp)
            .num_seconds()
            .max(0),
        total_pixel_count,
        valid_pixel_count,
        valid_pixel_fraction,
        coverage_floor: DEFAULT_VALID_PIXEL_FLOOR,
        coverage_status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geotiff_paths_are_detected_by_extension() {
        assert!(is_geotiff_path("/scene/B4.tif"));
        assert!(is_geotiff_path("/scene/B4.TIFF"));
        assert!(!is_geotiff_path("/scene/B4.png"));
        assert!(!is_geotiff_path("/scene/B4.tif.json"));
    }

    #[test]
    fn nodata_dns_are_invalid_without_a_profile() {
        let raster = RasterBand::U16(vec![0, 500, 0, 700]);
        let loaded = load_band_values("B4", &raster, Some(0.0), None).unwrap();
        assert_eq!(loaded.values, vec![0.0, 500.0, 0.0, 700.0]);
        assert_eq!(loaded.valid, vec![false, true, false, true]);
    }

    #[test]
    fn profile_scales_dns_and_marks_nodata_and_fill_invalid() {
        let raster = RasterBand::U16(vec![0, 10000, 20000]);
        let loaded =
            load_band_values("B4", &raster, Some(0.0), Some(SensorProfile::LandsatC2L2Sr)).unwrap();
        assert_eq!(loaded.valid, vec![false, true, true]);
        assert!((loaded.values[1] - 0.075).abs() < 1e-6);
        assert!((loaded.values[2] - 0.35).abs() < 1e-6);
    }

    #[test]
    fn profile_rejects_non_u16_bands_with_typed_error() {
        let raster = RasterBand::F32(vec![0.1, 0.2]);
        let error =
            load_band_values("B4", &raster, None, Some(SensorProfile::LandsatC2L2Sr)).unwrap_err();
        assert!(matches!(
            error,
            GeoTiffIngestError::ProfileDtypeUnsupported { ref dtype, .. } if dtype == "f32"
        ));
    }
}
