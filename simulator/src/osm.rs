use anyhow::{anyhow, Context, Result};
use bevy::math::Vec2;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

pub const OVERPASS_ENDPOINT: &str = "https://overpass-api.de/api/interpreter";
pub const USER_AGENT: &str = "AgBot-Simulator/0.1 (+https://github.com/vivasaayi/agbot)";
pub const METERS_PER_DEGREE_LAT: f32 = 111_320.0;

#[derive(Debug, Clone, Default)]
pub struct BuildingAttributes {
    pub levels: Option<f32>,
    pub height_m: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum PolygonKind {
    Building(BuildingAttributes),
    Farmland,
    Park,
    Water,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct MapPolygon {
    pub kind: PolygonKind,
    pub coordinates: Vec<[f64; 2]>, // [lon, lat]
}

#[derive(Debug, Clone)]
pub enum LineKind {
    Road(String),
    Other(String),
}

#[derive(Debug, Clone)]
pub struct MapLine {
    pub kind: LineKind,
    pub coordinates: Vec<[f64; 2]>, // [lon, lat]
}

#[derive(Debug, Clone)]
pub struct OsmMapData {
    pub center_lat: f64,
    pub center_lon: f64,
    pub polygons: Vec<MapPolygon>,
    pub lines: Vec<MapLine>,
}

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<Element>,
}

#[derive(Debug, Deserialize)]
struct Element {
    #[serde(rename = "type")]
    element_type: String,
    id: i64,
    tags: Option<HashMap<String, String>>,
    geometry: Option<Vec<Coordinate>>,
}

#[derive(Debug, Deserialize, Clone)]
struct Coordinate {
    lat: f64,
    lon: f64,
}

/// Fetch OSM map data using Overpass API for the given location and radius in meters.
pub async fn fetch_osm_data(lat: f64, lon: f64, radius_m: f64) -> Result<OsmMapData> {
    let query = build_overpass_query(lat, lon, radius_m);

    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .post(OVERPASS_ENDPOINT)
        .body(query)
        .send()
        .await
        .context("failed to send Overpass request")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Overpass API responded with status {}",
            response.status()
        ));
    }

    let overpass: OverpassResponse = response
        .json()
        .await
        .context("failed to parse Overpass response")?;

    let mut polygons = Vec::new();
    let mut lines = Vec::new();

    for element in overpass.elements.into_iter() {
        if element.element_type != "way" {
            continue;
        }

        let Some(geometry) = element.geometry else {
            continue;
        };
        if geometry.len() < 2 {
            continue;
        }

        let coords: Vec<[f64; 2]> = geometry.iter().map(|c| [c.lon, c.lat]).collect();
        let tags = element.tags.unwrap_or_default();

        if let Some(building_tag) = tags.get("building") {
            if coords.len() >= 3 {
                let levels = tags
                    .get("building:levels")
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| *v > 0.0);
                let height = tags
                    .get("height")
                    .and_then(|v| parse_height(v))
                    .filter(|v| *v > 0.0);

                polygons.push(MapPolygon {
                    kind: PolygonKind::Building(BuildingAttributes {
                        levels,
                        height_m: height,
                    }),
                    coordinates: coords,
                });
            } else {
                tracing::debug!(
                    id = element.id,
                    building = building_tag,
                    "Skipping building without enough points"
                );
            }
            continue;
        }

        if let Some(highway) = tags.get("highway") {
            lines.push(MapLine {
                kind: LineKind::Road(highway.clone()),
                coordinates: coords,
            });
            continue;
        }

        if let Some(landuse) = tags.get("landuse") {
            match landuse.as_str() {
                "farmland" | "field" | "meadow" | "orchard" | "vineyard" => {
                    if coords.len() >= 3 {
                        polygons.push(MapPolygon {
                            kind: PolygonKind::Farmland,
                            coordinates: coords,
                        });
                    }
                }
                "grass" | "forest" | "recreation_ground" | "village_green" => {
                    if coords.len() >= 3 {
                        polygons.push(MapPolygon {
                            kind: PolygonKind::Park,
                            coordinates: coords,
                        });
                    }
                }
                other => {
                    if coords.len() >= 3 {
                        polygons.push(MapPolygon {
                            kind: PolygonKind::Other(other.to_string()),
                            coordinates: coords,
                        });
                    }
                }
            }
            continue;
        }

        if matches!(tags.get("natural").map(String::as_str), Some("water")) {
            if coords.len() >= 3 {
                polygons.push(MapPolygon {
                    kind: PolygonKind::Water,
                    coordinates: coords,
                });
            }
            continue;
        }

        if matches!(
            tags.get("leisure").map(String::as_str),
            Some("park" | "garden")
        ) {
            if coords.len() >= 3 {
                polygons.push(MapPolygon {
                    kind: PolygonKind::Park,
                    coordinates: coords,
                });
            }
            continue;
        }

        // Fallback: if it has area-like geometry, keep it as generic polygon for potential debug rendering.
        if coords.len() >= 3 {
            if let Some(kind) = tags.get("area:highway").cloned() {
                polygons.push(MapPolygon {
                    kind: PolygonKind::Other(kind),
                    coordinates: coords,
                });
            }
        }
    }

    Ok(OsmMapData {
        center_lat: lat,
        center_lon: lon,
        polygons,
        lines,
    })
}

fn build_overpass_query(lat: f64, lon: f64, radius_m: f64) -> String {
    format!(
        r#"[out:json][timeout:25];
        (
          way["building"](around:{radius},{lat},{lon});
          way["highway"](around:{radius},{lat},{lon});
          way["landuse"](around:{radius},{lat},{lon});
          way["natural"="water"](around:{radius},{lat},{lon});
          way["leisure"="park"](around:{radius},{lat},{lon});
        );
        out geom;"#,
        radius = radius_m,
        lat = lat,
        lon = lon
    )
}

fn parse_height(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    let normalized = trimmed.to_lowercase();

    let mut numeric = String::new();
    for ch in normalized.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            numeric.push(ch);
        } else {
            break;
        }
    }

    if numeric.is_empty() {
        return None;
    }

    let mut height: f32 = numeric.parse().ok()?;

    if normalized.contains("ft") || normalized.contains("feet") {
        height *= 0.3048;
    }

    Some(height)
}

/// Converts a longitude/latitude pair into local planar meters relative to the given center.
pub fn lonlat_to_local(center_lat: f32, center_lon: f32, lon: f64, lat: f64) -> Vec2 {
    let lat0_rad = center_lat.to_radians();
    let meters_per_degree_lon = METERS_PER_DEGREE_LAT * lat0_rad.cos().max(0.0001);

    Vec2::new(
        ((lon as f32) - center_lon) * meters_per_degree_lon,
        ((lat as f32) - center_lat) * METERS_PER_DEGREE_LAT,
    )
}
