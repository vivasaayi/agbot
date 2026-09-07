//! Provider-neutral elevation raster ingestion.
//!
//! Elevation products enter the same normalized source/scene/product graph as
//! optical and radar imagery. The initial adapter accepts a server-local
//! GeoTIFF/COG, validates that the existing GIS tiler can place it, stages an
//! immutable checksum-addressed copy, then registers:
//!
//! - an L0 `raw_elevation_source` product; and
//! - an L1 `elevation_dsm` or `elevation_dtm` product consuming that L0 node.
//!
//! Raw interferograms are intentionally outside this contract. An InSAR
//! processor can later emit a geocoded DEM into this boundary without making
//! the catalog or viewer understand mission-specific SAR processing.

use crate::db::DbPool;
use crate::ingest_contract::{commit_ingest, IngestError, IngestScene, NormalizedIngest};
use chrono::{DateTime, SecondsFormat, Utc};
use raster_io::GeoTiffReader;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::product_graph::{
    ProductArtifact, ProductInputRef, ProductLevel, ProductRecordDraft, ProductScope,
};
use shared::schemas::RasterSpatialRef;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElevationSurfaceModel {
    Dtm,
    Dsm,
}

impl ElevationSurfaceModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dtm => "dtm",
            Self::Dsm => "dsm",
        }
    }

    fn product_kind(self) -> &'static str {
        match self {
            Self::Dtm => "elevation_dtm",
            Self::Dsm => "elevation_dsm",
        }
    }
}

/// A named source contract. Profiles supply evidence defaults; they do not
/// download provider data or embed provider credentials.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ElevationSourceProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub source_id: &'static str,
    pub source_kind: &'static str,
    pub provider: &'static str,
    pub platform: &'static str,
    pub sensor: &'static str,
    pub default_surface: ElevationSurfaceModel,
    pub nominal_resolution_m: f64,
    pub vertical_datum: &'static str,
    pub dataset_url: &'static str,
    pub license_url: &'static str,
    pub access_notes: &'static str,
}

