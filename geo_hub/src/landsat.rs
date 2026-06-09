use anyhow::{anyhow, Context, Result};
use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;

const PLANETARY_COMPUTER_STAC_SEARCH: &str =
    "https://planetarycomputer.microsoft.com/api/stac/v1/search";
const PLANETARY_COMPUTER_DATA_ITEM: &str =
    "https://planetarycomputer.microsoft.com/api/data/v1/item";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatelliteDataset {
    Landsat,
    Sentinel2,
}

impl SatelliteDataset {
    fn collection_id(self) -> &'static str {
        match self {
            Self::Landsat => "landsat-c2-l2",
            Self::Sentinel2 => "sentinel-2-l2a",
        }
    }

    fn source_value(self) -> &'static str {
        match self {
            Self::Landsat => "landsat",
            Self::Sentinel2 => "sentinel2",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Landsat => "Landsat 8/9 Collection 2",
            Self::Sentinel2 => "Sentinel-2 L2A",
        }
    }

    fn resolution_m(self) -> f64 {
        match self {
            Self::Landsat => 30.0,
            Self::Sentinel2 => 10.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LandsatSceneCandidate {
    pub dataset: String,
    pub dataset_label: String,
    pub provider: String,
    pub collection: String,
    pub item_id: String,
    pub acquired_at: String,
    pub cloud_cover: Option<f64>,
    pub resolution_m: f64,
    pub asset_count: usize,
    pub assets: BTreeMap<String, String>,
}

pub fn datasets_for_source(source: &str) -> Vec<SatelliteDataset> {
    match source.trim().to_lowercase().as_str() {
        "sentinel" | "sentinel2" | "sentinel-2" | "sentinel_2" => vec![SatelliteDataset::Sentinel2],
        "landsat" | "landsat8" | "landsat9" => vec![SatelliteDataset::Landsat],
        "auto" | "" => vec![SatelliteDataset::Sentinel2, SatelliteDataset::Landsat],
        _ => vec![SatelliteDataset::Sentinel2, SatelliteDataset::Landsat],
    }
}

pub async fn search_best_scene(
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
) -> Result<Option<LandsatSceneCandidate>> {
    Ok(
        search_scenes_for_source("landsat", latitude, longitude, target_date, days, 10)
            .await?
            .into_iter()
            .next(),
    )
}

pub async fn search_scenes(
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
    limit: usize,
) -> Result<Vec<LandsatSceneCandidate>> {
    search_scenes_for_source("landsat", latitude, longitude, target_date, days, limit).await
}

pub async fn search_best_scene_for_source(
    source: &str,
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
) -> Result<Option<LandsatSceneCandidate>> {
    Ok(
        search_scenes_for_source(source, latitude, longitude, target_date, days, 10)
            .await?
            .into_iter()
            .next(),
    )
}

pub async fn search_scenes_for_source(
    source: &str,
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
    limit: usize,
) -> Result<Vec<LandsatSceneCandidate>> {
    let datasets = datasets_for_source(source);
    let allow_partial = datasets.len() > 1;
    let mut candidates = Vec::new();
    for dataset in datasets {
        match search_dataset_scenes(dataset, latitude, longitude, target_date, days, limit).await {
            Ok(found) => candidates.extend(found),
            Err(err) if allow_partial => {
                tracing::warn!(error = %err, dataset = dataset.label(), "satellite dataset search failed; continuing with remaining datasets");
            }
            Err(err) => return Err(err),
        }
    }
    sort_candidates(&mut candidates);
    candidates.truncate(limit.clamp(1, 25));
    Ok(candidates)
}

async fn search_dataset_scenes(
    dataset: SatelliteDataset,
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
    limit: usize,
) -> Result<Vec<LandsatSceneCandidate>> {
    let date = NaiveDate::parse_from_str(target_date, "%Y-%m-%d")
        .with_context(|| format!("invalid target date: {target_date}"))?;
    let half_window = i64::from(days.saturating_sub(1)) / 2;
    let start = date - ChronoDuration::days(half_window);
    let end = date + ChronoDuration::days(i64::from(days.max(1)) - half_window - 1);
    let datetime = format!("{start}T00:00:00Z/{end}T23:59:59Z");

    let body = json!({
        "collections": [dataset.collection_id()],
        "intersects": {
            "type": "Point",
            "coordinates": [longitude, latitude]
        },
        "datetime": datetime,
        "limit": limit.clamp(1, 25),
        "query": {
            "eo:cloud_cover": { "lt": 85.0 }
        }
    });

    let client = http_client()?;
    let response = client
        .post(PLANETARY_COMPUTER_STAC_SEARCH)
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to call {} STAC search", dataset.label()))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "{} STAC search failed with {status}: {text}",
            dataset.label()
        ));
    }

    let collection: StacFeatureCollection = response
        .json()
        .await
        .with_context(|| format!("failed to parse {} STAC response", dataset.label()))?;

    let mut candidates = collection
        .features
        .into_iter()
        .filter_map(|feature| LandsatSceneCandidate::try_from_feature(dataset, feature))
        .collect::<Vec<_>>();
    sort_candidates(&mut candidates);

    Ok(candidates)
}

