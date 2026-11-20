//! Public, read-only exploration of a single archive pixel. No farm database or
//! user-supplied asset URLs are involved.
use crate::explorer_observation::{asset, observation, Observation, ROLES};
use crate::landsat::{PLANETARY_COMPUTER_DATA_ITEM, PLANETARY_COMPUTER_STAC_SEARCH};
use crate::landsat_derive::{instrument_for_scene, LandsatInstrument};
use axum::{extract::Query, http::StatusCode, response::Html, Json};
use imagery_processor::pipeline::calibration::SensorProfile;
use serde::Deserialize;
use std::{sync::LazyLock, time::Duration};

pub async fn explorer_home() -> Html<&'static str> {
    Html(include_str!("../../web/explorer/index.html"))
}

#[derive(Deserialize)]
pub struct ObservationQuery {
    collection: String,
    item: String,
    latitude: f64,
    longitude: f64,
}

type RequestError = (StatusCode, &'static str);
const PROVIDER_ERROR: RequestError = (StatusCode::BAD_GATEWAY, "Satellite provider unavailable");
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);
static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(16);

async fn provider_json<T: serde::de::DeserializeOwned>(
    url: reqwest::Url,
) -> Result<T, RequestError> {
    CLIENT
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| PROVIDER_ERROR)?
        .json()
        .await
        .map_err(|_| PROVIDER_ERROR)
}

pub async fn explore_observation(
    Query(q): Query<ObservationQuery>,
) -> Result<Json<Observation>, RequestError> {
    super::validate_lat_lon(q.latitude, q.longitude)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid coordinates"))?;
    let sentinel = match q.collection.as_str() {
        "sentinel-2-l2a" => true,
        "landsat-c2-l2" => false,
        _ => return Err((StatusCode::BAD_REQUEST, "Unsupported collection")),
    };
    if q.item.is_empty()
        || q.item.len() > 150
        || !q
            .item
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err((StatusCode::BAD_REQUEST, "Invalid scene ID"));
    }
    let _slot = SLOTS.try_acquire().map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Explorer busy. Try again shortly.",
        )
    })?;
    let profile = if sentinel {
        let base = PLANETARY_COMPUTER_STAC_SEARCH.trim_end_matches("/search");
        let url = format!("{base}/collections/sentinel-2-l2a/items/{}", q.item);
        let metadata: crate::earth_search::EarthSearchItem =
            provider_json(url.parse().map_err(|_| PROVIDER_ERROR)?).await?;
        let baseline = metadata
            .s2_processing_baseline()
            .filter(|v| v.is_finite())
            .ok_or((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Missing Sentinel processing baseline",
            ))?;
        if baseline >= 4.0 {
            SensorProfile::Sentinel2L2ABaseline0400
        } else {
            SensorProfile::Sentinel2L2ALegacy
        }
    } else {
        SensorProfile::LandsatC2L2Sr
    };
    let mut url = reqwest::Url::parse(&format!(
        "{PLANETARY_COMPUTER_DATA_ITEM}/point/{},{}",
        q.longitude, q.latitude
    ))
    .map_err(|_| PROVIDER_ERROR)?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("collection", &q.collection)
            .append_pair("item", &q.item);
        for role in ROLES {
            query.append_pair("assets", asset(role, sentinel));
        }
        if sentinel {
            query.append_pair("assets", "SCL");
        } else {
            let surface_qa = match instrument_for_scene(&q.item) {
                LandsatInstrument::Oli => "qa_aerosol",
                LandsatInstrument::TmEtm => "cloud_qa",
            };
            for key in ["qa_pixel", "qa_radsat", surface_qa] {
                query.append_pair("assets", key);
            }
        }
    }
    Ok(Json(observation(
        &provider_json(url).await?,
        profile,
        &q.item,
    )))
}
