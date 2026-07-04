// Findings panel (Track B phase B3): list a field's application findings and
// trigger a crop-health run over per-zone NDVI stats. The run's inputs are the
// field's cataloged L2 NDVI products, so its findings trace back to source.
// Backend URLs come only from api.js.

import {
  apiGet,
  apiPost,
  scenePath,
  fieldFindingsPath,
  cropHealthRunsPath,
  catalogProductsPath,
} from "../api.js";

function asItems(page, ...keys) {
  if (Array.isArray(page)) return page;
  for (const key of keys) {
    if (Array.isArray(page?.[key])) return page[key];
  }
  return page?.items ?? [];
}

function status(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

function heading(text) {
  const h = document.createElement("h3");
  h.textContent = text;
  return h;
}

function findingRow(stored) {
  const finding = stored.finding ?? stored;
  const li = document.createElement("li");
  li.className = "finding-row";
  const title = document.createElement("span");
  title.className = "finding-kind";
  title.textContent = finding.kind ?? "(finding)";
  li.appendChild(title);
  if (finding.severity) {
    const tag = document.createElement("span");
    tag.className = `tag severity-${finding.severity}`;
    tag.textContent = finding.severity;
    li.appendChild(tag);
  }
  const zoneId = finding.metrics?.zone_id;
  if (zoneId) {
    const zone = document.createElement("span");
    zone.className = "finding-zone";
    zone.textContent = zoneId;
    li.appendChild(zone);
  }
  return li;
}

// A single editable zone-stats row for the run trigger.
function zoneInputRow() {
  const row = document.createElement("div");
  row.className = "zone-input";
  const fields = [
    ["zone_id", "text", "zone id"],
    ["mean_ndvi", "number", "mean NDVI"],
    ["ndvi_delta", "number", "NDVI Δ"],
    ["area_m2", "number", "area m²"],
  ];
  const inputs = {};
  for (const [name, type, placeholder] of fields) {
    const input = document.createElement("input");
    input.name = name;
    input.type = type;
    input.placeholder = placeholder;
    if (type === "number") input.step = "any";
    row.appendChild(input);
    inputs[name] = input;
  }
  row.readZone = (inputProductIds) => {
    const zoneId = inputs.zone_id.value.trim();
    if (!zoneId) return null;
    return {
      zone_id: zoneId,
      mean_ndvi: Number(inputs.mean_ndvi.value) || 0,
      ndvi_delta: Number(inputs.ndvi_delta.value) || 0,
      area_m2: Number(inputs.area_m2.value) || 0,
      input_product_ids: inputProductIds,
    };
  };
  return row;
}

// Resolve the field's cataloged L2 NDVI products; these are the run's inputs so
// findings trace to source. Returns product-id strings.
async function fieldNdviProductIds(fieldId) {
  const page = await apiGet(
    catalogProductsPath({ field_id: fieldId, level: "L2", kind: "ndvi" }),
  );
  return asItems(page, "products")
    .map((p) => p.product_id ?? p.id)
    .filter(Boolean);
}

async function loadFindings(list, fieldId) {
  list.replaceChildren(status("Loading findings…"));
  try {
    const findings = asItems(await apiGet(fieldFindingsPath(fieldId)), "findings");
    if (findings.length === 0) {
      list.replaceChildren(status("No findings yet. Run crop-health to compose some."));
      return;
    }
    const ul = document.createElement("ul");
    ul.className = "finding-list";
    for (const finding of findings) ul.appendChild(findingRow(finding));
    list.replaceChildren(ul);
  } catch (error) {
    list.replaceChildren(status(`Failed to load findings: ${error.message}`, true));
  }
}

function runForm(fieldId, list, note) {
  const form = document.createElement("form");
  form.className = "crop-health-run";
  const rows = document.createElement("div");
  rows.className = "zone-inputs";
  rows.appendChild(zoneInputRow());
  form.appendChild(rows);

  const addRow = document.createElement("button");
  addRow.type = "button";
  addRow.className = "link-button add-zone";
  addRow.textContent = "+ zone";
  addRow.addEventListener("click", () => rows.appendChild(zoneInputRow()));
  form.appendChild(addRow);

  const submit = document.createElement("button");
  submit.type = "submit";
  submit.className = "run-crop-health";
  submit.textContent = "Run crop-health";
  form.appendChild(submit);

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    note.replaceChildren(status("Resolving inputs…"));
    try {
      const inputProductIds = await fieldNdviProductIds(fieldId);
      if (inputProductIds.length === 0) {
        note.replaceChildren(status("No cataloged L2 NDVI products for this field.", true));
        return;
      }
      const zones = Array.from(rows.querySelectorAll(".zone-input"))
        .map((row) => row.readZone(inputProductIds))
        .filter(Boolean);
      if (zones.length === 0) {
        note.replaceChildren(status("Enter at least one zone (with a zone id).", true));
        return;
      }
      const run = await apiPost(cropHealthRunsPath(), { field_id: fieldId, zones });
      note.replaceChildren(
        status(`Run ${run.run_id} recorded ${run.output_finding_ids.length} finding(s).`),
      );
      await loadFindings(list, fieldId);
    } catch (error) {
      note.replaceChildren(status(`Run failed: ${error.message}`, true));
    }
  });
  return form;
}

/**
 * Render the findings panel for the field owning `sceneId`: list its findings
 * and expose a crop-health run trigger.
 */
export async function renderFindingsPanel(container, sceneId) {
  container.replaceChildren(status("Loading field…"));
  let fieldId;
  try {
    const scene = await apiGet(scenePath(sceneId));
    fieldId = scene.field_id ?? scene.fieldId;
  } catch (error) {
    container.replaceChildren(status(`Failed to load scene: ${error.message}`, true));
    return;
  }
  if (!fieldId) {
    container.replaceChildren(
      heading("Findings"),
      status("This scene is not linked to a field; findings are field-scoped."),
    );
    return;
  }

  const note = document.createElement("div");
  note.className = "run-note";
  const list = document.createElement("div");
  list.className = "findings-list";
  // The run form refreshes `list` on success and reports into `note`.
  container.replaceChildren(
    heading("Findings"),
    runForm(fieldId, list, note),
    note,
    list,
  );
  await loadFindings(list, fieldId);
}
