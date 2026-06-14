use anyhow::Result;
use bevy::prelude::*;
use bevy::tasks::Task;
use image::{DynamicImage, Rgba, RgbaImage};
use sensor_overlay_engine::{utils::OverlayValueRange, SpatialBounds};
use serde::{Deserialize, Serialize};
use shared::schemas::{
    assert_raster_spatial_ref, AnnotationGeometry, AnnotationRecord, FarmRecord, FieldRecord,
    GeoPoint, GpsCoords, RasterResolution, RasterSpatialRef, RecommendationPriority,
    RecommendationRecord, RecommendationStatus, ReportRecord, GEO_EXTENT_ASSERTION_TOLERANCE,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

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
pub struct AnnotationCreateTask(pub Option<Task<AnnotationCreateResult>>);

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
    #[serde(default)]
    pub product_id: Option<String>,
    pub kind: String,
    #[serde(default)]
    pub field_id: Option<String>,
    #[serde(default)]
    pub season_id: Option<String>,
    pub filename: String,
    pub content_type: String,
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
    #[serde(default)]
    pub source_image_ids: Vec<String>,
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
    #[serde(default)]
    pub spatial_ref: Option<RasterSpatialRef>,
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
    assert_manifest_layer_placement(geospatial, width, height)
        .map(|placement| placement.world_dimensions)
        .unwrap_or(Vec2::ZERO)
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerPlacement {
    pub crs: String,
    pub extent: SceneExtent,
    pub resolution: RasterResolution,
    pub world_dimensions: Vec2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerMetadataReadout {
    pub crs: String,
    pub extent: String,
    pub resolution: String,
    pub dimensions: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveProductSelection {
    pub selected_layer: usize,
    pub product_kind: String,
    pub tile_source: TileSource,
    pub legend: ProductLegend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompareLayout {
    #[default]
    Swipe,
    SideBySide,
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct CompareModeState {
    pub active: Option<CompareModeSession>,
    pub right_product_index: usize,
    pub layout: CompareLayout,
    pub last_error: Option<String>,
}

impl Default for CompareModeState {
    fn default() -> Self {
        Self {
            active: None,
            right_product_index: 1,
            layout: CompareLayout::Swipe,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareModeSession {
    pub left: CompareLayer,
    pub right: CompareLayer,
    pub placement: LayerPlacement,
    pub shared_view: CompareSharedView,
    pub layout: CompareLayout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareLayer {
    pub scene_id: String,
    pub product_index: usize,
    pub product_kind: String,
    pub tile_source: TileSource,
    pub field_id: Option<String>,
    pub season_id: Option<String>,
    pub acquired_at: Option<String>,
    pub placement: LayerPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompareSharedView {
    pub center: Vec2,
    pub zoom_level: f32,
    pub divider_fraction: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn open_compare_mode(
    left_manifest: &SceneManifestState,
    left_product_index: usize,
    right_manifest: &SceneManifestState,
    right_product_index: usize,
    base_url: &str,
    viewer_state: &ViewerState,
    map_view: &MapViewState,
    layout: CompareLayout,
) -> Result<CompareModeSession> {
    let left = compare_layer_for_manifest("left", left_manifest, left_product_index, base_url)?;
    let right = compare_layer_for_manifest("right", right_manifest, right_product_index, base_url)?;
    assert_compare_field_match(&left, &right)?;
    assert_compare_placement_match(&left.placement, &right.placement)?;

    Ok(CompareModeSession {
        placement: left.placement.clone(),
        shared_view: CompareSharedView {
            center: map_view.center,
            zoom_level: viewer_state.zoom_level,
            divider_fraction: 0.5,
        },
        layout,
        left,
        right,
    })
}

pub fn sync_compare_shared_view(
    session: &mut CompareModeSession,
    viewer_state: &ViewerState,
    map_view: &MapViewState,
) {
    session.shared_view.center = map_view.center;
    session.shared_view.zoom_level = viewer_state.zoom_level;
}

pub fn set_compare_divider(session: &mut CompareModeSession, divider_fraction: f32) {
    session.shared_view.divider_fraction = if divider_fraction.is_finite() {
        divider_fraction.clamp(0.0, 1.0)
    } else {
        0.5
    };
}

fn compare_layer_for_manifest(
    side: &'static str,
    manifest_state: &SceneManifestState,
    product_index: usize,
    base_url: &str,
) -> Result<CompareLayer> {
    let scene_id = required_compare_text(manifest_state.scene_id.as_deref(), side, "scene_id")?;
    let product = manifest_state.products.get(product_index).ok_or_else(|| {
        anyhow::anyhow!("compare {side} product index {product_index} is not available")
    })?;
    if product.kind.trim().is_empty() {
        anyhow::bail!("compare {side} product kind is required");
    }
    if product.tile_url_template.trim().is_empty() {
        anyhow::bail!(
            "compare {side} product {} is missing a tile URL template",
            product.kind
        );
    }

    let placement = assert_manifest_layer_placement(
        &manifest_state.geospatial,
        manifest_state.width,
        manifest_state.height,
    )
    .map_err(|err| anyhow::anyhow!("compare {side} layer assertion failed: {err}"))?;
    assert_product_provenance_alignment(product, manifest_state)
        .map_err(|err| anyhow::anyhow!("compare {side} product assertion failed: {err}"))?;

    Ok(CompareLayer {
        scene_id,
        product_index,
        product_kind: product.kind.clone(),
        tile_source: product.tile_source(base_url),
        field_id: non_empty_owned(manifest_state.field_id.as_deref())
            .or_else(|| non_empty_owned(product.field_id.as_deref())),
        season_id: non_empty_owned(manifest_state.season_id.as_deref())
            .or_else(|| non_empty_owned(product.season_id.as_deref())),
        acquired_at: non_empty_owned(manifest_state.acquired_at.as_deref()),
        placement,
    })
}

fn required_compare_text(value: Option<&str>, side: &str, label: &str) -> Result<String> {
    non_empty_owned(value).ok_or_else(|| anyhow::anyhow!("compare {side} {label} is required"))
}

fn non_empty_owned(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn assert_compare_field_match(left: &CompareLayer, right: &CompareLayer) -> Result<()> {
    let left_field = left
        .field_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("compare left field_id is required"))?;
    let right_field = right
        .field_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("compare right field_id is required"))?;
    if left_field != right_field {
        anyhow::bail!(
            "compare mismatch: field_id {left_field} cannot share view with {right_field}"
        );
    }
    Ok(())
}

fn assert_compare_placement_match(left: &LayerPlacement, right: &LayerPlacement) -> Result<()> {
    if left.crs != right.crs {
        anyhow::bail!(
            "compare mismatch: CRS {} cannot share view with {}",
            left.crs,
            right.crs
        );
    }
    assert_compare_extent_edge("min_lon", left.extent.min_lon, right.extent.min_lon)?;
    assert_compare_extent_edge("min_lat", left.extent.min_lat, right.extent.min_lat)?;
    assert_compare_extent_edge("max_lon", left.extent.max_lon, right.extent.max_lon)?;
    assert_compare_extent_edge("max_lat", left.extent.max_lat, right.extent.max_lat)?;
    Ok(())
}

fn assert_compare_extent_edge(edge: &'static str, left: f64, right: f64) -> Result<()> {
    if (left - right).abs() > GEO_EXTENT_ASSERTION_TOLERANCE {
        anyhow::bail!("compare mismatch: extent {edge} {left} cannot share view with {right}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductLegend {
    pub product_kind: String,
    pub colormap: String,
    pub value_range: Option<OverlayValueRange>,
    pub stops: Vec<ProductLegendStop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductLegendStop {
    pub value: f32,
    pub color: [u8; 3],
    pub alpha: u8,
}

pub fn layer_metadata_readout(
    geospatial: &SceneGeospatialMetadata,
    width: Option<u32>,
    height: Option<u32>,
) -> LayerMetadataReadout {
    let crs = geospatial
        .crs
        .as_deref()
        .filter(|crs| !crs.trim().is_empty())
        .unwrap_or("missing CRS")
        .to_string();
    let extent = geospatial
        .extent
        .as_ref()
        .map(|extent| {
            format!(
                "{:.5}, {:.5} -> {:.5}, {:.5}",
                extent.min_lon, extent.min_lat, extent.max_lon, extent.max_lat
            )
        })
        .unwrap_or_else(|| "missing extent".to_string());
    let resolution = geospatial
        .spatial_ref
        .as_ref()
        .and_then(|spatial_ref| spatial_ref.resolution)
        .map(|resolution| format!("{:.8} x {:.8}", resolution.x, resolution.y))
        .unwrap_or_else(|| "missing resolution".to_string());
    let dimensions = width
        .zip(height)
        .map(|(width, height)| format!("{width}x{height} px"))
        .unwrap_or_else(|| "missing dimensions".to_string());

    LayerMetadataReadout {
        crs,
        extent,
        resolution,
        dimensions,
    }
}

pub fn active_product_selection(
    manifest_state: &SceneManifestState,
    config: &TileConfig,
    target_idx: usize,
) -> Result<ActiveProductSelection> {
    let placement = assert_manifest_layer_placement(
        &manifest_state.geospatial,
        manifest_state.width,
        manifest_state.height,
    )?;
    let product = manifest_state
        .products
        .get(target_idx)
        .ok_or_else(|| anyhow::anyhow!("product index {target_idx} is not available"))?;
    if product.kind.trim().is_empty() {
        anyhow::bail!("product kind is required before switching layers");
    }
    if product.tile_url_template.trim().is_empty() {
        anyhow::bail!("product {} is missing a tile URL template", product.kind);
    }
    assert_product_provenance_alignment(product, manifest_state)?;

    Ok(ActiveProductSelection {
        selected_layer: target_idx,
        product_kind: product.kind.clone(),
        tile_source: product.tile_source(&config.base_url),
        legend: product_legend_for_kind(&product.kind, &placement.extent)?,
    })
}

fn assert_product_provenance_alignment(
    product: &SceneProduct,
    manifest_state: &SceneManifestState,
) -> Result<()> {
    let product_label = product
        .product_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(product.kind.as_str());

    if product
        .product_id
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        anyhow::bail!("product {} has an empty product_id", product.kind);
    }
    for source_image_id in &product.source_image_ids {
        if source_image_id.trim().is_empty() {
            anyhow::bail!("product {product_label} has an empty source image id");
        }
    }
    if let Some(field_id) = product
        .field_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let manifest_field = manifest_state
            .field_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "product {product_label} declares field_id but manifest is unlinked"
                )
            })?;
        if manifest_field != field_id {
            anyhow::bail!(
                "product {product_label} field_id {field_id} does not match manifest field_id {manifest_field}"
            );
        }
    }
    if let Some(season_id) = product
        .season_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let manifest_season = manifest_state
            .season_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "product {product_label} declares season_id but manifest has no season_id"
                )
            })?;
        if manifest_season != season_id {
            anyhow::bail!(
                "product {product_label} season_id {season_id} does not match manifest season_id {manifest_season}"
            );
        }
    }
    if let Some(spatial_ref) = product.spatial_ref.as_ref() {
        let width = manifest_state.width.ok_or_else(|| {
            anyhow::anyhow!(
                "product {product_label} declares spatial_ref but manifest width is missing"
            )
        })?;
        let height = manifest_state.height.ok_or_else(|| {
            anyhow::anyhow!(
                "product {product_label} declares spatial_ref but manifest height is missing"
            )
        })?;
        let asserted =
            assert_raster_spatial_ref(Some(spatial_ref), width, height).map_err(|err| {
                anyhow::anyhow!("product {product_label} spatial_ref assertion failed: {err}")
            })?;
        let manifest_crs = manifest_state
            .geospatial
            .crs
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "product {product_label} declares spatial_ref but manifest CRS is missing"
                )
            })?;
        let product_crs = asserted
            .crs
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("product {product_label} spatial_ref missing CRS"))?;
        if manifest_crs != product_crs {
            anyhow::bail!(
                "product {product_label} CRS mismatch: manifest {manifest_crs} != product {product_crs}"
            );
        }
        let manifest_extent = manifest_state.geospatial.extent.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "product {product_label} declares spatial_ref but manifest extent is missing"
            )
        })?;
        let product_extent = asserted
            .bbox
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("product {product_label} spatial_ref missing extent"))?;
        assert_extent_edge("min_lon", manifest_extent.min_lon, product_extent.min_lon)?;
        assert_extent_edge("min_lat", manifest_extent.min_lat, product_extent.min_lat)?;
        assert_extent_edge("max_lon", manifest_extent.max_lon, product_extent.max_lon)?;
        assert_extent_edge("max_lat", manifest_extent.max_lat, product_extent.max_lat)?;
    }

    Ok(())
}

