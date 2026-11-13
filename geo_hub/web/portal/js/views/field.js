// Field detail view (batch F-B7): boundary map with an NDVI raster toggle,
// NDVI time-series chart with activity markers, findings, actionable
// recommendations (with "log as activity"), the farm activity log, and a
// grower-report shortcut.
//
// All backend URLs come from ../api.js (route-manifest discipline). Leaflet
// is vendored under /workspace/vendor/leaflet/ (precached by sw.js) and
// loaded lazily the first time a field map is shown.

import {
  createFieldActivity,
  deleteActivity,
  fieldActivities,
  fieldActivitySummary,
  fieldOverview,
  fieldRecord,
  fieldTimeseries,
  generateGrowerReport,
  catalogTileBlob,
  updateActivity,
  updateRecommendationStatus,
} from "../api.js";
import { renderTimeseriesChart } from "../chart.js";

const LEAFLET_JS = "/workspace/vendor/leaflet/leaflet.js";
const LEAFLET_CSS = "/workspace/vendor/leaflet/leaflet.css";

const TIMESERIES_METRIC = "sat.ndvi.mean";
const TIMESERIES_YEARS = 3;

const ACTIVITY_TYPES = [
  "planting",
  "irrigation",
  "spraying",
  "fertilizing",
  "scouting",
  "harvest",
  "tillage",
  "other",
];

const ACTIVITY_ICONS = {
  planting: "🌱",
  irrigation: "💧",
  spraying: "🧴",
  fertilizing: "🧪",
  scouting: "👁",
  harvest: "🌾",
  tillage: "🚜",
  other: "📝",
};

const SEVERITY_ORDER = ["critical", "high", "medium", "low"];

// --- Small helpers -----------------------------------------------------------

function severityClass(severity) {
  const normalized = (severity || "").trim().toLowerCase();
  return SEVERITY_ORDER.includes(normalized)
    ? `severity-${normalized}`
    : "severity-none";
}

function formatDate(iso) {
  if (!iso) return "—";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleDateString();
}

function todayInputValue() {
  return new Date().toISOString().slice(0, 10);
}

function defaultRangeStart() {
  const start = new Date();
  start.setFullYear(start.getFullYear() - TIMESERIES_YEARS);
  return start.toISOString().slice(0, 10);
}

