# VellumFE Core & Architecture — Inventory for the Cena Port

**Date:** 2026-09-17
**Source tree:** `E:/Cena/reference/VellumFE` @ `Cargo.toml` `version = "0.3.0-beta.51"` (`Cargo.toml:` version line).
**Files read for this document:** `tests/architecture.rs` (359 lines, read in full), `CLAUDE.md` (read in full), `src/lib.rs`, `src/main.rs`, `src/network.rs`, `src/parser.rs`, `src/parser/text.rs`, `src/parser/handlers.rs`, `src/core/mod.rs`, `src/core/state.rs`, `src/core/character_state.rs`, `src/core/messages.rs`, `src/core/app_core/mod.rs`, `src/core/app_core/state.rs`, `src/core/app_core/commands.rs`, `src/data/mod.rs`, `src/data/widget.rs`, `src/data/ui_state.rs`, `src/data/view_kind.rs`, `src/config.rs`, `src/config/paths.rs`, `src/config/settings.rs`, module doc-headers of the core services listed in §7, plus `GemStoneIV-XML-Elements.md`. Lich-side citations come from `E:/Cena/reference/lich-5`.

**Scale.** `find src -name "*.rs" | xargs wc -l` → **324,605 lines** total Rust under `src/`. The layers Cena inherits:

| Layer | LOC | Files |
|---|---:|---|
| `src/core/` | 102,438 | 128 |
| `src/config/` (+ `config.rs` 595) | 32,099 | 30 |
| `src/data/` | 8,359 | 11 |
| `src/parser/` (+ `parser.rs` 1,063) | 7,857 | 7 |
| `src/core/app_core/` (subset of core) | 26,825 | 29 |
| `src/network.rs` | 1,476 | 1 |
| `src/main.rs` | 1,375 | 1 |
| `src/lib.rs` | 22 | 1 |

`src/lib.rs:1` states its purpose outright: *"Library exports for reusing core modules in auxiliary binaries (e.g., migration tools)."* It is a flat `pub mod` list of 21 modules with no logic — the binary and the library share one surface.

---

## 1. The enforced architecture (`tests/architecture.rs`)

This is the file Cena's "strict separation of responsibility" principle actually comes from. It is **359 lines of plain `fs::read_to_string` + substring scanning** — no proc macros, no dependency graph analysis, no `cargo-deny`. One helper, `scan_dir` (`tests/architecture.rs:10-26`), walks a directory recursively, reads every `.rs` file, and records `path:line: text` for any line containing any of a list of "needles". Every rule below is that helper plus an `assert!(violations.is_empty(), ...)`.

The whole mechanism is worth stating plainly for Cena, because its cheapness is the point: **the rules are enforced by grep, and they still hold.** A Cena equivalent needs no new tooling.

### Rule 1 — `core/` and `data/` must not reference `frontend/`

`tests/architecture.rs:28-46`, test `core_and_data_do_not_reference_frontend`.

```rust
for layer in ["core", "data"] {
    scan_dir(
        &src.join(layer),
        &["crate::frontend", "super::frontend"],
        &mut violations,
    );
}
assert!(
    violations.is_empty(),
    "core/ and data/ must not reference frontend/ (see CLAUDE.md architecture rules).\n\
     Pure input/event data types belong in data/ (e.g. data/input.rs).\n\
     Violations:\n{}",
    violations.join("\n")
);
```

The failure message carries the remedy: input/event types that *look* like frontend concerns live in `data/input.rs` (850 lines) instead. This is why `KeyEvent`, `KeyCode` and `MouseEvent` are Vellum's own types, not `crossterm`'s or `egui`'s.

### Rule 2 — `gui/` must not reference `tui/` or terminal crates

`tests/architecture.rs:48-66`, test `gui_does_not_reference_tui`. Needles:

```rust
scan_dir(
    &src.join("frontend/gui"),
    &["crate::frontend::tui", "super::tui", "ratatui", "crossterm"],
    &mut violations,
);
```

Comment at `tests/architecture.rs:50-53`: *"The GUI and TUI are peer frontends sharing core/, data/, and frontend/common/. Anything both need belongs in one of those shared layers (e.g. parse_color_flexible lives in frontend/common/color.rs), never imported across frontends."* Note this bans the *crate names* `ratatui` and `crossterm`, not just the module path — so the GUI cannot even reach the terminal libraries directly.

### Rule 3 — `gui/` must not touch `TuiPlacement` / `.placement`

`tests/architecture.rs:68-92`, test `gui_does_not_reference_tui_placement`. Needles: `[".placement", "TuiPlacement"]`.

This is the most instructive rule for Cena because it documents a *bug class*, not a taste preference. The comment (`tests/architecture.rs:69-79`):

> TuiPlacement (src/config/widgets.rs) is TUI-only per-window state (terminal-cell geometry, TUI show/hide selection), flattened into WindowBase's on-disk shape but split out as its own type specifically so the GUI cannot reach it — the GUI has its own selection/placement (hidden_tabs, main_window_rects/zones/groups). Reaching `.placement` or naming `TuiPlacement` from GUI code reintroduces the coupling that **caused loading a layout in one frontend to wreck the other's tab set**. Fields behind this boundary: row/col/rows/cols/min_rows/max_rows/min_cols/max_cols/visibility/show_title/title_position.

The technique generalizes: **one on-disk shape, split into per-frontend Rust types, and a grep rule that keeps each frontend inside its own type.** Cena will face exactly this with desktop vs. mobile layout.

### Rule 4 — `gui/` must not drive the TUI's show/hide selection

`tests/architecture.rs:94-118`, test `gui_does_not_use_tui_show_hide_selection`. Needles: `["set_known_window_shown(", ".hide_window(", ".show_window("]`.

Comment (`tests/architecture.rs:95-102`): `placement.visibility` is the TUI's *selection* of which catalog windows it shows; the GUI's selection is `hidden_tabs` in its own `layout_v1.json`. Core's `set_known_window_shown` / `hide_window` / `show_window` all write the TUI slot *plus* emit a `"Window 'X' hidden"` message and a `.savelayout` tip — so a GUI call would produce TUI-flavoured chatter. The sanctioned GUI path is `VellumGuiApp::gui_set_tab_shown`, and shared dialog-popup bookkeeping goes through `AppCore::sync_dialog_popup_allow_set`.

### Rule 5 — `gui/` must not assign `config.active_theme`, and may read it in exactly one file

`tests/architecture.rs:120-165`, test `gui_does_not_assign_config_active_theme`. This rule has **two** assertions.

Write side, needle `["config.active_theme ="]`:

```rust
assert!(
    violations.is_empty(),
    "frontend/gui/ must not assign config.active_theme (the TUI's theme slot). \
     Write the GUI slot via VellumGuiApp::set_gui_theme_id instead.\nViolations:\n{}",
    violations.join("\n")
);
```

Read side (`tests/architecture.rs:145-164`): scans for the bare `config.active_theme`, then filters out lines starting with `//` (prose mentions are fine) and filters out any path containing `app/theme.rs` — a **single-file allowlist**, implemented with `v.replace(char::from(92u8), "/")` to normalize Windows backslashes. Remaining hits fail:

```rust
assert!(
    stray.is_empty(),
    "config.active_theme may only be read in the documented first-run \
     fallback (VellumGuiApp::gui_theme_id, src/frontend/gui/app/theme.rs).\
     \nViolations:\n{:#?}",
    stray
);
```

Principle: *theme is presentation, so it is per-frontend.* `config.active_theme` (per-character `config.toml`) is the TUI's slot; `ui_settings.active_theme` (`layout_v1.json`) is the GUI's.

### Rule 6 — `core/`, `data/` and `config/` must not reference `egui`/`eframe`

`tests/architecture.rs:167-181`, test `core_and_data_do_not_reference_egui`. Needles `["egui", "eframe"]` across **three** layers — note `config` is included here but *not* in Rule 1, so `config/` may name `frontend` but may not name a UI toolkit. Comment: *"Rendering stays in frontends: core/, data/, and config/ must compile without any UI toolkit."*

### Rule 7 — `core/` is Android-safe

`tests/architecture.rs:183-231`, test `core_is_android_safe`. This is the rule that makes Cena's mobile requirement tractable, so it deserves the full needle list (`tests/architecture.rs:198-206`):

```rust
let needles = &[
    "arboard::",
    "open::that",
    "sysinfo::",
    "keyring::",
    "rpassword::",
    "rodio::",
    "use tts::",
];
```

Scanned over `core/`, `data/`, **and** `parser.rs` explicitly (read as a file, lines enumerated, `src/parser.rs:{n}` synthesized at `tests/architecture.rs:211-220`), **and** `src/parser/` recursively — the comment at `tests/architecture.rs:221-222` notes the parser split carried the same obligation down into the submodules.

The comment (`tests/architecture.rs:184-192`) names the escape hatches precisely:

> The Android build is `--no-default-features`: core/, data/, and the parser must never reference desktop-only crates directly. Platform integrations go through the feature-gated wrapper modules instead: `crate::clipboard` (arboard), `crate::platform` (open), `crate::tts`, `crate::sound` (rodio). The remaining desktop crates (sysinfo, keyring, rpassword) are gated at their designated sites: `src/performance.rs`, `src/config/profiles.rs`, `src/network.rs`, `src/frontend/web/server.rs`.

So: seven banned crates, four wrapper modules, four named gate sites. `CLAUDE.md` adds the CI gate — `cargo test --no-default-features` — and states the feature split: `default = ["tui", "gui", "sound", "tts", "desktop", "gamepad"]`, with Android/iOS building the library as core + parser + network + web + headless.

### Rule 8 — `src/config.rs` stays a facade (line cap 700)

`tests/architecture.rs:233-251`, test `config_root_stays_a_facade`:

```rust
const MAX_CONFIG_ROOT_LINES: usize = 700;
assert!(
    lines <= MAX_CONFIG_ROOT_LINES,
    "src/config.rs has {lines} lines (limit {MAX_CONFIG_ROOT_LINES}). \
     Move new types/impls into the appropriate src/config/ submodule."
);
```

Actual: `src/config.rs` is **595 lines** — 105 of headroom.

### Rule 9 — split parents stay facades (per-file line caps)

`tests/architecture.rs:253-285`, test `split_parents_stay_facades`. The cap table verbatim (`tests/architecture.rs:261-268`):

| File | Cap | Actual (`wc -l`) | Headroom |
|---|---:|---:|---:|
| `src/frontend/gui/app.rs` | 3600 | — (out of scope) | — |
| `src/core/app_core/state.rs` | 2100 | 2,089 | 11 |
| `src/frontend/tui/window_editor.rs` | 1700 | — (out of scope) | — |
| `src/core/messages.rs` | 800 | 793 | 7 |
| `src/frontend/gui/app/widgets.rs` | 700 | — (out of scope) | — |
| `src/parser.rs` | 1100 | 1,063 | 37 |
| `src/config.rs` | 700 (Rule 8) | 595 | 105 |

The comment states the policy without ambiguity (`tests/architecture.rs:254-260`): *"The residual parents hold the type definitions, dispatchers, and (for now) the test modules — new methods belong in the matching submodule. Limits are the current size rounded up a little; if one trips, move code down into a submodule instead of raising the cap."* `CLAUDE.md` repeats it in bold: **"If a cap trips, move code down, don't raise the cap."**

Note the headroom column: three of the four in-scope caps sit within 40 lines of their limit. This is a *deliberately tight ratchet*, not a generous ceiling.

### Rule 10 — catalog access goes through the seams

`tests/architecture.rs:287-327`, test `catalog_access_goes_through_the_seams`. Ten needles (`tests/architecture.rs:294-305`):

```rust
let needles = [
    "dialog_id_to_template",
    "stream_id_to_template",
    "id_has_widget_template",
    "Config::list_window_templates",
    "Config::template_game_type",
    "Config::get_templates_by_category",
    "Config::get_addable_templates_by_category",
    "Config::get_visible_templates_by_category",
    "Config::get_layout_templates_by_category",
    "Config::get_window_template",
];
```

Scanned over **all of `src/`**, then filtered: allowed if the path contains `/src/config`, or ends with `/src/core/local_catalog` or `/src/core/view_resolver`; comment-only lines exempt. The first three are *deleted outright* — naming them anywhere is an error. The rest are `Config` methods production code must reach via the seam, "so catalog swaps stay provable."

This is a **migration scaffold**: `src/core/local_catalog.rs:1-12` describes itself as a façade where *"today every function is a PURE DELEGATION — output byte-identical, pinned by the Phase 0 golden fixture and the menu-content tests. Phase 6 swaps the delegation for the real `CatalogEntry` table and deletes the 55-arm template match behind this unchanged seam."* Cena should note the pattern: introduce the seam, prove it is a no-op with a golden test, enforce it with grep, *then* swap the implementation.

### Rule 11 — `server_time_offset` has exactly one owning field

`tests/architecture.rs:329-359`, test `server_time_offset_has_a_single_owning_field`. Needle: `["pub server_time_offset:"]` over all of `src/`, asserting `hits.len() == 1` and that the one hit is in `/core/messages.rs`.

The doc comment (`tests/architecture.rs:329-339`) is a post-mortem:

> The server clock offset must have exactly ONE owning field: the one on `MessageProcessor`, written by the `<prompt time=...>` arm. A second copy lived on AppCore from the Beta 2 rewrite until 2026-09-16 and was never assigned, so **every GUI countdown silently rendered with an offset of 0 and displayed roundtime was inflated by the machine's clock skew**. Two fields that must agree will drift again; one field cannot.
>
> Frontends read the live value via `AppCore::server_time_offset()`. Passing the offset into a pure helper as a parameter stays fine — what must never come back is a struct that STORES its own copy.

The needle is chosen with care: *"A stored copy is a struct field declaration. Function parameters share the `name: i64` shape but never carry the `pub ` prefix, so this needle matches owning fields only"* (`tests/architecture.rs:346-348`). The absence is even commented *in the struct it was removed from* — `src/core/app_core/state.rs:82-88` carries a nine-line tombstone explaining why `AppCore` has no such field.

