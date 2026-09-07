// AGBot Workspace app shell: wire the catalog tree (farms -> fields -> scenes),
// the Leaflet map, and the per-scene layers + detail panels.

import { initCatalogPanel, renderSceneDetail } from "./panels/catalog.js";
import { renderLayersPanel } from "./panels/layers.js";
import { renderAnnotationsPanel } from "./panels/annotations.js";
import { renderRecommendationsPanel } from "./panels/recommendations.js";
import { renderFindingsPanel } from "./panels/findings.js";
import { renderAlertsPanel } from "./panels/alerts.js";
import { renderProposalsPanel } from "./panels/proposals.js";
import { renderProvenancePanel } from "./panels/provenance.js";
import { renderPipelinePanel } from "./panels/pipeline.js";
import { setupCompare } from "./panels/compare.js";
import { initMap } from "./map.js";

function bootstrap() {
  initMap(document.getElementById("map-primary"));
  setupCompare(
    document.getElementById("compare-toggle"),
    document.getElementById("map-compare"),
    {
      wrapper: document.getElementById("compare-controls"),
      form: document.getElementById("compare-controls"),
      input: document.getElementById("compare-scene"),
      status: document.getElementById("compare-status"),
    },
  );
  const tree = document.getElementById("catalog-tree");
  const detail = document.getElementById("scene-detail");
  const layers = document.getElementById("layers-panel");
  const annotations = document.getElementById("annotations-panel");
  const recommendations = document.getElementById("recommendations-panel");
  const findings = document.getElementById("findings-panel");
  const alerts = document.getElementById("alerts-panel");
  const proposals = document.getElementById("proposals-panel");

  // The provenance inspector is a standing tool (trace any artifact id).
  renderProvenancePanel(document.getElementById("provenance-panel"));

  // The ingestion & sources oversight dashboard is also a standing tool; it
  // refreshes globally and drills into the selected scene's field on demand.
  const pipeline = document.getElementById("pipeline-panel");
  renderPipelinePanel(pipeline);

  initCatalogPanel(tree, (sceneId, fieldId) => {
    renderSceneDetail(detail, sceneId);
    renderLayersPanel(layers, sceneId);
    renderAnnotationsPanel(annotations, sceneId);
    renderRecommendationsPanel(recommendations, sceneId);
    renderFindingsPanel(findings, sceneId);
    renderAlertsPanel(alerts, sceneId);
    renderProposalsPanel(proposals, fieldId);
    if (pipeline.__setField) {
      pipeline.__setField(fieldId);
    }
  });
}

document.addEventListener("DOMContentLoaded", bootstrap);
