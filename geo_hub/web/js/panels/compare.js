// Compare mode (Track B phase A7): reveal a second Leaflet pane synced to the
// primary map, and load a chosen scene's product layer into it for a
// side-by-side comparison. Backend URLs via api.js only.

import { apiGet, catalogProductsPath } from "../api.js";
import { getMap, initCompareMap, setCompareProductLayer, syncMaps } from "../map.js";

let enabled = false;
let synced = false;

async function loadCompareScene(sceneId, notify) {
  const trimmed = (sceneId ?? "").trim();
  if (!trimmed) return;
  try {
    const page = await apiGet(catalogProductsPath({ scene_id: trimmed }));
    const items = Array.isArray(page) ? page : (page?.items ?? []);
    const product = items.find((p) => p.kind);
    if (!product) {
      notify(`No tileable products for ${trimmed}.`, true);
      return;
    }
    setCompareProductLayer(trimmed, product.kind, 1.0);
    notify(`Comparing ${trimmed} · ${product.kind}.`);
  } catch (error) {
    notify(`Failed to load ${trimmed}: ${error.message}`, true);
  }
}

/**
 * Wire the compare toggle: shows/hides `compareElement`, initializes and syncs a
 * second map, and exposes a scene selector for the compare pane.
 */
export function setupCompare(toggleButton, compareElement, controls) {
  const notify = (text, isError = false) => {
    controls.status.className = isError ? "status error" : "status";
    controls.status.textContent = text;
  };

  toggleButton.addEventListener("click", () => {
    enabled = !enabled;
    compareElement.hidden = !enabled;
    controls.wrapper.hidden = !enabled;
    toggleButton.classList.toggle("active", enabled);

    const primary = getMap();
    if (!primary) return;

    if (enabled) {
      const second = initCompareMap(compareElement);
      // Layout changed: Leaflet must recompute pane sizes.
      primary.invalidateSize();
      second.invalidateSize();
      second.setView(primary.getCenter(), primary.getZoom(), { animate: false });
      if (!synced) {
        syncMaps(primary, second);
        synced = true;
      }
    } else {
      primary.invalidateSize();
    }
  });

  controls.form.addEventListener("submit", (event) => {
    event.preventDefault();
    loadCompareScene(controls.input.value, notify);
  });
}