**For Cena:** this rule is the template for single-ownership of any derived clock or cached value. It is dated (2026-09-16) and fixed a real rendering bug.

### What the architecture test does *not* enforce

Worth stating so Cena does not over-credit it:

- It is **substring matching on source text**, so it can be evaded by aliasing (`use crate::frontend as f;`) or by string concatenation. No test checks for that.
- Nothing enforces the Rule 1 direction for `config/` (Rule 6 covers `config/` for egui only). `config/` may name `frontend`.
- Line caps cover **6 files**. The largest core file, `src/core/travel/executor.rs` at **6,654 lines**, is uncapped. So is `src/core/app_core/commands.rs` at **5,132**.
- There is no test that `frontend/tui` avoids `egui`, only the reverse.
- UNVERIFIED: whether CI runs `cargo test --test architecture` on every PR. `CLAUDE.md` says to run it "after structural changes" and that CI gates `cargo test --no-default-features`; I did not read `.github/workflows/` to confirm the architecture test is wired in. To verify: read `.github/workflows/*.yml`.

---

## 2. Data flow: Network → Parser → AppCore → data layer → Frontend

`CLAUDE.md` draws it as:

```
Network (TCP) → Parser (XML) → Core (AppCore) → Data Layer → Frontend (TUI)
                                    ↑
                              User Input ←────────────────────┘
```

Each hop, with the type that crosses it and the file that owns it:

| # | Hop | Type crossing | Owning file | Notes |
|---|---|---|---|---|
| 1 | Socket → runtime | `ServerMessage` over `tokio::sync::mpsc` | `src/network.rs:25-29` | 3 variants: `Text(String)`, `Connected`, `Disconnected` |
| 2 | Runtime → core | `&str` (one line) | `src/frontend/tui/runtime.rs:1019-1036` | calls `app_core.process_server_data(&line)` |
| 3 | Core → parser | `&str` → `Vec<ParsedElement>` | `src/core/app_core/state.rs:1398` | `self.parser.parse_line(data)` |
| 4 | Parser → state mutation | `&ParsedElement` | `src/core/app_core/state.rs:1400` | `self.process_element(&element)?` |
| 5 | Text assembly | `Vec<TextSegment>` → `StyledLine` | `src/core/messages.rs` (`MessageProcessor`) | `flush_current_stream_with_tts(&mut self.ui_state, ...)` (`state.rs:1402-1403`) |
| 6 | Core → data layer | field writes on `GameState` / `UiState` | `src/core/state.rs`, `src/data/ui_state.rs` | frontends never see `ParsedElement` |
| 7 | Data layer → frontend | reads of `AppCore.game_state` / `AppCore.ui_state` | frontend files | pull, not push; `CLAUDE.md` rule 6 |
| 8 | Frontend → core (input) | `data::input::KeyEvent` / `MouseEvent` | `src/data/input.rs` (850 lines) | frontend-neutral input types exist *because of* Rule 1 |

### Hop 1 — `src/network.rs`

```rust
pub enum ServerMessage {
    Text(String),
    Connected,
    Disconnected,
}
```
(`src/network.rs:25-29`)

Backpressure is explicit and documented (`src/network.rs:31-34`):

```rust
/// Capacity of the server→UI message channel. When full, the network read
/// task blocks on send and TCP flow control provides backpressure, instead
/// of the queue growing without bound while the UI is stalled.
pub const SERVER_CHANNEL_CAPACITY: usize = 4096;
```

Two transports share this enum: `LichConnection` (`src/network.rs:37`, `start` at `:347`) and `DirectConnection` (`src/network.rs:302`, `start` at `:378`). `AuthFailed(pub String)` (`src/network.rs:45`) is a marker error distinguishing credential rejection from transport failure, so the headless reconnect supervisor stops retrying rather than hammering eAccess with a wrong password. `DirectConnectConfig` (`src/network.rs:57-64`) carries `account`, `password`, `character`, `game_code`, `data_dir`. A `RawLogger` (`src/network.rs:75-79`) tees pre-parse XML to disk over a `SyncSender` with a dropped-line counter.

Direct eAccess details from `CLAUDE.md`: TLS to `eaccess.play.net:7910` via `native-tls` with SNI disabled (the server only speaks TLS 1.2 static-RSA `AES128-GCM-SHA256`, which rustls refuses to implement); challenge-response obfuscation `((password[i] - 32) ^ hashkey[i]) + 32`; login payload `A\t{account}\t{encoded_password}\n`. **Critical noted constraint:** `send_line` must write message + newline in a *single* TLS write.

### Hop 2/3/4 — `process_server_data`

`src/core/app_core/state.rs:1347-1360` is a thin timing wrapper:

```rust
pub fn process_server_data(&mut self, data: &str) -> Result<()> {
    if self.jinx_nudge_pending {
        self.jinx_nudge_pending = false;
        self.emit_stale_data_nudge();
    }
    let parse_start = std::time::Instant::now();
    let result = self.process_server_data_inner(data);
    self.perf_stats.record_parse(parse_start.elapsed());
    result
}
```

Note the comment at `:1354-1355`: *"Parse timing lives here so every frontend gets it for free — runtimes must not also time this call (double counting)."* This is the single funnel: `src/core/app_core/state.rs:1348-1349` says *"every frontend funnels through here."*

`process_server_data_inner` (`src/core/app_core/state.rs:1392`) parses, dispatches each element, flushes, then performs a fixed **drain sequence** of side effects that were *deferred* because they were computed in a context without `&mut GameState`:

1. `message_processor.pending_sounds` → `game_state.queue_sound` (`:1406-1408`)
2. `queue_highlight_rumbles()` — haptics (`:1410`)
3. `apply_pending_status_actions()` — highlight-driven custom statuses (`:1412`)
4. `apply_pending_alerts()` — overlay alerts (`:1414`)
5. `message_processor.pending_evidence` → `evidence.record(uid, ...)` attributed to the current `nav_room_id` (`:1417-1426`)

**This deferred-drain pattern is the single most important structural idea in Vellum's core for Cena to copy.** The text-flush path (`MessageProcessor::flush_*`) does not hold `GameState`, so anything it discovers is staged in a `pending_*` field on `MessageProcessor` and applied at the prompt. `src/core/messages.rs` declares at least ten such staging fields, each with a comment saying why: `pending_container_ingest` (`:179`), `pending_ready_stow` (`:182` — "drained into `game_state.objects` at the prompt"), `pending_move_feedback` (`:186` — "so the walk executor sees each one exactly once"), `pending_creature_effects` (`:190`), `pending_recent_lines` (`:197`), `pending_character_lines` (`:206`), `pending_day_pass_lines` (`:209`), `pending_silver` (`:212`), `pending_group` (`:217-220`).

Ordering is load-bearing in at least two of them. `src/core/messages.rs:214-216`: *"Applied to `game_state.group` at the prompt IN ORDER — a `group` reply's roster line and its status sentinel must not be reordered."* And `:207-209`: day-pass lines apply "IN ORDER (expiry follows desc)".

`MessageProcessor` also owns the frame-boundary flags `chunk_has_main_text` (`:223`) and `remote_chunk_has_story_text` (`:232`), which exist because the remote (phone) feed and the local main window can disagree about whether anything happened since the last prompt — see `:224-236` for the reasoning about stranded prompt separators.

And it owns the one true `game_line_no` (`src/core/messages.rs:194`), described at `:191-193` as *"Monotone count of flushed game lines — the stamp on move-feedback events (Lich's `room_count` guard generalized): the executor ignores reactive events whose line predates its last send."* Explicit Lich lineage.

One performance guard worth noting: `capture_recent_lines` (`src/core/messages.rs:202`) is **off by default**; `:198-202` explains *"awaits are the only consumer, and copying every game line into a ring for a feature nobody is using is pure waste. `tick_travel` raises it when travel starts and drops it when travel ends."*

### Hop 5 — highlights are applied once, in core

`src/core/highlight_engine.rs:1-6`:

> Core-level highlight engine for applying highlight patterns to text. This module applies highlights ONCE during message processing (in MessageProcessor), before text reaches any frontend. **Text arrives at widgets pre-colored.** NO frontend imports — works directly with `TextSegment` from the data layer.

2,260 lines. Uses `aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind}` plus `regex::Regex` (`src/core/highlight_engine.rs:10-11`) — literal patterns go through Aho-Corasick, regex patterns through `regex`. Exports `CoreHighlightEngine`, `HighlightResult`, `DeferredReplacement`, `apply_deferred_for_window` (`src/core/mod.rs:61-64`) and `SoundTrigger` (`:13-16`).

**Consequence for Cena:** with 4 frontends, applying highlights per-frontend would be 4× the regex work and 4 chances to drift. Vellum pays it once. Any Cena Lua highlight API should hook here, not at render.

### Hop 7 — the pull model

`CLAUDE.md` rule 6: *"Frontends render by reading `AppCore.ui_state` and `AppCore.game_state`."* There is no observer registration, no event bus to frontends, no dirty-rect protocol. Instead, core sets coarse flags that frontends poll: `needs_render` (`src/core/app_core/state.rs:+57` region), `running`, `chunk_has_main_text`, `chunk_has_silent_updates`, `room_window_dirty`, plus **generation counters** on individual state structs (see §5) so a frontend can cheaply detect "this roster changed".

The one exception is the remote/web path: `src/core/remote.rs:1-16` describes `RemoteSink`, held as an `Option` inside `MessageProcessor` (*"None when `[web]` is disabled — the cost is one branch per finalized line"*), which **pushes** finalized styled lines into a shared ring (`src/data/remote_buffer.rs`, 210 lines) and broadcasts `RemoteDelta::Text`, sharing one `Arc<StyledLine>` between both. State deltas (vitals, room, hands, indicators, roundtime) are coalesced and flushed **once per message batch by diffing against the last flush**. The web server task holds only channel ends: *"Channels and this small shared ring are the only coupling — the server never touches `AppCore`."*

**For Cena:** this is the proven shape for a mobile/remote frontend that is not in the same process — a ring buffer for scrollback, a broadcast channel for deltas, a `watch` of latest state for connect-time snapshots, and diff-based coalescing rather than per-field notification.

---

## 3. The parser

`src/parser.rs` is a 1,063-line facade (cap 1,100) over `src/parser/`:

| File | LOC | Role (from `CLAUDE.md` architecture block) |
|---|---:|---|
| `src/parser/tests.rs` | 4,393 | tests |
| `src/parser/dialogs.rs` | 1,403 | dialog tag family |
| `src/parser/handlers.rs` | 1,180 | tag handlers |
| `src/parser/text.rs` | 514 | entities / tag-name extraction / stream glue |
| `src/parser/links.rs` | 275 | `<a>` / `<d>` link metadata |
| `src/parser/numbers.rs` | 55 | numeric attribute parsing |
| `src/parser/builders.rs` | 37 | in-flight multi-line captures |
| **`src/parser.rs`** | **1,063** | `ParsedElement` enum + `XmlParser` struct + dispatch chain |

Non-test parser code: **4,527 lines**.

### `KNOWN_WIRE_TAGS`

`src/parser/text.rs:174-192`. **116 entries**, declared `pub(super) const KNOWN_WIRE_TAGS: &'static [&'static str]`. Doc comment (`src/parser/text.rs:167-173`):

> Every element name the Wrayth wire protocol is known to emit, **whether or not we handle it**. Union of our dispatch chain and the tag set the Saga client recognizes. Unknown names indicate new server markup and are passed through as visible text (never silently dropped) so protocol changes announce themselves. **MUST stay sorted: binary search.**

Lookup is `Self::KNOWN_WIRE_TAGS.binary_search(&name).is_ok()` (`src/parser/text.rs:194-196`). Sortedness is itself tested — `src/parser/tests.rs:205-216` asserts pairwise ordering with the message `"KNOWN_WIRE_TAGS out of order: {:?} >= {:?}"`, and spot-checks `is_known_wire_tag("pushStream")`, `("dialogData")` true and `("unknownFutureTag")` false.

The full set, verbatim:

```
FEVersion, LaunchURL, LichWebUI, PantheonStatus, a, action, annotate, app, b,
br, castTime, celebration, checkBox, clearContainer, clearDynaStream,
clearStream, cli, closeButton, closeDialog, closedialog, cmdButton,
cmdlist, cmdtimestamp, compDef, compass, component, container,
continuation, crtrStatus, d, deleteContainer, description, dialogData,
dir, dropDownBox, dynaStream, editBox, endSetup, exists, exits,
exposeContainer, exposeDialog, exposeStream, extra, flag, forcesave,
getSkinVersion, group, hScrollBar, hostile, i, image, indicator, inv,
inventoryManager, inventoryViewItem, label, launchURL, left, link,
macros, menu, menuImage, menuLink, mi, mode, monopolize, name, nav,
nomenu, noverbupdates, objective, objectives, openDialog, opendialog,
output, palette, playerID, players, popBold, popInputState, popStream,
popup, preset, presets, progressBar, prompt, pulse, pushBold,
pushInputState, pushStream, radio, resource, result, reward, right, roomDesc,
roommeta, roundTime, sentSettings, sep, settings, settingsInfo, skin,
spell, stream, streamId, streamWindow, string, switchQuickBar, timer,
tipInfo, upDownEditBox, updateverbs, vScrollBar, worldEvent
```

Note it contains **case variants as separate entries** (`closeDialog`/`closedialog`, `openDialog`/`opendialog`, `LaunchURL`/`launchURL`) — the wire is inconsistent and the table matches it literally rather than lowercasing.

### How unknown tags are handled

The dispatch chain in `src/parser.rs` is a long `if/else if` on `tag.starts_with(...)`. Its terminal `else` (`src/parser.rs:1037-1051`) is the whole unknown-tag policy:

