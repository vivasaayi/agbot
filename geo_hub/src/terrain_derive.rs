//! Catalog-to-simulator L3 terrain derivation.
//!
//! Elevation products are normalized to the deliberately narrow GeoTIFF
//! boundary consumed by `flight_sim_cpp`, compiled into `.agbworld` +
//! `.agbscn`, then registered as a traceable L3 catalog product.

use crate::catalog::{self, CatalogError, RegisteredProduct};
use crate::db::DbPool;
use chrono::{SecondsFormat, Utc};
use raster_io::{write_geotiff_f32, GeoTiffReader, GeoTiffTags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::schemas::{GeoBounds, RasterSpatialRef};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use thiserror::Error;

const ALGORITHM_ID: &str = "flight_sim_cpp.world_compiler";
const ALGORITHM_VERSION: &str = "agbworld-1";
const PRODUCT_KIND: &str = "sim_terrain_package";
const DEFAULT_RESOLUTION: u32 = 128;
const MAX_RESOLUTION: u32 = 2048;
const NORMALIZED_NODATA: f32 = f32::MIN;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TerrainDeriveRequest {
    pub elevation_product_id: String,
    #[serde(default)]
    pub aoi: Option<GeoBounds>,
    #[serde(default)]
    pub resolution: Option<u32>,
    #[serde(default)]
    pub target_gsd_m: Option<f64>,
    #[serde(default)]
    pub expected_vertical_datum: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainDeriveOutcome {
    pub terrain_product_id: String,
    pub product_kind: String,
    pub elevation_product_id: String,
    pub manifest_path: PathBuf,
    pub scene_path: PathBuf,
    pub validation_path: PathBuf,
    pub vertical_datum: String,
    pub world_hash: u64,
    pub elevation_state: String,
    pub terrain_min_m: f64,
    pub terrain_max_m: f64,
    pub terrain_cell_count: u64,
    pub terrain_nodata_cells: u64,
    pub reused_existing: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainCompileSpec {
    pub dem_path: PathBuf,
    pub output_dir: PathBuf,
    pub name: String,
    pub aoi: GeoBounds,
    pub resolution: u32,
    pub target_gsd_m: f64,
    pub vertical_datum: String,
    pub seed: u64,
    pub source_id: String,
    pub source_version: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TerrainCompileOutput {
    pub manifest_path: PathBuf,
    pub scene_path: PathBuf,
    pub validation_path: PathBuf,
    pub world_hash: u64,
    pub elevation_state: String,
    pub terrain_min_m: f64,
    pub terrain_max_m: f64,
    pub terrain_cell_count: u64,
    pub terrain_nodata_cells: u64,
}

pub trait TerrainCompiler: Send + Sync {
    fn compile(
        &self,
        spec: &TerrainCompileSpec,
    ) -> Result<TerrainCompileOutput, TerrainDeriveError>;
}

#[derive(Debug, Clone)]
pub struct ProcessTerrainCompiler {
    program: PathBuf,
}

impl ProcessTerrainCompiler {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }
}

impl TerrainCompiler for ProcessTerrainCompiler {
    fn compile(
        &self,
        spec: &TerrainCompileSpec,
    ) -> Result<TerrainCompileOutput, TerrainDeriveError> {
        let output = Command::new(&self.program)
            .arg("--dem")
            .arg(&spec.dem_path)
            .arg("--output-dir")
            .arg(&spec.output_dir)
            .arg("--name")
            .arg(&spec.name)
            .arg("--min-lat")
            .arg(spec.aoi.min_lat.to_string())
            .arg("--min-lon")
            .arg(spec.aoi.min_lon.to_string())
            .arg("--max-lat")
            .arg(spec.aoi.max_lat.to_string())
            .arg("--max-lon")
            .arg(spec.aoi.max_lon.to_string())
            .arg("--resolution")
            .arg(spec.resolution.to_string())
            .arg("--target-gsd-m")
            .arg(spec.target_gsd_m.to_string())
            .arg("--vertical-datum")
            .arg(&spec.vertical_datum)
            .arg("--seed")
            .arg(spec.seed.to_string())
            .arg("--source-id")
            .arg(&spec.source_id)
            .arg("--source-version")
            .arg(&spec.source_version)
            .output()
            .map_err(|source| TerrainDeriveError::CompilerUnavailable {
                path: self.program.clone(),
                source,
            })?;
        if !output.status.success() {
            return Err(TerrainDeriveError::CompilerFailed {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }
        serde_json::from_slice(&output.stdout).map_err(|source| {
            TerrainDeriveError::InvalidCompilerOutput {
                detail: source.to_string(),
            }
        })
    }
}

#[derive(Debug, Error)]
pub enum TerrainDeriveError {
    #[error("elevation product {0} was not found")]
    ProductNotFound(String),
    #[error("product {product_id} is not a registered L1 elevation product")]
    UnsupportedProduct { product_id: String },
    #[error("elevation product {product_id} has no local GeoTIFF artifact")]
    MissingArtifact { product_id: String },
    #[error("elevation artifact checksum mismatch: catalog={expected}, actual={actual}")]
    SourceChecksumMismatch { expected: String, actual: String },
    #[error("terrain derivation currently requires EPSG:4326; product declares {0}")]
    UnsupportedCrs(String),
    #[error("elevation product has no valid geographic extent")]
    MissingExtent,
    #[error("terrain AOI is invalid: {0}")]
    InvalidAoi(String),
    #[error("terrain AOI must be contained by the elevation product extent")]
    AoiOutsideCoverage,
    #[error("elevation product does not declare a supported vertical datum")]
    MissingVerticalDatum,
    #[error("unsupported vertical datum {0}")]
    UnsupportedVerticalDatum(String),
    #[error("vertical datum mismatch: product={product}, expected={expected}")]
    VerticalDatumMismatch { product: String, expected: String },
    #[error("terrain resolution must be in [2, {MAX_RESOLUTION}]")]
    InvalidResolution,
    #[error("target_gsd_m must be finite and positive")]
    InvalidTargetGsd,
    #[error("failed to decode or normalize elevation GeoTIFF: {0}")]
    Raster(String),
    #[error("terrain compiler is unavailable at {path}: {source}")]
    CompilerUnavailable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("terrain compiler failed (status {status:?}): {stderr}")]
    CompilerFailed { status: Option<i32>, stderr: String },
    #[error("terrain compiler returned invalid output: {detail}")]
    InvalidCompilerOutput { detail: String },
    #[error(
        "terrain package checksum mismatch for {artifact}: expected {expected}, actual {actual}"
    )]
    PackageChecksumMismatch {
        artifact: String,
        expected: String,
        actual: String,
    },
    #[error("terrain compiler task failed: {0}")]
    CompilerTask(String),
    #[error("terrain artifact I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

impl TerrainDeriveError {
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::UnsupportedProduct { .. }
                | Self::MissingArtifact { .. }
                | Self::SourceChecksumMismatch { .. }
                | Self::UnsupportedCrs(_)
                | Self::MissingExtent
                | Self::InvalidAoi(_)
                | Self::AoiOutsideCoverage
                | Self::MissingVerticalDatum
                | Self::UnsupportedVerticalDatum(_)
                | Self::VerticalDatumMismatch { .. }
                | Self::InvalidResolution
                | Self::InvalidTargetGsd
                | Self::Raster(_)
        )
    }
}

fn canonical_vertical_datum(value: &str) -> Result<String, TerrainDeriveError> {
    let normalized = value.trim().to_ascii_lowercase().replace(['_', '-'], " ");
    match normalized.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["egm96"] | ["egm", "96"] => Ok("EGM96".to_string()),
        ["egm2008"] | ["egm", "2008"] => Ok("EGM2008".to_string()),
        ["navd88"] => Ok("NAVD88".to_string()),
        ["navd88", "geoid18"] | ["geoid18"] => Ok("NAVD88_GEOID18".to_string()),
        ["wgs84", "ellipsoid"] | ["wgs84", "ellipsoidal"] => Ok("WGS84 ellipsoid".to_string()),
        ["ellipsoid"] | ["ellipsoidal"] => Ok("ellipsoidal".to_string()),
        _ => Err(TerrainDeriveError::UnsupportedVerticalDatum(
            value.trim().to_string(),
        )),
    }
}

