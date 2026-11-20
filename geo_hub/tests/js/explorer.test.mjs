import test from 'node:test';
import assert from 'node:assert/strict';
import { archiveUrl, parseCoordinates, mapTileUrl, nextPage } from '../../web/explorer/archive.mjs';

test('point search uses longitude first and includes both complete UTC dates', () => {
  const point = parseCoordinates('13.00815, 80.05538');
  const params = archiveUrl(point, 'both', '1993-01-01', '2024-01-01').searchParams;
  assert.deepEqual(JSON.parse(params.get('intersects')).coordinates, [80.05538, 13.00815]);
  assert.equal(params.get('datetime'), '1993-01-01T00:00:00Z/2024-01-01T23:59:59Z');
  assert.equal(params.get('collections'), 'landsat-c2-l2,sentinel-2-l2a');
  for (const value of ['13,', ', 80', '91, 0', '0, 181', 'Village'])
    assert.equal(parseCoordinates(value), null);
});

test('index tiles use calibrated floating point bands and the correct Sentinel baseline', () => {
  const scene = {
    collection: 'sentinel-2-l2a',
    id: 'example',
    properties: { 's2:processing_baseline': '05.10' },
  };
  const url = mapTileUrl(scene, 'ndwi');
  assert.ok(url.includes('/{z}/{x}/{y}.png'));
  const expression = new URL(url).searchParams.get('expression');
  assert.match(expression, /B03/);
  assert.match(expression, /B08/);
  assert.match(expression, /0\.1/);
  assert.equal(mapTileUrl({ ...scene, properties: {} }, 'ndwi'), null);
  assert.ok(mapTileUrl({ ...scene, properties: {} }, 'rgb'));
});

test('pagination accepts only the archive search endpoint', () => {
  assert.equal(nextPage([{ rel: 'next', href: 'https://example.com/' }]), null);
  assert.equal(
    nextPage([
      { rel: 'next', href: 'https://planetarycomputer.microsoft.com/api/stac/v1/search?token=next' },
    ]),
    'https://planetarycomputer.microsoft.com/api/stac/v1/search?token=next',
  );
});
