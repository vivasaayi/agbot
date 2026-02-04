//! OpenStreetMap Integration & Field Boundary Detection
//!
//! Fetches geographic features from OpenStreetMap:
//! - Farm field boundaries (landuse=farmland)
//! - Buildings (barns, silos, farmhouses)
//! - Roads and tracks
//! - Water features (irrigation, ponds)
//!
//! Also provides image-based field boundary detection using edge detection.

use anyhow::{anyhow, Context, Result};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{tile_cache::{TileCache, TileType}, GeoBounds, TileCoord};

/// OSM feature configuration
#[derive(Resource, Clone)]
pub struct OsmConfig {
    /// Whether OSM overlay is enabled
    pub enabled: bool,
    /// Show field boundaries
    pub show_fields: bool,
    /// Show buildings
    pub show_buildings: bool,
    /// Show roads
    pub show_roads: bool,
    /// Show water features
    pub show_water: bool,
    /// Line width for boundaries
    pub line_width: f32,
}

impl Default for OsmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_fields: true,
            show_buildings: true,
            show_roads: true,
            show_water: true,
            line_width: 2.0,
        }
    }
}

/// Types of OSM features we care about for agriculture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OsmFeatureType {
    // Agricultural
    Farmland,
    Farmyard,
    Orchard,
    Vineyard,
    Greenhouse,
    
    // Buildings
    Barn,
    Silo,
    FarmBuilding,
    House,
    Shed,
    
    // Infrastructure
    Road,
    Track,
    Path,
    Fence,
    
    // Water
    Stream,
    Ditch,
    Pond,
    Reservoir,
    IrrigationCanal,
    
    // Other
    Forest,
    Meadow,
    Unknown,
}

impl OsmFeatureType {
    /// Get color for rendering this feature type
    pub fn color(&self) -> Color {
        match self {
            // Agricultural - greens and browns
            Self::Farmland => Color::srgba(0.5, 0.7, 0.3, 0.3),
            Self::Farmyard => Color::srgba(0.6, 0.5, 0.3, 0.4),
            Self::Orchard => Color::srgba(0.3, 0.6, 0.2, 0.3),
            Self::Vineyard => Color::srgba(0.4, 0.3, 0.5, 0.3),
            Self::Greenhouse => Color::srgba(0.7, 0.9, 0.9, 0.4),
            
            // Buildings - warm colors
            Self::Barn => Color::srgba(0.6, 0.3, 0.2, 0.8),
            Self::Silo => Color::srgba(0.5, 0.5, 0.6, 0.8),
            Self::FarmBuilding => Color::srgba(0.5, 0.4, 0.3, 0.7),
            Self::House => Color::srgba(0.7, 0.5, 0.4, 0.8),
            Self::Shed => Color::srgba(0.4, 0.3, 0.3, 0.7),
            
            // Roads - grays
            Self::Road => Color::srgba(0.3, 0.3, 0.3, 0.9),
            Self::Track => Color::srgba(0.5, 0.4, 0.3, 0.7),
            Self::Path => Color::srgba(0.4, 0.4, 0.3, 0.5),
            Self::Fence => Color::srgba(0.4, 0.3, 0.2, 0.6),
            
            // Water - blues
            Self::Stream => Color::srgba(0.3, 0.5, 0.8, 0.8),
            Self::Ditch => Color::srgba(0.4, 0.5, 0.7, 0.6),
            Self::Pond => Color::srgba(0.2, 0.4, 0.7, 0.7),
            Self::Reservoir => Color::srgba(0.2, 0.3, 0.6, 0.8),
            Self::IrrigationCanal => Color::srgba(0.3, 0.5, 0.8, 0.7),
            
            // Other
            Self::Forest => Color::srgba(0.1, 0.4, 0.1, 0.4),
            Self::Meadow => Color::srgba(0.5, 0.7, 0.4, 0.3),
            Self::Unknown => Color::srgba(0.5, 0.5, 0.5, 0.2),
        }
    }
    
    /// Whether this feature should be rendered as a polygon fill
    pub fn is_polygon(&self) -> bool {
        matches!(self, 
            Self::Farmland | Self::Farmyard | Self::Orchard | Self::Vineyard |
            Self::Greenhouse | Self::Pond | Self::Reservoir | Self::Forest | Self::Meadow
        )
    }
    
    /// Whether this feature is a building
    pub fn is_building(&self) -> bool {
        matches!(self,
            Self::Barn | Self::Silo | Self::FarmBuilding | Self::House | Self::Shed
        )
    }
}

/// A geographic point (lat/lon)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// An OSM feature with geometry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmFeature {
    pub id: i64,
    pub feature_type: OsmFeatureType,
    pub name: Option<String>,
    /// For ways: list of points forming the geometry
    pub geometry: Vec<GeoPoint>,
    /// Additional OSM tags
    pub tags: HashMap<String, String>,
}

