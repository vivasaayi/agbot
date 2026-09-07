//! Planetary Computer SAS signing for Landsat COG reads (satellite batch
//! S-10, Task A).
//!
//! Planetary Computer STAC assets live on Azure Blob Storage
//! (`*.blob.core.windows.net`) and require a short-lived SAS token appended
//! to the URL query string. Tokens are issued anonymously per collection by
//! `GET https://planetarycomputer.microsoft.com/api/sas/v1/token/{collection}`
//! -> `{ "token": "st=...&se=...&sig=...", "msft:expiry": "<RFC 3339>" }`.
//!
//! Three pieces compose the read path:
//! - [`PcSasTokenCache`]: fetches and caches one token per collection,
//!   refreshing when a token is missing or within 60 s of expiry.
//! - [`sign_href`]: pure query-string appending, applied only to blob hosts.
//! - [`PcSignedCogResolver`]: a [`CogStoreResolver`] that routes blob hrefs
//!   through [`SasHttpStore`] — a minimal read-only `ObjectStore` doing
//!   SAS-signed HTTP range GETs — and delegates every other href to the
//!   plain [`UrlCogResolver`].
//!
//! [`SasHttpStore`] exists because the workspace `object_store` build has
//! only the `http` feature: `parse_url` classifies `*.blob.core.windows.net`
//! as the (uncompiled) Azure scheme, and the generic HTTP store drops URL
//! query strings — either way the SAS token would be lost. Signing inside
//! the store's async `get_opts` also lets the token be fetched and refreshed
//! lazily per range read instead of requiring a pre-warm step behind the
//! synchronous resolver seam.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration, Utc};
use futures_util::stream::{self, BoxStream, StreamExt};
use raster_io::object_store::path::Path as ObjectPath;
use raster_io::object_store::{
    CopyOptions, Error as ObjectStoreError, GetOptions, GetRange, GetResult, GetResultPayload,
    ListResult, MultipartUpload, ObjectMeta, ObjectStore, PutMultipartOptions, PutOptions,
    PutPayload, PutResult, Result as ObjectStoreResult,
};
use serde::Deserialize;
use thiserror::Error;

use crate::satellite_derivation::{CogStoreResolver, DerivationError, UrlCogResolver};

/// Planetary Computer anonymous SAS token endpoint base; the collection id
/// is appended as the final path segment.
pub const PC_SAS_TOKEN_API: &str = "https://planetarycomputer.microsoft.com/api/sas/v1/token";

/// Tokens this close to `msft:expiry` are refreshed instead of reused, so a
/// token never expires in the middle of a multi-band derivation.
const REFRESH_MARGIN_SECONDS: i64 = 60;

const USER_AGENT: &str = "agbot-geo-hub/0.1";

#[derive(Debug, Error)]
pub enum PcSignError {
    #[error("failed to build SAS HTTP client: {0}")]
    Client(#[source] reqwest::Error),
    #[error("SAS token request for collection {collection} failed: {source}")]
    Request {
        collection: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("SAS token endpoint returned {status} for collection {collection}: {body}")]
    Status {
        collection: String,
        status: u16,
        body: String,
    },
    #[error("SAS token response for collection {collection} is invalid: {message}")]
    InvalidResponse { collection: String, message: String },
}

/// Wire shape of the PC token endpoint response.
#[derive(Debug, Deserialize)]
struct SasTokenResponse {
    token: String,
    #[serde(rename = "msft:expiry")]
    expiry: String,
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expiry: DateTime<Utc>,
}

impl CachedToken {
    fn is_fresh(&self) -> bool {
        self.expiry - Utc::now() > Duration::seconds(REFRESH_MARGIN_SECONDS)
    }
}

/// Per-collection cache of Planetary Computer SAS tokens.
pub struct PcSasTokenCache {
    http: reqwest::Client,
    base_url: String,
    tokens: Mutex<HashMap<String, CachedToken>>,
}

impl fmt::Debug for PcSasTokenCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PcSasTokenCache")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl PcSasTokenCache {
    /// Cache against the real Planetary Computer token endpoint.
    pub fn new() -> Result<Self, PcSignError> {
        Self::with_base_url(PC_SAS_TOKEN_API)
    }

