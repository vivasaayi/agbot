// Reports view: inbox list (unread bold, mark-read on open, download) plus
// per-field grower-report generation.

import {
  fields,
  generateGrowerReport,
  markReportRead,
  reportDownloadUrl,
  reports,
} from "../api.js";

function formatDate(iso) {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleDateString();
}

function reportRow(report, onOpen) {
  const row = document.createElement("a");
  row.className = report.read ? "report-row" : "report-row unread";
  row.href = reportDownloadUrl(report.report_id);
  row.target = "_blank";
  row.rel = "noopener";
  row.innerHTML = `
    <div class="report-row-top">
      <span class="report-title"></span>
      <span class="muted report-date">${formatDate(report.created_at)}</span>
    </div>
    <div class="report-row-meta muted">
      <span class="report-field"></span>
      <span>${report.format.toUpperCase()}</span>
    </div>
  `;
  row.querySelector(".report-title").textContent = report.title;
  row.querySelector(".report-field").textContent = report.field_name;

  row.addEventListener("click", () => {
    // Opening a report marks it read; the download continues in a new tab.
    if (!report.read) {
      onOpen(report.report_id);
    }
  });
  return row;
}

export async function renderReports(container) {
  const section = document.createElement("section");
  section.className = "view reports-view";
  section.innerHTML = `
    <h2>Reports</h2>
    <div id="report-list" class="card-list">
      <p class="muted">Loading reports…</p>
    </div>
    <div class="generate-report">
      <h3>Generate grower report</h3>
      <label for="report-field-select" class="muted">Field</label>
      <select id="report-field-select"></select>
      <button id="generate-report-button" class="primary-button">
        Generate report
      </button>
      <p id="generate-status" class="muted" hidden></p>
    </div>
  `;
  container.appendChild(section);

  const list = section.querySelector("#report-list");
  const fieldSelect = section.querySelector("#report-field-select");
  const generateButton = section.querySelector("#generate-report-button");
  const generateStatus = section.querySelector("#generate-status");

  async function loadReports() {
    try {
      const inbox = await reports();
      list.innerHTML = "";
      if (inbox.length === 0) {
        list.innerHTML = '<p class="muted">No reports yet.</p>';
        return;
      }
      for (const report of inbox) {
        list.appendChild(
          reportRow(report, async (reportId) => {
            try {
              await markReportRead(reportId);
              await loadReports();
            } catch {
              // Read receipts are best-effort; the next load retries.
            }
          }),
        );
      }
    } catch (error) {
      list.innerHTML = "";
      const message = document.createElement("p");
      message.className = "error-text";
      message.textContent = error.offline
        ? "Offline — the report inbox is unavailable until you reconnect."
        : "Could not load reports. Try again later.";
      list.appendChild(message);
    }
  }

  async function loadFieldOptions() {
    try {
      const cards = await fields();
      fieldSelect.innerHTML = "";
      for (const card of cards) {
        const option = document.createElement("option");
        option.value = card.field_id;
        option.textContent = card.name;
        fieldSelect.appendChild(option);
      }
      generateButton.disabled = cards.length === 0;
    } catch {
      generateButton.disabled = true;
    }
  }

  generateButton.addEventListener("click", async () => {
    const fieldId = fieldSelect.value;
    if (!fieldId) {
      return;
    }
    generateButton.disabled = true;
    generateStatus.textContent = "Generating…";
    generateStatus.hidden = false;
    try {
      await generateGrowerReport(fieldId);
      generateStatus.textContent = "Report generated.";
      await loadReports();
    } catch (error) {
      generateStatus.textContent = error.offline
        ? "Offline — reconnect to generate reports."
        : "Report generation failed. Try again later.";
    } finally {
      generateButton.disabled = false;
    }
  });

  await Promise.all([loadReports(), loadFieldOptions()]);
}