fn sort_candidates(candidates: &mut [LandsatSceneCandidate]) {
    candidates.sort_by(|left, right| {
        let left_cloud = left.cloud_cover.unwrap_or(f64::MAX);
        let right_cloud = right.cloud_cover.unwrap_or(f64::MAX);
        left_cloud
            .partial_cmp(&right_cloud)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.acquired_at.cmp(&right.acquired_at))
            .then_with(|| left.dataset.cmp(&right.dataset))
    });
}

pub async fn render_product_png(scene: &LandsatSceneCandidate, kind: &str) -> Result<Vec<u8>> {
    let render = product_render(scene, kind)
        .ok_or_else(|| anyhow!("unsupported {} product: {kind}", scene.dataset_label))?;
    let mut url = reqwest::Url::parse(&format!("{PLANETARY_COMPUTER_DATA_ITEM}/preview.png"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("collection", &scene.collection)
            .append_pair("item", &scene.item_id)
            .append_pair("format", "png")
            .append_pair("width", "512")
            .append_pair("height", "512");
        match render {
            ProductRender::Assets {
                assets,
                color_formula,
            } => {
                for asset in assets {
                    query.append_pair("assets", asset);
                }
                if let Some(color_formula) = color_formula {
                    query.append_pair("color_formula", color_formula);
                }
            }
            ProductRender::Expression {
                expression,
                colormap_name,
            } => {
                query
                    .append_pair("expression", &expression)
                    .append_pair("asset_as_band", "true")
                    .append_pair("unscale", "true")
                    .append_pair("rescale", "-1,1")
                    .append_pair("colormap_name", colormap_name);
            }
        }
    }

    let response = http_client()?
        .get(url)
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .send()
        .await
        .with_context(|| format!("failed to render {} {kind} product", scene.dataset_label))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "{} {kind} render failed with {status}: {text}",
            scene.dataset_label
        ));
    }

    Ok(response.bytes().await?.to_vec())
}

pub async fn product_statistics(
    scene: &LandsatSceneCandidate,
    kind: &str,
    geometry: Option<&serde_json::Value>,
) -> Result<Option<serde_json::Value>> {
    let Some(ProductRender::Expression { expression, .. }) = product_render(scene, kind) else {
        return Ok(None);
    };
    let mut url = reqwest::Url::parse(&format!("{PLANETARY_COMPUTER_DATA_ITEM}/statistics"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("collection", &scene.collection)
            .append_pair("item", &scene.item_id)
            .append_pair("expression", &expression)
            .append_pair("asset_as_band", "true")
            .append_pair("unscale", "true")
            .append_pair("max_size", "512");
    }

    let client = http_client()?;
    let request = if let Some(geometry) = geometry {
        let feature = json!({
            "type": "Feature",
            "properties": {},
            "geometry": geometry,
        });
        client.post(url).json(&feature)
    } else {
        client.get(url)
    };
    let response = request
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .send()
        .await
        .with_context(|| format!("failed to fetch {} {kind} statistics", scene.dataset_label))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "{} {kind} statistics failed with {status}: {text}",
            scene.dataset_label
        ));
    }

    let value: serde_json::Value = response.json().await?;
    let Some(stats) = extract_statistics_value(&value).cloned() else {
        return Ok(None);
    };
    Ok(Some(json!({
        "index": kind,
        "min": stats.get("min").cloned().unwrap_or(serde_json::Value::Null),
        "max": stats.get("max").cloned().unwrap_or(serde_json::Value::Null),
        "mean": stats.get("mean").cloned().unwrap_or(serde_json::Value::Null),
        "std": stats.get("std").cloned().unwrap_or(serde_json::Value::Null),
        "count": stats.get("count").cloned().unwrap_or(serde_json::Value::Null),
        "masked_pixels": stats.get("masked_pixels").cloned().unwrap_or(serde_json::Value::Null),
        "valid_percent": stats.get("valid_percent").cloned().unwrap_or(serde_json::Value::Null),
        "valid_pixels": stats.get("valid_pixels").cloned().unwrap_or(serde_json::Value::Null),
        "percentile_2": stats.get("percentile_2").cloned().unwrap_or(serde_json::Value::Null),
        "percentile_98": stats.get("percentile_98").cloned().unwrap_or(serde_json::Value::Null),
        "summary_scope": if geometry.is_some() { "field_aoi" } else { "scene" },
        "source_scene": scene.item_id,
        "provider": scene.provider,
        "dataset": scene.dataset,
        "dataset_label": scene.dataset_label,
        "resolution_m": scene.resolution_m,
    })))
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .context("failed to build HTTP client")
}