    /// Cache against a custom token endpoint base (tests point this at a
    /// local server). The collection id is appended as `{base}/{collection}`.
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, PcSignError> {
        let http = reqwest::Client::builder()
            .timeout(StdDuration::from_secs(30))
            .build()
            .map_err(PcSignError::Client)?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            tokens: Mutex::new(HashMap::new()),
        })
    }

    /// The cached token for `collection` when present and not within the
    /// refresh margin of expiry. Never fetches.
    pub fn cached_token(&self, collection: &str) -> Option<String> {
        let tokens = self.tokens.lock().expect("sas token cache lock");
        tokens
            .get(collection)
            .filter(|cached| cached.is_fresh())
            .map(|cached| cached.token.clone())
    }

    /// The SAS token for `collection`: served from cache when fresh,
    /// otherwise fetched from the token endpoint and cached.
    pub async fn token_for(&self, collection: &str) -> Result<String, PcSignError> {
        if let Some(token) = self.cached_token(collection) {
            return Ok(token);
        }
        let url = format!("{}/{collection}", self.base_url);
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .map_err(|source| PcSignError::Request {
                collection: collection.to_string(),
                source,
            })?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(PcSignError::Status {
                collection: collection.to_string(),
                status: status.as_u16(),
                body: body.chars().take(300).collect(),
            });
        }
        let invalid = |message: String| PcSignError::InvalidResponse {
            collection: collection.to_string(),
            message,
        };
        let parsed: SasTokenResponse =
            serde_json::from_str(&body).map_err(|err| invalid(err.to_string()))?;
        if parsed.token.is_empty() {
            return Err(invalid("empty token".to_string()));
        }
        let expiry = DateTime::parse_from_rfc3339(&parsed.expiry)
            .map_err(|err| invalid(format!("bad msft:expiry {:?}: {err}", parsed.expiry)))?
            .with_timezone(&Utc);
        self.tokens.lock().expect("sas token cache lock").insert(
            collection.to_string(),
            CachedToken {
                token: parsed.token.clone(),
                expiry,
            },
        );
        Ok(parsed.token)
    }
}

/// True when `href` points at Azure Blob Storage — the only hosts a
/// Planetary Computer SAS token applies to.
pub fn requires_sas(href: &str) -> bool {
    url::Url::parse(href)
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| host.ends_with(".blob.core.windows.net"))
        })
        .unwrap_or(false)
}

/// Append a SAS token (already a query string, `st=...&se=...&sig=...`) to
/// an href's query.
fn append_token(href: &str, token: &str) -> String {
    let separator = if href.contains('?') { '&' } else { '?' };
    format!("{href}{separator}{token}")
}

/// Sign an asset href with a SAS token: appends `?{token}` (or `&{token}`
/// when the href already has a query). Non-blob hosts and empty tokens pass
/// through unchanged.
pub fn sign_href(href: &str, token: &str) -> String {
    if token.is_empty() || !requires_sas(href) {
        return href.to_string();
    }
    append_token(href, token)
}

// --- SAS-signed HTTP object store ---------------------------------------------

