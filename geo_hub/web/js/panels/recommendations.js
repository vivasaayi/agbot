// Recommendations + reports panel (Track B phase A5): list a scene's advisor
// recommendations and reports; open a report to view its provenance lineage.
// Backend URLs via api.js only.

import {
  apiGet,
  sceneRecommendationsPath,
  sceneReportsPath,
  sceneReportLineagePath,
} from "../api.js";

function status(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

function asItems(page, ...keys) {
  if (Array.isArray(page)) return page;
  for (const key of keys) {
    if (Array.isArray(page?.[key])) return page[key];
  }
  return page?.items ?? [];
}

function heading(text) {
  const h = document.createElement("h3");
  h.textContent = text;
  return h;
}

function recommendationRow(rec) {
  const li = document.createElement("li");
  const title = document.createElement("span");
  title.textContent = rec.title ?? rec.recommendation_id ?? "(recommendation)";
  li.appendChild(title);
  if (rec.category ?? rec.severity) {
    const tag = document.createElement("span");
    tag.className = "tag";
    tag.textContent = rec.category ?? rec.severity;
    li.appendChild(tag);
  }
  return li;
}

async function showLineage(container, sceneId, reportId) {
  container.replaceChildren(status("Loading lineage…"));
  try {
    const lineage = await apiGet(sceneReportLineagePath(sceneId, reportId));
    // The lineage payload is a backward trace: { records: [...], gaps: [...] }.
    const records = asItems(lineage, "records");
    const gaps = Array.isArray(lineage?.gaps) ? lineage.gaps : [];
    container.replaceChildren(heading(`Lineage — ${reportId}`));
    if (records.length === 0) {
      container.appendChild(status("No lineage records."));
      return;
    }
    const list = document.createElement("ol");
    list.className = "lineage-list";
    for (const record of records) {
      const li = document.createElement("li");
      const kind = record.kind ?? "artifact";
      const id = record.artifact_id ?? record.product_id ?? "";
      li.textContent = `${kind}: ${id}`;
      list.appendChild(li);
    }
    container.appendChild(list);
    const note = document.createElement("p");
    note.className = gaps.length === 0 ? "status" : "status error";
    note.textContent =
      gaps.length === 0
        ? `Traced to source with no gaps (${records.length} records).`
        : `${gaps.length} lineage gap(s) detected.`;
    container.appendChild(note);
  } catch (error) {
    container.replaceChildren(status(`Failed to load lineage: ${error.message}`, true));
  }
}

function reportRow(container, sceneId, report) {
  const li = document.createElement("li");
  const open = document.createElement("button");
  open.className = "link-button open-report";
  open.textContent = report.title ?? report.report_id ?? "(report)";
  const reportId = report.report_id ?? report.id;
  open.addEventListener("click", () => {
    const lineageBox = document.getElementById("report-lineage");
    showLineage(lineageBox, sceneId, reportId);
  });
  li.appendChild(open);
  return li;
}

/** Render recommendations + reports for `sceneId` into `container`. */
export async function renderRecommendationsPanel(container, sceneId) {
  container.replaceChildren(status("Loading recommendations…"));
  let recs = [];
  let reports = [];
  try {
    [recs, reports] = await Promise.all([
      apiGet(sceneRecommendationsPath(sceneId)).then((p) =>
        asItems(p, "recommendations"),
      ),
      apiGet(sceneReportsPath(sceneId)).then((p) => asItems(p, "reports")),
    ]);
  } catch (error) {
    container.replaceChildren(status(`Failed to load: ${error.message}`, true));
    return;
  }

  container.replaceChildren(heading("Recommendations"));
  if (recs.length === 0) {
    container.appendChild(status("No recommendations."));
  } else {
    const list = document.createElement("ul");
    list.className = "recommendation-list";
    for (const rec of recs) list.appendChild(recommendationRow(rec));
    container.appendChild(list);
  }

  container.appendChild(heading("Reports"));
  if (reports.length === 0) {
    container.appendChild(status("No reports."));
  } else {
    const list = document.createElement("ul");
    list.className = "report-list";
    for (const report of reports) list.appendChild(reportRow(container, sceneId, report));
    container.appendChild(list);
  }

  const lineage = document.createElement("div");
  lineage.id = "report-lineage";
  container.appendChild(lineage);
}
