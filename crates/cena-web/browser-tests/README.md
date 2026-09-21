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
focus retention while a receipt is pending, and reconnect without command replay.
It requires local sockets and browser process launch permissions. No package or
browser download is performed by the runner.
