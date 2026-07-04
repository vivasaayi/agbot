//! STAC API route handlers (`/api/stac/...`).
//!
//! Thin HTTP wrappers over `crate::stac_catalog`: landing page, conformance,
//! collections, per-collection items, and cross-collection item search
//! (GET + POST). All catalog reads, item mapping, and query parsing live in
//! the domain module; this file only decodes parameters and encodes the typed
//! STAC error body (`{code, description}`).

use crate::stac_catalog::{
    self, clamp_limit, SearchRequest, StacCatalog, StacCollection, StacError,
    StacFeatureCollection, StacItem, StacLink, STAC_API_ROOT,
};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

/// Typed STAC error response: HTTP status + `{code, description}` JSON body.
pub struct StacApiError(StacError);

impl From<StacError> for StacApiError {
    fn from(err: StacError) -> Self {
        Self(err)
    }
}

impl IntoResponse for StacApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            StacError::CollectionNotFound(_) | StacError::ItemNotFound { .. } => {
                StatusCode::NOT_FOUND
            }
            StacError::InvalidDatetime(_)
            | StacError::InvalidBbox(_)
            | StacError::InvalidLimit(_)
            | StacError::InvalidToken(_) => StatusCode::BAD_REQUEST,
            StacError::Catalog(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = serde_json::json!({
            "code": self.0.code(),
            "description": self.0.to_string(),
        });
        (status, Json(body)).into_response()
    }
}

type StacResult<T> = Result<T, StacApiError>;

/// `GET /api/stac` — landing page.
pub async fn stac_landing_page() -> Json<StacCatalog> {
    Json(stac_catalog::landing_page())
}

/// `GET /api/stac/conformance`.
pub async fn stac_conformance() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "conformsTo": stac_catalog::CONFORMANCE_CLASSES,
    }))
}

/// `GET /api/stac/collections` — list collections derived from the catalog.
pub async fn stac_list_collections(
    State(state): State<AppState>,
) -> StacResult<Json<serde_json::Value>> {
    let summaries = stac_catalog::collection_summaries(&state.pool).await?;
    let collections: Vec<StacCollection> = summaries
        .iter()
        .map(stac_catalog::summary_to_collection)
        .collect();
    Ok(Json(serde_json::json!({
        "collections": collections,
        "links": [
            StacLink::new("self", format!("{STAC_API_ROOT}/collections")),
            StacLink::new("root", STAC_API_ROOT.to_string()),
        ],
    })))
}

/// `GET /api/stac/collections/:collection_id`.
pub async fn stac_get_collection(
    Path(collection_id): Path<String>,
    State(state): State<AppState>,
) -> StacResult<Json<StacCollection>> {
    let summaries = stac_catalog::collection_summaries(&state.pool).await?;
    let summary = summaries
        .iter()
        .find(|s| s.id == collection_id)
        .ok_or(StacError::CollectionNotFound(collection_id))?;
    Ok(Json(stac_catalog::summary_to_collection(summary)))
}

/// Pagination query for `GET .../items`.
#[derive(Debug, Default, Deserialize)]
pub struct ItemsQuery {
    pub limit: Option<usize>,
    /// Offset pagination token, as issued in the `next` link.
    pub token: Option<String>,
}

fn parse_token(token: Option<&str>) -> Result<usize, StacError> {
    match token.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(0),
        Some(text) => text
            .parse::<usize>()
            .map_err(|_| StacError::InvalidToken(text.to_string())),
    }
}

fn feature_collection(
    page: stac_catalog::SearchPage,
    self_href: String,
    next_link: Option<StacLink>,
) -> StacFeatureCollection {
    let mut links = vec![
        StacLink::geojson("self", self_href),
        StacLink::new("root", STAC_API_ROOT.to_string()),
    ];
    links.extend(next_link);
    StacFeatureCollection {
        type_: "FeatureCollection".to_string(),
        number_returned: page.items.len(),
        skipped: page.skipped,
        features: page.items,
        links,
    }
}

/// `GET /api/stac/collections/:collection_id/items`.
pub async fn stac_list_collection_items(
    Path(collection_id): Path<String>,
    Query(query): Query<ItemsQuery>,
    State(state): State<AppState>,
) -> StacResult<Json<StacFeatureCollection>> {
    let limit = clamp_limit(query.limit);
    let offset = parse_token(query.token.as_deref())?;
    let request = SearchRequest {
        collections: Some(vec![collection_id.clone()]),
        limit,
        offset,
        ..Default::default()
    };
    let page = stac_catalog::search(&state.pool, &request).await?;
    let base = format!("{STAC_API_ROOT}/collections/{collection_id}/items");
    let next_link = page
        .next_offset
        .map(|next| StacLink::geojson("next", format!("{base}?limit={limit}&token={next}")));
    Ok(Json(feature_collection(page, base, next_link)))
}

