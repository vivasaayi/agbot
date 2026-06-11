#[cfg(feature = "gdal-io")]
pub mod gdal_util;

use crate::{IndicesArgs, SensorPreset};
use serde::{Deserialize, Serialize};
use shared::{
    error::AgroError,
    schemas::{
        assert_raster_spatial_ref, GeoBounds, MultispectralImage, RasterResolution,
        RasterSpatialRef,
    },
    AgroResult,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BandOverrides {
    pub red: Option<String>,
    pub nir: Option<String>,
    pub red_edge: Option<String>,
}

impl BandOverrides {
    pub fn from_indices_args(args: &IndicesArgs) -> Self {
        Self {
            red: args.red.clone(),
            nir: args.nir.clone(),
            red_edge: args.red_edge.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IngestedMultispectralImage {
    pub image: MultispectralImage,
    pub evidence: BandIngestEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BandIngestEvidence {
    pub image_id: uuid::Uuid,
    pub sensor: Option<String>,
    pub band_index_to_name: BTreeMap<usize, String>,
    pub resolved_bands: BTreeMap<String, String>,
    pub band_grids: BTreeMap<String, BandGridEvidence>,
    pub radiometric_calibration: RadiometricCalibrationEvidence,
    pub spatial_ref: RasterSpatialRef,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RadiometricCalibrationEvidence {
    pub status: CalibrationStatus,
    pub coefficients: BTreeMap<String, BandCalibrationCoefficients>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CalibrationStatus {
    CalibratedReflectance,
    UncalibratedDn,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BandCalibrationCoefficients {
    pub gain: f32,
    pub offset: f32,
    pub output_min: f32,
    pub output_max: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BandGridEvidence {
    pub width: u32,
    pub height: u32,
    pub dtype: String,
    pub nodata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeoTiffSpatialSidecar {
    pub format_version: u8,
    pub crs: String,
    pub bbox: GeoBounds,
    pub resolution: RasterResolution,
    pub geo_transform: [f64; 6],
}

impl GeoTiffSpatialSidecar {
    pub fn from_spatial_ref(spatial_ref: &RasterSpatialRef) -> AgroResult<Self> {
        Ok(Self {
            format_version: 1,
            crs: spatial_ref
                .crs
                .clone()
                .ok_or_else(|| geotiff_spatial_ref_error("missing CRS"))?,
            bbox: spatial_ref
                .bbox
                .clone()
                .ok_or_else(|| geotiff_spatial_ref_error("missing extent bbox"))?,
            resolution: spatial_ref
                .resolution
                .ok_or_else(|| geotiff_spatial_ref_error("missing resolution"))?,
            geo_transform: spatial_ref
                .geo_transform
                .ok_or_else(|| geotiff_spatial_ref_error("missing transform"))?,
        })
    }
}

pub fn geotiff_spatial_sidecar_path(product_path: &Path) -> PathBuf {
    let file_name = product_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("{name}.spatial_ref.json"))
        .unwrap_or_else(|| "product.tif.spatial_ref.json".to_string());
    product_path.with_file_name(file_name)
}

pub async fn write_geotiff_spatial_sidecar(
    product_path: &Path,
    spatial_ref: &RasterSpatialRef,
) -> AgroResult<PathBuf> {
    let sidecar = GeoTiffSpatialSidecar::from_spatial_ref(spatial_ref)?;
    let sidecar_path = geotiff_spatial_sidecar_path(product_path);
    tokio::fs::write(&sidecar_path, serde_json::to_vec_pretty(&sidecar)?).await?;
    Ok(sidecar_path)
}

fn geotiff_spatial_ref_error(message: &str) -> AgroError {
    AgroError::Processing(format!("GeoTIFF spatial sidecar error: {message}"))
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum BandIngestError {
    #[error("failed to read metadata {path}: {message}")]
    MetadataRead { path: PathBuf, message: String },
    #[error("failed to parse metadata {path}: {message}")]
    MetadataParse { path: PathBuf, message: String },
    #[error("required band '{band_name}' is missing from metadata")]
    MissingRequiredBand { band_name: String },
    #[error("failed to inspect raster for band '{band_name}' at {path}: {message}")]
    RasterInspect {
        band_name: String,
        path: PathBuf,
        message: String,
    },
    #[error(
        "band '{band_name}' dimensions mismatch: expected {expected_width}x{expected_height}, got {actual_width}x{actual_height}"
    )]
    DimensionMismatch {
        band_name: String,
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
    #[error("{message}")]
    SpatialRefInvalid { message: String },
    #[error("failed to write band ingest evidence {path}: {message}")]
    EvidenceWrite { path: PathBuf, message: String },
}

pub async fn load_multispectral_metadata(
    metadata_path: &Path,
) -> Result<MultispectralImage, BandIngestError> {
    let metadata_content = tokio::fs::read_to_string(metadata_path)
        .await
        .map_err(|err| BandIngestError::MetadataRead {
            path: metadata_path.to_path_buf(),
            message: err.to_string(),
        })?;

    serde_json::from_str(&metadata_content).map_err(|err| BandIngestError::MetadataParse {
        path: metadata_path.to_path_buf(),
        message: err.to_string(),
    })
}

pub async fn ingest_multispectral_image(
    metadata_path: &Path,
    sensor: Option<SensorPreset>,
    overrides: &BandOverrides,
) -> Result<IngestedMultispectralImage, BandIngestError> {
    let image = load_multispectral_metadata(metadata_path).await?;
    let evidence = resolve_band_ingest_evidence(&image, sensor, overrides)?;
    Ok(IngestedMultispectralImage { image, evidence })
}

pub async fn write_band_ingest_evidence(
    output_dir: &Path,
    evidence: &BandIngestEvidence,
) -> Result<PathBuf, BandIngestError> {
    tokio::fs::create_dir_all(output_dir)
        .await
        .map_err(|err| BandIngestError::EvidenceWrite {
            path: output_dir.to_path_buf(),
            message: err.to_string(),
        })?;

    let evidence_path = output_dir.join(format!("band_ingest_{}.json", evidence.image_id));
    let payload =
        serde_json::to_vec_pretty(evidence).map_err(|err| BandIngestError::EvidenceWrite {
            path: evidence_path.clone(),
            message: err.to_string(),
        })?;
    tokio::fs::write(&evidence_path, payload)
        .await
        .map_err(|err| BandIngestError::EvidenceWrite {
            path: evidence_path.clone(),
            message: err.to_string(),
        })?;
    Ok(evidence_path)
}

pub fn resolve_band_ingest_evidence(
    image: &MultispectralImage,
    sensor: Option<SensorPreset>,
    overrides: &BandOverrides,
) -> Result<BandIngestEvidence, BandIngestError> {
    let mut band_index_to_name: BTreeMap<usize, String> = image
        .metadata
        .bands
        .iter()
        .enumerate()
        .map(|(index, name)| (index, name.clone()))
        .collect();
    if band_index_to_name.is_empty() {
        let mut band_names = image.file_paths.keys().cloned().collect::<Vec<_>>();
        band_names.sort();
        for (index, name) in band_names.into_iter().enumerate() {
            band_index_to_name.insert(index, name);
        }
    }

    let (def_red, def_nir, def_red_edge) = sensor
        .map(|preset| preset.default_bands())
        .unwrap_or((None, None, None));
    let red = overrides
        .red
        .clone()
        .or(def_red.map(str::to_string))
        .unwrap_or_else(|| "Red".to_string());
    let nir = overrides
        .nir
        .clone()
        .or(def_nir.map(str::to_string))
        .unwrap_or_else(|| "NIR".to_string());
    let red_edge = overrides
        .red_edge
        .clone()
        .or(def_red_edge.map(str::to_string))
        .unwrap_or_else(|| "RE".to_string());

    let mut resolved_bands = BTreeMap::from([("red".to_string(), red), ("nir".to_string(), nir)]);
    if sensor.is_some() || overrides.red_edge.is_some() {
        resolved_bands.insert("red_edge".to_string(), red_edge);
    }

    for band_name in resolved_bands.values() {
        if !image.file_paths.contains_key(band_name) {
            return Err(BandIngestError::MissingRequiredBand {
                band_name: band_name.clone(),
            });
        }
    }

    let band_grids = inspect_band_grids(&image, image.metadata.width, image.metadata.height)?;
    let radiometric_calibration = radiometric_calibration_evidence(sensor, &band_index_to_name);
    let spatial_ref = assert_raster_spatial_ref(
        image.metadata.spatial_ref.as_ref(),
        image.metadata.width,
        image.metadata.height,
    )
    .map_err(|err| BandIngestError::SpatialRefInvalid {
        message: err.to_string(),
    })?;

    Ok(BandIngestEvidence {
        image_id: image.image_id,
        sensor: sensor.map(sensor_name).map(str::to_string),
        band_index_to_name,
        resolved_bands,
        band_grids,
        radiometric_calibration,
        spatial_ref,
        width: image.metadata.width,
        height: image.metadata.height,
    })
}

fn radiometric_calibration_evidence(
    sensor: Option<SensorPreset>,
    band_index_to_name: &BTreeMap<usize, String>,
) -> RadiometricCalibrationEvidence {
    match sensor {
        Some(SensorPreset::Sentinel2) | Some(SensorPreset::Landsat8) => {
            let coefficients = band_index_to_name
                .values()
                .map(|band_name| {
                    (
                        band_name.clone(),
                        BandCalibrationCoefficients {
                            gain: 1.0 / 255.0,
                            offset: 0.0,
                            output_min: 0.0,
                            output_max: 1.0,
                        },
                    )
                })
                .collect();

            RadiometricCalibrationEvidence {
                status: CalibrationStatus::CalibratedReflectance,
                coefficients,
            }
        }
        Some(SensorPreset::DjiMultispectral) | None => RadiometricCalibrationEvidence {
            status: CalibrationStatus::UncalibratedDn,
            coefficients: BTreeMap::new(),
        },
    }
}

fn inspect_band_grids(
    image: &MultispectralImage,
    expected_width: u32,
    expected_height: u32,
) -> Result<BTreeMap<String, BandGridEvidence>, BandIngestError> {
    let mut band_names = if image.metadata.bands.is_empty() {
        image.file_paths.keys().cloned().collect::<Vec<_>>()
    } else {
        image.metadata.bands.clone()
    };
    band_names.sort();

    let mut band_grids = BTreeMap::new();
    for band_name in band_names {
        let band_path = image.file_paths.get(&band_name).ok_or_else(|| {
            BandIngestError::MissingRequiredBand {
                band_name: band_name.to_string(),
            }
        })?;
        let band_path = PathBuf::from(band_path);
        let band = image::open(&band_path).map_err(|err| BandIngestError::RasterInspect {
            band_name: band_name.clone(),
            path: band_path.clone(),
            message: err.to_string(),
        })?;
        let actual_width = band.width();
        let actual_height = band.height();
        if actual_width != expected_width || actual_height != expected_height {
            return Err(BandIngestError::DimensionMismatch {
                band_name: band_name.clone(),
                expected_width,
                expected_height,
                actual_width,
                actual_height,
            });
        }
        band_grids.insert(
            band_name,
            BandGridEvidence {
                width: actual_width,
                height: actual_height,
                dtype: format!("{:?}", band.color()),
                nodata: None,
            },
        );
    }

    Ok(band_grids)
}

fn sensor_name(sensor: SensorPreset) -> &'static str {
    match sensor {
        SensorPreset::Sentinel2 => "sentinel2",
        SensorPreset::Landsat8 => "landsat8",
        SensorPreset::DjiMultispectral => "dji_multispectral",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SensorPreset;
    use image::{GrayImage, Luma};
    use serde_json::Value;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    fn temp_test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("agbot_{name}_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_gray_image(path: &Path, width: u32, height: u32, value: u8) {
        let mut image = GrayImage::new(width, height);
        for pixel in image.pixels_mut() {
            *pixel = Luma([value]);
        }
        image.save(path).unwrap();
    }

    fn valid_spatial_ref(width: u32, height: u32) -> Value {
        let origin_x = -74.1;
        let origin_y = 40.8;
        let pixel_x = 0.0001;
        let pixel_y = -0.0001;

        serde_json::json!({
            "georeferenced": true,
            "crs": "EPSG:4326",
            "bbox": {
                "min_lon": origin_x,
                "min_lat": origin_y + pixel_y * height as f64,
                "max_lon": origin_x + pixel_x * width as f64,
                "max_lat": origin_y
            },
            "geo_transform": [origin_x, pixel_x, 0.0, origin_y, 0.0, pixel_y]
        })
    }

    fn write_metadata(
        input_dir: &Path,
        width: u32,
        height: u32,
        bands: &[(&str, &Path)],
    ) -> PathBuf {
        let file_paths = bands
            .iter()
            .map(|(name, path)| {
                (
                    (*name).to_string(),
                    Value::String(path.to_string_lossy().to_string()),
                )
            })
            .collect::<serde_json::Map<String, Value>>();

        let metadata = serde_json::json!({
            "metadata": {
                "timestamp": "2026-01-01T00:00:00Z",
                "gps_position": null,
                "bands": bands.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
                "exposure_time": 1.0,
                "gain": 1.0,
                "width": width,
                "height": height,
                "spatial_ref": valid_spatial_ref(width, height)
            },
            "file_paths": file_paths,
            "image_id": uuid::Uuid::new_v4()
        });

        let metadata_path = input_dir.join("metadata_test.json");
        fs::write(
            &metadata_path,
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        metadata_path
    }

    #[tokio::test]
    async fn sentinel2_ingest_resolves_required_bands_and_writes_evidence() {
        let root = temp_test_dir("sentinel2_ingest");
        let input_dir = root.join("input");
        let output_dir = root.join("output");
        fs::create_dir_all(&input_dir).unwrap();
        fs::create_dir_all(&output_dir).unwrap();

        let red_path = input_dir.join("b04.png");
        let nir_path = input_dir.join("b08.png");
        let red_edge_path = input_dir.join("b05.png");
        write_gray_image(&red_path, 2, 1, 10);
        write_gray_image(&nir_path, 2, 1, 30);
        write_gray_image(&red_edge_path, 2, 1, 20);
        let metadata_path = write_metadata(
            &input_dir,
            2,
            1,
            &[
                ("B04", red_path.as_path()),
                ("B08", nir_path.as_path()),
                ("B05", red_edge_path.as_path()),
            ],
        );

        let ingest = ingest_multispectral_image(
            &metadata_path,
            Some(SensorPreset::Sentinel2),
            &BandOverrides::default(),
        )
        .await
        .unwrap();

        assert_eq!(ingest.evidence.sensor.as_deref(), Some("sentinel2"));
        assert_eq!(ingest.evidence.width, 2);
        assert_eq!(ingest.evidence.height, 1);
        assert_eq!(ingest.evidence.band_index_to_name.get(&0).unwrap(), "B04");
        assert_eq!(ingest.evidence.band_index_to_name.get(&1).unwrap(), "B08");
        assert_eq!(ingest.evidence.band_index_to_name.get(&2).unwrap(), "B05");
        assert_eq!(ingest.evidence.resolved_bands.get("red").unwrap(), "B04");
        assert_eq!(ingest.evidence.resolved_bands.get("nir").unwrap(), "B08");
        assert_eq!(
            ingest.evidence.resolved_bands.get("red_edge").unwrap(),
            "B05"
        );

        let evidence_path = write_band_ingest_evidence(&output_dir, &ingest.evidence)
            .await
            .unwrap();
        let evidence_json: BandIngestEvidence =
            serde_json::from_str(&fs::read_to_string(evidence_path).unwrap()).unwrap();
        assert_eq!(evidence_json, ingest.evidence);
    }

    #[tokio::test]
    async fn sentinel2_ingest_reports_missing_required_band() {
        let root = temp_test_dir("sentinel2_missing_band");
        let input_dir = root.join("input");
        fs::create_dir_all(&input_dir).unwrap();

        let red_path = input_dir.join("b04.png");
        let nir_path = input_dir.join("b08.png");
        write_gray_image(&red_path, 2, 1, 10);
        write_gray_image(&nir_path, 2, 1, 30);
        let metadata_path = write_metadata(
            &input_dir,
            2,
            1,
            &[("B04", red_path.as_path()), ("B08", nir_path.as_path())],
        );

        let error = ingest_multispectral_image(
            &metadata_path,
            Some(SensorPreset::Sentinel2),
            &BandOverrides::default(),
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            BandIngestError::MissingRequiredBand { ref band_name } if band_name == "B05"
        ));
    }

    #[tokio::test]
    async fn ingest_records_grid_evidence_for_every_band() {
        let root = temp_test_dir("grid_evidence");
        let input_dir = root.join("input");
        fs::create_dir_all(&input_dir).unwrap();

        let red_path = input_dir.join("red.png");
        let nir_path = input_dir.join("nir.png");
        let blue_path = input_dir.join("blue.png");
        write_gray_image(&red_path, 2, 1, 10);
        write_gray_image(&nir_path, 2, 1, 30);
        write_gray_image(&blue_path, 2, 1, 5);
        let metadata_path = write_metadata(
            &input_dir,
            2,
            1,
            &[
                ("Red", red_path.as_path()),
                ("NIR", nir_path.as_path()),
                ("Blue", blue_path.as_path()),
            ],
        );

        let ingest = ingest_multispectral_image(&metadata_path, None, &BandOverrides::default())
            .await
            .unwrap();

        let blue_grid = ingest.evidence.band_grids.get("Blue").unwrap();
        assert_eq!(blue_grid.width, 2);
        assert_eq!(blue_grid.height, 1);
        assert_eq!(blue_grid.dtype, "L8");
        assert_eq!(blue_grid.nodata, None);
    }

    #[tokio::test]
    async fn ingest_rejects_mismatched_dimensions_in_any_metadata_band() {
        let root = temp_test_dir("grid_mismatch");
        let input_dir = root.join("input");
        fs::create_dir_all(&input_dir).unwrap();

        let red_path = input_dir.join("red.png");
        let nir_path = input_dir.join("nir.png");
        let blue_path = input_dir.join("blue.png");
        write_gray_image(&red_path, 2, 1, 10);
        write_gray_image(&nir_path, 2, 1, 30);
        write_gray_image(&blue_path, 1, 1, 5);
        let metadata_path = write_metadata(
            &input_dir,
            2,
            1,
            &[
                ("Red", red_path.as_path()),
                ("NIR", nir_path.as_path()),
                ("Blue", blue_path.as_path()),
            ],
        );

        let error = ingest_multispectral_image(&metadata_path, None, &BandOverrides::default())
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            BandIngestError::DimensionMismatch {
                ref band_name,
                expected_width: 2,
                expected_height: 1,
                actual_width: 1,
                actual_height: 1,
            } if band_name == "Blue"
        ));
    }
}
