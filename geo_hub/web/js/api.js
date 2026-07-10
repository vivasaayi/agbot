// AGBot Workspace API client.
//
// This module is the ONLY place backend URL literals may live. The
// route-manifest test (geo_hub/tests/workspace_static.rs) extracts every
// api path string literal from this file and asserts it matches a route
// registered in geo_hub/src/server.rs.

// Same-origin base URL: the workspace is served by geo_hub itself.
const BASE_URL = "";

/**
 * Endpoints used by the workspace. Every backend path lives here as a string
 * literal so the route-manifest test can verify it against the router. Path
 * templates use `:param` placeholders; the builder functions below fill them.
 */
export const endpoints = {
  farms: "/api/farms",
  fields: "/api/fields",
  scenes: "/api/scenes",
  farmFields: "/api/farms/:farm_id/fields",
  fieldScenes: "/api/fields/:field_id/scenes",
  scene: "/api/scenes/:scene_id",
  catalogProducts: "/api/catalog/products",
  fieldFindings: "/api/fields/:field_id/findings",
  fieldAlerts: "/api/fields/:field_id/alerts",
  fieldAlertEvaluation: "/api/fields/:field_id/alert-evaluation",
  alertLifecycle: "/api/alerts/:alert_id/lifecycle",
  alertAcknowledge: "/api/alerts/:alert_id/acknowledge",
  alertResolve: "/api/alerts/:alert_id/resolve",
  cropHealthRuns: "/api/applications/crop-health/runs",
  waterPriorityRuns: "/api/applications/water-priority/runs",
  anomalyRuns: "/api/applications/anomaly/runs",
  productTiles: "/api/scenes/:scene_id/products/:kind/tiles/:z/:x/:y.png",
  sceneAnnotations: "/api/scenes/:scene_id/annotations",
  sceneAnnotation: "/api/scenes/:scene_id/annotations/:annotation_id",
  sceneRecommendations: "/api/scenes/:scene_id/recommendations",
  sceneReports: "/api/scenes/:scene_id/reports",
  sceneReport: "/api/scenes/:scene_id/reports/:report_id",
  sceneReportLineage: "/api/scenes/:scene_id/reports/:report_id/lineage",
  provenanceTrace: "/api/provenance/trace/:artifact_id",
  fieldProposals: "/api/fields/:field_id/proposals",
  proposalAccept: "/api/proposals/:proposal_id/accept",
  proposalReject: "/api/proposals/:proposal_id/reject",
  catalogSources: "/api/catalog/sources",
  ingestHealth: "/api/ingest/health",
  pipelineJobs: "/api/pipeline/jobs",
  fieldSubscriptions: "/api/fields/:field_id/subscriptions",
  fieldBackfills: "/api/fields/:field_id/backfills",
  fieldTimeseries: "/api/fields/:field_id/timeseries",
};

/** A field's multi-source metric time-series (requires `metric`). */
export function fieldTimeseriesPath(fieldId, query) {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null && value !== "") {
      params.set(key, value);
    }
  }
  const suffix = params.toString();
  const base = `/api/fields/${encodeURIComponent(fieldId)}/timeseries`;
  return suffix ? `${base}?${suffix}` : base;
}

/** Registered ingestion sources, optionally filtered (status, source_kind). */
export function catalogSourcesPath(query) {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null && value !== "") {
      params.set(key, value);
    }
  }
  const suffix = params.toString();
  return suffix ? `/api/catalog/sources?${suffix}` : "/api/catalog/sources";
}

/** Scene-ingestion health summary (in-flight / succeeded / failed). */
export function ingestHealthPath() {
  return "/api/ingest/health";
}

/** Recent pipeline jobs, optionally filtered (field_id, status, limit). */
export function pipelineJobsPath(query) {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null && value !== "") {
      params.set(key, value);
    }
  }
  const suffix = params.toString();
  return suffix ? `/api/pipeline/jobs?${suffix}` : "/api/pipeline/jobs";
}

/** A field's dataset subscriptions. */
export function fieldSubscriptionsPath(fieldId) {
  return `/api/fields/${encodeURIComponent(fieldId)}/subscriptions`;
}

/** A field's backfill runs. */
export function fieldBackfillsPath(fieldId) {
  return `/api/fields/${encodeURIComponent(fieldId)}/backfills`;
}

/** A field's proposal queue. */
export function fieldProposalsPath(fieldId) {
  return `/api/fields/${encodeURIComponent(fieldId)}/proposals`;
}

/** Accept a queued proposal. */
export function proposalAcceptPath(proposalId) {
  return `/api/proposals/${encodeURIComponent(proposalId)}/accept`;
}

/** Reject a queued proposal. */
export function proposalRejectPath(proposalId) {
  return `/api/proposals/${encodeURIComponent(proposalId)}/reject`;
}

/** A scene's recommendations. */
export function sceneRecommendationsPath(sceneId) {
  return `/api/scenes/${encodeURIComponent(sceneId)}/recommendations`;
}

