// Proposals review panel (Track B/D phase D3): the unified accept/reject queue
// for a field. Each row shows the proposal's source, priority, and status;
// accept/reject capture a reviewer identity. Backend URLs via api.js only.

import {
  apiGet,
  apiPost,
  fieldProposalsPath,
  proposalAcceptPath,
  proposalRejectPath,
} from "../api.js";

function status(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

function asItems(page) {
  return Array.isArray(page) ? page : (page?.items ?? page?.proposals ?? []);
}

async function decide(fieldId, proposalId, accept, container) {
  const reviewer = window.prompt("Reviewer id", "operator-1");
  if (!reviewer) return;
  const path = accept ? proposalAcceptPath(proposalId) : proposalRejectPath(proposalId);
  try {
    await apiPost(path, { reviewer_id: reviewer });
    await renderProposalsPanel(container, fieldId);
  } catch (error) {
    container.appendChild(status(`Decision failed: ${error.message}`, true));
  }
}

function proposalRow(container, fieldId, proposal) {
  const li = document.createElement("li");

  const header = document.createElement("div");
  header.className = "proposal-head";
  const title = document.createElement("span");
  title.className = "proposal-title";
  title.textContent = proposal.title ?? proposal.proposal_id ?? "(proposal)";
  const badge = document.createElement("span");
  badge.className = `badge status-${proposal.status ?? "proposed"}`;
  badge.textContent = proposal.status ?? "proposed";
  header.append(title, badge);
  li.appendChild(header);

  const meta = document.createElement("div");
  meta.className = "proposal-meta";
  meta.textContent = `${proposal.source_kind ?? "?"} · ${proposal.action_category ?? "?"} · ${
    proposal.priority ?? "?"
  }`;
  li.appendChild(meta);

  if ((proposal.status ?? "proposed") === "proposed") {
    const actions = document.createElement("div");
    actions.className = "proposal-actions";
    const accept = document.createElement("button");
    accept.className = "accept";
    accept.textContent = "Accept";
    accept.addEventListener("click", () =>
      decide(fieldId, proposal.proposal_id, true, container),
    );
    const reject = document.createElement("button");
    reject.className = "reject";
    reject.textContent = "Reject";
    reject.addEventListener("click", () =>
      decide(fieldId, proposal.proposal_id, false, container),
    );
    actions.append(accept, reject);
    li.appendChild(actions);
  } else if (proposal.reviewed_by) {
    li.appendChild(status(`${proposal.status} by ${proposal.reviewed_by}`));
  }
  return li;
}

/** Render the proposal queue for `fieldId` into `container`. */
export async function renderProposalsPanel(container, fieldId) {
  if (!fieldId) {
    container.replaceChildren();
    return;
  }
  container.replaceChildren(status("Loading proposals…"));
  let proposals = [];
  try {
    proposals = asItems(await apiGet(fieldProposalsPath(fieldId)));
  } catch (error) {
    container.replaceChildren(status(`Failed to load proposals: ${error.message}`, true));
    return;
  }
  const heading = document.createElement("h3");
  heading.textContent = "Proposals";
  container.replaceChildren(heading);
  if (proposals.length === 0) {
    container.appendChild(status("No proposals in the queue."));
    return;
  }
  const list = document.createElement("ul");
  list.className = "proposal-list";
  for (const proposal of proposals) {
    list.appendChild(proposalRow(container, fieldId, proposal));
  }
  container.appendChild(list);
}