impl OsmFeature {
    /// Get the centroid of this feature
    pub fn centroid(&self) -> Option<GeoPoint> {
        if self.geometry.is_empty() {
            return None;
        }
        
        let sum_lat: f64 = self.geometry.iter().map(|p| p.lat).sum();
        let sum_lon: f64 = self.geometry.iter().map(|p| p.lon).sum();
        let n = self.geometry.len() as f64;
        
        Some(GeoPoint::new(sum_lat / n, sum_lon / n))
    }
    
    /// Get the bounding box of this feature
    pub fn bounds(&self) -> Option<GeoBounds> {
        if self.geometry.is_empty() {
            return None;
        }
        
        let min_lat = self.geometry.iter().map(|p| p.lat).fold(f64::MAX, f64::min);
        let max_lat = self.geometry.iter().map(|p| p.lat).fold(f64::MIN, f64::max);
        let min_lon = self.geometry.iter().map(|p| p.lon).fold(f64::MAX, f64::min);
        let max_lon = self.geometry.iter().map(|p| p.lon).fold(f64::MIN, f64::max);
        
        Some(GeoBounds { min_lat, min_lon, max_lat, max_lon })
    }
}

/// Container for all OSM features in a region
#[derive(Debug, Clone, Default)]
pub struct OsmData {
    pub features: Vec<OsmFeature>,
    pub bounds: Option<GeoBounds>,
    pub stats: OsmStats,
}

#[derive(Debug, Clone, Default)]
pub struct OsmStats {
    pub total_features: usize,
    pub field_count: usize,
    pub building_count: usize,
    pub road_length_m: f64,
    pub water_feature_count: usize,
}

/// Overpass API query response types
#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    element_type: String,
    id: i64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    nodes: Option<Vec<i64>>,
    #[serde(default)]
    geometry: Option<Vec<OverpassGeomNode>>,
    #[serde(default)]
    tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct OverpassGeomNode {
    lat: f64,
    lon: f64,
}

/// Fetch OSM features for a geographic area using Overpass API
pub async fn fetch_osm_features(
    bounds: GeoBounds,
    config: &OsmConfig,
    _cache: &mut TileCache,
) -> Result<OsmData> {
    // Build Overpass query for agricultural features
    let query = build_overpass_query(&bounds, config);
    
    tracing::info!("Fetching OSM features for bounds: {:?}", bounds);
    
    let client = reqwest::Client::builder()
        .user_agent("AgBot-GIS/0.1")
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    
    // Use Overpass API (multiple mirrors available)
    let url = "https://overpass-api.de/api/interpreter";
    
    let response = client
        .post(url)
        .body(query)
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(anyhow!("Overpass API error: HTTP {}", response.status()));
    }
    
    let text = response.text().await?;
    let overpass: OverpassResponse = serde_json::from_str(&text)
        .context("Failed to parse Overpass response")?;
    
    // Convert to our feature types
    let features = parse_overpass_response(overpass);
    let stats = compute_osm_stats(&features);
    
    Ok(OsmData {
        features,
        bounds: Some(bounds),
        stats,
    })
}

