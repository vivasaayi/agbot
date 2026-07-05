// Annotations panel (Track B phase A4): list, create (click-to-place point),
// and delete a scene's annotations, linked to the map. Backend URLs via api.js.

import {
  apiDelete,
  apiGet,
  apiPost,
  sceneAnnotationsPath,
  sceneAnnotationPath,
} from "../api.js";
import { addAnnotationMarker, captureNextClick, clearAnnotationMarkers } from "../map.js";

let activeScene = null;

function status(text, isError = false) {
  const p = document.createElement("p");
  p.className = isError ? "status error" : "status";
  p.textContent = text;
  return p;
}

function asItems(page) {
  return Array.isArray(page) ? page : (page?.items ?? page?.annotations ?? []);
}

async function refresh(container, sceneId) {
  container.replaceChildren(status("Loading annotations…"));
  clearAnnotationMarkers();
  let annotations = [];
  try {
    annotations = asItems(await apiGet(sceneAnnotationsPath(sceneId)));
  } catch (error) {
    container.replaceChildren(status(`Failed to load annotations: ${error.message}`, true));
    return;
  }
  container.replaceChildren();
  container.appendChild(addButton(container, sceneId));

  if (annotations.length === 0) {
    container.appendChild(status("No annotations yet."));
    return;
  }
  const list = document.createElement("ul");
  list.className = "annotation-list";
  for (const annotation of annotations) {
    addAnnotationMarker(annotation);
    list.appendChild(annotationRow(container, sceneId, annotation));
  }
  container.appendChild(list);
}

function annotationRow(container, sceneId, annotation) {
  const li = document.createElement("li");
  const text = document.createElement("span");
  text.textContent = annotation.label ?? annotation.annotation_id ?? "(annotation)";
  const del = document.createElement("button");
  del.textContent = "delete";
  del.className = "link-button";
  del.addEventListener("click", async () => {
    const id = annotation.annotation_id ?? annotation.id;
    try {
      await apiDelete(sceneAnnotationPath(sceneId, id));
      await refresh(container, sceneId);
    } catch (error) {
      container.appendChild(status(`Delete failed: ${error.message}`, true));
    }
  });
  li.append(text, del);
  return li;
}

function addButton(container, sceneId) {
  const button = document.createElement("button");
  button.textContent = "+ Add point (click the map)";
  button.className = "add-annotation";
  button.addEventListener("click", () => {
    button.disabled = true;
    button.textContent = "Click the map to place…";
    captureNextClick(async ({ latitude, longitude }) => {
      const label = window.prompt("Annotation label", "note") ?? "note";
      try {
        await apiPost(sceneAnnotationsPath(sceneId), {
          label,
          geometry: { type: "point", coordinate: { latitude, longitude } },
        });
        await refresh(container, sceneId);
      } catch (error) {
        container.appendChild(status(`Create failed: ${error.message}`, true));
        button.disabled = false;
        button.textContent = "+ Add point (click the map)";
      }
    });
  });
  return button;
}

/** Render the annotations panel for `sceneId` into `container`. */
export async function renderAnnotationsPanel(container, sceneId) {
  activeScene = sceneId;
  await refresh(container, sceneId);
}

export function activeAnnotationScene() {
  return activeScene;
}
