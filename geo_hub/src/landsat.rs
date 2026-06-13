use anyhow::{anyhow, Context, Result};
use chrono::{Duration as ChronoDuration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::schemas::GeoBounds;
use std::collections::BTreeMap;
use std::{
    env, fmt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::fs as async_fs;

const PLANETARY_COMPUTER_STAC_SEARCH: &str =
    "https://planetarycomputer.microsoft.com/api/stac/v1/search";
const PLANETARY_COMPUTER_DATA_ITEM: &str =
    "https://planetarycomputer.microsoft.com/api/data/v1/item";
const USGS_M2M_JSON_API: &str = "https://m2m.cr.usgs.gov/api/api/json/stable";
const USGS_LANDSAT_DATASET: &str = "landsat_ot_c2_l2";
pub const AGBOT_USGS_USERNAME_ENV: &str = "AGBOT_USGS_USERNAME";
pub const AGBOT_USGS_PASSWORD_ENV: &str = "AGBOT_USGS_PASSWORD";
pub const AGBOT_RUN_USGS_INTEGRATION_ENV: &str = "AGBOT_RUN_USGS_INTEGRATION";

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
    pub bbox: Option<GeoBounds>,
    pub resolution_m: f64,
    pub asset_count: usize,
    pub assets: BTreeMap<String, String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct UsgsCredentials {
    username: String,
    password: String,
}

impl UsgsCredentials {
    pub fn new(
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> UsgsVerificationResult<Self> {
        let username = username.into();
        let password = password.into();
        if username.trim().is_empty() {
            return Err(UsgsVerificationError::new(
                UsgsVerificationErrorCode::MissingCredentials,
                "USGS username is required",
                None,
            ));
        }
        if password.trim().is_empty() {
            return Err(UsgsVerificationError::new(
                UsgsVerificationErrorCode::MissingCredentials,
                "USGS password is required",
                None,
            ));
        }
        Ok(Self { username, password })
    }

    pub fn from_env() -> UsgsVerificationResult<Self> {
        let username = env::var(AGBOT_USGS_USERNAME_ENV).map_err(|_| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::MissingCredentials,
                format!("{AGBOT_USGS_USERNAME_ENV} is required"),
                None,
            )
        })?;
        let password = env::var(AGBOT_USGS_PASSWORD_ENV).map_err(|_| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::MissingCredentials,
                format!("{AGBOT_USGS_PASSWORD_ENV} is required"),
                None,
            )
        })?;
        Self::new(username, password)
    }

    fn secret_values(&self) -> [&str; 2] {
        [self.username.as_str(), self.password.as_str()]
    }
}

impl fmt::Debug for UsgsCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UsgsCredentials")
            .field("username", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsgsVerificationErrorCode {
    MissingCredentials,
    AuthFailed,
    NotFound,
    SearchFailed,
    DownloadFailed,
    StoreFailed,
    InvalidResponse,
}

impl UsgsVerificationErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingCredentials => "missing_credentials",
            Self::AuthFailed => "auth_failed",
            Self::NotFound => "not_found",
            Self::SearchFailed => "search_failed",
            Self::DownloadFailed => "download_failed",
            Self::StoreFailed => "store_failed",
            Self::InvalidResponse => "invalid_response",
        }
    }
}

impl fmt::Display for UsgsVerificationErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsgsVerificationError {
    code: UsgsVerificationErrorCode,
    message: String,
}

impl UsgsVerificationError {
    fn new(
        code: UsgsVerificationErrorCode,
        message: impl Into<String>,
        credentials: Option<&UsgsCredentials>,
    ) -> Self {
        let mut message = message.into();
        if let Some(credentials) = credentials {
            message = sanitize_usgs_error_message(message, credentials);
        }
        Self { code, message }
    }

    pub fn code(&self) -> UsgsVerificationErrorCode {
        self.code
    }
}

impl fmt::Display for UsgsVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for UsgsVerificationError {}

pub type UsgsVerificationResult<T> = std::result::Result<T, UsgsVerificationError>;

#[derive(Debug, Clone)]
pub struct UsgsIngestVerificationRequest {
    pub credentials: UsgsCredentials,
    pub latitude: f64,
    pub longitude: f64,
    pub target_date: String,
    pub days: u8,
    pub limit: usize,
    pub output_root: PathBuf,
}

