use anyhow::{anyhow, Context, Result};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

const PLANETARY_COMPUTER_STAC_SEARCH: &str =
    "https://planetarycomputer.microsoft.com/api/stac/v1/search";
const PLANETARY_COMPUTER_DATA_ITEM: &str =
    "https://planetarycomputer.microsoft.com/api/data/v1/item";

#[derive(Debug, Clone, Serialize)]
pub struct LandsatSceneCandidate {
    pub provider: String,
    pub collection: String,
    pub item_id: String,
    pub acquired_at: String,
    pub cloud_cover: Option<f64>,
    pub assets: BTreeMap<String, String>,
}

pub async fn search_best_scene(
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
) -> Result<Option<LandsatSceneCandidate>> {
    Ok(search_scenes(latitude, longitude, target_date, days, 10)
        .await?
        .into_iter()
        .next())
}

pub async fn search_scenes(
    latitude: f64,
    longitude: f64,
    target_date: &str,
    days: u8,
    limit: usize,
) -> Result<Vec<LandsatSceneCandidate>> {
    let date = NaiveDate::parse_from_str(target_date, "%Y-%m-%d")
        .with_context(|| format!("invalid target date: {target_date}"))?;
    let half_window = i64::from(days.saturating_sub(1)) / 2;
    let start = date - Duration::days(half_window);
    let end = date + Duration::days(i64::from(days.max(1)) - half_window - 1);
    let datetime = format!("{start}T00:00:00Z/{end}T23:59:59Z");

    let body = json!({
        "collections": ["landsat-c2-l2"],
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

    let client = reqwest::Client::new();
    let response = client
        .post(PLANETARY_COMPUTER_STAC_SEARCH)
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .json(&body)
        .send()
        .await
        .context("failed to call Landsat STAC search")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("Landsat STAC search failed with {status}: {text}"));
    }

    let collection: StacFeatureCollection = response
        .json()
        .await
        .context("failed to parse Landsat STAC response")?;

    let mut candidates = collection
        .features
        .into_iter()
        .filter_map(LandsatSceneCandidate::try_from_feature)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_cloud = left.cloud_cover.unwrap_or(f64::MAX);
        let right_cloud = right.cloud_cover.unwrap_or(f64::MAX);
        left_cloud
            .partial_cmp(&right_cloud)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.acquired_at.cmp(&right.acquired_at))
    });

    Ok(candidates)
}

pub async fn render_product_png(scene: &LandsatSceneCandidate, kind: &str) -> Result<Vec<u8>> {
    let render =
        product_render(kind).ok_or_else(|| anyhow!("unsupported Landsat product: {kind}"))?;
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
                    .append_pair("expression", expression)
                    .append_pair("asset_as_band", "true")
                    .append_pair("unscale", "true")
                    .append_pair("rescale", "-1,1")
                    .append_pair("colormap_name", colormap_name);
            }
        }
    }

    let response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .send()
        .await
        .with_context(|| format!("failed to render Landsat {kind} product"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Landsat {kind} render failed with {status}: {text}"
        ));
    }

    Ok(response.bytes().await?.to_vec())
}

pub async fn product_statistics(
    scene: &LandsatSceneCandidate,
    kind: &str,
    geometry: Option<&serde_json::Value>,
) -> Result<Option<serde_json::Value>> {
    let Some(ProductRender::Expression { expression, .. }) = product_render(kind) else {
        return Ok(None);
    };
    let mut url = reqwest::Url::parse(&format!("{PLANETARY_COMPUTER_DATA_ITEM}/statistics"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("collection", &scene.collection)
            .append_pair("item", &scene.item_id)
            .append_pair("expression", expression)
            .append_pair("asset_as_band", "true")
            .append_pair("unscale", "true")
            .append_pair("max_size", "512");
    }

    let client = reqwest::Client::new();
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
        .with_context(|| format!("failed to fetch Landsat {kind} statistics"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Landsat {kind} statistics failed with {status}: {text}"
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
    })))
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
        expression: &'static str,
        colormap_name: &'static str,
    },
}

fn product_render(kind: &str) -> Option<ProductRender> {
    match kind.to_lowercase().as_str() {
        "rgb" => Some(ProductRender::Assets {
            assets: &["red", "green", "blue"],
            color_formula: Some("gamma RGB 2.7, saturation 1.4, sigmoidal RGB 15 0.55"),
        }),
        "ndvi" => Some(ProductRender::Expression {
            expression: "(nir08-red)/(nir08+red)",
            colormap_name: "rdylgn",
        }),
        "ndmi" => Some(ProductRender::Expression {
            expression: "(nir08-swir16)/(nir08+swir16)",
            colormap_name: "viridis",
        }),
        "nbr" => Some(ProductRender::Expression {
            expression: "(nir08-swir22)/(nir08+swir22)",
            colormap_name: "plasma",
        }),
        "mndwi" => Some(ProductRender::Expression {
            expression: "(green-swir16)/(green+swir16)",
            colormap_name: "blues",
        }),
        "evi2" => Some(ProductRender::Expression {
            expression: "2.5*((nir08-red)/(nir08+2.4*red+1))",
            colormap_name: "rdylgn",
        }),
        "evi" => Some(ProductRender::Expression {
            expression: "2.5*((nir08-red)/(nir08+6*red-7.5*blue+1))",
            colormap_name: "rdylgn",
        }),
        "savi" => Some(ProductRender::Expression {
            expression: "1.5*((nir08-red)/(nir08+red+0.5))",
            colormap_name: "rdylgn",
        }),
        "vari" => Some(ProductRender::Expression {
            expression: "(green-red)/(green+red-blue)",
            colormap_name: "rdylgn",
        }),
        "gndvi" => Some(ProductRender::Expression {
            expression: "(nir08-green)/(nir08+green)",
            colormap_name: "rdylgn",
        }),
        "ndwi" => Some(ProductRender::Expression {
            expression: "(green-nir08)/(green+nir08)",
            colormap_name: "blues",
        }),
        _ => None,
    }
}

impl LandsatSceneCandidate {
    fn try_from_feature(feature: StacFeature) -> Option<Self> {
        let item_id = feature.id;
        let collection = feature
            .collection
            .unwrap_or_else(|| "landsat-c2-l2".to_string());
        let acquired_at = feature
            .properties
            .datetime
            .or(feature.properties.created)
            .unwrap_or_else(|| "unknown".to_string());
        let cloud_cover = feature.properties.cloud_cover;
        let assets = extract_assets(feature.assets);
        if assets.is_empty() {
            return None;
        }

        Some(Self {
            provider: "Microsoft Planetary Computer".to_string(),
            collection,
            item_id,
            acquired_at,
            cloud_cover,
            assets,
        })
    }
}

fn extract_assets(assets: BTreeMap<String, StacAsset>) -> BTreeMap<String, String> {
    let mut mapped = BTreeMap::new();
    for (target, candidates) in [
        ("B2", &["blue", "SR_B2", "B2"][..]),
        ("B3", &["green", "SR_B3", "B3"][..]),
        ("B4", &["red", "SR_B4", "B4"][..]),
        ("B5", &["nir08", "nir", "SR_B5", "B5"][..]),
        ("B6", &["swir16", "swir1", "SR_B6", "B6"][..]),
        ("B7", &["swir22", "swir2", "SR_B7", "B7"][..]),
        ("qa_pixel", &["qa_pixel", "QA_PIXEL", "qa"][..]),
    ] {
        if let Some(asset) = candidates
            .iter()
            .find_map(|candidate| assets.get(*candidate))
        {
            mapped.insert(target.to_string(), asset.href.clone());
        }
    }
    mapped
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
