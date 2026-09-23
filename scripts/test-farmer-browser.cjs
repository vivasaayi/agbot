#!/usr/bin/env node
/**
 * Real-browser farmer onboarding smoke for a DISPOSABLE AGBot server.
 * Creates one small Iowa test field, a farm, a monitoring subscription,
 * an imagery discovery job, and a scouting note. Leaves them for inspection.
 * Never run against a real farm installation. No domain/network mocks.
 *
 * Usage (install Playwright and its Chromium browser first):
 *   AGBOT_TEST_DISPOSABLE=1 AGBOT_TEST_URL=http://127.0.0.1:18080 \
 *   AGBOT_TEST_ACCESS_CODE=your-test-code node scripts/test-farmer-browser.cjs
 *
 * Optional PLAYWRIGHT_MODULE points to an installed Playwright package;
 * default: playwright. AGBOT_TEST_SCREENSHOT saves a full-page screenshot.
 * Tests drawing, GeoJSON import validation, existing farm selector, cancel,
 * mobile layout, persisted scouting note, and rejected/revoked sessions.
 */
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');

async function main() {
  const base = process.env.AGBOT_TEST_URL;
  const code = process.env.AGBOT_TEST_ACCESS_CODE;
  if (process.env.AGBOT_TEST_DISPOSABLE !== '1' || !base || !code) {
    throw new Error('Refusing mutations: set AGBOT_TEST_DISPOSABLE=1, AGBOT_TEST_URL and AGBOT_TEST_ACCESS_CODE for a disposable server.');
  }
  const url = new URL(base);
  assert.ok(['http:', 'https:'].includes(url.protocol), 'test URL must be HTTP(S)');
  assert.equal(url.username + url.password, '', 'do not embed credentials in URL');
  const moduleName = process.env.PLAYWRIGHT_MODULE || 'playwright';
  const playwright = moduleName.endsWith('.mjs')
    ? await import(pathToFileURL(path.resolve(moduleName)).href)
    : require(moduleName);
  const browser = await playwright.chromium.launch({ headless: true });
  let fieldId;
  try {
    const context = await browser.newContext({
      viewport: { width: 390, height: 844 },
      geolocation: { longitude: -93.68, latitude: 42.03 },
      permissions: ['geolocation'],
    });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const portal = new URL('/portal/', url).href;
    const runId = new Date().toISOString();
    await page.goto(portal);
    await page.locator('#access-code').fill(code);
    await page.locator('#login-submit').click();
    await page.getByRole('link', { name: 'Add field', exact: true }).click();
    await page.getByRole('link', { name: 'Cancel', exact: true }).click();
    await page.waitForURL('**/#/home');
    await page.getByRole('link', { name: 'Add field', exact: true }).click();
    await page.locator('#farm-name').fill(`Browser test farm ${runId}`);
    await page.locator('#field-name').fill(`Browser test field ${runId}`);
    await page.locator('#field-crop').fill('corn');
    await page.locator('#field-season').fill(String(new Date().getFullYear()));

    // Exercise the actual file input, including rejection of holes.
    const ring = [[-93.68,42.03],[-93.679,42.03],[-93.679,42.031],[-93.68,42.03]];
    const upload = geometry => page.locator('#boundary-file').setInputFiles({
      name: 'test-field.geojson', mimeType: 'application/geo+json',
      buffer: Buffer.from(JSON.stringify(geometry)),
    });
    await upload({type:'Polygon', coordinates:[ring,ring]});
    await page.getByText('Choose a single Polygon without holes, using longitude and latitude.', {exact:true}).waitFor();
    await upload({type:'Polygon', coordinates:[ring]});
    await page.getByText('3 boundary corners selected. Check that the outline matches your field.', {exact:true}).waitFor();
    await page.locator('#clear-boundary').click();

    // Geolocation centers the real map. Zoom in four levels even if location
    // arrives late, then use a small screen polygon to cap field size.
    await page.locator('#locate-field').click();
    await page.waitForTimeout(1500);
    for (let i=0; i<3; i++) await page.locator('#setup-map .leaflet-control-zoom-in').click();
    await page.waitForTimeout(500);
    await page.locator('#draw-field').click();
    const map = page.locator('#setup-map');
    await map.scrollIntoViewIfNeeded();
    for (const [x,y] of [[140,130],[200,130],[200,190],[140,190]]) {
      await map.click({position:{x,y}});
    }
    await page.locator('#draw-field').click();
    assert.match(await page.locator('#boundary-summary').innerText(), /^4 boundary corners/);
    const widths = await page.evaluate(() => [innerWidth,document.documentElement.scrollWidth]);
    assert.equal(widths[1],widths[0],'mobile page must not overflow horizontally');
    if (process.env.AGBOT_TEST_SCREENSHOT) await page.screenshot({path:process.env.AGBOT_TEST_SCREENSHOT,fullPage:true});
    const savedResponse = page.waitForResponse(response => response.url().endsWith('/api/portal/fields') && response.request().method()==='POST');
    await page.locator('#save-field').click();
    const response = await savedResponse;
    assert.equal(response.status(),200,await response.text());
    const saved = await response.json();
    fieldId = saved.field.field_id;
    assert.ok(saved.field.area_ha > 0 && saved.field.area_ha < 4,'test field must remain below four hectares');
    assert.ok(['queued','worker_disabled'].includes(saved.monitoring.status));
    await page.getByRole('link',{name:'Open your field',exact:true}).click();
    await page.getByText('Add activity',{exact:true}).click();
    const form = page.locator('#fd-add-activity-body');
    const note = `Browser smoke scouting observation ${runId}`;
    await form.locator('.af-type').selectOption('scouting');
    await form.locator('.af-note').fill(note);
    await form.getByRole('button',{name:'Save activity',exact:true}).click();
    await page.locator('#fd-activity-list').getByText(note,{exact:true}).waitFor();
    await page.reload();
    await page.locator('#fd-activity-list').getByText(note,{exact:true}).waitFor();

    // Rapid refresh used to initialize Leaflet twice and throw an uncaught
    // "Map container is already initialized" error.
    await page.locator('#fd-refresh').dblclick();
    await page.waitForFunction(() => !document.querySelector('#fd-refresh').disabled);
    await page.locator('#fd-map .leaflet-pane').first().waitFor({state:'attached'});

    // Reusing a farm is offered without creating a second field.
    await page.goto(`${portal}#/add-field`);
    await page.locator(`#farm-choice option[value="${saved.field.farm_id}"]`).waitFor({state:'attached'});
    await page.locator('#farm-choice').selectOption(saved.field.farm_id);
    assert.ok(await page.locator('#farm-name').isHidden());
    await page.getByRole('link',{name:'Cancel',exact:true}).click();

    // Actual server revocation, then restoring the stale token verifies the
    // same 401 flow used for expired sessions without altering server clocks.
    const token = await page.evaluate(() => localStorage.getItem('agbot_portal_token'));
    await page.locator('#logout-button').click();
    await page.locator('#access-code').waitFor();
    await page.evaluate(stale => localStorage.setItem('agbot_portal_token',stale),token);
    await page.goto(`${portal}#/add-field`);
    await page.waitForURL('**/#/login');
    assert.equal(await page.evaluate(() => localStorage.getItem('agbot_portal_token')),null);
    assert.deepEqual(errors,[],'no uncaught browser exceptions');
    console.log(JSON.stringify({status:'passed',field_id:fieldId,tests:['login','cancel','GeoJSON import and holes rejection','map drawing','mobile layout','atomic field setup','scouting persistence after reload','existing farm selection','revoked session redirect']},null,2));
  } finally {
    if (fieldId) console.log(`Disposable test field retained: ${fieldId}`);
    await browser.close();
  }
}
main().catch(error => { console.error(error.message); process.exitCode=1; });