function elWithText(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function activityTypeSelect(id, selected) {
  const select = document.createElement("select");
  select.id = id;
  for (const type of ACTIVITY_TYPES) {
    const option = document.createElement("option");
    option.value = type;
    option.textContent = `${ACTIVITY_ICONS[type]} ${type}`;
    if (type === selected) option.selected = true;
    select.appendChild(option);
  }
  return select;
}

/** Read an activity draft {activity_type, occurred_at, note?, quantity?, unit?, cost?} from a form container. */
function readActivityDraft(root) {
  const type = root.querySelector(".af-type").value;
  const date = root.querySelector(".af-date").value;
  if (!type || !date) {
    return { error: "Type and date are required." };
  }
  const draft = { activity_type: type, occurred_at: date };
  const note = root.querySelector(".af-note").value.trim();
  if (note) draft.note = note;
  const quantityRaw = root.querySelector(".af-quantity").value.trim();
  if (quantityRaw !== "") {
    const quantity = Number(quantityRaw);
    if (!Number.isFinite(quantity)) return { error: "Quantity must be a number." };
    draft.quantity = quantity;
    const unit = root.querySelector(".af-unit").value.trim();
    if (!unit) return { error: "Quantity needs a unit." };
    draft.unit = unit;
  }
  const costInput = root.querySelector(".af-cost");
  if (costInput) {
    const costRaw = costInput.value.trim();
    if (costRaw !== "") {
      const cost = Number(costRaw);
      if (!Number.isFinite(cost)) return { error: "Cost must be a number." };
      draft.cost = cost;
    }
  }
  return { draft };
}

/** Shared activity form fields (type/date/note/quantity/unit[/cost]). */
function activityFormFields({ withCost = true, initial = {} } = {}) {
  const wrap = document.createElement("div");
  wrap.className = "activity-form-fields";

  const typeSelect = activityTypeSelect("", initial.activity_type || "other");
  typeSelect.classList.add("af-type");
  wrap.appendChild(labeled("Type", typeSelect));

  const date = document.createElement("input");
  date.type = "date";
  date.className = "af-date";
  date.value = (initial.occurred_at || "").slice(0, 10) || todayInputValue();
  wrap.appendChild(labeled("Date", date));

  const note = document.createElement("input");
  note.type = "text";
  note.className = "af-note";
  note.placeholder = "Note (optional)";
  note.value = initial.note || "";
  wrap.appendChild(labeled("Note", note));

  const pair = document.createElement("div");
  pair.className = "form-pair";
  const quantity = document.createElement("input");
  quantity.type = "number";
  quantity.step = "any";
  quantity.className = "af-quantity";
  quantity.placeholder = "Quantity";
  if (initial.quantity !== undefined && initial.quantity !== null) {
    quantity.value = String(initial.quantity);
  }
  const unit = document.createElement("input");
  unit.type = "text";
  unit.className = "af-unit";
  unit.placeholder = "Unit (e.g. mm, L/ha)";
  unit.value = initial.unit || "";
  pair.append(quantity, unit);
  wrap.appendChild(labeled("Quantity + unit (optional)", pair));

  if (withCost) {
    const cost = document.createElement("input");
    cost.type = "number";
    cost.step = "any";
    cost.className = "af-cost";
    cost.placeholder = "Cost (optional)";
    if (initial.cost !== undefined && initial.cost !== null) {
      cost.value = String(initial.cost);
    }
    wrap.appendChild(labeled("Cost (optional)", cost));
  }

  return wrap;
}

function labeled(text, control) {
  const label = document.createElement("label");
  label.className = "stacked-label";
  label.appendChild(elWithText("span", "stacked-label-text", text));
  label.appendChild(control);
  return label;
}

// --- Leaflet lazy loader --------------------------------------------------------

let leafletPromise = null;

function loadLeaflet() {
  if (window.L) return Promise.resolve(window.L);
  if (leafletPromise) return leafletPromise;
  leafletPromise = new Promise((resolve, reject) => {
    if (!document.querySelector(`link[href="${LEAFLET_CSS}"]`)) {
      const css = document.createElement("link");
      css.rel = "stylesheet";
      css.href = LEAFLET_CSS;
      document.head.appendChild(css);
    }
    const script = document.createElement("script");
    script.src = LEAFLET_JS;
    script.onload = () => resolve(window.L);
    script.onerror = () => {
      leafletPromise = null;
      reject(new Error("failed to load Leaflet"));
    };
    document.head.appendChild(script);
  });
  return leafletPromise;
}

/** Boundary {coordinates:[{longitude,latitude},…]} -> Leaflet latlng list. */
function boundaryLatLngs(boundary) {
  if (!boundary || !Array.isArray(boundary.coordinates)) return null;
  const latlngs = boundary.coordinates
    .filter(
      (point) =>
        point &&
        Number.isFinite(point.latitude) &&
        Number.isFinite(point.longitude),
    )
    .map((point) => [point.latitude, point.longitude]);
  return latlngs.length >= 3 ? latlngs : null;
}

// --- View --------------------------------------------------------------------

export function renderField(container, fieldId) {
  const section = document.createElement("section");
  section.className = "view field-view";
  section.innerHTML = `
    <a href="#/home" class="back-link">&larr; Back to fields</a>
    <div class="field-header">
      <h2 id="fd-name">Loading field…</h2>
      <div id="fd-chips" class="chips"></div>
    </div>
    <p id="fd-error" class="error-text" hidden></p>
    <section class="detail-card" aria-live="polite">
      <h3>Satellite monitoring</h3>
      <p id="fd-monitoring">Checking monitoring status…</p>
      <button type="button" id="fd-refresh" class="small-button secondary">Refresh status</button>
    </section>

    <section class="detail-card">
      <h3>Map</h3>
      <div id="fd-map" class="field-map"><p class="muted">Loading map…</p></div>
      <div class="map-controls">
        <label class="toggle-row">
          <input type="checkbox" id="fd-ndvi-toggle" disabled>
          <span>NDVI overlay</span>
        </label>
        <label class="toggle-row opacity-row">
          <span>Opacity</span>
          <input type="range" id="fd-ndvi-opacity" min="10" max="100" value="70" disabled>
        </label>
      </div>
      <p id="fd-ndvi-hint" class="muted small-note" hidden></p>
    </section>

    <section class="detail-card">
      <h3>NDVI trend</h3>
      <button id="fd-all-history" type="button">Show all history</button>
      <div id="fd-chart" class="chart-holder"><p class="muted">Loading history…</p></div>
    </section>

    <section class="detail-card">
      <h3>Findings</h3>
      <div id="fd-severity" class="chips"></div>
      <ul id="fd-findings" class="plain-list"></ul>
    </section>

    <section class="detail-card">
      <h3>To do</h3>
      <p id="fd-rec-count" class="muted small-note" hidden></p>
      <ul id="fd-recs" class="plain-list"></ul>
    </section>

    <section class="detail-card">
      <h3>Activity log</h3>
      <p id="fd-activity-summary" class="muted small-note" hidden></p>
      <div id="fd-activity-list" class="activity-list"><p class="muted">Loading activities…</p></div>
      <details id="fd-add-activity" class="inline-form">
        <summary>Add activity</summary>
        <div id="fd-add-activity-body"></div>
      </details>
    </section>

    <section class="detail-card">
      <h3>Grower report</h3>
      <p class="muted small-note">Generate a PDF summary of this field for your records.</p>
      <button id="fd-generate-report" class="primary-button">Generate grower report</button>
      <p id="fd-report-status" class="small-note" hidden></p>
    </section>
  `;
  container.appendChild(section);

  const state = {
    fieldId,
    series: null,
    activities: [],
    allHistory: false,
    chartHolder: section.querySelector("#fd-chart"),
  };

  loadOverviewAndSections(section, state);
  section.querySelector("#fd-refresh").addEventListener("click", () => {
    loadOverviewAndSections(section, state); loadTimeseries(section, state);
  });
  setupReportButton(section, fieldId);
  section.querySelector("#fd-all-history").addEventListener("click", async (event) => {
    state.allHistory = true;
    event.currentTarget.disabled = true;
    event.currentTarget.textContent = "Showing all available history";
    await loadTimeseries(section, state);
  });
  setupAddActivityForm(section, state);
  loadActivities(section, state);
  loadTimeseries(section, state);
}

async function loadOverviewAndSections(section, state) {
  // Serialize refreshes, including the initial load: two overlapping Leaflet
  // initializations on one container otherwise throw and leave a broken map.
  if (state.overviewLoading) return;
  state.overviewLoading = true;
  const refresh = section.querySelector("#fd-refresh");
  refresh.disabled = true;
  state.map?.remove();
  state.map = null;
  try {
    const errorLine = section.querySelector("#fd-error");
    let overview;
    try {
      overview = await fieldOverview(state.fieldId);
    } catch (error) {
      section.querySelector("#fd-name").textContent = "Field unavailable";
      errorLine.textContent = error.offline
        ? "Offline — field details are unavailable until you reconnect."
        : "Could not load this field. It may have been removed.";
      errorLine.hidden = false;
      section.querySelector("#fd-map").innerHTML = "";
      return;
    }

    section.querySelector("#fd-monitoring").textContent = overview.monitoring?.message || "Monitoring status is unavailable.";
    section.querySelector("#fd-generate-report").disabled = !overview.latest_scene;
    renderHeader(section, overview.field);
    renderFindings(section, overview);
    renderRecommendations(section, state, overview);
    errorLine.hidden = true;
    await setupMap(section, state, overview);
  } finally {
    state.overviewLoading = false;
    refresh.disabled = false;
  }
}

function renderHeader(section, field) {
  section.querySelector("#fd-name").textContent = field.name || field.field_id;
  const chips = section.querySelector("#fd-chips");
  chips.innerHTML = "";
  if (field.crop) chips.appendChild(elWithText("span", "chip", field.crop));
  if (field.season) chips.appendChild(elWithText("span", "chip chip-muted", field.season));
}

// --- Map + NDVI overlay -----------------------------------------------------------

async function setupMap(section, state, overview) {
  const mapHolder = section.querySelector("#fd-map");
  const toggle = section.querySelector("#fd-ndvi-toggle");
  const opacity = section.querySelector("#fd-ndvi-opacity");
  const hint = section.querySelector("#fd-ndvi-hint");

  const showHint = (text) => {
    hint.textContent = text;
    hint.hidden = false;
  };

  // Boundary comes from the open field record API; the portal overview
  // intentionally omits geometry.
  let latlngs = null;
  try {
    const record = await fieldRecord(state.fieldId);
    latlngs = boundaryLatLngs(record.boundary);
  } catch {
    latlngs = null;
  }

  if (!latlngs) {
    mapHolder.innerHTML = "";
    mapHolder.classList.add("field-map--empty");
    mapHolder.appendChild(
      elWithText(
        "p",
        "muted",
        "No boundary is available for this field. Add a field boundary to show its map.",
      ),
    );
    toggle.disabled = true;
    showHint("Map overlays need a field boundary.");
    return;
  }

  let leaflet;
  try {
    leaflet = await loadLeaflet();
  } catch {
    mapHolder.innerHTML = "";
    mapHolder.appendChild(
      elWithText("p", "error-text", "Map library failed to load (offline?)."),
    );
    return;
  }

  mapHolder.innerHTML = "";
  const map = leaflet.map(mapHolder, { zoomControl: true, attributionControl: false });
  state.map = map;
  const polygon = leaflet
    .polygon(latlngs, { color: "#1b5e20", weight: 2, fillOpacity: 0.08 })
    .addTo(map);
  map.fitBounds(polygon.getBounds(), { padding: [12, 12] });

  const layer = overview.latest_layer;
  if (!layer) {
    toggle.disabled = true; opacity.disabled = true;
    showHint("No accepted, georeferenced NDVI imagery is available yet. Satellite monitoring status is shown above.");
    return;
  }
  const [west, south, east, north] = layer.bbox;
  // Catalog tiles are true Web Mercator XYZ; legacy scene tiles are pixel-local
  // and must never be placed on this map as XYZ tiles.
  const AuthenticatedTiles = leaflet.GridLayer.extend({
    createTile(coords, done) {
      const image = document.createElement("img");
      image.alt = "";
      catalogTileBlob(layer.product_id, coords).then(blob => {
        const url = URL.createObjectURL(blob);
        image.onload = () => { URL.revokeObjectURL(url); done(null, image); };
        image.onerror = () => { URL.revokeObjectURL(url); done(new Error("Tile could not be decoded"), image); };
        image.src = url;
      }).catch(error => { showHint(`Imagery could not load: ${error.message}`); done(error, image); });
      return image;
    },
  });
  const ndviLayer = new AuthenticatedTiles({
    tileSize: 256, opacity: Number(opacity.value) / 100, maxZoom: 19,
    bounds: [[south, west], [north, east]], noWrap: true,
  });
  toggle.disabled = false;
  toggle.checked = true;
  opacity.disabled = false;
  ndviLayer.addTo(map);
  const valid = layer.quality_summary?.valid_pixels;
  const invalid = layer.quality_summary?.invalid_pixels;
  const coverage = Number.isFinite(valid) && Number.isFinite(invalid) && valid + invalid > 0
    ? ` ${Math.round(100 * valid / (valid + invalid))}% of the processed image pixels passed quality checks.` : "";
  const resolution = Number.isFinite(layer.gsd_m_per_px)
    ? ` Pixel size is about ${Math.round(layer.gsd_m_per_px)} m; small fields and narrow features may mix with their surroundings.` : "";
  showHint(`NDVI acquired ${formatDate(layer.acquired_at)}.${coverage}${resolution} Transparent areas have no usable data. NDVI indicates vegetation condition, not a diagnosis.`);
  toggle.onchange = () => {
    if (toggle.checked) ndviLayer.addTo(map); else map.removeLayer(ndviLayer);
    opacity.disabled = !toggle.checked;
  };
  opacity.oninput = () => ndviLayer.setOpacity(Number(opacity.value) / 100);

}

// --- Time-series chart -----------------------------------------------------------

async function loadTimeseries(section, state) {
  const holder = state.chartHolder;
  try {
    state.series = await fieldTimeseries(state.fieldId, {
      metric: TIMESERIES_METRIC,
      ...(state.allHistory ? {} : { start: defaultRangeStart() }),
    });
  } catch (error) {
    holder.innerHTML = "";
    if (error.status === 404) {
      // Field unknown to the open API: same message as an empty history.
      state.series = { per_source: {}, merged: [] };
    } else {
      holder.appendChild(
        elWithText(
          "p",
          "muted",
          error.offline
            ? "Offline — satellite history is unavailable until you reconnect."
            : "Could not load satellite history.",
        ),
      );
      return;
    }
  }
  renderChart(state);
}

function renderChart(state) {
  if (!state.series) return;
  renderTimeseriesChart(state.chartHolder, state.series, state.activities, {
    label: `NDVI trend (${TIMESERIES_METRIC})`,
    emptyMessage: state.allHistory
      ? "No satellite history yet. Check satellite monitoring status above; clouds and coverage can delay usable imagery."
      : `No satellite observations in the past ${TIMESERIES_YEARS} years. Choose Show all history to check older observations.`,
  });
}

// --- Findings ---------------------------------------------------------------------

function renderFindings(section, overview) {
  const badges = section.querySelector("#fd-severity");
  const list = section.querySelector("#fd-findings");
  badges.innerHTML = "";
  list.innerHTML = "";

  const bySeverity = overview.findings_by_severity || {};
  const orderedKeys = SEVERITY_ORDER.filter((key) => key in bySeverity).concat(
    Object.keys(bySeverity).filter((key) => !SEVERITY_ORDER.includes(key)),
  );
  for (const severity of orderedKeys) {
    badges.appendChild(
      elWithText(
        "span",
        `pill ${severityClass(severity)}`,
        `${severity}: ${bySeverity[severity]}`,
      ),
    );
  }

  const findings = (overview.recent_findings || []).slice(0, 5);
  const labels = {
    nominal_zone: "No unusual vegetation signal",
    healthy_zone: "Vegetation in expected range",
    declining_zone: "Vegetation signal decreased",
    improving_zone: "Vegetation signal increased",
    anomaly_zone: "Unusual vegetation signal — inspect this field",
  };
  if (findings.length === 0) {
    list.appendChild(elWithText("li", "muted", "No findings for this field."));
    return;
  }
  list.appendChild(elWithText("li", "muted small-note", "Latest five checks. These are satellite observations, not diagnoses. Processing dates may differ from imagery acquisition dates."));
  for (const finding of findings) {
    const item = document.createElement("li");
    item.className = "finding-row";
    item.appendChild(elWithText("span", "finding-kind", labels[finding.kind] || finding.kind.replaceAll("_", " ")));
    if (finding.severity) {
      item.appendChild(
        elWithText("span", `pill ${severityClass(finding.severity)}`, `Priority: ${finding.severity}`),
      );
    }
    item.appendChild(
      elWithText("span", "muted finding-date", `Processed ${formatDate(finding.created_at)}`),
    );
    list.appendChild(item);
  }
}

// --- Recommendations ---------------------------------------------------------------

function renderRecommendations(section, state, overview) {
  const countLine = section.querySelector("#fd-rec-count");
  const list = section.querySelector("#fd-recs");
  list.innerHTML = "";

  let openCount = overview.open_recommendation_count || 0;
  const updateCountLine = () => {
    if (openCount > (overview.open_recommendations || []).length) {
      countLine.textContent = `${openCount} open recommendations (top ${list.children.length} shown).`;
      countLine.hidden = false;
    } else {
      countLine.hidden = true;
    }
  };

  const recs = overview.open_recommendations || [];
  if (recs.length === 0) {
    list.appendChild(elWithText("li", "muted", "No recommendations have been recorded. Continue normal field scouting."));
    return;
  }

  for (const rec of recs) {
    list.appendChild(
      recommendationRow(rec, state, () => {
        openCount = Math.max(0, openCount - 1);
        if (list.querySelectorAll(".rec-row").length === 0) {
          list.appendChild(elWithText("li", "muted", "No recommendations have been recorded. Continue normal field scouting."));
        }
        updateCountLine();
      }),
    );
  }
  updateCountLine();
}

function recommendationRow(rec, state, onResolved) {
  const item = document.createElement("li");
  item.className = "rec-row";

  const top = document.createElement("div");
  top.className = "rec-top";
  top.appendChild(elWithText("span", "rec-title", rec.title));
  top.appendChild(
    elWithText("span", `pill ${severityClass(rec.priority)}`, rec.priority),
  );
  item.appendChild(top);

  const meta = [rec.category, formatDate(rec.created_at)].filter(Boolean).join(" · ");
  if (meta) item.appendChild(elWithText("div", "muted small-note", meta));

  const errorLine = elWithText("p", "error-text small-note", "");
  errorLine.hidden = true;
  item.appendChild(errorLine);

  const actions = document.createElement("div");
  actions.className = "rec-actions";
  const completeButton = elWithText("button", "small-button", "Complete");
  const dismissButton = elWithText("button", "small-button secondary", "Dismiss");
  actions.append(completeButton, dismissButton);
  item.appendChild(actions);

  const fail = (error) => {
    errorLine.textContent = error.offline
      ? "Offline — try again once reconnected."
      : error.message || "Update failed.";
    errorLine.hidden = false;
  };

  dismissButton.addEventListener("click", async () => {
    dismissButton.disabled = true;
    try {
      await updateRecommendationStatus(rec.recommendation_id, "dismissed");
      item.remove();
      onResolved();
    } catch (error) {
      dismissButton.disabled = false;
      fail(error);
    }
  });

  completeButton.addEventListener("click", () => {
    if (item.querySelector(".complete-form")) return;
    const form = completeForm(rec, state, item, onResolved, fail);
    item.appendChild(form);
  });

  return item;
}

/** Inline "log as activity?" form shown when completing a recommendation. */
function completeForm(rec, state, row, onResolved, fail) {
  const form = document.createElement("div");
  form.className = "complete-form inline-form-body";

  const logToggle = document.createElement("input");
  logToggle.type = "checkbox";
  logToggle.checked = true;
  const toggleRow = document.createElement("label");
  toggleRow.className = "toggle-row";
  toggleRow.append(logToggle, elWithText("span", "", "Log as activity?"));
  form.appendChild(toggleRow);

  const fields = activityFormFields({
    withCost: true,
    initial: { activity_type: "other", note: rec.title },
  });
  form.appendChild(fields);
  logToggle.addEventListener("change", () => {
    fields.hidden = !logToggle.checked;
  });

  const actions = document.createElement("div");
  actions.className = "rec-actions";
  const confirm = elWithText("button", "small-button", "Confirm complete");
  const cancel = elWithText("button", "small-button secondary", "Cancel");
  actions.append(confirm, cancel);
  form.appendChild(actions);

  cancel.addEventListener("click", () => form.remove());

  confirm.addEventListener("click", async () => {
    let draft;
    if (logToggle.checked) {
      const result = readActivityDraft(fields);
      if (result.error) {
        fail(new Error(result.error));
        return;
      }
      draft = result.draft;
    }
    confirm.disabled = true;
    try {
      const response = await updateRecommendationStatus(
        rec.recommendation_id,
        "completed",
        draft,
      );
      row.remove();
      onResolved();
      if (response.logged_activity_id) {
        // Refresh the activity timeline (and chart markers) in place.
        const view = document.querySelector(".field-view");
        if (view) await loadActivities(view, state);
      }
    } catch (error) {
      confirm.disabled = false;
      fail(error);
    }
  });

  return form;
}

// --- Activity log -------------------------------------------------------------------

async function loadActivities(section, state) {
  const list = section.querySelector("#fd-activity-list");
  const summaryLine = section.querySelector("#fd-activity-summary");

  try {
    const [listing, summary] = await Promise.all([
      fieldActivities(state.fieldId),
      fieldActivitySummary(state.fieldId).catch(() => null),
    ]);
    state.activities = listing.activities || [];
    renderActivityList(list, state, section);
    renderActivitySummary(summaryLine, summary);
    renderChart(state);
  } catch (error) {
    list.innerHTML = "";
    list.appendChild(
      elWithText(
        "p",
        "muted",
        error.offline
          ? "Offline — the activity log is unavailable until you reconnect."
          : "Could not load activities.",
      ),
    );
  }
}

function renderActivitySummary(summaryLine, summary) {
  if (!summary || !summary.total_count) {
    summaryLine.hidden = true;
    return;
  }
  const parts = [`${summary.total_count} activities this season`];
  if (summary.total_cost > 0) {
    parts.push(`total cost ${summary.total_cost.toFixed(2)}`);
  }
  const typeParts = Object.entries(summary.by_type || {}).map(
    ([type, entry]) => `${type} ×${entry.count}`,
  );
  if (typeParts.length > 0) parts.push(typeParts.join(", "));
  summaryLine.textContent = parts.join(" · ");
  summaryLine.hidden = false;
}

function renderActivityList(list, state, section) {
  list.innerHTML = "";
  if (state.activities.length === 0) {
    list.appendChild(
      elWithText("p", "muted", "No activities logged yet. Add the first one below."),
    );
    return;
  }
  for (const activity of state.activities) {
    list.appendChild(activityRow(activity, state, section));
  }
}

function activityRow(activity, state, section) {
  const row = document.createElement("div");
  row.className = "activity-row";

  const top = document.createElement("div");
  top.className = "activity-top";
  top.appendChild(
    elWithText(
      "span",
      "activity-type",
      `${ACTIVITY_ICONS[activity.activity_type] || "📝"} ${activity.activity_type}`,
    ),
  );
  top.appendChild(elWithText("span", "muted", formatDate(activity.occurred_at)));
  row.appendChild(top);

  if (activity.note) {
    row.appendChild(elWithText("div", "activity-note", activity.note));
  }
  const metaParts = [];
  if (activity.quantity !== null && activity.quantity !== undefined) {
    metaParts.push(`${activity.quantity} ${activity.unit || ""}`.trim());
  }
  if (activity.cost !== null && activity.cost !== undefined) {
    metaParts.push(`cost ${activity.cost}`);
  }
  if (activity.source && activity.source !== "manual") {
    metaParts.push(`from ${activity.source}`);
  }
  if (metaParts.length > 0) {
    row.appendChild(elWithText("div", "muted small-note", metaParts.join(" · ")));
  }

  const errorLine = elWithText("p", "error-text small-note", "");
  errorLine.hidden = true;
  row.appendChild(errorLine);

  const actions = document.createElement("div");
  actions.className = "rec-actions";
  const editButton = elWithText("button", "small-button secondary", "Edit");
  const deleteButton = elWithText("button", "small-button danger", "Delete");
  actions.append(editButton, deleteButton);
  row.appendChild(actions);

  deleteButton.addEventListener("click", async () => {
    if (!window.confirm("Delete this activity?")) return;
    deleteButton.disabled = true;
    try {
      await deleteActivity(activity.activity_id);
      await loadActivities(section, state);
    } catch (error) {
      deleteButton.disabled = false;
      errorLine.textContent = error.message || "Delete failed.";
      errorLine.hidden = false;
    }
  });

  editButton.addEventListener("click", () => {
    if (row.querySelector(".activity-form-fields")) return;
    const fields = activityFormFields({ withCost: true, initial: activity });
    const formActions = document.createElement("div");
    formActions.className = "rec-actions";
    const save = elWithText("button", "small-button", "Save");
    const cancel = elWithText("button", "small-button secondary", "Cancel");
    formActions.append(save, cancel);
    row.append(fields, formActions);

    cancel.addEventListener("click", () => {
      fields.remove();
      formActions.remove();
    });
    save.addEventListener("click", async () => {
      const result = readActivityDraft(fields);
      if (result.error) {
        errorLine.textContent = result.error;
        errorLine.hidden = false;
        return;
      }
      save.disabled = true;
      try {
        await updateActivity(activity.activity_id, result.draft);
        await loadActivities(section, state);
      } catch (error) {
        save.disabled = false;
        errorLine.textContent = error.message || "Update failed.";
        errorLine.hidden = false;
      }
    });
  });

  return row;
}

function setupAddActivityForm(section, state) {
  const body = section.querySelector("#fd-add-activity-body");
  const details = section.querySelector("#fd-add-activity");
  const fields = activityFormFields({ withCost: true });
  body.appendChild(fields);

  const errorLine = elWithText("p", "error-text small-note", "");
  errorLine.hidden = true;
  body.appendChild(errorLine);

  const submit = elWithText("button", "primary-button", "Save activity");
  body.appendChild(submit);

  submit.addEventListener("click", async () => {
    const result = readActivityDraft(fields);
    if (result.error) {
      errorLine.textContent = result.error;
      errorLine.hidden = false;
      return;
    }
    errorLine.hidden = true;
    submit.disabled = true;
    try {
      await createFieldActivity(state.fieldId, result.draft);
      submit.disabled = false;
      details.open = false;
      fields.querySelector(".af-note").value = "";
      fields.querySelector(".af-quantity").value = "";
      fields.querySelector(".af-unit").value = "";
      const cost = fields.querySelector(".af-cost");
      if (cost) cost.value = "";
      await loadActivities(section, state);
    } catch (error) {
      submit.disabled = false;
      errorLine.textContent = error.offline
        ? "Offline — activities cannot be saved until you reconnect."
        : error.message || "Could not save the activity.";
      errorLine.hidden = false;
    }
  });
}

// --- Grower report shortcut --------------------------------------------------------

function setupReportButton(section, fieldId) {
  const button = section.querySelector("#fd-generate-report");
  const status = section.querySelector("#fd-report-status");

  button.addEventListener("click", async () => {
    button.disabled = true;
    status.className = "small-note muted";
    status.textContent = "Generating report…";
    status.hidden = false;
    try {
      await generateGrowerReport(fieldId);
      status.className = "small-note";
      status.innerHTML = "";
      status.appendChild(document.createTextNode("Report ready — "));
      const link = elWithText("a", "", "view it in Reports");
      link.href = "#/reports";
      status.appendChild(link);
      status.appendChild(document.createTextNode("."));
    } catch (error) {
      status.className = "small-note error-text";
      status.textContent = error.offline
        ? "Offline — reports cannot be generated until you reconnect."
        : error.message || "Report generation failed.";
    }
    button.disabled = false;
  });
}
