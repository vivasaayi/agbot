// Uses an already populated disposable catalog; owns only its child server.
const assert = require('node:assert/strict');
const {spawn} = require('node:child_process');
const {randomBytes} = require('node:crypto');
const fs = require('node:fs/promises');
(async () => {
  assert.equal(process.env.AGBOT_TEST_DISPOSABLE, '1');
  const binary=process.env.AGBOT_HUB_BINARY, state=process.env.AGBOT_TEST_STATE;
  assert.ok(binary && state);
  const code=randomBytes(24).toString('hex');
  const playwright=require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  let child;
  async function start() {
    return await new Promise((resolve,reject) => {
      let logs='';
      child=spawn(binary,[],{env:{...process.env,
        GEO_HUB__DATABASE_URL:`sqlite://${state}/hub.db?mode=rwc`,
        GEO_HUB__DATA_ROOT:`${state}/hub-data`,GEO_HUB__BIND_ADDRESS:'127.0.0.1:0',
        GEO_HUB__BOOTSTRAP__ACCESS_CODE:code,GEO_HUB__BOOTSTRAP__SEED_DEMO:'false',
        GEO_HUB__SECURITY__REQUIRE_SESSION:'true',GEO_HUB__PIPELINE__ENABLED:'false'}});
      const timer=setTimeout(()=>reject(new Error('server startup timed out')),20000);
      child.on('error', e=>{clearTimeout(timer);reject(e)});
      child.on('exit',code=>{clearTimeout(timer);reject(new Error(`server exited ${code}`))});
      for(const stream of [child.stdout,child.stderr]) stream.on('data',data=>{
        logs+=data.toString();
        const match=logs.match(/geo_hub listening.*?127\.0\.0\.1:(\d+)/);
        if(match){clearTimeout(timer);resolve(`http://127.0.0.1:${match[1]}`)}
      });
    });
  }
  async function stop(){if(child && child.exitCode===null){const done=new Promise(r=>child.once('exit',r));child.kill('SIGTERM');await done;}}
  const browser=await playwright.chromium.launch({headless:true});
  let savedField;
  try {
    for(let round=0;round<2;round++){
      const base=await start();
      const context=await browser.newContext();
      const page=await context.newPage(); const errors=[];
      page.on('pageerror',e=>errors.push(e.message));
      await page.goto(`${base}/portal/`);
      await page.locator('#access-code').fill(code); await page.locator('#login-submit').click();
      await page.waitForURL('**/#/home');
      await page.goto(`${base}/workspace/`);
      const toggle=page.locator('#layers-panel .layer-row').filter({hasText:'sim_elevation'}).locator('input[type=checkbox]');
      await toggle.waitFor();
      const tile=page.waitForResponse(r=>r.url().includes('/api/catalog/products/') && r.url().includes('/tiles/') && r.status()===200);
      await toggle.check();
      const response=await tile;
      assert.match(response.request().headers().authorization,/^Bearer /);
      await page.locator('#map-primary .leaflet-tile-loaded').first().waitFor();
      await page.waitForFunction(() => [...document.querySelectorAll('#map-primary img.leaflet-tile-loaded')].some(img => {
        if (!img.complete || !img.naturalWidth) return false;
        const canvas=document.createElement('canvas');canvas.width=img.naturalWidth;canvas.height=img.naturalHeight;
        const context=canvas.getContext('2d');context.drawImage(img,0,0);
        const pixels=context.getImageData(0,0,canvas.width,canvas.height).data;
        for(let i=3;i<pixels.length;i+=4) if(pixels[i]>0)return true;
        return false;
      }));
      assert.deepEqual(errors,[]);
      await page.screenshot({path:`${state}/workspace-${round}.png`});
      if (round === 0) {
        const {promisify}=require('node:util');
        const {execFile}=require('node:child_process');
        const result=await promisify(execFile)('node', ['scripts/test-farmer-browser.cjs'], {
          env:{...process.env,AGBOT_TEST_URL:base,AGBOT_TEST_ACCESS_CODE:code},
        });
        savedField=result.stdout.match(/"field_id":\s*"([^"]+)"/)[1];
        console.log('workspace: farmer onboarding and scouting browser checks passed');
      } else {
        await page.goto(`${base}/portal/#/field/${savedField}`);
        await page.locator('#fd-activity-list').getByText(/Browser smoke scouting observation/).waitFor();
        console.log('workspace: field and scouting note persisted across server restart');
      }
      await context.close(); await stop();
      console.log(`workspace: real catalog raster displayed ${round ? 'after restart' : 'before restart'}`);
    }
  } finally {await stop();await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