impl UsgsIngestVerificationRequest {
    pub fn from_env(output_root: impl Into<PathBuf>) -> UsgsVerificationResult<Self> {
        let latitude = parse_env_f64("AGBOT_USGS_LATITUDE", 41.25)?;
        let longitude = parse_env_f64("AGBOT_USGS_LONGITUDE", -96.45)?;
        let target_date =
            env::var("AGBOT_USGS_TARGET_DATE").unwrap_or_else(|_| "2024-06-15".to_string());
        let days = parse_env_u8("AGBOT_USGS_WINDOW_DAYS", 30)?;
        let limit = parse_env_usize("AGBOT_USGS_LIMIT", 5)?;
        Ok(Self {
            credentials: UsgsCredentials::from_env()?,
            latitude,
            longitude,
            target_date,
            days,
            limit,
            output_root: output_root.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsgsSceneSummary {
    pub scene_id: String,
    pub display_id: Option<String>,
    pub dataset_name: String,
    pub provider: String,
    pub acquired_at: Option<String>,
    pub cloud_cover: Option<f64>,
    pub bbox: Option<GeoBounds>,
    pub browse_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsgsIngestVerificationRecord {
    pub scene: UsgsSceneSummary,
    pub metadata_path: PathBuf,
    pub downloaded_browse_path: PathBuf,
    pub stored_at: String,
}

pub async fn verify_credentialed_usgs_landsat_ingest(
    request: UsgsIngestVerificationRequest,
) -> UsgsVerificationResult<UsgsIngestVerificationRecord> {
    let client = http_client().map_err(|err| {
        UsgsVerificationError::new(
            UsgsVerificationErrorCode::SearchFailed,
            err.to_string(),
            Some(&request.credentials),
        )
    })?;
    let api_key = authenticate_usgs(&client, &request.credentials).await?;
    let scene = search_usgs_landsat_scene(&client, &api_key, &request).await?;
    let browse_url = scene.browse_url.clone().ok_or_else(|| {
        UsgsVerificationError::new(
            UsgsVerificationErrorCode::DownloadFailed,
            format!("USGS scene {} did not include a browse URL", scene.scene_id),
            Some(&request.credentials),
        )
    })?;
    let browse_bytes =
        download_usgs_browse(&client, &api_key, &browse_url, &request.credentials).await?;
    store_usgs_verification(&request.output_root, scene, &browse_url, &browse_bytes).await
}

fn parse_env_f64(name: &str, default: f64) -> UsgsVerificationResult<f64> {
    match env::var(name) {
        Ok(value) => value.parse::<f64>().map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::InvalidResponse,
                format!("{name} must be numeric: {err}"),
                None,
            )
        }),
        Err(_) => Ok(default),
    }
}

fn parse_env_u8(name: &str, default: u8) -> UsgsVerificationResult<u8> {
    match env::var(name) {
        Ok(value) => value.parse::<u8>().map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::InvalidResponse,
                format!("{name} must be an integer from 0 to 255: {err}"),
                None,
            )
        }),
        Err(_) => Ok(default),
    }
}

fn parse_env_usize(name: &str, default: usize) -> UsgsVerificationResult<usize> {
    match env::var(name) {
        Ok(value) => value.parse::<usize>().map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::InvalidResponse,
                format!("{name} must be a positive integer: {err}"),
                None,
            )
        }),
        Err(_) => Ok(default),
    }
}

async fn authenticate_usgs(
    client: &reqwest::Client,
    credentials: &UsgsCredentials,
) -> UsgsVerificationResult<String> {
    let response = client
        .post(format!("{USGS_M2M_JSON_API}/login"))
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .json(&json!({
            "username": &credentials.username,
            "password": &credentials.password,
        }))
        .send()
        .await
        .map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::SearchFailed,
                format!("failed to call USGS login: {err}"),
                Some(credentials),
            )
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    parse_usgs_login_response(status, &body, credentials)
}

