# VellumFE Frontends and Platform Support — Inventory for Cena

**Scope.** Everything VellumFE presents to a human, and everything that constrains where
it can run. Read on 2026-09-17 against `E:/Cena/reference/VellumFE` at version
`0.3.0-beta.51` (`Cargo.toml:5`).

Files covered: `src/frontend/` (`mod.rs`, `events.rs`, `tui/`, `gui/`, `web/`, `headless/`,
`common/`), `src/theme.rs` + `src/theme/loader.rs`, `src/tts/mod.rs`, `src/launcher/`,
`src/window_position/`, `src/platform.rs`, `src/webui.rs`, `src/data/window.rs`,
`src/core/app_core/commands.rs` + `command_help.rs`, `android/`, `ios/`, `defaults/`,
`assets/`, `tests/architecture.rs`, `tests/web_server.rs`, `tests/embedded_runtime.rs`,
`book/src/widgets/`, `book/src/customization/`, `book/src/frontends/`.

This is input to a porting plan, so each section states: what it does, how big it is, what
it depends on, what game state it touches, and how hard it is to re-do in Rust for Cena.
Since VellumFE *is already Rust*, "re-implement" mostly means "lift, and decide what
becomes Lua-scriptable" — the notes flag where that is not true.

---

## 1. Size of the problem

Real `wc -l` over `src/frontend/`, measured 2026-09-17:

| Area | Files | Lines | Notes |
|---|---:|---:|---|
| `frontend/tui/` | 83 `.rs` | **58,167** | ratatui + crossterm |
| `frontend/gui/` | 87 `.rs` | **81,702** | egui/eframe + wgpu |
| `frontend/web/` | 5 `.rs` | 5,497 | axum server + wire protocol |
| `frontend/web/assets/` | 25 js/css/html | 22,853 | the browser client itself |
| **web total** | 30 | **28,350** | |
| `frontend/headless/` | 3 `.rs` | **4,679** | `runtime.rs` alone is 4,378 |
| `frontend/common/` | 5 `.rs` | 2,753 | shared input model, color, rect |
| `frontend/mod.rs` + `events.rs` | 2 | 138 | the entire abstraction |
| **frontend subtree** | | **~175,800** | |

Adjacent, frontend-serving modules:

| Module | Lines | Purpose |
|---|---:|---|
| `src/theme.rs` | 4,360 | `AppTheme` struct + built-in themes |
| `src/theme/loader.rs` | 942 | TOML `ThemeData` ↔ `AppTheme` |
| `src/frontend/gui/skin.rs` | 4,009 | egui texture/sprite pipeline for skins |
| `src/tts/mod.rs` | 821 | speech queue, gags, substitutions |
| `src/launcher/` | 3,522 | SSH cold-start of a remote headless Lich |
| `src/window_position/` | 1,307 | OS terminal-window geometry persistence |
| `src/webui.rs` | 24,083 bytes | Lich WebUI WebSocket bridge |
| `src/platform.rs` | 21 | the whole desktop/mobile shim |
| `src/core/app_core/commands.rs` | 5,132 | dot-command dispatcher |
| `src/core/app_core/command_help.rs` | 665 | the `.help` table (101 entries) |

Mobile shells (not Rust):

| Shell | Lines | Language |
|---|---:|---|
| `android/app/src/main/java/dev/vellumfe/` | 1,953 | Kotlin (11 files) |
| `android/rust/src/lib.rs` | 74 | Rust JNI glue |
| `ios/app/Sources/` | 1,250 | Swift (7 files) |
| `ios/rust/src/lib.rs` | ~100 | Rust C-ABI glue |

**The headline for Cena:** the two desktop frontends are 140k lines, 80% of the frontend
tree, and they share almost no rendering code. The mobile story costs only 4,679 lines of
headless runtime plus ~3,200 lines of shell — because it reuses the web client.

---

## 2. The frontend abstraction — and why it is a near-fiction

`src/frontend/mod.rs:26-73` defines the whole contract:

```rust
pub trait Frontend {
    /// Poll for user input events
    fn poll_events(&mut self) -> Result<Vec<FrontendEvent>>;

    /// Render the current application state
    fn render(&mut self, app: &mut dyn std::any::Any) -> Result<()>;

    /// Cleanup and shutdown the frontend
    fn cleanup(&mut self) -> Result<()>;

    /// Get current terminal/window size
    fn size(&self) -> (u16, u16);

    /// Downcast to concrete type (for accessing frontend-specific methods)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
```

**Exactly one type implements it.** `grep -rn 'impl Frontend for' src/` returns a single
hit: `src/frontend/tui/frontend_impl.rs:12`, `impl Frontend for TuiFrontend`.

The GUI says so explicitly at `src/frontend/gui/mod.rs:16-19`:

> The GUI is a native `eframe::App` driven by the egui event loop; it deliberately does not
> implement the `Frontend` trait (that trait models a frontend polled/rendered by an
> app-owned loop, which eframe inverts). The shared contract with the TUI is `AppCore` +
> `UiState` + the config layer.

The web frontend is not a frontend type at all — `src/frontend/web/mod.rs:4-8` calls it a
sidecar: the active frontend's runtime calls `web::start()`, attaches a `RemoteSink` to
`AppCore`, and calls `AppCore::flush_remote_state()` once per message batch.

### So what actually unifies the four UIs?

Three things, none of them the trait:

1. **`AppCore` + `UiState` + `config`.** This is the stated shared contract
   (`gui/mod.rs:18-19`).
2. **`FrontendEvent`** (`src/frontend/events.rs:11-25`) — five variants only:
   `Key { code, modifiers }`, `Mouse(MouseEvent)`, `Resize { width, height }`,
   `Paste { text }`, `Quit`. Key/mouse types come from `crate::data::input`, *not* from
   crossterm or egui, which is what lets the GUI reuse the same keybind resolution.
3. **`WindowContent`** (`src/data/widget.rs`) — the GUI dispatches rendering on this enum
   (`src/frontend/gui/app/widgets.rs:121`, `match &window.content`), the TUI dispatches on
   per-type `HashMap`s in `widget_manager.rs`, and the web serializes deltas from it.

`FrontendType` (`src/main.rs:131-138`) has **three** variants — `Tui`, `Gui`, `Headless` —
and `main.rs:851-891` is a plain `match` into three unrelated `run()` functions. There is
no dynamic dispatch through `Box<dyn Frontend>` anywhere in the shipped path.

**Porting note for Cena.** Do not copy this trait. It is five methods that one type uses,
and it forced `render(&mut self, app: &mut dyn std::any::Any)` — a downcast at
`frontend_impl.rs:76-78` — because the trait could not name `AppCore`. Cena's real seam is
the one VellumFE converged on in practice: a frontend-agnostic *state snapshot* plus a
frontend-agnostic *input event*, with each UI owning its own loop. That seam is already
proven here twice (GUI inverts the loop; web pushes deltas over a socket).

---

## 3. TUI (ratatui) — 58,167 lines

**Entry:** `frontend::tui::run` (`src/frontend/tui/runtime.rs:28`), called from
`main.rs:852-869`.

**Dependencies:** `ratatui 0.29`, `crossterm` (a Nisugi fork, see below),
`tui-textarea 0.7`, `unicode-segmentation` — all gated behind the `tui` feature
(`Cargo.toml:151`).

### Structure

`TuiFrontend` (`src/frontend/tui/mod.rs:88-146`) holds a `Terminal<CrosstermBackend<Stdout>>`,
a `WidgetManager`, and **17 `Option<...Editor|Browser|Form>` fields** — the modal editors are
literally fields on the frontend struct: `window_editor`, `indicator_template_editor`,
`highlight_browser`, `highlight_form`, `keybind_browser`, `keybind_form`, `hotbar_editor`,
`color_palette_browser`, `color_form`, `uicolors_browser`, `spell_color_browser`,
`spell_color_form`, `menu_keybind_editor`, `status_abbrev_editor`, `theme_browser`,
`theme_editor`, `settings_editor`, `pack_editor`, `skill_trainer_panel`.

Largest files:

| File | Lines | What |
|---|---:|---|
| `tui/input.rs` | 3,572 | hit-testing, mouse routing, palette slot assignment |
| `tui/sync.rs` | 3,107 | `UiState` → widget caches, generation-gated |
| `tui/window_editor/render.rs` | 2,680 | the window editor UI |
| `tui/highlight_form.rs` | 2,644 | highlight rule editor |
| `tui/input_handlers.rs` | 2,585 | keybind → action dispatch |
| `tui/text_window.rs` | 1,750 | the main scrollback widget |
| `tui/dialog.rs` | 1,542 | modal dialogs |
| `tui/window_editor.rs` | 1,514 | |
| `tui/settings_editor.rs` | 1,312 | |
| `tui/runtime.rs` | 1,184 | the main loop |

### Widget cache model

`WidgetManager` (`src/frontend/tui/widget_manager.rs:12-79`) is **30 parallel
`HashMap<String, W>`** — one per widget type (`text_windows`, `room_windows`,
`progress_bars`, `countdowns`, `active_effects_windows`, `hand_widgets`, `targets_widgets`,
`players_widgets`, `compass_widgets`, `injury_doll_widgets`, …) plus two generation maps
(`last_synced_generation`, `widget_data_generation`) that gate re-sync.

This is why `.scroll_window()` (`tui/mod.rs`) is a 10-branch cascade trying each map in
turn — there is no `dyn Widget`.

### Input handling

`poll_events` (`frontend_impl.rs:13-72`) waits 16 ms for the first event, then drains
everything already queued with zero-timeout polls, capped at
`MAX_EVENTS_PER_POLL = 128` (`frontend_impl.rs:20`) — so a paste or mouse drag does not pay
a render pass per event. Only `KeyEventKind::Press` is forwarded (`:26`); release events are
dropped. Resizes go through a `ResizeDebouncer` at **300 ms**
(`tui/mod.rs`, `ResizeDebouncer::new(300)`).

### The kitty-keyboard negotiation (numpad, part 1)

`TuiFrontend::new()` (`tui/mod.rs`) calls
`crossterm::terminal::supports_keyboard_enhancement()` and, when the terminal answers,
pushes `DISAMBIGUATE_ESCAPE_CODES | REPORT_ALL_KEYS_AS_ESCAPE_CODES | REPORT_ALTERNATE_KEYS`.
The comment is the rationale:

> terminals that answer the enhancement query (Alacritty, kitty, WezTerm, ...) report keypad
> keys distinctly, which is the only way numpad keybinds can work over a VT stream — the
> KEYPAD-tagged events are promoted to `Keypad*` codes in `crossterm_bridge`.

Flags are pushed *after* `EnterAlternateScreen` so they die with the alternate screen even
on a crash. `REPORT_ALTERNATE_KEYS` is required or shifted characters arrive as base key +
SHIFT (`'a'` instead of `'A'`).