fn valid_aoi(aoi: &GeoBounds) -> bool {
    [aoi.min_lon, aoi.min_lat, aoi.max_lon, aoi.max_lat]
        .iter()
        .all(|value| value.is_finite())
        && aoi.min_lon >= -180.0
        && aoi.max_lon <= 180.0
        && aoi.min_lat >= -90.0
        && aoi.max_lat <= 90.0
        && aoi.min_lon < aoi.max_lon
        && aoi.min_lat < aoi.max_lat
}

fn contains(container: &[f64; 4], aoi: &GeoBounds) -> bool {
    aoi.min_lon >= container[0]
        && aoi.min_lat >= container[1]
        && aoi.max_lon <= container[2]
        && aoi.max_lat <= container[3]
}

fn safe_component(value: &str) -> String {
    let value: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    value.trim_matches(['.', '_']).to_string()
}

fn checksum_file(path: &Path) -> Result<String, TerrainDeriveError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn normalize_dem(source_path: &Path, output_path: &Path) -> Result<(), TerrainDeriveError> {
    let mut reader = GeoTiffReader::open(source_path)
        .map_err(|error| TerrainDeriveError::Raster(error.to_string()))?;
    let info = reader.info().clone();
    if info.epsg != Some(4326) {
        return Err(TerrainDeriveError::UnsupportedCrs(
            info.crs().unwrap_or_else(|| "undeclared".to_string()),
        ));
    }
    let transform = info
        .geo_transform
        .ok_or_else(|| TerrainDeriveError::Raster("missing geotransform".to_string()))?;
    let band = reader
        .read_band()
        .map_err(|error| TerrainDeriveError::Raster(error.to_string()))?;
    let source_nodata = info.nodata;
    let mut values = band.to_f32();
    for value in &mut values {
        let matches_nodata = source_nodata.is_some_and(|nodata| {
            (f64::from(*value) - nodata).abs()
                <= f64::EPSILON.max(nodata.abs() * f64::EPSILON * 4.0)
        });
        if !value.is_finite() || matches_nodata {
            *value = NORMALIZED_NODATA;
        }
    }
    let temporary = output_path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    write_geotiff_f32(
        &temporary,
        info.width,
        info.height,
        &values,
        &GeoTiffTags {
            epsg: Some(4326),
            geo_transform: Some(transform),
            nodata: Some(f64::from(NORMALIZED_NODATA)),
        },
    )
    .map_err(|error| TerrainDeriveError::Raster(error.to_string()))?;
    std::fs::rename(temporary, output_path)?;
    Ok(())
}