pub fn switch_active_product(
    manifest_state: &SceneManifestState,
    viewer_state: &mut ViewerState,
    config: &mut TileConfig,
    target_idx: usize,
) -> Result<ActiveProductSelection> {
    let selection = active_product_selection(manifest_state, config, target_idx)?;
    viewer_state.selected_layer = selection.selected_layer;
    config.product_kind = selection.product_kind.clone();
    Ok(selection)
}

pub fn product_legend_for_kind(kind: &str, extent: &SceneExtent) -> Result<ProductLegend> {
    let (colormap, value_range) = legend_profile_for_product(kind);
    let values = legend_sample_values(value_range, 5);
    let spatial_bounds = SpatialBounds {
        min_x: extent.min_lon,
        min_y: extent.min_lat,
        max_x: extent.max_lon,
        max_y: extent.max_lat,
        min_z: None,
        max_z: None,
    };
    let rendered = sensor_overlay_engine::utils::render_value_overlay(
        &values,
        values.len() as u32,
        1,
        &spatial_bounds,
        colormap,
        Some(value_range),
        5,
    )?;
    let metadata = rendered.metadata;
    let stops = metadata
        .legend_stops
        .into_iter()
        .map(|stop| ProductLegendStop {
            value: stop.value,
            color: [stop.color.r, stop.color.g, stop.color.b],
            alpha: stop.alpha,
        })
        .collect();

    Ok(ProductLegend {
        product_kind: kind.to_string(),
        colormap: metadata.colormap,
        value_range: metadata.value_range,
        stops,
    })
}

fn legend_profile_for_product(kind: &str) -> (&'static str, (f32, f32)) {
    let normalized = kind.to_ascii_lowercase();
    if normalized.contains("ndvi") || normalized.contains("vegetation") {
        ("viridis", (-1.0, 1.0))
    } else if normalized.contains("thermal") || normalized.contains("temperature") {
        ("hot", (-10.0, 50.0))
    } else if normalized.contains("source")
        || normalized.contains("rgb")
        || normalized.contains("visual")
    {
        ("grayscale", (0.0, 255.0))
    } else if normalized.contains("lidar")
        || normalized.contains("elevation")
        || normalized == "dsm"
        || normalized == "dtm"
        || normalized == "chm"
    {
        ("jet", (0.0, 1.0))
    } else if normalized.contains("occupancy")
        || normalized.contains("obstacle")
        || normalized.contains("density")
    {
        ("hot", (0.0, 1.0))
    } else {
        ("grayscale", (0.0, 1.0))
    }
}

fn legend_sample_values((min, max): (f32, f32), count: usize) -> Vec<f32> {
    if count <= 1 {
        return vec![min];
    }
    let last = count - 1;
    (0..count)
        .map(|idx| min + (max - min) * (idx as f32 / last as f32))
        .collect()
}