### TUI-only capabilities

- **OSC 4 terminal palette control.** `execute_setpalette` (`tui/mod.rs`) writes
  `ESC ] 4 ; <slot> ; rgb:rr/gg/bb BEL` per configured palette color into slots 16–231;
  `execute_resetpalette` writes `ESC ] 104 BEL`. Slot search avoids ANSI 0–15 and grayscale
  232–255 (`tui/input.rs:7-13`).
- **Terminal title** (`tui/terminal_title.rs`).
- **Console window geometry** via `src/window_position/` — but only for launcher-spawned
  sessions (`main.rs:853-861`): "manual runs leave the terminal alone."

**Cena difficulty:** medium-high volume, low conceptual risk. ratatui is stable and this is
the only frontend that already satisfies the `Frontend` trait shape. The 30-HashMap widget
manager and the 17 modal-editor fields are the parts worth redesigning rather than
transliterating.

---

## 4. GUI (egui/eframe + wgpu) — 81,702 lines

**Entry:** `EguiApp::run` → `app::run_native_gui` (`src/frontend/gui/mod.rs:61-63`), called
from `main.rs:870-874`, which first calls `detach_exclusive_console()` on Windows.

**Dependencies** (`Cargo.toml:32-38`): `egui` and `eframe` from
`https://github.com/Nisugi/egui.git` branch `numpad-support`, `eframe` with
`default-features = false` + `["default_fonts", "wgpu", "x11", "wayland"]`; `fontdb 0.23`
for system font discovery; `skrifa 0.42` to pre-validate picked fonts ("epaint panics on
unparseable font data"); `image 0.25` with png/jpeg/webp/bmp/gif for skin decoding and
animated custom-emoji GIF frames. There is also a vendored `gpu-allocator-0.28.0`
(`Cargo.toml`, `[patch.crates-io]` → `vendor/gpu-allocator-0.28.0`).

### Why the egui fork exists — the numpad story

Both the crossterm and egui dependencies are Nisugi forks, and both exist for the *same*
reason: the numpad.

- **crossterm** (`Cargo.toml:29-30`): *"Nisugi fork of justinpopa's keypad-aware fork; same
  content today, ours so phase-2 work (Windows VT input for numpad under Alacritty/ConPTY)
  has a home."*
- **egui/eframe** (`Cargo.toml:33-34`): branch literally named `numpad-support`.

The GUI-side mechanism is in `src/frontend/gui/app/global_input.rs`:

- `collect_numpad_key_events(frame: &eframe::Frame)` (`global_input.rs:205`) reads
  `frame.numpad_keys()` — an API that does not exist in upstream eframe — filters out
  repeats, and maps `numpad_key.keybind_name()` through
  `numpad_binding_name_to_frontend_code` into a `FrontendEvent` key code.
- There is a `#[cfg]`-off stub at `global_input.rs:234` returning empty for builds without it.
- `sync_numpad_capture_keys(&mut self, frame: &mut eframe::Frame)` (`global_input.rs:243`)
  tells eframe *which* numpad keys have bindings, "so unbound keys keep their native
  behavior (typing digits, NumpadEnter submitting text)".
- Because numpad presses arrive on eframe's side channel rather than through
  `egui::InputState`, the keybind editors cannot read them directly. `handle_global_input`
  stashes them in `self.frame_numpad_presses` (`global_input.rs:11-15`) for the editors that
  render later in the frame (`editors/keybinds.rs:279-282`,
  `editors/hotbars.rs:246-248`, `editors/menu_keybinds.rs:61-63`).

**Why it matters for Cena:** GemStone/DragonRealms play is numpad-driven (movement on the
keypad is the genre convention), and *no mainstream Rust UI toolkit distinguishes numpad
keys from their navigation twins by default.* VellumFE had to fork two ecosystems to get
it. Cena inherits this problem on day one; budget for either maintaining forks or choosing
a toolkit whose key model already carries keypad identity.

### Structure

Largest files:

| File | Lines | What |
|---|---:|---|
| `gui/app/editors/controller.rs` | 4,312 | gamepad binding editor |
| `gui/skin.rs` | 4,009 | texture/sprite/nine-slice pipeline |
| `gui/app.rs` | 3,572 | the `eframe::App` shell |
| `gui/app/menus.rs` | 3,186 | right-click + toolbar menus |
| `gui/app/gamepad.rs` | 3,023 | gilrs → action, incl. the touch-wheel machine |
| `gui/app/zones.rs` | 2,889 | dock zones |
| `gui/app/widgets/text.rs` | 2,719 | the scrollback renderer |
| `gui/app/widgets/tests.rs` | 2,700 | widget tests |
| `gui/launcher.rs` | 2,229 | the launcher window (see §9) |
| `gui/studio.rs` | 1,835 | |
| `gui/persistence.rs` | 1,685 | `layout_v1.json` schema + migration |
| `gui/app/window_manager.rs` | 1,480 | |
| `gui/app/editors/creature_calibration.rs` | 1,393 | creature-field sprite calibration |

### Rendering dispatch

`VellumGuiApp::render_window_content` (`gui/app/widgets.rs:79-…`) is one big
`match &window.content` over **`WindowContent`**, covering 38 variants (everything except
`Spacer`). Before the match it reserves a painter slot for the skin background
(`widgets.rs:95-102`) and overrides `Body`/`Monospace`/`Small` text styles from the
window's own `settings.text_size` and `font_id` (`widgets.rs:106-119`) so list/grid widgets
follow per-window font settings.

Some variants read config the core does not carry — e.g. `WindowContent::Hand`
(`widgets.rs:169-…`) re-finds its `WindowDef::Hand { data }` in `app_core.layout.windows`
and calls `core::conditions::resolve_hand(data, &app_core.game_state, now_server,
app_core.gameobj_data_cached())`, where `now_server = Utc::now().timestamp() +
app_core.message_processor.server_time_offset`.

### GUI-only capabilities

- **True OS-window detach** (`gui/app/detached.rs`, 820 lines): each detached tab renders in
  its own native window via egui multi-viewport `Context::show_viewport_immediate`, so it can
  be dragged to another monitor. Geometry is captured per frame into `ViewportState` and
  persisted in `layout_v1.json`'s `detached_viewports` map. Closing the native window
  reattaches the tab to its previous zone (`detached.rs:1-10`).
- **Docking / zones / tab drag** (`zones.rs` 2,889, `dock.rs` 1,611, `tab_drag.rs` 1,130,
  `snap.rs` 1,162).
- **Skins** — see §7.
- **Gamepad** via `gilrs 0.11` (`Cargo.toml:102-104`: Windows WGI, Linux evdev, macOS
  IOKit), explicitly "Desktop-only: the mobile builds get controllers via the browser
  Gamepad API."
- **Map explorer** (`gui/app/map_explorer.rs`, 1,241) and `gui/map_view.rs` (669).
- **Native WebUI panel** (`gui/app/webui_panel.rs`, 1,252 + `webui_bridge.rs`, 491).

**Cena difficulty:** the highest in the tree. 82k lines, an immediate-mode paradigm, two
forked upstreams, wgpu, and multi-viewport detach. If Cena must ship mobile, this is the
component most likely to be staged rather than ported wholesale.

---

## 5. Web — "Despana" and `/play` — 28,350 lines

Two browser presentations served by one axum server.

### 5.1 Server (`src/frontend/web/`, 5,497 Rust lines)

| File | Lines |
|---|---:|
| `web/protocol.rs` | 2,274 |
| `web/server.rs` | 2,008 |
| `web/server/despana.rs` | 639 |
| `web/doll.rs` | 482 |
| `web/mod.rs` | 94 |

`web::start(config, session_label)` (`web/mod.rs:22-42`) creates a `RemoteSink` +
`UnboundedReceiver<RemoteEvent>` and spawns the axum task on the caller's tokio runtime.
Three layered entry points exist: `start`, `start_with_classic_maps` (scopes classic-map
filesystem authority to the session), and `start_with_startup_failure`
(`web/mod.rs:62-89`) — the last one exists specifically for mobile: without it *"a waiting
embedder (mobile shell startup) could not distinguish 'still binding' from 'failed'"*.

**Routes** (`web/server.rs:257-298`). A *status-only* router when `local_status_only()`:
`/health`, `/api/v1/session/exit-logout`, `/ws`. The full router adds: `/` (dashboard),
`/play`, `/characters`, `/creatures`, `/sessions`, `/status`, `/manifest.webmanifest`,
`/sw.js`, `/icon.svg`, the JS/CSS assets (`/app.js`, `/wheel-core.js`, `/char-core.js`,
`/webui-core.js`, `/pairing-core.js`, `/app.css`), media (`/sounds/{name}`, `/emoji`,
`/emoji/{name}`, `/image/{name}`), maps (`/api/v1/maps/classic`,
`/api/v1/maps/classic/{name}/rooms`, `/api/v1/maps/classic/{name}`), the injury doll
(`/doll.json`, `/doll/image`), a Lich WebUI passthrough (`/webui/files/{*path}`), session
control (`POST /api/v1/session/stop`, `POST /api/v1/session/exit-logout`), and `/ws`.

**Despana** is mounted as a separate `Router` (`web/server/despana.rs:29-57`) — the module
header states the boundary: *"This module owns the presentation's routes and embedded
browser assets. It deliberately does not own session state, authentication, or protocol
behavior: those remain in VellumFE's shared web server."* Its routes are `/despana`, its
nine JS modules, `/despana/app.css`, and
`GET/PUT /api/v1/presentations/despana/workspace` with a hard
`WORKSPACE_LAYOUT_MAX_BYTES = 64 * 1024` body limit (`despana.rs:24`), stored as
`despana-workspace-v1.json` (`despana.rs:25`), `WORKSPACE_LAYOUT_VERSION = 1`.

All assets are `include_str!`-embedded and served `Cache-Control: no-cache`, with the
reason given at `despana.rs:59-60`: *"Embedded assets change with the binary. A cached
client from an older protocol revision is more harmful than the small cost of re-fetching."*

### 5.2 Wire protocol (`web/protocol.rs`)

`PROTOCOL_VERSION: u8 = 1` (`protocol.rs:23`). Envelope
(`protocol.rs:25-31`): `{ "v": 1, "seq": n, "t": "...", "d": {...} }`. Every server→client
message carries a non-decreasing `seq`; for `text` it is the line's own sequence number
(the client's reconnect-resume cursor), for state messages the newest line seq at send time
(`protocol.rs:3-7`). Colors inside `StyledLine` segments are **already CSS hex strings** —
the server does the color resolution, not the browser.

`HelloPayload` (`protocol.rs:43-50`) carries `character`, `streams`, and a process-instance
`session` id: *"seqs restart when it changes, so clients must drop their resume cursor on
mismatch."*

`SnapshotMode` (`protocol.rs:53-63`): `Full` (client clears and renders fresh), `Resume`
(text contains only lines newer than the cursor; client appends), and a third variant for
resume-failed-because-evicted.

Delta message types, from the `encode("…")` calls (`protocol.rs:277-662`): `snapshot`,
`vitals`, `indicators`, `group`, `minivitals`, `effects`, `objectives`, `spells`,
`inventory`, `inventory_received`, `inventory_tree`, `session`, `injuries`, `targets`,
`field`, `entities`, `portals`, `charinfo`, `map_scene`, `map_state`, `streams`, `profiles`,
`macros`, `wheels`, `denied`. These come from `core::remote::RemoteDelta`, so the web
frontend's game-state surface is exactly that enum.

### 5.3 Browser client (`src/frontend/web/assets/`, 22,853 lines)

Plain ESM, no build step, no bundler, no `node_modules` in the tree.

| File | Lines | Role |
|---|---:|---|
| `assets/app.js` | 8,449 | the `/play` client (the big one) |
| `assets/app.css` | 2,976 | |
| `assets/despana/app.js` | 2,105 | the Despana workspace client |
| `assets/despana/app.css` | 1,569 | |
| `assets/despana/session.js` | 1,465 | |
| `assets/despana/workspace.js` | 1,061 | |
| `assets/characters.html` | 856 | the characters picker page |
| `assets/despana/layout.js` | 841 | |
| `assets/despana/map.js` | 417 | |
| `assets/index.html` | 380 | `/play` shell |
| `assets/despana/interactions.js` | 330 | |
| `assets/despana/index.html` | 305 | |
| `assets/wheel-core.js` | 290 | touch long-press radial menu |
| `assets/dashboard.html` | 282 | multi-session dashboard |
| `assets/despana/workspace-persistence.js` | 248 | |
| `assets/creatures.html` | 247 | |
| `assets/despana/macro-editor.js` | 205 | |
| `assets/webui-core.js` | 151 | Lich WebUI rendering in the browser |
| `assets/pairing-core.js` | 138 | |
| `assets/despana/macros.js` | 138 | |
| `assets/despana/inventory-refresh.js` | 115 | |
| `assets/char-core.js` | 102 | |
| `assets/despana/inventory-tree.js` | 99 | |
| `assets/sw.js` | 49 | service worker (PWA install) |
| `assets/despana/font-scale.js` | 35 | |

### 5.4 How it is tested

Two distinct mechanisms, both worth copying.

**`tests/web_server.rs` (2,911 lines, 129 test/fn items)** — end-to-end over *real* TCP with
*a hand-rolled WebSocket client* to avoid a dev-dependency (`web_server.rs:1-7`). It covers
"core sink → ring buffer / broadcast → axum server → WS client, plus client cmd →
RemoteEvent and reconnect-with-resume." Scenarios that need process-global state run in
child processes selected by env vars: `DESPANA_PROCESS_FIXTURE`,
`DESPANA_PROCESS_FIXTURE_PORT`, `DESPANA_WALKED_PORT_FIXTURE`,
`DESPANA_TOKEN_FAILURE_FIXTURE`, `DESPANA_REGISTRY_FAILURE_FIXTURE`
(`web_server.rs:26-30`).

**`tests/wheel_parity.cjs`** — a Node runner that drives
`src/frontend/web/assets/wheel-core.js` (*"the EXACT bytes the phone runs"*) through
`tests/data/wheel_golden.json`, the same truth table the Rust gamepad machine is tested
against (`gamepad.rs wheel_tests::golden_vectors_match_the_rust_machine`). Its stated
purpose: *"A change on one side only turns the other side's run red instead of the phone
firing a different slice than the desktop."*

**Cena note:** this is the answer to "how do you keep a JS client and a Rust client
behaving identically" — a shared golden-vector file with a runner on each side. Cena will
have the same problem if it keeps a browser surface.

---

## 6. Headless, Android, and iOS — the mobile story

This is the section Cena should read first, because mobile is a driving factor and this
subtree is the entire proof that the architecture supports it.

### 6.1 The `--no-default-features` build

`Cargo.toml:146-160` defines the feature set. Reproduced in full because it *is* the
mobile contract:

```toml
[features]
# The Android build is `--no-default-features`: core + parser + network +
# web frontend + (Phase A2) headless runtime only.
default = ["tui", "gui", "sound", "tts", "desktop", "gamepad"]
tui = ["dep:ratatui", "dep:crossterm", "dep:tui-textarea", "dep:unicode-segmentation"]
gui = ["dep:egui", "dep:eframe", "dep:fontdb", "dep:skrifa", "dep:image"]
gamepad = ["dep:gilrs"]
sound = ["dep:rodio"]
tts = ["dep:tts"]
# Desktop OS integrations: clipboard, browser-open, process inspection,
# credential store, terminal password prompt.
desktop = ["dep:arboard", "dep:open", "dep:sysinfo", "dep:keyring", "dep:rpassword"]
```

Note the binary itself requires all of them: `required-features = ["tui", "gui", "desktop"]`
(`Cargo.toml:14`), with the comment *"the feature split exists so the library builds with
`--no-default-features` for Android (headless + web)."* **The mobile target is the library,
not the binary.**

What survives `--no-default-features`: `core/`, `data/`, `parser/`, `network`, `config`,
`frontend/web/`, `frontend/headless/`, `frontend/common/`, `theme`, `webui`, `launcher`
(russh with the `ring` backend — chosen because *"aws-lc-rs pulls aws-lc-sys, a C/CMake
build that is painful to cross-compile for iOS/Android"*, `Cargo.toml:64-72`).

Crucially, `frontend/headless/` is **not** feature-gated. `headless/mod.rs:11-13`:

> Always compiled (no feature gate): it depends only on tokio, core, and the web frontend,
> and `--no-default-features` builds — the Android configuration — must include it.

### 6.2 The mechanically enforced Android-safety rule

`tests/architecture.rs:186-227`, `fn core_is_android_safe()`. It greps `src/core/`,
`src/data/`, `src/parser.rs` and `src/parser/` for seven needles:

```rust
let needles = &[
    "arboard::", "open::that", "sysinfo::", "keyring::",
    "rpassword::", "rodio::", "use tts::",
];
```

and fails the build with: *"core/, data/, and parser.rs must not use desktop-only crates
directly (the Android build compiles them with `--no-default-features`). Use the
feature-gated wrappers: `crate::clipboard`, `crate::platform`, `crate::tts`,
`crate::sound`."*

The comment names the four wrapper modules and the exact sites where the remaining desktop
crates *are* allowed: `src/performance.rs` (sysinfo), `src/config/profiles.rs` (keyring),
`src/network.rs` (rpassword), `src/frontend/web/server.rs`.

A companion rule, `core_and_data_do_not_reference_egui()` (`architecture.rs:168-184`),
scans `core`, `data`, `config` for `egui`/`eframe`.

`src/platform.rs` is the whole platform shim — 21 lines. `open_url` is `open::that()` under
`#[cfg(feature = "desktop")]` and, without it, a `tracing::warn!` + `anyhow::bail!`
(`platform.rs:10-21`), because *"on Android the WebView shell routes external links to the
system browser itself, so nothing should reach this call."*

**Cena should copy this test verbatim in spirit.** A grep-based architecture test is the
cheapest possible enforcement of "mobile is a real target" and it demonstrably worked — the
rule is what makes the Android build *possible* rather than aspirational.

### 6.3 The headless runtime (`src/frontend/headless/`, 4,679 lines)

`headless/mod.rs:1-8` frames it: *"the web sidecar's plan-doc 'Phase 7' and the Android
entrypoint: the game session runs here and web clients (a phone WebView, a desktop browser)
are the only interface. Unlike the TUI/GUI runtimes it owns a reconnect supervisor — on
mobile radios a dropped TCP session must recover without user intervention."*

`runtime.rs` (4,378 lines) is *"modeled on `frontend/tui/runtime.rs::async_run` with all
rendering, terminal, and geometry concerns removed, plus a session supervisor"*
(`runtime.rs:1-6`). Real constants:

| Constant | Value | Line | Purpose |
|---|---|---|---|
| `NOMINAL_COLS` / `NOMINAL_ROWS` | 120 / 40 | `runtime.rs:26-27` | windows still exist as stream-routing containers with no terminal, so highlight/stream processing behaves like a desktop session |
| `BACKOFF` | `&[1, 2, 5, 10, 30]` secs, ±20% jitter | `runtime.rs:30-31` | reconnect schedule, capped at the last entry |
| `MAX_UNATTENDED_LOSSES` | 2 | `runtime.rs:37` | *"the game idle-kicks after ~30 minutes, and without this cap the supervisor would re-login all night (battery + pointless auth churn)"* |
| `CONNECTION_WATCHDOG_INTERVAL` | 5 s | `runtime.rs:41` | persistent interval; incoming traffic must not postpone it |
| `CONNECTION_STALL_TIMEOUT` | 45 s | `runtime.rs:45` | silent connection or unanswered Lich identity probe → recycle or fail closed |

Three entry points: `run()` (desktop `--frontend headless`, builds its own tokio runtime and
handles Ctrl+C, `mod.rs:37-56`), `run_launcher_web_client()` (launcher-resolved profile,
`mod.rs:60-84`), and `async_run` / `async_run_embedded` / `StartupReporter` re-exported for
embedders (`mod.rs:116`).

`HeadlessLaunchOptions` (`mod.rs:19-31`) carries a `startup_identity:
Option<SessionLaunchIdentity>` whose doc note is a security boundary worth keeping:
*"Immutable identity of a launcher-profile-owned child. When present, web session controls
may reconnect only this character and connection; they cannot retarget the process away
from the registry/endpoint lease acquired at startup."*

### 6.4 The embedding contract (`headless/embedded.rs`, 195 lines)

This is the **single shared bootstrap both mobile shells call**; the platform crates own
"only string marshalling and platform logging" (`embedded.rs:1-5`). The contract, stated at
`embedded.rs:6-25`:

- **`data_dir`** — the app's private storage directory. Becomes `VELLUM_FE_DIR`, from which
  every config/profile/log path derives (`config/paths.rs`). Set via
  `std::env::set_var` at `embedded.rs:~75`, with a note that this is *"Safe on edition 2021;
  revisit if the crate moves to 2024 (set_var becomes unsafe there because of concurrent
  readers)"*.
- **`VELLUM_PASSWORD_KEY`** (optional) — 64 lowercase hex chars (32 bytes), set by the shell
  *before* `start()`, so saved passwords are sealed with ChaCha20-Poly1305
  (`config/profiles.rs`). **Missing key = persistent secret saving is DISABLED** (session-only
  login still works; never plaintext). Desktop headless testing may opt into plaintext with
  `VELLUM_ALLOW_PLAINTEXT_SECRETS=1`.
- **`start()` blocks until the web server is actually serving**, then returns
  `(port, token)` — *the listener's real bound port* (an unpinned instance may walk past an
  occupied base port) and the exact pairing token that listener authenticates.
- The shell then health-polls `http://127.0.0.1:<port>/health` and loads
  `http://127.0.0.1:<port>/play#token=<token>` in its WebView.

Implementation details that matter:

- `static CORE: Mutex<Option<Core>>` (`embedded.rs:38`) — process-global singleton. Idempotent:
  a second `start()` returns the running instance's info.
- `STARTUP_WAIT = Duration::from_secs(30)` (`embedded.rs:45`) — bounded so *"a shell never
  blocks its boot forever"*.
- The instance lock is held for the whole startup wait, *"so concurrent callers (Android
  activity + foreground service) serialize and all receive the same successful instance —
  and a retry can never overlap the cleanup of a failed startup"* (`embedded.rs:52-58`).
- The token is deliberately **not** loaded in `embedded.rs`: *"the server is the sole token
  authority, and a second load could race first-run creation."*
- Failed startup sends shutdown and **joins** the thread so nothing survives into a retry;
  timeout sends shutdown but **detaches** so a wedged runtime cannot block the shell.
- `lock_core()` recovers from poisoning via `poisoned.into_inner()` (`embedded.rs:47-52`).

Public surface, all JSON-in-strings: `start(data_dir) -> Result<(u16, String), String>`,
`start_json(data_dir) -> String` (`{"port":N,"token":"..."}` / `{"error":"..."}`),
`stop()`, `status_json() -> String` (`{"running":bool,"port":N}`).

`tests/embedded_runtime.rs` (283 lines) pins this contract, running every scenario in a
child process because *"the embedded runtime owns process-global state (the `CORE`
singleton, `VELLUM_FE_DIR`, the session registry's cached directory)"* — env vars
`EMBEDDED_FIXTURE_MODE`, `EMBEDDED_FIXTURE_DATA_DIR`, `EMBEDDED_FIXTURE_PORT`. It even
reserves ports with headroom (`reserve_walkable_port`, `embedded_runtime.rs:21-29`) to test
port-walking.

### 6.5 Android shell (`android/`)

**Rust side** (`android/rust/`, `crate-type = ["cdylib"]`, `vellum-fe` with
`default-features = false`): exactly three JNI functions on
`dev.vellumfe.core.VellumCore` —
`Java_dev_vellumfe_core_VellumCore_startCore(dataDir) -> jstring`,
`_stopCore()`, `_coreStatus() -> jstring` (`android/rust/src/lib.rs:44-73`). Everything else
is `embedded::start_json` / `stop` / `status_json`. Logging: `android_logger` at
`LevelFilter::Info`, tag `VellumCore`, behind a `std::sync::Once`. The manifest declares
`tracing = { features = ["log"] }` directly because *"the forwarding is this crate's
load-bearing behavior, not an accident of feature unification"* (`android/rust/Cargo.toml:14-18`).

**Kotlin side** (1,953 lines, 11 files):

| File | Lines | Role |
|---|---:|---|
| `MainActivity.kt` | 507 | fullscreen WebView + picker views |
| `RemotePickerView.kt` | 308 | the Characters picker |
| `CoreService.kt` | 227 | foreground service + wakelock |
| `CryptoKeys.kt` | 198 | Keystore-backed key material |
| `RemoteStore.kt` | 185 | `remote.bin` sealed saved servers |
| `QrScannerActivity.kt` | 151 | in-app pairing QR scan |
| `BootNavState.kt` | 119 | |
| `KeyBlobPolicy.kt` | 94 | |
| `CharacterWheel.kt` | 75 | |
| `SessionStatus.kt` | 66 | |
| `core/VellumCore.kt` | 23 | the JNI declaration |

Five of these have unit tests (`android/app/src/test/java/dev/vellumfe/`:
`BootNavStateTest`, `CharacterWheelTest`, `KeyBlobPolicyTest`, `RemoteTokenUpdateTest`,
`SessionStatusTest`).

**WebView config** (`MainActivity.kt:67-102`): `javaScriptEnabled = true`,
`domStorageEnabled = true`, `mediaPlaybackRequiresUserGesture = false` (matched *"in the iOS
shell's `WebViewContainer`"*). `shouldOverrideUrlLoading` allows in-app navigation **only**
when `host == "127.0.0.1"` or `host == allowedRemoteHost` — every other URL goes to the
system browser. The boot URL is
`http://127.0.0.1:$port/play#token=$token&app=1&nativepicker=1` (`MainActivity.kt:206`) —
note `app=1` and `nativepicker=1`, the flags that make the web client hide its in-page
Remote tab in favor of the native picker. Health is polled at
`http://127.0.0.1:$port/health` (`MainActivity.kt:433`). Remote (pairing) mode navigates to
`http://$host:${target.port}/#$fragment` (`MainActivity.kt:269`).

**Build targets** (`android/app/build.gradle.kts:8-20`): `compileSdk = 35`,
`minSdk = 26` (*"Android 8.0: adaptive icons + startForegroundService"*), `targetSdk = 35`,
`ndk { abiFilters += listOf("arm64-v8a", "x86_64") }` — arm64 for phones, x86_64 for the
Studio emulator, matching the cargo-ndk targets that populate `src/main/jniLibs`.

**Manifest** (`AndroidManifest.xml`): permissions `INTERNET`, `FOREGROUND_SERVICE`,
`FOREGROUND_SERVICE_SPECIAL_USE`, `WAKE_LOCK`, `POST_NOTIFICATIONS`,
`REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` (with the note that the Play Store restricts it but
*"a persistent game session is exactly its intended use case"* — this is a sideload app),
and optional `CAMERA` (`uses-feature … required="false"` so it installs on camera-less
devices).

The service is `foregroundServiceType="specialUse"` with an explicit
`PROPERTY_SPECIAL_USE_FGS_SUBTYPE` of *"Persistent connection to a text-game server the user
is actively playing"*, and the comment states why: *"specialUse (not dataSync): a persistent
game connection has no 6-hour budget; Android 15 caps dataSync services at 6h."*

Deep links: `vellum://lich?host=…&port=…` prefills the Lich tab and
`vellum://remote?host=…&port=…&token=…` prefills Remote; `launchMode="singleTask"` routes
them through `onNewIntent`. Neither ever auto-connects.

**Cleartext policy** (`res/xml/network_security_config.xml`): `cleartextTrafficPermitted="true"`
globally, with the reasoning spelled out — the embedded server is plain HTTP by design and a
paired desktop host is unknowable at build time, so *"the real gate lives in MainActivity's
WebView allowlist: only 127.0.0.1 and the paired remote host ever load in-app. Game/auth
traffic is TLS or plain TCP at the Rust layer and unaffected by this policy."*

### 6.6 iOS shell (`ios/`)

**Rust side** (`ios/rust/`, `crate-type = ["staticlib"]`): four C-ABI functions —
`vellum_start_core(const char* data_dir) -> char*`, `vellum_stop_core()`,
`vellum_core_status() -> char*`, `vellum_string_free(char*)`. The header
`ios/rust/include/vellum_core.h` is hand-written and states *"keep the two in sync"*; every
returned `char*` is a Rust-owned `CString` that Swift must free exactly once. Logging via
`oslog` under subsystem `dev.vellumfe.core`. Null and non-UTF-8 `data_dir` are returned as
`{"error": …}` rather than panicking across the FFI edge (`ios/rust/src/lib.rs:54-73`).
Two round-trip tests through the real C ABI live at `ios/rust/src/lib.rs:88-107`.

**Swift side** (1,250 lines): `ContentView.swift` (446), `RemotePickerView.swift` (330),
`WebViewContainer.swift` (163, the WKWebView host), `RemoteStore.swift` (142),
`CryptoKeys.swift` (71), `CoreBridge.swift` (53, the C-ABI wrapper), `VellumApp.swift` (45).
Project generated by XcodeGen (`ios/project.yml`), with `PrivacyInfo.xcprivacy` and
`BridgingHeader.h`.

Distribution differs materially: Android is a sideloaded APK
(`vellum-fe-android-arm64.apk` from GitHub releases, per `book/src/frontends/android.md`);
iOS ships through **TestFlight** while in beta, with *"no downloadable file on the releases
page"* (`book/src/frontends/ios.md:59-63`). That is an Apple-policy constraint Cena
inherits.

### 6.7 What the mobile client is, and is not

From `book/src/frontends/android.md`: *"The app is a full client and a pairing client — not
one or the other."* Three routes, all live:

1. **Direct** — the phone connects to play.net itself (account/password/character/game
   selector, every GemStone IV and DragonRealms world). Nothing running at home.
2. **Lich attach** — host/port to a `--detachable-client` Lich on the PC, so a scripted
   character keeps its scripts.
3. **Pairing / second screen** — scan the `vellum://remote?…` QR from a desktop session's
   `.webinfo` and mirror it. *"Lich's one-client limit does not bite here — the web layer
   mirrors the session rather than replacing its client."*

**What the phone can author** (`book/src/frontends/web.md:157-185`): theme presets, chrome
toggles, opacity, text size 6–24 px, the full desktop settings registry over the wire at
character or global scope (written to the *hosting machine's* config), highlight rules
(including **squelch** and **redirect off / redirect only / redirect + copy**), stream and
prompt colors, streams, touch wheel, controller, speech, SSH launcher, and raw TOML editors
for highlights and colors with import/export.

**What the phone genuinely cannot author** (`web.md:182-184`): *"layout and panel placement,
window resizing, game keybinds, and macro `hidden_when` conditions."*

**Battery and lifecycle rules** (`android.md`, "Battery & lifecycle"): wakelock held **only
while a session is active**; swiping the app away mid-session keeps playing, swiping it away
at the login screen stops the service; repeated drops with no user input stop the reconnect
loop (this is `MAX_UNATTENDED_LOSSES = 2` surfacing in the product); a one-time
battery-optimization exemption prompt on first launch.

**WebView-engine gotcha** (`android.md`, Tips): on Android 8 and 9 the rendering engine comes
from **Chrome**, not Android System WebView — a blank client there is a stale Chrome. On
Android 10+ it is Android System WebView.

---

## 7. The widget system

There are three separate counts, and conflating them is the usual mistake:

| Count | What | Source |
|---:|---|---|
| **39** | `WidgetType` enum variants (the *kinds* of widget) | `src/data/window.rs:23-80` |
| **81** | `[templates.*]` entries in the window catalog (the *presets* you can add) | `defaults/globals/window_catalog.toml` |
| **77** | ordered entries in the `CATALOG` table (which presets the menu shows, and per-game gating) | `src/config/presets.rs:174-353` |
| **33** | prose pages in `book/src/widgets/` (one per *family*, not per preset) | `book/src/widgets/` |

### 7.1 The type roster

`WidgetType` (`src/data/window.rs:23-80`), all 39 variants:

`Text`, `TabbedText`, `Progress`, `Countdown`, `Compass`, `Indicator`, `Room`, `Inventory`,
`Reserve`, `CommandInput`, `Dashboard`, `InjuryDoll`, `Hand`, `ActiveEffects`, `Quests`,
`Targets`, `Players`, `Items`, `Spells`, `Spacer`, `Performance`, `Perception`, `Container`,
`Experience`, `GS4Experience`, `Encumbrance`, `Quickbar`, `Hotkeybar`, `MiniVitals`,
`Betrayer`, `WebUi`, `Map`, `DialogPanel`, `MissingSpells`, `MultiAccount`, `Containers`,
`BestiaryView`, `CreatureField`.

`WidgetType::from_str` defaults unknown strings to `Text` (`window.rs:85-87`);
`try_from_str` (`window.rs:92-133`) is the strict version and carries the aliases:
`injury_doll|injuries`, `command_input|commandinput`, `creaturefield|creature_field`,
`webui|lichui`, `dialogpanel|dialog_panel`. `VALID_TYPES` (`window.rs:136-178`) lists **37**
names against 38 parseable ones — **`"items"` is the single omission** (verified by diffing
the `try_from_str` arms against the `VALID_TYPES` array). `VALID_TYPES` feeds help messages
only, so `.addwindow items` still parses; the list just never advertises it. A small
real bug, and exactly the kind the `command_help.rs` tripwire pattern (§10) would have
caught if it had been applied here too.

### 7.2 Which frontend implements what

| Widget | TUI | GUI | Web (`/play` + Despana) | Game state it reads |
|---|:-:|:-:|:-:|---|
| Text | ✓ `text_window.rs` (1,750) | ✓ `widgets/text.rs` (2,719) | ✓ `text` msg | stream-routed `StyledLine`s |
| TabbedText | ✓ `tabbed_text_window.rs` (1,065) | ✓ | ✓ stream chips | several streams + unread badges |
| Progress | ✓ `progress_bar.rs` (387) | ✓ `widgets/vitals.rs` | ✓ `vitals` | named vitals feed (health/mana/stamina/spirit/stance/concentration) |
| Countdown | ✓ `countdown.rs` (481) | ✓ | ✓ | roundtime / casttime / stuntime / aimtime / pulse + `server_time_offset` |
| Compass | ✓ `compass.rs` | ✓ `widgets/map_compass.rs` (493) | ✓ floating compass | room exits |
| Indicator | ✓ `indicator.rs` | ✓ | ✓ `indicators` | `StatusInfo` flags (13 presets) |
| Room | ✓ `room_window.rs` (879) + `room_window_ops.rs` (660) | ✓ | ✓ | room name/desc/objects/players/exits |
| Inventory | ✓ `inventory_window.rs` | ✓ | ✓ `inventory` | inventory snapshot stream |
| Reserve | ✓ | ✓ | ✓ | `reserve` stream (GS4-gated) |
| CommandInput | ✓ `command_input.rs` (845) | ✓ `widgets/command_widget.rs` (404) | ✓ (page input) | history + `cmdlist` |
| Dashboard | ✓ `dashboard.rs` (698) | ✓ | ✓ status drawer | composite of indicator feeds |
| InjuryDoll | ✓ `injury_doll.rs` (585) | ✓ `widgets/injury.rs` (573) | ✓ `injuries` + `/doll.json`, `/doll/image` | injury/scar parts |
| Hand | ✓ `hand.rs` (394) | ✓ (+`resolve_hand` conditions) | ✓ | left/right/spell hand items + links |
| ActiveEffects | ✓ `active_effects.rs` | ✓ | ✓ `effects` | spells/buffs/debuffs/cooldowns/timers by `category` |
| Quests | ✓ `quests_window.rs` | ✓ | ✓ `objectives` | `GameState.objectives` (GS4) |
| Targets | ✓ `targets.rs` (801) | ✓ | ✓ `targets` | room creatures + status |
| Players | ✓ `players.rs` (486) | ✓ | ✓ `entities` | room "Also here" line |
| Items | ✓ `items.rs` | ✓ `WindowContent::Items` | ✓ `entities` | room objects (non-creature) |
| Spells | ✓ `spells_window.rs` | ✓ | ✓ `spells` | known-spell list |
| Spacer | ✓ `spacer.rs` | **✗** (only non-`Spacer` `WindowContent` arm) | n/a | none — layout only |
| Performance | ✓ `performance_stats.rs` | ✓ | ✗ | `src/performance.rs` (sysinfo, desktop-gated) |
| Perception | ✓ `perception.rs` (699) | ✓ | ✓ | DR perception list (DR-gated) |
| Container | ✓ `container_window.rs` (352) | ✓ `widgets/containers.rs` (646) | ✓ | one bag's contents, session-only |
| Experience | ✓ `experience.rs` | ✓ | ✓ | DR skill training (DR-gated) |
| GS4Experience | ✓ `gs4_experience.rs` (499) | ✓ | ✓ | level / mind state / next-level (GS4-gated) |
| Encumbrance | ✓ `encumbrance.rs` (406) | ✓ | ✓ | encumbrance level + text |
| Quickbar | ✓ `quickbar.rs` (449) | ✓ | ✓ macro rail | game-sent `cmdlist` bars |
| Hotkeybar | ✓ `hotkey_bar.rs` (386) | ✓ `widgets/links_bars.rs` (928) | ✓ macro tray | `hotbars.toml` + condition eval |
| MiniVitals | ✓ `minivitals.rs` (658) | ✓ | ✓ `minivitals` | all four vitals in one strip (GS4-gated) |
| Betrayer | ✓ `betrayer.rs` (373) | ✓ `widgets/panels.rs:314` | ✗ | blood pool 0–100 + feeding items (GS4-gated) |
| WebUi | ✓ `webui_window.rs` (428) | ✓ `webui_panel.rs` (1,252) | ✓ `webui-core.js` + `/webui/files/{*path}` | Lich WebUI JSON component tree |
| Map | ✓ (via `core/map_service.rs`) | ✓ `map_view.rs` + `map_explorer.rs` | ✓ `despana/map.js`, `map_scene`/`map_state` | mapdb room graph + current room |
| DialogPanel | ✗ | ✓ | ✗ | accumulated game dialog store by id |
| MissingSpells | ✓ `missing_spells.rs` | ✓ | ✗ | `character.watched_spells` + effects |
| MultiAccount | ✗ | ✓ `widgets/multiaccount.rs` (882) | ✓ `/sessions`, `dashboard.html` | **not** `GameState` — the `MultiAccountHub` on the GUI app |
| Containers | ✓ `containers_window.rs` | ✓ | ✓ `inventory_tree` | `GameState.managed_inventory` (`.invsync`, GS4-gated) |
| BestiaryView | points at `.bestiary` | ✓ `widgets/bestiary.rs` (513) | ✓ `creatures.html` | bundled `defaults/bestiary.json` (GS4-gated) |
| CreatureField | ✗ | ✓ `widgets/creature_field.rs` (1,339) | ✓ `field` | `AppCore.creature_field` + `GameState.room_creatures` (GS4-gated) |

Evidence for the ✓/✗ columns: the TUI column is `src/frontend/tui/widget_manager.rs:12-72`
(30 typed `HashMap`s) plus `src/frontend/tui/mod.rs` module list; the GUI column is the
`match &window.content` at `src/frontend/gui/app/widgets.rs:121` (38 of 39 `WindowContent`
variants, everything but `Spacer`); the web column is the `encode("…")` delta set at
`src/frontend/web/protocol.rs:277-662` plus the route table at `web/server.rs:269-298`.

**UNVERIFIED:** the TUI ✗ marks for `DialogPanel`, `MultiAccount` and `CreatureField` are
inferred from the absence of a corresponding `widget_manager` field and module; I did not
trace `tui/sync.rs` exhaustively to prove no fallback path renders them as text. To confirm,
grep `src/frontend/tui/sync.rs` for each `WindowContent` variant.

### 7.3 The catalog, and per-game gating

`defaults/globals/window_catalog.toml` (2,312 lines, 81 `[templates.*]`) is described in its
own header as **"GENERATED, then maintained by hand. Produced by serializing the former
77-arm match in `config/presets.rs`"**, and *"Each entry is a `WindowDef` exactly as
`layout.toml` stores one, so the format is already familiar and already round-trips."*
Crucially: *"Order and per-game gating are NOT here: they live in the `CATALOG` table in
`config/presets.rs`. This file answers only 'what IS a compass', never 'which windows
exist'."* A golden fixture (`tests/template_characterization.rs`, 246 lines, against
`tests/fixtures/template_characterization.txt`) pins every template.

Template names (81): `active_effects_custom active_spells aimtime alert_timers ambients
announcements bestiary bestiaryview betrayer bleeding bounty buffs casttime chat
command_input compass concentration containers cooldowns countdown_custom creaturefield
dashboard dead death debuffs diseased encum entity_custom experience familiar gs4_experience
health hidden hotkeybar injuries injury_doll inventory invisible items joined kneeling left
logons loot main mana map minivitals missingspells multiaccount perception performance
players poisoned progress_custom prone pulse quests quickbar reserve right room roundtime
sitting society spacer speech spell spells spirit stamina stance standing stunned stuntime
tabbedtext_custom targets text_custom thoughts webbed`.

Widget-type distribution across those 81 templates: `text` ×13, `indicator` ×13,
`progress` ×7, `countdown` ×6, `active_effects` ×6, `hand` ×3, `targets`/`tabbedtext`/
`injury_doll` ×2 each, and one each of the remaining 26 types.

**Per-game gating** — 13 of the 77 `CATALOG` entries are gated
(`src/config/presets.rs:174-353`):

- `Some(GameType::DR)` ×3: `concentration`, `perception`, `experience`
- `Some(GameType::GS4)` ×10: `creaturefield`, `aimtime`, `pulse`, `reserve`, `containers`,
  `bestiaryview`, `gs4_experience`, `minivitals`, `betrayer`, `quests`

Everything else is `None` (both games). **For Cena this is the cheapest possible model for
two-game support: one flag per catalog row, not two codebases.**

The widget philosophy is stated plainly in `book/src/widgets/README.md:7-18`: *"every widget
type is bound to one specific feed coming off the wire… you do not configure a widget to go
find data — you place the window and the feed arrives. A bar with no feed id draws nothing at
all, not an error."* The in-app catalog groups them as **Status, Progress Bars, Countdowns,
Active Effects, Entities, Hands, Text Windows, Character, Navigation, Hotbars, Containers,
Dialogs, Other**.

---

## 8. Theming and skins — two separate systems

The distinction is stated at `book/src/customization/skins.md:4-5`: **"Themes own colors and
fonts; skins own graphics."**

### 8.1 Themes — cross-frontend

`src/theme.rs` (4,360 lines) + `src/theme/loader.rs` (942). `AppTheme`
(`theme.rs:14-…`) is a flat struct of **58 `Color` fields**, grouped by comment into
window / text / background / editor / browser / form / status / link categories.
`Color` comes from `crate::frontend::common::Color` (`theme.rs:8`) — a frontend-agnostic
type in `frontend/common/color.rs` (684 lines), which is what lets the same theme drive
ratatui and egui.

**36 built-in themes** as `pub fn <name>() -> AppTheme` constructors (counted via
`grep -c 'pub fn [a-z_0-9]*() -> AppTheme' src/theme.rs`): `dark` (`theme.rs:966`),
`light`, `nord`, `dracula`, `solarized_dark`, `solarized_light`, `monokai`, `gruvbox_dark`,
`night_owl`, `catppuccin`, `cyberpunk`, `retro_terminal`, `apex`, `minimalist_warm`,
`forest_creek`, `synthwave`, `ocean_depths`, `forest_canopy`, `sunset_boulevard`,
`arctic_night`, and more. `book/src/customization/themes.md:27-31` adds that the roster
includes a deliberate **accessibility** set: `high-contrast-dark`, `high-contrast-light`,
`deuteranopia`, `protanopia`, `tritanopia`, `monochrome`, `low-blue-light`, `photophobia`,
`adhd-focus`, `reduced-motion`.

`theme/loader.rs` is explicitly *"Frontend-agnostic theme loading and serialization… Both
TUI and GUI frontends can use this module"* (`loader.rs:1-6`). `ThemeData` is the TOML
mirror of `AppTheme` with `String` hex fields instead of `Color`. Custom themes live in
`~/.vellum-fe/themes/<name>.toml`; **every color field must be present or the file is
skipped** (`themes.md:44-46`). Field values may be hex *or* a named palette color from
`colors.toml` (e.g. `link_color = "Link"`).

The TUI caches the resolved theme in a `ThemeCache` (`tui/mod.rs`, `theme_cache.rs`) keyed by
a `theme_version: u64`, because a `HashMap` lookup + clone per render was measurable.

### 8.2 Skins — GUI only

`src/frontend/gui/skin.rs` (4,009 lines) + `src/config/skins.rs` (the frontend-agnostic
schema). The split is deliberate (`gui/skin.rs:3-6`): *"The image-pool conventions and the
canonical injury doll part table live in `crate::config::skins`/`crate::config::pool`
(shared with the web frontend, which compiles without egui). This module owns everything
egui: texture loading, the appearance-driven runtime state, widget sprite lookups, and the
paint helpers."*

`skins.md:7-10` is unambiguous: *"Skins apply to the Desktop GUI only — the terminal has no
image pipeline."*

Schema types in `src/config/skins.rs`: `SkinManifest` (`:43`), `SheetSpec` (`:110`),
`CompassSkin` (`:127`), `InjuryDollSkin` (`:147`), `DollPartSpec` (`:186`), `DollVariant`
(`:211`), `DollSet` (`:222`), `DollDotSpec` (`:237`), `CreatureCardSkin` (`:294`),
`CreatureFieldSkin` (`:327`), `CreatureFieldCamera` (`:343`), `CreatureFieldSolver` (`:380`),
`CardOverlay` (`:445`), `OverlaySpace` (`:479`), `OverlaySource` (`:488`), `AnimateSpec`
(`:499`), `AnimateKind` (`:517`), `CardVariant` (`:542`), `CardSet` (`:553`), `LiftSpec`
(`:570`), `SkinMeta` (`:672`), `UiPalette` (`:684`), `WindowSkin` (`:754`), `EdgeSpec`
(`:771`), `BorderSpec` (`:799`), `BackgroundSpec` (`:815`), `BackgroundFit` (`:838`).

A skin is a folder `~/.vellum-fe/global/skins/<name>/` with a `skin.toml` plus PNG/JPEG/WebP/
BMP images. Shared art lives in a **pool** at `~/.vellum-fe/global/images/` with subfolders
`icons/`, `frames/`, `dolls/`, `compass/`, `backgrounds/`; relative manifest paths resolve
skin-folder-first then pool, so `image = "backgrounds/paper.png"` works from any skin without
copying (`skins.md:47-56`). `skin.toml` sections: `[meta]`, `[window.<name>.background]`
(`fit = stretch|cover|contain|tile|center`, `opacity`, `tint`, `scrim`),
`[window.<name>.border]` (nine-slice: `slice = [t,r,b,l]`, `scale`), `[frames.*]` (named
frames for a per-window picker), `[icons]`, `[compass]`, and a deep `[injury_doll]` tree with
`anchors`, `dots`, per-part entries (`nsys`, `leftArm`, `leftHand`, …), `[[variants]]` with
`when` predicates, and `[sets.*]` (e.g. `silhouette`).

`skin.toml` **hot-reloads within a second** while a skin is active; image swaps need
`.reloadskin` because they don't touch the manifest (`skins.md:42-45`). The skin runtime is
described at `gui/skin.rs:7-9` as *"The legacy live-manifest skin runtime is gone — skins are
inert presets applied to the appearance store (`config::skin_pack`); everything here resolves
from the pool."*

**The Wrayth connection.** VellumFE targets the look and feel of Simutronics' official
Wrayth/StormFront client. Concrete evidence rather than vibes:

- `src/config/wrayth_import.rs` (~300 lines) imports highlights from a Wrayth/StormFront
  per-character settings XML: `line="y"` → whole-line color, `case="y"` → case-SENSITIVE
  (`wrayth_import.rs:37`), imported rules tagged `category = "wrayth"` /
  `"wrayth-names"` with keys `wrayth_<slug>` / `wrayth_names_NN`. *"Wrayth has no alert
  concept to import"* (`:136`).
- `src/config/appearance.rs:81` — a default sized so *"Default matches Wrayth, whose hand
  icons span about two text lines."*
- `src/config/skins.rs:34` — the pool exists so a user can point at *"a user's local Wrayth
  art"* without copying.
- `src/config/skins.rs:99` — nine-slice sprites for *"interactive dialog-panel controls
  (Wrayth …)"*.
- `src/config/widgets.rs:1006` — indicators can share ONE cell with icons painted over each
  other, *"Wrayth-style"*.

### 8.3 Other customization surfaces

`book/src/customization/` (1,852 lines across 13 pages): `highlights.md` (130),
`keybinds.md` (78), `layouts.md` (190), `themes.md` (69), `skins.md` (370),
`skin-art-prompts.md` (364 — a prompt kit for generating skin art with an image model,
including a keying script that turns black backgrounds into transparency),
`inline-images.md` (251), `submitting-art.md` (152), `sounds.md` (110), `emoji.md` (82),
`controller.md` (3, stub), `ui-packs.md` (3, stub), `README.md` (50).

Shipped defaults (`defaults/globals/`): `colors.toml` (1,481 lines), `config.toml` (387),
`keybinds.toml` (177), `controller.toml` (154), `hotbars.toml` (107), `macros.toml` (55),
plus `cmdlist1.xml`, `effect-list.xml`, `gameobj-data.xml`, `mazes.toml`,
`spell_abbrev.toml`, `travel_overrides.toml`, three layouts
(`layouts/layout.toml`, `none.toml`, `sidebar.toml`), two templates, and a sound
(`sounds/wizard_music.mp3`). Root-level: `defaults/bestiary.json`,
`defaults/curated_maps.toml`, `defaults/map_overrides.json`. All embedded via `include_dir 0.7`
(`Cargo.toml:47`).

Bundled font/emoji assets: `assets/fonts/NotoEmoji-VariableFont_wght.ttf`,
`assets/fonts/NotoSansSymbols2-Regular.ttf`, and a full `assets/twemoji/72x72/` PNG set.
Shortcode expansion (`:grin:`) uses the embedded gemoji data from `emojis 0.9.0`
(`Cargo.toml:44`).

**UI packs.** `.uiexport` / `.uiimport` produce *"a zip with the config files + skin assets
that make a UI. Deflate only — no extra codecs"* (`Cargo.toml:83-85`,
`zip 2` with `default-features = false, features = ["deflate"]`). Implementation:
`src/core/uipack.rs`, TUI editor `tui/pack_editor.rs` (440).

---

## 9. TTS, window positioning, and the launcher

### 9.1 TTS (`src/tts/mod.rs`, 821 lines)

Behind the `tts` feature → the `tts 0.26` crate ("stub on non-desktop builds",
`Cargo.toml:107-108`). Cross-platform: Windows SAPI, macOS AVSpeechSynthesizer, Linux Speech
Dispatcher (`tts/mod.rs:5`).

Model (`tts/mod.rs:11-19`): *"lines are spoken strictly in arrival order. New lines never
interrupt the current utterance — auto-play advances only when the engine reports the
previous utterance finished (`handle_utterance_ended`, wired from the frontends' event loops
via `try_recv_event`). The exception is `Priority::Alert`, which stops current speech and
speaks at once. Manual navigation (next/previous/next-unread) always interrupts — that's the
user grabbing the microphone."*

Types: `TtsEvent { UtteranceStarted, UtteranceEnded, UtteranceStopped }` (`:28-33`);
`Priority { Normal = 0, High = 1, Alert = 2 }` (`:39-43`); `SpeechEntry { text,
source_window, priority, spoken, repeats }` (`:46-53`) — `repeats` implements the
coalescing feature (*"'You hit!' x4 → spoken once with a count"*); `SpeechSubstitution
{ pattern: regex::Regex, replacement: String }` (`:56-59`) for pronunciation fixes.

API (≈35 `pub fn`): `new(enabled, rate, volume)` (`:126`), `set_filters(&gags,
&substitutions)` (`:224`), `enqueue` (`:253`), `speak_text_now` (`:316`), `speak_next`
(`:332`), `speak_previous` (`:357`), `speak_next_unread` (`:380`), `auto_play_next` (`:405`),
`handle_utterance_ended` (`:428`), `pump` (`:441`), `handle_utterance_stopped` (`:467`),
`stop` (`:510`), `toggle_mute` (`:525`), `clear_queue` (`:537`), plus getters and
rate/volume setters with increase/decrease helpers, and `available_voices` (`:647`).
The `#[cfg(feature = "tts")]` guards are threaded *inside* each method so the whole surface
compiles either way — that is the pattern that keeps `core/` Android-safe.

**Cena note:** TTS is `[web] Speech` on mobile (a phone settings section per
`book/src/frontends/web.md`), i.e. the *browser's* speech synthesis, not this crate. Two
implementations of one feature.

### 9.2 Window positioning (`src/window_position/`, 1,307 lines)

Persists the **terminal** window's screen geometry per character profile. Platform matrix
(`window_position/mod.rs:6-10`, file sizes measured):

| Platform | File | Lines | Mechanism |
|---|---|---:|---|
| Windows | `windows.rs` | 485 | Win32 APIs (`Cargo.toml` pulls `windows 0.58` with `Win32_UI_WindowsAndMessaging`, `Win32_Graphics_Gdi`, `Win32_System_Console`, …) |
| Linux/X11 | `linux.rs` | 225 | shells out to `xdotool` |
| macOS | `macos.rs` | 203 | AppleScript via `osascript` |
| **Wayland** | — | — | **"Not supported (Wayland prohibits window positioning)"** |
| storage | `storage.rs` | 87 | `load` / `save` |
| shared | `mod.rs` | 307 | `WindowRect` |

`WindowRect::is_sane()` (`mod.rs:39-42`) requires `width >= 100 && height >= 100`, because
*"ConPTY pseudo-windows and other broken handles yield zero/absurd rects; persisting or
applying those would shrink a real window to nothing."*

Used only for launcher-spawned TUI sessions (`main.rs:853-861`): *"Launcher-spawned sessions
own their console window, so the TUI restores/saves its size; manual runs leave the terminal
alone"* — resizing a terminal the user already owns (a tmux pane, a Windows Terminal tab)
*"would be rude."*

### 9.3 The launcher (`src/launcher/`, 3,522 lines + `gui/launcher.rs` 2,229)

| File | Lines |
|---|---:|
| `launcher/session_lifecycle.rs` | 1,314 |
| `launcher/flow.rs` | 809 |
| `launcher/ssh.rs` | 783 |
| `launcher/config.rs` | 581 |
| `launcher/mod.rs` | 35 |
| `gui/launcher.rs` (the egui window) | 2,229 |
| `gui/app/editors/launcher.rs` | 415 |

Purpose (`launcher/mod.rs:3-19`): VellumFE can already *attach* to a headless Lich
(`--without-frontend --detachable-client=PORT`), but *"every Lich-based mode needs Lich to be
already running… a Prime character strands you at RDP just to run one launch command."* So
over an existing WireGuard/Tailscale tunnel, VellumFE SSHes into the home PC, runs
`rubyw lich.rbw ... --detachable-client=PORT` **spawned detached so it survives the SSH
channel closing**, waits for the port to open, and hands `host:port` to the unchanged attach
path in `crate::network::LichConnection`. *"SSHD is the pre-existing listener; nothing new is
exposed to the internet."*

Security model (`launcher/mod.rs:21-31`), three rules worth carrying into Cena verbatim:

1. A **dedicated ed25519 keypair generated by VellumFE**. The private half lives only in the
   OS secure store (keyring on desktop, Keychain/Keystore on mobile) — *"never on a command
   line or in a config file."* The user pastes the public half into `authorized_keys` once.
2. **Host keys pinned on first use (TOFU)** to a launcher-owned known-hosts file; a changed
   key hard-fails with a MITM warning.
3. Because the key is purpose-built, it can be restricted server-side
   (`restrict,command="..."`) so a leaked key *"can only launch the game, not open a shell."*

Crate choice (`Cargo.toml:64-75`): `russh 0.62` with `default-features = false, features =
["ring", "flate2"]`. `aws-lc-rs` is dropped deliberately — it *"pulls aws-lc-sys, a C/CMake
build that is painful to cross-compile for iOS/Android; ring is prebuilt-friendly and the
mobile builds already share the ring lineage."* `rsa` is dropped because the launcher key is
ed25519 (server host keys may still be RSA — ring verifies those without the `rsa` signing
feature). Key generation uses `rand 0.10` to match russh's own `PrivateKey::random`.

The launcher is reachable from the GUI (a full egui window, `gui/launcher.rs`), from the
phone (`SSH launcher (cold-start Lich)` is a phone settings section per `web.md`), and from
dot-commands `.launch [character]` and `.launcher`.

---

## 10. Dot-commands — the in-client command surface

`src/core/app_core/commands.rs` (5,132 lines) holds `handle_dot_command`;
`src/core/app_core/command_help.rs` (665 lines) holds `COMMAND_HELP`, **101 `entry!`
invocations across 15 sections**.

The two files are kept in sync by a **tripwire test** at the bottom of `command_help.rs`.
Its header (`command_help.rs:1-8`) is worth quoting because it names the failure mode Cena
will hit too:

> Every dispatcher arm in `handle_dot_command` must have a row here and vice versa: the
> tripwire test at the bottom extracts the arm literals from commands.rs (between the COMMAND
> ARMS markers) and asserts both directions, so `.help` can no longer silently drift from
> reality — the failure mode the 2026-07 parity audit found (a dozen shipped commands missing
> from help).

Data model (`command_help.rs:10-24`): `CommandHelpEntry { names: &'static [&'static str],
args: &'static str, blurb: &'static str, notes: &'static [&'static str] }` where *"the first
[name] is canonical. Must match the dispatcher arm literals exactly"*; and
`CommandHelpSection { title, entries }`.

### The complete roster

**APPLICATION** (22) — `.quit`/`.q`, `.exit`, `.help`/`.h`/`.?`, `.version`/`.ver`,
`.goals [refresh|web]`, `.reconnect`, `.launch [character]`, `.launcher`, `.menu`,
`.settings`, `.reload [category]`, `.room`, `.mapdb [download|remove|repo <r>]`,
`.mappromote [<map-key>|all]`, `.data [status|reload|update <name>]`, `.jinx`, `.tts ...`,
`.portal [n]`, `.go2 <target>`, `.sorter [on|off|edit]`,
`.foreach ... in <bag>; cmd; cmd`, `.stop`.

**LAYOUTS** (8) — `.savelayout [name]`, `.loadlayout [name] [--keep-skin]`, `.layouts`,
`.undotabmove`, `.undolayout` (GUI; *"One step per autosave; up to 30 steps"*),
`.redolayout` (GUI), `.resize [name]` (GUI), `.anchorinfer` (GUI).

**WINDOWS** (18) — `.windows`, `.spellwatch [add|rem <n>|[n,n]|all]`, `.emptyhands`/`.eh`,
`.fillhands`/`.fh`, `.viewitem`/`.inspect <exist-id>`, `.find <name fragment>`,
`.drag <exist> left|right|drop|wear|feet | in|on|behind|underneath <dest>`, `.invsync`,
`.bestiary [...]`, `.addwindow`, `.deletewindow`/`.delwindow <name>`,
`.hidewindow`/`.hidewin [name]`, `.header`/`.footer`/`.leftbar`/`.rightbar [on|off|toggle]`,
`.editwindow`/`.editwin [name]`, `.rename <win> <title>`, `.border <win> <style> [color]`,
`.streams`, `.hidecontainers [title]`.

**TESTING** (1) — `.testline <text>` (*"Test highlights/squelch with fake game line"*).

**HIGHLIGHTS** (7) — `.highlights`/`.hl`, `.addhighlight`/`.addhl`,
`.alertpacks [list|on|off|show|approve|revoke] [name]`, `.edithighlight`/`.edithl [name]`,
`.savehighlights`/`.savehl [name]`, `.loadhighlights`/`.loadhl [name]`,
`.highlightprofiles`/`.hlprofiles`.

**KEYBINDS** (7) — `.keybinds`/`.kb`, `.menukeybinds`/`.menukb`, `.controller` (GUI),
`.addkeybind`/`.addkey`, `.savekeybinds`/`.savekb [name]`, `.loadkeybinds`/`.loadkb <name>`,
`.keybindprofiles`/`.kbprofiles`.

**HOTBARS** (1) — `.hotbars`/`.hotbar`.

**INDICATORS** (1) — `.indicators`/`.indicator`.

**COLORS** (8) — `.colors`/`.colorpalette`, `.addcolor`/`.createcolor`, `.uicolors`,
`.spellcolors`, `.addspellcolor`/`.newspellcolor`, **`.setpalette` (TUI only)**,
**`.resetpalette` (TUI only)**, `.harmony [scheme|schemes|skin <name>]`.

**THEMES & SKINS** (12) — `.themes`, `.settheme`/`.theme <name>`, `.edittheme`,
`.skins`, `.setskin`/`.skin <name>`, `.makeskin <name>`,
`.roomimages`/`.roomimg [on|off|set <image>|clear|list|edit]`,
`.creaturefield`/`.fieldcamera` (GUI), `.saveskin <name>` (GUI), `.reloadskin`,
`.exportskin <name>`, `.importskin <file>`.

**SHARING** (3) — `.packs`/`.packeditor`, `.uiexport <name> [parts]`,
`.uiimport <name|file>`.

**TAB NAVIGATION** (5) — `.nexttab`, `.prevtab`, `.gonew`/`.nextunread`,
`.menuof <exist_id> [noun]`, `.switcher`/`.palette` (GUI).

**TOGGLES** (3) — `.transparent`, `.snapdebug` (GUI), `.performance`/`.perf [dump]`.

**WINDOW LOCKING** (2) — `.lockwindows`/`.lockall`/`.unlockwindows`/`.unlockall [on|off]`,
`.lockwindow`/`.unlockwindow <window> [on|off]`.

**WEB & PHONES** (3) — `.webinfo` (*"Show the phone pairing URL + QR code"*),
`.reloadmacros` (*"Reload macros.toml and push to connected phones"*),
`.webui [page|off]` (GUI).

### Frontend-specific commands

Commands whose blurb explicitly names a frontend — these are the parity gaps a Cena port
must consciously decide about:

| Frontend | Commands |
|---|---|
| **TUI only** | `.setpalette`, `.resetpalette` (OSC 4 terminal palette) |
| **GUI only** | `.undolayout`, `.redolayout`, `.resize`, `.anchorinfer`, `.controller`, `.creaturefield`/`.fieldcamera`, `.saveskin`, `.switcher`/`.palette`, `.snapdebug`, `.webui`, and the whole `.header`/`.footer`/`.leftbar`/`.rightbar` shell-zone family |

**Cena note:** 101 commands with a machine-enforced help table is a strong pattern. The
natural Cena adaptation is to make each command a Lua-registrable entry with the same
`{names, args, blurb, notes}` shape, so a script can add a dot-command and it appears in
`.help` for free — but keep the tripwire, because a registry does not prevent drift, it
just moves where drift happens.

---

## 11. Mobile constraints — everything that limits Cena's mobile story

Collected from the whole tree, each with its evidence.

### 11.1 Build-level

| Constraint | Evidence |
|---|---|
| Mobile builds the **library**, never the binary. The bin has `required-features = ["tui","gui","desktop"]`. | `Cargo.toml:12-14` |
| Mobile = `--no-default-features`: core + parser + network + web + headless. Nothing else. | `Cargo.toml:146-148`, `android/rust/Cargo.toml:11-13`, `ios/rust/Cargo.toml:13-15` |
| `frontend/headless/` **must not** be feature-gated, or the Android build breaks. | `headless/mod.rs:11-13` |
| Losing `desktop` loses: clipboard (arboard), URL opening (open), process inspection (sysinfo), OS credential store (keyring), terminal password prompt (rpassword). | `Cargo.toml:157-159` |
| Losing `sound` and `tts` loses rodio playback and native speech; the browser provides both instead. | `Cargo.toml:155-156`; `web.md` "Speech" settings section |
| Losing `gamepad` loses gilrs — *"Desktop-only: the mobile builds get controllers via the browser Gamepad API."* | `Cargo.toml:102-104` |
| `core/`, `data/`, `parser.rs`, `parser/` may not name a desktop crate; enforced by a grep test. | `tests/architecture.rs:186-227` |
| `core/`, `data/`, `config/` may not name egui/eframe; enforced by a grep test. | `tests/architecture.rs:168-184` |
| Every platform integration must go through a feature-gated wrapper: `crate::clipboard`, `crate::platform`, `crate::tts`, `crate::sound`. | `tests/architecture.rs:190-193` |
| TLS: `native-tls` with **vendored OpenSSL** on Linux *and* Android, because *"Android has none."* Perl is a build requirement there. | `Cargo.toml:161-167` |
| `rustls` is **not an option**: *"eaccess.play.net only speaks TLS 1.2 with static-RSA key exchange (AES128-GCM-SHA256), which rustls refuses to implement."* | `Cargo.toml:52-56` |
| SSH must avoid `aws-lc-rs` — its `aws-lc-sys` C/CMake build is *"painful to cross-compile for iOS/Android."* Use `ring`. | `Cargo.toml:64-72` |
| Password sealing on non-desktop is `chacha20poly1305` (pure Rust, compiles everywhere) instead of the OS keyring. | `Cargo.toml:76-81` |
| Android ABIs: `arm64-v8a` + `x86_64` only, matching the cargo-ndk targets that populate `src/main/jniLibs`. | `android/app/build.gradle.kts:18-20` |
| `minSdk = 26` (Android 8.0), `compileSdk`/`targetSdk = 35`. | `android/app/build.gradle.kts:8-15` |
| Android lib is `cdylib` + JNI; iOS lib is `staticlib` + hand-written C header that must be kept in sync by hand. | `android/rust/Cargo.toml:8-9`; `ios/rust/Cargo.toml:9-11`; `ios/rust/include/vellum_core.h:3-5` |
| `std::env::set_var("VELLUM_FE_DIR", …)` in the embedded bootstrap is safe on edition 2021 only — *"revisit if the crate moves to 2024 (set_var becomes unsafe there because of concurrent readers)."* | `headless/embedded.rs` |

### 11.2 What the mobile shells can and cannot do

**Can:**

- Run the full game session locally (direct play.net login, every GS4 and DR world),
  attach to a remote Lich, or mirror a paired desktop VellumFE session — all three, in one
  app (`book/src/frontends/android.md`).
- Survive app-swipe mid-session via a foreground service holding a wakelock
  (`AndroidManifest.xml`, `CoreService.kt`).
- Scan a pairing QR with the camera and seal saved servers into the Android Keystore /
  iOS Keychain — the reason the native picker replaces the browser's in-page Remote tab
  (`android.md`, the ⚠ callout).
- Auto-recover a dropped connection with `BACKOFF = [1,2,5,10,30]` s ± 20 % jitter
  (`headless/runtime.rs:30-31`).
- Edit highlights (including **squelch** and redirect off / redirect-only / redirect+copy),
  colors, streams, speech, controller, touch wheel, the SSH launcher, and the *entire*
  desktop settings registry at character or global scope — writing to the **host's** config
  (`web.md:157-181`).
- Receive controller input via the browser Gamepad API (`Cargo.toml:102-104`).

**Cannot:**

- Author **layout and panel placement, window resizing, game keybinds, or macro
  `hidden_when` conditions** — *"Those stay on the desktop"* (`web.md:182-184`).
- Render skins: *"Skins apply to the Desktop GUI only — the terminal has no image
  pipeline"*, and the whole skin texture path is `egui`-only (`skins.md:7-10`,
  `gui/skin.rs:3-6`).
