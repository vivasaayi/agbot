// Browser smoke for authenticated workspace and browse; it creates and revokes
// a session but does not create or change farm data.
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');

async function main() {
  const base = process.env.AGBOT_TEST_URL;
  const code = process.env.AGBOT_TEST_ACCESS_CODE;
  if ((process.env.AGBOT_TEST_DISPOSABLE !== '1' && process.env.AGBOT_TEST_READ_ONLY !== '1') || !base || !code) {
    throw new Error('Set AGBOT_TEST_READ_ONLY=1 (or AGBOT_TEST_DISPOSABLE=1), AGBOT_TEST_URL and AGBOT_TEST_ACCESS_CODE.');
  }
  const origin = new URL(base);
  assert.ok(['http:', 'https:'].includes(origin.protocol));
  assert.equal(origin.username + origin.password, '');
  const moduleName = process.env.PLAYWRIGHT_MODULE || 'playwright';
  const {chromium} = moduleName.endsWith('.mjs')
    ? await import(pathToFileURL(path.resolve(moduleName)).href) : require(moduleName);
  const args = process.env.AGBOT_TEST_SOFTWARE_WEBGL === '1'
    ? ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] : [];
  const browser = await chromium.launch({headless:true, args});
  let page, token;
  try {
    page = await browser.newPage();
    const errors = [], externalLeaks = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('request', request => {
      if (new URL(request.url()).origin !== origin.origin && request.headers().authorization) externalLeaks.push(request.url());
    });
    await page.goto(new URL('/portal/', origin).href);
    await page.locator('#access-code').fill(code);
    const login = page.waitForResponse(r => r.url() === new URL('/api/portal/login', origin).href && r.request().method() === 'POST');
    await page.locator('#login-submit').click();
    const loginResponse = await login;
    assert.equal(loginResponse.status(), 200, 'portal login');
    token = (await loginResponse.json()).token;
    assert.ok(token, 'portal session');
    await page.waitForURL('**/#/home');
    let next = page.waitForResponse(response => response.url().includes('/api/farms') && response.request().method() === 'GET', {timeout:20000});
    await page.goto(new URL('/workspace/', origin).href);
    let response = await next;
    assert.equal(response.status(), 200, 'workspace farms request');
    assert.ok(response.request().headers().authorization?.startsWith('Bearer '));
    if (process.env.AGBOT_TEST_TILE_SCENE) {
      next = page.waitForResponse(r => r.url().includes('/products/ndvi/tiles/') && r.status() === 200, {timeout:20000});
      await page.evaluate(async scene => {
        const map = await import('/workspace/js/map.js');
        map.addProductLayer(scene, 'ndvi');
      }, process.env.AGBOT_TEST_TILE_SCENE);
      response = await next;
      assert.ok(response.request().headers().authorization?.startsWith('Bearer '), 'workspace tile carries session');
    }
    next = page.waitForResponse(response => response.url().includes('/api/stac/collections'), {timeout:30000});
    await page.goto(new URL('/browse', origin).href);
    response = await next;
    assert.equal(response.status(), 200, 'browse collections request');
    assert.ok(response.request().headers().authorization?.startsWith('Bearer '));
    assert.deepEqual(externalLeaks, [], 'external requests must not receive the session');
    assert.deepEqual(errors, [], 'no uncaught browser exceptions');
    console.log(`Signed-in workspace and browse requests${process.env.AGBOT_TEST_TILE_SCENE ? ' and workspace tile' : ''} passed without leaking the session.`);
  } finally {
    try {
      if (page && token) {
        const result = await page.request.post(new URL('/api/portal/logout', origin).href, {
          headers: {Authorization: `Bearer ${token}`}, data: {}, maxRedirects: 0,
        });
        assert.ok(result.ok(), 'temporary portal session revoked');
      }
    } finally {
      await browser.close();
    }
  }
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
