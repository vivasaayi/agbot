import { archiveUrl, mapTileUrl, nextPage, parseCoordinates, platform, tileId } from './archive.mjs';
import { plot } from './chart.js';

const $ = (id) => document.getElementById(id);
const isoDay = (date) => date.toISOString().slice(0, 10);
const now = new Date();
$('end').value = isoDay(now);
$('start').value = isoDay(new Date(now.valueOf() - 365 * 86400000));
let point = { lat: 13.0081524, lon: 80.055375 };
let session,
  overlay,
  selected,
  rows = [],
  next,
  busy,
  placeLookup;
let map,
  marker,
  fullInterval = false,
  manualSelection = false;
if (window.L) {
  map = L.map('map', { scrollWheelZoom: false }).setView([point.lat, point.lon], 13);
  L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '© <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
  }).addTo(map);
  marker = L.circleMarker([point.lat, point.lon], {
    radius: 6,
    color: '#fff',
    weight: 3,
    fillColor: '#bc5427',
    fillOpacity: 1,
  }).addTo(map);
  map.on('click', (e) => choose({ lat: e.latlng.lat, lon: e.latlng.wrap().lng }, 'Map pin'));
} else {
  $('map-message').textContent = 'Map unavailable. Use the location search to explore scenes.';
}
function updatePoint() {
  $('coords').textContent = point.lat.toFixed(5) + ', ' + point.lon.toFixed(5);
  if (map) {
    marker.setLatLng([point.lat, point.lon]);
    map.setView(marker.getLatLng(), Math.max(12, map.getZoom()));
  }
}

function choose(location, name) {
  placeLookup?.abort();
  $('matches').replaceChildren();
  point = location;
  $('place-name').textContent = name;
  $('place').value = '';
  updatePoint();
  search();
}

async function json(url, signal) {
  const response = await fetch(url, { signal, credentials: 'omit' });
  if (!response.ok) throw new Error('HTTP ' + response.status);
  return response.json();
}

$('location-form').onsubmit = async (event) => {
  event.preventDefault();
  placeLookup?.abort();
  const query = $('place').value.trim(),
    coords = parseCoordinates(query);
  if (coords) return choose(coords, 'Map pin');
  const lookup = new AbortController();
  placeLookup = lookup;
  $('matches').textContent = 'Finding places…';
  try {
    const data = await json(
      'https://photon.komoot.io/api/?' + new URLSearchParams({ q: query, limit: '5' }),
      lookup.signal,
    );
    $('matches').replaceChildren();
    for (const { properties: p, geometry } of data.features) {
      const [lon, lat] = geometry.coordinates;
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = [...new Set([p.name, p.city, p.state, p.country].filter(Boolean))].join(', ');
      button.onclick = () => choose({ lat, lon }, p.name ?? query);
      $('matches').append(button);
    }
    if (!data.features.length)
      $('matches').textContent = 'No matching place. Try coordinates or pick the map.';
  } catch {
    if (!lookup.signal.aborted)
      $('matches').textContent = 'Place search unavailable. Enter coordinates or pick the map.';
  }
};

function drawMap() {
  if (!map) return;
  if (overlay) {
    overlay.remove();
    overlay = null;
  }
  $('map-message').textContent = '';
  const layer = $('map-layer').value;
  $('map-caption').textContent = 'Location map';
  if (!selected || layer === 'base') return;
  const scene = selected.scene;
  const kind = layer === 'index' ? $('metric').value : 'rgb';
  const url = mapTileUrl(scene, kind);
  if (!url) {
    $('map-message').textContent = 'Calibration unavailable for this scene.';
    return;
  }
  const bbox = scene.bbox;
  const tiles = L.tileLayer(url, {
    maxZoom: 19,
    maxNativeZoom: 15,
    bounds: bbox
      ? [
          [bbox[1], bbox[0]],
          [bbox[3], bbox[2]],
        ]
      : undefined,
    attribution: 'Satellite imagery · Microsoft Planetary Computer',
  });
  tiles.on('tileerror', () => {
    if (tiles === overlay) $('map-message').textContent = 'Some imagery tiles could not be loaded.';
  });
  overlay = tiles;
  tiles.addTo(map);
  $('map-caption').textContent =
    platform(scene) +
    ' · ' +
    scene.properties.datetime.slice(0, 10) +
    (layer === 'index' ? ' · ' + kind.toUpperCase() + ' (clouds included)' : '');
}

