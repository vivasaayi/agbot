// Layers panel (Track B phase A3): for a selected scene, list its catalog
// products as toggleable tile layers with an opacity slider. Backend URLs via
// api.js only.

import { apiGet, catalogProductsPath } from "../api.js";
import {
  addProductLayer,
  removeProductLayer,
  setProductLayerOpacity,
} from "../map.js";

function status(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

function layerRow(sceneId, product) {
  const kind = product.kind;
  const row = document.createElement("div");
  row.className = "layer-row";

  const toggle = document.createElement("input");
  toggle.type = "checkbox";
  toggle.id = `layer-${kind}`;

  const label = document.createElement("label");
  label.htmlFor = toggle.id;
  label.textContent = kind;

  const opacity = document.createElement("input");
  opacity.type = "range";
  opacity.min = "0";
  opacity.max = "1";
  opacity.step = "0.05";
  opacity.value = "1";
  opacity.disabled = true;

  toggle.addEventListener("change", () => {
    if (toggle.checked) {
      addProductLayer(sceneId, kind, Number(opacity.value));
      opacity.disabled = false;
    } else {
      removeProductLayer(sceneId, kind);
      opacity.disabled = true;
    }
  });
  opacity.addEventListener("input", () => {
    setProductLayerOpacity(sceneId, kind, Number(opacity.value));
  });

  row.append(toggle, label, opacity);
  return row;
}

/** Populate `container` with the toggleable product layers for `sceneId`. */
export async function renderLayersPanel(container, sceneId) {
  container.replaceChildren(status("Loading layers…"));
  try {
    const products = await apiGet(catalogProductsPath({ scene_id: sceneId }));
    const items = Array.isArray(products) ? products : (products?.items ?? []);
    // De-duplicate by kind (a scene may have one tileable product per kind).
    const seen = new Set();
    const layers = items.filter((p) => p.kind && !seen.has(p.kind) && seen.add(p.kind));
    container.replaceChildren();
    if (layers.length === 0) {
      container.appendChild(status("No tileable products for this scene."));
      return;
    }
    const heading = document.createElement("h3");
    heading.textContent = "Layers";
    container.appendChild(heading);
    for (const product of layers) {
      container.appendChild(layerRow(sceneId, product));
    }
  } catch (error) {
    container.replaceChildren(status(`Failed to load layers: ${error.message}`, true));
  }
}
