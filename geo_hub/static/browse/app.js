// AGBot layer browser — plain ES module, no build step. Talks to the geo_hub
// APIs on the same origin (no CORS needed):
//
//   GET /api/stac/collections
//     -> { "collections": [ { "id", "description",
//            "extent": { "spatial": { "bbox": [[minLon,minLat,maxLon,maxLat]] },
//                        "temporal": { "interval": [[start,end]] } },
//            "links": [...] } ], "links": [...] }
//
//   GET /api/stac/search?collections=<id>&datetime=<start>/<end>&limit=N
//     -> { "type": "FeatureCollection", "features": [StacItem],
//          "links": [{"rel":"next", ...}?], "numberReturned": N, "agbot:skipped": M }
//
//   StacItem -> { "id", "collection",
//                 "bbox": [minLon,minLat,maxLon,maxLat]  // WGS84; ABSENT when the
//                     // stored CRS is projected (see agbot:geometry_omitted_reason),
//                 "properties": { "datetime", "processing:level",
//                     "agbot:product_kind", "agbot:scene_id"?, "proj:code"?, "gsd"?,
//                     "agbot:geometry_omitted_reason"? },
//                 "links": [ { "rel": "derived_from", "href", "title" }, ... ],
//                 "assets": { "data": { "href": "/api/scenes/<id>/products/<kind>" },
//                             "tiles": { "href":
//                       "/api/scenes/<id>/products/<kind>/tiles/{z}/{x}/{y}.png" } } }
//
//   GET /api/fields/export/geojson
//     -> standard GeoJSON FeatureCollection of Polygon features with
//        properties { field_id, name, farm_id?, crop?, area_ha?, ... }
//
// TILE GRID SEMANTICS (important): geo_hub product tiles are SCENE-LOCAL, not
// Web Mercator. `generate_tile_bytes` in geo_hub splits the product image
// itself into 2^z x 2^z equal pixel-space tiles and resizes each to 256px;
// z=0/0/0 is the whole image. A MapLibre `raster` (XYZ) source would therefore
// place them wrongly. Instead we stitch all tiles at a fixed detail zoom into
// an offscreen canvas and add it as a MapLibre `image` source positioned by
// the STAC item's WGS84 bbox (assuming a north-up raster: tile row y=0 is the
// max-latitude edge). A true global-TMS (Web Mercator) tiler is future work;
// when it lands, switch to a `raster` source with the tile URL template.

const STITCH_ZOOM = 2; // 2^2 x 2^2 tiles of 256px -> 1024x1024 stitched image
const TILE_PX = 256;
const SEARCH_LIMIT = 100;

const statusEl = document.getElementById("status");
const collectionsEl = document.getElementById("collections");

/** Currently displayed item layers: itemId -> { sourceId, layerId } */
const activeLayers = new Map();

function setStatus(text, isError = false) {
  statusEl.textContent = text;
  statusEl.classList.toggle("error", isError);
}

/** fetch wrapper: throws with a readable message, surfaces errors in the panel. */
async function fetchJson(url, options) {
  let response;
  try {
    response = await fetch(url, options);
  } catch (err) {
    throw new Error(`network error fetching ${url}: ${err.message}`);
  }
  if (!response.ok) {
    let detail = "";
    try {
      const body = await response.json();
      detail = body.description || body.message || "";
    } catch (_) {
      /* non-JSON error body */
    }
    throw new Error(`${url} -> HTTP ${response.status} ${detail}`.trim());
  }
  return response.json();
}

// --- Base map -----------------------------------------------------------------
// OSM raster basemap with the required attribution. External network dependency
// is acceptable for this internal tool (same rationale as the MapLibre CDN pin).

const map = new maplibregl.Map({
  container: "map",
  style: {
    version: 8,
    sources: {
      osm: {
        type: "raster",
        tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
        tileSize: 256,
        attribution: "&copy; OpenStreetMap contributors",
      },
    },
    layers: [{ id: "osm", type: "raster", source: "osm" }],
  },
  center: [0, 20],
  zoom: 2,
});
map.addControl(new maplibregl.NavigationControl(), "top-right");

const mapReady = new Promise((resolve) => map.on("load", resolve));