const ELEVATION_SOURCES: &[ElevationSourceProfile] = &[
    ElevationSourceProfile {
        id: "copernicus_dem_glo30",
        label: "Copernicus DEM GLO-30",
        source_id: "esa:cop-dem-glo-30",
        source_kind: "satellite",
        provider: "European Space Agency",
        platform: "TanDEM-X",
        sensor: "X-band SAR derived DEM",
        default_surface: ElevationSurfaceModel::Dsm,
        nominal_resolution_m: 30.0,
        vertical_datum: "EGM2008",
        dataset_url: "https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM",
        license_url: "https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM",
        access_notes: "Registration as a Copernicus Contributing Missions user may be required for access.",
    },
    ElevationSourceProfile {
        id: "nasadem_hgt",
        label: "NASADEM HGT v001",
        source_id: "nasa:nasadem-hgt-v001",
        source_kind: "satellite",
        provider: "NASA",
        platform: "Space Shuttle Endeavour",
        sensor: "SRTM C-band SAR",
        default_surface: ElevationSurfaceModel::Dsm,
        nominal_resolution_m: 30.0,
        vertical_datum: "EGM96",
        dataset_url: "https://lpdaac.usgs.gov/products/nasadem_hgtv001/",
        license_url: "https://www.earthdata.nasa.gov/engage/open-data-services-software/data-use-policy",
        access_notes: "Free and open NASA data; an Earthdata Login is required for download.",
    },
    ElevationSourceProfile {
        id: "srtm_gl1",
        label: "SRTM GL1 v003",
        source_id: "nasa:srtm-gl1-v003",
        source_kind: "satellite",
        provider: "NASA / USGS",
        platform: "Space Shuttle Endeavour",
        sensor: "SRTM C-band SAR",
        default_surface: ElevationSurfaceModel::Dsm,
        nominal_resolution_m: 30.0,
        vertical_datum: "EGM96",
        dataset_url: "https://lpdaac.usgs.gov/products/srtmgl1v003/",
        license_url: "https://www.earthdata.nasa.gov/engage/open-data-services-software/data-use-policy",
        access_notes: "Free and open NASA data; an Earthdata Login is required for download.",
    },
    ElevationSourceProfile {
        id: "alos_aw3d30",
        label: "ALOS World 3D 30 m",
        source_id: "jaxa:alos-aw3d30",
        source_kind: "satellite",
        provider: "JAXA",
        platform: "ALOS",
        sensor: "PRISM",
        default_surface: ElevationSurfaceModel::Dsm,
        nominal_resolution_m: 30.0,
        vertical_datum: "EGM96",
        dataset_url: "https://www.eorc.jaxa.jp/ALOS/en/dataset/aw3d30/",
        license_url: "https://www.eorc.jaxa.jp/ALOS/en/dataset/aw3d30/",
        access_notes: "No-charge use under the AW3D30 terms; account registration is required for download.",
    },
    ElevationSourceProfile {
        id: "tandem_x_dem",
        label: "TanDEM-X 90 m DEM",
        source_id: "dlr:tandem-x-dem-90",
        source_kind: "satellite",
        provider: "German Aerospace Center (DLR)",
        platform: "TerraSAR-X / TanDEM-X",
        sensor: "X-band SAR interferometry",
        default_surface: ElevationSurfaceModel::Dsm,
        nominal_resolution_m: 90.0,
        vertical_datum: "WGS84 ellipsoid",
        dataset_url: "https://web.geoservice.dlr.de/web/dataguide/tdm90",
        license_url: "https://web.geoservice.dlr.de/web/dataguide/tdm90",
        access_notes: "DLR registration and the TanDEM-X scientific-use license apply.",
    },
    ElevationSourceProfile {
        id: "usgs_3dep",
        label: "USGS 3D Elevation Program 1/3 arc-second DEM",
        source_id: "usgs:3dep-1-3-arc-second",
        source_kind: "field_survey",
        provider: "USGS",
        platform: "3D Elevation Program",
        sensor: "Lidar / IfSAR / source DEM mosaic",
        default_surface: ElevationSurfaceModel::Dtm,
        nominal_resolution_m: 10.0,
        vertical_datum: "NAVD88",
        dataset_url: "https://data.usgs.gov/datacatalog/data/USGS%3A3a81321b-c153-416f-98b7-cc8e5f0e17c3",
        license_url: "https://data.usgs.gov/datacatalog/data/USGS%3A3a81321b-c153-416f-98b7-cc8e5f0e17c3",
        access_notes: "Public domain; geographic coverage and vertical datum vary outside CONUS.",
    },
];

pub fn supported_elevation_sources() -> &'static [ElevationSourceProfile] {
    ELEVATION_SOURCES
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevationIngestRequest {
    pub profile_id: String,
    pub scene_id: String,
    /// Server-local GeoTIFF/COG path supplied by the existing satellite
    /// acquisition pipeline.
    pub artifact_path: String,
    pub acquired_at: String,
    #[serde(default)]
    pub surface_model: Option<ElevationSurfaceModel>,
    #[serde(default)]
    pub vertical_datum: Option<String>,
    #[serde(default)]
    pub checksum_sha256: Option<String>,
    #[serde(default)]
    pub scope: ProductScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevationIngestOutcome {
    pub source_id: String,
    pub scene_id: String,
    pub raw_product_id: String,
    pub elevation_product_id: String,
    pub product_kind: String,
    pub artifact_path: PathBuf,
    pub tile_url_template: String,
}

#[derive(Debug, Error)]
pub enum ElevationIngestError {
    #[error("unknown elevation source profile `{0}`")]
    UnknownProfile(String),
    #[error("invalid elevation ingest request: {0}")]
    InvalidRequest(String),
    #[error("elevation artifact must be a local GeoTIFF (.tif or .tiff): {0}")]
    UnsupportedArtifact(String),
    #[error("elevation source georeferencing is invalid: {0}")]
    InvalidGeoreferencing(String),
    #[error(
        "elevation source CRS EPSG:{0} is not supported by GIS tiles; use EPSG:4326 or WGS84 UTM"
    )]
    UnsupportedCrs(u32),
    #[error("elevation source grid is rotated; GIS tiles require a north-up geotransform")]
    RotatedGrid,
    #[error("elevation artifact checksum mismatch: expected {expected}, found {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("failed to {action} elevation artifact `{path}`: {source}")]
    Io {
        action: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[error("elevation artifact task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
    #[error("failed to serialize elevation {what}: {source}")]
    Serialize {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error(transparent)]
    Ingest(#[from] IngestError),
}

impl ElevationIngestError {
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::UnknownProfile(_)
                | Self::InvalidRequest(_)
                | Self::UnsupportedArtifact(_)
                | Self::InvalidGeoreferencing(_)
                | Self::UnsupportedCrs(_)
                | Self::RotatedGrid
                | Self::ChecksumMismatch { .. }
        )
    }
}

fn required_text(value: &str, name: &str) -> Result<String, ElevationIngestError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ElevationIngestError::InvalidRequest(format!(
            "{name} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn normalize_timestamp(value: &str, name: &str) -> Result<String, ElevationIngestError> {
    DateTime::parse_from_rfc3339(value.trim())
        .map_err(|_| {
            ElevationIngestError::InvalidRequest(format!("{name} must be an RFC3339 timestamp"))
        })
        .map(|timestamp| {
            timestamp
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        })
}

fn safe_component(value: &str) -> String {
    let component: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let component = component.trim_matches(['.', '_']);
    if component.is_empty() {
        "scene".to_string()
    } else {
        component.to_string()
    }
}

fn artifact_is_geotiff(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("tif") || extension.eq_ignore_ascii_case("tiff")
        })
}

