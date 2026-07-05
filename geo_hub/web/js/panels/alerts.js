// Alerts panel (Track C phase C1): list a field's fired alerts and trigger
// alert evaluation, which screens the field's findings into alerts against the
// default rule set. Backend URLs come only from api.js.

import {
  apiGet,
  apiPost,
  scenePath,
  fieldAlertsPath,
  fieldAlertEvaluationPath,
  alertLifecyclePath,
  alertAcknowledgePath,
  alertResolvePath,
} from "../api.js";

// The operator attributed to lifecycle transitions from the workspace.
const WORKSPACE_ACTOR = "workspace-operator";

function asItems(page) {
  if (Array.isArray(page)) return page;
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

// Advance an alert's lifecycle, then reflect the new state in `stateTag`.
async function transitionAlert(pathFn, alertId, stateTag) {
  stateTag.textContent = "…";
  try {
    const action = await apiPost(pathFn(alertId), { actor_id: WORKSPACE_ACTOR });
    stateTag.textContent = action.state ?? "";
    stateTag.className = `tag state-${action.state}`;
  } catch (error) {
    stateTag.textContent = `error: ${error.message}`;
    stateTag.className = "tag state-error";
  }
}

function alertRow(alert) {
  const li = document.createElement("li");
  li.className = "alert-row";
  const title = document.createElement("span");
  title.className = "alert-event";
  title.textContent = alert.event_type ?? "(alert)";
  li.appendChild(title);
  // Prefer the evidence-based classified severity (Track C C3); fall back to
  // the rule severity. Show the classified value with the rule value muted.
  const shownSeverity = alert.classified_severity ?? alert.severity;
  if (shownSeverity) {
    const tag = document.createElement("span");
    tag.className = `tag severity-${shownSeverity}`;
    tag.textContent = shownSeverity;
    if (alert.classified_severity && alert.classified_severity !== alert.severity) {
      tag.title = `classified from evidence (rule: ${alert.severity})`;
    }
    li.appendChild(tag);
  }
  const rule = document.createElement("span");
  rule.className = "alert-rule";
  rule.textContent = alert.matched_rule_id ?? "";
  li.appendChild(rule);

  const alertId = alert.alert_id;
  const stateTag = document.createElement("span");
  stateTag.className = "tag state-unknown";
  stateTag.textContent = "";
  // Reflect the persisted lifecycle state (opens at `fired` on first read).
  apiGet(alertLifecyclePath(alertId))
    .then((life) => {
      stateTag.textContent = life.state ?? "";
      stateTag.className = `tag state-${life.state}`;
    })
    .catch(() => {});
  li.appendChild(stateTag);

  const ack = document.createElement("button");
  ack.type = "button";
  ack.className = "link-button ack-alert";
  ack.textContent = "Ack";
  ack.addEventListener("click", () =>
    transitionAlert(alertAcknowledgePath, alertId, stateTag),
  );
  li.appendChild(ack);

  const resolve = document.createElement("button");
  resolve.type = "button";
  resolve.className = "link-button resolve-alert";
  resolve.textContent = "Resolve";
  resolve.addEventListener("click", () =>
    transitionAlert(alertResolvePath, alertId, stateTag),
  );
  li.appendChild(resolve);
  return li;
}

async function loadAlerts(list, fieldId) {
  list.replaceChildren(status("Loading alerts…"));
  try {
    const alerts = asItems(await apiGet(fieldAlertsPath(fieldId)));
    if (alerts.length === 0) {
      list.replaceChildren(status("No alerts. Evaluate to screen findings into alerts."));
      return;
    }
    const ul = document.createElement("ul");
    ul.className = "alert-list";
    for (const alert of alerts) ul.appendChild(alertRow(alert));
    list.replaceChildren(ul);
  } catch (error) {
    list.replaceChildren(status(`Failed to load alerts: ${error.message}`, true));
  }
}

/**
 * Render the alerts panel for the field owning `sceneId`: list its fired alerts
 * and expose an evaluation trigger over the default rule set.
 */
export async function renderAlertsPanel(container, sceneId) {
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
      heading("Alerts"),
      status("This scene is not linked to a field; alerts are field-scoped."),
    );
    return;
  }

  const note = document.createElement("div");
  note.className = "alert-note";
  const list = document.createElement("div");
  list.className = "alerts-list";

  const evaluate = document.createElement("button");
  evaluate.type = "button";
  evaluate.className = "evaluate-alerts";
  evaluate.textContent = "Evaluate alerts";
  evaluate.addEventListener("click", async () => {
    note.replaceChildren(status("Evaluating…"));
    try {
      const fired = asItems(await apiPost(fieldAlertEvaluationPath(fieldId), {}));
      note.replaceChildren(status(`Evaluation fired ${fired.length} alert(s).`));
      await loadAlerts(list, fieldId);
    } catch (error) {
      note.replaceChildren(status(`Evaluation failed: ${error.message}`, true));
    }
  });

  container.replaceChildren(heading("Alerts"), evaluate, note, list);
  await loadAlerts(list, fieldId);
}
