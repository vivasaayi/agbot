// AGBot Farm portal API client.
//
// This module is the ONLY place backend URL literals may live. The
// route-manifest test (geo_hub/tests/portal_static.rs) extracts every
// backend path string literal from this file and asserts it matches a
// route registered in geo_hub/src/server.rs.

import { getToken, clearSession } from "./auth.js";

// Same-origin base URL: the portal is served by geo_hub itself.
const BASE_URL = "";

/**
 * Every backend path used by the portal, as a string literal so the
 * route-manifest test can verify it against the router. Path templates use
 * `:param` placeholders; `buildPath` fills them.
 */
export const endpoints = {
  login: "/api/portal/login",
  logout: "/api/portal/logout",
  me: "/api/portal/me",
  farms: "/api/portal/farms",
  fields: "/api/portal/fields",
  fieldOverview: "/api/portal/fields/:field_id/overview",
  reports: "/api/portal/reports",
  reportRead: "/api/portal/reports/:report_id/read",
  reportDownload: "/api/portal/reports/:report_id/download",
  growerReport: "/api/portal/fields/:field_id/grower-report",
  recommendationStatus: "/api/portal/recommendations/:recommendation_id/status",
  alerts: "/api/portal/alerts",
  notificationsSummary: "/api/portal/notifications/summary",
  fieldActivities: "/api/portal/fields/:field_id/activities",
  fieldActivitySummary: "/api/portal/fields/:field_id/activities/summary",
  activity: "/api/portal/activities/:activity_id",
  // Open (non-portal) read routes, called with the same fetch wrapper: the
  // Bearer token is harmless there. Field record carries the boundary
  // polygon the overview response omits.
  fieldRecord: "/api/fields/:field_id",
  fieldTimeseries: "/api/fields/:field_id/timeseries",
  sceneProduct: "/api/scenes/:scene_id/products/:kind",
  sceneProductTile: "/api/scenes/:scene_id/products/:kind/tiles/:z/:x/:y.png",
};

/** Fill `:param` placeholders with URI-encoded values. */
export function buildPath(template, params = {}) {
  return template.replace(/:([A-Za-z_]+)/g, (match, name) => {
    if (!(name in params)) {
      throw new Error(`missing path parameter ${name} for ${template}`);
    }
    return encodeURIComponent(params[name]);
  });
}

/** Dispatched on `window` whenever a request hits the SW offline sentinel. */
export const OFFLINE_EVENT = "agbot:offline-response";

function notifyOffline() {
  window.dispatchEvent(new CustomEvent(OFFLINE_EVENT));
}

function navigateToLogin() {
  if (!window.location.hash.startsWith("#/login")) {
    window.location.hash = "#/login";
  }
}

/**
 * Fetch wrapper: attaches the Bearer token from localStorage, clears the
 * session and returns to #/login on 401, and surfaces the service worker's
 * offline sentinel ({offline:true}, 503) via OFFLINE_EVENT.
 */