/// `GET /api/stac/collections/:collection_id/items/:item_id`.
pub async fn stac_get_collection_item(
    Path((collection_id, item_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> StacResult<Json<StacItem>> {
    let item = stac_catalog::get_item(&state.pool, &collection_id, &item_id).await?;
    Ok(Json(item))
}

/// `GET /api/stac/search` query parameters (comma-separated list forms).
#[derive(Debug, Default, Deserialize)]
pub struct SearchGetQuery {
    pub collections: Option<String>,
    pub ids: Option<String>,
    pub bbox: Option<String>,
    pub datetime: Option<String>,
    pub limit: Option<usize>,
    pub token: Option<String>,
}

/// `POST /api/stac/search` JSON body (array forms).
#[derive(Debug, Default, Deserialize)]
pub struct SearchPostBody {
    pub collections: Option<Vec<String>>,
    pub ids: Option<Vec<String>>,
    pub bbox: Option<Vec<f64>>,
    pub datetime: Option<String>,
    pub limit: Option<usize>,
    pub token: Option<String>,
}

fn split_csv(text: Option<String>) -> Option<Vec<String>> {
    let values: Vec<String> = text?
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    (!values.is_empty()).then_some(values)
}

fn normalize_list(values: Option<Vec<String>>) -> Option<Vec<String>> {
    let values: Vec<String> = values?
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    (!values.is_empty()).then_some(values)
}

fn build_request(
    collections: Option<Vec<String>>,
    ids: Option<Vec<String>>,
    bbox: Option<Vec<f64>>,
    datetime: Option<String>,
    limit: Option<usize>,
    token: Option<String>,
) -> Result<SearchRequest, StacError> {
    let bbox = match bbox {
        Some(values) => Some(stac_catalog::parse_bbox_values(&values)?),
        None => None,
    };
    let datetime = match datetime.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(text) => Some(stac_catalog::parse_datetime_param(text)?),
        None => None,
    };
    Ok(SearchRequest {
        collections: normalize_list(collections),
        ids: normalize_list(ids),
        bbox,
        datetime,
        limit: clamp_limit(limit),
        offset: parse_token(token.as_deref())?,
    })
}

async fn run_search(
    state: &AppState,
    request: SearchRequest,
    next_link: impl Fn(usize) -> StacLink,
) -> StacResult<Json<StacFeatureCollection>> {
    let page = stac_catalog::search(&state.pool, &request).await?;
    let next = page.next_offset.map(next_link);
    Ok(Json(feature_collection(
        page,
        format!("{STAC_API_ROOT}/search"),
        next,
    )))
}

/// `GET /api/stac/search`.
pub async fn stac_search_get(
    Query(query): Query<SearchGetQuery>,
    State(state): State<AppState>,
) -> StacResult<Json<StacFeatureCollection>> {
    let bbox = match query
        .bbox
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(text) => Some(
            text.split(',')
                .map(|p| p.trim().parse::<f64>())
                .collect::<Result<Vec<f64>, _>>()
                .map_err(|_| {
                    StacError::InvalidBbox("bbox must be comma-separated numbers".to_string())
                })?,
        ),
        None => None,
    };
    let request = build_request(
        split_csv(query.collections),
        split_csv(query.ids),
        bbox,
        query.datetime.clone(),
        query.limit,
        query.token,
    )?;
    let limit = request.limit;
    // Rebuild the query string for the next link from the normalized request.
    let mut params: Vec<String> = Vec::new();
    if let Some(collections) = &request.collections {
        params.push(format!("collections={}", collections.join(",")));
    }
    if let Some(ids) = &request.ids {
        params.push(format!("ids={}", ids.join(",")));
    }
    if let Some(bbox) = &request.bbox {
        params.push(format!(
            "bbox={},{},{},{}",
            bbox[0], bbox[1], bbox[2], bbox[3]
        ));
    }
    if let Some(datetime) = query
        .datetime
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        params.push(format!("datetime={datetime}"));
    }
    params.push(format!("limit={limit}"));
    run_search(&state, request, move |next| {
        StacLink::geojson(
            "next",
            format!("{STAC_API_ROOT}/search?{}&token={next}", params.join("&")),
        )
    })
    .await
}

/// `POST /api/stac/search`.
pub async fn stac_search_post(
    State(state): State<AppState>,
    Json(body): Json<SearchPostBody>,
) -> StacResult<Json<StacFeatureCollection>> {
    let request = build_request(
        body.collections,
        body.ids,
        body.bbox,
        body.datetime,
        body.limit,
        body.token,
    )?;
    run_search(&state, request, |next| {
        let mut link = StacLink::geojson("next", format!("{STAC_API_ROOT}/search"));
        link.method = Some("POST".to_string());
        link.body = Some(serde_json::json!({ "token": next.to_string() }));
        link
    })
    .await
}
