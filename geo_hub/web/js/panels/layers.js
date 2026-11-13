// Layers panel (Track B phase A3): for a selected scene, list its catalog
// products as toggleable tile layers with an opacity slider. Backend URLs via
// api.js only.

import { apiGet, catalogProductsPath } from "../api.js";
import {
  addCatalogProductLayer,
  focusCatalogProduct,
  removeCatalogProductLayer,
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
  toggle.id = `layer-${product.product_id}`;
  toggle.dataset.productId = product.product_id;

  const label = document.createElement("label");
  label.htmlFor = toggle.id;
  label.textContent = `${kind}${product.quality_summary?.simulation ? " (simulation)" : ""}`;

  const opacity = document.createElement("input");
  opacity.type = "range";
  opacity.min = "0";
  opacity.max = "1";
  opacity.step = "0.05";
  opacity.value = "1";
  opacity.disabled = true;

  toggle.addEventListener("change", () => {
    if (toggle.checked) {
      addCatalogProductLayer(product.product_id, Number(opacity.value));
      focusCatalogProduct(product);
      opacity.disabled = false;
    } else {
      removeCatalogProductLayer(product.product_id);
      opacity.disabled = true;
    }
  });
  opacity.addEventListener("input", () => {
    addCatalogProductLayer(product.product_id, Number(opacity.value));
  });

  row.append(toggle, label, opacity);
  return row;
}

/** Populate `container` with the toggleable product layers for `sceneId`. */
export async function renderLayersPanel(container, sceneId) {
  container.replaceChildren(status("Loading layers…"));
  try {
    const products = await apiGet(catalogProductsPath({ scene_id: sceneId, level: "l2" }));
    const items = Array.isArray(products) ? products : (products?.items ?? []);
    const layers = items.filter(p => p.product_id && p.status === "registered" &&
      (p.format === "tif" || p.format === "tiff") && p.crs);
    container.replaceChildren();
    if (layers.length === 0) {
      container.appendChild(status("No geographic raster products available."));
      return;
    }
    const heading = document.createElement("h3");
    heading.textContent = sceneId ? "Scene layers" : "Catalog layers";
    container.appendChild(heading);
    for (const product of layers) {
      container.appendChild(layerRow(sceneId, product));
    }
  } catch (error) {
    container.replaceChildren(status(`Failed to load layers: ${error.message}`, true));
  }
}