async function request(method, path, { body, auth = true } = {}) {
  const headers = { Accept: "application/json" };
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  if (auth) {
    const token = getToken();
    if (token) {
      headers.Authorization = `Bearer ${token}`;
    }
  }

  const response = await fetch(BASE_URL + path, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (response.status === 401) {
    clearSession();
    navigateToLogin();
    const error = new Error("unauthorized");
    error.status = 401;
    throw error;
  }

  const contentType = response.headers.get("content-type") || "";
  const payload = contentType.includes("json") ? await response.json() : null;

  if (response.status === 503 && payload && payload.offline === true) {
    notifyOffline();
    const error = new Error("offline");
    error.offline = true;
    error.status = 503;
    throw error;
  }

  if (!response.ok) {
    const message =
      payload && payload.error ? payload.error : `request failed (${response.status})`;
    const error = new Error(message);
    error.status = response.status;
    throw error;
  }

  return payload;
}

// --- Session ---------------------------------------------------------------

export function login(accessCode) {
  return request("POST", endpoints.login, {
    body: { access_code: accessCode },
    auth: false,
  });
}

export function logout() {
  return request("POST", endpoints.logout, {});
}

export function me() {
  return request("GET", endpoints.me, {});
}

// --- Farms and fields --------------------------------------------------------

export function farms() {
  return request("GET", endpoints.farms, {});
}

export function fields() {
  return request("GET", endpoints.fields, {});
}

export function fieldOverview(fieldId) {
  return request("GET", buildPath(endpoints.fieldOverview, { field_id: fieldId }), {});
}

// --- Reports -----------------------------------------------------------------

export function reports({ unreadOnly = false } = {}) {
  const suffix = unreadOnly ? "?unread_only=true" : "";
  return request("GET", endpoints.reports + suffix, {});
}

export function markReportRead(reportId) {
  return request("POST", buildPath(endpoints.reportRead, { report_id: reportId }), {});
}

/** Download happens via a plain link so the browser handles the PDF. */
export function reportDownloadUrl(reportId) {
  return BASE_URL + buildPath(endpoints.reportDownload, { report_id: reportId });
}

export function generateGrowerReport(fieldId) {
  return request("POST", buildPath(endpoints.growerReport, { field_id: fieldId }), {});
}

// --- Recommendations, alerts, notifications ---------------------------------

export function updateRecommendationStatus(recommendationId, status, logActivity) {
  const body = { status };
  if (logActivity !== undefined) {
    body.log_activity = logActivity;
  }
  return request(
    "PUT",
    buildPath(endpoints.recommendationStatus, { recommendation_id: recommendationId }),
    { body },
  );
}

export function alerts() {
  return request("GET", endpoints.alerts, {});
}

export function notificationsSummary() {
  return request("GET", endpoints.notificationsSummary, {});
}

// --- Field activities (used by F-B7 field detail) ---------------------------

export function fieldActivities(fieldId, { from, to, activityType, page, pageSize } = {}) {
  const params = new URLSearchParams();
  if (from) params.set("from", from);
  if (to) params.set("to", to);
  if (activityType) params.set("activity_type", activityType);
  if (page) params.set("page", String(page));
  if (pageSize) params.set("page_size", String(pageSize));
  const query = params.toString();
  return request(
    "GET",
    buildPath(endpoints.fieldActivities, { field_id: fieldId }) +
      (query ? `?${query}` : ""),
    {},
  );
}

export function createFieldActivity(fieldId, draft) {
  return request("POST", buildPath(endpoints.fieldActivities, { field_id: fieldId }), {
    body: draft,
  });
}

export function fieldActivitySummary(fieldId) {
  return request(
    "GET",
    buildPath(endpoints.fieldActivitySummary, { field_id: fieldId }),
    {},
  );
}

export function updateActivity(activityId, patch) {
  return request("PUT", buildPath(endpoints.activity, { activity_id: activityId }), {
    body: patch,
  });
}

export function deleteActivity(activityId) {
  return request("DELETE", buildPath(endpoints.activity, { activity_id: activityId }), {});
}

// --- Open read APIs used by the field detail view (F-B7) --------------------

/** Full field record (includes the boundary polygon) from the open API. */
export function fieldRecord(fieldId) {
  return request("GET", buildPath(endpoints.fieldRecord, { field_id: fieldId }), {});
}

/**
 * Multi-source field time-series ({per_source, merged, harmonization}).
 * `metric` is required by the server (e.g. "sat.ndvi.mean").
 */
export function fieldTimeseries(fieldId, { metric, start, end, source } = {}) {
  const params = new URLSearchParams({ metric: metric || "sat.ndvi.mean" });
  if (start) params.set("start", start);
  if (end) params.set("end", end);
  if (source) params.set("source", source);
  return request(
    "GET",
    buildPath(endpoints.fieldTimeseries, { field_id: fieldId }) + `?${params}`,
    {},
  );
}

/**
 * Leaflet tile URL template for a scene product raster
 * (…/tiles/{z}/{x}/{y}.png). Kept here so every backend URL, including tile
 * templates, lives in api.js.
 */
export function sceneProductTileUrlTemplate(sceneId, kind) {
  return (
    BASE_URL +
    endpoints.sceneProductTile
      .replace(":scene_id", encodeURIComponent(sceneId))
      .replace(":kind", encodeURIComponent(kind))
      .replace(":z", "{z}")
      .replace(":x", "{x}")
      .replace(":y.png", "{y}.png")
  );
}

/**
 * True when the scene product exists and is servable (HEAD on the product
 * route). Used to disable the raster toggle instead of surfacing tile 404s.
 */
export async function sceneProductAvailable(sceneId, kind) {
  const path = buildPath(endpoints.sceneProduct, { scene_id: sceneId, kind });
  try {
    const response = await fetch(BASE_URL + path, { method: "HEAD" });
    return response.ok;
  } catch {
    return false;
  }
}