- Detach a window to a second OS window (egui multi-viewport, GUI-only,
  `gui/app/detached.rs`).
- Open external URLs from Rust — `platform::open_url` bails without the `desktop` feature
  (`platform.rs:17-21`); the WebView shell routes links to the system browser itself.
- Use the native `tts` crate, the `rodio` sound path, the OS keyring, or clipboard access
  from core.
- Reach the `Betrayer`, `MissingSpells`, `DialogPanel`, or `Performance` widgets — none has
  a web delta in `protocol.rs`.
- Reconnect forever: after `MAX_UNATTENDED_LOSSES = 2` consecutive drops **with zero user
  input in between**, the supervisor stops, because *"the game idle-kicks after ~30 minutes,
  and without this cap the supervisor would re-login all night (battery + pointless auth
  churn)"* (`headless/runtime.rs:33-37`).

### 11.3 Android-platform constraints

| Constraint | Evidence |
|---|---|
| The foreground service must be `specialUse`, **not** `dataSync` — *"a persistent game connection has no 6-hour budget; Android 15 caps dataSync services at 6h."* It carries a mandatory `PROPERTY_SPECIAL_USE_FGS_SUBTYPE` justification string. | `AndroidManifest.xml` |
| `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` is Play-Store-restricted; VellumFE is a **sideload** APK for that reason. iOS ships via TestFlight instead. | `AndroidManifest.xml`; `android.md`; `ios.md:59-63` |
| Cleartext HTTP must be permitted globally, because the paired desktop host is unknowable at build time; the real gate is a **WebView URL allowlist** of `127.0.0.1` + the one paired remote host. | `res/xml/network_security_config.xml`; `MainActivity.kt:102` |
| On Android 8–9 the WebView engine comes from **Chrome**, not Android System WebView — a blank client is a stale Chrome. | `android.md`, Tips |
| Deep links (`vellum://lich`, `vellum://remote`) **prefill and stop** — they never auto-connect. `launchMode="singleTask"` routes them through `onNewIntent`. | `AndroidManifest.xml` |
| The camera is `required="false"` so the app still installs on camera-less devices. | `AndroidManifest.xml` |
| The WebView needs `javaScriptEnabled`, `domStorageEnabled`, and `mediaPlaybackRequiresUserGesture = false` (for game sounds) — the last one is matched in the iOS `WebViewContainer`. | `MainActivity.kt:69-74` |

