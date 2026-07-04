//! MapLibre GL JS browse UI (`/browse`) — static assets embedded into the
//! binary via `include_str!`, mirroring the `mobile_app.html` convention so
//! the page works regardless of the process working directory (and in route
//! tests). Source files live in `geo_hub/static/browse/`; see the README
//! there for what the UI expects from the running hub.

use axum::http::header;
use axum::response::{Html, IntoResponse};

const BROWSE_INDEX_HTML: &str = include_str!("../../static/browse/index.html");
const BROWSE_APP_JS: &str = include_str!("../../static/browse/app.js");
const BROWSE_STYLE_CSS: &str = include_str!("../../static/browse/style.css");

/// `GET /browse` — the layer-browser page.
pub async fn browse_app() -> Html<&'static str> {
    Html(BROWSE_INDEX_HTML)
}

/// `GET /browse/app.js` — the browse UI ES module.
pub async fn browse_app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        BROWSE_APP_JS,
    )
}

/// `GET /browse/style.css` — the browse UI stylesheet.
pub async fn browse_style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        BROWSE_STYLE_CSS,
    )
}