// --- Datetime filter ------------------------------------------------------------

/** Compose the STAC `datetime` query value ("start/end" with ".." open ends), or null. */
function datetimeFilterValue() {
  const from = document.getElementById("filter-from").value;
  const to = document.getElementById("filter-to").value;
  if (!from && !to) return null;
  const toRfc3339 = (local) => new Date(local).toISOString();
  return `${from ? toRfc3339(from) : ".."}/${to ? toRfc3339(to) : ".."}`;
}

document.getElementById("filter-apply").addEventListener("click", () => {
  reloadOpenCollections();
});
document.getElementById("filter-clear").addEventListener("click", () => {
  document.getElementById("filter-from").value = "";
  document.getElementById("filter-to").value = "";
  reloadOpenCollections();
});

function reloadOpenCollections() {
  for (const details of collectionsEl.querySelectorAll("details.collection[open]")) {
    loadItems(details);
  }
  setStatus("Datetime filter applied to open collections.");
}

// --- Collections & items --------------------------------------------------------

async function loadCollections() {
  try {
    const body = await fetchJson("/api/stac/collections");
    collectionsEl.innerHTML = "";
    if (!body.collections || body.collections.length === 0) {
      collectionsEl.innerHTML =
        '<p class="hint">No collections. Seed products via /api/catalog/products.</p>';
      return;
    }
    for (const collection of body.collections) {
      collectionsEl.appendChild(renderCollection(collection));
    }
  } catch (err) {
    collectionsEl.innerHTML = `<p class="error">${err.message}</p>`;
    setStatus("Failed to load STAC collections.", true);
  }
}

function renderCollection(collection) {
  const details = document.createElement("details");
  details.className = "collection";
  details.dataset.collectionId = collection.id;

  const summary = document.createElement("summary");
  summary.textContent = collection.id;
  details.appendChild(summary);

  const list = document.createElement("ul");
  list.className = "items";
  details.appendChild(list);

  // Lazy-load items the first time the collection is expanded (and on re-open
  // after a filter change, loadItems is invoked explicitly).
  details.addEventListener("toggle", () => {
    if (details.open && !details.dataset.loaded) loadItems(details);
  });
  return details;
}

async function loadItems(details) {
  const collectionId = details.dataset.collectionId;
  const list = details.querySelector("ul.items");
  list.innerHTML = '<li class="hint">Loading items…</li>';
  const params = new URLSearchParams({
    collections: collectionId,
    limit: String(SEARCH_LIMIT),
  });
  const datetime = datetimeFilterValue();
  if (datetime) params.set("datetime", datetime);
  try {
    const body = await fetchJson(`/api/stac/search?${params}`);
    details.dataset.loaded = "true";
    list.innerHTML = "";
    for (const item of body.features) {
      list.appendChild(renderItem(item));
    }
    if (body.features.length === 0) {
      list.innerHTML = '<li class="hint">No items match.</li>';
    }
    const notes = [];
    if (body["agbot:skipped"] > 0) notes.push(`${body["agbot:skipped"]} skipped (no time/space evidence)`);
    if ((body.links || []).some((l) => l.rel === "next")) notes.push(`showing first ${SEARCH_LIMIT}`);
    if (notes.length) {
      const note = document.createElement("li");
      note.className = "hint";
      note.textContent = notes.join(" · ");
      list.appendChild(note);
    }
  } catch (err) {
    list.innerHTML = `<li class="error">${err.message}</li>`;
  }
}

