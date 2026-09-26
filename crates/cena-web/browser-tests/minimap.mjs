// Real native HTTP/CSP + synthetic authenticated session feed. No game login.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {readFile, mkdir} from 'node:fs/promises';
import {resolve} from 'node:path';

const {chromium} = createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE || 'playwright');
const server = spawn(process.env.ATLAS_PREVIEW_BIN || resolve('target/debug/examples/atlas_preview'), [], {stdio: ['ignore', 'pipe', 'inherit']});
const fixture = JSON.parse(await readFile(new URL('../../cena-ui/tests/fixtures/snapshot-v1.json', import.meta.url)));
let browser;
try {
  const base = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(Error('Preview timeout')), 15000);
    server.once('error', reject);
    server.stdout.on('data', chunk => {
      const match = String(chunk).match(/http:\/\/127\.0\.0\.1:\d+/);
      if (match) { clearTimeout(timer); resolve(match[0]); }
    });
  });
  const manifest = await (await fetch(base + '/atlas/corpus/manifest.json')).json();
  const ready = structuredClone(fixture);
  ready.view.lifecycle = {kind: 'ready'};
  ready.view.map_location = {map_sha256: manifest.source_map_sha256, room: 228};
  const context = () => browser.newContext({viewport: {width: 1500, height: 1000}});
  browser = await chromium.launch({headless: true, executablePath: process.env.BROWSER_EXECUTABLE_PATH || undefined});
  const first = await context(), second = await context();
  for (const owner of [first, second]) await owner.addInitScript(snapshot => {
    window.sent = [];
    window.WebSocket = class {
      constructor() { this.readyState = 0; window.socket = this; setTimeout(() => {this.readyState = 1; this.onopen();}, 0); }
      send(raw) { const message = JSON.parse(raw); window.sent.push(message); if (message.kind === 'authenticate') this.onmessage({data: JSON.stringify(snapshot)}); }
      close(code = 1000) { this.readyState = 3; this.onclose?.({code}); }
    };
    window.move = (room, hash = snapshot.view.map_location.map_sha256) => {
      snapshot.cursor = String(BigInt(snapshot.cursor) + 1n);
      snapshot.view.map_location = {map_sha256: hash, room};
      window.socket.onmessage({data: JSON.stringify(snapshot)});
    };
  }, ready);
  const page = await first.newPage(), other = await second.newPage(), errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', msg => {if (msg.type() === 'error' && !msg.text().startsWith('Failed to load resource:')) errors.push(msg.text());});
  await page.goto(base + '/#token=synthetic-no-game');
  await page.locator('[data-current-room="228"]').waitFor({timeout: 60000});
  await other.goto(base + '/#token=second-synthetic-no-game');
  await other.locator('[data-current-room="228"]').waitFor({timeout: 60000});
  assert.equal(await page.locator('#minimap svg').isVisible(), true);
  await page.evaluate(() => { window.firstNode = document.querySelector('#minimap .mini-room'); window.move(229); });
  await page.locator('[data-current-room="229"]').waitFor();
  assert.equal(await page.evaluate(() => window.firstNode.isConnected), true, 'normal movement reuses scene DOM');
  assert.equal(await other.locator('[data-current-room="228"]').count(), 1, 'other character stays put');
  assert.match(await page.locator('[data-map-explorer]').getAttribute('href'), /room=229/);
  await page.evaluate(() => window.move(7501));
  await page.locator('[data-current-room="7501"]').waitFor();
  assert.match(await page.locator('[data-map-status]').textContent(), /Catacomb|Tunnel/i);
  assert.ok(await page.locator('[data-map-transitions] li').count() >= 10);
  await page.locator('#minimap summary').click();
  const transition = page.locator('[data-map-transitions] a').first();
  const popupEvent = page.waitForEvent('popup');
  await transition.click();
  const popup = await popupEvent;
  await popup.waitForLoadState();
  assert.ok(new URL(popup.url()).pathname.startsWith('/atlas/'));
  await popup.close();
  // Setup opens from the same character's minimap, never a typed identity.
  // This fixture token is deliberately not the HTTP server's real pairing.
  const setupEvent = page.waitForEvent('popup');
  await page.locator('#native-hunt-launch').click();
  const setup = await setupEvent;
  await setup.waitForLoadState();
  await setup.locator('#native-setup-status').filter({hasText:'Pairing and same-origin authorization required'}).waitFor();
  assert.ok(!setup.url().includes('setup_token='));
  const paired = await setup.evaluate(() => JSON.parse(sessionStorage.getItem('hydra-hunt-setup')));
  assert.equal(paired.token, 'synthetic-no-game');
  assert.equal(paired.session, String(ready.session));
  await setup.close();
  assert.deepEqual(await page.evaluate(() => window.sent.map(m => m.kind)), ['authenticate']);
  await mkdir('target/atlas-evidence', {recursive: true});
  await page.screenshot({path: 'target/atlas-evidence/live-minimap.png', fullPage: true});
  await page.evaluate(() => window.move(228, 'b'.repeat(64)));
  await page.locator('[data-map-status]').filter({hasText: 'versions differ'}).waitFor();
  assert.equal(await page.locator('#minimap svg').isVisible(), false);
  await page.evaluate(hash => window.move(228, hash), manifest.source_map_sha256);
  await page.locator('#minimap svg:not([hidden])').waitFor();
  await page.evaluate(() => window.socket.close(1000));
  await page.locator('[data-map-status]').filter({hasText: 'connected character'}).waitFor();
  assert.equal(await page.locator('#minimap svg').isVisible(), false);
  assert.equal(await other.locator('#minimap svg').isVisible(), true);
  assert.deepEqual(errors, []);
  console.log('Native-host minimap browser checks passed; no game commands sent.');
} finally { await browser?.close(); server.kill('SIGTERM'); }