pub fn assert_manifest_layer_placement(
    geospatial: &SceneGeospatialMetadata,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<LayerPlacement> {
    let product_placement = product_placement_for_manifest(geospatial);
    if !product_placement.is_placeable() {
        anyhow::bail!(
            "{}",
            product_placement
                .reason()
                .unwrap_or("scene products are unplaceable")
        );
    }

    let width = width.ok_or_else(|| anyhow::anyhow!("layer placement missing raster width"))?;
    let height = height.ok_or_else(|| anyhow::anyhow!("layer placement missing raster height"))?;
    let spatial_ref = geospatial
        .spatial_ref
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("layer placement missing asserted spatial_ref"))?;
    let asserted = assert_raster_spatial_ref(Some(spatial_ref), width, height)?;
    let manifest_crs = geospatial
        .crs
        .as_deref()
        .filter(|crs| !crs.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("layer placement missing manifest CRS"))?;
    let layer_crs = asserted
        .crs
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("layer placement missing layer CRS"))?;
    if manifest_crs != layer_crs {
        anyhow::bail!("layer placement CRS mismatch: manifest {manifest_crs} != layer {layer_crs}");
    }

    let manifest_extent = geospatial
        .extent
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("layer placement missing manifest extent"))?;
    let layer_extent = asserted
        .bbox
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("layer placement missing layer extent"))?;
    assert_extent_edge("min_lon", manifest_extent.min_lon, layer_extent.min_lon)?;
    assert_extent_edge("min_lat", manifest_extent.min_lat, layer_extent.min_lat)?;
    assert_extent_edge("max_lon", manifest_extent.max_lon, layer_extent.max_lon)?;
    assert_extent_edge("max_lat", manifest_extent.max_lat, layer_extent.max_lat)?;

    let resolution = asserted
        .resolution
        .ok_or_else(|| anyhow::anyhow!("layer placement missing layer resolution"))?;

    Ok(LayerPlacement {
        crs: layer_crs.to_string(),
        extent: manifest_extent.clone(),
        resolution,
        world_dimensions: scene_extent_world_dimensions(manifest_extent),
    })
}

fn assert_extent_edge(edge: &'static str, manifest: f64, layer: f64) -> Result<()> {
    if (manifest - layer).abs() > GEO_EXTENT_ASSERTION_TOLERANCE {
        anyhow::bail!(
            "layer placement extent mismatch at {edge}: manifest {manifest} != layer {layer}"
        );
    }
    Ok(())
}