fn generic_error(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> ObjectStoreError {
    ObjectStoreError::Generic {
        store: "SasHttpStore",
        source: source.into(),
    }
}

fn not_implemented(operation: &str) -> ObjectStoreError {
    ObjectStoreError::NotImplemented {
        operation: operation.to_string(),
        implementer: "SasHttpStore (read-only SAS-signed COG access)".to_string(),
    }
}

/// `Content-Range: bytes {start}-{end}/{total}` -> `(start, end, total)`.
fn parse_content_range(header: &str) -> Option<(u64, u64, u64)> {
    let spec = header.strip_prefix("bytes ")?;
    let (range, total) = spec.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((
        start.trim().parse().ok()?,
        end.trim().parse().ok()?,
        total.trim().parse().ok()?,
    ))
}

fn range_header_value(range: &GetRange) -> String {
    match range {
        GetRange::Bounded(bounded) => format!("bytes={}-{}", bounded.start, bounded.end - 1),
        GetRange::Offset(offset) => format!("bytes={offset}-"),
        GetRange::Suffix(length) => format!("bytes=-{length}"),
    }
}

/// Minimal read-only [`ObjectStore`] doing SAS-signed HTTP range GETs
/// against one host. The token is looked up (and lazily fetched/refreshed)
/// from a shared [`PcSasTokenCache`] on every request, so long derivations
/// survive token expiry. Writes, listing, and copies are not implemented.
pub struct SasHttpStore {
    http: reqwest::Client,
    /// `scheme://host[:port]`, no trailing slash.
    base_url: String,
    cache: Arc<PcSasTokenCache>,
    collection: String,
}

impl SasHttpStore {
    pub fn new(
        base_url: impl Into<String>,
        cache: Arc<PcSasTokenCache>,
        collection: impl Into<String>,
    ) -> Result<Self, PcSignError> {
        let http = reqwest::Client::builder()
            .timeout(StdDuration::from_secs(60))
            .build()
            .map_err(PcSignError::Client)?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            cache,
            collection: collection.into(),
        })
    }

    async fn signed_url(&self, location: &ObjectPath) -> ObjectStoreResult<String> {
        let token = self
            .cache
            .token_for(&self.collection)
            .await
            .map_err(generic_error)?;
        Ok(append_token(
            &format!("{}/{location}", self.base_url),
            &token,
        ))
    }
}

impl fmt::Debug for SasHttpStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SasHttpStore")
            .field("base_url", &self.base_url)
            .field("collection", &self.collection)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for SasHttpStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SasHttpStore({}, collection {})",
            self.base_url, self.collection
        )
    }
}

#[async_trait::async_trait]
impl ObjectStore for SasHttpStore {
    async fn get_opts(
        &self,
        location: &ObjectPath,
        options: GetOptions,
    ) -> ObjectStoreResult<GetResult> {
        let url = self.signed_url(location).await?;
        let mut request = if options.head {
            self.http.head(&url)
        } else {
            self.http.get(&url)
        };
        request = request.header(reqwest::header::USER_AGENT, USER_AGENT);
        if let Some(range) = &options.range {
            request = request.header(reqwest::header::RANGE, range_header_value(range));
        }
        let response = request.send().await.map_err(generic_error)?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ObjectStoreError::NotFound {
                path: location.to_string(),
                source: format!("HTTP 404 from {}", self.base_url).into(),
            });
        }
        if !status.is_success() {
            return Err(generic_error(format!(
                "SAS-signed GET {location} returned HTTP {status}"
            )));
        }
        let content_range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let content_length = response.content_length();
        // HEAD responses carry no body; `bytes()` yields empty for them.
        let bytes = response.bytes().await.map_err(generic_error)?;
        let (range, size) = match content_range.as_deref().and_then(parse_content_range) {
            Some((start, end, total)) => (start..end + 1, total),
            None => (
                0..bytes.len() as u64,
                content_length.unwrap_or(bytes.len() as u64),
            ),
        };
        let meta = ObjectMeta {
            location: location.clone(),
            last_modified: Utc::now(),
            size,
            e_tag: None,
            version: None,
        };
        Ok(GetResult {
            payload: GetResultPayload::Stream(
                stream::once(async move { Ok::<_, ObjectStoreError>(bytes) }).boxed(),
            ),
            meta,
            range,
            attributes: Default::default(),
        })
    }

    async fn put_opts(
        &self,
        _location: &ObjectPath,
        _payload: PutPayload,
        _opts: PutOptions,
    ) -> ObjectStoreResult<PutResult> {
        Err(not_implemented("put_opts"))
    }

    async fn put_multipart_opts(
        &self,
        _location: &ObjectPath,
        _opts: PutMultipartOptions,
    ) -> ObjectStoreResult<Box<dyn MultipartUpload>> {
        Err(not_implemented("put_multipart_opts"))
    }

    fn delete_stream(
        &self,
        _locations: BoxStream<'static, ObjectStoreResult<ObjectPath>>,
    ) -> BoxStream<'static, ObjectStoreResult<ObjectPath>> {
        stream::once(async { Err(not_implemented("delete_stream")) }).boxed()
    }

    fn list(
        &self,
        _prefix: Option<&ObjectPath>,
    ) -> BoxStream<'static, ObjectStoreResult<ObjectMeta>> {
        stream::once(async { Err(not_implemented("list")) }).boxed()
    }

    async fn list_with_delimiter(
        &self,
        _prefix: Option<&ObjectPath>,
    ) -> ObjectStoreResult<ListResult> {
        Err(not_implemented("list_with_delimiter"))
    }

    async fn copy_opts(
        &self,
        _from: &ObjectPath,
        _to: &ObjectPath,
        _options: CopyOptions,
    ) -> ObjectStoreResult<()> {
        Err(not_implemented("copy_opts"))
    }
}