### 11.4 Network-topology constraints

The embedded server is **plain HTTP with a pairing token**, on purpose. Consequences stated
in the manual (`web.md`, `android.md` Tips):

- `bind = "127.0.0.1"` (the default) serves that PC only; a phone needs `bind = "0.0.0.0"`.
  `--web-port` enables the server but **does not change `bind`**.
- *"Security: keep the port on a network you trust. Pairing keeps strangers out, but the
  traffic is plain HTTP. For play away from home use Tailscale or WireGuard — never forward
  this port to the open internet."* `.webinfo` reprints this every run.
- *"The phone needs a private path to your PC. Home Wi-Fi, Tailscale, or a VPN. Never the
  open internet, for either the Lich port or the VellumFE web port."*
- Unpinned instances treat the configured port as a **base** and walk upward to the next
  free one, which is precisely why `embedded::start()` returns *the listener's real bound
  port* rather than the configured one (`embedded.rs:19-22`).
- One pairing covers all characters on a device; unpaired connections are refused and
  repeated bad attempts get rate-locked (`web.md`).
- Two QR codes, not one: `http://…/#token=…` for a browser, `vellum://remote?…` for the
  native apps. Scanning the wrong one silently does nothing.

### 11.5 What Cena should take from this

1. **The mobile path costs ~4,700 lines of Rust + ~3,200 lines of shell, because the phone
   renders a web UI the desktop already had to build.** There is no third native UI. If Cena
   wants mobile, the cheapest architecture VellumFE found is: core → delta protocol → one
   browser client → thin platform shell that only starts the core and points a WebView at
   `127.0.0.1`.