fn checksum_file(path: &Path) -> Result<String, ElevationIngestError> {
    let mut file = std::fs::File::open(path).map_err(|source| ElevationIngestError::Io {
        action: "open",
        path: path.display().to_string(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| ElevationIngestError::Io {
                action: "read",
                path: path.display().to_string(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn inspect_georeferencing(
    path: &Path,
) -> Result<(RasterSpatialRef, u32, [f64; 6]), ElevationIngestError> {
    let reader = GeoTiffReader::open(path)
        .map_err(|error| ElevationIngestError::InvalidGeoreferencing(error.to_string()))?;
    let epsg = reader
        .info()
        .epsg
        .ok_or_else(|| ElevationIngestError::InvalidGeoreferencing("missing CRS".to_string()))?;
    let transform = reader.info().geo_transform.ok_or_else(|| {
        ElevationIngestError::InvalidGeoreferencing("missing geotransform".to_string())
    })?;
    let supported =
        epsg == 4326 || (32601..=32660).contains(&epsg) || (32701..=32760).contains(&epsg);
    if !supported {
        return Err(ElevationIngestError::UnsupportedCrs(epsg));
    }
    if transform[2] != 0.0 || transform[4] != 0.0 {
        return Err(ElevationIngestError::RotatedGrid);
    }
    let spatial_ref = reader
        .spatial_ref()
        .map_err(|error| ElevationIngestError::InvalidGeoreferencing(error.to_string()))?;
    Ok((spatial_ref, epsg, transform))
}

fn source_profile(
    profile_id: &str,
) -> Result<&'static ElevationSourceProfile, ElevationIngestError> {
    ELEVATION_SOURCES
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| ElevationIngestError::UnknownProfile(profile_id.to_string()))
}

fn artifact(format_path: &Path, checksum: &str) -> ProductArtifact {
    ProductArtifact {
        path: format_path.to_string_lossy().to_string(),
        format: "tif".to_string(),
        checksum_sha256: Some(checksum.to_string()),
    }
}

/// Validate, stage, and register an elevation GeoTIFF through the normalized
/// catalog path.
pub async fn ingest_elevation(
    pool: &DbPool,
    data_root: &Path,
    request: &ElevationIngestRequest,
) -> Result<ElevationIngestOutcome, ElevationIngestError> {
    let profile_id = required_text(&request.profile_id, "profile_id")?;
    let profile = source_profile(&profile_id)?;
    let scene_id = required_text(&request.scene_id, "scene_id")?;
    let artifact_path = required_text(&request.artifact_path, "artifact_path")?;
    let source_path = PathBuf::from(artifact_path);
    if !artifact_is_geotiff(&source_path) {
        return Err(ElevationIngestError::UnsupportedArtifact(
            source_path.display().to_string(),
        ));
    }
    let source_path =
        std::fs::canonicalize(&source_path).map_err(|source| ElevationIngestError::Io {
            action: "resolve",
            path: source_path.display().to_string(),
            source,
        })?;
    let acquired_at = normalize_timestamp(&request.acquired_at, "acquired_at")?;
    let vertical_datum = request
        .vertical_datum
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(profile.vertical_datum)
        .to_string();
    let surface_model = request.surface_model.unwrap_or(profile.default_surface);

    let inspect_path = source_path.clone();
    let (spatial_ref, epsg, transform) =
        tokio::task::spawn_blocking(move || inspect_georeferencing(&inspect_path)).await??;
    let checksum_path = source_path.clone();
    let source_checksum =
        tokio::task::spawn_blocking(move || checksum_file(&checksum_path)).await??;
    if let Some(expected) = request
        .checksum_sha256
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !expected.eq_ignore_ascii_case(&source_checksum) {
            return Err(ElevationIngestError::ChecksumMismatch {
                expected: expected.to_ascii_lowercase(),
                actual: source_checksum,
            });
        }
    }

    let elevation_dir = data_root
        .join("elevation")
        .join(profile.id)
        .join(safe_component(&scene_id));
    tokio::fs::create_dir_all(&elevation_dir)
        .await
        .map_err(|source| ElevationIngestError::Io {
            action: "create managed directory for",
            path: elevation_dir.display().to_string(),
            source,
        })?;
    let staged_path = elevation_dir.join(format!("{source_checksum}.tif"));
    if source_path != staged_path {
        if tokio::fs::try_exists(&staged_path)
            .await
            .map_err(|source| ElevationIngestError::Io {
                action: "inspect managed",
                path: staged_path.display().to_string(),
                source,
            })?
        {
            let existing_path = staged_path.clone();
            let existing_checksum =
                tokio::task::spawn_blocking(move || checksum_file(&existing_path)).await??;
            if existing_checksum != source_checksum {
                return Err(ElevationIngestError::ChecksumMismatch {
                    expected: source_checksum,
                    actual: existing_checksum,
                });
            }
        } else {
            tokio::fs::copy(&source_path, &staged_path)
                .await
                .map_err(|source| ElevationIngestError::Io {
                    action: "stage",
                    path: staged_path.display().to_string(),
                    source,
                })?;
            let verify_path = staged_path.clone();
            let staged_checksum =
                tokio::task::spawn_blocking(move || checksum_file(&verify_path)).await??;
            if staged_checksum != source_checksum {
                return Err(ElevationIngestError::ChecksumMismatch {
                    expected: source_checksum,
                    actual: staged_checksum,
                });
            }
        }
    }

    let mut scope = request.scope.clone();
    scope.scene_id = Some(scene_id.clone());
    scope.temporal_start = if scope.temporal_start.trim().is_empty() {
        acquired_at.clone()
    } else {
        normalize_timestamp(&scope.temporal_start, "scope.temporal_start")?
    };
    scope.temporal_end = if scope.temporal_end.trim().is_empty() {
        acquired_at.clone()
    } else {
        normalize_timestamp(&scope.temporal_end, "scope.temporal_end")?
    };
    if scope.temporal_start > scope.temporal_end {
        return Err(ElevationIngestError::InvalidRequest(
            "scope.temporal_start must not be after scope.temporal_end".to_string(),
        ));
    }
    let gsd_m_per_px = if epsg == 4326 {
        profile.nominal_resolution_m
    } else {
        transform[1].abs().max(transform[5].abs())
    };

    let raw_draft = ProductRecordDraft {
        level: ProductLevel::L0,
        kind: "raw_elevation_source".to_string(),
        algorithm_id: "elevation.source_ingest".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "profile_id": profile.id,
            "scene_id": scene_id,
            "checksum_sha256": source_checksum,
        }),
        inputs: Vec::new(),
        scope: scope.clone(),
        spatial_ref: Some(spatial_ref.clone()),
        gsd_m_per_px: Some(gsd_m_per_px),
        artifact: Some(artifact(&staged_path, &source_checksum)),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: Some(serde_json::json!({
            "georeferencing_validated": true,
            "vertical_datum": vertical_datum,
            "units": "m",
        })),
        evidence_digests: vec![source_checksum.clone()],
        source_id: Some(profile.source_id.to_string()),
    };
    let raw_product_id = raw_draft.product_id();
    let product_kind = surface_model.product_kind().to_string();
    let elevation_draft = ProductRecordDraft {
        level: ProductLevel::L1,
        kind: product_kind.clone(),
        algorithm_id: "elevation.normalized_surface".to_string(),
        algorithm_version: "1.0.0".to_string(),
        parameters: serde_json::json!({
            "profile_id": profile.id,
            "surface_model": surface_model.as_str(),
            "vertical_datum": vertical_datum,
            "units": "m",
            "source_checksum_sha256": source_checksum,
        }),
        inputs: vec![ProductInputRef {
            product_id: raw_product_id.clone(),
            role: "raw_elevation_source".to_string(),
        }],
        scope,
        spatial_ref: Some(spatial_ref),
        gsd_m_per_px: Some(gsd_m_per_px),
        artifact: Some(artifact(&staged_path, &source_checksum)),
        quality_mask: None,
        confidence: None,
        confidence_method: None,
        quality_summary: Some(serde_json::json!({
            "georeferencing_validated": true,
            "vertical_datum": vertical_datum,
            "units": "m",
            "surface_model": surface_model.as_str(),
        })),
        evidence_digests: vec![source_checksum.clone()],
        source_id: Some(profile.source_id.to_string()),
    };
    let elevation_product_id = elevation_draft.product_id();
    let source_config = serde_json::json!({
        "profile_id": profile.id,
        "label": profile.label,
        "provider": profile.provider,
        "dataset_url": profile.dataset_url,
        "license_url": profile.license_url,
        "access_notes": profile.access_notes,
        "nominal_resolution_m": profile.nominal_resolution_m,
        "default_surface": profile.default_surface.as_str(),
        "vertical_datum": profile.vertical_datum,
    });
    let metadata_json = serde_json::to_string(&serde_json::json!({
        "profile": source_config,
        "surface_model": surface_model.as_str(),
        "vertical_datum": vertical_datum,
        "checksum_sha256": source_checksum,
        "spatial_ref": elevation_draft.spatial_ref,
    }))
    .map_err(|source| ElevationIngestError::Serialize {
        what: "scene metadata",
        source,
    })?;
    let normalized = NormalizedIngest {
        source_id: profile.source_id.to_string(),
        source_kind: profile.source_kind.to_string(),
        platform: Some(profile.platform.to_string()),
        sensor: Some(profile.sensor.to_string()),
        source_config: Some(source_config),
        scene: Some(IngestScene {
            scene_id: scene_id.clone(),
            owner: None,
            sensor: profile.sensor.to_string(),
            acquired_at,
            data_path: staged_path.to_string_lossy().to_string(),
            metadata_json,
            cloud_cover: None,
        }),
        l0_products: vec![raw_draft],
        l1_products: vec![elevation_draft],
        quality: Some(serde_json::json!({
            "georeferencing_validated": true,
            "checksum_sha256": source_checksum,
        })),
    };
    let actor = provenance::ActorIdentity::system("geo_hub:elevation-ingest");
    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let receipt = commit_ingest(pool, &normalized, &actor, &created_at).await?;
    let raw_product_id = receipt
        .product_ids
        .first()
        .cloned()
        .unwrap_or(raw_product_id);
    let elevation_product_id = receipt
        .product_ids
        .get(1)
        .cloned()
        .unwrap_or(elevation_product_id);

    Ok(ElevationIngestOutcome {
        source_id: receipt.source_id,
        scene_id,
        raw_product_id,
        elevation_product_id: elevation_product_id.clone(),
        product_kind,
        artifact_path: staged_path,
        tile_url_template: format!(
            "/api/catalog/products/{elevation_product_id}/tiles/{{z}}/{{x}}/{{y}}.png"
        ),
    })
}
