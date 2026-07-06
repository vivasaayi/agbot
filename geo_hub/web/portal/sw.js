// AGBot Farm portal service worker (scope: /portal/).
//
// Strategy:
// - Precache the app shell plus the same-origin vendored Leaflet assets
//   (already served under /workspace/vendor/leaflet/; precached now so the
//   F-B7 field-detail map works offline).
// - Cache-first for precached/static /portal/* assets and Leaflet.
// - Network-first for /api/* requests; on network failure respond with
//   JSON {"offline": true} and status 503 so the client can show cached or
//   placeholder state.
// - There is intentionally NO background sync: mutations made while offline
//   are not queued or replayed. The user must retry once back online.

const CACHE_VERSION = "agbot-portal-v1";

const PRECACHE_URLS = [
  "/portal/",
  "/portal/index.html",
  "/portal/manifest.json",
  "/portal/css/portal.css",
  "/portal/js/api.js",
  "/portal/js/app.js",
  "/portal/js/auth.js",
  "/portal/js/views/login.js",
  "/portal/js/views/home.js",
  "/portal/js/views/reports.js",
  "/portal/js/views/field.js",
  "/portal/icons/icon-192.png",
  "/portal/icons/icon-512.png",
  "/workspace/vendor/leaflet/leaflet.js",
  "/workspace/vendor/leaflet/leaflet.css",
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE_VERSION)
      .then((cache) => cache.addAll(PRECACHE_URLS))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key !== CACHE_VERSION)
            .map((key) => caches.delete(key)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

function offlineApiResponse() {
  return new Response(JSON.stringify({ offline: true }), {
    status: 503,
    headers: { "Content-Type": "application/json" },
  });
}

function isStaticAsset(url) {
  return (
    url.origin === self.location.origin &&
    (url.pathname.startsWith("/portal/") ||
      url.pathname.startsWith("/workspace/vendor/leaflet/"))
  );
}

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);

  // API calls: network-first, JSON offline sentinel on failure. Only GETs
  // are safe to leave to the sentinel path too -- mutations simply fail
  // with the same 503 body and are NOT queued (no background sync).
  if (url.origin === self.location.origin && url.pathname.startsWith("/api/")) {
    event.respondWith(fetch(event.request).catch(() => offlineApiResponse()));
    return;
  }

  // Static portal shell + Leaflet: cache-first, falling back to the
  // network (and caching successful GET responses for next time).
  if (event.request.method === "GET" && isStaticAsset(url)) {
    event.respondWith(
      caches.match(event.request, { ignoreSearch: true }).then((cached) => {
        if (cached) {
          return cached;
        }
        return fetch(event.request).then((response) => {
          if (response.ok) {
            const copy = response.clone();
            caches
              .open(CACHE_VERSION)
              .then((cache) => cache.put(event.request, copy));
          }
          return response;
        });
      }),
    );
  }
});