/** A scene's reports. */
export function sceneReportsPath(sceneId) {
  return `/api/scenes/${encodeURIComponent(sceneId)}/reports`;
}

/** A single report. */
export function sceneReportPath(sceneId, reportId) {
  return `/api/scenes/${encodeURIComponent(sceneId)}/reports/${encodeURIComponent(reportId)}`;
}

/** A report's provenance lineage. */
export function sceneReportLineagePath(sceneId, reportId) {
  return `/api/scenes/${encodeURIComponent(sceneId)}/reports/${encodeURIComponent(
    reportId,
  )}/lineage`;
}

/** Backward provenance trace for any artifact id. */
export function provenanceTracePath(artifactId) {
  return `/api/provenance/trace/${encodeURIComponent(artifactId)}`;
}

/** A scene's annotations collection (GET list / POST create). */
export function sceneAnnotationsPath(sceneId) {
  return `/api/scenes/${encodeURIComponent(sceneId)}/annotations`;
}

/** A single annotation (PUT update / DELETE). */
export function sceneAnnotationPath(sceneId, annotationId) {
  return `/api/scenes/${encodeURIComponent(sceneId)}/annotations/${encodeURIComponent(
    annotationId,
  )}`;
}

/**
 * Leaflet XYZ tile-url template for a scene product layer. Leaflet fills the
 * `{z}/{x}/{y}` placeholders per tile request.
 */
export function productTilesUrlTemplate(sceneId, kind) {
  return `/api/scenes/${encodeURIComponent(sceneId)}/products/${encodeURIComponent(
    kind,
  )}/tiles/{z}/{x}/{y}.png`;
}

/** A field's application findings (most recent first). */
export function fieldFindingsPath(fieldId) {
  return `/api/fields/${encodeURIComponent(fieldId)}/findings`;
}

/** A field's fired alerts (most recent first). */
export function fieldAlertsPath(fieldId) {
  return `/api/fields/${encodeURIComponent(fieldId)}/alerts`;
}

/** Alert-evaluation trigger for a field (POST optional rule set). */
export function fieldAlertEvaluationPath(fieldId) {
  return `/api/fields/${encodeURIComponent(fieldId)}/alert-evaluation`;
}

/** An alert's lifecycle record. */
export function alertLifecyclePath(alertId) {
  return `/api/alerts/${encodeURIComponent(alertId)}/lifecycle`;
}

/** Acknowledge-transition endpoint for an alert (POST { actor_id }). */
export function alertAcknowledgePath(alertId) {
  return `/api/alerts/${encodeURIComponent(alertId)}/acknowledge`;
}

/** Resolve-transition endpoint for an alert (POST { actor_id }). */
export function alertResolvePath(alertId) {
  return `/api/alerts/${encodeURIComponent(alertId)}/resolve`;
}

/** Crop-health application run trigger (POST zone stats). */
export function cropHealthRunsPath() {
  return "/api/applications/crop-health/runs";
}

/** Water-priority application run trigger (POST zone stats). */
export function waterPriorityRunsPath() {
  return "/api/applications/water-priority/runs";
}

/** Anomaly-detection application run trigger (POST zone index values). */
export function anomalyRunsPath() {
  return "/api/applications/anomaly/runs";
}

export function droughtWatchRunsPath() {
  return "/api/applications/drought-watch/runs";
}

/** Fields belonging to a farm. */
export function farmFieldsPath(farmId) {
  return `/api/farms/${encodeURIComponent(farmId)}/fields`;
}

/** Scenes linked to a field. */
export function fieldScenesPath(fieldId) {
  return `/api/fields/${encodeURIComponent(fieldId)}/scenes`;
}

/** A single scene's detail. */
export function scenePath(sceneId) {
  return `/api/scenes/${encodeURIComponent(sceneId)}`;
}

/** Catalog products, optionally filtered by a query object. */
export function catalogProductsPath(query) {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null && value !== "") {
      params.set(key, value);
    }
  }
  const suffix = params.toString();
  return suffix ? `/api/catalog/products?${suffix}` : "/api/catalog/products";
}

async function request(method, path, body) {
  const options = { method, headers: { Accept: "application/json" } };
  if (body !== undefined) {
    options.headers["Content-Type"] = "application/json";
    options.body = JSON.stringify(body);
  }
  const response = await fetch(`${BASE_URL}${path}`, options);
  if (!response.ok) {
    throw new Error(`${method} ${path} failed: ${response.status} ${response.statusText}`);
  }
  if (response.status === 204) {
    return null;
  }
  const contentType = response.headers.get("content-type") ?? "";
  return contentType.includes("application/json") ? response.json() : response.text();
}

export function apiGet(path) {
  return request("GET", path);
}

export function apiPost(path, body) {
  return request("POST", path, body);
}

export function apiPut(path, body) {
  return request("PUT", path, body);
}

export function apiDelete(path) {
  return request("DELETE", path);
}
