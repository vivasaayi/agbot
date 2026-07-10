// Compact inline-SVG time-series chart for the field detail view (F-B7).
//
// Renders the harmonized merged series as a line, raw per-source
// observations as dots, and farm activities as vertical ticks along the
// bottom edge. No dependencies: everything is created via the SVG DOM so
// user-supplied text (activity notes) can only land in text nodes.

const SVG_NS = "http://www.w3.org/2000/svg";

const WIDTH = 640;
const HEIGHT = 240;
const MARGIN = { top: 12, right: 12, bottom: 34, left: 44 };

/** Deterministic source -> CSS class mapping with a bounded palette. */
const SOURCE_CLASSES = ["src-a", "src-b", "src-c", "src-d", "src-e"];

const ACTIVITY_GLYPHS = {
  planting: "P",
  irrigation: "I",
  spraying: "S",
  fertilizing: "F",
  scouting: "👁",
  harvest: "H",
  tillage: "T",
  other: "•",
};

function el(name, attrs = {}) {
  const node = document.createElementNS(SVG_NS, name);
  for (const [key, value] of Object.entries(attrs)) {
    node.setAttribute(key, String(value));
  }
  return node;
}

function parseTime(iso) {
  const ms = Date.parse(iso);
  return Number.isNaN(ms) ? null : ms;
}

