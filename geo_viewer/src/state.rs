use anyhow::Result;
use bevy::prelude::*;
use bevy::tasks::Task;
use image::DynamicImage;
use serde::Deserialize;
use shared::schemas::{AnnotationRecord, FieldRecord, GeoPoint, GpsCoords};
use std::fmt;

pub const APP_TITLE: &str = "Geo Viewer";
pub const DEFAULT_PRODUCT_KIND: &str = "ndvi";
pub const MAP_UNITS_PER_DEGREE: f32 = 10_000.0;
pub const DEFAULT_TILE_ZOOM: u8 = 0;

#[derive(Resource)]
pub struct ViewerState {
    pub selected_layer: usize,
    pub zoom_level: f32,
    pub scene_id_input: String,
}

#[derive(Resource)]
pub struct TileConfig {
    pub base_url: String,
    pub scene_id: Option<String>,
    pub product_kind: String,
}

#[derive(Resource, Default)]
pub struct FieldListFetchTask(pub Option<Task<anyhow::Result<Vec<FieldRecord>>>>);

#[derive(Resource, Default)]
pub struct FieldScenesFetchTask(pub Option<Task<anyhow::Result<Vec<FieldSceneSummary>>>>);

#[derive(Resource, Default)]
pub struct ManifestFetchTask(pub Option<Task<anyhow::Result<SceneManifest>>>);

#[derive(Resource, Default)]
pub struct TileFetchTask(pub Option<Task<anyhow::Result<FetchedTile>>>);

#[derive(Resource, Default)]
pub struct AnnotationFetchTask(pub Option<Task<anyhow::Result<Vec<AnnotationRecord>>>>);

#[derive(Resource, Default)]
pub struct AnnotationCreateTask(pub Option<Task<anyhow::Result<AnnotationRecord>>>);

#[derive(Resource, Default)]
pub struct AnnotationUpdateTask(pub Option<Task<anyhow::Result<AnnotationRecord>>>);

#[derive(Resource, Default)]
pub struct AnnotationDeleteTask(pub Option<Task<anyhow::Result<String>>>);