async fn search_usgs_landsat_scene(
    client: &reqwest::Client,
    api_key: &str,
    request: &UsgsIngestVerificationRequest,
) -> UsgsVerificationResult<UsgsSceneSummary> {
    let date = NaiveDate::parse_from_str(&request.target_date, "%Y-%m-%d").map_err(|err| {
        UsgsVerificationError::new(
            UsgsVerificationErrorCode::SearchFailed,
            format!("invalid USGS target date {}: {err}", request.target_date),
            Some(&request.credentials),
        )
    })?;
    let half_window = i64::from(request.days.saturating_sub(1)) / 2;
    let start = date - ChronoDuration::days(half_window);
    let end = date + ChronoDuration::days(i64::from(request.days.max(1)) - half_window - 1);
    let delta = 0.01_f64;
    let body = json!({
        "datasetName": USGS_LANDSAT_DATASET,
        "maxResults": request.limit.clamp(1, 25),
        "metadataType": "summary",
        "sceneFilter": {
            "spatialFilter": {
                "filterType": "mbr",
                "lowerLeft": {
                    "latitude": request.latitude - delta,
                    "longitude": request.longitude - delta,
                },
                "upperRight": {
                    "latitude": request.latitude + delta,
                    "longitude": request.longitude + delta,
                }
            },
            "acquisitionFilter": {
                "start": start.to_string(),
                "end": end.to_string(),
            },
            "cloudCoverFilter": {
                "max": 85,
                "includeUnknown": true,
            }
        }
    });
    let response = client
        .post(format!("{USGS_M2M_JSON_API}/scene-search"))
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .header("X-Auth-Token", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::SearchFailed,
                format!("failed to call USGS scene-search: {err}"),
                Some(&request.credentials),
            )
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    parse_usgs_scene_search_response(status, &body, &request.credentials)
}

async fn download_usgs_browse(
    client: &reqwest::Client,
    api_key: &str,
    browse_url: &str,
    credentials: &UsgsCredentials,
) -> UsgsVerificationResult<Vec<u8>> {
    let response = client
        .get(browse_url)
        .header(reqwest::header::USER_AGENT, "agbot-geo-hub/0.1")
        .header("X-Auth-Token", api_key)
        .send()
        .await
        .map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::DownloadFailed,
                format!("failed to download USGS browse asset: {err}"),
                Some(credentials),
            )
        })?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let body = response.text().await.unwrap_or_default();
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::AuthFailed,
            format!("USGS browse download rejected credentials with {status}: {body}"),
            Some(credentials),
        ));
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::NotFound,
            format!("USGS browse asset was not found: {browse_url}"),
            Some(credentials),
        ));
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::DownloadFailed,
            format!("USGS browse download failed with {status}: {body}"),
            Some(credentials),
        ));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::DownloadFailed,
                format!("failed to read USGS browse bytes: {err}"),
                Some(credentials),
            )
        })
}

async fn store_usgs_verification(
    output_root: &Path,
    scene: UsgsSceneSummary,
    browse_url: &str,
    browse_bytes: &[u8],
) -> UsgsVerificationResult<UsgsIngestVerificationRecord> {
    let scene_dir = output_root.join(sanitize_scene_component(&scene.scene_id));
    async_fs::create_dir_all(&scene_dir).await.map_err(|err| {
        UsgsVerificationError::new(
            UsgsVerificationErrorCode::StoreFailed,
            format!("failed to create USGS verification directory: {err}"),
            None,
        )
    })?;
    let browse_path = scene_dir.join(format!("browse.{}", browse_extension(browse_url)));
    async_fs::write(&browse_path, browse_bytes)
        .await
        .map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::StoreFailed,
                format!("failed to store USGS browse asset: {err}"),
                None,
            )
        })?;
    let metadata_path = scene_dir.join("metadata_usgs_verification.json");
    let record = UsgsIngestVerificationRecord {
        scene,
        metadata_path: metadata_path.clone(),
        downloaded_browse_path: browse_path,
        stored_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let metadata = serde_json::to_vec_pretty(&record).map_err(|err| {
        UsgsVerificationError::new(
            UsgsVerificationErrorCode::StoreFailed,
            format!("failed to encode USGS verification metadata: {err}"),
            None,
        )
    })?;
    async_fs::write(&metadata_path, metadata)
        .await
        .map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::StoreFailed,
                format!("failed to store USGS verification metadata: {err}"),
                None,
            )
        })?;
    Ok(record)
}