```rust
// Known wire tags we deliberately don't act on: swallow quietly.
// Unknown names mean new server markup — pass through as visible
// text so nothing is ever silently dropped.
else {
    let name = Self::tag_name(tag);
    if Self::is_known_wire_tag(name) {
        tracing::debug!("Parser: known unhandled tag <{}>", name);
    } else {
        tracing::warn!(
            "Parser: unknown tag passed through as text: {}",
            &tag[..tag.len().min(120)]
        );
        text_buffer.push_str(tag);
    }
}
```

Three-way outcome, not two:

| Case | Behaviour | Log level |
|---|---|---|
| Handled tag | produces `ParsedElement`(s) | — |
| In `KNOWN_WIRE_TAGS`, no handler | swallowed | `debug!` |
| Not in `KNOWN_WIRE_TAGS` | **rendered as literal text** | `warn!` |

Two earlier arms sit just above it: a diagnostic catch for `tag.contains("dropDown") || tag.contains("dropdown") || tag.contains("dDB")` that `warn!`s "Unhandled dropdown-like tag" (`src/parser.rs:1023-1028`, with a comment noting the case-sensitive checks avoid a per-tag `to_lowercase` allocation), and a silent-ignore list for `<compDef `, `</compDef>`, `<streamWindow `, `<skin ` (`src/parser.rs:1030-1035`).

`CLAUDE.md` restates the contract as a **parser invariant**: *"Nothing is silently dropped: unknown tag names render as literal text with a `warn!`; known-but-unhandled tags swallow with a debug log. New wire tags must be added to the set."* And it gives the field-diagnostic recipe: `grep -a "\[parser\]" ~/.vellum-fe/vellum-fe.log`.

**For Cena this is a strong, cheap policy and should be kept verbatim:** a protocol change becomes visible garbage plus a warning, never a silent loss.

### The other parser invariants (`CLAUDE.md`, "Parser Invariants")

- **Golden snapshot.** `tests/parser_characterization.rs` runs every fixture and diffs against `tests/data/parser_golden.snap`. Regenerate with `UPDATE_PARSER_GOLDEN=1 cargo test --test parser_characterization`; *any* behaviour change must appear as a reviewed diff committed with the change. Window templates use the same workflow with `UPDATE_GOLDEN=1 cargo test --test template_characterization`.
- **Streams are a stack.** `pushStream` nests; a pop restores the enclosing stream via `ParsedElement::StreamResume` — a re-route **without** arrival side effects. `<prompt>` force-closes any stream still open (warn = eaten popStream upstream) and resets all style/bold/mono state.
- **Multi-line captures.** `dialogData`/`openDialog`/`component`/`compDef`/`worldEvent`/`compass` may close on a later line (`PairedCapture`, `src/parser/builders.rs`); a prompt mid-capture discards with a warn; **256 KiB cap**. `prompt`/`left`/`right`/`spell`/`inv` are same-line-only *by policy*.
- **Mangled markup.** A `<` preceded by `$` is broken server escaping — rendered as literal text, never interpreted.
- **Entities.** 5 named + numeric (`&#nnn;` / `&#xhh;`), decoded **exactly once**; `decode_entities_stable` is for double-encoded display titles ONLY. Unknown entities copy the `&` through verbatim (`src/parser/text.rs:292`).
- **Control characters.** `strip_control_chars` (`src/parser/text.rs:198-211`) removes C0 except `\t`/`\n`, plus DEL, with a no-allocation fast path.

### `XmlParser` state

`src/parser.rs:468-511`. The struct is almost entirely **stacks**, which is what makes the nesting invariants hold:

| Field | Type | Purpose |
|---|---|---|
| `current_stream` | `String` | mirrors top of `stream_stack`, or `"main"` |
| `stream_stack` | `Vec<String>` | open stream redirects, innermost last (`:470-472`) |
| `presets` | `HashMap<String,(Option<String>,Option<String>)>` | preset id → (fg, bg) |
| `color_stack` / `preset_stack` / `style_stack` | `Vec<ColorStyle>` | nested styling |
| `bold_stack` | `Vec<bool>` | `pushBold`/`popBold` |
| `mono_output` | `bool` | inside `<output class="mono"/>` (`:480-483`) |
| `link_depth`, `spell_depth` | `usize` | nesting counters |
| `link_stack` | `Vec<LinkData>` | one per open `<a>`/`<d>`; innermost wins (`:489-494`) |
| `link_pushed_color` | `Vec<bool>` | parallel to `link_stack`; "The close pops color iff its own open pushed one" (`:495-499`) |
| `current_link_data`, `current_preset_id` | `Option<…>` | mirrors of stack tops |
| `current_menu_id`, `current_menu_coords` | menu assembly | |
| `inv_manager`, `inv_viewitem`, `paired_capture` | `Option<…Builder>` | in-flight multi-line captures |
| `event_matchers` | `Vec<(Regex, config::EventPattern)>` | **compiled from config** at construction (`:514-530`); disabled patterns skipped, invalid regexes `warn!`ed and dropped |

`link_pushed_color` is worth flagging: its comment (`src/parser.rs:495-499`) records that without it *"the color leaked onto every later line"* when a link opened outside bold and closed inside it. Cena will hit the same class of bug with any stack-paired styling.

### `ParsedElement` — the parser's output vocabulary

`src/parser.rs:37-405`. **~80 variants.** Rather than list all, here is the shape and the parts that matter for a port.

`Text` (`:38-48`) is the common case and carries `content`, `stream`, `fg_color`, `bg_color`, `bold`, `mono`, `span_type: SpanType`, `link_data: Option<LinkData>`. Note styling is resolved to `Option<String>` colour names at parse time, not left as markup.

**Design rule visible throughout: raw attributes cross the parser boundary when the mapping is core's business.** Four variants do this explicitly and say so:

- `CreatureStatus { id, attrs: Vec<(String,String)> }` — *"`attrs` holds every attribute except `exist`, raw, so the core layer owns the flag-name mapping"* (`:135-141`)
- `RoomMeta { attrs }` — *"Attributes are passed raw so the core layer owns the field mapping, like CreatureStatus"* (`:142-147`)
- `WindowHints { id, attrs }` — placement/persistence attributes (`:201-209`)
- `InventoryManager { …, items: Vec<Vec<(String,String)>>, continuations: Vec<Vec<(String,String)>> }` (`:373-393`)

**Stream-stack variants** (the invariant made concrete): `StreamPush{id}`, `StreamPop`, `StreamPopForced` (*"Consumers must treat any per-stream buffer as TORN (possibly truncated)"*, `:166-169`), `StreamResume{id}` (*"subsequent text belongs to `id`, but WITHOUT the arrival side effects of a fresh StreamPush (buffer clears for room/inv/reserve)"*, `:170-176`), `ClearStream{id}`.

**VellumFE's own wire extensions** — tags Vellum invented, emitted by Lich scripts. These are the closest existing thing to a Cena scripting bridge and matter a lot for the port:

| Variant | Wire form | Semantics |
|---|---|---|
| `VellumTimer{id, value:i64}` | `<vellumTimer id='…' value='…'/>` | script-driven countdown; `value` is absolute epoch end seconds; 0/past clears (`:78-84`) |
| `VellumCommand{command}` | `<vellumCmd cmd=".header off"/>` | **"Only dot-commands are honored downstream, so the feed can drive client UI but never send game commands."** (`:85-91`) |
| `VellumImage{src, rows:f32, align}` | `<vellumImg src='banner' rows='4' align='left'/>` | **"`src` is a pool image NAME (shortcode alphabet), never a path … so a feed can name art but can never read an arbitrary file."** `rows` clamped by the renderer (`:92-104`) |
| `LichWebUI(WebUiHandshake)` | `<LichWebUI …/>` | reply to `;ui handshake` (`:330-331`) |

Both extension guards are **capability restrictions on a channel the script controls**, and both are stated as security properties. Cena is replacing Ruby-side Lich scripts with in-process Lua, which *removes* the wire hop — but the same two questions (can a script issue arbitrary game commands? can it name arbitrary files?) become sandbox-policy questions instead of parser-policy questions. These two comments are the existing answer worth carrying forward.

**Extended-feed variants** (Saga/Wrayth 1.0.1.28+ protocol, richer than what Lich consumes): `Pulse{mana, min, max}` (`:353-368` — *"the server declares the alternation, nothing is inferred … Replaces the old trick of inferring pulses from observed mana gain or exp absorption"*), `InventoryManager`, `InventoryViewItem(InventoryViewItemResponse)` (`:394-405` — the bare `closed` attribute is *"the authoritative open/closed signal"* for containers), `WorldEvent{realm, expires_min, text}` (expiry in **minutes**), `PantheonStatus{value}`, `MindStateExp{…}` (`:148-164` — exp numbers sticky, event-bonus flags a snapshot, *"which is why this fires even with no attrs"*).

**"Dark" variants** — parsed but not yet acted on, marked in the source: `Expose{kind, id}` (*"DARK in redesign Phase 1; expose-=-show semantics land in Phase 4"*, `:186-194`, with wire-frequency evidence "bank sends exposeDialog ×4,265"), `DeleteContainer{id}` (*"×7,559 on the wire — container removal; clearContainer was handled, delete was silently dropped. DARK in Phase 1"*, `:195-200`), `WindowHints` (`:201-209`). Cena should note the practice of **citing observed wire counts in the source comment** to justify handling a tag.

### GSIV XML element coverage vs `GemStoneIV-XML-Elements.md`

`GemStoneIV-XML-Elements.md` (18,117 bytes) is a wire-log analysis listing ~103 element names across sections (Core System, Stream Management, Component System, Container/Inventory, Dialog System, UI Control, Navigation, Interactive, Character State, Text Formatting, Dialog Control, Settings/Flags, Client Command Markers). Cross-checking its element table against `KNOWN_WIRE_TAGS`:

**Documented elements absent from `KNOWN_WIRE_TAGS`:**

| Element | Doc ref | Status |
|---|---|---|
| `style` | `GemStoneIV-XML-Elements.md:154` (`id` = roomName, roomDesc, …) | **Handled but unlisted.** `src/parser.rs:759` and `:793` both branch on `tag.starts_with("<style ")`, and `src/parser/handlers.rs:114` has the `<style id='roomName'>` handler. Harmless (the dispatch chain matches before the terminal `else`), but it means the table's claim to be the "union of our dispatch chain and the Saga tag set" is **not exactly true**. |
| `c` | `:327` — client command wrapper inside `<!-- CLIENT -->` | not in table |
| `m`, `o`, `r`, `s`, `w` | `:328-332` — styled text/metadata spans | not in table; doc itself calls these "Rare/Styling" (`:379`) |
| `columnFont` | `:379` | not in table |

The remaining apparent mismatches from a naive table scrape are **not elements**: `main`, `thoughts`, `speech`, `loot`, `bounty`, `society`, `death`, `logons`, `familiar`, `ambients`, `announcements` are *stream ids*; `health`, `mana`, `stamina`, `spirit`, `mindState`, `nextLvlPB`, `encumlevel`, `pbarStance` are *progressBar ids*; `bank`, `befriend`, `charprofile`, `charsheet`, `Spells`, `BetrayerPanel`, `misc`, `toggles`, `panels`, `dialog`, `builtin`, `detach`, `room` are *dialog/window ids or config keys* (`GemStoneIV-XML-Elements.md:423-435`).

**In `KNOWN_WIRE_TAGS` but not in the doc** (52 names) — the table is the broader of the two, as its comment claims. It adds the whole extended feed (`inventoryManager`, `inventoryViewItem`, `continuation`, `pulse`, `worldEvent`, `PantheonStatus`, `result`, `reward`, `crtrStatus`, `roommeta`, `objectives`/`objective`, `exits`, `exists`, `description`, `hostile`, `players`), Vellum/Lich extensions (`LichWebUI`, `FEVersion`, `LaunchURL`), UI machinery the doc omits (`checkBox`, `editBox`, `hScrollBar`, `vScrollBar`, `popup`, `menu`, `mi`, `nomenu`, `palette`, `presets`, `settings`, `sentSettings`, `macros`, `timer`, `tipInfo`, `dynaStream`, `clearDynaStream`, `streamId`, `getSkinVersion`, `forcesave`, `updateverbs`, `noverbupdates`, `monopolize`, `celebration`, `cli`, `action`, `extra`, `name`, `string`, `br`, `i`, `roomDesc`).

**Net assessment for Cena:** `KNOWN_WIRE_TAGS` is the more authoritative artifact and the one to port. It should gain `style` (and, if the rare styling spans `c`/`m`/`o`/`r`/`s`/`w`/`columnFont` genuinely appear, those too) so the "nothing unknown is swallowed" reasoning stays airtight.

UNVERIFIED: whether `c`/`m`/`o`/`r`/`s`/`w`/`columnFont` are handled elsewhere in the dispatch chain or currently fall through to literal-text passthrough. To verify: grep `src/parser.rs` and `src/parser/handlers.rs` for each single-letter tag pattern (`"<c "`, `"<c>"`, `"<m "`, …) — single-letter greps are noisy, so this needs a careful manual read of the dispatch chain rather than a bulk grep.

---

## 4. AppCore

`src/core/app_core/mod.rs:1-5`:

> Core application logic — Pure business logic without UI coupling. AppCore manages game state, configuration, and message processing. It has NO knowledge of rendering — all state is stored in data structures that frontends read from.

**26,825 lines across 29 files.** Module breakdown (`find src/core/app_core -name "*.rs" | xargs wc -l`):

