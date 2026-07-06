// Home view: one card per field with health-at-a-glance badges.

import { fields, notificationsSummary } from "../api.js";

const SEVERITY_ORDER = ["critical", "high", "medium", "low"];

function severityClass(severity) {
  const normalized = (severity || "").trim().toLowerCase();
  return SEVERITY_ORDER.includes(normalized)
    ? `severity-${normalized}`
    : "severity-none";
}

function formatDate(iso) {
  if (!iso) {
    return "No scenes yet";
  }
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleDateString();
}

function fieldCard(card) {
  const item = document.createElement("a");
  item.className = "field-card";
  item.href = `#/field/${encodeURIComponent(card.field_id)}`;

  const badges = [];
  if (card.latest_finding_severity) {
    badges.push(
      `<span class="pill ${severityClass(card.latest_finding_severity)}">
        ${card.latest_finding_severity}
      </span>`,
    );
  }
  if (card.open_recommendations > 0) {
    badges.push(
      `<span class="pill pill-recs">${card.open_recommendations} to do</span>`,
    );
  }
  if (card.recent_alerts_7d > 0) {
    badges.push(
      `<span class="pill pill-alerts">${card.recent_alerts_7d} alerts (7d)</span>`,
    );
  }

  const cropSeason = [card.crop, card.season].filter(Boolean).join(" · ");
  item.innerHTML = `
    <div class="field-card-top">
      <span class="field-name"></span>
      <span class="field-badges">${badges.join("")}</span>
    </div>
    <div class="field-card-meta">
      <span class="field-crop"></span>
      <span class="field-scene muted">Latest scene: ${formatDate(
        card.latest_scene_at,
      )}</span>
    </div>
  `;
  // Names and crop labels come from user data; assign via textContent.
  item.querySelector(".field-name").textContent = card.name;
  item.querySelector(".field-crop").textContent = cropSeason || "—";
  return item;
}

export async function renderHome(container) {
  const section = document.createElement("section");
  section.className = "view home-view";
  section.innerHTML = `
    <h2>Your fields</h2>
    <p id="home-summary" class="muted" hidden></p>
    <div id="field-list" class="card-list">
      <p class="muted">Loading fields…</p>
    </div>
  `;
  container.appendChild(section);

  const list = section.querySelector("#field-list");
  const summaryLine = section.querySelector("#home-summary");

  // The badge summary is decorative; field cards are the primary content.
  notificationsSummary()
    .then((summary) => {
      const parts = [];
      if (summary.unread_reports > 0) {
        parts.push(`${summary.unread_reports} unread reports`);
      }
      if (summary.open_recommendations > 0) {
        parts.push(`${summary.open_recommendations} open recommendations`);
      }
      if (summary.alerts_last_7d > 0) {
        parts.push(`${summary.alerts_last_7d} alerts this week`);
      }
      if (parts.length > 0) {
        summaryLine.textContent = parts.join(" · ");
        summaryLine.hidden = false;
      }
    })
    .catch(() => {});

  try {
    const cards = await fields();
    list.innerHTML = "";
    if (cards.length === 0) {
      list.innerHTML =
        '<p class="muted">No fields yet. Your agronomist will add them.</p>';
      return;
    }
    for (const card of cards) {
      list.appendChild(fieldCard(card));
    }
  } catch (error) {
    list.innerHTML = "";
    const message = document.createElement("p");
    message.className = "error-text";
    message.textContent = error.offline
      ? "Offline — field data is unavailable until you reconnect."
      : "Could not load your fields. Pull to refresh or try again later.";
    list.appendChild(message);
  }
}
