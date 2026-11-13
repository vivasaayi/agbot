import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const source = await readFile(new URL('../../web/js/api.js', import.meta.url), 'utf8');
const api = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));

test('workspace API requests carry the signed-in portal session', async () => {
  globalThis.window = {localStorage: {getItem: () => 'session'}};
  let headers;
  globalThis.fetch = async (_url, options) => { headers = options.headers; return new Response('{}', {headers: {'content-type': 'application/json'}}); };
  await api.apiGet('/api/farms');
  assert.equal(headers.Authorization, 'Bearer session');
  await assert.rejects(() => api.apiGet('https://other.example/api/farms'), /local API path/);
  await assert.rejects(() => api.apiGet('//other.example/api/farms'), /local API path/);
});

test('workspace tile requests use the session and reject failed responses', async () => {
  globalThis.window = {localStorage: {getItem: () => 'session'}};
  let headers;
  globalThis.fetch = async (_url, options) => { headers = options.headers; return new Response('tile'); };
  assert.equal((await api.apiTileBlob('/api/scenes/s/products/ndvi/tiles/0/0/0.png')).size, 4);
  assert.equal(headers.Authorization, 'Bearer session');
  globalThis.fetch = async () => new Response('', {status: 401});
  await assert.rejects(() => api.apiTileBlob('/api/scenes/s/products/ndvi/tiles/0/0/0.png'), /401/);
});

test('workspace map loads product tiles through the authenticated fetch path', async () => {
  const dataModule = text => 'data:text/javascript;base64,' + Buffer.from(text).toString('base64');
  const mapSource = await readFile(new URL('../../web/js/map.js', import.meta.url), 'utf8');
  const mapModule = await import(dataModule(mapSource.replace('"./api.js"', JSON.stringify(dataModule(source)))));
  let layer;
  let rectangles = 0, fittedBounds;
  const map = {fitBounds: bounds => { fittedBounds = bounds; }, removeLayer() {}};
  class GridLayer {
    constructor(options) { this.options = options; layer = this; }
    addTo() { return this; }
    static extend(methods) { return class extends GridLayer { createTile(...args) { return methods.createTile(...args); } }; }
  }
  globalThis.window = {localStorage:{getItem:()=> 'session'}, L:{GridLayer, map:()=>map, rectangle:()=>{ rectangles++; return {addTo(){}}; }}};
  globalThis.document = {createElement:()=>({set src(_url) { this.onload(); }})};
  let fetched;
  globalThis.fetch = async (url, options) => { fetched={url, options}; return new Response('tile'); };
  mapModule.initMap({});
  assert.equal(rectangles, 0, 'initial map must not add an opaque layer over imagery');
  mapModule.addProductLayer('scene', 'ndvi');
  await new Promise((resolve, reject) => layer.createTile({z:2,x:3,y:1}, error => error ? reject(error) : resolve()));
  assert.equal(fetched.url, '/api/scenes/scene/products/ndvi/tiles/2/3/1.png');
  assert.equal(fetched.options.headers.Authorization, 'Bearer session');
  mapModule.addCatalogProductLayer('product:one');
  await new Promise((resolve, reject) => layer.createTile({z:1,x:0,y:1}, error => error ? reject(error) : resolve()));
  assert.equal(fetched.url, '/api/catalog/products/product%3Aone/tiles/1/0/1.png');
  assert.equal(fetched.options.headers.Authorization, 'Bearer session');
  mapModule.focusCatalogProduct({crs:'EPSG:4326',bbox:[-96,40,-95,41]});
  assert.equal(rectangles, 1);
  assert.deepEqual(fittedBounds, [[40,-96],[41,-95]]);
});