fn scene_extent_world_dimensions(extent: &SceneExtent) -> Vec2 {
    Vec2::new(
        ((extent.max_lon - extent.min_lon) as f32).abs() * MAP_UNITS_PER_DEGREE,
        ((extent.max_lat - extent.min_lat) as f32).abs() * MAP_UNITS_PER_DEGREE,
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub failed_commits: Vec<PendingAnnotationCommit>,
    pub last_error: Option<String>,
    pub draft_author: String,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnnotationCommitPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_id: Option<String>,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    pub geometry: AnnotationGeometry,
    pub author: String,
    pub crs: String,
    pub audit_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingAnnotationCommit {
    pub payload: AnnotationCommitPayload,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationCreateResult {
    Saved(AnnotationRecord),
    Failed(PendingAnnotationCommit),
}

impl Default for AnnotationOverlayState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_annotation_id: None,
            hovered_annotation_id: None,
            failed_commits: Vec::new(),
            last_error: None,
            draft_author: "geo_viewer".to_string(),
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
    pub zones: Vec<ReportZoneOverlay>,
    pub draft_title: String,
    pub last_overlay_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportFindingZone {
    pub report_id: String,
    pub finding_id: String,
    pub zone_id: String,
    pub crs: String,
    pub coordinates: Vec<GeoPoint>,
    pub reason: String,
    pub priority: RecommendationPriority,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportZoneOverlay {
    pub report_id: String,
    pub finding_id: String,
    pub zone_id: String,
    pub crs: String,
    pub coordinates: Vec<GeoPoint>,
    pub world_polygon: Vec<Vec2>,
    pub reason: String,
    pub priority: RecommendationPriority,
    pub label: String,
}

pub fn build_report_result_overlay(
    report: &ReportRecord,
    zones: &[ReportFindingZone],
    active_layer: &LayerPlacement,
) -> Result<Vec<ReportZoneOverlay>> {
    let report_id = normalize_viewer_text(&report.report_id)
        .ok_or_else(|| anyhow::anyhow!("report_id is required for report overlay"))?;
    if zones.is_empty() {
        return Ok(Vec::new());
    }

    zones
        .iter()
        .map(|zone| build_report_zone_overlay(&report_id, zone, active_layer))
        .collect()
}

fn build_report_zone_overlay(
    report_id: &str,
    zone: &ReportFindingZone,
    active_layer: &LayerPlacement,
) -> Result<ReportZoneOverlay> {
    if normalize_viewer_text(&zone.report_id).as_deref() != Some(report_id) {
        anyhow::bail!(
            "report zone {} belongs to report {}, not {}",
            zone.zone_id,
            zone.report_id,
            report_id
        );
    }
    let finding_id = normalize_viewer_text(&zone.finding_id)
        .ok_or_else(|| anyhow::anyhow!("report zone finding_id is required"))?;
    let zone_id = normalize_viewer_text(&zone.zone_id)
        .ok_or_else(|| anyhow::anyhow!("report zone zone_id is required"))?;
    let zone_crs = normalize_viewer_text(&zone.crs)
        .ok_or_else(|| anyhow::anyhow!("report zone {zone_id} CRS is required"))?;
    if zone_crs != active_layer.crs {
        anyhow::bail!(
            "report zone {zone_id} CRS mismatch: active layer {} != zone {}",
            active_layer.crs,
            zone_crs
        );
    }
    if zone.coordinates.len() < 3 {
        anyhow::bail!("report zone {zone_id} requires at least three polygon coordinates");
    }
    let reason = normalize_viewer_text(&zone.reason)
        .ok_or_else(|| anyhow::anyhow!("report zone {zone_id} reason is required"))?;
    let world_polygon = zone
        .coordinates
        .iter()
        .map(|point| report_zone_point_to_world(&active_layer.extent, point))
        .collect::<Result<Vec<_>>>()?;
    let label = format!(
        "{}: {}",
        recommendation_priority_label(zone.priority),
        reason
    );

    Ok(ReportZoneOverlay {
        report_id: report_id.to_string(),
        finding_id,
        zone_id,
        crs: zone_crs,
        coordinates: zone.coordinates.clone(),
        world_polygon,
        reason,
        priority: zone.priority,
        label,
    })
}

fn report_zone_point_to_world(extent: &SceneExtent, point: &GeoPoint) -> Result<Vec2> {
    if !point.longitude.is_finite() || !point.latitude.is_finite() {
        anyhow::bail!("report zone coordinate must be finite");
    }
    let center_lon = (extent.min_lon + extent.max_lon) / 2.0;
    let center_lat = (extent.min_lat + extent.max_lat) / 2.0;
    Ok(Vec2::new(
        ((point.longitude - center_lon) as f32) * MAP_UNITS_PER_DEGREE,
        ((point.latitude - center_lat) as f32) * MAP_UNITS_PER_DEGREE,
    ))
}

fn recommendation_priority_label(priority: RecommendationPriority) -> &'static str {
    match priority {
        RecommendationPriority::Low => "low",
        RecommendationPriority::Medium => "medium",
        RecommendationPriority::High => "high",
        RecommendationPriority::Critical => "critical",
    }
}

fn normalize_viewer_text(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SavedViewState {
    pub draft_name: String,
    pub saved_view_path: String,
    pub snapshot_image_path: String,
    pub snapshot_metadata_path: String,
    pub last_saved: Option<SavedView>,
    pub last_snapshot: Option<SnapshotExportMetadata>,
    pub last_error: Option<String>,
}

impl Default for SavedViewState {
    fn default() -> Self {
        let temp_dir = std::env::temp_dir();
        Self {
            draft_name: "Field review".to_string(),
            saved_view_path: temp_dir
                .join("agbot_geo_viewer_saved_view.json")
                .to_string_lossy()
                .to_string(),
            snapshot_image_path: temp_dir
                .join("agbot_geo_viewer_snapshot.png")
                .to_string_lossy()
                .to_string(),
            snapshot_metadata_path: temp_dir
                .join("agbot_geo_viewer_snapshot.json")
                .to_string_lossy()
                .to_string(),
            last_saved: None,
            last_snapshot: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    pub name: String,
    pub field_id: Option<String>,
    pub scene_id: Option<String>,
    pub season_id: Option<String>,
    pub active_product: String,
    pub selected_layer: usize,
    pub camera: SavedCameraState,
    pub geospatial: SavedGeospatialState,
    pub overlays: SavedOverlayState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedCameraState {
    pub center_x: f32,
    pub center_y: f32,
    pub zoom_level: f32,
    pub base_scale: f32,
    pub needs_fit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedGeospatialState {
    pub georeferenced: bool,
    pub crs: Option<String>,
    pub extent: Option<SceneExtent>,
    pub resolution: Option<RasterResolution>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedOverlayState {
    pub selected_annotation_id: Option<String>,
    pub annotation_filter_label: String,
    pub show_points: bool,
    pub show_polygons: bool,
    pub show_low: bool,
    pub show_medium: bool,
    pub show_high: bool,
    pub show_critical: bool,
    pub show_other: bool,
    pub selected_recommendation_id: Option<String>,
    pub recommendation_status_filter: Option<RecommendationStatus>,
    pub recommendation_priority_filter: Option<RecommendationPriority>,
    pub report_draft_title: String,
    pub annotation_count: usize,
    pub recommendation_count: usize,
    pub report_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotExportMetadata {
    pub view: SavedView,
    pub image_path: String,
    pub metadata_path: String,
    pub width: u32,
    pub height: u32,
    pub georeferenced: bool,
    pub georeference_label: String,
    pub crs: Option<String>,
    pub extent: Option<SceneExtent>,
    pub georeference_warning: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn capture_saved_view(
    name: &str,
    config: &TileConfig,
    viewer_state: &ViewerState,
    map_view: &MapViewState,
    manifest_state: &SceneManifestState,
    annotations: &AnnotationOverlayState,
    recommendations: &RecommendationOverlayState,
    reports: &ReportOverlayState,
) -> Result<SavedView> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("saved view name is required");
    }

    let active_product = manifest_state
        .products
        .get(viewer_state.selected_layer)
        .map(|product| product.kind.clone())
        .filter(|kind| !kind.trim().is_empty())
        .unwrap_or_else(|| config.product_kind.clone());

    Ok(SavedView {
        name: name.to_string(),
        field_id: non_empty_owned(manifest_state.field_id.as_deref()).or_else(|| {
            manifest_state
                .field
                .as_ref()
                .and_then(|field| non_empty_owned(Some(field.field_id.as_str())))
        }),
        scene_id: non_empty_owned(config.scene_id.as_deref())
            .or_else(|| non_empty_owned(manifest_state.scene_id.as_deref())),
        season_id: non_empty_owned(manifest_state.season_id.as_deref()),
        active_product,
        selected_layer: viewer_state.selected_layer,
        camera: SavedCameraState {
            center_x: map_view.center.x,
            center_y: map_view.center.y,
            zoom_level: viewer_state.zoom_level,
            base_scale: map_view.base_scale,
            needs_fit: map_view.needs_fit,
        },
        geospatial: saved_geospatial_state(manifest_state),
        overlays: SavedOverlayState {
            selected_annotation_id: annotations.selected_annotation_id.clone(),
            annotation_filter_label: annotations.filter_label.clone(),
            show_points: annotations.show_points,
            show_polygons: annotations.show_polygons,
            show_low: annotations.show_low,
            show_medium: annotations.show_medium,
            show_high: annotations.show_high,
            show_critical: annotations.show_critical,
            show_other: annotations.show_other,
            selected_recommendation_id: recommendations.selected_recommendation_id.clone(),
            recommendation_status_filter: recommendations.status_filter,
            recommendation_priority_filter: recommendations.priority_filter,
            report_draft_title: reports.draft_title.clone(),
            annotation_count: annotations.items.len(),
            recommendation_count: recommendations.items.len(),
            report_count: reports.items.len(),
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub fn restore_saved_view(
    view: &SavedView,
    catalog: &mut FieldCatalogState,
    config: &mut TileConfig,
    viewer_state: &mut ViewerState,
    map_view: &mut MapViewState,
    annotations: &mut AnnotationOverlayState,
    recommendations: &mut RecommendationOverlayState,
    reports: &mut ReportOverlayState,
) -> Result<()> {
    if view.active_product.trim().is_empty() {
        anyhow::bail!("saved view active_product is required");
    }

    catalog.selected_field_id = view.field_id.clone();
    catalog.selected_scene_id = view.scene_id.clone();
    catalog.selected_season_id = view.season_id.clone();
    config.scene_id = view.scene_id.clone();
    config.product_kind = view.active_product.clone();
    viewer_state.selected_layer = view.selected_layer;
    viewer_state.zoom_level = view.camera.zoom_level;
    viewer_state.scene_id_input = view.scene_id.clone().unwrap_or_default();
    map_view.center = Vec2::new(view.camera.center_x, view.camera.center_y);
    map_view.base_scale = view.camera.base_scale;
    map_view.needs_fit = view.camera.needs_fit;

    annotations.selected_annotation_id = view.overlays.selected_annotation_id.clone();
    annotations.filter_label = view.overlays.annotation_filter_label.clone();
    annotations.show_points = view.overlays.show_points;
    annotations.show_polygons = view.overlays.show_polygons;
    annotations.show_low = view.overlays.show_low;
    annotations.show_medium = view.overlays.show_medium;
    annotations.show_high = view.overlays.show_high;
    annotations.show_critical = view.overlays.show_critical;
    annotations.show_other = view.overlays.show_other;
    recommendations.selected_recommendation_id = view.overlays.selected_recommendation_id.clone();
    recommendations.status_filter = view.overlays.recommendation_status_filter;
    recommendations.priority_filter = view.overlays.recommendation_priority_filter;
    reports.draft_title = view.overlays.report_draft_title.clone();

    Ok(())
}

pub fn save_view_to_json(path: impl AsRef<Path>, view: &SavedView) -> Result<()> {
    write_json_file(path, view)
}

pub fn load_view_from_json(path: impl AsRef<Path>) -> Result<SavedView> {
    let bytes = std::fs::read(path.as_ref())?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn export_snapshot(
    view: &SavedView,
    manifest_state: &SceneManifestState,
    tile_state: &TileRenderState,
    image_path: impl AsRef<Path>,
    metadata_path: impl AsRef<Path>,
) -> Result<SnapshotExportMetadata> {
    let geospatial = saved_geospatial_state(manifest_state);
    let width = snapshot_dimension(tile_state.image_dimensions.x, 640);
    let height = snapshot_dimension(tile_state.image_dimensions.y, 360);
    let image_path = image_path.as_ref();
    let metadata_path = metadata_path.as_ref();
    ensure_parent_dir(image_path)?;
    ensure_parent_dir(metadata_path)?;

    let metadata = SnapshotExportMetadata {
        view: view.clone(),
        image_path: image_path.to_string_lossy().to_string(),
        metadata_path: metadata_path.to_string_lossy().to_string(),
        width,
        height,
        georeferenced: geospatial.georeferenced,
        georeference_label: if geospatial.georeferenced {
            "georeferenced"
        } else {
            "non_georeferenced"
        }
        .to_string(),
        crs: geospatial.crs.clone(),
        extent: geospatial.extent.clone(),
        georeference_warning: geospatial.warning.clone(),
    };

    let mut image = RgbaImage::from_pixel(
        width,
        height,
        if metadata.georeferenced {
            Rgba([34, 112, 84, 255])
        } else {
            Rgba([128, 38, 38, 255])
        },
    );
    if !metadata.georeferenced {
        for y in 0..height.min(8) {
            for x in 0..width {
                image.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
    }
    DynamicImage::ImageRgba8(image).save(image_path)?;
    write_json_file(metadata_path, &metadata)?;

    Ok(metadata)
}

fn saved_geospatial_state(manifest_state: &SceneManifestState) -> SavedGeospatialState {
    match assert_manifest_layer_placement(
        &manifest_state.geospatial,
        manifest_state.width,
        manifest_state.height,
    ) {
        Ok(placement) => SavedGeospatialState {
            georeferenced: true,
            crs: Some(placement.crs),
            extent: Some(placement.extent),
            resolution: Some(placement.resolution),
            warning: None,
        },
        Err(err) => SavedGeospatialState {
            georeferenced: false,
            crs: manifest_state.geospatial.crs.clone(),
            extent: manifest_state.geospatial.extent.clone(),
            resolution: manifest_state
                .geospatial
                .spatial_ref
                .as_ref()
                .and_then(|spatial_ref| spatial_ref.resolution),
            warning: Some(err.to_string()),
        },
    }
}

fn write_json_file(path: impl AsRef<Path>, value: &impl Serialize) -> Result<()> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn snapshot_dimension(value: f32, fallback: u32) -> u32 {
    if value.is_finite() && value > 0.0 {
        (value.round() as u32).clamp(1, 2048)
    } else {
        fallback
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
    use super::{
        assert_manifest_layer_placement, build_report_result_overlay, capture_saved_view,
        export_snapshot, layer_metadata_readout, load_view_from_json, manifest_world_dimensions,
        open_compare_mode, product_legend_for_kind, product_placement_for_manifest,
        restore_saved_view, save_view_to_json, select_catalog_scene, set_compare_divider,
        summarize_tile_presences, switch_active_product, sync_compare_shared_view,
        AnnotationOverlayState, CompareLayout, FieldCatalogState, FieldSceneSummary,
        LayerPlacement, MapViewState, RecommendationOverlayState, ReportFindingZone,
        ReportOverlayState, SceneExtent, SceneGeospatialMetadata, SceneManifest,
        SceneManifestState, SceneProduct, TileConfig, TileId, TilePresence, TileRenderState,
        TileStatus, ViewerState, DEFAULT_PRODUCT_KIND, DEFAULT_TILE_ZOOM,
    };
    use bevy::prelude::Vec2;
    use shared::schemas::{
        GeoBounds, GeoPoint, RasterResolution, RasterSpatialRef, RecommendationPriority,
        RecommendationStatus, ReportFormat, ReportRecord, ReportVisibility,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    #[test]
    fn tile_source_formats_tile_url_from_template() {
        let product = SceneProduct {
            product_id: None,
            kind: "ndvi".to_string(),
            field_id: None,
            season_id: None,
            filename: "ndvi.png".to_string(),
            content_type: "image/png".to_string(),
            spatial_ref: None,
            source_image_ids: Vec::new(),
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
                        "product_id": "scene-1:ndvi",
                        "kind": "ndvi",
                        "field_id": "field-1",
                        "season_id": "2026",
                        "filename": "ndvi.png",
                        "content_type": "image/png",
                        "spatial_ref": {
                            "georeferenced": true,
                            "crs": "EPSG:4326",
                            "bbox": {
                                "min_lon": -89.5,
                                "min_lat": 40.0,
                                "max_lon": -88.5,
                                "max_lat": 41.0
                            },
                            "geo_transform": [-89.5, 0.01, 0.0, 41.0, 0.0, -0.01],
                            "resolution": {
                                "x": 0.01,
                                "y": 0.01
                            }
                        },
                        "source_image_ids": ["image-1"],
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
        assert_eq!(
            manifest.available_products[0].product_id.as_deref(),
            Some("scene-1:ndvi")
        );
        assert_eq!(
            manifest.available_products[0].field_id.as_deref(),
            Some("field-1")
        );
        assert_eq!(
            manifest.available_products[0].season_id.as_deref(),
            Some("2026")
        );
        assert_eq!(
            manifest.available_products[0].source_image_ids,
            vec!["image-1".to_string()]
        );
        assert_eq!(
            manifest.available_products[0]
                .spatial_ref
                .as_ref()
                .and_then(|spatial_ref| spatial_ref.crs.as_deref()),
            Some("EPSG:4326")
        );
    }

    #[test]
    fn product_placement_refuses_not_georeferenced_manifest() {
        let placement = product_placement_for_manifest(&SceneGeospatialMetadata {
            georeferenced: false,
            crs: None,
            center: None,
            extent: None,
            spatial_ref: None,
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
                spatial_ref: None,
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
                spatial_ref: Some(valid_spatial_ref()),
            },
            Some(100),
            Some(50),
        );

        assert_ne!(dimensions, Vec2::ZERO);
    }

    #[test]
    fn layer_placement_assertion_accepts_matching_manifest_spatial_ref() {
        let placement = assert_manifest_layer_placement(
            &SceneGeospatialMetadata {
                georeferenced: true,
                crs: Some("EPSG:4326".to_string()),
                center: None,
                extent: Some(sample_extent()),
                spatial_ref: Some(valid_spatial_ref()),
            },
            Some(100),
            Some(50),
        )
        .expect("matching layer spatial ref should place");

        assert_eq!(placement.crs, "EPSG:4326");
        assert_eq!(placement.extent, sample_extent());
        assert_eq!(placement.resolution, RasterResolution { x: 0.01, y: 0.02 });
        assert_ne!(placement.world_dimensions, Vec2::ZERO);
    }

    #[test]
    fn layer_placement_assertion_refuses_crs_mismatch() {
        let err = assert_manifest_layer_placement(
            &SceneGeospatialMetadata {
                georeferenced: true,
                crs: Some("EPSG:3857".to_string()),
                center: None,
                extent: Some(sample_extent()),
                spatial_ref: Some(valid_spatial_ref()),
            },
            Some(100),
            Some(50),
        )
        .expect_err("CRS mismatch should be refused");

        assert!(err.to_string().contains("CRS"));
    }

    #[test]
    fn layer_placement_assertion_refuses_extent_mismatch() {
        let err = assert_manifest_layer_placement(
            &SceneGeospatialMetadata {
                georeferenced: true,
                crs: Some("EPSG:4326".to_string()),
                center: None,
                extent: Some(SceneExtent {
                    min_lon: -89.5,
                    min_lat: 40.0,
                    max_lon: -88.4,
                    max_lat: 41.0,
                }),
                spatial_ref: Some(valid_spatial_ref()),
            },
            Some(100),
            Some(50),
        )
        .expect_err("extent mismatch should be refused");

        assert!(err.to_string().contains("extent"));
    }

    #[test]
    fn layer_metadata_readout_reports_manifest_values() {
        let readout = layer_metadata_readout(
            &SceneGeospatialMetadata {
                georeferenced: true,
                crs: Some("EPSG:4326".to_string()),
                center: None,
                extent: Some(sample_extent()),
                spatial_ref: Some(valid_spatial_ref()),
            },
            Some(100),
            Some(50),
        );

        assert_eq!(readout.crs, "EPSG:4326");
        assert_eq!(readout.extent, "-89.50000, 40.00000 -> -88.50000, 41.00000");
        assert_eq!(readout.resolution, "0.01000000 x 0.02000000");
        assert_eq!(readout.dimensions, "100x50 px");
    }

    #[test]
    fn layer_metadata_readout_flags_missing_fields_explicitly() {
        let readout = layer_metadata_readout(
            &SceneGeospatialMetadata {
                georeferenced: false,
                crs: None,
                center: None,
                extent: None,
                spatial_ref: None,
            },
            None,
            None,
        );

        assert_eq!(readout.crs, "missing CRS");
        assert_eq!(readout.extent, "missing extent");
        assert_eq!(readout.resolution, "missing resolution");
        assert_eq!(readout.dimensions, "missing dimensions");
    }

    #[test]
    fn switch_active_product_updates_kind_and_legend_without_resetting_view() {
        let manifest = sample_manifest_state();
        let mut viewer = ViewerState {
            selected_layer: 0,
            zoom_level: 2.5,
            scene_id_input: "scene-1".to_string(),
        };
        let mut config = TileConfig {
            base_url: "http://127.0.0.1:8080".to_string(),
            scene_id: Some("scene-1".to_string()),
            product_kind: DEFAULT_PRODUCT_KIND.to_string(),
        };

        let selection = switch_active_product(&manifest, &mut viewer, &mut config, 1)
            .expect("thermal product should switch");

        assert_eq!(viewer.selected_layer, 1);
        assert_eq!(viewer.zoom_level, 2.5);
        assert_eq!(viewer.scene_id_input, "scene-1");
        assert_eq!(config.product_kind, "thermal");
        assert_eq!(selection.product_kind, "thermal");
        assert_eq!(selection.legend.colormap, "hot");
        assert_eq!(selection.legend.stops.len(), 5);
        assert_eq!(
            selection.tile_source.tile_url(TileId { z: 2, x: 3, y: 1 }),
            "http://127.0.0.1:8080/api/scenes/scene-1/products/thermal/tiles/2/3/1.png"
        );
    }

    #[test]
    fn switch_active_product_refuses_bad_geospatial_assertion_and_keeps_prior_layer() {
        let mut manifest = sample_manifest_state();
        manifest.geospatial.crs = Some("EPSG:3857".to_string());
        let mut viewer = ViewerState {
            selected_layer: 0,
            zoom_level: 1.75,
            scene_id_input: "scene-1".to_string(),
        };
        let mut config = TileConfig {
            base_url: "http://127.0.0.1:8080".to_string(),
            scene_id: Some("scene-1".to_string()),
            product_kind: DEFAULT_PRODUCT_KIND.to_string(),
        };

        let err = switch_active_product(&manifest, &mut viewer, &mut config, 1)
            .expect_err("bad layer placement should refuse the switch");

        assert!(err.to_string().contains("CRS"));
        assert_eq!(viewer.selected_layer, 0);
        assert_eq!(viewer.zoom_level, 1.75);
        assert_eq!(config.product_kind, DEFAULT_PRODUCT_KIND);
    }

    #[test]
    fn product_legend_for_kind_uses_sensor_overlay_colormaps() {
        let ndvi = product_legend_for_kind("ndvi", &sample_extent())
            .expect("NDVI legend should be available");
        let thermal = product_legend_for_kind("thermal", &sample_extent())
            .expect("thermal legend should be available");
        let source = product_legend_for_kind("source", &sample_extent())
            .expect("source legend should be available");
        let lidar_elevation = product_legend_for_kind("lidar_elevation", &sample_extent())
            .expect("LiDAR elevation legend should be available");
        let occupancy = product_legend_for_kind("occupancy_density", &sample_extent())
            .expect("occupancy legend should be available");

        assert_eq!(ndvi.colormap, "viridis");
        assert_eq!(
            ndvi.value_range.map(|range| (range.min, range.max)),
            Some((-1.0, 1.0))
        );
        assert_eq!(thermal.colormap, "hot");
        assert_eq!(
            thermal.value_range.map(|range| (range.min, range.max)),
            Some((-10.0, 50.0))
        );
        assert_eq!(source.colormap, "grayscale");
        assert_eq!(source.stops.len(), 5);
        assert_eq!(lidar_elevation.colormap, "jet");
        assert_eq!(
            lidar_elevation
                .value_range
                .map(|range| (range.min, range.max)),
            Some((0.0, 1.0))
        );
        assert_eq!(occupancy.colormap, "hot");
        assert_eq!(
            occupancy.value_range.map(|range| (range.min, range.max)),
            Some((0.0, 1.0))
        );
    }

    #[test]
    fn switch_active_product_accepts_lidar_overlay_with_product_spatial_ref() {
        let mut manifest = sample_manifest_state();
        manifest.products.push(SceneProduct {
            spatial_ref: Some(valid_spatial_ref()),
            ..sample_product("lidar_elevation")
        });
        let mut viewer = ViewerState {
            selected_layer: 0,
            zoom_level: 1.0,
            scene_id_input: "scene-1".to_string(),
        };
        let mut config = TileConfig {
            base_url: "http://127.0.0.1:8080".to_string(),
            scene_id: Some("scene-1".to_string()),
            product_kind: DEFAULT_PRODUCT_KIND.to_string(),
        };

        let selection = switch_active_product(&manifest, &mut viewer, &mut config, 3)
            .expect("LiDAR overlay should switch");

        assert_eq!(selection.product_kind, "lidar_elevation");
        assert_eq!(selection.legend.colormap, "jet");
        assert_eq!(viewer.selected_layer, 3);
    }

    #[test]
    fn switch_active_product_refuses_ungeoreferenced_lidar_product() {
        let mut manifest = sample_manifest_state();
        let mut bad_spatial_ref = valid_spatial_ref();
        bad_spatial_ref.georeferenced = false;
        manifest.products.push(SceneProduct {
            spatial_ref: Some(bad_spatial_ref),
            ..sample_product("lidar_elevation")
        });
        let mut viewer = ViewerState {
            selected_layer: 0,
            zoom_level: 1.0,
            scene_id_input: "scene-1".to_string(),
        };
        let mut config = TileConfig {
            base_url: "http://127.0.0.1:8080".to_string(),
            scene_id: Some("scene-1".to_string()),
            product_kind: DEFAULT_PRODUCT_KIND.to_string(),
        };

        let err = switch_active_product(&manifest, &mut viewer, &mut config, 3)
            .expect_err("ungeoreferenced LiDAR product should be refused");

        assert!(err
            .to_string()
            .contains("product lidar_elevation spatial_ref assertion failed"));
        assert_eq!(viewer.selected_layer, 0);
        assert_eq!(config.product_kind, DEFAULT_PRODUCT_KIND);
    }

    #[test]
    fn switch_active_product_refuses_lidar_product_extent_mismatch() {
        let mut manifest = sample_manifest_state();
        let mut bad_spatial_ref = valid_spatial_ref();
        bad_spatial_ref.bbox.as_mut().expect("bbox exists").max_lon = -88.4;
        manifest.products.push(SceneProduct {
            spatial_ref: Some(bad_spatial_ref),
            ..sample_product("lidar_elevation")
        });
        let mut viewer = ViewerState {
            selected_layer: 0,
            zoom_level: 1.0,
            scene_id_input: "scene-1".to_string(),
        };
        let mut config = TileConfig {
            base_url: "http://127.0.0.1:8080".to_string(),
            scene_id: Some("scene-1".to_string()),
            product_kind: DEFAULT_PRODUCT_KIND.to_string(),
        };

        let err = switch_active_product(&manifest, &mut viewer, &mut config, 3)
            .expect_err("misaligned LiDAR product should be refused");

        assert!(err.to_string().contains("extent"));
        assert_eq!(viewer.selected_layer, 0);
        assert_eq!(config.product_kind, DEFAULT_PRODUCT_KIND);
    }

    #[test]
    fn compare_mode_opens_shared_georeferenced_view_for_comparable_scenes() {
        let left = sample_manifest_state();
        let right = compare_manifest("scene-2", "2025");
        let viewer = ViewerState {
            selected_layer: 0,
            zoom_level: 2.25,
            scene_id_input: "scene-1".to_string(),
        };
        let map_view = MapViewState {
            center: Vec2::new(42.0, -17.5),
            base_scale: 0.75,
            needs_fit: false,
        };

        let mut session = open_compare_mode(
            &left,
            0,
            &right,
            0,
            "http://127.0.0.1:8080",
            &viewer,
            &map_view,
            CompareLayout::Swipe,
        )
        .expect("matching field scenes should open compare mode");

        assert_eq!(session.left.scene_id, "scene-1");
        assert_eq!(session.right.scene_id, "scene-2");
        assert_eq!(session.left.field_id.as_deref(), Some("field-1"));
        assert_eq!(session.right.field_id.as_deref(), Some("field-1"));
        assert_eq!(session.left.season_id.as_deref(), Some("2026"));
        assert_eq!(session.right.season_id.as_deref(), Some("2025"));
        assert_eq!(session.left.product_kind, "ndvi");
        assert_eq!(session.right.product_kind, "ndvi");
        assert_eq!(session.placement.crs, "EPSG:4326");
        assert_eq!(session.placement.extent, sample_extent());
        assert_eq!(session.shared_view.center, Vec2::new(42.0, -17.5));
        assert_eq!(session.shared_view.zoom_level, 2.25);
        assert_eq!(session.shared_view.divider_fraction, 0.5);

        let moved_viewer = ViewerState {
            selected_layer: 1,
            zoom_level: 3.5,
            scene_id_input: "scene-1".to_string(),
        };
        let moved_map = MapViewState {
            center: Vec2::new(-80.0, 14.0),
            base_scale: 0.5,
            needs_fit: false,
        };
        sync_compare_shared_view(&mut session, &moved_viewer, &moved_map);
        set_compare_divider(&mut session, 0.85);

        assert_eq!(session.shared_view.center, Vec2::new(-80.0, 14.0));
        assert_eq!(session.shared_view.zoom_level, 3.5);
        assert_eq!(session.shared_view.divider_fraction, 0.85);
    }

    #[test]
    fn compare_mode_refuses_incompatible_scene_crs() {
        let left = sample_manifest_state();
        let mut right = compare_manifest("scene-2", "2025");
        right.geospatial.crs = Some("EPSG:3857".to_string());
        right
            .geospatial
            .spatial_ref
            .as_mut()
            .expect("right manifest spatial ref")
            .crs = Some("EPSG:3857".to_string());
        for product in &mut right.products {
            if let Some(spatial_ref) = product.spatial_ref.as_mut() {
                spatial_ref.crs = Some("EPSG:3857".to_string());
            }
        }

        let err = open_compare_mode(
            &left,
            0,
            &right,
            0,
            "http://127.0.0.1:8080",
            &sample_viewer_state(),
            &sample_map_view(),
            CompareLayout::Swipe,
        )
        .expect_err("CRS mismatch should refuse compare");

        assert!(err.to_string().contains("compare mismatch"));
        assert!(err.to_string().contains("CRS"));
    }

    #[test]
    fn compare_mode_refuses_incompatible_scene_extent() {
        let left = sample_manifest_state();
        let mut right = compare_manifest("scene-2", "2025");
        let shifted_extent = SceneExtent {
            max_lon: -88.25,
            ..sample_extent()
        };
        let shifted_ref = RasterSpatialRef {
            bbox: Some(GeoBounds {
                min_lon: shifted_extent.min_lon,
                min_lat: shifted_extent.min_lat,
                max_lon: shifted_extent.max_lon,
                max_lat: shifted_extent.max_lat,
            }),
            geo_transform: Some([-89.5, 0.0125, 0.0, 41.0, 0.0, -0.02]),
            resolution: Some(RasterResolution { x: 0.0125, y: 0.02 }),
            ..valid_spatial_ref()
        };
        right.geospatial.extent = Some(shifted_extent.clone());
        right.geospatial.spatial_ref = Some(shifted_ref.clone());
        for product in &mut right.products {
            product.spatial_ref = Some(shifted_ref.clone());
        }

        let err = open_compare_mode(
            &left,
            0,
            &right,
            0,
            "http://127.0.0.1:8080",
            &sample_viewer_state(),
            &sample_map_view(),
            CompareLayout::SideBySide,
        )
        .expect_err("extent mismatch should refuse compare");

        assert!(err.to_string().contains("compare mismatch"));
        assert!(err.to_string().contains("extent"));
    }

    #[test]
    fn compare_mode_refuses_product_spatial_ref_mismatch() {
        let left = sample_manifest_state();
        let mut right = compare_manifest("scene-2", "2025");
        right.products[0]
            .spatial_ref
            .as_mut()
            .expect("product spatial ref")
            .bbox
            .as_mut()
            .expect("product bbox")
            .max_lon = -88.25;

        let err = open_compare_mode(
            &left,
            0,
            &right,
            0,
            "http://127.0.0.1:8080",
            &sample_viewer_state(),
            &sample_map_view(),
            CompareLayout::Swipe,
        )
        .expect_err("bad product spatial ref should refuse compare");

        assert!(err.to_string().contains("product"));
        assert!(err.to_string().contains("extent"));
    }

    #[test]
    fn report_result_overlay_builds_zone_labels_and_world_polygon() {
        let overlay = build_report_result_overlay(
            &sample_report_record(),
            &[sample_report_zone("EPSG:4326")],
            &sample_layer_placement(),
        )
        .expect("matching CRS report zone should overlay");

        assert_eq!(overlay.len(), 1);
        assert_eq!(overlay[0].report_id, "report-1");
        assert_eq!(overlay[0].finding_id, "finding:09:stress-ne-zone");
        assert_eq!(overlay[0].zone_id, "zone-ne");
        assert_eq!(overlay[0].crs, "EPSG:4326");
        assert_eq!(
            overlay[0].label,
            "high: NDVI decline aligned with water stress"
        );
        assert_eq!(overlay[0].world_polygon.len(), 4);
        assert_eq!(overlay[0].world_polygon[0], Vec2::new(-4000.0, 1000.0));
    }

    #[test]
    fn report_result_overlay_refuses_zone_crs_mismatch() {
        let err = build_report_result_overlay(
            &sample_report_record(),
            &[sample_report_zone("EPSG:3857")],
            &sample_layer_placement(),
        )
        .expect_err("mismatched CRS must not draw");

        assert!(err.to_string().contains("CRS mismatch"));
        assert!(err.to_string().contains("EPSG:4326"));
        assert!(err.to_string().contains("EPSG:3857"));
    }

    #[test]
    fn saved_view_round_trip_restores_scene_product_camera_and_overlays() {
        let manifest = sample_manifest_state();
        let mut config = TileConfig {
            base_url: "http://127.0.0.1:8080".to_string(),
            scene_id: Some("scene-1".to_string()),
            product_kind: "thermal".to_string(),
        };
        let mut viewer = ViewerState {
            selected_layer: 1,
            zoom_level: 2.75,
            scene_id_input: "scene-1".to_string(),
        };
        let mut catalog = FieldCatalogState {
            selected_field_id: Some("old-field".to_string()),
            selected_scene_id: Some("old-scene".to_string()),
            ..Default::default()
        };
        let map_view = MapViewState {
            center: Vec2::new(125.0, -52.0),
            base_scale: 0.45,
            needs_fit: false,
        };
        let mut annotations = AnnotationOverlayState {
            selected_annotation_id: Some("annotation-7".to_string()),
            filter_label: "weeds".to_string(),
            show_low: false,
            show_polygons: false,
            ..Default::default()
        };
        let mut recommendations = RecommendationOverlayState {
            selected_recommendation_id: Some("rec-9".to_string()),
            status_filter: Some(RecommendationStatus::Reviewed),
            priority_filter: Some(RecommendationPriority::High),
            ..Default::default()
        };
        let mut reports = ReportOverlayState {
            draft_title: "Grower handoff".to_string(),
            ..Default::default()
        };

        let view = capture_saved_view(
            "North Field scout",
            &config,
            &viewer,
            &map_view,
            &manifest,
            &annotations,
            &recommendations,
            &reports,
        )
        .expect("configured view should be captured");
        let path = temp_artifact_path("saved_view", "json");
        save_view_to_json(&path, &view).expect("saved view should persist");
        let loaded = load_view_from_json(&path).expect("saved view should reload");

        config.scene_id = None;
        config.product_kind = "ndvi".to_string();
        viewer.selected_layer = 0;
        viewer.zoom_level = 1.0;
        viewer.scene_id_input.clear();
        annotations.selected_annotation_id = None;
        annotations.filter_label.clear();
        annotations.show_low = true;
        annotations.show_polygons = true;
        recommendations.selected_recommendation_id = None;
        recommendations.status_filter = None;
        recommendations.priority_filter = None;
        reports.draft_title.clear();
        let mut restored_map = MapViewState {
            center: Vec2::ZERO,
            base_scale: 1.0,
            needs_fit: true,
        };

        restore_saved_view(
            &loaded,
            &mut catalog,
            &mut config,
            &mut viewer,
            &mut restored_map,
            &mut annotations,
            &mut recommendations,
            &mut reports,
        )
        .expect("saved view should restore into viewer state");

        assert_eq!(loaded, view);
        assert_eq!(catalog.selected_field_id.as_deref(), Some("field-1"));
        assert_eq!(catalog.selected_scene_id.as_deref(), Some("scene-1"));
        assert_eq!(config.scene_id.as_deref(), Some("scene-1"));
        assert_eq!(config.product_kind, "thermal");
        assert_eq!(viewer.selected_layer, 1);
        assert_eq!(viewer.zoom_level, 2.75);
        assert_eq!(viewer.scene_id_input, "scene-1");
        assert_eq!(restored_map.center, Vec2::new(125.0, -52.0));
        assert_eq!(restored_map.base_scale, 0.45);
        assert!(!restored_map.needs_fit);
        assert_eq!(
            annotations.selected_annotation_id.as_deref(),
            Some("annotation-7")
        );
        assert_eq!(annotations.filter_label, "weeds");
        assert!(!annotations.show_low);
        assert!(!annotations.show_polygons);
        assert_eq!(
            recommendations.selected_recommendation_id.as_deref(),
            Some("rec-9")
        );
        assert_eq!(
            recommendations.status_filter,
            Some(RecommendationStatus::Reviewed)
        );
        assert_eq!(
            recommendations.priority_filter,
            Some(RecommendationPriority::High)
        );
        assert_eq!(reports.draft_title, "Grower handoff");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn snapshot_export_marks_ungeoreferenced_state() {
        let mut manifest = sample_manifest_state();
        manifest.geospatial.georeferenced = false;
        manifest.geospatial.crs = None;
        manifest.geospatial.extent = None;
        manifest.geospatial.spatial_ref = None;
        let view = capture_saved_view(
            "Unplaced scout",
            &TileConfig {
                base_url: "http://127.0.0.1:8080".to_string(),
                scene_id: Some("scene-1".to_string()),
                product_kind: "ndvi".to_string(),
            },
            &sample_viewer_state(),
            &sample_map_view(),
            &manifest,
            &AnnotationOverlayState::default(),
            &RecommendationOverlayState::default(),
            &ReportOverlayState::default(),
        )
        .expect("ungeoreferenced views can still be captured");
        let image_path = temp_artifact_path("snapshot", "png");
        let metadata_path = temp_artifact_path("snapshot", "json");

        let metadata = export_snapshot(
            &view,
            &manifest,
            &sample_tile_render_state(),
            &image_path,
            &metadata_path,
        )
        .expect("snapshot export should run for ungeoreferenced state");

        assert!(!metadata.georeferenced);
        assert_eq!(metadata.georeference_label, "non_georeferenced");
        assert!(metadata
            .georeference_warning
            .as_deref()
            .expect("warning")
            .contains("not georeferenced"));
        assert!(image_path.exists());
        let decoded: super::SnapshotExportMetadata =
            serde_json::from_slice(&fs::read(&metadata_path).expect("metadata file"))
                .expect("metadata json");
        assert_eq!(decoded.georeference_label, "non_georeferenced");
        assert_eq!(decoded.view.name, "Unplaced scout");

        let _ = fs::remove_file(image_path);
        let _ = fs::remove_file(metadata_path);
    }

    fn sample_extent() -> SceneExtent {
        SceneExtent {
            min_lon: -89.5,
            min_lat: 40.0,
            max_lon: -88.5,
            max_lat: 41.0,
        }
    }

    fn valid_spatial_ref() -> RasterSpatialRef {
        RasterSpatialRef {
            georeferenced: true,
            crs: Some("EPSG:4326".to_string()),
            bbox: Some(GeoBounds {
                min_lon: -89.5,
                min_lat: 40.0,
                max_lon: -88.5,
                max_lat: 41.0,
            }),
            geo_transform: Some([-89.5, 0.01, 0.0, 41.0, 0.0, -0.02]),
            resolution: Some(RasterResolution { x: 0.01, y: 0.02 }),
        }
    }

    fn sample_layer_placement() -> LayerPlacement {
        LayerPlacement {
            crs: "EPSG:4326".to_string(),
            extent: sample_extent(),
            resolution: RasterResolution { x: 0.01, y: 0.02 },
            world_dimensions: Vec2::new(10_000.0, 10_000.0),
        }
    }

    fn sample_report_zone(crs: &str) -> ReportFindingZone {
        ReportFindingZone {
            report_id: "report-1".to_string(),
            finding_id: "finding:09:stress-ne-zone".to_string(),
            zone_id: "zone-ne".to_string(),
            crs: crs.to_string(),
            coordinates: vec![
                GeoPoint {
                    longitude: -89.4,
                    latitude: 40.6,
                },
                GeoPoint {
                    longitude: -89.1,
                    latitude: 40.6,
                },
                GeoPoint {
                    longitude: -89.1,
                    latitude: 40.9,
                },
                GeoPoint {
                    longitude: -89.4,
                    latitude: 40.9,
                },
            ],
            reason: "NDVI decline aligned with water stress".to_string(),
            priority: RecommendationPriority::High,
        }
    }

    fn sample_report_record() -> ReportRecord {
        ReportRecord {
            report_id: "report-1".to_string(),
            scene_id: "scene-1".to_string(),
            field_id: Some("field-1".to_string()),
            season_id: Some("season-2026".to_string()),
            org_id: "org-a".to_string(),
            generated_by: "advisor-1".to_string(),
            source_refs: vec!["finding:09:stress-ne-zone".to_string()],
            title: "North Field report".to_string(),
            format: ReportFormat::Html,
            artifact_path: "/tmp/report-1.html".to_string(),
            artifact_uri: "s3://reports/report-1.html".to_string(),
            download_url: "/api/scenes/scene-1/reports/report-1".to_string(),
            visibility: ReportVisibility::Org,
            annotation_count: 1,
            recommendation_count: 1,
            created_at: "2026-06-12T10:00:00Z".to_string(),
        }
    }

    fn sample_manifest_state() -> SceneManifestState {
        SceneManifestState {
            scene_id: Some("scene-1".to_string()),
            owner: Some("org-alpha".to_string()),
            sensor: Some("landsat8".to_string()),
            acquired_at: Some("2026-05-01T00:00:00Z".to_string()),
            width: Some(100),
            height: Some(50),
            bands: vec!["red".to_string(), "nir".to_string(), "thermal".to_string()],
            gps_position: None,
            data_path: Some("/tmp/scene-1".to_string()),
            field_id: Some("field-1".to_string()),
            season_id: Some("2026".to_string()),
            linked_at: Some("2026-05-01T00:00:00Z".to_string()),
            field: None,
            geospatial: SceneGeospatialMetadata {
                georeferenced: true,
                crs: Some("EPSG:4326".to_string()),
                center: None,
                extent: Some(sample_extent()),
                spatial_ref: Some(valid_spatial_ref()),
            },
            products: vec![
                sample_product("ndvi"),
                sample_product("thermal"),
                sample_product("source"),
            ],
        }
    }

    fn sample_product(kind: &str) -> SceneProduct {
        SceneProduct {
            product_id: None,
            kind: kind.to_string(),
            field_id: None,
            season_id: None,
            filename: format!("{kind}.png"),
            content_type: "image/png".to_string(),
            spatial_ref: None,
            source_image_ids: Vec::new(),
            url_path: format!("/api/scenes/scene-1/products/{kind}"),
            tile_url_template: format!(
                "/api/scenes/scene-1/products/{kind}/tiles/{{z}}/{{x}}/{{y}}.png"
            ),
        }
    }

    fn compare_manifest(scene_id: &str, season_id: &str) -> SceneManifestState {
        let mut manifest = sample_manifest_state();
        manifest.scene_id = Some(scene_id.to_string());
        manifest.season_id = Some(season_id.to_string());
        manifest.acquired_at = Some(format!("{season_id}-05-01T00:00:00Z"));
        for product in &mut manifest.products {
            product.product_id = Some(format!("{scene_id}:{}", product.kind));
            product.field_id = Some("field-1".to_string());
            product.season_id = Some(season_id.to_string());
            product.spatial_ref = Some(valid_spatial_ref());
            product.url_path = format!("/api/scenes/{scene_id}/products/{}", product.kind);
            product.tile_url_template = format!(
                "/api/scenes/{scene_id}/products/{}/tiles/{{z}}/{{x}}/{{y}}.png",
                product.kind
            );
        }
        manifest
    }

    fn sample_viewer_state() -> ViewerState {
        ViewerState {
            selected_layer: 0,
            zoom_level: 1.5,
            scene_id_input: "scene-1".to_string(),
        }
    }

    fn sample_map_view() -> MapViewState {
        MapViewState {
            center: Vec2::ZERO,
            base_scale: 1.0,
            needs_fit: false,
        }
    }

    fn sample_tile_render_state() -> TileRenderState {
        TileRenderState {
            tiles: BTreeMap::new(),
            visible_tiles: BTreeSet::new(),
            image_dimensions: Vec2::new(32.0, 16.0),
            world_dimensions: Vec2::ZERO,
            current_zoom: DEFAULT_TILE_ZOOM,
            status: TileStatus::Ready,
        }
    }

    fn temp_artifact_path(label: &str, extension: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "agbot_geo_viewer_{label}_{}_{}.{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test"),
            extension
        ))
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
