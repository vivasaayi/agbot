import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const source = await readFile(new URL('../../web/js/api.js', import.meta.url), 'utf8');
const { apiAuthorization } = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));

test('sessions are sent only to same-origin API URLs', () => {
  const origin = 'https://agbot.example';
  assert.equal(apiAuthorization('/api/stac/collections', origin, 'secret'), 'Bearer secret');
  assert.equal(apiAuthorization('https://agbot.example/api/tiles/1', origin, 'secret'), 'Bearer secret');
  assert.equal(apiAuthorization('https://tiles.example/api/tiles/1', origin, 'secret'), undefined);
  assert.equal(apiAuthorization('/workspace/', origin, 'secret'), undefined);
  assert.equal(apiAuthorization('/api/stac', origin, ''), undefined);
});