2. **Make the mobile constraint mechanically enforced from day one.** `core_is_android_safe()`
   is 40 lines of grep and it is the only reason the Android build stays possible while 140k
   lines of desktop frontend grow beside it.
3. **Shape the platform shim as a 21-line module with two `#[cfg]` arms**, not as scattered
   conditionals.
4. **Return the *actual* bound port and token from the bootstrap**, not the configured ones —
   port-walking makes the configured value a lie, and `tests/embedded_runtime.rs` exists
   entirely to pin that.
5. **The forked-toolkit numpad problem is real and unavoidable** for this genre. Decide it
   before picking a UI stack, not after.
6. **Keep one golden-vector file per behavior that exists in both Rust and JS**
   (`tests/wheel_parity.cjs` + `tests/data/wheel_golden.json`), or the two clients will
   diverge silently.

---

## 12. Open items

- **UNVERIFIED:** exact TUI coverage of `DialogPanel`, `MultiAccount`, `CreatureField` — see
  §7.2. Needs an exhaustive read of `src/frontend/tui/sync.rs` (3,107 lines).
- **UNVERIFIED:** whether the web/Despana client renders `Performance`, `MissingSpells`, or
  `Betrayer` through a generic text path rather than a dedicated delta. I confirmed no
  dedicated `encode(...)` exists for them in `protocol.rs`; I did not read all 8,449 lines of
  `assets/app.js` to rule out a text-based fallback.
- **UNVERIFIED:** the 33 `book/src/widgets/*.md` pages map to widget *families*, not 1:1 to
  the 39 `WidgetType` variants. I did not build the exact page→variant mapping; that needs a
  read of each page's front matter.
Two items initially flagged here were resolved during the pass and are recorded above
rather than left open:

- `VALID_TYPES` omits exactly one name, `"items"` — see §7.1.
- The `WidgetType::Label` seen in the GUI is **egui's own** `egui::WidgetType::Label`
  (accessibility metadata at `src/frontend/gui/app/widgets/text.rs:341`,
  `egui::WidgetInfo::labeled(...)`), not VellumFE's enum. The 39-variant count stands.
