// AGBot Workspace API client.
//
// This module is the ONLY place backend URL literals may live. The
// route-manifest test (geo_hub/tests/workspace_static.rs) extracts every
// api path string literal from this file and asserts it matches a route
// registered in geo_hub/src/server.rs.

// Same-origin base URL: the workspace is served by geo_hub itself.
const BASE_URL = "";

/** Endpoints used by the workspace (phase A1). */
export const endpoints = {
  farms: "/api/farms",
  fields: "/api/fields",
  scenes: "/api/scenes",
};

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