fn product_scope(product: &RegisteredProduct) -> ProductScope {
    ProductScope {
        farm_id: product.farm_id.clone(),
        field_id: product.field_id.clone(),
        season_id: product.season_id.clone(),
        scene_id: product.scene_id.clone(),
        temporal_start: product.temporal_start.clone().unwrap_or_default(),
        temporal_end: product.temporal_end.clone().unwrap_or_default(),
    }
}

fn outcome_from_product(
    product: &RegisteredProduct,
    elevation_product_id: &str,
    reused_existing: bool,
) -> Result<TerrainDeriveOutcome, TerrainDeriveError> {
    let manifest_path = product.path.as_deref().map(PathBuf::from).ok_or_else(|| {
        TerrainDeriveError::MissingArtifact {
            product_id: product.product_id.clone(),
        }
    })?;
    let directory = manifest_path
        .parent()
        .ok_or_else(|| TerrainDeriveError::InvalidCompilerOutput {
            detail: "manifest has no parent directory".to_string(),
        })?
        .to_path_buf();
    let string_parameter = |name: &str| {
        product.parameters[name]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| TerrainDeriveError::InvalidCompilerOutput {
                detail: format!("cataloged terrain package is missing {name}"),
            })
    };
    let quality = product.quality_summary.as_ref();
    let quality_string = |name: &str| {
        quality
            .and_then(|summary| summary[name].as_str())
            .map(str::to_string)
            .ok_or_else(|| TerrainDeriveError::InvalidCompilerOutput {
                detail: format!("cataloged terrain package is missing quality evidence {name}"),
            })
    };
    Ok(TerrainDeriveOutcome {
        terrain_product_id: product.product_id.clone(),
        product_kind: product.kind.clone(),
        elevation_product_id: elevation_product_id.to_string(),
        manifest_path,
        scene_path: directory.join(string_parameter("scene_file")?),
        validation_path: directory.join(string_parameter("validation_file")?),
        vertical_datum: string_parameter("vertical_datum")?,
        world_hash: quality
            .and_then(|summary| summary["world_hash"].as_u64())
            .ok_or_else(|| TerrainDeriveError::InvalidCompilerOutput {
                detail: "cataloged terrain package is missing world_hash evidence".to_string(),
            })?,
        elevation_state: quality_string("elevation_state")?,
        terrain_min_m: quality
            .and_then(|summary| summary["terrain_min_m"].as_f64())
            .unwrap_or_default(),
        terrain_max_m: quality
            .and_then(|summary| summary["terrain_max_m"].as_f64())
            .unwrap_or_default(),
        terrain_cell_count: quality
            .and_then(|summary| summary["terrain_cell_count"].as_u64())
            .unwrap_or_default(),
        terrain_nodata_cells: quality
            .and_then(|summary| summary["terrain_nodata_cells"].as_u64())
            .unwrap_or_default(),
        reused_existing,
    })
}

