use serde::{Deserialize, Serialize};
use shared::schemas::{
    assert_raster_spatial_ref, GeoBounds, RasterSpatialRef, RasterSpatialRefError,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropModelTask {
    StandCount,
    CanopyCover,
    DiseaseDetection,
    PestDetection,
    WeedMapping,
}

impl CropModelTask {
    pub fn as_str(self) -> &'static str {
        match self {
            CropModelTask::StandCount => "stand_count",
            CropModelTask::CanopyCover => "canopy_cover",
            CropModelTask::DiseaseDetection => "disease_detection",
            CropModelTask::PestDetection => "pest_detection",
            CropModelTask::WeedMapping => "weed_mapping",
        }
    }
}

impl std::str::FromStr for CropModelTask {
    type Err = CropModelRegistryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "stand_count" => Ok(CropModelTask::StandCount),
            "canopy_cover" => Ok(CropModelTask::CanopyCover),
            "disease_detection" => Ok(CropModelTask::DiseaseDetection),
            "pest_detection" => Ok(CropModelTask::PestDetection),
            "weed_mapping" => Ok(CropModelTask::WeedMapping),
            _ => Err(CropModelRegistryError::UnsupportedTask {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelVersionRegistrationRequest {
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub version: String,
    pub task: CropModelTask,
    #[serde(default)]
    pub training_set_ref: String,
    #[serde(default = "default_model_metrics")]
    pub metrics: serde_json::Value,
    #[serde(default)]
    pub provenance_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelVersionRecord {
    pub model_id: String,
    pub version: String,
    pub task: CropModelTask,
    pub training_set_ref: String,
    pub metrics: serde_json::Value,
    pub provenance_ref: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InferenceModelReference {
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelGateResponse {
    pub model_id: String,
    pub version: String,
    pub registered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlantCountConfig {
    pub min_component_pixels: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantCountTile {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub valid: bool,
    pub width_px: u32,
    pub height_px: u32,
    pub min_x_m: f64,
    pub min_y_m: f64,
    pub max_x_m: f64,
    pub max_y_m: f64,
    pub crop_mask: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantCountZeroReason {
    InvalidTile,
    NoValidCropPixels,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantLocation {
    pub plant_id: String,
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub crs: String,
    pub x_m: f64,
    pub y_m: f64,
    pub pixel_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilePlantCount {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub plant_count: usize,
    pub tile_area_m2: f64,
    pub density_plants_per_ha: f64,
    #[serde(default)]
    pub zero_reason: Option<PlantCountZeroReason>,
    pub plant_locations: Vec<PlantLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZonePlantCount {
    pub zone_id: String,
    pub plant_count: usize,
    pub area_m2: f64,
    pub density_plants_per_ha: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandCountReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub total_count: usize,
    pub field_area_m2: f64,
    pub field_density_plants_per_ha: f64,
    pub tiles: Vec<TilePlantCount>,
    pub zones: Vec<ZonePlantCount>,
    pub plant_locations: Vec<PlantLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanopyCoverConfig {
    pub vegetation_index_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanopyCoverTile {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub width_px: u32,
    pub height_px: u32,
    pub spatial_ref: RasterSpatialRef,
    pub index_values: Vec<f64>,
    pub valid_mask: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanopyCoverMask {
    pub width_px: u32,
    pub height_px: u32,
    pub vegetation_mask: Vec<bool>,
    pub valid_mask: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileCanopyCover {
    pub tile_id: String,
    #[serde(default)]
    pub zone_id: Option<String>,
    pub spatial_ref: RasterSpatialRef,
    pub total_pixels: usize,
    pub valid_pixels: usize,
    pub vegetation_pixels: usize,
    pub excluded_pixels: usize,
    pub pixel_area_m2: f64,
    pub valid_area_m2: f64,
    pub excluded_area_m2: f64,
    pub cover_fraction: f64,
    pub mask: CanopyCoverMask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneCanopyCover {
    pub zone_id: String,
    pub valid_pixels: usize,
    pub vegetation_pixels: usize,
    pub excluded_pixels: usize,
    pub valid_area_m2: f64,
    pub excluded_area_m2: f64,
    pub cover_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanopyCoverReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub valid_pixels: usize,
    pub vegetation_pixels: usize,
    pub excluded_pixels: usize,
    pub valid_area_m2: f64,
    pub excluded_area_m2: f64,
    pub cover_fraction: f64,
    pub tiles: Vec<TileCanopyCover>,
    pub zones: Vec<ZoneCanopyCover>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DiseaseDetectionConfig {
    pub low_confidence_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiseaseLesionCandidate {
    pub tile_id: String,
    pub confidence: f64,
    pub bbox: GeoBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectionZoneGeometry {
    pub crs: String,
    pub bbox: GeoBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiseaseLesionDetection {
    pub detection_id: String,
    pub confidence: f64,
    pub low_confidence: bool,
    pub evidence_tile_ref: String,
    pub zone_geometry: DetectionZoneGeometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiseaseDetectionReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub model: ModelGateResponse,
    pub deterministic_cover_valid_pixels: usize,
    pub low_confidence_count: usize,
    pub detections: Vec<DiseaseLesionDetection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WeedMappingConfig {
    pub low_confidence_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeedZoneCandidate {
    pub tile_id: String,
    pub confidence: f64,
    pub bbox: GeoBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeedMapZone {
    pub zone_id: String,
    pub confidence: f64,
    pub low_confidence: bool,
    pub evidence_tile_ref: String,
    pub area_m2: f64,
    pub geometry: DetectionZoneGeometry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeedMapReport {
    pub field_id: String,
    pub crs: String,
    pub generated_at: String,
    pub model: ModelGateResponse,
    pub deterministic_cover_valid_pixels: usize,
    pub total_weed_area_m2: f64,
    pub low_confidence_count: usize,
    pub zones: Vec<WeedMapZone>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CropModelRegistryError {
    #[error("model_id cannot be empty")]
    EmptyModelId,
    #[error("model version cannot be empty")]
    EmptyVersion,
    #[error("training_set_ref cannot be empty")]
    EmptyTrainingSetRef,
    #[error("provenance_ref cannot be empty")]
    EmptyProvenanceRef,
    #[error("created_at cannot be empty")]
    EmptyCreatedAt,
    #[error("metrics must be a non-empty JSON object")]
    InvalidMetrics,
    #[error("unsupported crop model task {value}")]
    UnsupportedTask { value: String },
    #[error("unregistered model {model_id}@{version}")]
    UnregisteredModel { model_id: String, version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StandCountError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("CRS cannot be empty")]
    EmptyCrs,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("stand count requires at least one tile")]
    EmptyTiles,
    #[error("min_component_pixels must be greater than zero")]
    InvalidConfig,
    #[error("tile {tile_id} has invalid geometry")]
    InvalidTileGeometry { tile_id: String },
    #[error("tile {tile_id} crop mask length does not match dimensions")]
    CropMaskSizeMismatch { tile_id: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CanopyCoverError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("canopy cover requires at least one tile")]
    EmptyTiles,
    #[error("vegetation_index_threshold must be finite")]
    InvalidThreshold,
    #[error("tile_id cannot be empty")]
    EmptyTileId,
    #[error("tile {tile_id} has invalid spatial reference: {source}")]
    SpatialRefInvalid {
        tile_id: String,
        #[source]
        source: RasterSpatialRefError,
    },
    #[error("tile {tile_id} index values length does not match dimensions")]
    IndexSizeMismatch { tile_id: String },
    #[error("tile {tile_id} valid mask length does not match dimensions")]
    ValidMaskSizeMismatch { tile_id: String },
    #[error("tile {tile_id} CRS {actual} does not match field CRS {expected}")]
    CrsMismatch {
        tile_id: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DiseaseDetectionError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("deterministic canopy cover is required before disease detection")]
    DeterministicCoverRequired,
    #[error("disease model gate failed: {source}")]
    ModelGate {
        #[source]
        source: CropModelRegistryError,
    },
    #[error("low_confidence_threshold must be finite and between 0 and 1")]
    InvalidThreshold,
    #[error("tile_id cannot be empty")]
    EmptyTileId,
    #[error("candidate on tile {tile_id} has invalid confidence")]
    InvalidConfidence { tile_id: String },
    #[error("candidate on tile {tile_id} has invalid zone geometry")]
    InvalidZoneGeometry { tile_id: String },
    #[error("candidate references tile {tile_id} without deterministic cover evidence")]
    MissingCoverTile { tile_id: String },
    #[error("cover report field {actual} does not match requested field {expected}")]
    CoverFieldMismatch { expected: String, actual: String },
    #[error("candidate on tile {tile_id} falls outside the deterministic cover tile extent")]
    ZoneOutsideTileExtent { tile_id: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WeedMappingError {
    #[error("field_id cannot be empty")]
    EmptyFieldId,
    #[error("generated_at cannot be empty")]
    EmptyGeneratedAt,
    #[error("deterministic canopy cover is required before weed mapping")]
    DeterministicCoverRequired,
    #[error("weed model gate failed: {source}")]
    ModelGate {
        #[source]
        source: CropModelRegistryError,
    },
    #[error("low_confidence_threshold must be finite and between 0 and 1")]
    InvalidThreshold,
    #[error("tile_id cannot be empty")]
    EmptyTileId,
    #[error("candidate on tile {tile_id} has invalid confidence")]
    InvalidConfidence { tile_id: String },
    #[error("candidate on tile {tile_id} has invalid zone geometry")]
    InvalidZoneGeometry { tile_id: String },
    #[error("candidate references tile {tile_id} without deterministic cover evidence")]
    MissingCoverTile { tile_id: String },
    #[error("cover report field {actual} does not match requested field {expected}")]
    CoverFieldMismatch { expected: String, actual: String },
    #[error("candidate on tile {tile_id} falls outside the deterministic cover tile extent")]
    ZoneOutsideTileExtent { tile_id: String },
}

pub fn build_model_version_record(
    request: ModelVersionRegistrationRequest,
    created_at: String,
) -> Result<ModelVersionRecord, CropModelRegistryError> {
    let model_id = normalize_required_text(request.model_id, CropModelRegistryError::EmptyModelId)?;
    let version = normalize_required_text(request.version, CropModelRegistryError::EmptyVersion)?;
    let training_set_ref = normalize_required_text(
        request.training_set_ref,
        CropModelRegistryError::EmptyTrainingSetRef,
    )?;
    let provenance_ref = normalize_required_text(
        request.provenance_ref,
        CropModelRegistryError::EmptyProvenanceRef,
    )?;
    let created_at = normalize_required_text(created_at, CropModelRegistryError::EmptyCreatedAt)?;
    validate_metrics(&request.metrics)?;

    Ok(ModelVersionRecord {
        model_id,
        version,
        task: request.task,
        training_set_ref,
        metrics: request.metrics,
        provenance_ref,
        created_at,
    })
}

pub fn validate_model_reference(
    reference: InferenceModelReference,
    registered: bool,
) -> Result<ModelGateResponse, CropModelRegistryError> {
    let model_id =
        normalize_required_text(reference.model_id, CropModelRegistryError::EmptyModelId)?;
    let version = normalize_required_text(reference.version, CropModelRegistryError::EmptyVersion)?;

    if !registered {
        return Err(CropModelRegistryError::UnregisteredModel { model_id, version });
    }

    Ok(ModelGateResponse {
        model_id,
        version,
        registered,
    })
}

pub fn run_stand_count(
    field_id: String,
    crs: String,
    tiles: Vec<PlantCountTile>,
    config: PlantCountConfig,
    generated_at: String,
) -> Result<StandCountReport, StandCountError> {
    let field_id = normalize_stand_text(field_id, StandCountError::EmptyFieldId)?;
    let crs = normalize_stand_text(crs, StandCountError::EmptyCrs)?;
    let generated_at = normalize_stand_text(generated_at, StandCountError::EmptyGeneratedAt)?;
    if tiles.is_empty() {
        return Err(StandCountError::EmptyTiles);
    }
    if config.min_component_pixels == 0 {
        return Err(StandCountError::InvalidConfig);
    }

    let mut tile_counts = Vec::new();
    let mut plant_locations = Vec::new();
    let mut zone_rollups: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    let mut field_area_m2 = 0.0;

    for tile in tiles {
        let tile_count = count_tile_plants(tile, &crs, config)?;
        field_area_m2 += tile_count.tile_area_m2;
        if let Some(zone_id) = tile_count.zone_id.as_ref() {
            let entry = zone_rollups.entry(zone_id.clone()).or_insert((0, 0.0));
            entry.0 += tile_count.plant_count;
            entry.1 += tile_count.tile_area_m2;
        }
        plant_locations.extend(tile_count.plant_locations.clone());
        tile_counts.push(tile_count);
    }
    tile_counts.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    plant_locations.sort_by(|left, right| left.plant_id.cmp(&right.plant_id));

    let total_count = tile_counts.iter().map(|tile| tile.plant_count).sum();
    let zones = zone_rollups
        .into_iter()
        .map(|(zone_id, (plant_count, area_m2))| ZonePlantCount {
            zone_id,
            plant_count,
            area_m2,
            density_plants_per_ha: density_per_ha(plant_count, area_m2),
        })
        .collect();

    Ok(StandCountReport {
        field_id,
        crs,
        generated_at,
        total_count,
        field_area_m2,
        field_density_plants_per_ha: density_per_ha(total_count, field_area_m2),
        tiles: tile_counts,
        zones,
        plant_locations,
    })
}

pub fn run_canopy_cover(
    field_id: String,
    tiles: Vec<CanopyCoverTile>,
    config: CanopyCoverConfig,
    generated_at: String,
) -> Result<CanopyCoverReport, CanopyCoverError> {
    let field_id = normalize_canopy_text(field_id, CanopyCoverError::EmptyFieldId)?;
    let generated_at = normalize_canopy_text(generated_at, CanopyCoverError::EmptyGeneratedAt)?;
    if tiles.is_empty() {
        return Err(CanopyCoverError::EmptyTiles);
    }
    if !config.vegetation_index_threshold.is_finite() {
        return Err(CanopyCoverError::InvalidThreshold);
    }

    let mut field_crs: Option<String> = None;
    let mut tile_reports = Vec::new();
    let mut zone_rollups: BTreeMap<String, CanopyZoneAccumulator> = BTreeMap::new();

    for tile in tiles {
        let tile_report = evaluate_canopy_tile(tile, config)?;
        let tile_crs = tile_report.spatial_ref.crs.clone().unwrap_or_default();
        match field_crs.as_ref() {
            Some(expected) if expected != &tile_crs => {
                return Err(CanopyCoverError::CrsMismatch {
                    tile_id: tile_report.tile_id,
                    expected: expected.clone(),
                    actual: tile_crs,
                });
            }
            None => field_crs = Some(tile_crs),
            _ => {}
        }
        if let Some(zone_id) = tile_report.zone_id.as_ref() {
            let entry = zone_rollups.entry(zone_id.clone()).or_default();
            entry.valid_pixels += tile_report.valid_pixels;
            entry.vegetation_pixels += tile_report.vegetation_pixels;
            entry.excluded_pixels += tile_report.excluded_pixels;
            entry.valid_area_m2 += tile_report.valid_area_m2;
            entry.excluded_area_m2 += tile_report.excluded_area_m2;
        }
        tile_reports.push(tile_report);
    }

    tile_reports.sort_by(|left, right| left.tile_id.cmp(&right.tile_id));
    let valid_pixels = tile_reports.iter().map(|tile| tile.valid_pixels).sum();
    let vegetation_pixels = tile_reports.iter().map(|tile| tile.vegetation_pixels).sum();
    let excluded_pixels = tile_reports.iter().map(|tile| tile.excluded_pixels).sum();
    let valid_area_m2 = tile_reports.iter().map(|tile| tile.valid_area_m2).sum();
    let excluded_area_m2 = tile_reports.iter().map(|tile| tile.excluded_area_m2).sum();
    let zones = zone_rollups
        .into_iter()
        .map(|(zone_id, rollup)| ZoneCanopyCover {
            zone_id,
            valid_pixels: rollup.valid_pixels,
            vegetation_pixels: rollup.vegetation_pixels,
            excluded_pixels: rollup.excluded_pixels,
            valid_area_m2: rollup.valid_area_m2,
            excluded_area_m2: rollup.excluded_area_m2,
            cover_fraction: cover_fraction(rollup.vegetation_pixels, rollup.valid_pixels),
        })
        .collect();

    Ok(CanopyCoverReport {
        field_id,
        crs: field_crs.unwrap_or_default(),
        generated_at,
        valid_pixels,
        vegetation_pixels,
        excluded_pixels,
        valid_area_m2,
        excluded_area_m2,
        cover_fraction: cover_fraction(vegetation_pixels, valid_pixels),
        tiles: tile_reports,
        zones,
    })
}

pub fn run_disease_lesion_detection(
    field_id: String,
    model: InferenceModelReference,
    model_registered: bool,
    deterministic_cover: Option<&CanopyCoverReport>,
    candidates: Vec<DiseaseLesionCandidate>,
    config: DiseaseDetectionConfig,
    generated_at: String,
) -> Result<DiseaseDetectionReport, DiseaseDetectionError> {
    let field_id = normalize_disease_text(field_id, DiseaseDetectionError::EmptyFieldId)?;
    let generated_at =
        normalize_disease_text(generated_at, DiseaseDetectionError::EmptyGeneratedAt)?;
    if !is_unit_fraction(config.low_confidence_threshold) {
        return Err(DiseaseDetectionError::InvalidThreshold);
    }
    let model = validate_model_reference(model, model_registered)
        .map_err(|source| DiseaseDetectionError::ModelGate { source })?;
    let cover = deterministic_cover.ok_or(DiseaseDetectionError::DeterministicCoverRequired)?;
    if cover.field_id != field_id {
        return Err(DiseaseDetectionError::CoverFieldMismatch {
            expected: field_id,
            actual: cover.field_id.clone(),
        });
    }

    let cover_tiles = cover
        .tiles
        .iter()
        .map(|tile| (tile.tile_id.as_str(), tile))
        .collect::<BTreeMap<_, _>>();
    let mut detections = Vec::new();
    let mut tile_detection_counts: BTreeMap<String, usize> = BTreeMap::new();

    for candidate in candidates {
        let tile_id = normalize_disease_text(
            candidate.tile_id.clone(),
            DiseaseDetectionError::EmptyTileId,
        )?;
        if !is_unit_fraction(candidate.confidence) {
            return Err(DiseaseDetectionError::InvalidConfidence { tile_id });
        }
        let cover_tile = cover_tiles.get(tile_id.as_str()).ok_or_else(|| {
            DiseaseDetectionError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        let tile_bbox = cover_tile.spatial_ref.bbox.as_ref().ok_or_else(|| {
            DiseaseDetectionError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        if !valid_bbox(&candidate.bbox) {
            return Err(DiseaseDetectionError::InvalidZoneGeometry { tile_id });
        }
        if !bbox_within(&candidate.bbox, tile_bbox) {
            return Err(DiseaseDetectionError::ZoneOutsideTileExtent { tile_id });
        }

        let sequence = tile_detection_counts.entry(tile_id.clone()).or_insert(0);
        *sequence += 1;
        detections.push(DiseaseLesionDetection {
            detection_id: format!("disease:{tile_id}:{}", *sequence),
            confidence: candidate.confidence,
            low_confidence: candidate.confidence < config.low_confidence_threshold,
            evidence_tile_ref: tile_id,
            zone_geometry: DetectionZoneGeometry {
                crs: cover.crs.clone(),
                bbox: candidate.bbox,
            },
        });
    }

    detections.sort_by(|left, right| left.detection_id.cmp(&right.detection_id));
    let low_confidence_count = detections
        .iter()
        .filter(|detection| detection.low_confidence)
        .count();

    Ok(DiseaseDetectionReport {
        field_id,
        crs: cover.crs.clone(),
        generated_at,
        model,
        deterministic_cover_valid_pixels: cover.valid_pixels,
        low_confidence_count,
        detections,
    })
}

pub fn run_weed_mapping(
    field_id: String,
    model: InferenceModelReference,
    model_registered: bool,
    deterministic_cover: Option<&CanopyCoverReport>,
    candidates: Vec<WeedZoneCandidate>,
    config: WeedMappingConfig,
    generated_at: String,
) -> Result<WeedMapReport, WeedMappingError> {
    let field_id = normalize_weed_text(field_id, WeedMappingError::EmptyFieldId)?;
    let generated_at = normalize_weed_text(generated_at, WeedMappingError::EmptyGeneratedAt)?;
    if !is_unit_fraction(config.low_confidence_threshold) {
        return Err(WeedMappingError::InvalidThreshold);
    }
    let model = validate_model_reference(model, model_registered)
        .map_err(|source| WeedMappingError::ModelGate { source })?;
    let cover = deterministic_cover.ok_or(WeedMappingError::DeterministicCoverRequired)?;
    if cover.field_id != field_id {
        return Err(WeedMappingError::CoverFieldMismatch {
            expected: field_id,
            actual: cover.field_id.clone(),
        });
    }

    let cover_tiles = cover
        .tiles
        .iter()
        .map(|tile| (tile.tile_id.as_str(), tile))
        .collect::<BTreeMap<_, _>>();
    let mut zones = Vec::new();
    let mut tile_zone_counts: BTreeMap<String, usize> = BTreeMap::new();

    for candidate in candidates {
        let tile_id =
            normalize_weed_text(candidate.tile_id.clone(), WeedMappingError::EmptyTileId)?;
        if !is_unit_fraction(candidate.confidence) {
            return Err(WeedMappingError::InvalidConfidence { tile_id });
        }
        let cover_tile = cover_tiles.get(tile_id.as_str()).ok_or_else(|| {
            WeedMappingError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        let tile_bbox = cover_tile.spatial_ref.bbox.as_ref().ok_or_else(|| {
            WeedMappingError::MissingCoverTile {
                tile_id: tile_id.clone(),
            }
        })?;
        if !valid_bbox(&candidate.bbox) {
            return Err(WeedMappingError::InvalidZoneGeometry { tile_id });
        }
        if !bbox_within(&candidate.bbox, tile_bbox) {
            return Err(WeedMappingError::ZoneOutsideTileExtent { tile_id });
        }

        let sequence = tile_zone_counts.entry(tile_id.clone()).or_insert(0);
        *sequence += 1;
        zones.push(WeedMapZone {
            zone_id: format!("weed:{tile_id}:{}", *sequence),
            confidence: candidate.confidence,
            low_confidence: candidate.confidence < config.low_confidence_threshold,
            evidence_tile_ref: tile_id,
            area_m2: bbox_area_m2(&candidate.bbox),
            geometry: DetectionZoneGeometry {
                crs: cover.crs.clone(),
                bbox: candidate.bbox,
            },
        });
    }

    zones.sort_by(|left, right| left.zone_id.cmp(&right.zone_id));
    let total_weed_area_m2 = zones.iter().map(|zone| zone.area_m2).sum();
    let low_confidence_count = zones.iter().filter(|zone| zone.low_confidence).count();

    Ok(WeedMapReport {
        field_id,
        crs: cover.crs.clone(),
        generated_at,
        model,
        deterministic_cover_valid_pixels: cover.valid_pixels,
        total_weed_area_m2,
        low_confidence_count,
        zones,
    })
}

fn normalize_required_text(
    value: String,
    error: CropModelRegistryError,
) -> Result<String, CropModelRegistryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_metrics(metrics: &serde_json::Value) -> Result<(), CropModelRegistryError> {
    match metrics.as_object() {
        Some(metrics) if !metrics.is_empty() => Ok(()),
        _ => Err(CropModelRegistryError::InvalidMetrics),
    }
}

fn default_model_metrics() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Default)]
struct CanopyZoneAccumulator {
    valid_pixels: usize,
    vegetation_pixels: usize,
    excluded_pixels: usize,
    valid_area_m2: f64,
    excluded_area_m2: f64,
}

fn evaluate_canopy_tile(
    tile: CanopyCoverTile,
    config: CanopyCoverConfig,
) -> Result<TileCanopyCover, CanopyCoverError> {
    let tile_id = normalize_canopy_text(tile.tile_id.clone(), CanopyCoverError::EmptyTileId)?;
    let zone_id = tile
        .zone_id
        .as_ref()
        .and_then(|zone_id| normalize_optional_stand_text(zone_id.clone()));
    let spatial_ref =
        assert_raster_spatial_ref(Some(&tile.spatial_ref), tile.width_px, tile.height_px).map_err(
            |source| CanopyCoverError::SpatialRefInvalid {
                tile_id: tile_id.clone(),
                source,
            },
        )?;
    let pixel_count = tile.width_px as usize * tile.height_px as usize;
    if tile.index_values.len() != pixel_count {
        return Err(CanopyCoverError::IndexSizeMismatch { tile_id });
    }
    if tile.valid_mask.len() != pixel_count {
        return Err(CanopyCoverError::ValidMaskSizeMismatch { tile_id });
    }

    let pixel_area_m2 = spatial_ref
        .resolution
        .map(|resolution| resolution.x * resolution.y)
        .unwrap_or(0.0);
    let mut valid_mask = Vec::with_capacity(pixel_count);
    let mut vegetation_mask = Vec::with_capacity(pixel_count);
    let mut valid_pixels = 0;
    let mut vegetation_pixels = 0;

    for (index_value, qa_valid) in tile.index_values.iter().zip(tile.valid_mask.iter()) {
        let valid = *qa_valid && index_value.is_finite();
        let vegetation = valid && *index_value >= config.vegetation_index_threshold;
        valid_mask.push(valid);
        vegetation_mask.push(vegetation);
        if valid {
            valid_pixels += 1;
        }
        if vegetation {
            vegetation_pixels += 1;
        }
    }

    let excluded_pixels = pixel_count - valid_pixels;
    let valid_area_m2 = valid_pixels as f64 * pixel_area_m2;
    let excluded_area_m2 = excluded_pixels as f64 * pixel_area_m2;

    Ok(TileCanopyCover {
        tile_id,
        zone_id,
        spatial_ref,
        total_pixels: pixel_count,
        valid_pixels,
        vegetation_pixels,
        excluded_pixels,
        pixel_area_m2,
        valid_area_m2,
        excluded_area_m2,
        cover_fraction: cover_fraction(vegetation_pixels, valid_pixels),
        mask: CanopyCoverMask {
            width_px: tile.width_px,
            height_px: tile.height_px,
            vegetation_mask,
            valid_mask,
        },
    })
}

fn count_tile_plants(
    tile: PlantCountTile,
    crs: &str,
    config: PlantCountConfig,
) -> Result<TilePlantCount, StandCountError> {
    let tile_id = normalize_stand_text(
        tile.tile_id.clone(),
        StandCountError::InvalidTileGeometry {
            tile_id: String::new(),
        },
    )?;
    let zone_id = tile
        .zone_id
        .as_ref()
        .and_then(|zone_id| normalize_optional_stand_text(zone_id.clone()));
    validate_tile_geometry(&tile_id, &tile)?;
    let tile_area_m2 = (tile.max_x_m - tile.min_x_m) * (tile.max_y_m - tile.min_y_m);

    if !tile.valid {
        return Ok(TilePlantCount {
            tile_id,
            zone_id,
            plant_count: 0,
            tile_area_m2,
            density_plants_per_ha: 0.0,
            zero_reason: Some(PlantCountZeroReason::InvalidTile),
            plant_locations: Vec::new(),
        });
    }

    let plant_locations = connected_crop_components(&tile_id, zone_id.clone(), crs, &tile, config);
    let plant_count = plant_locations.len();
    let zero_reason = (plant_count == 0).then_some(PlantCountZeroReason::NoValidCropPixels);

    Ok(TilePlantCount {
        tile_id,
        zone_id,
        plant_count,
        tile_area_m2,
        density_plants_per_ha: density_per_ha(plant_count, tile_area_m2),
        zero_reason,
        plant_locations,
    })
}

fn connected_crop_components(
    tile_id: &str,
    zone_id: Option<String>,
    crs: &str,
    tile: &PlantCountTile,
    config: PlantCountConfig,
) -> Vec<PlantLocation> {
    let width = tile.width_px as usize;
    let height = tile.height_px as usize;
    let mut visited = vec![false; tile.crop_mask.len()];
    let mut plants = Vec::new();

    for index in 0..tile.crop_mask.len() {
        if visited[index] || !tile.crop_mask[index] {
            continue;
        }
        let component = flood_fill_component(index, width, height, &tile.crop_mask, &mut visited);
        if component.len() < config.min_component_pixels {
            continue;
        }
        let (x_m, y_m) = component_centroid(&component, width, tile);
        plants.push(PlantLocation {
            plant_id: format!("plant:{tile_id}:{}", plants.len() + 1),
            tile_id: tile_id.to_string(),
            zone_id: zone_id.clone(),
            crs: crs.to_string(),
            x_m,
            y_m,
            pixel_count: component.len(),
        });
    }

    plants
}

fn flood_fill_component(
    start: usize,
    width: usize,
    height: usize,
    crop_mask: &[bool],
    visited: &mut [bool],
) -> Vec<usize> {
    let mut component = Vec::new();
    let mut stack = vec![start];
    while let Some(index) = stack.pop() {
        if visited[index] || !crop_mask[index] {
            continue;
        }
        visited[index] = true;
        component.push(index);
        let row = index / width;
        let col = index % width;
        if col > 0 {
            stack.push(index - 1);
        }
        if col + 1 < width {
            stack.push(index + 1);
        }
        if row > 0 {
            stack.push(index - width);
        }
        if row + 1 < height {
            stack.push(index + width);
        }
    }
    component
}

fn component_centroid(component: &[usize], width: usize, tile: &PlantCountTile) -> (f64, f64) {
    let pixel_width_m = (tile.max_x_m - tile.min_x_m) / tile.width_px as f64;
    let pixel_height_m = (tile.max_y_m - tile.min_y_m) / tile.height_px as f64;
    let mut x_sum = 0.0;
    let mut y_sum = 0.0;
    for index in component {
        let row = index / width;
        let col = index % width;
        x_sum += tile.min_x_m + (col as f64 + 0.5) * pixel_width_m;
        y_sum += tile.max_y_m - (row as f64 + 0.5) * pixel_height_m;
    }
    (
        x_sum / component.len() as f64,
        y_sum / component.len() as f64,
    )
}

fn validate_tile_geometry(tile_id: &str, tile: &PlantCountTile) -> Result<(), StandCountError> {
    let valid = tile.width_px > 0
        && tile.height_px > 0
        && tile.min_x_m.is_finite()
        && tile.min_y_m.is_finite()
        && tile.max_x_m.is_finite()
        && tile.max_y_m.is_finite()
        && tile.max_x_m > tile.min_x_m
        && tile.max_y_m > tile.min_y_m;
    if !valid {
        return Err(StandCountError::InvalidTileGeometry {
            tile_id: tile_id.to_string(),
        });
    }
    if tile.crop_mask.len() != tile.width_px as usize * tile.height_px as usize {
        return Err(StandCountError::CropMaskSizeMismatch {
            tile_id: tile_id.to_string(),
        });
    }
    Ok(())
}

fn normalize_stand_text(value: String, error: StandCountError) -> Result<String, StandCountError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_optional_stand_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn normalize_canopy_text(
    value: String,
    error: CanopyCoverError,
) -> Result<String, CanopyCoverError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn cover_fraction(vegetation_pixels: usize, valid_pixels: usize) -> f64 {
    if valid_pixels > 0 {
        vegetation_pixels as f64 / valid_pixels as f64
    } else {
        0.0
    }
}

fn normalize_disease_text(
    value: String,
    error: DiseaseDetectionError,
) -> Result<String, DiseaseDetectionError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn is_unit_fraction(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn valid_bbox(bbox: &GeoBounds) -> bool {
    bbox.min_lon.is_finite()
        && bbox.min_lat.is_finite()
        && bbox.max_lon.is_finite()
        && bbox.max_lat.is_finite()
        && bbox.max_lon > bbox.min_lon
        && bbox.max_lat > bbox.min_lat
}

fn bbox_within(inner: &GeoBounds, outer: &GeoBounds) -> bool {
    const TOLERANCE: f64 = 1.0e-9;
    inner.min_lon + TOLERANCE >= outer.min_lon
        && inner.min_lat + TOLERANCE >= outer.min_lat
        && inner.max_lon <= outer.max_lon + TOLERANCE
        && inner.max_lat <= outer.max_lat + TOLERANCE
}

fn normalize_weed_text(value: String, error: WeedMappingError) -> Result<String, WeedMappingError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(error)
    } else {
        Ok(trimmed.to_string())
    }
}

fn bbox_area_m2(bbox: &GeoBounds) -> f64 {
    (bbox.max_lon - bbox.min_lon) * (bbox.max_lat - bbox.min_lat)
}

fn density_per_ha(count: usize, area_m2: f64) -> f64 {
    if area_m2 > 0.0 {
        count as f64 / (area_m2 / 10_000.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_model_version_record, run_canopy_cover, run_disease_lesion_detection,
        run_stand_count, run_weed_mapping, validate_model_reference, CanopyCoverConfig,
        CanopyCoverError, CanopyCoverTile, CropModelRegistryError, CropModelTask,
        DiseaseDetectionConfig, DiseaseDetectionError, DiseaseLesionCandidate,
        InferenceModelReference, ModelVersionRegistrationRequest, PlantCountConfig, PlantCountTile,
        PlantCountZeroReason, WeedMappingConfig, WeedMappingError, WeedZoneCandidate,
    };
    use shared::schemas::{GeoBounds, RasterResolution, RasterSpatialRef};

    #[test]
    fn model_version_record_requires_versioned_provenance() {
        let record = build_model_version_record(
            ModelVersionRegistrationRequest {
                model_id: " lesion-detector ".to_string(),
                version: " 2026.06.1 ".to_string(),
                task: CropModelTask::DiseaseDetection,
                training_set_ref: " dataset:lesion-v3 ".to_string(),
                metrics: serde_json::json!({
                    "precision": 0.91,
                    "recall": 0.87,
                    "iou": 0.73
                }),
                provenance_ref: " provenance:model/lesion-detector/2026.06.1 ".to_string(),
            },
            " 2026-06-12T12:00:00Z ".to_string(),
        )
        .expect("model version should be valid");

        assert_eq!(record.model_id, "lesion-detector");
        assert_eq!(record.version, "2026.06.1");
        assert_eq!(record.task, CropModelTask::DiseaseDetection);
        assert_eq!(record.training_set_ref, "dataset:lesion-v3");
        assert_eq!(
            record
                .metrics
                .get("precision")
                .and_then(|value| value.as_f64()),
            Some(0.91)
        );
        assert_eq!(
            record.provenance_ref,
            "provenance:model/lesion-detector/2026.06.1"
        );
    }

    #[test]
    fn model_version_rejects_missing_metrics() {
        let error = build_model_version_record(
            ModelVersionRegistrationRequest {
                model_id: "lesion-detector".to_string(),
                version: "2026.06.1".to_string(),
                task: CropModelTask::DiseaseDetection,
                training_set_ref: "dataset:lesion-v3".to_string(),
                metrics: serde_json::json!({}),
                provenance_ref: "provenance:model/lesion-detector/2026.06.1".to_string(),
            },
            "2026-06-12T12:00:00Z".to_string(),
        )
        .expect_err("empty metrics should be rejected");

        assert_eq!(error, CropModelRegistryError::InvalidMetrics);
    }

    #[test]
    fn unregistered_model_reference_is_rejected() {
        let error = validate_model_reference(
            InferenceModelReference {
                model_id: "unknown-model".to_string(),
                version: "v0".to_string(),
            },
            false,
        )
        .expect_err("unknown model should be rejected");

        assert_eq!(
            error,
            CropModelRegistryError::UnregisteredModel {
                model_id: "unknown-model".to_string(),
                version: "v0".to_string()
            }
        );
    }

    #[test]
    fn stand_count_detects_plants_per_field_zone_and_locations() {
        let report = run_stand_count(
            "field-1".to_string(),
            "EPSG:32614".to_string(),
            vec![plant_tile(
                "tile-1",
                Some("zone-a"),
                true,
                vec![
                    true, true, false, false, false, false, false, false, false, false, false,
                    false, false, false, false, true,
                ],
            )],
            PlantCountConfig {
                min_component_pixels: 1,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("stand count should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.total_count, 2);
        assert_eq!(report.tiles[0].plant_count, 2);
        assert_eq!(report.zones[0].zone_id, "zone-a");
        assert_eq!(report.zones[0].plant_count, 2);
        assert_eq!(report.plant_locations.len(), 2);
        assert!(report
            .plant_locations
            .iter()
            .all(|plant| plant.crs == "EPSG:32614"));
        assert!(report.field_density_plants_per_ha > 0.0);
    }

    #[test]
    fn stand_count_invalid_tile_contributes_zero_with_reason() {
        let report = run_stand_count(
            "field-1".to_string(),
            "EPSG:32614".to_string(),
            vec![plant_tile(
                "tile-cloud",
                Some("zone-a"),
                false,
                vec![
                    true, true, true, true, true, true, true, true, true, true, true, true, true,
                    true, true, true,
                ],
            )],
            PlantCountConfig {
                min_component_pixels: 1,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("stand count should run");

        assert_eq!(report.total_count, 0);
        assert!(report.plant_locations.is_empty());
        assert_eq!(
            report.tiles[0].zero_reason,
            Some(PlantCountZeroReason::InvalidTile)
        );
    }

    #[test]
    fn stand_count_bare_tile_contributes_zero_with_reason() {
        let report = run_stand_count(
            "field-1".to_string(),
            "EPSG:32614".to_string(),
            vec![plant_tile(
                "tile-bare",
                Some("zone-a"),
                true,
                vec![false; 16],
            )],
            PlantCountConfig {
                min_component_pixels: 1,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("stand count should run");

        assert_eq!(report.total_count, 0);
        assert!(report.plant_locations.is_empty());
        assert_eq!(
            report.tiles[0].zero_reason,
            Some(PlantCountZeroReason::NoValidCropPixels)
        );
    }

    #[test]
    fn canopy_cover_returns_georeferenced_masks_and_zone_fractions() {
        let report = run_canopy_cover(
            "field-1".to_string(),
            vec![canopy_tile(
                "tile-1",
                Some("zone-a"),
                3,
                2,
                vec![0.7, 0.2, 0.5, 0.1, 0.8, 0.4],
                vec![true; 6],
            )],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("canopy cover should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.valid_pixels, 6);
        assert_eq!(report.vegetation_pixels, 3);
        assert_eq!(report.excluded_pixels, 0);
        assert_eq!(report.cover_fraction, 0.5);
        assert_eq!(report.zones[0].zone_id, "zone-a");
        assert_eq!(report.zones[0].cover_fraction, 0.5);
        assert_eq!(
            report.tiles[0].mask.vegetation_mask,
            vec![true, false, true, false, true, false]
        );
        assert_eq!(report.tiles[0].mask.valid_mask, vec![true; 6]);
        assert_eq!(
            report.tiles[0].spatial_ref.crs.as_deref(),
            Some("EPSG:32614")
        );
        assert_eq!(report.tiles[0].spatial_ref, spatial_ref(3, 2));
    }

    #[test]
    fn canopy_cover_excludes_cloud_nodata_pixels_from_fraction() {
        let report = run_canopy_cover(
            "field-1".to_string(),
            vec![canopy_tile(
                "tile-cloud",
                Some("zone-a"),
                2,
                2,
                vec![0.8, 0.0, 0.1, 0.9],
                vec![true, false, true, false],
            )],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("canopy cover should run");

        assert_eq!(report.valid_pixels, 2);
        assert_eq!(report.vegetation_pixels, 1);
        assert_eq!(report.excluded_pixels, 2);
        assert_eq!(report.cover_fraction, 0.5);
        assert_eq!(
            report.tiles[0].mask.vegetation_mask,
            vec![true, false, false, false]
        );
        assert_eq!(
            report.tiles[0].mask.valid_mask,
            vec![true, false, true, false]
        );
    }

    #[test]
    fn canopy_cover_rejects_bad_spatial_ref() {
        let mut tile = canopy_tile(
            "tile-bad-ref",
            Some("zone-a"),
            2,
            2,
            vec![0.8, 0.0, 0.1, 0.9],
            vec![true; 4],
        );
        tile.spatial_ref.georeferenced = false;

        let error = run_canopy_cover(
            "field-1".to_string(),
            vec![tile],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect_err("bad spatial ref should be rejected");

        assert!(matches!(
            error,
            CanopyCoverError::SpatialRefInvalid { tile_id, .. } if tile_id == "tile-bad-ref"
        ));
    }

    #[test]
    fn disease_detection_returns_confidence_evidence_and_bounded_zone() {
        let cover = cover_report();
        let report = run_disease_lesion_detection(
            "field-1".to_string(),
            registered_model(),
            true,
            Some(&cover),
            vec![lesion_candidate(
                "tile-1",
                0.82,
                GeoBounds {
                    min_lon: 5.0,
                    min_lat: 5.0,
                    max_lon: 15.0,
                    max_lat: 15.0,
                },
            )],
            DiseaseDetectionConfig {
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:15:00Z".to_string(),
        )
        .expect("disease detection should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.model.model_id, "lesion-detector");
        assert_eq!(report.deterministic_cover_valid_pixels, cover.valid_pixels);
        assert_eq!(report.detections.len(), 1);
        let detection = &report.detections[0];
        assert_eq!(detection.evidence_tile_ref, "tile-1");
        assert_eq!(detection.confidence, 0.82);
        assert!(!detection.low_confidence);
        assert_eq!(detection.zone_geometry.crs, "EPSG:32614");
        assert_eq!(
            detection.zone_geometry.bbox,
            GeoBounds {
                min_lon: 5.0,
                min_lat: 5.0,
                max_lon: 15.0,
                max_lat: 15.0,
            }
        );
    }

    #[test]
    fn disease_detection_marks_low_confidence_without_hiding_detection() {
        let cover = cover_report();
        let report = run_disease_lesion_detection(
            "field-1".to_string(),
            registered_model(),
            true,
            Some(&cover),
            vec![lesion_candidate(
                "tile-1",
                0.42,
                GeoBounds {
                    min_lon: 5.0,
                    min_lat: 5.0,
                    max_lon: 15.0,
                    max_lat: 15.0,
                },
            )],
            DiseaseDetectionConfig {
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:15:00Z".to_string(),
        )
        .expect("low-confidence detection should be retained");

        assert_eq!(report.detections.len(), 1);
        assert_eq!(report.low_confidence_count, 1);
        assert!(report.detections[0].low_confidence);
    }

    #[test]
    fn disease_detection_refuses_to_run_without_deterministic_cover() {
        let error = run_disease_lesion_detection(
            "field-1".to_string(),
            registered_model(),
            true,
            None,
            vec![lesion_candidate(
                "tile-1",
                0.82,
                GeoBounds {
                    min_lon: 5.0,
                    min_lat: 5.0,
                    max_lon: 15.0,
                    max_lat: 15.0,
                },
            )],
            DiseaseDetectionConfig {
                low_confidence_threshold: 0.7,
            },
            "2026-06-01T12:15:00Z".to_string(),
        )
        .expect_err("deterministic cover is required");

        assert_eq!(error, DiseaseDetectionError::DeterministicCoverRequired);
    }

    #[test]
    fn weed_mapping_returns_georeferenced_confidence_zones_and_area() {
        let cover = cover_report();
        let report = run_weed_mapping(
            "field-1".to_string(),
            weed_model(),
            true,
            Some(&cover),
            vec![weed_candidate(
                "tile-1",
                0.76,
                GeoBounds {
                    min_lon: 0.0,
                    min_lat: 0.0,
                    max_lon: 10.0,
                    max_lat: 10.0,
                },
            )],
            WeedMappingConfig {
                low_confidence_threshold: 0.65,
            },
            "2026-06-01T12:20:00Z".to_string(),
        )
        .expect("weed mapping should run");

        assert_eq!(report.field_id, "field-1");
        assert_eq!(report.crs, "EPSG:32614");
        assert_eq!(report.model.model_id, "weed-detector");
        assert_eq!(report.zones.len(), 1);
        assert_eq!(report.total_weed_area_m2, 100.0);
        let zone = &report.zones[0];
        assert_eq!(zone.evidence_tile_ref, "tile-1");
        assert_eq!(zone.confidence, 0.76);
        assert_eq!(zone.area_m2, 100.0);
        assert!(!zone.low_confidence);
        assert_eq!(zone.geometry.crs, "EPSG:32614");
        assert_eq!(
            zone.geometry.bbox,
            GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: 10.0,
                max_lat: 10.0,
            }
        );
    }

    #[test]
    fn weed_mapping_returns_no_zones_for_weed_free_field() {
        let cover = cover_report();
        let report = run_weed_mapping(
            "field-1".to_string(),
            weed_model(),
            true,
            Some(&cover),
            Vec::new(),
            WeedMappingConfig {
                low_confidence_threshold: 0.65,
            },
            "2026-06-01T12:20:00Z".to_string(),
        )
        .expect("weed-free mapping should run");

        assert!(report.zones.is_empty());
        assert_eq!(report.total_weed_area_m2, 0.0);
        assert_eq!(report.low_confidence_count, 0);
    }

    #[test]
    fn weed_mapping_refuses_to_run_without_deterministic_cover() {
        let error = run_weed_mapping(
            "field-1".to_string(),
            weed_model(),
            true,
            None,
            Vec::new(),
            WeedMappingConfig {
                low_confidence_threshold: 0.65,
            },
            "2026-06-01T12:20:00Z".to_string(),
        )
        .expect_err("deterministic cover is required");

        assert_eq!(error, WeedMappingError::DeterministicCoverRequired);
    }

    fn plant_tile(
        tile_id: &str,
        zone_id: Option<&str>,
        valid: bool,
        crop_mask: Vec<bool>,
    ) -> PlantCountTile {
        PlantCountTile {
            tile_id: tile_id.to_string(),
            zone_id: zone_id.map(ToOwned::to_owned),
            valid,
            width_px: 4,
            height_px: 4,
            min_x_m: 0.0,
            min_y_m: 0.0,
            max_x_m: 40.0,
            max_y_m: 40.0,
            crop_mask,
        }
    }

    fn canopy_tile(
        tile_id: &str,
        zone_id: Option<&str>,
        width_px: u32,
        height_px: u32,
        index_values: Vec<f64>,
        valid_mask: Vec<bool>,
    ) -> CanopyCoverTile {
        CanopyCoverTile {
            tile_id: tile_id.to_string(),
            zone_id: zone_id.map(ToOwned::to_owned),
            width_px,
            height_px,
            spatial_ref: spatial_ref(width_px, height_px),
            index_values,
            valid_mask,
        }
    }

    fn cover_report() -> super::CanopyCoverReport {
        run_canopy_cover(
            "field-1".to_string(),
            vec![canopy_tile(
                "tile-1",
                Some("zone-a"),
                3,
                2,
                vec![0.7, 0.2, 0.5, 0.1, 0.8, 0.4],
                vec![true; 6],
            )],
            CanopyCoverConfig {
                vegetation_index_threshold: 0.5,
            },
            "2026-06-01T12:10:00Z".to_string(),
        )
        .expect("cover report should be valid")
    }

    fn registered_model() -> InferenceModelReference {
        InferenceModelReference {
            model_id: "lesion-detector".to_string(),
            version: "2026.06.1".to_string(),
        }
    }

    fn lesion_candidate(tile_id: &str, confidence: f64, bbox: GeoBounds) -> DiseaseLesionCandidate {
        DiseaseLesionCandidate {
            tile_id: tile_id.to_string(),
            confidence,
            bbox,
        }
    }

    fn weed_model() -> InferenceModelReference {
        InferenceModelReference {
            model_id: "weed-detector".to_string(),
            version: "2026.06.1".to_string(),
        }
    }

    fn weed_candidate(tile_id: &str, confidence: f64, bbox: GeoBounds) -> WeedZoneCandidate {
        WeedZoneCandidate {
            tile_id: tile_id.to_string(),
            confidence,
            bbox,
        }
    }

    fn spatial_ref(width_px: u32, height_px: u32) -> RasterSpatialRef {
        let max_x = width_px as f64 * 10.0;
        let max_y = height_px as f64 * 10.0;
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:32614".to_string()),
            bbox: Some(GeoBounds {
                min_lon: 0.0,
                min_lat: 0.0,
                max_lon: max_x,
                max_lat: max_y,
            }),
            geo_transform: Some([0.0, 10.0, 0.0, max_y, 0.0, -10.0]),
            resolution: Some(RasterResolution { x: 10.0, y: 10.0 }),
        }
    }
}
