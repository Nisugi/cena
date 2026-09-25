# Offline Despana browser smoke

The transport contract suite requires Node 24 and no npm dependencies:

```sh
node crates/cena-web/assets/tests/session.test.mjs
```

The visual smoke additionally needs an existing Playwright installation and a
Chromium-family browser. Set `PLAYWRIGHT_MODULE` to that installation's module
directory and `BROWSER_EXECUTABLE_PATH` to the browser executable, then run:

```sh
node crates/cena-web/browser-tests/smoke.mjs
```

The test serves bundled assets over loopback and replaces WebSocket with a
fixture-driven stand-in inside the browser. It does not log into the game or
prove native integration; the Rust `m4_native_web` tests cover that interface.
The shared Rust/JavaScript fixture is synthetic contract data, not a real game
capture. Screenshots and the smoke result are written to a temporary directory
whose path is printed by the runner.

Coverage includes desktop/narrow-screen layout, unknown versus empty panels,
plain-text rendering of HTML-looking game text, supported styles, Enter input,
focus retention while a receipt is pending, reconnect without command replay,
and refresh followed by reopening a pairing link in the same document. Re-pairing
an outstanding command preserves uncertainty and does not replay it or duplicate
the command handler. The browser regression failed before the hashchange fix.
It requires local sockets and browser process launch permissions. No package or
browser download is performed by the runner.

## Native Hunt setup candidate

`native-hunt-setup.mjs` exercises real map clicks, authenticated configuration,
native TOML preview/save/reload, overwrite refusal and independent scrolling.
It launches only the offline fixture example; no game session exists.

```sh
cargo build -p cena --example hunt_setup_preview
NATIVE_SETUP_MAP=/mounted-storage/matching-gs.map node crates/cena-web/browser-tests/native-hunt-setup.mjs
```

Use the same Playwright/browser environment variables as above. The map's SHA-256
must match the bundled explorer catalogue. Set `NATIVE_SETUP_BIN` to test a
packaged copy of the offline example. Test profiles are intentionally retained
under `target/native-setup-browser/` for inspection; pairing URLs are not logged.
Keep the checkout, target and browser temporary directories on mounted storage.

The small native configuration, session and driver fixtures run in ordinary
`cargo test --workspace` without a private map. The full-map browser rehearsal
is an additional local acceptance gate, not a claimed hermetic CI fixture.

The `browser-smoke` CI job installs Playwright 1.63.0 and its Chromium before
running this same fixture test. The dependency-free contract suite remains a
separate check on both desktop platforms.
