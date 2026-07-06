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

export function fieldActivities(fieldId) {
  return request("GET", buildPath(endpoints.fieldActivities, { field_id: fieldId }), {});
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