fn extract_statistics_value(value: &serde_json::Value) -> Option<&serde_json::Value> {
    value
        .get("properties")
        .and_then(|properties| properties.get("statistics"))
        .and_then(|statistics| statistics.as_object())
        .and_then(|object| object.values().next())
        .or_else(|| value.as_object().and_then(|object| object.values().next()))
}

enum ProductRender {
    Assets {
        assets: &'static [&'static str],
        color_formula: Option<&'static str>,
    },
    Expression {
        expression: String,
        colormap_name: &'static str,
    },
}

fn product_render(scene: &LandsatSceneCandidate, kind: &str) -> Option<ProductRender> {
    match scene.dataset.as_str() {
        "sentinel2" => sentinel2_product_render(kind),
        _ => landsat_product_render(kind),
    }
}

fn landsat_product_render(kind: &str) -> Option<ProductRender> {
    product_render_for_bands(
        kind,
        BandSet {
            blue: "blue",
            green: "green",
            red: "red",
            nir: "nir08",
            swir1: "swir16",
            swir2: "swir22",
            rgb: &["red", "green", "blue"],
            color_formula: "gamma RGB 2.7, saturation 1.4, sigmoidal RGB 15 0.55",
        },
    )
}

fn sentinel2_product_render(kind: &str) -> Option<ProductRender> {
    product_render_for_bands(
        kind,
        BandSet {
            blue: "B02",
            green: "B03",
            red: "B04",
            nir: "B08",
            swir1: "B11",
            swir2: "B12",
            rgb: &["B04", "B03", "B02"],
            color_formula: "gamma RGB 2.2, saturation 1.3, sigmoidal RGB 15 0.45",
        },
    )
}

struct BandSet {
    blue: &'static str,
    green: &'static str,
    red: &'static str,
    nir: &'static str,
    swir1: &'static str,
    swir2: &'static str,
    rgb: &'static [&'static str],
    color_formula: &'static str,
}

fn product_render_for_bands(kind: &str, bands: BandSet) -> Option<ProductRender> {
    match kind.to_lowercase().as_str() {
        "rgb" => Some(ProductRender::Assets {
            assets: bands.rgb,
            color_formula: Some(bands.color_formula),
        }),
        "ndvi" => Some(ProductRender::Expression {
            expression: format!(
                "({nir}-{red})/({nir}+{red})",
                nir = bands.nir,
                red = bands.red
            ),
            colormap_name: "rdylgn",
        }),
        "ndmi" => Some(ProductRender::Expression {
            expression: format!(
                "({nir}-{swir1})/({nir}+{swir1})",
                nir = bands.nir,
                swir1 = bands.swir1
            ),
            colormap_name: "viridis",
        }),
        "nbr" => Some(ProductRender::Expression {
            expression: format!(
                "({nir}-{swir2})/({nir}+{swir2})",
                nir = bands.nir,
                swir2 = bands.swir2
            ),
            colormap_name: "plasma",
        }),
        "mndwi" => Some(ProductRender::Expression {
            expression: format!(
                "({green}-{swir1})/({green}+{swir1})",
                green = bands.green,
                swir1 = bands.swir1
            ),
            colormap_name: "blues",
        }),
        "evi2" => Some(ProductRender::Expression {
            expression: format!(
                "2.5*(({nir}-{red})/({nir}+2.4*{red}+1))",
                nir = bands.nir,
                red = bands.red
            ),
            colormap_name: "rdylgn",
        }),
        "evi" => Some(ProductRender::Expression {
            expression: format!(
                "2.5*(({nir}-{red})/({nir}+6*{red}-7.5*{blue}+1))",
                nir = bands.nir,
                red = bands.red,
                blue = bands.blue
            ),
            colormap_name: "rdylgn",
        }),
        "savi" => Some(ProductRender::Expression {
            expression: format!(
                "1.5*(({nir}-{red})/({nir}+{red}+0.5))",
                nir = bands.nir,
                red = bands.red
            ),
            colormap_name: "rdylgn",
        }),
        "vari" => Some(ProductRender::Expression {
            expression: format!(
                "({green}-{red})/({green}+{red}-{blue})",
                green = bands.green,
                red = bands.red,
                blue = bands.blue
            ),
            colormap_name: "rdylgn",
        }),
        "gndvi" => Some(ProductRender::Expression {
            expression: format!(
                "({nir}-{green})/({nir}+{green})",
                nir = bands.nir,
                green = bands.green
            ),
            colormap_name: "rdylgn",
        }),
        "ndwi" => Some(ProductRender::Expression {
            expression: format!(
                "({green}-{nir})/({green}+{nir})",
                green = bands.green,
                nir = bands.nir
            ),
            colormap_name: "blues",
        }),
        _ => None,
    }
}