function formatTick(ms, spanMs) {
  const date = new Date(ms);
  if (spanMs > 400 * 24 * 3600 * 1000) {
    // Multi-year range: month + year keeps ticks unambiguous.
    return date.toLocaleDateString(undefined, { month: "short", year: "numeric" });
  }
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/**
 * Render the chart into `container` (cleared first).
 *
 * `series` is the /timeseries response ({per_source, merged}); `activities`
 * is a list of activity records ({activity_type, occurred_at, note}).
 * `options.emptyMessage` is shown when there are no plottable points.
 */
export function renderTimeseriesChart(container, series, activities = [], options = {}) {
  container.innerHTML = "";

  const perSource = (series && series.per_source) || {};
  const merged = ((series && series.merged) || [])
    .map((point) => ({ ...point, ms: parseTime(point.t) }))
    .filter((point) => point.ms !== null && Number.isFinite(point.value))
    .sort((a, b) => a.ms - b.ms);

  const sourceNames = Object.keys(perSource).sort();
  const sourcePoints = sourceNames.map((source, index) => ({
    source,
    className: SOURCE_CLASSES[index % SOURCE_CLASSES.length],
    points: (perSource[source] || [])
      .map((point) => ({ ...point, ms: parseTime(point.t) }))
      .filter((point) => point.ms !== null && Number.isFinite(point.value)),
  }));

  const allPoints = merged.concat(...sourcePoints.map((entry) => entry.points));
  if (allPoints.length === 0) {
    const empty = document.createElement("p");
    empty.className = "muted chart-empty";
    empty.textContent =
      options.emptyMessage ||
      "No satellite history yet — subscribe or backfill from the workspace.";
    container.appendChild(empty);
    return;
  }

  // --- Scales ---------------------------------------------------------------
  let minMs = Math.min(...allPoints.map((p) => p.ms));
  let maxMs = Math.max(...allPoints.map((p) => p.ms));
  if (options.startMs !== undefined) minMs = Math.min(minMs, options.startMs);
  if (options.endMs !== undefined) maxMs = Math.max(maxMs, options.endMs);
  if (maxMs === minMs) {
    // Single-observation series: pad a week each side so the dot is visible.
    minMs -= 7 * 24 * 3600 * 1000;
    maxMs += 7 * 24 * 3600 * 1000;
  }

  let minValue = Math.min(...allPoints.map((p) => p.value));
  let maxValue = Math.max(...allPoints.map((p) => p.value));
  const pad = (maxValue - minValue || Math.abs(maxValue) || 1) * 0.1;
  minValue -= pad;
  maxValue += pad;

  const plotWidth = WIDTH - MARGIN.left - MARGIN.right;
  const plotHeight = HEIGHT - MARGIN.top - MARGIN.bottom;
  const x = (ms) => MARGIN.left + ((ms - minMs) / (maxMs - minMs)) * plotWidth;
  const y = (value) =>
    MARGIN.top + (1 - (value - minValue) / (maxValue - minValue)) * plotHeight;

  const svg = el("svg", {
    viewBox: `0 0 ${WIDTH} ${HEIGHT}`,
    class: "ts-chart",
    role: "img",
  });
  svg.setAttribute("aria-label", options.label || "Field time series");

  // --- Axes -----------------------------------------------------------------
  const axis = el("g", { class: "ts-axis" });
  const bottom = MARGIN.top + plotHeight;
  axis.appendChild(
    el("line", { x1: MARGIN.left, y1: bottom, x2: WIDTH - MARGIN.right, y2: bottom }),
  );
  axis.appendChild(
    el("line", { x1: MARGIN.left, y1: MARGIN.top, x2: MARGIN.left, y2: bottom }),
  );

  for (const fraction of [0, 0.5, 1]) {
    const value = minValue + (maxValue - minValue) * fraction;
    const ty = y(value);
    axis.appendChild(
      el("line", {
        x1: MARGIN.left,
        y1: ty,
        x2: WIDTH - MARGIN.right,
        y2: ty,
        class: "ts-grid",
      }),
    );
    const label = el("text", { x: MARGIN.left - 6, y: ty + 4, class: "ts-tick-label ts-tick-y" });
    label.textContent = value.toFixed(2);
    axis.appendChild(label);
  }

  const spanMs = maxMs - minMs;
  for (const fraction of [0, 0.5, 1]) {
    const ms = minMs + spanMs * fraction;
    const tx = x(ms);
    const label = el("text", {
      x: tx,
      y: bottom + 16,
      class: "ts-tick-label ts-tick-x",
    });
    label.textContent = formatTick(ms, spanMs);
    axis.appendChild(label);
  }
  svg.appendChild(axis);

  // --- Merged line ------------------------------------------------------------
  if (merged.length > 1) {
    const points = merged.map((p) => `${x(p.ms).toFixed(1)},${y(p.value).toFixed(1)}`);
    svg.appendChild(el("polyline", { points: points.join(" "), class: "ts-merged-line" }));
  }

  // --- Per-source dots ----------------------------------------------------------
  for (const { source, className, points } of sourcePoints) {
    const group = el("g", { class: `ts-source ${className}` });
    for (const point of points) {
      const dot = el("circle", { cx: x(point.ms), cy: y(point.value), r: 3 });
      const title = el("title");
      title.textContent = `${source} · ${point.t} · ${point.value.toFixed(3)}`;
      dot.appendChild(title);
      group.appendChild(dot);
    }
    svg.appendChild(group);
  }

  // --- Activity markers ----------------------------------------------------------
  const markerGroup = el("g", { class: "ts-activities" });
  for (const activity of activities) {
    const ms = parseTime(activity.occurred_at);
    if (ms === null || ms < minMs || ms > maxMs) {
      continue;
    }
    const tx = x(ms);
    const marker = el("g", { class: "ts-activity" });
    marker.appendChild(el("line", { x1: tx, y1: bottom - 12, x2: tx, y2: bottom }));
    const glyph = el("text", { x: tx, y: bottom - 15, class: "ts-activity-glyph" });
    glyph.textContent = ACTIVITY_GLYPHS[activity.activity_type] || "•";
    marker.appendChild(glyph);
    const title = el("title");
    const when = new Date(ms).toLocaleDateString();
    title.textContent = activity.note
      ? `${activity.activity_type} · ${when} · ${activity.note}`
      : `${activity.activity_type} · ${when}`;
    marker.appendChild(title);
    markerGroup.appendChild(marker);
  }
  svg.appendChild(markerGroup);

  container.appendChild(svg);

  // --- Legend (plain DOM, below the SVG) ---------------------------------------
  if (sourcePoints.length > 0) {
    const legend = document.createElement("div");
    legend.className = "ts-legend";
    if (merged.length > 1) {
      const item = document.createElement("span");
      item.className = "ts-legend-item ts-legend-merged";
      item.textContent = "merged";
      legend.appendChild(item);
    }
    for (const { source, className } of sourcePoints) {
      const item = document.createElement("span");
      item.className = `ts-legend-item ${className}`;
      item.textContent = source;
      legend.appendChild(item);
    }
    container.appendChild(legend);
  }
}
