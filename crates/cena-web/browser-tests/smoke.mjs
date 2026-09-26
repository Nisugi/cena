// Optional real-browser fixture smoke; no game socket, credentials, or npm install.
// PLAYWRIGHT_MODULE=/path/to/playwright BROWSER_EXECUTABLE_PATH=/path/to/browser node .../smoke.mjs
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFile, mkdtemp } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";

// "We do not know whether that command ran" -- matched as a PROPERTY of the
// message, not as its exact words. `session.test.mjs` carries the same
// constant for the same reason: these assertions read `/uncertain/`, which was
// the old wording, and a plainer rewording broke them here twice -- once at
// line 108 and again at 134, because the first fix did not grep the file.
const UNSURE = /may or may not|Not sure if/;

const { chromium } = createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE || "playwright");
const fixture = JSON.parse(await readFile(new URL("../../cena-ui/tests/fixtures/snapshot-v1.json", import.meta.url)));
const assets = new URL("../assets/", import.meta.url);
const output = await mkdtemp(join(tmpdir(), "hydra-browser-smoke-"));
const server = createServer(async (request, response) => {
  const name = request.url === "/" ? "index.html" : request.url.slice(1);
  if (!["index.html", "app.js", "session.js", "style.css", ...[
    'minimap', 'model', 'display-layout', 'profile', 'town-layout', 'reference-layout',
  ].map(module => `atlas/${module}.mjs`)].includes(name)) {
    response.writeHead(404).end();
    return;
  }
  response.setHeader("Content-Type", name.endsWith("js") ? "text/javascript" : name.endsWith("css") ? "text/css" : "text/html");
  response.end(await readFile(new URL(name, assets)));
});
await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
let browser;
try {
  browser = await chromium.launch({ headless: true, executablePath: process.env.BROWSER_EXECUTABLE_PATH || undefined });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript((snapshot) => {
    window.__sent = [];
    window.__sockets = [];
    window.WebSocket = class {
      constructor(url) {
        this.url = url;
        this.readyState = 0;
        window.__sockets.push(this);
        setTimeout(() => { this.readyState = 1; this.onopen(); }, 0);
      }
      send(raw) {
        const message = JSON.parse(raw);
        window.__sent.push(message);
        if (message.kind === "authenticate") setTimeout(() => this.message(snapshot), 0);
      }
      message(value) { this.onmessage({ data: JSON.stringify(value) }); }
      close(code = 1000) { this.readyState = 3; this.onclose?.({ code }); }
    };
    window.__message = (message) => window.__sockets.at(-1).message(message);
  }, fixture);
  await page.goto(`http://127.0.0.1:${server.address().port}/#token=synthetic-test-only`);
  await page.locator("#connection-status").filter({ hasText: "attempt 2" }).waitFor();
  assert.equal(new URL(page.url()).hash, "");
  assert.equal(await page.title(), "Hydra");
  assert.equal(await page.locator("#command-input").isDisabled(), true);
  assert.equal(await page.locator("#hands-left").textContent(), "Unknown");
  assert.equal(await page.locator("#hands-right").textContent(), "Empty");
  assert.equal(await page.locator("#mana-value").textContent(), "Unknown");
  assert.equal(await page.locator("#mana-meter").isVisible(), false);
  assert.equal(await page.locator("#health-value").textContent(), "-5 / 100 · 0%");
  assert.equal(await page.locator("#room-exits").textContent(), "None");
  assert.equal(await page.locator("#room-creatures").textContent(), "Unknown");
  assert.match(await page.locator("#story-output").textContent(), /<b>a test creature<\/b>/);
  assert.equal(await page.locator("#story-output b").count(), 0);
  assert.equal(await page.locator("#history-gap").isVisible(), true);
  await page.locator("#diagnostics-label").click();
  assert.match(await page.locator("#unknown-tags").textContent(), /<script>/);
  assert.equal(await page.locator("#unknown-tags script").count(), 0);
  assert.deepEqual(await page.evaluate(() => window.__sent.map((m) => m.kind)), ["authenticate"]);
  await page.screenshot({ path: join(output, "unknown-desktop.png"), fullPage: true });

  const ready = structuredClone(fixture);
  ready.cursor = String(BigInt(ready.cursor) + 1n);
  ready.view.lifecycle = { kind: "ready" };
  ready.view.room.title = "Town Square, East";
  ready.view.room.description = [{ text: "A broad cobblestone square opens beneath the evening sky. Lanterns glow beside the weathered stone buildings.", bold: false, monospace: false, preset: "roomDesc" }];
  ready.view.room.exits = ["north", "east", "southwest"];
  ready.view.room.creatures = [];
  ready.view.room.players = [{ id: "1", noun: "traveler", text: "a hooded traveler", status: null }];
  ready.view.left_hand = { kind: "empty" };
  ready.view.right_hand = { kind: "holding", id: "2", noun: "sword", name: "a steel longsword" };
  ready.view.vitals.health = { percent: 95, current: 213, max: 223 };
  ready.view.vitals.mana = { percent: 72, current: 86, max: 120 };
  ready.view.roundtime = { ends_at: null, remaining_seconds: 0 };
  ready.story = [
    { stream: "", runs: [{ text: "[Town Square, East]", bold: true, monospace: false, preset: "roomName" }], truncated: false },
    { stream: "", runs: ready.view.room.description, truncated: false },
    { stream: "", runs: [{ text: "Obvious paths: north, east, southwest", bold: false, monospace: false, preset: null }], truncated: false },
    { stream: "thoughts", runs: [{ text: "A distant voice greets the gathering evening.", bold: false, monospace: false, preset: "thought" }], truncated: false },
  ];
  ready.history_gap = false;
  await page.evaluate((value) => window.__message(value), ready);
  assert.equal(await page.locator("#command-input").isEnabled(), true);
  assert.equal(await page.locator("#story-output .text-room").count(), 1);
  await page.locator("#command-input").fill("look");
  await page.locator("#command-input").press("Enter");
  const command = await page.evaluate(() => window.__sent.at(-1));
  assert.equal(command.kind, "command");
  assert.equal(command.session, fixture.session);
  assert.equal(command.generation, fixture.generation);
  assert.equal(command.line, "look");
  assert.equal(await page.locator("#command-input").inputValue(), "");
  assert.equal(await page.locator("#command-input").evaluate((input) => document.activeElement === input && input.readOnly), true);
  assert.equal(await page.locator("#command-send").isDisabled(), true);
  await page.evaluate((value) => window.__message(value), {
    kind: "receipt", version: 1, session: command.session, generation: command.generation,
    request_id: command.request_id, status: "sent", detail: "Written to the game connection.",
  });
  // `Sent` must not claim the game ACTED -- only that the bytes went out, with
  // the server's own detail after it. Matched on the label rather than on
  // prose: this asserted /game outcome unconfirmed/, which was the old wording,
  // and a plainer rewording broke it here while `session.test.mjs` passed.
  assert.match(await page.locator("#command-status").textContent(), /^Sent: Written to the game connection\.$/);
  assert.equal(await page.locator("#command-input").evaluate((input) => document.activeElement === input && !input.readOnly), true);
  assert.equal(await page.locator("#history-gap").isVisible(), false);
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  await page.screenshot({ path: join(output, "ready-desktop.png"), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  await page.screenshot({ path: join(output, "ready-mobile.png"), fullPage: true });

  const injection = structuredClone(ready);
  injection.cursor = String(BigInt(ready.cursor) + 1n);
  injection.story[0].runs[0] = { text: '<img src=x onerror="window.__injected=true">', bold: false, monospace: false,
    preset: 'x" style="background:url(https://invalid.example)' };
  await page.evaluate((value) => window.__message(value), injection);
  assert.equal(await page.locator("#story-output img").count(), 0);
  assert.equal(await page.locator("#story-output [style]").count(), 0);
  assert.equal(await page.evaluate(() => window.__injected), undefined);

  await page.locator("#command-input").fill("north");
  await page.locator("#command-input").press("Enter");
  await page.evaluate(() => window.__sockets.at(-1).close(1006));
  assert.equal(await page.locator("#command-input").isDisabled(), true);
  assert.match(await page.locator("#command-status").textContent(), UNSURE);
  await page.waitForFunction(() => window.__sockets.length === 2);
  await page.locator("#connection-status").filter({ hasText: "attempt 2" }).waitFor();
  assert.deepEqual(await page.evaluate(() => window.__sent.map((m) => m.kind)), ["authenticate", "command", "command", "authenticate"]);

  // A reload keeps this tab's pairing: the token is in the tab's
  // sessionStorage, never in the URL (author, 2026-09-24 -- a refreshed hub
  // showed "Pairing required"). It reconnects with the same token, naming no
  // session because this page was opened for none.
  await page.reload();
  await page.waitForFunction(() => window.__sent.length === 1);
  assert.deepEqual(await page.evaluate(() => window.__sent), [
    { kind: "authenticate", version: 1, token: "synthetic-test-only" },
  ]);
  assert.equal(new URL(page.url()).hash, "", "the token did not come back into the URL");
  // Opening a pairing URL in this existing tab is a fragment navigation, not
  // another mount/document load, and re-pairs with the new token.
  await page.evaluate(() => { window.__sameDocument = true; window.__sent = []; });
  await page.goto(`http://127.0.0.1:${server.address().port}/#token=synthetic-repair-only`);
  await page.waitForFunction(() => location.hash === "", null, { timeout: 2000 });
  assert.equal(await page.evaluate(() => window.__sameDocument), true);
  await page.locator("#connection-status").filter({ hasText: "attempt 2" }).waitFor();
  assert.deepEqual(await page.evaluate(() => window.__sent), [
    { kind: "authenticate", version: 1, token: "synthetic-repair-only" },
  ]);
  await page.evaluate((value) => window.__message(value), ready);
  await page.locator("#command-input").fill("look");
  await page.locator("#command-input").press("Enter");
  assert.deepEqual(await page.evaluate(() => window.__sent.map((m) => m.kind)), ["authenticate", "command"]);

  // Re-pair while a command receipt is outstanding: close the old socket,
  // keep the input empty and delivery uncertain, and never resend that command.
  await page.goto(`http://127.0.0.1:${server.address().port}/#token=synthetic-second-pair`);
  await page.waitForFunction(() => window.__sent.length === 3 && location.hash === "", null, { timeout: 2000 });
  // The socket that carried the outstanding command -- the one before this
  // re-pair's -- is closed.
  assert.equal(await page.evaluate(() => window.__sockets.at(-2).readyState), 3);
  assert.equal(await page.locator("#command-input").inputValue(), "");
  assert.match(await page.locator("#command-status").textContent(), UNSURE);
  assert.deepEqual(await page.evaluate(() => window.__sent.map((m) => m.kind)), ["authenticate", "command", "authenticate"]);
  await page.evaluate((value) => window.__message(value), ready);
  await page.locator("#command-input").fill("inventory");
  await page.locator("#command-input").press("Enter");
  assert.deepEqual(await page.evaluate(() => window.__sent.filter((m) => m.kind === "command").map((m) => m.line)), ["look", "inventory"]);
  assert.deepEqual(errors, []);
  console.log(`PASS: shared fixture rendering, text safety, keyboard command, delivery receipt, reconnect/no replay, refresh keeps pairing, same-tab re-pair, desktop/mobile overflow. Screenshots: ${output}`);
} finally {
  await browser?.close();
  await new Promise((resolve) => server.close(resolve));
}
