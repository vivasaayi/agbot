// Scatter by acquisition time. The table exposes the same samples to assistive technology.
export function plot(container, rows, metric) {
  const points = rows.filter((row) => Number.isFinite(row.indices?.[metric]));
  container.replaceChildren();
  if (!points.length) {
    const message = document.createElement('div');
    message.className = 'chart-empty';
    message.textContent = 'No usable readings to plot for the loaded scenes.';
    container.append(message);
    return;
  }
  const width = Math.max(300, container.clientWidth);
  const height = 150,
    left = 45,
    right = width - 10;
  const times = rows.map((row) => Date.parse(row.scene.properties.datetime));
  const first = Math.min(...times),
    last = Math.max(...times);
  const values = points.map((row) => row.indices[metric]);
  const low = Math.min(-1, ...values),
    high = Math.max(1, ...values);
  const x = (t) =>
    first === last ? (left + right) / 2 : left + ((t - first) / (last - first)) * (right - left);
  const y = (value) => 11 + ((high - value) / (high - low)) * 109;
  const svg = el('svg', {
    viewBox: '0 0 ' + width + ' ' + height,
    role: 'img',
    'aria-label':
      points.length + ' dated ' + metric.toUpperCase() + ' readings. Values are listed in the table.',
  });
  function el(tag, attrs, text) {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
    if (text != null) node.textContent = text;
    return node;
  }
  function label(px, py, anchor, text) {
    svg.append(
      el('text', { x: px, y: py, 'text-anchor': anchor, 'font-size': 11, fill: '#52665c' }, text),
    );
  }
  for (const value of [low, 0, high]) {
    svg.append(el('line', { x1: left, x2: right, y1: y(value), y2: y(value), stroke: '#d5ded4' }));
    label(left - 6, y(value) + 4, 'end', String(Number(value.toFixed(2))));
  }
  label(left, 144, 'start', new Date(first).toISOString().slice(0, 10));
  if (first !== last) label(right, 144, 'end', new Date(last).toISOString().slice(0, 10));
  for (const { scene, indices } of points) {
    const circle = el('circle', {
      cx: x(Date.parse(scene.properties.datetime)),
      cy: y(indices[metric]),
      r: 4,
      fill: scene.collection === 'landsat-c2-l2' ? '#9f5121' : '#116656',
      stroke: 'white',
      'stroke-width': 1,
    });
    circle.append(el('title', {}, scene.id + ' · ' + indices[metric].toFixed(3)));
    svg.append(circle);
  }
  container.append(svg);
}
