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
  productTiles: "/api/scenes/:scene_id/products/:kind/tiles/:z/:x/:y.png",
  sceneAnnotations: "/api/scenes/:scene_id/annotations",
  sceneAnnotation: "/api/scenes/:scene_id/annotations/:annotation_id",
};

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