/// Build an Overpass QL query for agricultural features
fn build_overpass_query(bounds: &GeoBounds, config: &OsmConfig) -> String {
    let bbox = format!(
        "{},{},{},{}",
        bounds.min_lat, bounds.min_lon, bounds.max_lat, bounds.max_lon
    );
    
    let mut query_parts = Vec::new();
    
    if config.show_fields {
        query_parts.push(format!(r#"way["landuse"~"farmland|farmyard|orchard|vineyard|meadow|greenhouse"]({bbox});"#));
        query_parts.push(format!(r#"relation["landuse"~"farmland|farmyard|orchard|vineyard"]({bbox});"#));
    }
    
    if config.show_buildings {
        query_parts.push(format!(r#"way["building"~"barn|silo|farm|farm_auxiliary|greenhouse|shed"]({bbox});"#));
        query_parts.push(format!(r#"way["building"="yes"]["farm"="yes"]({bbox});"#));
    }
    
    if config.show_roads {
        query_parts.push(format!(r#"way["highway"~"track|path|service|unclassified"]({bbox});"#));
        query_parts.push(format!(r#"way["barrier"="fence"]({bbox});"#));
    }
    
    if config.show_water {
        query_parts.push(format!(r#"way["waterway"~"stream|ditch|canal|drain"]({bbox});"#));
        query_parts.push(format!(r#"way["natural"="water"]({bbox});"#));
        query_parts.push(format!(r#"way["landuse"="reservoir"]({bbox});"#));
    }
    
    format!(
        r#"[out:json][timeout:30];
(
  {}
);
out body geom;"#,
        query_parts.join("\n  ")
    )
}

/// Parse Overpass response into our feature types
fn parse_overpass_response(response: OverpassResponse) -> Vec<OsmFeature> {
    let mut features = Vec::new();
    
    for element in response.elements {
        // Extract geometry
        let geometry: Vec<GeoPoint> = if let Some(geom) = element.geometry {
            geom.iter().map(|n| GeoPoint::new(n.lat, n.lon)).collect()
        } else if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
            vec![GeoPoint::new(lat, lon)]
        } else {
            continue; // Skip elements without geometry
        };
        
        if geometry.is_empty() {
            continue;
        }
        
        let tags = element.tags.unwrap_or_default();
        let feature_type = classify_feature(&tags);
        let name = tags.get("name").cloned();
        
        features.push(OsmFeature {
            id: element.id,
            feature_type,
            name,
            geometry,
            tags,
        });
    }
    
    features
}

/// Classify an OSM feature based on its tags
fn classify_feature(tags: &HashMap<String, String>) -> OsmFeatureType {
    // Check landuse
    if let Some(landuse) = tags.get("landuse") {
        match landuse.as_str() {
            "farmland" => return OsmFeatureType::Farmland,
            "farmyard" => return OsmFeatureType::Farmyard,
            "orchard" => return OsmFeatureType::Orchard,
            "vineyard" => return OsmFeatureType::Vineyard,
            "meadow" => return OsmFeatureType::Meadow,
            "forest" => return OsmFeatureType::Forest,
            "reservoir" => return OsmFeatureType::Reservoir,
            "greenhouse_horticulture" => return OsmFeatureType::Greenhouse,
            _ => {}
        }
    }
    
    // Check building type
    if let Some(building) = tags.get("building") {
        match building.as_str() {
            "barn" => return OsmFeatureType::Barn,
            "silo" => return OsmFeatureType::Silo,
            "farm" | "farm_auxiliary" => return OsmFeatureType::FarmBuilding,
            "house" | "residential" => return OsmFeatureType::House,
            "shed" => return OsmFeatureType::Shed,
            "greenhouse" => return OsmFeatureType::Greenhouse,
            _ => return OsmFeatureType::FarmBuilding,
        }
    }
    
    // Check highway type
    if let Some(highway) = tags.get("highway") {
        match highway.as_str() {
            "track" => return OsmFeatureType::Track,
            "path" | "footway" => return OsmFeatureType::Path,
            _ => return OsmFeatureType::Road,
        }
    }
    
    // Check waterway
    if let Some(waterway) = tags.get("waterway") {
        match waterway.as_str() {
            "stream" | "river" => return OsmFeatureType::Stream,
            "ditch" | "drain" => return OsmFeatureType::Ditch,
            "canal" => return OsmFeatureType::IrrigationCanal,
            _ => return OsmFeatureType::Stream,
        }
    }
    
    // Check natural
    if let Some(natural) = tags.get("natural") {
        match natural.as_str() {
            "water" => return OsmFeatureType::Pond,
            "wood" => return OsmFeatureType::Forest,
            _ => {}
        }
    }
    
    // Check barrier
    if tags.get("barrier").map(|s| s == "fence").unwrap_or(false) {
        return OsmFeatureType::Fence;
    }
    
    OsmFeatureType::Unknown
}

/// Compute statistics from OSM features
fn compute_osm_stats(features: &[OsmFeature]) -> OsmStats {
    let mut stats = OsmStats {
        total_features: features.len(),
        ..Default::default()
    };
    
    for feature in features {
        match feature.feature_type {
            OsmFeatureType::Farmland | OsmFeatureType::Farmyard |
            OsmFeatureType::Orchard | OsmFeatureType::Vineyard => {
                stats.field_count += 1;
            }
            t if t.is_building() => {
                stats.building_count += 1;
            }
            OsmFeatureType::Road | OsmFeatureType::Track | OsmFeatureType::Path => {
                // Estimate road length
                stats.road_length_m += estimate_geometry_length(&feature.geometry);
            }
            OsmFeatureType::Stream | OsmFeatureType::Ditch | OsmFeatureType::Pond |
            OsmFeatureType::Reservoir | OsmFeatureType::IrrigationCanal => {
                stats.water_feature_count += 1;
            }
            _ => {}
        }
    }
    
    stats
}

/// Estimate the length of a geometry in meters
fn estimate_geometry_length(points: &[GeoPoint]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    
    let mut total = 0.0;
    for window in points.windows(2) {
        let p1 = &window[0];
        let p2 = &window[1];
        total += haversine_distance(p1.lat, p1.lon, p2.lat, p2.lon);
    }
    total
}

/// Calculate haversine distance between two points in meters
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_000.0; // Earth radius in meters
    
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();
    
    let a = (delta_lat / 2.0).sin().powi(2) +
            lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    
    R * c
}

/// Convert GeoPoint to local ENU coordinates relative to a reference point
pub fn geo_to_local(point: &GeoPoint, reference: &GeoPoint) -> (f32, f32) {
    // Approximate conversion using local tangent plane
    let lat_scale = 111_320.0; // meters per degree latitude
    let lon_scale = 111_320.0 * reference.lat.to_radians().cos();
    
    let x = ((point.lon - reference.lon) * lon_scale) as f32;
    let z = ((point.lat - reference.lat) * lat_scale) as f32;
    
    (x, z)
}

/// Image-based field boundary detection using edge detection
/// 
/// This is a simple Sobel edge detector that can identify field boundaries
/// from satellite imagery where OSM data is incomplete.
pub fn detect_field_boundaries_from_image(
    pixels: &[u8],
    width: u32,
    height: u32,
    threshold: f32,
) -> Vec<u8> {
    let mut edges = vec![0u8; (width * height) as usize];
    
    // Convert to grayscale
    let gray: Vec<f32> = pixels.chunks(4)
        .map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0)
        .collect();
    
    // Sobel operator
    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let idx = |dx: i32, dy: i32| -> f32 {
                let nx = (x as i32 + dx) as usize;
                let ny = (y as i32 + dy) as usize;
                gray[ny * width as usize + nx]
            };
            
            // Sobel X
            let gx = -idx(-1, -1) - 2.0 * idx(-1, 0) - idx(-1, 1)
                   + idx(1, -1) + 2.0 * idx(1, 0) + idx(1, 1);
            
            // Sobel Y
            let gy = -idx(-1, -1) - 2.0 * idx(0, -1) - idx(1, -1)
                   + idx(-1, 1) + 2.0 * idx(0, 1) + idx(1, 1);
            
            let magnitude = (gx * gx + gy * gy).sqrt();
            
            let i = (y * width + x) as usize;
            edges[i] = if magnitude > threshold { 255 } else { 0 };
        }
    }
    
    edges
}

/// Convert detected edges to boundary polygons (simplified)
pub fn edges_to_boundaries(
    edges: &[u8],
    width: u32,
    height: u32,
    bounds: GeoBounds,
    min_area: f32,
) -> Vec<OsmFeature> {
    // This is a simplified implementation
    // A full implementation would use connected component analysis
    // and polygon simplification (Douglas-Peucker)
    
    let mut features = Vec::new();
    let mut visited = vec![false; edges.len()];
    let mut feature_id = -1i64;
    
    // Find connected edge regions
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if edges[idx] > 0 && !visited[idx] {
                // Start a new boundary trace
                let mut boundary_points = Vec::new();
                trace_boundary(edges, width, height, x, y, &mut visited, &mut boundary_points);
                
                if boundary_points.len() >= 4 {
                    // Convert to geo coordinates
                    let geo_points: Vec<GeoPoint> = boundary_points.iter()
                        .map(|&(px, py)| {
                            let u = px as f64 / width as f64;
                            let v = py as f64 / height as f64;
                            GeoPoint::new(
                                bounds.max_lat - v * (bounds.max_lat - bounds.min_lat),
                                bounds.min_lon + u * (bounds.max_lon - bounds.min_lon),
                            )
                        })
                        .collect();
                    
                    feature_id -= 1; // Use negative IDs for detected features
                    features.push(OsmFeature {
                        id: feature_id,
                        feature_type: OsmFeatureType::Farmland,
                        name: Some(format!("Detected Field {}", -feature_id)),
                        geometry: geo_points,
                        tags: HashMap::new(),
                    });
                }
            }
        }
    }
    
    features
}

/// Trace a boundary starting from a point
fn trace_boundary(
    edges: &[u8],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
    visited: &mut [bool],
    points: &mut Vec<(u32, u32)>,
) {
    let mut x = start_x;
    let mut y = start_y;
    let max_points = 1000; // Limit to avoid infinite loops
    
    while points.len() < max_points {
        let idx = (y * width + x) as usize;
        if visited[idx] {
            break;
        }
        visited[idx] = true;
        points.push((x, y));
        
        // Find next unvisited edge pixel (8-connected)
        let mut found = false;
        for &(dx, dy) in &[(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                let nidx = (ny as u32 * width + nx as u32) as usize;
                if edges[nidx] > 0 && !visited[nidx] {
                    x = nx as u32;
                    y = ny as u32;
                    found = true;
                    break;
                }
            }
        }
        
        if !found {
            break;
        }
    }
}
