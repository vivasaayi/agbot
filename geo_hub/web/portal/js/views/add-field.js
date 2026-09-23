// First-field setup: draw a boundary or import one GeoJSON polygon.
import { farms, createField } from "../api.js";

export function boundaryFromGeoJSON(value) {
  if (value.type === "FeatureCollection") {
    if (value.features?.length !== 1) throw new Error("Choose a file containing exactly one field.");
    value = value.features[0];
  }
  if (value.type === "Feature") value = value.geometry;
  if (value?.type !== "Polygon" || value.coordinates?.length !== 1) {
    throw new Error("Choose a single Polygon without holes, using longitude and latitude.");
  }
  const points = value.coordinates[0];
  if (points.length < 4 || points.some(p => !Array.isArray(p) || !Number.isFinite(p[0]) || !Number.isFinite(p[1]) || Math.abs(p[0]) > 180 || Math.abs(p[1]) > 90)) {
    throw new Error("The file does not contain a valid geographic field boundary.");
  }
  return points.map(([longitude, latitude]) => ({ longitude, latitude }));
}

async function loadMapLibrary() {
  if (window.L) return window.L;
  const css = document.createElement("link");
  css.rel = "stylesheet"; css.href = "/workspace/vendor/leaflet/leaflet.css";
  document.head.appendChild(css);
  await new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = "/workspace/vendor/leaflet/leaflet.js";
    script.onload = resolve; script.onerror = () => reject(new Error("Map could not load. You can still import a field boundary."));
    document.head.appendChild(script);
  });
  return window.L;
}

