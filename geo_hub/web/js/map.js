// Leaflet map + product tile layers (Track B phase A3). Uses vendored Leaflet
// (window.L, loaded by index.html). Product tile URLs come from api.js only.

import { productTilesUrlTemplate } from "./api.js";

let map = null;
const productLayers = new Map(); // "sceneId::kind" -> L.TileLayer

/** Initialize the Leaflet map on `element`. Idempotent. */
export function initMap(element) {
  if (map) return map;
  const L = window.L;
  map = L.map(element, { center: [40.0, -100.0], zoom: 4, worldCopyJump: true });
  // A neutral offline-friendly backdrop; product tiles overlay on top. No
  // external basemap is loaded (field deployments are offline).
  L.rectangle(
    [
      [-85, -180],
      [85, 180],
    ],
    { color: "#2c313a", weight: 0, fillColor: "#12141a", fillOpacity: 1 },
  ).addTo(map);
  return map;
}

function layerKey(sceneId, kind) {
  return `${sceneId}::${kind}`;
}

/** Add (or reveal) a scene product tile layer at the given opacity. */
export function addProductLayer(sceneId, kind, opacity = 1.0) {
  if (!map) return;
  const key = layerKey(sceneId, kind);
  if (productLayers.has(key)) {
    productLayers.get(key).setOpacity(opacity);
    return;
  }
  const layer = window.L.tileLayer(productTilesUrlTemplate(sceneId, kind), {
    opacity,
    tileSize: 256,
    minZoom: 0,
    maxZoom: 22,
    noWrap: true,
  });
  layer.addTo(map);
  productLayers.set(key, layer);
}

/** Remove a scene product tile layer. */
export function removeProductLayer(sceneId, kind) {
  const key = layerKey(sceneId, kind);
  const layer = productLayers.get(key);
  if (layer && map) {
    map.removeLayer(layer);
    productLayers.delete(key);
  }
}

/** Set the opacity of an active product layer. */
export function setProductLayerOpacity(sceneId, kind, opacity) {
  const layer = productLayers.get(layerKey(sceneId, kind));
  if (layer) {
    layer.setOpacity(opacity);
  }
}

/** True when the given scene/kind layer is currently shown. */
export function hasProductLayer(sceneId, kind) {
  return productLayers.has(layerKey(sceneId, kind));
}