function renderHistory() {
  const metric = $('metric').value;
  $('value-head').textContent = metric.toUpperCase();
  $('chart-label').textContent = metric.toUpperCase() + ' at the pin';
  $('metric-note').textContent =
    {
      ndwi: 'Higher NDWI can indicate surface water. Water depth needs a gauge or local calibration.',
      mndwi: 'MNDWI uses green and shortwave infrared to highlight open water.',
      ndvi: 'NDVI measures vegetation greenness from red and near-infrared reflectance.',
      ndmi: 'NDMI measures a moisture signal from near-infrared and shortwave infrared reflectance.',
    }[metric] +
    ' Landsat pixels span 30 m. Sentinel-2 spans ' +
    (['ndwi', 'ndvi'].includes(metric) ? '10' : '20') +
    ' m for this index. Small ponds can share a pixel with the bank. Cloud and missing-data gaps are omitted.';
  plot($('chart'), rows, metric);
  const body = document.createDocumentFragment();
  for (const row of rows) {
    const scene = row.scene,
      props = scene.properties,
      value = row.indices?.[metric];
    const tr = document.createElement('tr');
    if (selected === row) tr.className = 'active';
    const valueText = Number.isFinite(value)
      ? value.toFixed(3)
      : { ready: 'Quality gap', error: 'Unavailable', pending: 'Reading…' }[row.state];
    for (const text of [
      props.datetime.slice(0, 10),
      platform(scene),
      tileId(scene),
      Number.isFinite(props['eo:cloud_cover']) ? Math.round(props['eo:cloud_cover']) + '%' : '—',
      valueText,
    ]) {
      const td = document.createElement('td');
      td.textContent = text;
      tr.append(td);
    }
    const record = document.createElement('a');
    record.href =
      'https://planetarycomputer.microsoft.com/api/stac/v1/collections/' +
      encodeURIComponent(scene.collection) +
      '/items/' +
      encodeURIComponent(scene.id);
    record.title = scene.id;
    record.textContent = tileId(scene);
    record.target = '_blank';
    record.rel = 'noreferrer';
    tr.cells[2].replaceChildren(record);
    const td = document.createElement('td'),
      button = document.createElement('button');
    button.textContent = selected === row ? 'On map' : 'View';
    button.title = scene.id;
    button.setAttribute('aria-label', 'Show scene ' + scene.id);
    button.setAttribute('aria-pressed', String(selected === row));
    button.onclick = () => {
      manualSelection = true;
      selected = row;
      if ($('map-layer').value === 'base') $('map-layer').value = 'rgb';
      drawMap();
      renderHistory();
    };
    td.append(button);
    tr.append(td);
    body.append(tr);
  }
  $('scenes').replaceChildren(body);
  $('retry').hidden = busy || !rows.some((row) => row.state === 'error');
  $('more').hidden = !next;
  $('more').disabled = busy;
  $('all-pages').hidden = !next && !fullInterval;
  $('all-pages').disabled = busy && !fullInterval;
  $('all-pages').textContent = fullInterval ? 'Pause after this page' : 'Load full interval';
}

function summary() {
  if (!rows.length)
    return 'No scenes cover this point in the selected dates. Try a wider interval or both archives.';
  const count = rows.filter((row) => Number.isFinite(row.indices?.[$('metric').value])).length;
  const unavailable = rows.filter((row) => row.state === 'error').length;
  return (
    rows.length +
    ' scenes loaded · ' +
    count +
    ' usable readings' +
    (unavailable ? ' · ' + unavailable + ' unavailable' : '') +
    ' · ' +
    (next ? 'More scenes available' : 'End of selected interval')
  );
}