async fn verify_existing_package(
    product: &RegisteredProduct,
    outcome: &TerrainDeriveOutcome,
) -> Result<(), TerrainDeriveError> {
    let quality = product.quality_summary.as_ref().ok_or_else(|| {
        TerrainDeriveError::InvalidCompilerOutput {
            detail: "cataloged terrain package has no quality evidence".to_string(),
        }
    })?;
    let required_checksum = |name: &str| {
        quality[name].as_str().map(str::to_string).ok_or_else(|| {
            TerrainDeriveError::InvalidCompilerOutput {
                detail: format!("cataloged terrain package is missing {name}"),
            }
        })
    };
    let artifacts = vec![
        (
            "manifest".to_string(),
            outcome.manifest_path.clone(),
            product.checksum_sha256.clone().ok_or_else(|| {
                TerrainDeriveError::InvalidCompilerOutput {
                    detail: "cataloged terrain manifest has no checksum".to_string(),
                }
            })?,
        ),
        (
            "scene".to_string(),
            outcome.scene_path.clone(),
            required_checksum("scene_checksum_sha256")?,
        ),
        (
            "validation".to_string(),
            outcome.validation_path.clone(),
            required_checksum("validation_checksum_sha256")?,
        ),
    ];
    tokio::task::spawn_blocking(move || {
        for (artifact, path, expected) in artifacts {
            let actual = checksum_file(&path)?;
            if !actual.eq_ignore_ascii_case(&expected) {
                return Err(TerrainDeriveError::PackageChecksumMismatch {
                    artifact,
                    expected,
                    actual,
                });
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| TerrainDeriveError::CompilerTask(error.to_string()))?
}

fn validate_compiler_output(
    output: &TerrainCompileOutput,
    expected_directory: &Path,
    vertical_datum: &str,
) -> Result<serde_json::Value, TerrainDeriveError> {
    let package_root = std::fs::canonicalize(expected_directory).map_err(|source| {
        TerrainDeriveError::InvalidCompilerOutput {
            detail: format!("cannot resolve terrain package directory: {source}"),
        }
    })?;
    for (kind, path, expected) in [
        (
            "manifest",
            &output.manifest_path,
            expected_directory.join("terrain.agbworld"),
        ),
        (
            "scene",
            &output.scene_path,
            expected_directory.join("terrain.agbscn"),
        ),
        (
            "validation",
            &output.validation_path,
            expected_directory.join("terrain.validation.json"),
        ),
    ] {
        let resolved =
            std::fs::canonicalize(path).map_err(|_| TerrainDeriveError::InvalidCompilerOutput {
                detail: format!(
                    "compiler artifact {} is missing or outside its package",
                    path.display()
                ),
            })?;
        let expected = std::fs::canonicalize(&expected).map_err(|_| {
            TerrainDeriveError::InvalidCompilerOutput {
                detail: format!(
                    "compiler {kind} artifact {} is missing or outside its package",
                    path.display()
                ),
            }
        })?;
        if !resolved.starts_with(&package_root) || resolved != expected || !resolved.is_file() {
            return Err(TerrainDeriveError::InvalidCompilerOutput {
                detail: format!(
                    "compiler artifact {} is missing or outside its package",
                    path.display()
                ),
            });
        }
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output.manifest_path)?).map_err(|source| {
            TerrainDeriveError::InvalidCompilerOutput {
                detail: format!("manifest JSON: {source}"),
            }
        })?;
    if manifest["world_hash"].as_u64() != Some(output.world_hash) {
        return Err(TerrainDeriveError::InvalidCompilerOutput {
            detail: "stdout world_hash does not match the manifest".to_string(),
        });
    }
    if manifest["crs_policy"]["horizontal"] != "EPSG:4326"
        || manifest["crs_policy"]["vertical_datum"] != vertical_datum
    {
        return Err(TerrainDeriveError::InvalidCompilerOutput {
            detail: "manifest CRS or vertical datum does not match the requested policy"
                .to_string(),
        });
    }
    let state = manifest["tiles"]
        .as_array()
        .and_then(|tiles| tiles.first())
        .and_then(|tile| tile["elevation_state"].as_str());
    let scene_path = manifest["tiles"]
        .as_array()
        .and_then(|tiles| tiles.first())
        .and_then(|tile| tile["scene_path"].as_str());
    if scene_path != Some("terrain.agbscn") {
        return Err(TerrainDeriveError::InvalidCompilerOutput {
            detail: "manifest does not reference the cataloged scene payload".to_string(),
        });
    }
    if state != Some(output.elevation_state.as_str())
        || !matches!(
            output.elevation_state.as_str(),
            "authoritative" | "fallback" | "masked_water" | "missing"
        )
    {
        return Err(TerrainDeriveError::InvalidCompilerOutput {
            detail: "manifest elevation state does not match compiler output".to_string(),
        });
    }
    let validation: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output.validation_path)?).map_err(|source| {
            TerrainDeriveError::InvalidCompilerOutput {
                detail: format!("validation JSON: {source}"),
            }
        })?;
    if validation["ok"].as_bool() != Some(true) {
        return Err(TerrainDeriveError::InvalidCompilerOutput {
            detail: "terrain validation did not pass".to_string(),
        });
    }
    Ok(manifest)
}