fn parse_usgs_login_response(
    status: reqwest::StatusCode,
    body: &str,
    credentials: &UsgsCredentials,
) -> UsgsVerificationResult<String> {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::AuthFailed,
            format!("USGS login rejected credentials with {status}: {body}"),
            Some(credentials),
        ));
    }
    if !status.is_success() {
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::SearchFailed,
            format!("USGS login failed with {status}: {body}"),
            Some(credentials),
        ));
    }
    let envelope = decode_usgs_envelope(body, UsgsVerificationErrorCode::AuthFailed, credentials)?;
    if let Some(error) = envelope.error_for(UsgsVerificationErrorCode::AuthFailed, credentials) {
        return Err(error);
    }
    envelope
        .data
        .and_then(|data| {
            data.as_str()
                .map(ToOwned::to_owned)
                .or_else(|| value_string(&data, &["apiKey", "api_key", "token"]))
        })
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::InvalidResponse,
                "USGS login response did not include an API key",
                Some(credentials),
            )
        })
}

fn parse_usgs_scene_search_response(
    status: reqwest::StatusCode,
    body: &str,
    credentials: &UsgsCredentials,
) -> UsgsVerificationResult<UsgsSceneSummary> {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::AuthFailed,
            format!("USGS scene-search rejected credentials with {status}: {body}"),
            Some(credentials),
        ));
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::NotFound,
            format!("USGS scene-search endpoint returned not found: {body}"),
            Some(credentials),
        ));
    }
    if !status.is_success() {
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::SearchFailed,
            format!("USGS scene-search failed with {status}: {body}"),
            Some(credentials),
        ));
    }
    let envelope =
        decode_usgs_envelope(body, UsgsVerificationErrorCode::SearchFailed, credentials)?;
    if let Some(error) = envelope.error_for(UsgsVerificationErrorCode::SearchFailed, credentials) {
        return Err(error);
    }
    let data = envelope.data.ok_or_else(|| {
        UsgsVerificationError::new(
            UsgsVerificationErrorCode::InvalidResponse,
            "USGS scene-search response did not include data",
            Some(credentials),
        )
    })?;
    let results = data
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::InvalidResponse,
                "USGS scene-search response did not include results",
                Some(credentials),
            )
        })?;
    if results.is_empty() {
        return Err(UsgsVerificationError::new(
            UsgsVerificationErrorCode::NotFound,
            "USGS scene-search returned no Landsat scenes",
            Some(credentials),
        ));
    }
    results
        .iter()
        .find_map(usgs_scene_summary_from_value)
        .ok_or_else(|| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::InvalidResponse,
                "USGS scene-search results did not include a usable scene identifier",
                Some(credentials),
            )
        })
}

#[derive(Debug, Deserialize)]
struct UsgsEnvelope {
    #[serde(rename = "errorCode")]
    error_code: Option<String>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
    data: Option<Value>,
}

impl UsgsEnvelope {
    fn error_for(
        &self,
        default_code: UsgsVerificationErrorCode,
        credentials: &UsgsCredentials,
    ) -> Option<UsgsVerificationError> {
        let code = self.error_code.as_deref().unwrap_or_default().trim();
        let message = self.error_message.as_deref().unwrap_or_default().trim();
        if code.is_empty() && message.is_empty() {
            return None;
        }
        let classified = classify_usgs_error_code(code, message, default_code);
        Some(UsgsVerificationError::new(
            classified,
            format!("USGS API error {code}: {message}"),
            Some(credentials),
        ))
    }
}

fn decode_usgs_envelope(
    body: &str,
    code: UsgsVerificationErrorCode,
    credentials: &UsgsCredentials,
) -> UsgsVerificationResult<UsgsEnvelope> {
    serde_json::from_str(body).map_err(|err| {
        UsgsVerificationError::new(
            UsgsVerificationErrorCode::InvalidResponse,
            format!("failed to decode USGS response for {code}: {err}"),
            Some(credentials),
        )
    })
}

fn classify_usgs_error_code(
    code: &str,
    message: &str,
    default_code: UsgsVerificationErrorCode,
) -> UsgsVerificationErrorCode {
    let text = format!("{code} {message}").to_lowercase();
    if text.contains("auth")
        || text.contains("credential")
        || text.contains("login")
        || text.contains("password")
        || text.contains("token")
        || text.contains("unauthorized")
        || text.contains("forbidden")
    {
        UsgsVerificationErrorCode::AuthFailed
    } else if text.contains("not_found")
        || text.contains("not found")
        || text.contains("no scenes")
        || text.contains("no results")
    {
        UsgsVerificationErrorCode::NotFound
    } else {
        default_code
    }
}

