// AGBot Workspace app shell: wire the catalog tree (farms -> fields -> scenes)
// and render selected scene detail into the inspector. Richer inspector tabs
// arrive in later phases.

import { initCatalogPanel } from "./panels/catalog.js";

function bootstrap() {
  const tree = document.getElementById("catalog-tree");
  const detail = document.getElementById("scene-detail");
  initCatalogPanel(tree, detail);
}

document.addEventListener("DOMContentLoaded", bootstrap);