// --- Resolver -----------------------------------------------------------------

/// [`CogStoreResolver`] for Planetary Computer collections: blob hrefs read
/// through [`SasHttpStore`] (token fetched/refreshed lazily per range read);
/// every other href delegates to the plain [`UrlCogResolver`].
pub struct PcSignedCogResolver {
    inner: UrlCogResolver,
    cache: Arc<PcSasTokenCache>,
    collection: String,
}

impl PcSignedCogResolver {
    pub fn new(cache: Arc<PcSasTokenCache>, collection: impl Into<String>) -> Self {
        Self {
            inner: UrlCogResolver,
            cache,
            collection: collection.into(),
        }
    }
}

impl CogStoreResolver for PcSignedCogResolver {
    fn resolve(&self, href: &str) -> Result<(Arc<dyn ObjectStore>, String), DerivationError> {
        if !requires_sas(href) {
            return self.inner.resolve(href);
        }
        let resolve_error = |message: String| DerivationError::Resolve {
            href: href.to_string(),
            message,
        };
        let url = url::Url::parse(href).map_err(|err| resolve_error(err.to_string()))?;
        let base_url = url[..url::Position::BeforePath].to_string();
        let location = ObjectPath::from_url_path(url.path().trim_start_matches('/'))
            .map_err(|err| resolve_error(err.to_string()))?;
        let store = SasHttpStore::new(base_url, self.cache.clone(), self.collection.clone())
            .map_err(|err| resolve_error(err.to_string()))?;
        Ok((Arc::new(store), location.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_sas_matches_blob_hosts_only() {
        assert!(requires_sas(
            "https://landsateuwest.blob.core.windows.net/landsat-c2/x.TIF"
        ));
        assert!(!requires_sas(
            "https://sentinel-cogs.s3.us-west-2.amazonaws.com/x/B04.tif"
        ));
        assert!(!requires_sas("s3://usgs-landsat/x.TIF"));
        assert!(!requires_sas("not a url"));
    }

    #[test]
    fn content_range_parsing_is_exact() {
        assert_eq!(
            parse_content_range("bytes 0-2047/2048"),
            Some((0, 2047, 2048))
        );
        assert_eq!(parse_content_range("bytes 5-9/100"), Some((5, 9, 100)));
        assert_eq!(parse_content_range("2048"), None);
        assert_eq!(parse_content_range("bytes */2048"), None);
    }

    #[test]
    fn range_headers_cover_all_get_range_shapes() {
        assert_eq!(
            range_header_value(&GetRange::Bounded(10..20)),
            "bytes=10-19"
        );
        assert_eq!(range_header_value(&GetRange::Offset(5)), "bytes=5-");
        assert_eq!(range_header_value(&GetRange::Suffix(16)), "bytes=-16");
    }
}
