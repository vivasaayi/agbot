use anyhow::Result;
use bevy::prelude::*;
use bevy::tasks::Task;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use shared::schemas::{
    AnnotationRecord, FarmRecord, FieldRecord, GeoPoint, GpsCoords, RecommendationPriority,
    RecommendationRecord, RecommendationStatus, ReportRecord,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const APP_TITLE: &str = "Geo Viewer";
pub const DEFAULT_PRODUCT_KIND: &str = "ndvi";
pub const MAP_UNITS_PER_DEGREE: f32 = 10_000.0;
pub const DEFAULT_TILE_ZOOM: u8 = 2;

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
pub struct FarmListFetchTask(pub Option<Task<anyhow::Result<Vec<FarmRecord>>>>);

#[derive(Resource, Default)]
pub struct FarmFieldHistoryFetchTask(pub Option<Task<anyhow::Result<Vec<FieldSeasonGroup>>>>);

#[derive(Resource, Default)]
pub struct FieldImportTask(pub Option<Task<anyhow::Result<Vec<FieldRecord>>>>);

#[derive(Resource, Default)]
pub struct ManifestFetchTask(pub Option<Task<anyhow::Result<SceneManifest>>>);

#[derive(Resource, Default)]
pub struct TileFetchTasks(pub BTreeMap<TileId, Task<anyhow::Result<FetchedTile>>>);

#[derive(Resource, Default)]
pub struct AnnotationFetchTask(pub Option<Task<anyhow::Result<Vec<AnnotationRecord>>>>);

#[derive(Resource, Default)]
pub struct AnnotationCreateTask(pub Option<Task<anyhow::Result<AnnotationRecord>>>);

#[derive(Resource, Default)]
pub struct AnnotationUpdateTask(pub Option<Task<anyhow::Result<AnnotationRecord>>>);

#[derive(Resource, Default)]
pub struct AnnotationDeleteTask(pub Option<Task<anyhow::Result<String>>>);

#[derive(Resource, Default)]
pub struct RecommendationFetchTask(pub Option<Task<anyhow::Result<Vec<RecommendationRecord>>>>);

#[derive(Resource, Default)]
pub struct RecommendationCreateTask(pub Option<Task<anyhow::Result<RecommendationRecord>>>);

#[derive(Resource, Default)]
pub struct RecommendationUpdateTask(pub Option<Task<anyhow::Result<RecommendationRecord>>>);

#[derive(Resource, Default)]
pub struct RecommendationDeleteTask(pub Option<Task<anyhow::Result<String>>>);

#[derive(Resource, Default)]
pub struct ReportFetchTask(pub Option<Task<anyhow::Result<Vec<ReportRecord>>>>);

#[derive(Resource, Default)]
pub struct ReportGenerateTask(pub Option<Task<anyhow::Result<ReportRecord>>>);

pub struct FetchedTile {
    pub tile_id: TileId,
    pub image: DynamicImage,
    pub missing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    #[serde(default)]
    pub owner: Option<String>,
    pub sensor: String,
    pub acquired_at: String,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
    #[serde(default)]
    pub linked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSceneSelection {
    pub field_id: String,
    pub scene_id: String,
    pub season_id: String,
    pub owner: String,
    pub linked_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldSeasonGroup {
    pub season: Option<String>,
    pub fields: Vec<FieldRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SceneManifest {
    pub scene_id: String,
    pub owner: Option<String>,
    pub sensor: Option<String>,
    pub acquired_at: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bands: Vec<String>,
    pub gps_position: Option<GpsCoords>,
    pub data_path: Option<String>,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub linked_at: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductPlacement {
    Placeable,
    Unplaceable(String),
}

impl ProductPlacement {
    pub fn is_placeable(&self) -> bool {
        matches!(self, Self::Placeable)
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Placeable => None,
            Self::Unplaceable(reason) => Some(reason.as_str()),
        }
    }
}

pub fn product_placement_for_manifest(geospatial: &SceneGeospatialMetadata) -> ProductPlacement {
    if geospatial.georeferenced {
        ProductPlacement::Placeable
    } else {
        ProductPlacement::Unplaceable("scene manifest is not georeferenced".to_string())
    }
}

pub fn manifest_world_dimensions(
    geospatial: &SceneGeospatialMetadata,
    width: Option<u32>,
    height: Option<u32>,
) -> Vec2 {
    if !product_placement_for_manifest(geospatial).is_placeable() {
        return Vec2::ZERO;
    }

    geospatial
        .extent
        .as_ref()
        .map(|extent| {
            Vec2::new(
                ((extent.max_lon - extent.min_lon) as f32).abs() * MAP_UNITS_PER_DEGREE,
                ((extent.max_lat - extent.min_lat) as f32).abs() * MAP_UNITS_PER_DEGREE,
            )
        })
        .or_else(|| {
            width
                .zip(height)
                .map(|(width, height)| Vec2::new(width as f32, height as f32))
        })
        .unwrap_or(Vec2::ZERO)
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
    pub owner: Option<String>,
    pub sensor: Option<String>,
    pub acquired_at: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bands: Vec<String>,
    pub gps_position: Option<GpsCoords>,
    pub data_path: Option<String>,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub linked_at: Option<String>,
    pub field: Option<FieldRecord>,
    pub geospatial: SceneGeospatialMetadata,
    pub products: Vec<SceneProduct>,
}

#[derive(Resource, Default)]
pub struct FieldCatalogState {
    pub farms: Vec<FarmRecord>,
    pub fields: Vec<FieldRecord>,
    pub season_groups: Vec<FieldSeasonGroup>,
    pub scenes: Vec<FieldSceneSummary>,
    pub selected_farm_id: Option<String>,
    pub selected_field_id: Option<String>,
    pub selected_scene_id: Option<String>,
    pub selected_season_id: Option<String>,
    pub selected_owner: Option<String>,
    pub selected_linked_at: Option<String>,
}

pub fn select_catalog_scene(
    catalog: &mut FieldCatalogState,
    scene: &FieldSceneSummary,
) -> Result<CatalogSceneSelection> {
    let field_id = required_scene_context(scene, scene.field_id.as_deref(), "field_id")?;
    let season_id = required_scene_context(scene, scene.season_id.as_deref(), "season_id")?;
    let owner = required_scene_context(scene, scene.owner.as_deref(), "owner")?;
    let linked_at = required_scene_context(scene, scene.linked_at.as_deref(), "linked_at")?;

    catalog.selected_scene_id = Some(scene.scene_id.clone());
    catalog.selected_field_id = Some(field_id.clone());
    catalog.selected_season_id = Some(season_id.clone());
    catalog.selected_owner = Some(owner.clone());
    catalog.selected_linked_at = Some(linked_at.clone());

    Ok(CatalogSceneSelection {
        field_id,
        scene_id: scene.scene_id.clone(),
        season_id,
        owner,
        linked_at,
    })
}

fn required_scene_context(
    scene: &FieldSceneSummary,
    value: Option<&str>,
    label: &str,
) -> Result<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("unlinked scene {} missing {label}", scene.scene_id))
}

#[derive(Resource, Default)]
pub struct FieldImportState {
    pub shapefile_path: String,
    pub name_prefix: String,
    pub crop: String,
    pub season: String,
    pub notes: String,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShapefileImportRequest {
    pub path: String,
    pub name_prefix: Option<String>,
    pub farm_id: Option<String>,
    pub crop: Option<String>,
    pub season: Option<String>,
    pub notes: Option<String>,
}

#[derive(Resource)]
pub struct TileRenderState {
    pub tiles: BTreeMap<TileId, RenderedTile>,
    pub visible_tiles: BTreeSet<TileId>,
    pub image_dimensions: Vec2,
    pub world_dimensions: Vec2,
    pub current_zoom: u8,
    pub status: TileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePresence {
    Loading,
    Ready,
    Missing,
    Failed,
}

pub struct RenderedTile {
    pub entity: Entity,
    pub presence: TilePresence,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TilePresenceSummary {
    pub loading: usize,
    pub ready: usize,
    pub missing: usize,
    pub failed: usize,
}

pub fn summarize_tile_presences<I>(presences: I) -> TilePresenceSummary
where
    I: IntoIterator<Item = TilePresence>,
{
    let mut summary = TilePresenceSummary::default();
    for presence in presences {
        match presence {
            TilePresence::Loading => summary.loading += 1,
            TilePresence::Ready => summary.ready += 1,
            TilePresence::Missing => summary.missing += 1,
            TilePresence::Failed => summary.failed += 1,
        }
    }
    summary
}

impl TileRenderState {
    pub fn presence_summary(&self) -> TilePresenceSummary {
        summarize_tile_presences(self.tiles.values().map(|tile| tile.presence))
    }

    pub fn loading_tile_count(&self) -> usize {
        self.presence_summary().loading
    }

    pub fn ready_tile_count(&self) -> usize {
        self.presence_summary().ready
    }

    pub fn missing_tile_count(&self) -> usize {
        self.presence_summary().missing
    }

    pub fn failed_tile_count(&self) -> usize {
        self.presence_summary().failed
    }
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

#[derive(Resource)]
pub struct RecommendationOverlayState {
    pub items: Vec<RecommendationRecord>,
    pub selected_recommendation_id: Option<String>,
    pub draft_title: String,
    pub draft_note: String,
    pub draft_category: String,
    pub draft_priority: RecommendationPriority,
    pub draft_status: RecommendationStatus,
    pub linked_annotation_ids: Vec<String>,
    pub status_filter: Option<RecommendationStatus>,
    pub priority_filter: Option<RecommendationPriority>,
}

impl Default for RecommendationOverlayState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_recommendation_id: None,
            draft_title: String::new(),
            draft_note: String::new(),
            draft_category: String::new(),
            draft_priority: RecommendationPriority::default(),
            draft_status: RecommendationStatus::default(),
            linked_annotation_ids: Vec::new(),
            status_filter: None,
            priority_filter: None,
        }
    }
}

#[derive(Resource, Default)]
pub struct ReportOverlayState {
    pub items: Vec<ReportRecord>,
    pub draft_title: String,
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
    use super::{
        manifest_world_dimensions, product_placement_for_manifest, select_catalog_scene,
        summarize_tile_presences, FieldCatalogState, FieldSceneSummary, SceneExtent,
        SceneGeospatialMetadata, SceneManifest, SceneProduct, TileId, TilePresence,
    };
    use bevy::prelude::Vec2;

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

    #[test]
    fn field_scene_catalog_decodes_geo_hub_linkage_context() {
        let scenes: Vec<FieldSceneSummary> = serde_json::from_str(
            r#"[
                {
                    "scene_id": "scene-1",
                    "owner": "org-alpha",
                    "sensor": "landsat8",
                    "acquired_at": "2026-05-01T00:00:00Z",
                    "field_id": "field-1",
                    "season_id": "2026",
                    "linked_at": "2026-05-01T00:00:00Z"
                }
            ]"#,
        )
        .expect("geo hub field scene payload should decode");

        let scene = scenes
            .first()
            .expect("catalog payload should contain a scene");

        assert_eq!(scene.scene_id, "scene-1");
        assert_eq!(scene.owner.as_deref(), Some("org-alpha"));
        assert_eq!(scene.field_id.as_deref(), Some("field-1"));
        assert_eq!(scene.season_id.as_deref(), Some("2026"));
        assert_eq!(scene.linked_at.as_deref(), Some("2026-05-01T00:00:00Z"));
    }

    #[test]
    fn tile_presence_summary_distinguishes_loading_ready_missing_and_failed_tiles() {
        let summary = summarize_tile_presences([
            TilePresence::Loading,
            TilePresence::Ready,
            TilePresence::Missing,
            TilePresence::Failed,
            TilePresence::Ready,
        ]);

        assert_eq!(summary.loading, 1);
        assert_eq!(summary.ready, 2);
        assert_eq!(summary.missing, 1);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn scene_manifest_decodes_geospatial_metadata_and_product_list() {
        let manifest: SceneManifest = serde_json::from_str(
            r#"{
                "scene_id": "scene-1",
                "owner": "org-alpha",
                "sensor": "landsat8",
                "acquired_at": "2026-05-01T00:00:00Z",
                "width": 256,
                "height": 256,
                "bands": ["red", "nir"],
                "gps_position": null,
                "data_path": "/tmp/scene-1",
                "field_id": "field-1",
                "season_id": "2026",
                "linked_at": "2026-05-01T00:00:00Z",
                "field": null,
                "geospatial": {
                    "georeferenced": true,
                    "crs": "EPSG:4326",
                    "center": null,
                    "extent": {
                        "min_lon": -89.5,
                        "min_lat": 40.0,
                        "max_lon": -88.5,
                        "max_lat": 41.0
                    }
                },
                "available_products": [
                    {
                        "kind": "ndvi",
                        "filename": "ndvi.png",
                        "content_type": "image/png",
                        "url_path": "/api/scenes/scene-1/products/ndvi",
                        "tile_url_template": "/api/scenes/scene-1/products/ndvi/tiles/{z}/{x}/{y}.png"
                    }
                ]
            }"#,
        )
        .expect("scene manifest should decode");

        assert_eq!(manifest.scene_id, "scene-1");
        assert!(manifest.geospatial.georeferenced);
        assert_eq!(manifest.geospatial.crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(manifest.available_products.len(), 1);
        assert_eq!(manifest.available_products[0].kind, "ndvi");
    }

    #[test]
    fn product_placement_refuses_not_georeferenced_manifest() {
        let placement = product_placement_for_manifest(&SceneGeospatialMetadata {
            georeferenced: false,
            crs: None,
            center: None,
            extent: None,
        });

        assert!(!placement.is_placeable());
        assert!(placement
            .reason()
            .expect("unplaceable reason")
            .contains("not georeferenced"));
    }

    #[test]
    fn manifest_world_dimensions_do_not_fabricate_ungeoreferenced_ground() {
        let dimensions = manifest_world_dimensions(
            &SceneGeospatialMetadata {
                georeferenced: false,
                crs: None,
                center: None,
                extent: None,
            },
            Some(256),
            Some(128),
        );

        assert_eq!(dimensions, Vec2::ZERO);
    }

    #[test]
    fn manifest_world_dimensions_use_extent_when_georeferenced() {
        let dimensions = manifest_world_dimensions(
            &SceneGeospatialMetadata {
                georeferenced: true,
                crs: Some("EPSG:4326".to_string()),
                center: None,
                extent: Some(SceneExtent {
                    min_lon: -89.5,
                    min_lat: 40.0,
                    max_lon: -88.5,
                    max_lat: 41.0,
                }),
            },
            Some(256),
            Some(128),
        );

        assert_ne!(dimensions, Vec2::ZERO);
    }

    #[test]
    fn selecting_linked_scene_records_field_season_and_owner_context() {
        let mut catalog = FieldCatalogState::default();
        let scene = FieldSceneSummary {
            scene_id: "scene-1".to_string(),
            owner: Some("org-alpha".to_string()),
            sensor: "landsat8".to_string(),
            acquired_at: "2026-05-01T00:00:00Z".to_string(),
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            linked_at: Some("2026-05-01T00:00:00Z".to_string()),
        };

        let selection =
            select_catalog_scene(&mut catalog, &scene).expect("linked scene should select");

        assert_eq!(selection.scene_id, "scene-1");
        assert_eq!(selection.field_id, "field-1");
        assert_eq!(selection.season_id, "2026");
        assert_eq!(selection.owner, "org-alpha");
        assert_eq!(catalog.selected_scene_id.as_deref(), Some("scene-1"));
        assert_eq!(catalog.selected_field_id.as_deref(), Some("field-1"));
        assert_eq!(catalog.selected_season_id.as_deref(), Some("2026"));
        assert_eq!(catalog.selected_owner.as_deref(), Some("org-alpha"));
    }

    #[test]
    fn selecting_unlinked_scene_is_refused() {
        let mut catalog = FieldCatalogState::default();
        let scene = FieldSceneSummary {
            scene_id: "scene-unlinked".to_string(),
            owner: Some("org-alpha".to_string()),
            sensor: "landsat8".to_string(),
            acquired_at: "2026-05-01T00:00:00Z".to_string(),
            field_id: None,
            season_id: Some("2026".to_string()),
            linked_at: None,
        };

        let err = select_catalog_scene(&mut catalog, &scene)
            .expect_err("unlinked scene should be refused");

        assert!(err.to_string().contains("unlinked scene"));
        assert!(catalog.selected_scene_id.is_none());
        assert!(catalog.selected_field_id.is_none());
        assert!(catalog.selected_season_id.is_none());
        assert!(catalog.selected_owner.is_none());
    }
}
