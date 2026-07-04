// Leaflet map + product tile layers (Track B phase A3). Uses vendored Leaflet
// (window.L, loaded by index.html). Product tile URLs come from api.js only.

import { productTilesUrlTemplate } from "./api.js";

let map = null;
let compareMap = null;
const productLayers = new Map(); // "sceneId::kind" -> L.TileLayer

function backdrop(target) {
  window.L.rectangle(
    [
      [-85, -180],
      [85, 180],
    ],
    { color: "#2c313a", weight: 0, fillColor: "#12141a", fillOpacity: 1 },
  ).addTo(target);
}

/** Initialize the primary Leaflet map on `element`. Idempotent. */
export function initMap(element) {
  if (map) return map;
  map = window.L.map(element, { center: [40.0, -100.0], zoom: 4, worldCopyJump: true });
  // A neutral offline-friendly backdrop; product tiles overlay on top. No
  // external basemap is loaded (field deployments are offline).
  backdrop(map);
  return map;
}

/** Initialize (or return) the compare-pane map on `element`. */
export function initCompareMap(element) {
  if (compareMap) return compareMap;
  compareMap = window.L.map(element, { center: [40.0, -100.0], zoom: 4, worldCopyJump: true });
  backdrop(compareMap);
  return compareMap;
}

/** The compare-pane map instance (null before initCompareMap). */
export function getCompareMap() {
  return compareMap;
}

/**
 * Keep two maps' center/zoom in lock-step. A guard flag prevents the echo that
 * would otherwise bounce a `move` event back and forth between the panes.
 */
export function syncMaps(a, b) {
  let syncing = false;
  const link = (src, dst) => {
    src.on("move", () => {
      if (syncing) return;
      syncing = true;
      dst.setView(src.getCenter(), src.getZoom(), { animate: false });
      syncing = false;
    });
  };
  link(a, b);
  link(b, a);
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

let compareLayer = null;

/** Show a single product tile layer in the compare pane (replaces any prior). */
export function setCompareProductLayer(sceneId, kind, opacity = 1.0) {
  if (!compareMap) return;
  if (compareLayer) {
    compareMap.removeLayer(compareLayer);
    compareLayer = null;
  }
  compareLayer = window.L.tileLayer(productTilesUrlTemplate(sceneId, kind), {
    opacity,
    tileSize: 256,
    minZoom: 0,
    maxZoom: 22,
    noWrap: true,
  });
  compareLayer.addTo(compareMap);
}

/** The Leaflet map instance (null before initMap). */
export function getMap() {
  return map;
}

let annotationLayer = null;

function annotationGroup() {
  if (!annotationLayer && map) {
    annotationLayer = window.L.layerGroup().addTo(map);
  }
  return annotationLayer;
}

/** Remove all annotation markers from the map. */
export function clearAnnotationMarkers() {
  if (annotationLayer) {
    annotationLayer.clearLayers();
  }
}

/**
 * Draw an annotation marker (point) or outline (polygon) on the map. Returns
 * the created Leaflet layer, or null when the geometry is unsupported.
 */
export function addAnnotationMarker(annotation) {
  const group = annotationGroup();
  if (!group) return null;
  const geometry = annotation.geometry;
  const L = window.L;
  if (geometry?.type === "point" && geometry.coordinate) {
    const marker = L.marker([geometry.coordinate.latitude, geometry.coordinate.longitude]);
    marker.bindTooltip(annotation.label ?? annotation.annotation_id ?? "annotation");
    marker.addTo(group);
    return marker;
  }
  if (geometry?.type === "polygon" && Array.isArray(geometry.coordinates)) {
    const latlngs = geometry.coordinates.map((p) => [p.latitude, p.longitude]);
    const poly = L.polygon(latlngs, { color: "#5b9e6f", weight: 2 });
    poly.bindTooltip(annotation.label ?? annotation.annotation_id ?? "annotation");
    poly.addTo(group);
    return poly;
  }
  return null;
}

/**
 * Capture the next single map click and invoke `callback({ latitude, longitude })`.
 * Returns a cancel function.
 */
export function captureNextClick(callback) {
  if (!map) return () => {};
  const handler = (event) => {
    map.off("click", handler);
    callback({ latitude: event.latlng.lat, longitude: event.latlng.lng });
  };
  map.on("click", handler);
  return () => map.off("click", handler);
}