pub struct FetchedTile {
    pub tile_id: TileId,
    pub image: DynamicImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl fmt::Display for TileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.x, self.y)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldSceneSummary {
    pub scene_id: String,
    pub sensor: String,
    pub acquired_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SceneManifest {
    pub scene_id: String,
    pub sensor: Option<String>,
    pub acquired_at: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bands: Vec<String>,
    pub gps_position: Option<GpsCoords>,
    pub data_path: Option<String>,
    pub field: Option<FieldRecord>,
    pub geospatial: SceneGeospatialMetadata,
    pub available_products: Vec<SceneProduct>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SceneProduct {
    pub kind: String,
    pub filename: String,
    pub content_type: String,
    pub url_path: String,
    pub tile_url_template: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileSource {
    pub base_url: String,
    pub template: String,
}

impl TileSource {
    pub fn tile_url(&self, tile_id: TileId) -> String {
        format!(
            "{}{}",
            self.base_url.trim_end_matches('/'),
            self.template
                .replace("{z}", &tile_id.z.to_string())
                .replace("{x}", &tile_id.x.to_string())
                .replace("{y}", &tile_id.y.to_string())
        )
    }
}

impl SceneProduct {
    pub fn tile_source(&self, base_url: &str) -> TileSource {
        TileSource {
            base_url: base_url.to_string(),
            template: self.tile_url_template.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SceneGeospatialMetadata {
    pub georeferenced: bool,
    pub crs: Option<String>,
    pub center: Option<GpsCoords>,
    pub extent: Option<SceneExtent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SceneExtent {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

#[derive(Resource, Default)]
pub struct SceneManifestState {
    pub scene_id: Option<String>,
    pub sensor: Option<String>,
    pub acquired_at: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bands: Vec<String>,
    pub gps_position: Option<GpsCoords>,
    pub data_path: Option<String>,
    pub field: Option<FieldRecord>,
    pub geospatial: SceneGeospatialMetadata,
    pub products: Vec<SceneProduct>,
}

#[derive(Resource, Default)]
pub struct FieldCatalogState {
    pub fields: Vec<FieldRecord>,
    pub scenes: Vec<FieldSceneSummary>,
    pub selected_field_id: Option<String>,
    pub selected_scene_id: Option<String>,
}

#[derive(Resource)]
pub struct TileRenderState {
    pub entity: Option<Entity>,
    pub handle: Option<Handle<Image>>,
    pub image_dimensions: Vec2,
    pub world_dimensions: Vec2,
    pub status: TileStatus,
}

#[derive(Resource)]
pub struct MapViewState {
    pub center: Vec2,
    pub base_scale: f32,
    pub needs_fit: bool,
}

#[derive(Resource, Default)]
pub struct CursorMapState {
    pub world_position: Option<Vec2>,
    pub geo_position: Option<(f64, f64)>,
}

#[derive(Resource)]
pub struct AnnotationOverlayState {
    pub items: Vec<AnnotationRecord>,
    pub selected_annotation_id: Option<String>,
    pub hovered_annotation_id: Option<String>,
    pub draft_label: String,
    pub draft_note: String,
    pub draft_severity: String,
    pub draft_mode: DraftMode,
    pub draft_point: Option<GeoPoint>,
    pub draft_polygon_vertices: Vec<GeoPoint>,
    pub hovered_draft_vertex_index: Option<usize>,
    pub hovered_draft_segment_index: Option<usize>,
    pub dragged_draft_vertex_index: Option<usize>,
    pub filter_label: String,
    pub show_points: bool,
    pub show_polygons: bool,
    pub show_low: bool,
    pub show_medium: bool,
    pub show_high: bool,
    pub show_critical: bool,
    pub show_other: bool,
}

impl Default for AnnotationOverlayState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_annotation_id: None,
            hovered_annotation_id: None,
            draft_label: String::new(),
            draft_note: String::new(),
            draft_severity: String::new(),
            draft_mode: DraftMode::default(),
            draft_point: None,
            draft_polygon_vertices: Vec::new(),
            hovered_draft_vertex_index: None,
            hovered_draft_segment_index: None,
            dragged_draft_vertex_index: None,
            filter_label: String::new(),
            show_points: true,
            show_polygons: true,
            show_low: true,
            show_medium: true,
            show_high: true,
            show_critical: true,
            show_other: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DraftMode {
    #[default]
    Point,
    Polygon,
}

#[derive(Debug, Clone)]
pub enum TileStatus {
    Idle,
    Fetching,
    Ready,
    MissingScene,
    Error(String),
}

impl TileStatus {
    pub fn message(&self) -> String {
        match self {
            Self::Idle => "Idle".to_string(),
            Self::Fetching => "Fetching tile data…".to_string(),
            Self::Ready => "Tile ready".to_string(),
            Self::MissingScene => "Enter a scene ID to load a product".to_string(),
            Self::Error(err) => format!("Error: {}", err),
        }
    }
}

#[derive(Component)]
pub struct TileDisplay;

#[derive(Component)]
pub struct MapCamera;

pub fn initial_tile_config() -> (TileConfig, ViewerState, TileStatus) {
    let base_url =
        std::env::var("GEO_HUB_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let scene_id_env = std::env::var("GEO_VIEWER_SCENE_ID")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let initial_status = if scene_id_env.is_some() {
        TileStatus::Idle
    } else {
        TileStatus::MissingScene
    };

    (
        TileConfig {
            base_url,
            scene_id: scene_id_env.clone(),
            product_kind: DEFAULT_PRODUCT_KIND.to_string(),
        },
        ViewerState {
            selected_layer: 0,
            zoom_level: 1.0,
            scene_id_input: scene_id_env.unwrap_or_default(),
        },
        initial_status,
    )
}

pub fn ensure_scene_id(config: &TileConfig, action: &str) -> Result<String> {
    config
        .scene_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("scene_id is required to {}", action))
}

#[cfg(test)]
mod tests {
    use super::{SceneProduct, TileId};

    #[test]
    fn tile_source_formats_tile_url_from_template() {
        let product = SceneProduct {
            kind: "ndvi".to_string(),
            filename: "ndvi.png".to_string(),
            content_type: "image/png".to_string(),
            url_path: "/api/scenes/scene-1/products/ndvi".to_string(),
            tile_url_template: "/api/scenes/scene-1/products/ndvi/tiles/{z}/{x}/{y}.png"
                .to_string(),
        };

        let url = product
            .tile_source("http://127.0.0.1:8080")
            .tile_url(TileId { z: 2, x: 3, y: 1 });

        assert_eq!(
            url,
            "http://127.0.0.1:8080/api/scenes/scene-1/products/ndvi/tiles/2/3/1.png"
        );
    }
}
