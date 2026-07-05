// Catalog tree panel (Track B phase A2): farms -> fields -> scenes, lazily
// expanded, with a scene-detail hook. Backend URLs come only from api.js.

import {
  apiGet,
  endpoints,
  farmFieldsPath,
  fieldScenesPath,
  scenePath,
} from "../api.js";

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
