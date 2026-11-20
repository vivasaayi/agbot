const origin = 'https://planetarycomputer.microsoft.com';
const search = origin + '/api/stac/v1/search';

export function parseCoordinates(input) {
  const parts = input.trim().split(',');
  if (parts.length !== 2 || parts.some((value) => !value.trim())) return null;
  const [lat, lon] = parts.map(Number);
  return Number.isFinite(lat) && Math.abs(lat) <= 90 && Number.isFinite(lon) && Math.abs(lon) <= 180
    ? { lat, lon }
    : null;
}

export function archiveUrl({ lat, lon }, source, start, end) {
  const url = new URL(search);
  url.search = new URLSearchParams({
    collections: source === 'both' ? 'landsat-c2-l2,sentinel-2-l2a' : source,
    intersects: JSON.stringify({ type: 'Point', coordinates: [lon, lat] }),
    datetime: start + 'T00:00:00Z/' + end + 'T23:59:59Z',
    sortby: '-datetime',
    limit: '12',
  });
  return url;
}

export function nextPage(links) {
  const href = links?.find(
    (link) => link.rel === 'next' && (!link.method || link.method === 'GET'),
  )?.href;
  if (!href) return null;
  const url = new URL(href, search);
  return url.origin + url.pathname === search ? url.href : null;
}

export function mapTileUrl(scene, layer) {
  const sentinel = scene.collection === 'sentinel-2-l2a';
  const params = new URLSearchParams({ collection: scene.collection, item: scene.id });
  if (layer === 'rgb') {
    if (sentinel) {
      params.set('assets', 'visual');
      params.set('asset_bidx', 'visual|1,2,3');
    } else {
      for (const asset of ['red', 'green', 'blue']) params.append('assets', asset);
      params.set('color_formula', 'gamma RGB 2.7, saturation 1.5, sigmoidal RGB 15 0.55');
    }
  } else {
    const rawBaseline = scene.properties['s2:processing_baseline'];
    if (sentinel && (!rawBaseline || !Number.isFinite(Number(rawBaseline)))) return null;
    const [green, nir, red, swir] = sentinel
      ? ['B03', 'B08', 'B04', 'B11']
      : ['green', 'nir08', 'red', 'swir16'];
    const [a, b] = { ndwi: [green, nir], mndwi: [green, swir], ndvi: [nir, red], ndmi: [nir, swir] }[
      layer
    ];
    const scale = sentinel ? 0.0001 : 0.0000275;
    const offset = sentinel ? (Number(rawBaseline) >= 4 ? 0.1 : 0) : 0.2;
    const band = (name) => '(' + name + '*' + scale + '-' + offset + ')';
    params.set('asset_as_band', 'true');
    params.set('expression', '(' + band(a) + '-' + band(b) + ')/(' + band(a) + '+' + band(b) + ')');
    params.set('rescale', '-1,1');
    params.set('colormap_name', layer === 'ndvi' ? 'rdylgn' : 'blues');
  }
  return origin + '/api/data/v1/item/tiles/WebMercatorQuad/{z}/{x}/{y}.png?' + params;
}

export const platform = (scene) => scene.properties.platform ?? scene.collection;
export function tileId(scene) {
  const p = scene.properties;
  if (p['s2:mgrs_tile']) return 'MGRS ' + p['s2:mgrs_tile'];
  if (p['landsat:wrs_path'] && p['landsat:wrs_row'])
    return 'WRS ' + p['landsat:wrs_path'] + '/' + p['landsat:wrs_row'];
  return '—';
}
