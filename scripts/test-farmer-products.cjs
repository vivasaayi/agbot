// Invoked by the Landsat acceptance fixture against its disposable live router.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');

(async () => {
  const {AGBOT_TEST_URL: base, AGBOT_TEST_TOKEN: token, AGBOT_TEST_FIELD_ID: field} = process.env;
  assert.equal(process.env.AGBOT_TEST_DISPOSABLE, '1');
  assert.ok(base && token && field, 'disposable fixture URL, session and field required');
  assert.ok(['127.0.0.1', 'localhost'].includes(new URL(base).hostname));
  const playwright = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await playwright.chromium.launch({headless:true});
  try {
    for (const width of [1280, 390]) {
      const context = await browser.newContext({viewport:{width,height:844},acceptDownloads:true});
      await context.addInitScript(value => localStorage.setItem('agbot_portal_token', value), token);
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', e => errors.push(e.message));
      const tileResponse = page.waitForResponse(r => /\/api\/catalog\/products\/.*\/tiles\//.test(r.url()) && r.status() === 200);
      await page.goto(`${base}/portal/#/field/${encodeURIComponent(field)}`);
      const tile = await tileResponse;
      assert.match(tile.request().headers().authorization, /^Bearer /);
      assert.equal((await tile.body()).subarray(1,4).toString(), 'PNG');
      await page.locator('#fd-map img.leaflet-tile-loaded').first().waitFor();
      await page.locator('#fd-findings').waitFor();
      await page.locator('#fd-findings .finding-date').first().waitFor();
      await page.locator('#fd-all-history').click();
      await page.locator('#fd-chart svg').waitFor();
      await page.locator('#fd-generate-report').click();
      await page.locator('#fd-report-status a').waitFor();
      await page.locator('#fd-report-status a').click();
      const downloadEvent = page.waitForEvent('download');
      await page.locator('.report-row').first().click();
      const download = await downloadEvent;
      const bytes = await fs.readFile(await download.path());
      assert.equal(bytes.subarray(0,4).toString(), '%PDF');
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'page overflows viewport');
      assert.deepEqual(errors, []);
      await context.close();
      console.log(`farmer-products: imagery and PDF verified at width ${width}`);
    }
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