impl LandsatSceneCandidate {
    fn try_from_feature(dataset: SatelliteDataset, feature: StacFeature) -> Option<Self> {
        let item_id = feature.id;
        let collection = feature
            .collection
            .unwrap_or_else(|| dataset.collection_id().to_string());
        let acquired_at = feature
            .properties
            .datetime
            .or(feature.properties.created)
            .unwrap_or_else(|| "unknown".to_string());
        let cloud_cover = feature.properties.cloud_cover;
        let assets = extract_assets(dataset, feature.assets);
        if assets.is_empty() {
            return None;
        }

        Some(Self {
            dataset: dataset.source_value().to_string(),
            dataset_label: dataset.label().to_string(),
            provider: "Microsoft Planetary Computer".to_string(),
            collection,
            item_id,
            acquired_at,
            cloud_cover,
            resolution_m: dataset.resolution_m(),
            asset_count: assets.len(),
            assets,
        })
    }
}

fn extract_assets(
    dataset: SatelliteDataset,
    assets: BTreeMap<String, StacAsset>,
) -> BTreeMap<String, String> {
    let mut mapped = BTreeMap::new();
    for (target, candidates) in asset_candidates(dataset) {
        if let Some(asset) = candidates
            .iter()
            .find_map(|candidate| assets.get(*candidate))
        {
            mapped.insert(target.to_string(), asset.href.clone());
        }
    }
    mapped
}

fn asset_candidates(
    dataset: SatelliteDataset,
) -> &'static [(&'static str, &'static [&'static str])] {
    match dataset {
        SatelliteDataset::Landsat => &[
            ("B2", &["blue", "SR_B2", "B2"]),
            ("B3", &["green", "SR_B3", "B3"]),
            ("B4", &["red", "SR_B4", "B4"]),
            ("B5", &["nir08", "nir", "SR_B5", "B5"]),
            ("B6", &["swir16", "swir1", "SR_B6", "B6"]),
            ("B7", &["swir22", "swir2", "SR_B7", "B7"]),
            ("qa_pixel", &["qa_pixel", "QA_PIXEL", "qa"]),
        ],
        SatelliteDataset::Sentinel2 => &[
            ("B02", &["B02", "blue"]),
            ("B03", &["B03", "green"]),
            ("B04", &["B04", "red"]),
            ("B08", &["B08", "nir"]),
            ("B11", &["B11", "swir16"]),
            ("B12", &["B12", "swir22"]),
            ("SCL", &["SCL"]),
        ],
    }
}

#[derive(Debug, Deserialize)]
struct StacFeatureCollection {
    features: Vec<StacFeature>,
}

#[derive(Debug, Deserialize)]
struct StacFeature {
    id: String,
    collection: Option<String>,
    properties: StacProperties,
    assets: BTreeMap<String, StacAsset>,
}

#[derive(Debug, Deserialize)]
struct StacProperties {
    datetime: Option<String>,
    created: Option<String>,
    #[serde(rename = "eo:cloud_cover")]
    cloud_cover: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct StacAsset {
    href: String,
}
