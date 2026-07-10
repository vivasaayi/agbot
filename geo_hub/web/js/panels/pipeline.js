// Ingestion & sources oversight panel (standing tool). One read-only view over
// the pipeline: registered sources -> scene-ingest health -> recent pipeline
// jobs, plus a per-field drill-down (subscriptions + backfill progress) driven
// by the current scene selection. All backend URLs come from api.js.

import {
  apiGet,
  catalogSourcesPath,
  ingestHealthPath,
  pipelineJobsPath,
  fieldSubscriptionsPath,
  fieldBackfillsPath,
} from "../api.js";

function status(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

function heading(text) {
  const h = document.createElement("h4");
  h.className = "pipeline-heading";
  h.textContent = text;
  return h;
}

function asItems(page, key) {
  if (Array.isArray(page)) return page;
  if (page && Array.isArray(page[key])) return page[key];
  return [];
}

function tag(text, kind) {
  const span = document.createElement("span");
  span.className = `tag${kind ? ` tag-${kind}` : ""}`;
  span.textContent = text;
  return span;
}

// Map a job/source/subscription status onto a semantic tag colour class.
function statusKind(value) {
  switch (value) {
    case "succeeded":
    case "active":
      return "ok";
    case "failed":
    case "dead":
      return "error";
    case "running":
    case "queued":
      return "info";
    case "paused":
      return "warn";
    default:
      return "";
  }
}

function buildTable(headers, rows) {
  const table = document.createElement("table");
  table.className = "pipeline-table";
  const thead = document.createElement("thead");
  const htr = document.createElement("tr");
  for (const h of headers) {
    const th = document.createElement("th");
    th.textContent = h;
    htr.appendChild(th);
  }
  thead.appendChild(htr);
  table.appendChild(thead);
  const tbody = document.createElement("tbody");
  for (const cells of rows) {
    const tr = document.createElement("tr");
    for (const cell of cells) {
      const td = document.createElement("td");
      if (cell instanceof Node) {
        td.appendChild(cell);
      } else {
        td.textContent = cell === null || cell === undefined ? "—" : String(cell);
      }
      tr.appendChild(td);
    }
    tbody.appendChild(tr);
  }
  table.appendChild(tbody);
  return table;
}

async function renderHealth(section) {
  section.replaceChildren(heading("Ingest health"));
  try {
    const health = await apiGet(ingestHealthPath());
    const row = document.createElement("div");
    row.className = "pipeline-metrics";
    const metric = (label, value) => {
      const box = document.createElement("div");
      box.className = "pipeline-metric";
      const v = document.createElement("span");
      v.className = "pipeline-metric-value";
      v.textContent = value ?? 0;
      const l = document.createElement("span");
      l.className = "pipeline-metric-label";
      l.textContent = label;
      box.append(v, l);
      return box;
    };
    row.append(
      metric("in-flight", health.in_flight),
      metric("succeeded", health.succeeded),
      metric("failed", health.failed),
    );
    section.appendChild(row);
    if (health.last_error) {
      const err = status(
        `Last error — scene ${health.last_error.scene_id}: ${
          health.last_error.reason_code ?? "unknown"
        } (${health.last_error.updated_at ?? ""})`,
        true,
      );
      section.appendChild(err);
    }
  } catch (error) {
    section.appendChild(status(`Health unavailable: ${error.message}`, true));
  }
}

async function renderSources(section) {
  section.replaceChildren(heading("Registered sources"));
  try {
    const sources = await apiGet(catalogSourcesPath());
    if (sources.length === 0) {
      section.appendChild(status("No sources registered yet."));
      return;
    }
    const rows = sources.map((s) => [
      s.source_id,
      s.source_kind,
      [s.platform, s.sensor].filter(Boolean).join(" / ") || "—",
      tag(s.status, statusKind(s.status)),
      s.registered_at,
    ]);
    section.appendChild(
      buildTable(["Source", "Kind", "Platform / sensor", "Status", "Registered"], rows),
    );
  } catch (error) {
    section.appendChild(status(`Sources unavailable: ${error.message}`, true));
  }
}

async function renderJobs(section) {
  section.replaceChildren(heading("Recent pipeline jobs"));
  try {
    const page = await apiGet(pipelineJobsPath({ limit: 25 }));
    const jobs = asItems(page, "jobs");
    if (jobs.length === 0) {
      section.appendChild(status("No pipeline jobs recorded."));
      return;
    }
    const rows = jobs.map((j) => [
      j.job_key ?? j.job_id,
      j.kind,
      tag(j.status, statusKind(j.status)),
      j.field_id ?? "—",
      `${j.attempts}/${j.max_attempts}`,
      j.last_error ? status(j.last_error, true) : "—",
    ]);
    section.appendChild(
      buildTable(["Job", "Kind", "Status", "Field", "Attempts", "Last error"], rows),
    );
  } catch (error) {
    section.appendChild(status(`Jobs unavailable: ${error.message}`, true));
  }
}

async function renderFieldDetail(section, fieldId) {
  section.replaceChildren(heading("Field subscriptions & backfills"));
  if (!fieldId) {
    section.appendChild(status("Select a scene to see its field's subscriptions and backfills."));
    return;
  }
  const label = document.createElement("p");
  label.className = "status";
  label.textContent = `Field ${fieldId}`;
  section.appendChild(label);

  // Subscriptions.
  try {
    const page = await apiGet(fieldSubscriptionsPath(fieldId));
    const subs = asItems(page, "subscriptions");
    const sub = document.createElement("div");
    sub.appendChild(heading("Subscriptions"));
    if (subs.length === 0) {
      sub.appendChild(status("No dataset subscriptions."));
    } else {
      const rows = subs.map((s) => [
        s.dataset,
        (s.indices ?? []).join(", ") || "—",
        `${s.cadence_hours}h`,
        tag(s.status, statusKind(s.status)),
        s.last_checked_at ?? "never",
      ]);
      sub.appendChild(
        buildTable(["Dataset", "Indices", "Cadence", "Status", "Last checked"], rows),
      );
    }
    section.appendChild(sub);
  } catch (error) {
    section.appendChild(status(`Subscriptions unavailable: ${error.message}`, true));
  }

  // Backfills.
  try {
    const page = await apiGet(fieldBackfillsPath(fieldId));
    const runs = asItems(page, "backfills");
    const bf = document.createElement("div");
    bf.appendChild(heading("Backfills"));
    if (runs.length === 0) {
      bf.appendChild(status("No backfill runs."));
    } else {
      const rows = runs.map((r) => [
        `${r.start_date} → ${r.end_date}`,
        (r.datasets ?? []).join(", ") || "—",
        tag(r.status, statusKind(r.status)),
        `${r.scenes_discovered ?? 0} scenes`,
        `${r.jobs_enqueued ?? 0} jobs`,
      ]);
      bf.appendChild(
        buildTable(["Window", "Datasets", "Status", "Discovered", "Enqueued"], rows),
      );
    }
    section.appendChild(bf);
  } catch (error) {
    section.appendChild(status(`Backfills unavailable: ${error.message}`, true));
  }
}

/**
 * Render the oversight panel into `container`. It refreshes the global sections
 * immediately and exposes `container.__setField(fieldId)` so the app shell can
 * drive the per-field drill-down from the current scene selection.
 */
export function renderPipelinePanel(container) {
  container.replaceChildren();
  const title = document.createElement("h3");
  title.textContent = "Ingestion & sources";
  container.appendChild(title);

  const refresh = document.createElement("button");
  refresh.type = "button";
  refresh.className = "pipeline-refresh";
  refresh.textContent = "Refresh";
  container.appendChild(refresh);

  const healthSection = document.createElement("section");
  const sourcesSection = document.createElement("section");
  const jobsSection = document.createElement("section");
  const fieldSection = document.createElement("section");
  container.append(healthSection, sourcesSection, jobsSection, fieldSection);

  let currentField = null;
  const refreshAll = () => {
    renderHealth(healthSection);
    renderSources(sourcesSection);
    renderJobs(jobsSection);
    renderFieldDetail(fieldSection, currentField);
  };

  refresh.addEventListener("click", refreshAll);
  refreshAll();

  container.__setField = (fieldId) => {
    currentField = fieldId || null;
    renderFieldDetail(fieldSection, currentField);
  };
}