| File | LOC | Role |
|---|---:|---|
| `commands.rs` | 5,132 | dot-command dispatch (`handle_dot_command`, `:2360`) |
| `layout.rs` | 2,455 | window layout management |
| `state.rs` | **2,089** | the `AppCore` struct + central methods — **capped at 2,100** |
| `state/tests.rs` | 1,886 | tests |
| `state/window_lifecycle.rs` | 1,713 | window creation/realization/teardown |
| `state/menus.rs` | 1,286 | `build_*_menu` |
| `config_editor.rs` | 1,223 | in-client config editing |
| `state/tab_moves.rs` | 1,195 | tear-out / join / convert / reorder + undo |
| `state/windows.rs` | 1,091 | window ops |
| `state/alerts.rs` | 1,004 | alert admission |
| `state/travel_ticks.rs` | 948 | per-frame travel tick |
| `state/tab_moves/tests.rs` | 915 | tests |
| `state/persistence.rs` | 704 | save/load |
| `keybinds.rs` | 676 | keybind resolution, `HotbarKeyConflict` |
| `skill_goals.rs` | 674 | GOALS launch ordering |
| `command_help.rs` | 665 | help table (tripwire-paired with `commands.rs`) |
| `state/remote.rs` | 622 | remote/web state sync |
| `interact.rs` | 549 | `InteractAction`, `InteractEntity` |
| `webui.rs` | 499 | Lich WebUI bridge |
| `targeting.rs` | 305 | target cycling |
| `state/focus.rs` | 303 | focus model |
| `state/stage_scene.rs` | 152 | creature-field stage scene |
| `haptics.rs` | 152 | `HapticEvent`, `HapticSnapshot` |
| `streams.rs` | 128 | stream routing |
| `automation.rs` | 127 | `AutomationOwner` — the automation lease |
| `state/streams.rs` | 126 | stream state |
| `state/custom_status.rs` | 92 | highlight-set custom statuses |
| `jinx.rs` | 88 | asset-manager glue |
| `mod.rs` | 26 | facade |

`mod.rs` exports only `AutomationOwner`, `HapticEvent`, `HapticSnapshot`, `InteractAction`, `InteractEntity`, `HotbarKeyConflict`, and `pub use state::*` (`src/core/app_core/mod.rs:20-25`). Every other submodule is private — `mod automation; mod commands; mod config_editor; …` (`:7-19`). Only `command_help` is `pub(crate)`.

### `AppCore` struct fields

`src/core/app_core/state.rs:40`. The struct is grouped by `// === Section ===` comments. Selected fields with their ownership meaning:

**Configuration**
| Field | Type | Note |
|---|---|---|
| `config` | `Config` | `:43` |
| `layout` | `Layout` | current window layout def, `:46` |
| `baseline_layout` | `Option<Layout>` | baseline for proportional resizing, `:49` |

**State** — the two roots frontends read (`:52-56`)
| `game_state` | `GameState` | connection, character, room, vitals |
| `ui_state` | `UiState` | windows, focus, input, popups |

**Message processing** (`:58-63`)
| `parser` | `XmlParser` | |
| `message_processor` | `MessageProcessor` | routes parsed elements to state updates |

**Stream management** (`:65-73`): `current_stream: String`, `discard_current_stream: bool` (*"discard text because no window exists for stream"*), `stream_buffer: String`.

**Timing** — deliberately empty. `src/core/app_core/state.rs:82-88` is the nine-line tombstone quoted in Rule 11 above. This is a field's *absence* documented as load-bearing.

**Menus / interaction** (`:91-104`): `cmdlist: Option<CmdList>`, `menu_request_counter: u32`, `pending_menu_requests: HashMap<String, PendingMenuRequest>`, `menu_categories`, `last_link_click_pos: Option<(u16,u16)>`.

**Tab-move undo** (`:106-113`): `last_tab_move: Option<tab_moves::UndoRecord>` — *"Single-level undo … Gated by a hash of the layout defs and cleared on any wholesale layout replacement."* Plus `tab_undo_to_apply: Option<tab_moves::UndoOutcome>`, parked for the GUI when the undo ran via `.undotabmove`; *"The TUI never reads it."*

**Creature field** (`:115-131`): `creature_field`, `creature_field_synced_gen: u64`, `creature_field_synced_cal: u64`, `stage_scene: Option<Arc<StageScene>>`, `field_overrides`. The sync comment (`:116-117`) states the performance contract: *"Synced from `game_state.room_creatures` by `sync_creature_field`, **event-driven on the roster generation — never per frame**."*

**Outbound command queues** — Vellum has **four** distinct paths for core-originated game commands, and the distinctions matter:
| Field | `state.rs` line | Purpose |
|---|---|---|
| `queued_game_commands: Vec<String>` | `:136` | core logic outside the typed-command path (e.g. target cycling); *"core-initiated sends ride the same per-frame `take_outbound` drain as travel/foreach automation"* |
| `timed_commands: Vec<(Instant, String)>` | `:235` | macro sleep segments (`look\rs2\rhide`); *"insertion order preserved among same-tick due commands"* |
| `travel.outbound` (inside `TravelService`) | `src/core/travel/mod.rs:29` | walk-executor commands |
| `item_mover` | `:238` | verified item moves (`_drag`, extended feed), *"one at a time, confirmed against hand events"* |

All drain through `take_outbound`. **For Cena, this is the concrete answer to "how does a script send a command?"** — not a direct socket write, but a queue the frontend drains through the same path as user typing, so echoes and history behave identically.

**Services held by value on AppCore** (`:139-238`): `perf_stats: PerformanceStats`, `sound_player: Option<SoundPlayer>`, `tts_manager: TtsManager`, `pending_haptics: Vec<HapticEvent>` + `haptic_prev: HapticSnapshot` + `last_highlight_rumble: Option<Instant>`, `map: MapService`, `map_updater: MapDbUpdater`, `jinx_worker: JinxWorker`, `skill_trainer_worker: SkillTrainerWorker`, `alerts: AlertState`, `alert_packs: Vec<AlertPack>`, `alertpack_approvals`, `indicator_templates: HashMap<String, IndicatorTemplateEntry>`, `jinx_catalog: Option<Vec<CatalogEntry>>`, `travel: TravelService`, `item_mover: ItemMover`, `evidence: EvidenceStore`, `foreach: ForeachService`, `window_registry: WindowRegistry`, `gameobj_data: Option<Arc<GameObjData>>`.

Several carry explicit **caching-for-frame-budget** rationale, which Cena will need the same answers for:
- `alert_packs` (`:185-188`): cached in memory *"so per-room re-arming never touches the disk (`reload_highlights` does, and would be far too expensive to run every time the player walks through a door)"*, with `last_pack_scope: Option<RoomScope>` so *"moving between rooms in the same area rebuilds nothing"*.
- `indicator_templates` (`:196-202`): *"Status icon resolution (indicator windows + dashboards) reads this per frame; the underlying `Config::list_indicator_templates()` does file IO, so it must not run in the render loop."*

**Deferred/pending one-shots** — a second family of the staging pattern, this time on AppCore rather than MessageProcessor: `pending_goals_launches: VecDeque<PendingGoalsLaunch>` + `staged_goals_launch` (`:169-177` — a launch target *"moves into `pending_goals_launches` only after the owning frontend confirms that exact command reached its network queue"*), `pending_urchin_refresh: Option<(u32, Instant)>` (`:212-216`), `pending_day_pass_scan: Option<(u32, Instant, Vec<String>)>` + `day_pass_scan_open: Option<(String,bool)>` + `day_pass_sack_probed: bool` (`:217-231`), `custom_status_expiries: HashMap<String, Instant>` (`:178-180`), `jinx_nudge_pending: bool` (`:207-209`).

The day-pass trio is a good miniature of the whole design: `:220-222` cites *"Lich's `mapdb_find_day_pass` sweep — the cache must learn what's held BEFORE routing so a held pair routes at 0.8"*, and `:224-228` records that a naive implementation *"churned the sack live"* with round-by-round open/close.

**Frontend-coordination flags** (from `state.rs` ~`:50-135` of the later block): `running`, `needs_render`, `chunk_has_main_text`, `chunk_has_silent_updates`, `layout_modified_since_save`, `layout_autosave_pending: Option<Instant>`, `save_reminder_shown`, `force_show_command_input`, `reconnect_requested`, `disconnect_requested`, `detach_quit_supported`, `launch_requested: Option<String>`, `base_layout_name: Option<String>`, `appearance_changed_externally` (`:75-79` — *"the GUI must copy the store into its `ui_settings` next frame or its layout save would stomp the new look. The GUI clears it after syncing"*), `room_window_dirty`, `keybind_map: HashMap<KeyEvent, KeyBindAction>`, `hotbar_key_conflicts`, `nav_room_id: Option<String>`, `lich_room_id: Option<String>`, `room_subtitle`, `room_components: HashMap<String, Vec<Vec<TextSegment>>>`, `current_room_component`, `saved_dialog_positions: SavedDialogPositions`.

Note `nav_room_id` **and** `lich_room_id` are separate fields — Vellum distinguishes the room id from `<nav rm=…/>` from the one Lich reports.

### Command dispatch

`AppCore::handle_dot_command(&mut self, command: &str) -> Result<CommandOutcome>` at `src/core/app_core/commands.rs:2360`:

```rust
fn handle_dot_command(&mut self, command: &str) -> Result<CommandOutcome> {
    let parts: Vec<&str> = command[1..].split_whitespace().collect();
    let cmd = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
    tracing::debug!("handle_dot_command: '{}'", command);

    match cmd.as_str() {
        // === COMMAND ARMS BEGIN === (command_help.rs tripwire: every
        // top-level arm literal here must have a help-table row, and
        // vice versa; the test extracts them from this source span)
```

Two things to carry into Cena:

1. **The return type is `CommandOutcome`, not `()`.** Dot-commands do not send to the network themselves; they return an outcome the caller acts on. `CommandOutcome` is re-exported from `src/data/ui_action.rs` (666 lines) alongside `UiAction`, `ShellZoneTarget`, `ZoneOp` (`src/data/mod.rs:18`). Commands live in core; *acting* on them is the frontend's job.
2. **The `=== COMMAND ARMS BEGIN ===` sentinel is a source-span tripwire.** A test in `command_help.rs` (665 lines) parses this exact span out of `commands.rs` and asserts a bijection with the help table. Documentation drift is a test failure. Cena should copy this — a Lua-facing command surface will drift from its help text within weeks otherwise.

**106 top-level match arms**, extracted from the span at `commands.rs:2365` onward (`grep -cE '^            "[a-z0-9_]+"'` → 106). Aliases share an arm (`"quit" | "q"`). The full command vocabulary:

```
addcolor addhighlight addhl addkey addkeybind addspellcolor addwindow
alertpacks anchorinfer bestiary border colorpalette colors controller
createcolor creaturefield data deletewindow delwindow drag edithighlight
edithl edittheme editwin editwindow eh emptyhands exit exportskin fh
fieldcamera fillhands find footer foreach go2 goals gonew harmony header
hidecontainers hidewin hidewindow highlightprofiles highlights hl
hlprofiles hotbar hotbars importskin indicator indicators inspect invsync
jinx kb kbprofiles keybindprofiles keybinds launch launcher layouts
leftbar loadhighlights loadhl loadkb loadkeybinds loadlayout lockall
lockwindow lockwindows makeskin mapdb mappromote menu menukb menukeybinds
menuof newspellcolor nexttab nextunread packeditor packs palette perf
performance portal prevtab q quit reconnect redolayout reload reloadmacros
reloadskin rename resetpalette resize rightbar room roomimages roomimg
savehighlights savehl savekb savekeybinds savelayout saveskin setpalette
setskin settheme settings skin skins snapdebug sorter spellcolors
spellwatch stop streams switcher testline theme themes transparent tts
uicolors uiexport uiimport undolayout undotabmove unlockall unlockwindow
unlockwindows ver version viewitem webinfo webui windows
```

Roughly: ~45 are window/layout management, ~20 are appearance (themes, skins, colors, palettes), ~12 are highlights/keybinds/profiles, ~8 are game automation (`go2`, `foreach`, `drag`, `stop`, `spellwatch`, `invsync`, `goals`, `viewitem`), and the rest are session/diagnostics. **Note how few are game automation** — that is Lich's territory, and it is the gap §8 is about.

One arm is worth quoting for the multi-session story (`commands.rs:2370-2385`), `"quit" | "q"`:

```rust
if self.detach_quit_supported
    && self.config.ui.keep_open_on_quit
    && self.game_state.connected
{
    self.save_on_quit();
    self.disconnect_requested = true;
    self.add_system_message(
        "Detached — window stays open. .reconnect or .launch <character> to resume; .quit again or .exit to close.",
    );
} else {
    self.quit();
```

First `.quit` detaches but keeps the window and scrollback; a second (connection already down) exits. `.exit` always closes in one step.

---

## 5. Tracked game state — the Infomon counterpart

Two files hold it, and the split is not what the names suggest:

- **`src/core/state.rs` (2,831 lines) — `GameState`.** This is the real Infomon counterpart: everything parsed live from the wire feed.
- **`src/core/character_state.rs` (717 lines) — `CharacterState`.** A *narrow* port covering only what travel needs. Its own header (`src/core/character_state.rs:1-4`) says so:

  > Character state parsed from the game feed — **a focused port of Lich's Infomon** (`lib/gemstone/infomon/parser.rb`) for the fields that gate travel: society status/rank (seeking), profession (guild tags), CHE/House (locker tags), and citizenship.

  And `:5-11` gives the mechanism and its limit: *"Like Lich's `Infomon::Parser.parse`, this is a per-line text matcher: the parser feeds each game line to `CharacterState::parse_line` … The state is populated in response to the SOCIETY / INFO / PROFILE / CITIZENSHIP commands (and in-game society join/step/resign events) — **none are spontaneous, so travel features that need this state ask for it first.**"*

`CharacterState` is reachable as `game_state.character` (`src/core/state.rs:259`).

### 5.1 `GameState` — field by field

`src/core/state.rs:38`, `#[derive(Clone, Debug)]`.