async function sample(batch, signal) {
  const queue = [...batch];
  async function read() {
    while (queue.length && !signal.aborted) {
      const row = queue.shift(),
        scene = row.scene;
      const query = new URLSearchParams({
        collection: scene.collection,
        item: scene.id,
        latitude: point.lat,
        longitude: point.lon,
      });
      try {
        const data = await json('/api/explore/observation?' + query, signal);
        row.indices = data.indices;
        row.state = 'ready';
      } catch {
        row.state = 'error';
      }
      if (!signal.aborted) {
        const done = batch.filter((item) => item.state !== 'pending').length;
        $('status').textContent = 'Reading ' + done + ' of ' + batch.length + ' scenes on this page…';
      }
    }
  }
  await Promise.all([read(), read()]);
}

async function page(url) {
  const { signal } = session;
  busy = true;
  $('status').textContent = 'Searching the satellite archive…';
  renderHistory();
  try {
    const result = await json(url, signal);
    if (signal.aborted) return;
    const seen = new Set(rows.map((row) => row.scene.id));
    const batch = [];
    for (const scene of result.features) {
      if (seen.has(scene.id) || !Number.isFinite(Date.parse(scene.properties.datetime))) continue;
      seen.add(scene.id);
      batch.push({ scene, state: 'pending' });
    }
    rows.push(...batch);
    next = nextPage(result.links);
    if (next === String(url)) next = null;
    if (!selected && batch.length) {
      selected = batch.reduce((best, row) =>
        (row.scene.properties['eo:cloud_cover'] ?? 101) <
        (best.scene.properties['eo:cloud_cover'] ?? 101)
          ? row
          : best,
      );
      drawMap();
    }
    $('status').textContent = 'Reading quality-screened pixels for ' + batch.length + ' scenes…';
    renderHistory();
    await sample(batch, signal);
    if (!signal.aborted) {
      const metric = $('metric').value;
      if (!manualSelection && selected && !Number.isFinite(selected.indices?.[metric])) {
        const usable = rows.find((row) => Number.isFinite(row.indices?.[metric]));
        if (usable) {
          selected = usable;
          drawMap();
        }
      }
      $('status').textContent = summary();
      return true;
    }
  } catch {
    if (!signal.aborted)
      $('status').textContent = 'Archive request failed. Retry the search or load this page again.';
    return false;
  } finally {
    if (!signal.aborted) {
      busy = false;
      renderHistory();
    }
  }
}

function search() {
  session?.abort();
  session = new AbortController();
  next = null;
  rows = [];
  selected = null;
  busy = false;
  fullInterval = false;
  manualSelection = false;
  drawMap();
  renderHistory();
  if ($('start').value > $('end').value || !$('archive-form').reportValidity()) {
    $('status').textContent = 'Choose a start date on or before the end date.';
    return;
  }
  page(archiveUrl(point, $('source').value, $('start').value, $('end').value));
}
$('archive-form').onsubmit = (event) => {
  event.preventDefault();
  search();
};
$('more').onclick = () => {
  if (next && !busy) page(next);
};
$('all-pages').onclick = async () => {
  if (fullInterval) {
    fullInterval = false;
    renderHistory();
    return;
  }
  const { signal } = session;
  fullInterval = true;
  while (fullInterval && next && !signal.aborted) {
    if (!(await page(next))) break;
    if (!signal.aborted && rows.slice(-12).every((row) => row.state === 'error')) {
      $('status').textContent =
        summary() + '. Loading paused because the imagery service is unavailable.';
      break;
    }
  }
  if (!signal.aborted) {
    fullInterval = false;
    renderHistory();
  }
};
$('retry').onclick = async () => {
  const { signal } = session;
  busy = true;
  const failed = rows.filter((row) => row.state === 'error');
  failed.forEach((row) => {
    row.state = 'pending';
  });
  $('status').textContent = 'Retrying unavailable readings…';
  renderHistory();
  await sample(failed, signal);
  if (!signal.aborted) {
    busy = false;
    $('status').textContent = summary();
    renderHistory();
  }
};
$('map-layer').onchange = drawMap;
$('metric').onchange = () => {
  renderHistory();
  drawMap();
  if (!busy && rows.length) $('status').textContent = summary();
};
new ResizeObserver(() => plot($('chart'), rows, $('metric').value)).observe($('chart'));
updatePoint();
search();
