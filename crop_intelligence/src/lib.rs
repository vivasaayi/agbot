use serde::{Deserialize, Serialize};
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
        build_model_version_record, run_stand_count, validate_model_reference,
        CropModelRegistryError, CropModelTask, InferenceModelReference,
        ModelVersionRegistrationRequest, PlantCountConfig, PlantCountTile, PlantCountZeroReason,
    };

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
}
