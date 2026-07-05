// Findings panel (Track B phase B3/B4): list a field's application findings and
// trigger a crop-health or water-priority run over per-zone stats. Each app's
// run inputs are the field's cataloged L2 products of the relevant kind, so its
// findings trace back to source. Backend URLs come only from api.js.

import {
  apiGet,
  apiPost,
  scenePath,
  fieldFindingsPath,
  cropHealthRunsPath,
  waterPriorityRunsPath,
  anomalyRunsPath,
  catalogProductsPath,
} from "../api.js";

// The applications the panel can trigger. Each declares the L2 product kind
// whose catalog ids become the run inputs, the run route, and the editable
// per-zone numeric fields (besides the always-present zone_id + area_m2).
const APPS = {
  crop_health: {
    label: "Crop health",
    productKind: "ndvi",
    runsPath: cropHealthRunsPath,
    fields: [
      ["mean_ndvi", "mean NDVI"],
      ["ndvi_delta", "NDVI Δ"],
    ],
  },
  water_priority: {
    label: "Water priority",
    productKind: "soil_moisture",
    runsPath: waterPriorityRunsPath,
    fields: [
      ["mean_soil_moisture", "mean soil moisture"],
      ["water_deficit_mm", "deficit mm"],
    ],
  },
  anomaly_detection: {
    label: "Anomaly detection",
    productKind: "ndvi",
    runsPath: anomalyRunsPath,
    fields: [["index_value", "index value"]],
  },
};

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

// A single editable zone-stats row for the given app. Always has zone_id and
// area_m2, plus the app's index-specific numeric fields.
function zoneInputRow(app) {
  const row = document.createElement("div");
  row.className = "zone-input";
  const spec = [
    ["zone_id", "text", "zone id"],
    ...app.fields.map(([name, placeholder]) => [name, "number", placeholder]),
    ["area_m2", "number", "area m²"],
  ];
  const inputs = {};
  for (const [name, type, placeholder] of spec) {
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
    const zone = { zone_id: zoneId, input_product_ids: inputProductIds };
    for (const name of Object.keys(inputs)) {
      if (name === "zone_id") continue;
      zone[name] = Number(inputs[name].value) || 0;
    }
    return zone;
  };
  return row;
}

// Resolve the field's cataloged L2 products of `kind`; these are the run's
// inputs so findings trace to source. Returns product-id strings.
async function fieldProductIds(fieldId, kind) {
  const page = await apiGet(
    catalogProductsPath({ field_id: fieldId, level: "L2", kind }),
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
      list.replaceChildren(status("No findings yet. Run an application to compose some."));
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
  form.className = "application-run";

  const picker = document.createElement("select");
  picker.className = "app-picker";
  for (const [id, app] of Object.entries(APPS)) {
    const option = document.createElement("option");
    option.value = id;
    option.textContent = app.label;
    picker.appendChild(option);
  }
  form.appendChild(picker);

  const rows = document.createElement("div");
  rows.className = "zone-inputs";
  form.appendChild(rows);

  const currentApp = () => APPS[picker.value];
  const resetRows = () => rows.replaceChildren(zoneInputRow(currentApp()));
  picker.addEventListener("change", resetRows);
  resetRows();

  const addRow = document.createElement("button");
  addRow.type = "button";
  addRow.className = "link-button add-zone";
  addRow.textContent = "+ zone";
  addRow.addEventListener("click", () => rows.appendChild(zoneInputRow(currentApp())));
  form.appendChild(addRow);

  const submit = document.createElement("button");
  submit.type = "submit";
  submit.className = "run-application";
  submit.textContent = "Run";
  form.appendChild(submit);

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const app = currentApp();
    note.replaceChildren(status("Resolving inputs…"));
    try {
      const inputProductIds = await fieldProductIds(fieldId, app.productKind);
      if (inputProductIds.length === 0) {
        note.replaceChildren(
          status(`No cataloged L2 ${app.productKind} products for this field.`, true),
        );
        return;
      }
      const zones = Array.from(rows.querySelectorAll(".zone-input"))
        .map((row) => row.readZone(inputProductIds))
        .filter(Boolean);
      if (zones.length === 0) {
        note.replaceChildren(status("Enter at least one zone (with a zone id).", true));
        return;
      }
      const run = await apiPost(app.runsPath(), { field_id: fieldId, zones });
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
 * and expose crop-health / water-priority run triggers.
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