function renderItem(item) {
  const li = document.createElement("li");
  li.className = "item";
  const props = item.properties || {};
  const displayable = canDisplay(item);

  const head = document.createElement("div");
  head.className = "item-head";

  const toggle = document.createElement("input");
  toggle.type = "checkbox";
  toggle.title = "Show on map";
  toggle.disabled = !displayable.ok;
  toggle.checked = activeLayers.has(item.id);
  toggle.addEventListener("change", () => {
    if (toggle.checked) {
      addItemLayer(item).catch((err) => {
        toggle.checked = false;
        setStatus(err.message, true);
      });
    } else {
      removeItemLayer(item.id);
    }
  });
  head.appendChild(toggle);

  const title = document.createElement("span");
  title.className = "item-title";
  title.textContent = `${props["agbot:product_kind"] || "?"} · ${item.id}`;
  title.title = item.id;
  head.appendChild(title);
  li.appendChild(head);

  const date = document.createElement("span");
  date.className = "item-date";
  date.textContent = props.datetime || "no datetime";
  li.appendChild(date);

  if (!displayable.ok) {
    const why = document.createElement("span");
    why.className = "undisplayable";
    why.textContent = `not displayable: ${displayable.reason}`;
    li.appendChild(why);
  } else {
    // Per-layer opacity slider (only meaningful while the layer is shown).
    const row = document.createElement("div");
    row.className = "opacity-row";
    row.innerHTML = "<span>opacity</span>";
    const slider = document.createElement("input");
    slider.type = "range";
    slider.min = "0";
    slider.max = "100";
    slider.value = "85";
    slider.addEventListener("input", () => {
      const active = activeLayers.get(item.id);
      if (active) {
        map.setPaintProperty(active.layerId, "raster-opacity", Number(slider.value) / 100);
      }
    });
    row.appendChild(slider);
    li.appendChild(row);
    li._opacitySlider = slider;
  }

  li.appendChild(renderMetadata(item));
  return li;
}

/** Item metadata popover: datetime, processing level, lineage links, etc. */
function renderMetadata(item) {
  const details = document.createElement("details");
  details.className = "meta";
  const summary = document.createElement("summary");
  summary.textContent = "metadata";
  details.appendChild(summary);

  const dl = document.createElement("dl");
  dl.className = "meta-body";
  const props = item.properties || {};
  const rows = [
    ["datetime", props.datetime],
    ["processing:level", props["processing:level"]],
    ["proj:code", props["proj:code"]],
    ["gsd (m/px)", props.gsd],
    ["scene", props["agbot:scene_id"]],
    ["bbox (WGS84)", item.bbox ? item.bbox.map((v) => v.toFixed(4)).join(", ") : undefined],
    ["geometry omitted", props["agbot:geometry_omitted_reason"]],
  ];
  for (const [label, value] of rows) {
    if (value === undefined || value === null) continue;
    const dt = document.createElement("dt");
    dt.textContent = label;
    const dd = document.createElement("dd");
    dd.textContent = String(value);
    dl.appendChild(dt);
    dl.appendChild(dd);
  }
  // Lineage: derived_from links point at the input products' STAC items.
  for (const link of item.links || []) {
    if (link.rel !== "derived_from") continue;
    const dt = document.createElement("dt");
    dt.textContent = `derived_from (${link.title || "input"})`;
    const dd = document.createElement("dd");
    const a = document.createElement("a");
    a.href = link.href;
    a.target = "_blank";
    a.textContent = link.href;
    dd.appendChild(a);
    dl.appendChild(dt);
    dl.appendChild(dd);
  }
  details.appendChild(dl);
  return details;
}

/** An item is displayable when it has a WGS84 bbox to position the image and a
 *  scene-tile asset to render. Projected-CRS items have no bbox by design
 *  (geo_hub does not reproject yet) and cannot be placed. */
function canDisplay(item) {
  if (!item.bbox) {
    return {
      ok: false,
      reason:
        (item.properties || {})["agbot:geometry_omitted_reason"] || "no WGS84 bbox",
    };
  }
  if (!item.assets || !item.assets.tiles) {
    return { ok: false, reason: "no tile asset (artifact-only product)" };
  }
  return { ok: true };
}

// --- Layer display: stitch scene-local tiles, place by bbox ---------------------

/** Fetch all scene-local tiles at STITCH_ZOOM and compose the full product
 *  image on a canvas. Failed tiles stay transparent; total failure throws. */