| Field | Type | Line | What it holds |
|---|---|---:|---|
| `connected` | `bool` | 40 | connection status |
| `character_name` | `Option<String>` | 43 | from `<app char=…>` (`ParsedElement::AppInfo`, *"the authoritative source of the character name in the game feed"*, `parser.rs:210-214`) |
| `room_id` | `Option<String>` | 46 | |
| `room_name` | `Option<String>` | 49 | |
| `exits` | `Vec<String>` | 52 | |
| `game_time` | `i64` | 56 | server time from last prompt (Unix); *"the authoritative time source for roundtime/casttime comparisons"* |
| `game_time_received` | `Option<Instant>` | 63 | local clock when `game_time` last updated — *"Prompts only arrive with traffic, so during silence `game_time` stands still — and a roundtime measured against it never counts down, freezing travel"* (`:57-62`) |
| `roundtime_end` | `Option<i64>` | 66 | absolute server end time |
| `casttime_end` | `Option<i64>` | 69 | |
| `spell` | `Option<String>` | 72 | prepared spell |
| `active_streams` | `HashMap<String,bool>` | 75 | |
| `status` | `StatusInfo` | 78 | see §5.2 |
| `vitals` | `Vitals` | 81 | percentages only |
| `minivitals` | `MiniVitalsState` | 299 | full value/max/text per vital |
| `inventory` | `Vec<StyledLine>` | 85 | |
| `inventory_received` | `bool` | 89 | |
| `left_hand` / `right_hand` | `Option<String>` | 92 / 95 | |
| `active_effects` | `Vec<String>` | 98 | |
| `effects` | `HashMap<String, ActiveEffectsContent>` | 105 | keyed by category: ActiveSpells / Buffs / Debuffs / Cooldowns / `Timers` |
| `objectives` | `ObjectivesContent` | 109 | Saga quest panel |
| `compass_dirs` | `Vec<String>` | 112 | |
| `injuries` | `HashMap<String,u8>` | 117 | body part → severity |
| `last_prompt` | `String` | 120 | |
| `target_list` | `TargetListState` | 123 | |
| `room_creatures` | `Vec<Creature>` | 127 | + `room_creatures_generation: u64` (129) |
| `creature_effects` | `HashMap<String, Vec<ActiveCreatureEffect>>` | 137 | per-creature effects, keyed by exist id |
| `room_objects` | `Vec<RoomObject>` | 146 | + `room_objects_generation` (148) |
| `room_players` | `Vec<Player>` | 151 | + `room_players_generation` (153) |
| `room_description` | `Vec<StyledLine>` | 159 | + `room_description_generation` (161) |
| `story_picture` | `Option<String>` | 173 | |
| `spellbook` | `Vec<StyledLine>` | 179 | + `spellbook_generation` (181) |
| `room_meta` | `RoomMetaState` | 184 | |
| `managed_inventory` | `Option<ManagedInventoryState>` | 189 | extended-feed structured inventory |
| `pulse_count` | `u64` | 195 | |
| `next_pulse_mana` | `bool` | 198 | |
| `pulse_next_earliest` / `pulse_next_latest` | `Option<i64>` | 213 / 216 | from `<pulse min max/>` |
| `viewed_item` | `Option<ViewedItem>` | 202 | `inventoryViewItem` response |
| `world_events` | `Vec<WorldEventState>` | 206 | |
| `pantheon_value` | `Option<u32>` | 208 | |
| `objects` | `game_objects::GameObjects` | 221 | the GameObj registry — see §7 |
| `move_feedback` | `VecDeque<(u64, MoveFeedback)>` | 227 | stamped with `game_line_no` |
| `game_line_no` | `u64` | 231 | |
| `recent_lines` | `VecDeque<(u64, String)>` | 246 | only populated when `capture_recent_lines` is on |
| `line_seq` | `u64` | 249 | |
| `spell_names_seen` | `HashMap<u16,String>` | 257 | spell number → name |
| `character` | `CharacterState` | 259 | see §5.3 |
| `silver` | `Option<u64>` | 263 | + `silver_line_no: u64` (268) |
| `nav_count` | `u64` | 272 | |
| `day_passes` | `day_pass::DayPassCache` | 276 | |
| `dr_experience` | `DRExperienceState` | 279 | **DragonRealms** exp/skill components |
| `encumbrance` | `EncumbranceState` | 285 | |
| `stance` | `StanceState` | 288 | |
| `group` | `group::GroupState` | 293 | |
| `betrayer` | `BetrayerState` | 296 | |
| `bounty` | `BountyState` | 303 | |
| `society` | `SocietyState` | 307 | raw SOCIETY lines (distinct from `character.society`) |
| `estimated_lag_ms` | `Option<i64>` | 312 | recomputed every `LAG_CHECK_INTERVAL_SECS = 30` (`:12`) |
| `sound_queue` | `SoundQueue` | 318 | pre-allocated capacity 5 (`:15-17`, `:29-32`) |

**The generation-counter convention.** At least nine sub-states carry a `generation: u64` bumped on change: `room_creatures`, `room_objects`, `room_players`, `room_description`, `spellbook`, `MiniVitalsState` (`:437`), `BountyState` (`:473`), `SocietyState` (`:504`), `TargetListState` (`:542`), `RoomMetaState` (`:1091`), `DRExperienceState` (`:1032`), `EncumbranceState` (`:1727`), `StanceState` (`:1780`), `BetrayerState` (`:1826`), `ViewedItem` (`:1188`), `ManagedInventoryState` (`:1343`), and `CharacterState` (`character_state.rs:155`). The update methods return `bool` for "changed" — e.g. `MiniVitalsState::update_vital` (`:440-465`) compares all three of value/max/text before bumping.

**For Cena this is the change-notification mechanism**, and it is the right one for a pull-model frontend: a frontend caches `(generation)` and re-renders only on mismatch, at the cost of one `u64` compare. Extending it to a Lua API is direct — a script can poll a generation as cheaply as a frontend can.

### 5.2 `StatusInfo` — the indicator model

`src/core/state.rs:339-343`, and it is *not* a struct of bools:

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StatusInfo {
    /// Lowercase indicator id -> active. Absent means "never reported",
    /// which reads as inactive.
    flags: BTreeMap<String, bool>,
}
```

Design points, all of which Cena should take:

- **Open vocabulary.** `set(&mut self, id, active) -> bool` (`:355-357`) accepts any id the game invents; `key()` (`:347-350`) strips the wire's `Icon` prefix and lowercases, so callers may pass `IconSTUNNED` or `stunned`. Return is "did it change", to avoid no-op deltas.
- **Tri-state read.** `get()` returns false for unknown (`:361-363`), but `is_known()` (`:367-369`) distinguishes *reported inactive* from *never reported*. The rationale (`:365-367`): *"Conditions do not need this, but a multi-account display does: an unreported indicator should render as unknown rather than as a confident 'no'."*
- **Typed accessors generated by macro** so the name list stays in one place (`:381-394`). The `status_accessors!` list (`:396-411`) is the vocabulary the client reasons about by name:

  `standing, kneeling, sitting, prone, stunned, bleeding, hidden, invisible, webbed, joined, dead, poisoned, diseased` — **13 indicators.**

  The `joined` entry carries a bug note (`:405-407`): *"True while grouped. Before the map refactor this was never written — the parser had no arm for it — so any code reading it saw a permanent `false`. It now reflects `IconJOINED`."* The `BTreeMap` design is exactly what let that bug hide, and `is_known` is the antidote.
- **Wire-compatible serialization.** `#[serde(transparent)]` with a comment (`:330-334`) noting the web client reads `d["stunned"]` and *"Unknown extra keys are ignored by that client, and absent keys read falsy, so adding ids is backward compatible in both directions."*
- The doc explicitly recommends the typed path: *"prefer `status.stunned()` over `status.get("stunned")` so a typo is a compile error rather than a silent `false`"* (`:335-337`). A Lua API loses that guarantee — worth deciding deliberately in Cena whether Lua gets the open string API, a checked enum, or both.

`GemStoneIV-XML-Elements.md:135-144` lists 10 indicator ids on the wire (`IconKNEELING`, `IconPRONE`, `IconSITTING`, `IconSTANDING`, `IconSTUNNED`, `IconHIDDEN`, `IconINVISIBLE`, `IconDEAD`, `IconWEBBED`, `IconJOINED`); Vellum's typed list adds `bleeding`, `poisoned`, `diseased` (which arrive as `ParsedElement::StatusIndicator`, `parser.rs:235-239`, whose own comment names exactly those four: *"poisoned", "diseased", "bleeding", "stunned"*).

### 5.3 `CharacterState` — the deliberate Infomon subset

`src/core/character_state.rs:112`. **Nine fields only:**

| Field | Type | Line | Source command |
|---|---|---:|---|
| `society` | `Option<String>` | 115 | SOCIETY |
| `society_rank` | `u32` | 118 | SOCIETY |
| `profession` | `Option<String>` | 121 | INFO / PROFILE |
| `level` | `Option<String>` | 125 | observed |
| `che` | `Option<String>` | 129 | CHE/House — drives locker tags |
| `citizenship` | `Option<String>` | 132 | CITIZENSHIP |
| `urchins_expire` | `i64` | 137 | `urchin status` |
| `watched_spells` | `Vec<u16>` | 142 | `.spellwatch` |
| `generation` | `u64` | 155 | change counter |

Methods: `parse_line(&mut self, line) -> bool` (`:200`), `can_seek()` (`:282`), `observed_name()` (`:288`), `clear_connection_observations()` (`:293`), `guild_tag(suffix)` (`:300`), `urchins_valid(now_epoch, hidden, invisible)` (`:313`), `locker_tags()` (`:319`), `observe_level(text)` (`:355`), plus `watch_spells` / `unwatch_spells` / `unwatch_all_spells` (`:162`, `:178`, `:190`).

Regexes are declared with a local `re!` macro over `LazyLock<Regex>` (`character_state.rs:17-22`) — the pattern compiles once, at first use. Example (`:27-30`):

```rust
re!(
    SOCIETY,
    r"^\s+You are a (?P<standing>Master|member) (?:in|of) the (?P<society>Order of Voln|Council of Light|Guardians of Sunfist)(?: at (?:rank|step) (?P<rank>[0-9]+))?\.$"
);
```

This is line-for-line the same technique as `lich-5/lib/gemstone/infomon/parser.rb` — anchored per-line regex with named captures — so porting the *rest* of Infomon into Cena is mechanical, not architectural. The reason it stopped at nine fields is scope, not difficulty.

---

## 6. The data layer (`src/data/`, 8,359 lines)

`src/data/mod.rs:1-5`:

> Data layer — Pure state without UI coupling. This module contains all the game state and UI state as pure data structures. NO imports from frontend/ or any rendering code. **Both TUI and GUI frontends read from these structures to render.**

| File | LOC | Contents |
|---|---:|---|
| `ui_state.rs` | 2,494 | `UiState` + dialog/popup/drag/input-mode model |
| `widget.rs` | 1,743 | the render-data vocabulary |
| `input.rs` | 850 | frontend-neutral `KeyCode` / `KeyEvent` / `MouseEvent` |
| `window.rs` | 847 | window state |
| `ui_action.rs` | 666 | `UiAction`, `CommandOutcome`, `ShellZoneTarget`, `ZoneOp` |
| `webui.rs` | 640 | Lich WebUI handshake types |
| `skill_trainer.rs` | 585 | skill-manager model |
| `geometry.rs` | 260 | `Rect`-like primitives |
| `remote_buffer.rs` | 210 | shared scrollback ring for the web frontend |
| `view_kind.rs` | 40 | `ViewKind` |
| `mod.rs` | 24 | facade, `pub use` of six modules |

**`widget.rs` types** (`grep -n "^pub struct \|^pub enum "`): `IconRef` (16), `TextContent` (33), **`StyledLine`** (58), **`TextSegment`** (72), `FloatAlign` (111), `InlineImage` (124), `SpanType` (243), `LinkData` (262), `QuickbarEntry` (284), `QuickbarData` (306), `ProgressData` (314), `CountdownData` (326), `CompassData` (337), `MapData` (343), `InjuryDollData` (356), `IndicatorData` (383), `RoomContent` (393), **`ActiveEffect`** (403), `ActiveEffectsContent` (478), `ObjectiveReward` (503), `ObjectiveAction` (511), `Objective` (518), `ObjectivesContent` (534), `TabSpec` (553), `TabDefinition` (564), `TabState` (588), `TabbedTextContent` (612), `PerceptionEntry` (925), `PerceptionFormat` (935), `PerceptionData` (946).

`TextSegment` / `StyledLine` are the currency of the whole text path: the parser produces segments, the highlight engine rewrites them, `MessageProcessor` assembles lines, the remote sink shares them as `Arc<StyledLine>`, and both frontends render them. **One type, four consumers.**

`ActiveEffect` is reused past its original purpose. `src/core/alert_timers.rs:1-14` explains why client-authored countdowns reuse it rather than inventing a bar type:

> An effect is already `{ label, percent, time string, absolute expiry }`, which is exactly a timer, and `display_value`/`display_time` already drain the bar smoothly and format HH:MM:SS. Both frontends' effects renderers read the window's own content and treat `category` as an opaque string, so a `Timers` category draws correctly **with no rendering changes at all**.

The catch is stated honestly at `:9-14`: server-fed effects are reaped by the server, client timers have no server, so `alert_timers` reaps its own — *"without that, every timer ever started would pile up in the window as a dead 00:00:00 bar."* The category constant is `TIMERS_CATEGORY = "Timers"` (`alert_timers.rs:21`).

**`ui_state.rs` types**: `UiState` (13), `WindowDiscovery` (158), `WindowDiscoveryKind` (168), `MouseDragState` (179), `DragOperation` (188), `DialogDragState` (197), `DialogDragOperation` (206), `LinkDragState` (220), `PendingLinkClick` (228), `InputMode` (235), `InteractCategory` (291), `InteractState` (322), `DialogState` (330), `DialogControlLayout` (484), `DialogButton` (503), `DialogLink` (519), `DialogImage` (530), `DialogSkin` (546), `DialogSpinBox` (558), `DialogDropDown` (571), `DialogField` (585), `LabelAlign` (680), `DialogLabel` (687), `DialogProgressBar` (717), `PositionedControlKind` (729), `PositionedControl` (751), `InjuriesPopupState` (1021), `PopupMenu` (1062), `PopupMenuItem` (1070).