fn usgs_scene_summary_from_value(value: &Value) -> Option<UsgsSceneSummary> {
    let scene_id = value_string(
        value,
        &["entityId", "entity_id", "displayId", "display_id", "id"],
    )?;
    Some(UsgsSceneSummary {
        scene_id,
        display_id: value_string(value, &["displayId", "display_id", "name"]),
        dataset_name: value_string(value, &["datasetName", "dataset_name"])
            .unwrap_or_else(|| USGS_LANDSAT_DATASET.to_string()),
        provider: "USGS".to_string(),
        acquired_at: value_string(
            value,
            &[
                "acquisitionDate",
                "acquired",
                "startDate",
                "publishDate",
                "temporalCoverage.startDate",
            ],
        ),
        cloud_cover: value_f64(value, &["cloudCover", "cloud_cover"]),
        bbox: usgs_bbox(value),
        browse_url: usgs_browse_url(value),
    })
}

fn value_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value_at_path(value, key))
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn value_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .filter_map(|key| value_at_path(value, key))
        .find_map(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
}

fn value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |current, key| current.get(key))
}

fn usgs_bbox(value: &Value) -> Option<GeoBounds> {
    let bounds = value
        .get("spatialBounds")
        .or_else(|| value.get("spatial_bounds"))
        .or_else(|| value.get("bbox"))?;
    if let Some(array) = bounds.as_array() {
        if array.len() == 4 {
            return Some(GeoBounds {
                min_lon: array.first()?.as_f64()?,
                min_lat: array.get(1)?.as_f64()?,
                max_lon: array.get(2)?.as_f64()?,
                max_lat: array.get(3)?.as_f64()?,
            });
        }
    }
    let west = value_f64(bounds, &["west", "minLon", "min_lon", "longitude.min"]);
    let east = value_f64(bounds, &["east", "maxLon", "max_lon", "longitude.max"]);
    let south = value_f64(bounds, &["south", "minLat", "min_lat", "latitude.min"]);
    let north = value_f64(bounds, &["north", "maxLat", "max_lat", "latitude.max"]);
    match (west, south, east, north) {
        (Some(min_lon), Some(min_lat), Some(max_lon), Some(max_lat)) => Some(GeoBounds {
            min_lon,
            min_lat,
            max_lon,
            max_lat,
        }),
        _ => None,
    }
}

fn usgs_browse_url(value: &Value) -> Option<String> {
    value_string(
        value,
        &["browseUrl", "browse_url", "thumbnailUrl", "thumbnail_url"],
    )
    .or_else(|| {
        let browse = value.get("browse")?;
        if let Some(array) = browse.as_array() {
            return array.iter().find_map(|entry| {
                value_string(
                    entry,
                    &["browsePath", "browse_path", "url", "href", "imageUrl"],
                )
            });
        }
        value_string(
            browse,
            &["browsePath", "browse_path", "url", "href", "imageUrl"],
        )
    })
}

fn sanitize_usgs_error_message(message: String, credentials: &UsgsCredentials) -> String {
    credentials
        .secret_values()
        .into_iter()
        .filter(|secret| !secret.trim().is_empty())
        .fold(message, |redacted, secret| {
            redacted.replace(secret, "<redacted>")
        })
}

fn sanitize_scene_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "usgs_scene".to_string()
    } else {
        trimmed.to_string()
    }
}

fn browse_extension(url: &str) -> &'static str {
    let path = reqwest::Url::parse(url)
        .ok()
        .map(|url| url.path().to_lowercase())
        .unwrap_or_else(|| url.to_lowercase());
    if path.ends_with(".png") {
        "png"
    } else if path.ends_with(".tif") || path.ends_with(".tiff") {
        "tif"
    } else {
        "jpg"
    }
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