export async function renderAddField(container) {
  const section = document.createElement("section");
  section.className = "view add-field-view";
  section.innerHTML = `
    <h2>Add your field</h2>
    <p>Mark the field boundary to start looking for usable Landsat vegetation imagery from the last 90 days.</p>
    <form id="add-field-form">
      <label for="farm-choice">Farm</label>
      <select id="farm-choice"><option value="">Create a new farm</option></select>
      <label for="farm-name" id="farm-name-label">New farm name</label>
      <input id="farm-name" maxlength="200" required autocomplete="organization">
      <label for="field-name">Field name</label>
      <input id="field-name" maxlength="200" required>
      <label for="field-crop">Crop (optional)</label>
      <input id="field-crop" maxlength="100" placeholder="For example, corn">
      <label for="field-season">Season (optional)</label>
      <input id="field-season" maxlength="100" placeholder="For example, 2026">
      <h3>Field boundary</h3>
      <p>Zoom in to your field. Turn on drawing, then click each corner. The last corner connects to the first.</p>
      <button type="button" id="locate-field">Find my location</button>
      <button type="button" id="draw-field" aria-pressed="false">Start drawing</button>
      <button type="button" id="undo-corner">Undo corner</button>
      <button type="button" id="clear-boundary">Clear boundary</button>
      <div id="setup-map" style="height:360px; margin:12px 0" aria-label="Field boundary map"></div>
      <p class="muted">Map tiles need an internet connection. Your location is used only to center the map.</p>
      <label for="boundary-file">Or import a GeoJSON field boundary</label>
      <input id="boundary-file" type="file" accept=".geojson,.json,application/geo+json,application/json">
      <p id="boundary-summary" role="status">No boundary selected.</p>
      <p id="setup-error" class="error-text" role="alert" hidden></p>
      <button id="save-field" class="primary-button" type="submit">Save field and start monitoring</button>
      <a href="#/home">Cancel</a>
    </form>
    <div id="setup-result" role="status" hidden></div>`;
  container.appendChild(section);
  const q = selector => section.querySelector(selector);
  let points = [];
  let map, polygon, leaflet;
  let drawing = false;
  let submitting = false;
  const error = message => { q("#setup-error").textContent = message; q("#setup-error").hidden = false; };
  const redraw = () => {
    if (polygon) polygon.remove();
    polygon = map && points.length ? leaflet.polygon(points.map(p => [p.latitude, p.longitude]), { color: "#2e7d32" }).addTo(map) : null;
    q("#boundary-summary").textContent = points.length ? `${points.length} boundary corners selected. Check that the outline matches your field.` : "No boundary selected.";
  };
  q("#farm-choice").addEventListener("change", () => {
    const existing = Boolean(q("#farm-choice").value);
    q("#farm-name").hidden = existing; q("#farm-name").required = !existing;
    q("#farm-name-label").hidden = existing;
  });
  farms().then(items => {
    for (const farm of items) {
      if (farm.status !== "active") continue;
      const option = document.createElement("option"); option.value = farm.farm_id; option.textContent = farm.name;
      q("#farm-choice").appendChild(option);
    }
  }).catch(() => {});
  q("#draw-field").addEventListener("click", () => {
    drawing = !drawing; q("#draw-field").setAttribute("aria-pressed", String(drawing));
    q("#draw-field").textContent = drawing ? "Finish drawing" : "Start drawing";
  });
  q("#undo-corner").addEventListener("click", () => { points.pop(); redraw(); });
  q("#clear-boundary").addEventListener("click", () => { points = []; redraw(); });
  q("#locate-field").addEventListener("click", () => {
    if (!navigator.geolocation) { error("Location is unavailable. Pan and zoom the map or import your boundary."); return; }
    navigator.geolocation.getCurrentPosition(position => map?.setView([position.coords.latitude, position.coords.longitude], 16), () => error("Location was unavailable. Pan and zoom the map or import your boundary."), {timeout:10000});
  });
  q("#boundary-file").addEventListener("change", async event => {
    q("#setup-error").hidden = true;
    const file = event.target.files[0]; if (!file) return;
    try {
      if (file.size > 2 * 1024 * 1024) throw new Error("Choose a boundary file smaller than 2 MB.");
      const ring = boundaryFromGeoJSON(JSON.parse(await file.text()));
      if (ring[0].longitude !== ring.at(-1).longitude || ring[0].latitude !== ring.at(-1).latitude) throw new Error("The GeoJSON polygon must have a closed boundary.");
      points = ring.slice(0, -1); redraw();
      if (polygon) map.fitBounds(polygon.getBounds(), {padding:[20,20]});
    } catch (err) { error(err.message); }
  });
  q("#add-field-form").addEventListener("submit", async event => {
    event.preventDefault(); if (submitting) return;
    q("#setup-error").hidden = true;
    if (points.length < 3) { error("Draw at least three corners or import a field boundary."); return; }
    const farmId = q("#farm-choice").value;
    const draft = {
      ...(farmId ? {farm_id:farmId} : {farm_name:q("#farm-name").value.trim()}),
      name:q("#field-name").value.trim(), crop:q("#field-crop").value.trim() || null,
      season:q("#field-season").value.trim() || null,
      boundary:{crs:"EPSG:4326",coordinates:[...points, {...points[0]}]},
    };
    submitting = true; q("#save-field").disabled = true; q("#save-field").textContent = "Saving…";
    try {
      const result = await createField(draft);
      q("#add-field-form").hidden = true;
      const message = document.createElement("p");
      message.textContent = result.monitoring.status === "worker_disabled"
        ? "Your field is saved. Satellite processing is turned off on this installation. Enable the pipeline worker to process the queued imagery search."
        : "Your field is saved and its imagery search is queued. Processing can take time and requires internet access. Clouds or unavailable imagery may leave gaps; no measurements are shown until usable imagery has been processed.";
      const link = document.createElement("a"); link.href = `#/field/${encodeURIComponent(result.field.field_id)}`; link.textContent = "Open your field";
      q("#setup-result").replaceChildren(message,link); q("#setup-result").hidden = false;
    } catch (err) { error(err.offline ? "You are offline. Reconnect and save your field again." : err.message); }
    finally { submitting = false; q("#save-field").disabled = false; q("#save-field").textContent = "Save field and start monitoring"; }
  });
  try {
    leaflet = await loadMapLibrary();
    if (!section.isConnected) return;
    map = leaflet.map(q("#setup-map"), {doubleClickZoom:false}).setView([39,-98],4);
    leaflet.tileLayer("https://tile.openstreetmap.org/{z}/{x}/{y}.png", {maxZoom:19, attribution:'&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'}).addTo(map);
    map.on("click", event => { if (drawing) { points.push({longitude:event.latlng.lng,latitude:event.latlng.lat}); redraw(); } });
    redraw();
  } catch (err) { error(err.message); }
}