pub async fn derive_sim_terrain(
    pool: &DbPool,
    data_root: &Path,
    request: &TerrainDeriveRequest,
    compiler: Arc<dyn TerrainCompiler>,
) -> Result<TerrainDeriveOutcome, TerrainDeriveError> {
    let elevation_product_id = request.elevation_product_id.trim();
    let product = catalog::get_product(pool, elevation_product_id)
        .await?
        .ok_or_else(|| TerrainDeriveError::ProductNotFound(elevation_product_id.to_string()))?;
    if product.level != ProductLevel::L1
        || !matches!(product.kind.as_str(), "elevation_dtm" | "elevation_dsm")
        || product.status != "registered"
    {
        return Err(TerrainDeriveError::UnsupportedProduct {
            product_id: product.product_id,
        });
    }
    let source_path = product
        .path
        .as_deref()
        .filter(|path| {
            Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("tif") || extension.eq_ignore_ascii_case("tiff")
                })
        })
        .map(PathBuf::from)
        .ok_or_else(|| TerrainDeriveError::MissingArtifact {
            product_id: product.product_id.clone(),
        })?;
    if !source_path.is_file() {
        return Err(TerrainDeriveError::MissingArtifact {
            product_id: product.product_id.clone(),
        });
    }
    let crs = product.crs.as_deref().unwrap_or("undeclared");
    if !crs.eq_ignore_ascii_case("EPSG:4326") {
        return Err(TerrainDeriveError::UnsupportedCrs(crs.to_string()));
    }
    let coverage = product.bbox.ok_or(TerrainDeriveError::MissingExtent)?;
    let default_aoi = GeoBounds {
        min_lon: coverage[0],
        min_lat: coverage[1],
        max_lon: coverage[2],
        max_lat: coverage[3],
    };
    let aoi = request.aoi.clone().unwrap_or(default_aoi);
    if !valid_aoi(&aoi) {
        return Err(TerrainDeriveError::InvalidAoi(format!("{aoi:?}")));
    }
    if !contains(&coverage, &aoi) {
        return Err(TerrainDeriveError::AoiOutsideCoverage);
    }
    let declared_datum = product.parameters["vertical_datum"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or(TerrainDeriveError::MissingVerticalDatum)?;
    let vertical_datum = canonical_vertical_datum(declared_datum)?;
    if let Some(expected) = request
        .expected_vertical_datum
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let expected = canonical_vertical_datum(expected)?;
        if expected != vertical_datum {
            return Err(TerrainDeriveError::VerticalDatumMismatch {
                product: vertical_datum,
                expected,
            });
        }
    }
    let resolution = request.resolution.unwrap_or(DEFAULT_RESOLUTION);
    if !(2..=MAX_RESOLUTION).contains(&resolution) {
        return Err(TerrainDeriveError::InvalidResolution);
    }
    let target_gsd_m = request
        .target_gsd_m
        .or(product.gsd_m_per_px)
        .unwrap_or(30.0);
    if !target_gsd_m.is_finite() || target_gsd_m <= 0.0 {
        return Err(TerrainDeriveError::InvalidTargetGsd);
    }
    let seed = request.seed.unwrap_or_default();
    let checksum_path = source_path.clone();
    let source_checksum = tokio::task::spawn_blocking(move || checksum_file(&checksum_path))
        .await
        .map_err(|error| TerrainDeriveError::CompilerTask(error.to_string()))??;
    if let Some(expected) = product.checksum_sha256.as_deref() {
        if !expected.eq_ignore_ascii_case(&source_checksum) {
            return Err(TerrainDeriveError::SourceChecksumMismatch {
                expected: expected.to_string(),
                actual: source_checksum,
            });
        }
    }
    let parameters = serde_json::json!({
        "aoi": aoi.clone(),
        "compiler_version": ALGORITHM_VERSION,
        "normalized_dem_file": "terrain-input-f32.tif",
        "nodata_policy": "preserve_explicit",
        "resolution": resolution,
        "resampling": "bilinear",
        "scene_file": "terrain.agbscn",
        "seed": seed,
        "source_checksum_sha256": source_checksum.clone(),
        "surface_model": product.kind.clone(),
        "target_gsd_m": target_gsd_m,
        "validation_file": "terrain.validation.json",
        "vertical_datum": vertical_datum.clone(),
    });
    let spatial_ref = RasterSpatialRef {
        georeferenced: true,
        crs: Some("EPSG:4326".to_string()),
        bbox: Some(aoi.clone()),
        geo_transform: None,
        resolution: None,
    };
    let mut draft = ProductRecordDraft {
        level: ProductLevel::L3,
        kind: PRODUCT_KIND.to_string(),
        algorithm_id: ALGORITHM_ID.to_string(),
        algorithm_version: ALGORITHM_VERSION.to_string(),
        parameters,
        inputs: vec![ProductInputRef {
            product_id: product.product_id.clone(),
            role: "elevation".to_string(),
        }],
        scope: product_scope(&product),
        spatial_ref: Some(spatial_ref),
        gsd_m_per_px: Some(target_gsd_m),
        artifact: None,
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: None,
        evidence_digests: vec![source_checksum.clone()],
        source_id: product.source_id.clone(),
    };
    let terrain_product_id = draft.product_id();
    if let Some(existing) = catalog::get_product(pool, &terrain_product_id).await? {
        let outcome = outcome_from_product(&existing, &product.product_id, true)?;
        verify_existing_package(&existing, &outcome).await?;
        return Ok(outcome);
    }

    let package_dir = data_root
        .join("terrain")
        .join(safe_component(&terrain_product_id));
    std::fs::create_dir_all(&package_dir)?;
    let normalized_path = package_dir.join("terrain-input-f32.tif");
    let normalize_source = source_path.clone();
    let normalize_output = normalized_path.clone();
    tokio::task::spawn_blocking(move || normalize_dem(&normalize_source, &normalize_output))
        .await
        .map_err(|error| TerrainDeriveError::CompilerTask(error.to_string()))??;
    let compile_spec = TerrainCompileSpec {
        dem_path: normalized_path,
        output_dir: package_dir.clone(),
        name: "terrain".to_string(),
        aoi,
        resolution,
        target_gsd_m,
        vertical_datum: vertical_datum.clone(),
        seed,
        source_id: format!("catalog:{}", product.product_id),
        source_version: source_checksum,
    };
    let output = tokio::task::spawn_blocking(move || compiler.compile(&compile_spec))
        .await
        .map_err(|error| TerrainDeriveError::CompilerTask(error.to_string()))??;
    validate_compiler_output(&output, &package_dir, &vertical_datum)?;
    let manifest_checksum = checksum_file(&output.manifest_path)?;
    let scene_checksum = checksum_file(&output.scene_path)?;
    let validation_checksum = checksum_file(&output.validation_path)?;

    draft.artifact = Some(ProductArtifact {
        path: output.manifest_path.to_string_lossy().to_string(),
        format: "agbworld".to_string(),
        checksum_sha256: Some(manifest_checksum.clone()),
    });
    let data_cells = output
        .terrain_cell_count
        .saturating_sub(output.terrain_nodata_cells);
    let confidence = if output.terrain_cell_count == 0 {
        0.0
    } else {
        data_cells as f64 / output.terrain_cell_count as f64
    };
    draft.confidence = Some(confidence);
    draft.confidence_method = Some("authoritative_dem_coverage_fraction".to_string());
    draft.quality_summary = Some(serde_json::json!({
        "elevation_state": output.elevation_state,
        "manifest_checksum_sha256": manifest_checksum.clone(),
        "scene_checksum_sha256": scene_checksum.clone(),
        "validation_checksum_sha256": validation_checksum.clone(),
        "scene_path": output.scene_path,
        "validation_path": output.validation_path,
        "terrain_min_m": output.terrain_min_m,
        "terrain_max_m": output.terrain_max_m,
        "terrain_cell_count": output.terrain_cell_count,
        "terrain_nodata_cells": output.terrain_nodata_cells,
        "vertical_datum": vertical_datum,
        "world_hash": output.world_hash,
    }));
    draft
        .evidence_digests
        .extend([manifest_checksum, scene_checksum, validation_checksum]);

    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let registered_id = catalog::register_product(pool, &draft, &created_at).await?;
    if registered_id != terrain_product_id {
        return Err(TerrainDeriveError::InvalidCompilerOutput {
            detail: "registered product identity changed after compilation".to_string(),
        });
    }
    let registered = catalog::get_product(pool, &registered_id)
        .await?
        .ok_or_else(|| TerrainDeriveError::ProductNotFound(registered_id.clone()))?;

    let mut outcome = outcome_from_product(&registered, &product.product_id, false)?;
    // Runtime evidence lives in the quality summary (outside deterministic
    // identity), so return it directly from the compiler on the creating call.
    outcome.world_hash = output.world_hash;
    outcome.elevation_state = output.elevation_state;
    outcome.terrain_min_m = output.terrain_min_m;
    outcome.terrain_max_m = output.terrain_max_m;
    outcome.terrain_cell_count = output.terrain_cell_count;
    outcome.terrain_nodata_cells = output.terrain_nodata_cells;
    Ok(outcome)
}