pub fn rank_scene_candidates(candidates: &mut [LandsatSceneCandidate]) {
    sort_candidates(candidates);
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
        let bbox = feature.bbox.map(|bbox| GeoBounds {
            min_lon: bbox[0],
            min_lat: bbox[1],
            max_lon: bbox[2],
            max_lat: bbox[3],
        });
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
            bbox,
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
    bbox: Option<[f64; 4]>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::schemas::GeoBounds;

    fn asset(href: &str) -> StacAsset {
        StacAsset {
            href: href.to_string(),
        }
    }

    fn candidate(
        item_id: &str,
        cloud_cover: Option<f64>,
        acquired_at: &str,
    ) -> LandsatSceneCandidate {
        LandsatSceneCandidate {
            dataset: "landsat".to_string(),
            dataset_label: "Landsat 8/9 Collection 2".to_string(),
            provider: "Microsoft Planetary Computer".to_string(),
            collection: "landsat-c2-l2".to_string(),
            item_id: item_id.to_string(),
            acquired_at: acquired_at.to_string(),
            cloud_cover,
            bbox: None,
            resolution_m: 30.0,
            asset_count: 0,
            assets: BTreeMap::new(),
        }
    }

    #[test]
    fn landsat_scene_candidate_extracts_bbox_and_metadata() {
        let feature = StacFeature {
            id: "LC09_TEST".to_string(),
            collection: Some("landsat-c2-l2".to_string()),
            bbox: Some([-97.0, 41.0, -96.0, 42.0]),
            properties: StacProperties {
                datetime: Some("2026-06-01T18:32:58Z".to_string()),
                created: None,
                cloud_cover: Some(3.5),
            },
            assets: BTreeMap::from([
                ("red".to_string(), asset("https://example.test/red.tif")),
                ("nir08".to_string(), asset("https://example.test/nir.tif")),
            ]),
        };

        let candidate =
            LandsatSceneCandidate::try_from_feature(SatelliteDataset::Landsat, feature).unwrap();

        assert_eq!(candidate.item_id, "LC09_TEST");
        assert_eq!(candidate.dataset, "landsat");
        assert_eq!(candidate.acquired_at, "2026-06-01T18:32:58Z");
        assert_eq!(candidate.cloud_cover, Some(3.5));
        assert_eq!(
            candidate.bbox,
            Some(GeoBounds {
                min_lon: -97.0,
                min_lat: 41.0,
                max_lon: -96.0,
                max_lat: 42.0,
            })
        );
        assert_eq!(candidate.asset_count, 2);
    }

    #[test]
    fn rank_scene_candidates_orders_by_cloud_then_date_then_dataset() {
        let mut candidates = vec![
            candidate("unknown-cloud", None, "2026-06-01T00:00:00Z"),
            candidate("clear-newer", Some(5.0), "2026-06-03T00:00:00Z"),
            candidate("clear-older", Some(5.0), "2026-06-01T00:00:00Z"),
            candidate("cloudy", Some(40.0), "2026-06-01T00:00:00Z"),
        ];

        rank_scene_candidates(&mut candidates);

        let ids = candidates
            .iter()
            .map(|candidate| candidate.item_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec!["clear-older", "clear-newer", "cloudy", "unknown-cloud"]
        );
    }

    #[test]
    fn usgs_auth_failure_is_distinct_from_not_found_and_redacts_credentials() {
        let credentials =
            UsgsCredentials::new("usgs-user@example.test", "super-secret-pass").unwrap();
        let body = json!({
            "errorCode": "AUTH_INVALID",
            "errorMessage": "invalid password super-secret-pass for usgs-user@example.test",
            "data": null
        })
        .to_string();

        let err = parse_usgs_login_response(reqwest::StatusCode::OK, &body, &credentials)
            .expect_err("auth failure should reject login");

        assert_eq!(err.code(), UsgsVerificationErrorCode::AuthFailed);
        assert_ne!(err.code(), UsgsVerificationErrorCode::NotFound);
        let rendered = err.to_string();
        assert!(rendered.contains("auth_failed"));
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("usgs-user@example.test"));
        assert!(!rendered.contains("super-secret-pass"));
    }

    #[test]
    fn usgs_scene_search_empty_results_maps_to_not_found() {
        let credentials = UsgsCredentials::new("valid-user", "valid-pass").unwrap();
        let body = json!({
            "errorCode": null,
            "errorMessage": null,
            "data": { "results": [] }
        })
        .to_string();

        let err = parse_usgs_scene_search_response(reqwest::StatusCode::OK, &body, &credentials)
            .expect_err("empty scene search should be not found");

        assert_eq!(err.code(), UsgsVerificationErrorCode::NotFound);
        assert_ne!(err.code(), UsgsVerificationErrorCode::AuthFailed);
    }

    #[test]
    fn usgs_scene_search_extracts_metadata_for_verification() {
        let credentials = UsgsCredentials::new("valid-user", "valid-pass").unwrap();
        let body = json!({
            "errorCode": null,
            "errorMessage": null,
            "data": {
                "results": [{
                    "entityId": "LC09_L2SP_029031_20240615_20240616_02_T1",
                    "displayId": "LC09 display",
                    "datasetName": "landsat_ot_c2_l2",
                    "acquisitionDate": "2024-06-15",
                    "cloudCover": 7.25,
                    "spatialBounds": {
                        "west": -96.70,
                        "south": 41.10,
                        "east": -96.40,
                        "north": 41.30
                    },
                    "browse": [{
                        "browsePath": "https://example.test/landsat.jpg"
                    }]
                }]
            }
        })
        .to_string();

        let scene =
            parse_usgs_scene_search_response(reqwest::StatusCode::OK, &body, &credentials).unwrap();

        assert_eq!(scene.scene_id, "LC09_L2SP_029031_20240615_20240616_02_T1");
        assert_eq!(scene.display_id.as_deref(), Some("LC09 display"));
        assert_eq!(scene.provider, "USGS");
        assert_eq!(scene.cloud_cover, Some(7.25));
        assert_eq!(
            scene.bbox,
            Some(GeoBounds {
                min_lon: -96.70,
                min_lat: 41.10,
                max_lon: -96.40,
                max_lat: 41.30,
            })
        );
        assert_eq!(
            scene.browse_url.as_deref(),
            Some("https://example.test/landsat.jpg")
        );
    }

    #[tokio::test]
    async fn usgs_verification_stores_browse_and_metadata() -> UsgsVerificationResult<()> {
        let tmp = tempfile::tempdir().map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::StoreFailed,
                err.to_string(),
                None,
            )
        })?;
        let scene = UsgsSceneSummary {
            scene_id: "LC09/TEST SCENE".to_string(),
            display_id: Some("LC09 display".to_string()),
            dataset_name: USGS_LANDSAT_DATASET.to_string(),
            provider: "USGS".to_string(),
            acquired_at: Some("2024-06-15".to_string()),
            cloud_cover: Some(2.0),
            bbox: None,
            browse_url: Some("https://example.test/browse.png".to_string()),
        };

        let record = store_usgs_verification(
            tmp.path(),
            scene,
            "https://example.test/browse.png",
            b"browse-bytes",
        )
        .await?;

        assert!(record.metadata_path.exists());
        assert!(record.downloaded_browse_path.exists());
        assert_eq!(
            std::fs::read(&record.downloaded_browse_path).unwrap(),
            b"browse-bytes"
        );
        let metadata = std::fs::read_to_string(&record.metadata_path).unwrap();
        assert!(metadata.contains("LC09/TEST SCENE"));
        assert!(!metadata.contains("super-secret-pass"));
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires AGBOT_RUN_USGS_INTEGRATION=1 and real AGBOT_USGS_USERNAME/AGBOT_USGS_PASSWORD"]
    async fn credentialed_usgs_landsat_ingest_verifies_search_download_and_store(
    ) -> UsgsVerificationResult<()> {
        if std::env::var(AGBOT_RUN_USGS_INTEGRATION_ENV).as_deref() != Ok("1") {
            eprintln!(
                "skipping live USGS verification; set {AGBOT_RUN_USGS_INTEGRATION_ENV}=1, \
                 {AGBOT_USGS_USERNAME_ENV}, and {AGBOT_USGS_PASSWORD_ENV}"
            );
            return Ok(());
        }
        let tmp = tempfile::tempdir().map_err(|err| {
            UsgsVerificationError::new(
                UsgsVerificationErrorCode::StoreFailed,
                err.to_string(),
                None,
            )
        })?;
        let request = UsgsIngestVerificationRequest::from_env(tmp.path())?;

        let record = verify_credentialed_usgs_landsat_ingest(request).await?;

        assert!(!record.scene.scene_id.trim().is_empty());
        assert!(record.metadata_path.exists());
        assert!(record.downloaded_browse_path.exists());
        assert!(record.downloaded_browse_path.metadata().unwrap().len() > 0);
        Ok(())
    }
}
