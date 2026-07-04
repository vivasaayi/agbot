// Provenance inspector (Track B phase A6): trace any artifact id (product,
// finding, recommendation, report) back to its L0 sources via the backward
// trace API, rendering the lineage chain and any gaps. Backend URLs via api.js.

import { apiGet, provenanceTracePath } from "../api.js";

function status(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

function records(trace) {
  return Array.isArray(trace?.records) ? trace.records : [];
}

/** Fetch and render the backward trace for `artifactId` into `output`. */
export async function traceArtifact(output, artifactId) {
  if (!artifactId) {
    output.replaceChildren(status("Enter an artifact id to trace."));
    return;
  }
  output.replaceChildren(status(`Tracing ${artifactId}…`));
  let trace;
  try {
    trace = await apiGet(provenanceTracePath(artifactId));
  } catch (error) {
    output.replaceChildren(status(`Trace failed: ${error.message}`, true));
    return;
  }
  const recs = records(trace);
  const gaps = Array.isArray(trace?.gaps) ? trace.gaps : [];
  output.replaceChildren();
  if (recs.length === 0) {
    output.appendChild(status("No lineage found for this artifact."));
    return;
  }

  const chain = document.createElement("ol");
  chain.className = "lineage-list";
  for (const record of recs) {
    const li = document.createElement("li");
    const kind = record.kind ?? "artifact";
    const id = record.artifact_id ?? record.product_id ?? "";
    const inputs = Array.isArray(record.inputs) ? record.inputs.length : 0;
    li.textContent = inputs > 0 ? `${kind}: ${id} (${inputs} input${inputs === 1 ? "" : "s"})` : `${kind}: ${id} (root)`;
    chain.appendChild(li);
  }
  output.appendChild(chain);

  const summary = document.createElement("p");
  summary.className = gaps.length === 0 ? "status" : "status error";
  summary.textContent =
    gaps.length === 0
      ? `Complete chain: ${recs.length} record(s), traced to source with no gaps.`
      : `${gaps.length} gap(s): ${gaps
          .map((g) => g.missing_artifact_id ?? "unknown")
          .join(", ")}`;
  output.appendChild(summary);
}

/** Render the provenance inspector (input + trace output) into `container`. */
export function renderProvenancePanel(container) {
  container.replaceChildren();
  const heading = document.createElement("h3");
  heading.textContent = "Provenance";
  container.appendChild(heading);

  const form = document.createElement("form");
  form.className = "provenance-form";
  const input = document.createElement("input");
  input.type = "text";
  input.placeholder = "artifact / product id";
  const button = document.createElement("button");
  button.type = "submit";
  button.textContent = "Trace";
  form.append(input, button);

  const output = document.createElement("div");
  output.className = "provenance-output";
  output.appendChild(status("Enter an artifact id to trace its lineage to source."));

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    traceArtifact(output, input.value.trim());
  });

  container.append(form, output);

  // Let other panels drive the inspector (e.g. selecting a report/product).
  container.__trace = (artifactId) => {
    input.value = artifactId;
    traceArtifact(output, artifactId);
  };
}