async function stitchTiles(tileTemplate) {
  const n = 1 << STITCH_ZOOM;
  const canvas = document.createElement("canvas");
  canvas.width = n * TILE_PX;
  canvas.height = n * TILE_PX;
  const ctx = canvas.getContext("2d");

  let failures = 0;
  const jobs = [];
  for (let x = 0; x < n; x++) {
    for (let y = 0; y < n; y++) {
      const url = tileTemplate
        .replace("{z}", String(STITCH_ZOOM))
        .replace("{x}", String(x))
        .replace("{y}", String(y));
      jobs.push(
        loadImage(url)
          .then((img) => ctx.drawImage(img, x * TILE_PX, y * TILE_PX, TILE_PX, TILE_PX))
          .catch(() => {
            failures += 1;
          })
      );
    }
  }
  await Promise.all(jobs);
  if (failures === n * n) {
    throw new Error(`all ${failures} tiles failed to load from ${tileTemplate}`);
  }
  if (failures > 0) {
    setStatus(`${failures}/${n * n} tiles failed to load; gaps left transparent.`, true);
  }
  return canvas.toDataURL("image/png");
}

function loadImage(url) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`failed to load ${url}`));
    img.src = url;
  });
}

async function addItemLayer(item) {
  await mapReady;
  if (activeLayers.has(item.id)) return;
  setStatus(`Loading ${item.id}…`);

  const dataUrl = await stitchTiles(item.assets.tiles.href);
  const [minLon, minLat, maxLon, maxLat] = item.bbox;
  const sourceId = `agbot-src-${item.id}`;
  const layerId = `agbot-lyr-${item.id}`;

  // Image source corners are [TL, TR, BR, BL]. Tile row y=0 is the top of the
  // product raster, which for a north-up raster is the max-latitude edge.
  map.addSource(sourceId, {
    type: "image",
    url: dataUrl,
    coordinates: [
      [minLon, maxLat],
      [maxLon, maxLat],
      [maxLon, minLat],
      [minLon, minLat],
    ],
  });
  map.addLayer({
    id: layerId,
    type: "raster",
    source: sourceId,
    paint: { "raster-opacity": 0.85, "raster-fade-duration": 0 },
  });
  activeLayers.set(item.id, { sourceId, layerId });

  // Zoom to the item's footprint when it is added.
  map.fitBounds(
    [
      [minLon, minLat],
      [maxLon, maxLat],
    ],
    { padding: 40, maxZoom: 16 }
  );
  setStatus(`Showing ${item.id}.`);
}

function removeItemLayer(itemId) {
  const active = activeLayers.get(itemId);
  if (!active) return;
  if (map.getLayer(active.layerId)) map.removeLayer(active.layerId);
  if (map.getSource(active.sourceId)) map.removeSource(active.sourceId);
  activeLayers.delete(itemId);
  setStatus(`Removed ${itemId}.`);
}

// --- Field boundaries overlay ----------------------------------------------------

const FIELDS_SOURCE = "agbot-fields";
const FIELDS_LINE_LAYER = "agbot-fields-line";
let fieldsLoaded = false;

document.getElementById("fields-toggle").addEventListener("change", async (event) => {
  await mapReady;
  const show = event.target.checked;
  try {
    if (show && !fieldsLoaded) {
      const geojson = await fetchJson("/api/fields/export/geojson");
      map.addSource(FIELDS_SOURCE, { type: "geojson", data: geojson });
      map.addLayer({
        id: FIELDS_LINE_LAYER,
        type: "line",
        source: FIELDS_SOURCE,
        paint: { "line-color": "#ff7a00", "line-width": 2 },
      });
      // Field name popup on click.
      map.on("click", FIELDS_LINE_LAYER, (e) => {
        const props = e.features && e.features[0] ? e.features[0].properties : {};
        new maplibregl.Popup()
          .setLngLat(e.lngLat)
          .setHTML(
            `<strong>${props.name || props.field_id || "field"}</strong>` +
              (props.crop ? `<br/>crop: ${props.crop}` : "") +
              (props.area_ha ? `<br/>area: ${Number(props.area_ha).toFixed(2)} ha` : "")
          )
          .addTo(map);
      });
      fieldsLoaded = true;
      setStatus(`Field boundaries loaded (${(geojson.features || []).length} fields).`);
    } else if (map.getLayer(FIELDS_LINE_LAYER)) {
      map.setLayoutProperty(FIELDS_LINE_LAYER, "visibility", show ? "visible" : "none");
    }
  } catch (err) {
    event.target.checked = false;
    setStatus(err.message, true);
  }
});

// --- Boot -------------------------------------------------------------------------

setStatus("Loading collections…");
loadCollections().then(() => setStatus("Ready."));