The dialog family (14 types) exists because the game ships *its own UI* over the wire — `openDialog`/`dialogData` with buttons, dropdowns, spinboxes, progress bars and absolutely-positioned controls (`GemStoneIV-XML-Elements.md:73-109`). Cena inherits this obligation whole: it is not optional chrome, it is how bank, befriend, combat and the character sheet work.

**`ViewKind`** (`src/data/view_kind.rs:17-30`) is the presentation/identity split:

```rust
pub enum ViewKind {
    Dedicated(String),  // a named widget view ("compass", "inventory", "gs4_experience")
    Text,               // generic scrolling text (unclaimed streams)
    DialogPanel,        // generic dynamic dialog renderer (unclaimed dialogs)
    Container,          // container listing
}
```

Its header (`:1-10`) states the principle: *"names a PRESENTATION (which renderer draws a window), decoupled from window identity (which game feed it binds to) … The template catalog conflates identity, presentation, and default geometry; identity already lives in `WindowBinding`, and this type extracts presentation."* Like `local_catalog`, it is **dark in Phase 1** — only `core::view_resolver` produces it, only tests consume it.

The `DialogPanel` arm carries the key robustness claim (`:24-27`): the generic dynamic renderer is *"the UberBar-proven anchor-grid path; always safe because the dialog store ingests every dialog."* **Cena should adopt this directly**: every game dialog is ingested and has a generic renderer, so an unrecognized dialog degrades to a working generic panel instead of vanishing. Same philosophy as the unknown-tag passthrough.

---

## 7. Notable core services

Sizes from `wc -l`; descriptions from each module's own header.

### `layout_engine` (`src/core/layout_engine/`, 4,572 lines across 8 files)

`mod.rs:1-10`: *"Map layout engine — **Rust port of the mapgen reference implementation**. Generates a 2D grid layout for one *location* of the Lich mapdb, live, on area entry (`docs/layout-engine-spec.md`). **Pure functions, no frontend imports**: rooms in, layout model out. Deterministic for fixed input; the canonical room iteration order is ascending room id."*

Pipeline, verbatim: *"direction analysis → BFS component placement with grid rips → per-component hill-climb + compaction → interior classification → cluster packing (outdoor sheet) + interior shelf."*

| Submodule | LOC |
|---|---:|
| `packer.rs` | 1,520 |
| `scene.rs` | 633 |
| `positioner.rs` | 616 |
| `overrides.rs` | 492 |
| `classifier.rs` | 453 |
| `direction.rs` | 301 |
| `cache.rs` | 295 |
| `mod.rs` | 262 |

Exports `Classification`, `LayoutCache`, `CacheOutcome`, `rooms_content_hash`, `EdgeAction`, `EdgeOverride`, `LocationOverrides`, `MapOverrides`, `SheetChoice` (`mod.rs:26-28`). Consumes `crate::core::mapdb::{Room, RoomTable}` (`:25`).

**Determinism + purity + a content hash for caching** is the combination that makes this portable and testable. Cena should preserve all three properties.

### `mapdb` (`src/core/mapdb/`, 1,015 lines) and `mapdb_update` (760)

`mapdb/mod.rs:1-8`: *"The mapdb — canonical room database shared by the layout engine (per-location room lists) and the pathing engine (the full wayto graph) … the layout-facing API is unchanged (`rooms(location)`, uid/id → location lookups over *mappable* rooms), while pathing sees every room through `room(id)` / `ids_of_uid` / `room_ids_with_tag`, including location-less rooms and **virtual urchin-hideout routing nodes that the map never draws**."*

Exports `Room`, `RoomTable`, `TimeTo`, `is_proc_command`, `rooms_for_location`, `rooms_from_array` (`:14`). Defines `SERVICE_TAGS` (`:17-20`) — *"the mapdb room tags that mark a room as offering a service worth a map marker (`.go2 bank` destinations). Everything else on a room's tag list (`meta:*`, hunting tags) is not marker material."* The list includes `advguard, advguard2, advguild, advpickup, alchemist, armorshop, bakery, bank, bardguild, …` (truncated in my read).

`mapdb_update.rs` (760) downloads released mapdbs from GitHub (`app_core/state.rs:161-162`).

**This is Lich's map database, consumed directly.** Cena does not need to invent a map format; it needs to read Lich's.

### `pathing` (`src/core/pathing/`, 5,507 lines)

`mod.rs:1-4`: *"Native pathfinding over the mapdb wayto graph — **the Lich `Map` pathing API (`map_base.rb`) without a Ruby session**. First piece of the go2 port (`docs/go2-port-plan.md`)."*

The v1 graph rules (`mod.rs:6-13`) are the most important honesty in this module, because they are a *deliberate* feature gap:

> - A wayto edge is only routable with a numeric `timeto` cost — exactly Lich's dijkstra, which skips edges whose weight is nil.
> - **StringProc wayto commands (`";e …"`) are excluded until the transpiler lands: a path we can't *walk* is worse than no path.**
> - StringProc timeto gates (portmasters, day passes, urchins) evaluate as "off" — the edge is excluded, matching those settings' v1 defaults.
> - Urchin-hideout teleport nodes never route.

| Submodule | LOC | Role |
|---|---:|---|
| `transpile.rs` | 4,447 | **the StringProc transpiler** — Ruby `wayto` procs → native |
| `dijkstra.rs` | 490 | `dijkstra`, `dijkstra_filtered`, `estimate_time`, `find_nearest`, `find_nearest_by_tag`, `path_to`, `path_to_filtered`, `silver_cost`, `PathTarget` (`mod.rs:18-21`) |
| `edge.rs` | 346 | `edge_cost` |
| `overrides.rs` | 202 | |
| `mod.rs` | 22 | |

**`transpile.rs` at 4,447 lines is the single most Cena-relevant file in the whole tree.** Lich's mapdb embeds *executable Ruby* in the graph (`StringProc` wayto commands), and Vellum is already paying 4,447 lines to translate that Ruby into something a Rust client can execute. Cena, which replaces Ruby with Lua, inherits this problem exactly — and inherits this solution. Any porting plan must budget for it.

### `travel` (`src/core/travel/`, 9,308 lines across 8 files)

`mod.rs:1-10`:

