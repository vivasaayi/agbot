import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
const root = new URL('../../web/portal/js/', import.meta.url);
const dataModule = text => 'data:text/javascript;base64,' + Buffer.from(text).toString('base64');
const auth = dataModule(await readFile(new URL('auth.js', root), 'utf8'));
const api = await import(dataModule((await readFile(new URL('api.js', root), 'utf8')).replace('"./auth.js"', JSON.stringify(auth))));
test('protected tile, report, and availability requests carry the session', async () => {
  globalThis.window = {localStorage: {getItem: () => 'session'}, location:{hash:'#/field/test'}};
  const requests=[];
  globalThis.fetch = async (url, options) => { requests.push({url,options}); return new Response('bytes',{status:200}); };
  assert.equal((await api.catalogTileBlob('product:one',{z:12,x:1,y:2})).size,5);
  assert.equal((await api.downloadReport('report:one')).size,5);
  assert.equal(await api.sceneProductAvailable('scene','ndvi'), true);
  assert.match(requests[0].url,/product%3Aone\/tiles\/12\/1\/2.png$/);
  for (const {options} of requests) assert.equal(options.headers.Authorization,'Bearer session');
});
test('binary downloads clear expired sessions and navigate to login', async () => {
  let cleared=0;
  globalThis.window={localStorage:{getItem:()=> 'expired',removeItem:()=>cleared++},location:{hash:'#/reports'}};
  globalThis.fetch=async()=>new Response('',{status:401});
  await assert.rejects(()=>api.downloadReport('report'), /session|unauthorized/i);
  assert.equal(window.location.hash,'#/login');
  assert.equal(cleared,2);
});
