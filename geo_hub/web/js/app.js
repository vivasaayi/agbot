// AGBot Workspace app shell: wire the catalog tree (farms -> fields -> scenes),
// the Leaflet map, and the per-scene layers + detail panels.

import { initCatalogPanel, renderSceneDetail } from "./panels/catalog.js";
import { renderLayersPanel } from "./panels/layers.js";
import { initMap } from "./map.js";

function bootstrap() {
  initMap(document.getElementById("map"));
  const tree = document.getElementById("catalog-tree");
  const detail = document.getElementById("scene-detail");
  const layers = document.getElementById("layers-panel");

  initCatalogPanel(tree, (sceneId) => {
    renderSceneDetail(detail, sceneId);
    renderLayersPanel(layers, sceneId);
  });
}

document.addEventListener("DOMContentLoaded", bootstrap);