> Automation tasks that drive the game connection — command injection + parser-state subscription + cancellation. The travel executor (go2's walk loop) is the first task; **future Lich-like features (scripted routines, guild tasks) are meant to slot into the same host rather than each inventing its own frontend plumbing.**
>
> Flow per frame/network-line: AppCore builds a `TravelContext` snapshot, ticks the active task, surfaces `Status` messages, and queues `Send` commands; frontends drain the queue through their normal typed-command path (**so travel moves echo like anything the user types**).

`TravelService` (`mod.rs:26-30`) holds `task: Option<TravelTask>` and `outbound: VecDeque<String>`. Exports `TravelContext`, `TravelEvent`, `TravelTask` (`:23`).

| Submodule | LOC |
|---|---:|
| `executor.rs` | 6,654 |
| `day_pass_buy.rs` | 696 |
| `stash.rs` | 662 |
| `confluence.rs` | 422 |
| `target.rs` | 286 |
| `minotaur.rs` | 286 |
| `mazes.rs` | 182 |
| `mod.rs` | 120 |

**This is the closest thing in Vellum to a script host, and its header says so explicitly.** The contract — *snapshot in, tick, events + queued commands out, one task at a time* — is a viable shape for Cena's Lua runtime: a Lua coroutine is a `TravelTask` with a different implementation. Note `TravelService` holds **one** `task: Option<_>`, not a list; Cena needs many concurrent scripts, so this is the one part of the shape that must change.

### `jinx` (`src/core/jinx/`, 3,266 lines across 9 files)

`mod.rs:1-9`:

> The asset manager: a native, federated client for the Jinx repository protocol. It downloads and keeps current the files that shape the game and the interface — **game data (item/spell databases, the map), skins, icon sets, and shareable layouts — without a Lich install.**
>
> VellumFE speaks Jinx's exact wire protocol, so it reads **the existing elanthia-online repositories** for game data *and* VellumFE's own repositories for skins/icons/layouts. **Nothing downloads on its own; every install is a user action** (or an opt-in `auto-update`).

| Submodule | LOC | Role |
|---|---:|---|
| `installer.rs` | 914 | verified single-file download + atomic write |
| `worker.rs` | 589 | off-thread worker, polled per frame |
| `resolve.rs` | 474 | |
| `repo.rs` | 431 | |
| `protocol.rs` | 360 | manifest/asset types + **SHA1-b64 digest** |
| `bundle.rs` | 210 | |
| `manifest.rs` | 135 | |
| `metadata.rs` | 128 | `InstalledDb` (`jinx-installed.toml`) |
| `mod.rs` | 25 | |

Integrity is `protocol`'s SHA1-base64 digest, writes are atomic, and `AppCore::emit_stale_data_nudge` (`app_core/state.rs:1365-1390`) reads `InstalledDb` once per session and warns when any `kind == "data"` asset is ≥ **30 days** old (`STALE_DAYS: i64 = 30`, `:1366`) or has no `last_updated` at all.

**For Cena:** if Lua scripts are distributed, this is the existing, wire-compatible distribution channel — and its "nothing downloads on its own" default is the right one for executable content.

### `creature_cards` (`src/core/creature_cards/`, 3,245 lines)

`creature_cards.rs:1-13`. Sprite-based creature display for the `creaturefield` widget. Settled decisions worth noting because they are *interface* decisions:

> - Noun/family for art resolution come from **Vellum's own room-objs parse, never from Lich/CreatureBar's Ruby side.**
> - Creatures take wounds only (injury1-3 + healthy) — no scars.
> - Status overlay art is shared across all families, never per-family.
> - CreatureBar's 16-part vocabulary maps onto the doll's 14 parts **at this adapter** rather than rippling foot parts and a `nerves` rename into the player-doll ecosystem.

Files: `solver.rs` 2,405, `geometry.rs` 623, `naming.rs` 217, `creature_cards.rs` 1,358. The first bullet is a direct statement of independence from Lich; the last is a good example of *adapter-at-the-boundary* instead of vocabulary leakage.

### `game_objects` (`src/core/game_objects/`, 1,535 lines)

`mod.rs:1-16` — the most explicit Lich-lineage statement in the tree:

> The GameObjects registry: one queryable model of the items, creatures, and players the game feed has told us are present.
>
> This is **Lich's `GameObj` reimplemented from the same stream Lich reads** (verified: `lich-5/lib/common/xmlparser.rb` builds its inv from the `<inv>` push feed, exactly what VellumFE parses). It replaces a set of per-feature silos that each re-derive items with their own parsing: `container_cache`, `room_objects`, `room_creatures`, `room_players`, and the bare-string hands/worn fields — plus a second, drifted `<a>` parser in the TUI container window.
>
> **It is a pure data model, not a scripting engine.** It exposes queries (`items_in`, `carried`, `hostile_creatures`, `classify`), never behavior. **The automation lease remains the only thing that acts**; features read from here and act through the lease.

Submodules: `mod.rs` 816, `ready_stow.rs` 309, `inv_scan.rs` 209, `parse.rs` 201. Lich side for comparison: `lich-5/lib/common/gameobj.rb` is **1,945 lines**.

The "pure data model, not a scripting engine" line and the automation-lease reference are the load-bearing part for Cena: **querying and acting are separated, and only one thing holds the right to act.** `AutomationOwner` (`src/core/app_core/automation.rs`, 127 lines) is that lease.

### `highlight_engine` (2,260) — see §2, hop 5.

### `inventory_service` (796)

`src/core/inventory_service.rs:1-8`: continuation-following for the extended feed's `<inventoryManager>`. *"Big containers arrive paginated … This service owns the request tokens, keeps at most `MAX_IN_FLIGHT` continuation requests outstanding, merges chunks into one snapshot, and publishes it only when every cursor has been drained."*

Failure discipline (`:9-16`), *"mirrors Saga's client"*: `state="stale"` tears down and re-requests from scratch (bounded restarts); any other non-empty `state` (including the parser-synthesized `malformed` for prompt-torn captures) fails the same way; a never-arriving continuation times out; repeated cursors and duplicate item ids are dropped, not re-requested. A worked example of how to consume a paginated game feed safely.

### `multiaccount` (`src/core/multiaccount/`, 1,533) + `session_registry` (1,646)

`multiaccount/mod.rs:1-13`: *"Multi-account status: what every character on this machine is doing, in one place … this module holds the *model* — what a peer is, how stale it is, and how peers cluster into groups — while the transport that fills it lives in `core::multiaccount::hub` (750 lines). **Deliberately pure and synchronous.** Clustering from six independently parsed rosters is the part with real logic in it, and it is much easier to trust when it can be tested without sockets."*

`session_registry.rs:1-17`: which VellumFE instances are running on this machine. Each instance writes one process entry when its web sidecar binds and removes it when the listener stops. It lives in the **machine-local runtime/data directory rather than `VELLUM_FE_DIR`**, because *"launcher profiles may use different data roots, but they still need one shared view of which processes and Lich detachable-client endpoints are already owned."* Crashed instances leave files behind, so reads **garbage-collect entries whose pid is gone**. It lives in core (not the web frontend) precisely because of Rule 1: *"Core must not import from `frontend/` (see `tests/architecture.rs`), so the shared thing lives"* here.

**Directly relevant to Cena's multi-session requirement**: separating *model* (pure, testable) from *transport* (hub), and putting the cross-process registry in core because the architecture rule forbids the alternative.

### `alerts` (719) + `alert_timers` (219)

`alerts.rs:1-13`:

> Core-owned alert state: the live set of overlay alerts plus the discipline layer that keeps them from becoming noise.
>
> This lives in core, NOT in a frontend, for the same reason the injury doll does: alerts are state, and state that is computed during rendering **double-fires on detached viewports**, can't be pushed to the phone bridge, and **turns painting into a side effect**. Frontends read `ActiveAlert`s and draw them; they never decide what fires.
>
> Anti-spam is not a nicety here — **it is the feature.** A heavy-scroll group encounter (Reim, an invasion) is precisely when alerts matter and precisely when an undisciplined trigger engine buries the screen and gets the whole feature switched off. Hence: per-rule cooldowns and a hard concurrent cap.

Two arguments Cena needs verbatim: (a) *computing state during render double-fires when there is more than one viewport*, which is exactly Cena's desktop+mobile case; (b) *rate discipline is a feature of a trigger engine, not a polish item* — which will apply to every Lua trigger a user writes.

### `hotbar` (759)

`hotbar.rs:1-8`: applies the shared condition engine (`core::conditions`, 1,010 lines) to bar definitions and produces per-button display state. *"Pure logic — no frontend imports. Both TUI and GUI call `resolve_bar` each frame with `now_server = local unix time + server_time_offset` (the same convention as the countdown widget)."* Produces `ResolvedHotbarButton` (`:16`). Consumes `HotbarCountdownSource`, `HotbarDef` from config, and `GameObjData`/`GameState`.

Note the stated `now_server` convention — a *frontend-visible* contract that keeps every countdown consistent. This is the same value Rule 11 protects.

### `input_router` (111)

`input_router.rs:1-6`: *"Routes keyboard input to the appropriate `MenuAction` based on: Current `InputMode` (which widget has focus), Menu keybinds configuration, Widget context (browser vs form vs editor)."*

The whole public surface is one free function (`:13-21`):

```rust
pub fn route_input(key: &KeyEvent, mode: &InputMode, config: &Config) -> MenuAction {
    let context = get_action_context(mode);
    config.menu_keybinds.resolve_action(key, context)
}
```

Pure, 111 lines, no state. `get_action_context` maps `InputMode` → `ActionContext` and is deliberately **total** — the comment at `:25-27` notes `HotbarEditor` handles raw keys itself and is grouped in *"only so the context mapping stays total."*

### `known_windows` (43) + `local_catalog` (129) + `view_resolver` (240)

`known_windows.rs:1-11`: *"The unified 'known windows' view — U3's replacement for the separate `window_offers` registry. Every window the client knows about (from the layout, plus session-only ephemeral windows like containers and dialog panels) is enumerated here as a single list the Windows menu renders, with per-row show/hide. **There is no parallel offer registry**: a window's binding + visibility on the layout (or its ephemeral runtime state) is the single source of truth."*

`KnownWindowKind` (`:15-28`): `Layout` (persistent widget, possibly feed-bound), `Dialog` (resident, dockable game panel), `Stream` (game text stream), `Container` (*"session-only, wiped on relog"*).

The module holds only the pure descriptor + classification; the enumeration/toggle methods live in `app_core::state` *"since they need both the layout and the ui_state"* (`:9-11`). `local_catalog` and `view_resolver` are the two seams Rule 10 enforces.

### Smaller core services referenced above

| Module | LOC | One-line role (from header/usage) |
|---|---:|---|
| `messages/` | 10,977 total | `element.rs` 2,574 (element→state arms), `flush_line.rs` 1,414, `component.rs` 806, `routing.rs` 501, `buffers.rs` 493, `tests.rs` 5,319 |
| `remote.rs` | 2,722 | core-owned end of the web sidecar (§2) |
| `map_service.rs` | 2,552 | live map state; `ensure_db` / `note_room` / `poll` |
| `bestiary.rs` | 1,621 | creature reference data |
| `uipack.rs` | 1,303 | UI pack import/export |
| `conditions.rs` | 1,010 | shared condition-expression engine (hotbars, indicators) |
| `group.rs` | 978 | `GroupState`, `GroupEvent`, `GroupMember` |
| `harmony.rs` / `harmony_skin.rs` | 888 / 519 | Harmony skin system |
| `foreach.rs` | 856 | `.foreach` iteration service |
| `skill_trainer.rs` | 734 | play.net web skill manager, off-thread |
| `day_pass.rs` | 659 | `DayPassCache` |
| `emoji.rs` / `custom_emoji.rs` | 647 / 367 | |
| `bounty_parser.rs` | 492 | |
| `ghost_rooms.rs` | 469 | |
| `move_feedback.rs` | 466 | classifies movement results for the walk executor |
| `spell_table.rs` | 431 | |
| `satellites.rs` | 402 | detached viewports |
| `sorter.rs` | 400 | categorized container looks (`.sorter`) |
| `item_mover.rs` | 374 | verified `_drag` moves |
| `gameobj_data.rs` | 372 | item/creature reference data (jinx-installed) |
| `curated_maps.rs` / `classic_maps.rs` | 314 / 294 | |
| `evidence.rs` | 293 | mapping observations attributed to room uid |
| `game_art.rs` | 291 | play.net room pictures |
| `elanthian_time.rs` | 250 | in-game calendar |
| `membership.rs` | 243 | |
| `menu_actions.rs` | 217 | `MenuAction`, `ActionContext` |
| `missing_spells.rs` | 188 | |
| `data_pack.rs` | 174 | |
| `inline_image.rs` | 149 | |
| `placement.rs` | 134 | |
| `window_style.rs` | 266 | |

---

## 8. Config system (`src/config/` + `src/config.rs`, 32,099 + 595 lines)

**30 submodules.** `src/config.rs` is a facade under a 700-line cap (Rule 8), currently 595. It declares the submodules at `:15-43` and re-exports at `:45-98`.

| Module | LOC | Covers |
|---|---:|---|
| `keybinds.rs` | 5,613 | keybinds + controller binds/wheels/rumble/tuning |
| `widgets.rs` | 2,383 | widget/window templates, **`TuiPlacement`** (Rule 3) |
| `pool.rs` | 1,965 | image pool (shortcode-named art — the `vellumImg` source) |
| `layout.rs` | 1,950 | `Layout`, `LayoutConfig`, `ContentAlign` |
| `skins.rs` | 1,775 | |
| `skin_pack.rs` | 1,702 | |
| `highlights.rs` | 1,454 | `HighlightPattern` |
| `settings.rs` | 1,388 | the `*Config` structs (below) |
| `colors.rs` | 1,115 | `ColorConfig` — presets, prompt colors, UI colors, spell colors, palette |
| `profiles.rs` | 1,039 | launcher profiles (**a `keyring` gate site**, per Rule 7) |
| `registry.rs` | 1,021 | |
| `spacer_tests.rs` | 983 | tests |
| `window_def.rs` | 979 | `WindowDef` |
| `io.rs` | 962 | load/save |
| `alertpacks.rs` | 948 | `AlertPack`, `AlertPackApprovals`, `RoomScope` |
| `presets.rs` | 788 | `IndicatorTemplateEntry`, `IndicatorTemplateStore`, `StatusIconState` |
| `hotbars.rs` | 731 | `HotbarDef`, `HotbarsConfig`, `HotbarCountdownSource` |
| `room_images.rs` | 690 | |
| `defaults_refresh.rs` | 610 | |
| `paths.rs` | 533 | every config path; `write_atomic`, `is_valid_layout_name` |
| `wrayth_import.rs` | 532 | **import from the Wrayth/Stormfront client** |
| `scenes.rs` | 505 | `StageScene` |
| `macros.rs` | 484 | `MacroButton`, `MacroGroup`, `MacroOption`, `MacrosConfig` |
| `sparse.rs` | 363 | sparse/inherit save semantics |
| `appearance.rs` | 352 | `AppearanceSettings` (per-character `appearance.toml`) |
| `conditions.rs` | 345 | condition config |
| `menu_keybind_validator.rs` | 284 | |
| `window_registry.rs` | 249 | `WindowRegistry`, `RegistryBinding` |
| `png_meta.rs` | 201 | PNG metadata (skins carry config in-image) |
| `creature_field.rs` | 155 | `FieldOverrides` |

### The `Config` struct

`src/config.rs:296-378`. **43 fields.** The striking property: **17 are `#[serde(skip)]`** — loaded from *separate files*, not from `config.toml`. `Config` is an in-memory aggregate of many on-disk files, not a mirror of one.

Typed sub-configs from `settings.rs`: `HighlightsConfig` (26), `ConnectionConfig` (86), `UiConfig` (108), `FocusConfig` (337), `SoundConfig` (361), `TtsConfig` (412), `TargetListConfig` (485), `LoggingConfig` (571), `StreamsConfig` (709), `RoomImagesSettings` (789), `GameArtSettings` (802), `SorterConfig` (812), `WebConfig` (888), `MapConfig` (1116). Plus `Go2Config`, `QuickbarsConfig`, `EventPattern` map, `ColorConfig`, `MenuKeybinds`, `AppearanceSettings`, `MacrosConfig` (×2: merged + phone-edited local overlay).

Controller support is unusually deep for a MUD client — six `Config` fields (`:308-327`): `controller_binds` (canonical keys: a bare button `south`, or a composite `l2+dpad_down`), `controller_wheel` (default radial), `controller_wheels` (named radials), `controller_wheels_meta`, `controller_overlay` (curated HUD legend), `controller_rumble`, `controller_tuning`. The comment at `:303-307` records a **migration**: the old split base/`[controller_shift]` banks were replaced by held-modifier composites, and *"legacy configs auto-migrate."* A separate `touch_wheel` (`:314-319`) is the phone's long-press radial, *"shipped to the phone as the named 'touch' wheel and editable from both frontends"*, with slices carrying either a `client` action or a game `command`.

### File formats and locations

Root: `~/.vellum-fe/` or `VELLUM_FE_DIR` (`CLAUDE.md`). Paths resolve through three scopes in `src/config/paths.rs`: `global_dir()`, `profile_dir(character)` (per-character), and `config_dir()`.

| File | Scope | `paths.rs` line |
|---|---|---:|
| `config.toml` | global **and** per-character | 287 / 104 |
| `colors.toml` | global and per-character | 281 / 110 |
| `highlights.toml` | global and per-character | 234 / 355 |
| `keybinds.toml` | global and per-character | 240 / 361 |
| `controller.toml` | global base + per-character override | 250 / 258 |
| `hotbars.toml` | global and per-character | 264 / 367 |
| `room_images.toml` | global and per-character | 269 / 275 |
| `layout.toml` | per-character | 373 |
| `widget_state.toml` | per-character | 305 |
| `alertpack-approvals.toml` | config dir | 166 |
| `spell_abbrev.toml` | global | 349 |
| `cmdlist1.xml` | global | 343 | **the one XML config file** — Simu's command list, used for context menus |
| `layouts/<name>.toml` | named layouts | 404 |
| `keybinds/<name>.toml` | named keybind profiles | 436 / 469 |

Not in `paths.rs` but named elsewhere: `layout_v1.json` (the GUI's own layout + `ui_settings`, per Rules 4 and 5 — **the only JSON**), `appearance.toml` (per-character, `config.rs:350-355`), `macros.toml` + `macros-local.toml` (`config.rs:374-377`), `touch_wheel.toml` (per-character, roams, `config.rs:314-319`), `jinx-installed.toml` (`core/jinx/metadata.rs`).

So: **TOML everywhere, one XML file (Simu's), one JSON file (the GUI's layout).**

Two mechanisms worth carrying to Cena:

1. **Embedded defaults.** `CLAUDE.md`: *"Defaults embedded from `defaults/` directory via `include_dir` crate."* First run needs no network and no installer step.
2. **Sparse save with explicit clears.** `src/config.rs:380-386` documents `empty_string_as_none`: *"The sparse save writes an empty string to record a deliberately cleared Option, since TOML cannot express null and an omitted key means 'inherit'."* A three-state encoding — present / explicitly-cleared / inherit — over a two-state format. Any Cena config with global→per-character inheritance needs this exact distinction.
3. **Atomic writes.** `write_atomic` is exported from `paths.rs` (`config.rs:74`) — config saves are never torn.
4. **Import path from the incumbent.** `wrayth_import.rs` (532 lines) reads the official client's settings. A migration path from Lich's own config is the Cena analogue.

---

## 9. Gaps vs Lich

What Lich-5 tracks or does that VellumFE's core has **no equivalent for**. Every row cites both sides. These are the holes Cena's Lua layer and core must fill; Vellum's core is not a superset of Lich.

### 9.1 Character attributes — the largest single gap

| Lich has | Vellum has |
|---|---|
| **Ten stats** with four values each. `lich-5/lib/attributes/stats.rb:29` declares `@@stats = %i(strength constitution dexterity agility discipline aura logic intuition wisdom influence)` and `:30-40` metaprograms an accessor per stat returning base `value`/`bonus` and enhanced `value`/`bonus`. The parser writes four keys per stat: `'stat.%s'`, `'stat.%s_bonus'`, `'stat.%s.enhanced'`, `'stat.%s.enhanced_bonus'` (`lib/gemstone/infomon/parser.rb:254-257`) plus `'stat.%s.base'` / `'stat.%s.base_bonus'` from `info full` (`:260`). | **Nothing.** `grep -n "strength\|dexterity\|aura\|logic\|intuition\|wisdom" src/core/state.rs` returns **zero matches**. `GameState` has no stat fields at all. |
| `Stats.race`, `.profession`, `.gender`, `.age`, `.level` (`lib/attributes/stats.rb:6-28`), backed by `stat.race` / `stat.profession` / `stat.gender` / `stat.age` (`lib/gemstone/infomon/parser.rb:242-249`) and `stat.experience` (`:250`). | `CharacterState.profession` and `.level` only (`src/core/character_state.rs:121, 125`). **No race, gender, or age.** |
| **Skills** — `lib/attributes/skills.rb` (82 lines) with `Skills.to_bonus(ranks)` implementing the GS ranks→bonus curve (`:9-30`) and per-skill accessors. | **Nothing for GemStone.** `grep -n "skill" src/core/state.rs` matches only `DRExperienceState`, the *DragonRealms* component feed (`src/core/state.rs:278-279, 1019-1032`). GS4 skill ranks are untracked. |

This is the single biggest hole. Vellum's `CharacterState` header is explicit that it is *"a focused port … for the fields that gate travel"* (`src/core/character_state.rs:1-4`) — nine fields against Infomon's hundreds of keys. Any Cena script doing `Char.strength` or `Skills.combatmaneuvers` has nothing to read from today.

### 9.2 PSMs (Combat Maneuvers, Feats, Weapon/Armor/Shield techniques, Warcries, Ascension)

| Lich has | Vellum has |
|---|---|
| `lib/gemstone/psms.rb` (267) plus eight databases: `cman.rb` (849), `qstrike.rb` (865), `feat.rb` (443), `shield.rb` (426), `weapon.rb` (405), `armor.rb` (287), `warcry.rb` (255), `ascension.rb` (159) — **3,956 lines** of ability tables and rank tracking. Infomon parses keys like `'warcry.bertrandts_bellow'`, `'warcry.carns_cry'`, `'warcry.gerrelles_growl'`, `'warcry.horlands_holler'`, `'warcry.seanettes_shout'`, `'warcry.yerties_yowlp'` (`lib/gemstone/infomon/parser.rb`). | **Nothing.** No `psm`, `cman`, `feat`, `warcry` or technique module in `src/core/mod.rs:7-59`. |

### 9.3 Detailed experience

| Lich has | Vellum has |
|---|---|
| `'experience.total_experience'`, `'experience.long_term_experience'`, `'experience.field_experience_current'`, `'experience.field_experience_max'`, `'experience.ascension_experience'`, `'experience.deaths_sting'`, `'experience.deeds'`, `'experience.fame'` (`lib/gemstone/infomon/parser.rb`), plus `lib/gemstone/experience.rb` (109). | **Partial.** `ParsedElement::MindStateExp` (`src/parser.rs:148-164`) carries `field_exp`, `max_field_exp`, `exp`, `ascension_exp`, `until_next`, `fashlonae`, `lumnis`, `rpa` — arriving as attributes on `<progressBar id='mindState'>`. Landing fields exist around `src/core/state.rs:1566-1586` (`ascension_exp`, `until_next`, `fashlonae`, `lumnis`, `rpa`, `ptps`, `mtps`, `atps`). **No `deaths_sting`, `deeds`, or `fame`.** |

Note the asymmetry of *mechanism*: Vellum reads exp from the extended-feed XML attributes (richer, structured, event-driven); Lich reads it from `EXP` command text. Vellum's channel is better where it overlaps.

### 9.4 Status conditions

| Lich has | Vellum has |
|---|---|
| `'status.bound'`, `'status.calmed'`, `'status.cutthroat'`, `'status.silenced'`, `'status.sleeping'`, `'status.thorned'` (`lib/gemstone/infomon/parser.rb`) — **text-derived** conditions with no XML indicator, plus `lib/gemstone/infomon/status.rb` (56). | **Nothing for these six.** `StatusInfo`'s typed list (`src/core/state.rs:396-411`) is the 13 XML-`indicator` states: `standing, kneeling, sitting, prone, stunned, bleeding, hidden, invisible, webbed, joined, dead, poisoned, diseased`. |

Important distinction: `StatusInfo` has an **open vocabulary** (`set()` accepts any id, `src/core/state.rs:355-357`), so the *container* can hold `bound`/`calmed`/`silenced` — what is missing is the **text parser that would set them**. That is the exact job `CharacterState::parse_line` (`src/core/character_state.rs:200`) does for nine other fields, so the extension point exists.

### 9.5 Spell tracking

| Lich has | Vellum has |
|---|---|
| `lib/gemstone/infomon/activespell.rb` (193) + `lib/gemstone/spellranks.rb` (80) + `lib/gemstone/effects.rb` (89): per-spell active state, durations, and **circle ranks**. | `GameState.effects: HashMap<String, ActiveEffectsContent>` (`src/core/state.rs:105`) keyed by category (ActiveSpells/Buffs/Debuffs/Cooldowns/Timers), fed by `ParsedElement::ActiveEffect` (`src/parser.rs:261-268`); `spell_names_seen: HashMap<u16,String>` (`:257`); `spellbook: Vec<StyledLine>` (`:179`) — **raw rendered text, not parsed**; `core/spell_table.rs` (431); `core/missing_spells.rs` (188). **No spell ranks per circle.** |

Vellum tracks *what is active* (well, from the dialog feed) but not *what the character knows*. `spellbook` being `Vec<StyledLine>` is the tell: it is stored for display, not for query.

### 9.6 Wounds and scars

| Lich has | Vellum has |
|---|---|
| `lib/gemstone/wounds.rb` (105) and `lib/gemstone/scars.rb` (110) as separate queryable models. | `GameState.injuries: HashMap<String,u8>` (`src/core/state.rs:117`) — a flat part→severity map, fed by `ParsedElement::InjuryImage { id, name }` where `name` is `"Injury1".."Injury3"` or `"Scar1".."Scar3"` (`src/parser.rs:225-228`). Wounds and scars are **collapsed into one map**. |

The raw data is present on the wire and parsed; the *model* does not separate the two axes. Note also `creature_cards.rs:6` records the deliberate decision that creatures take *"wounds only (injury1-3 + healthy) — no scars"*, so the asymmetry is intentional on the creature side.

### 9.7 The scripting engine itself — the defining gap

| Lich has | Vellum has |
|---|---|
| `lib/common/script.rb` — **3,675 lines**: the script lifecycle, threads, `start_script`/`kill_script`, per-script state. | **Nothing comparable.** |
| `lib/common/hook_registry.rb` (189) and `lib/common/downstreamhook.rb` (62): scripts register hooks that can **rewrite or suppress the downstream feed** before it reaches the client. | No hook registry. The nearest thing is the fixed pipeline: parser → `MessageProcessor` → highlight engine (which *can* rewrite text, via `DeferredReplacement` / `apply_deferred_for_window`, `src/core/mod.rs:61-64`) — but it is driven by config patterns, not by registered code. |
| `lib/common/buffer.rb` (176): the shared game-line buffer scripts read from. | `GameState.recent_lines: VecDeque<(u64,String)>` (`src/core/state.rs:246`) — but **off by default**: `capture_recent_lines` is raised only while a travel task runs (`src/core/messages.rs:198-202`). |
| ~473 community `.lic` scripts (`E:/Cena/reference/scripts`, `E:/Cena/reference/dr-scripts`). | 106 dot-commands (§4), of which ~8 are game automation. |

Vellum's automation is a **closed set of native features** (`travel`, `foreach`, `item_mover`, `skill_trainer`) reached through a single lease (`AutomationOwner`, `src/core/app_core/automation.rs`, 127 lines). Lich's is an **open set of user code**. This is the whole reason Cena exists, and it is why §7's note on `TravelService` holding one `task: Option<TravelTask>` matters — the host shape is right, the multiplicity is not.

What Vellum *does* provide as a bridge, and what Cena should treat as the current contract:
- `ParsedElement::VellumCommand` — *"Only dot-commands are honored downstream, so the feed can drive client UI but never send game commands"* (`src/parser.rs:85-91`).
- `ParsedElement::VellumTimer` — script-driven countdowns (`:78-84`).
- `ParsedElement::VellumImage` — *"`src` is a pool image NAME … never a path"* (`:92-104`).
- `ParsedElement::LichWebUI` / `AppCore::webui` (499 lines) — the `;ui handshake` bridge.

All four are **capability-restricted channels from Lich to Vellum**. Cena collapses the process boundary, so each becomes a sandbox-policy question instead. Both restriction comments are worth preserving as stated policy.

### 9.8 Pathing: the StringProc gap, stated by Vellum itself

| Lich has | Vellum has |
|---|---|
| `lib/common/map/map_base.rb` (954) + `map_gs.rb` (520) + `map_dr.rb` (419). Wayto edges may be **executable Ruby** (`StringProc`), and timeto gates may be Ruby predicates (portmasters, day passes, urchins). | `src/core/pathing/` (5,507) routes the graph, but `mod.rs:6-13` states the exclusions plainly: StringProc wayto commands are **excluded until the transpiler lands** (*"a path we can't walk is worse than no path"*), StringProc timeto gates **evaluate as 'off'**, urchin-hideout teleport nodes never route. `transpile.rs` (4,447) is the in-progress answer. |

This is the one gap Vellum documents against itself, and Cena inherits it directly: **Lich's map data contains Ruby, and Cena's language is Lua.** Either the transpiler is finished, or the mapdb is re-expressed.

### 9.9 DragonRealms

| Lich has | Vellum has |
|---|---|
| `lib/dragonrealms/drinfomon/` — 12 files (`drparser.rb`, `drskill.rb`, `drstats.rb`, `drspells.rb`, `drroom.rb`, `drbanking.rb`, `drexpmonitor.rb`, `drdefs.rb`, `drvariables.rb`, `events.rb`, `startup.rb`) plus `lib/common/map/map_dr.rb` (419). 222 community DR scripts. | `GameState.dr_experience: DRExperienceState` (`src/core/state.rs:279`, model at `:1022-1035`) — `field_order: Vec<String>`, `values: HashMap<String,String>`, `generation`. That is **essentially all**. Plus `MiniVitalsState::update_vital` mapping DR's `"concentration"` onto the mana slot (`src/core/state.rs:445`) and `Config` having a `GameType` concept (`src/core/local_catalog.rs:16`). |

DR support in Vellum's core is a stringly-typed passthrough of one component feed. Anything Cena wants for DR comes from the Lich side.

### 9.10 Smaller, concrete gaps

| Lich has | Vellum has | Citation |
|---|---|---|
| `lib/gemstone/bounty.rb` (41) + `lib/gemstone/bounty/` — a **parsed** bounty model | `BountyState { raw_text: String, compact_lines: Vec<String>, generation }` (`src/core/state.rs:467-473`) — raw text plus display lines; `core/bounty_parser.rs` (492) produces the compact lines but the result is `Vec<String>`, not a typed task | both |
| `lib/gemstone/society.rb` + societies dir | `SocietyState { lines: Vec<String>, generation }` (`src/core/state.rs:500-504`) — **raw lines**; the *parsed* society lives separately in `CharacterState.society`/`.society_rank` (`character_state.rs:115,118`) | both |
| `lib/gemstone/currency.rb`, `bank.rb` | `GameState.silver: Option<u64>` (`src/core/state.rs:263`) only — parsed from a `wealth` line (`src/core/messages.rs:210-212`). No bank balances, no other currencies | both |
| `lib/gemstone/gift.rb`, `fog.rb`, `claim.rb`, `disk.rb`, `overwatch.rb`, `stance.rb`, `injured.rb`, `readylist.rb`, `stowlist.rb`, `critranks.rb`, `armaments.rb`, `creature.rb` | `StanceState` (`state.rs:1772-1780`), `game_objects/ready_stow.rs` (309), `bestiary.rs` (1,621) cover stance/ready-stow/creatures. **No gift box, fog, claim, disk, or overwatch tracking** | `src/core/mod.rs:7-59` lists every core module; none match |

### 9.11 Where Vellum is ahead of Lich

Stated for balance, because a porting plan that only counts gaps will mis-size the work:

- **The extended feed.** `inventoryManager` with continuation-following (`src/core/inventory_service.rs`, 796), `inventoryViewItem` with the authoritative `closed` attribute (`src/parser.rs:394-405`), server-declared `<pulse>` (`:353-368`, replacing inference), `worldEvent`, `PantheonStatus`, `roommeta`, `crtrStatus`. `mod.rs:1-8` of `game_objects` notes Lich builds its inventory *"from the `<inv>` push feed"* — the older, thinner channel.
- **The map layout engine** (`src/core/layout_engine/`, 4,572) — Lich has pathing but draws no map.
- **Creature effects per exist-id** (`GameState.creature_effects`, `src/core/state.rs:137`) and `CreatureFlags` with 12 boolean flags plus `health`/`max_health`/`hp_estimated`/`injuries`/`condition` (`src/core/state.rs:689-718`).
- **`GameObjects`** as a single registry replacing per-feature silos (`src/core/game_objects/mod.rs:1-16`), explicitly a *rewrite* of `GameObj` rather than a port.
- **Multi-account / multi-session** (`multiaccount/` 1,533 + `session_registry.rs` 1,646) — Lich's detachable-client registry is narrower.
- **Asset distribution** (`jinx/`, 3,266) speaking the elanthia-online protocol natively, no Ruby.
- **Native skill trainer** (`skill_trainer.rs`, 734) against the play.net web skill manager.
- **The architecture test itself** — Lich has `spec/` but nothing enforcing layering.

### 9.12 Net summary for the porting plan

| Area | Vellum core status | Cena work |
|---|---|---|
| Wire parsing (GSIV) | **Strong** — 4,527 non-test lines, golden-snapshot pinned, nothing silently dropped | Port as-is; add `style` to `KNOWN_WIRE_TAGS` |
| Text/highlight pipeline | **Strong** — applied once in core, pre-colored to frontends | Port as-is; hook Lua triggers here |
| Layered architecture | **Strong** — 11 grep-enforced rules, 359 lines | Adopt verbatim, extend caps to `travel/executor.rs` and `app_core/commands.rs` |
| Android-safety | **Solved** — 7 banned crates, 4 wrappers, CI-gated | Adopt the same shape |
| Window/layout/config | **Strong** — 32k lines config, per-frontend slots | Large but mechanical |
| Game state (GS4) | **Partial** — rich on room/creature/effects/inventory; **empty on stats, skills, PSMs, spell ranks** | Port Infomon's parser tables (`infomon/parser.rb`, 839 lines) into `CharacterState`'s existing `re!`+`parse_line` shape |
| Pathing / travel | **Partial** — routes, but excludes every StringProc edge | Finish or replace `transpile.rs` (4,447 lines in flight) |
| Scripting | **Absent** | The entire Lua runtime; `TravelService`'s host shape is the model, but must go from one task to many |
| DragonRealms | **Near-absent** | From Lich's `drinfomon/` |
