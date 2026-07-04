// AGBot Workspace app shell (phase A1): populate the catalog tree with
// fields from geo_hub. Richer panels arrive in later phases.

import { apiGet, endpoints } from "./api.js";

function renderFieldList(container, fields) {
  container.replaceChildren();
  if (fields.length === 0) {
    const empty = document.createElement("p");
    empty.className = "status";
    empty.textContent = "No fields yet.";
    container.appendChild(empty);
    return;
  }
  const list = document.createElement("ul");
  for (const field of fields) {
    const item = document.createElement("li");
    item.textContent = field.name ?? field.field_id ?? "(unnamed field)";
    list.appendChild(item);
  }
  container.appendChild(list);
}

function renderError(container, error) {
  container.replaceChildren();
  const message = document.createElement("p");
  message.className = "status error";
  message.textContent = `Failed to load fields: ${error.message}`;
  container.appendChild(message);
}

async function loadCatalog() {
  const container = document.getElementById("catalog-tree");
  try {
    const page = await apiGet(endpoints.fields);
    const fields = Array.isArray(page) ? page : (page?.items ?? []);
    renderFieldList(container, fields);
  } catch (error) {
    renderError(container, error);
  }
}

document.addEventListener("DOMContentLoaded", loadCatalog);
