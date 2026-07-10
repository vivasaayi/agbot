// Catalog tree panel (Track B phase A2): farms -> fields -> scenes, lazily
// expanded, with a scene-detail hook. Backend URLs come only from api.js.

import {
  apiGet,
  endpoints,
  farmFieldsPath,
  fieldScenesPath,
  scenePath,
  catalogProductsPath,
  fieldTimeseriesPath,
} from "../api.js";
import {
  addCatalogProductLayer,
  removeCatalogProductLayer,
  hasCatalogProductLayer,
} from "../map.js";

const TRUE_COLOR_KINDS = new Set(["rgb", "truecolor", "true_color"]);

function asItems(page) {
  if (Array.isArray(page)) return page;
  return page?.items ?? [];
}

function label(record, ...keys) {
  for (const key of keys) {
    if (record?.[key]) return record[key];
  }
  return "(unnamed)";
}

function statusLine(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

// A tree node with a click-to-expand row and a lazily-filled child list.
function treeNode(text, onExpand) {
  const li = document.createElement("li");
  const row = document.createElement("div");
  row.className = "tree-row";
  row.textContent = text;
  li.appendChild(row);

  const children = document.createElement("ul");
  children.hidden = true;
  li.appendChild(children);

  let loaded = false;
  row.addEventListener("click", async (event) => {
    event.stopPropagation();
    children.hidden = !children.hidden;
    if (!loaded && !children.hidden) {
      loaded = true;
      children.replaceChildren(statusLine("Loading…"));
      try {
        await onExpand(children);
      } catch (error) {
        children.replaceChildren(statusLine(error.message, true));
      }
    }
  });
  return li;
}

async function expandField(container, field, onSelectScene) {
  const fieldId = field.field_id ?? field.id;
  const page = await apiGet(fieldScenesPath(fieldId));
  const scenes = asItems(page);
  container.replaceChildren();
  if (scenes.length === 0) {
    container.appendChild(statusLine("No scenes."));
    return;
  }
  for (const scene of scenes) {
    const sceneId = scene.scene_id ?? scene.id;
    const li = document.createElement("li");
    const row = document.createElement("div");
    row.className = "tree-row scene";
    row.textContent = label(scene, "scene_id", "id");
    row.addEventListener("click", (event) => {
      event.stopPropagation();
      onSelectScene(sceneId, fieldId);
    });
    li.appendChild(row);
    container.appendChild(li);
  }
}

async function expandFarm(container, farm, onSelectScene) {
  const farmId = farm.farm_id ?? farm.id;
  const page = await apiGet(farmFieldsPath(farmId));
  const fields = asItems(page);
  container.replaceChildren();
  if (fields.length === 0) {
    container.appendChild(statusLine("No fields."));
    return;
  }
  const list = document.createElement("ul");
  for (const field of fields) {
    list.appendChild(
      treeNode(label(field, "name", "field_id"), (children) =>
        expandField(children, field, onSelectScene),
      ),
    );
  }
  container.replaceChildren(list);
}

const LEVELS = ["l0", "l1", "l2", "l3"];
const LEVEL_LABELS = {
  l0: "L0 — raw / source",
  l1: "L1 — calibrated bands",
  l2: "L2 — surface products",
  l3: "L3 — analysis / indices",
};

// Draw a compact inline-SVG sparkline of {t, value} points.
function sparkline(points) {
  const width = 220;
  const height = 40;
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.setAttribute("class", "sparkline");
  svg.setAttribute("width", String(width));
  svg.setAttribute("height", String(height));
  if (points.length === 0) return svg;
  const values = points.map((p) => p.value);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const step = points.length > 1 ? width / (points.length - 1) : 0;
  const coords = points.map((p, i) => {
    const x = i * step;
    const y = height - ((p.value - min) / span) * (height - 4) - 2;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  const path = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
  path.setAttribute("points", coords.join(" "));
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.5");
  svg.appendChild(path);
  return svg;
}

// Plot the field's time-series for a metric under `host` (part 2.3c: link a
// scene's L2/L3 product to the field's level series).
async function plotSeries(host, fieldId, metric) {
  host.replaceChildren(statusLine(`Loading ${metric}…`));
  try {
    const series = await apiGet(fieldTimeseriesPath(fieldId, { metric }));
    const points = Array.isArray(series?.merged) ? series.merged : [];
    host.replaceChildren();
    if (points.length === 0) {
      host.appendChild(statusLine(`No ${metric} observations for this field yet.`));
      return;
    }
    host.appendChild(sparkline(points));
    const last = points[points.length - 1];
    const caption = document.createElement("p");
    caption.className = "status";
    caption.textContent = `${points.length} obs · latest ${Number(last.value).toFixed(3)} @ ${last.t}`;
    host.appendChild(caption);
  } catch (error) {
    host.replaceChildren(statusLine(`Series unavailable: ${error.message}`, true));
  }
}

// Render the scene's catalog products grouped by processing level (L0–L3),
// with a per-product link to plot the field's series for L2/L3 metrics.
async function renderLevelBrowser(inspector, sceneId) {
  const section = document.createElement("section");
  section.className = "level-browser";
  const heading = document.createElement("h4");
  heading.className = "pipeline-heading";
  heading.textContent = "Products by level";
  section.appendChild(heading);
  inspector.appendChild(section);

  let products;
  try {
    products = await apiGet(catalogProductsPath({ scene_id: sceneId }));
  } catch (error) {
    section.appendChild(statusLine(`Products unavailable: ${error.message}`, true));
    return;
  }
  if (!Array.isArray(products) || products.length === 0) {
    section.appendChild(statusLine("No cataloged products for this scene."));
    return;
  }

  const fieldId = products.find((p) => p.field_id)?.field_id ?? null;

  for (const level of LEVELS) {
    const atLevel = products.filter((p) => p.level === level);
    if (atLevel.length === 0) continue;
    const group = document.createElement("div");
    group.className = "level-group";
    const label = document.createElement("h5");
    label.className = "level-label";
    label.textContent = `${LEVEL_LABELS[level]} (${atLevel.length})`;
    group.appendChild(label);

    const list = document.createElement("ul");
    list.className = "level-product-list";
    for (const product of atLevel) {
      const li = document.createElement("li");
      const line = document.createElement("div");
      line.className = "level-product-row";
      line.textContent = `${product.kind} · ${product.status}`;
      if (product.temporal_start) {
        line.textContent += ` · ${product.temporal_start.slice(0, 10)}`;
      }
      li.appendChild(line);

      // True-color composites: toggle the RGB tile layer on the map.
      if (TRUE_COLOR_KINDS.has(product.kind)) {
        const toggle = document.createElement("button");
        toggle.type = "button";
        toggle.className = "pipeline-refresh";
        const sync = () => {
          toggle.textContent = hasCatalogProductLayer(product.product_id)
            ? "Hide true-color"
            : "Show true-color";
        };
        toggle.addEventListener("click", () => {
          if (hasCatalogProductLayer(product.product_id)) {
            removeCatalogProductLayer(product.product_id);
          } else {
            addCatalogProductLayer(product.product_id);
          }
          sync();
        });
        sync();
        li.appendChild(toggle);
      }

      // L2/L3 metric products: offer to plot the field's series for the
      // conventional `sat.<kind>.mean` metric.
      if ((level === "l2" || level === "l3") && fieldId) {
        const metric = `sat.${product.kind}.mean`;
        const plotHost = document.createElement("div");
        plotHost.className = "level-series";
        const button = document.createElement("button");
        button.type = "button";
        button.className = "pipeline-refresh";
        button.textContent = `Plot ${metric}`;
        button.addEventListener("click", () => plotSeries(plotHost, fieldId, metric));
        li.append(button, plotHost);
      }
      list.appendChild(li);
    }
    group.appendChild(list);
    section.appendChild(group);
  }
}

export async function renderSceneDetail(inspector, sceneId) {
  inspector.replaceChildren(statusLine("Loading scene…"));
  try {
    const scene = await apiGet(scenePath(sceneId));
    inspector.replaceChildren();
    const title = document.createElement("h3");
    title.textContent = scene.scene_id ?? sceneId;
    inspector.appendChild(title);
    const dl = document.createElement("dl");
    dl.className = "detail";
    for (const [key, value] of Object.entries(scene)) {
      if (value === null || typeof value === "object") continue;
      const dt = document.createElement("dt");
      dt.textContent = key;
      const dd = document.createElement("dd");
      dd.textContent = String(value);
      dl.append(dt, dd);
    }
    inspector.appendChild(dl);
    await renderLevelBrowser(inspector, sceneId);
  } catch (error) {
    inspector.replaceChildren(statusLine(`Failed to load scene: ${error.message}`, true));
  }
}

/**
 * Populate `treeContainer` with the farms -> fields -> scenes tree. Selecting a
 * scene calls `onSelectScene(sceneId)`.
 */
export async function initCatalogPanel(treeContainer, onSelectScene) {
  treeContainer.replaceChildren(statusLine("Loading farms…"));
  try {
    const farms = asItems(await apiGet(endpoints.farms));
    if (farms.length === 0) {
      treeContainer.replaceChildren(statusLine("No farms yet."));
      return;
    }
    const list = document.createElement("ul");
    for (const farm of farms) {
      list.appendChild(
        treeNode(label(farm, "name", "farm_id"), (children) =>
          expandFarm(children, farm, onSelectScene),
        ),
      );
    }
    treeContainer.replaceChildren(list);
  } catch (error) {
    treeContainer.replaceChildren(statusLine(`Failed to load catalog: ${error.message}`, true));
  }
}
