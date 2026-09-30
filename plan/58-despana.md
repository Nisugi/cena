# 58 — Despana, ported whole from VellumFE

PROPOSED 2026-09-30. The author: *"I think our next port should be the full despana from
vellumfe. Would you examine and write up a plan for implementation?"*

This is the "full port of it later" that `plan/47` recorded (*"keep despana like it is, and we
will do a full port of it later"*, `plan/47-m10-gui.md:25-26`), and what `plan/57` expects:
*"despana's features will probably be the default webui"* (`plan/57-themes.md:25`).

What follows was measured on 2026-09-30 against `reference/VellumFE` at `c1f7953` (a shallow
clone, current with its `origin/main`: `git fetch` then `git log HEAD..@{u}` gives 0 commits)
and against Hydra's `main` at `50ec44c`.

---

## 1. What VellumFE's Despana is

A browser workspace built into VellumFE (`book/src/frontends/despana.md`). The Story stays in
the middle; movable modules around it show the room, the character, combat, spells,
inventory and the map. Vellum owns the connection, the parse and the state; Despana only
draws that state and sends commands back. It has no dependency on the older `/play` phone
page, apart from a link to it and a shared token key.

### 1a. Size, measured

`wc -l` over `src/frontend/web/assets/despana/*`:

| File | Lines | What it is |
|---|---|---|
| `app.js` | 2,105 | the composition root: every module's renderer, the map controller, the context menu, the command input |
| `session.js` | 1,465 | `DesktopSession`: the WebSocket adapter and state reducer, no DOM |
| `workspace.js` | 1,061 | `DesktopWorkspace`: zones in the DOM, module menus, drag, resize separators |
| `layout.js` | 841 | `WorkspaceLayout`: the pure layout model, its intents, the default layout |
| `map.js` | 417 | the Local map's camera, Classic hit tests, `ClassicRoomStore` |
| `interactions.js` | 330 | links, the noun menus, URL tabs |
| `workspace-persistence.js` | 248 | the browser's copy and the server's, with revisions |
| `macro-editor.js`, `macros.js` | 205, 138 | the Macros & Hotkeys dialog; hotkey names and dispatch |
| `inventory-refresh.js`, `inventory-tree.js` | 115, 99 | the nested inventory's request and tree |
| `font-scale.js` | 35 | the font-scale setting |
| `index.html`, `app.css` | 305, 1,569 | five zones and 16 modules; dark tokens, responsive rules |

That is 6,938 lines of page. There are 5,081 lines of Node tests (`*.test.mjs`, 12 files, one of
them a real-Firefox smoke test through a hand-written WebDriver client). The server side is
`server/despana.rs` (639 lines), and the shared v1 protocol is `protocol.rs` (2,274 lines).
There is no bundler, no npm and no `build.rs`: plain ES modules embedded with `include_str!`
(`server/despana.rs:60-135`).

### 1b. Its modules

Each module is registered with the slices of state it reads (`app.js:532-678`). The workspace
re-renders only the modules whose slices changed (`workspace.js:261-286`).

| Module | Shows |
|---|---|
| Story | the main stream; styled segments and links; Pause and Bottom; the command input attached below (`index.html:138-159`, `app.js:469-516`) |
| Thoughts, Familiar | their streams. Only these three streams get a pane; the rest are kept and never drawn |
| Room | the name and id, the description, creatures then players then objects as links, and exits as buttons (`app.js:1604-1662`) |
| Compass | an 11-button grid, hidden by default |
| Hands / Prepared | left, right, the prepared spell |
| Vitals | four bars, value / max; mind, encumbrance and stance gauges with thresholds (`app.js:777-796`) |
| Conditions / Roundtime | a posture pill, an RT/CT countdown, and 8 condition pills (`app.js:1683-1734`) |
| Active Spells, Cooldowns | effects by category, time left against the server's clock, a bar (`app.js:680-736`) |
| Known Spells | the spellbook's lines, hidden by default |
| Injuries | text only (*Part: wound rank N*), with a picker for Injuries, Scars or Both. There is no picture (`app.js:745-775`) |
| Combat | five stance buttons, and the targets as buttons that open their menus (`app.js:798-856`) |
| Tasks / Bounty / Society | objective cards with action buttons, plus the bounty and society lines (`app.js:858-905`) |
| Map | Local (a canvas of the room graph, following the character) or Classic (an annotated image with a marker). A picker of maps; click to `.go2`, Ctrl+click to copy the room id (`app.js:907-1602`) |
| Inventory | the flat inventory feed, or with *Show Nested* a collapsible tree from `_inventory manager`, with Refresh (`app.js:256-339`) |

Around the modules:
- a menu bar: Exit & Log Out; View (font scale 75-200%); Workspace (hidden modules,
  Restore default); Macros & Hotkeys;
- a title of *name - profession - level* and a connection status;
- an overlay while nothing is playable (pairing, connecting, no session);
- the game's context menu.

### 1c. The workspace

The workspace has five zones: top, bottom, left, right and centre (`layout.js:10-16`).
- Each zone flows horizontally or vertically. Its modules carry weights that sum to 1,000.
- The four outer tracks are sized in pixels (48-4,096).
- Every change is an *intent*: move, hide, show, set-flow, resize-track, resize-pair,
  reset (`layout.js:170-357`).
- A hidden module remembers its neighbours, so showing it puts it back where it was.

Saving:
- The layout is kept per character, in the browser's `localStorage` first, then on the
  server at `PUT /api/v1/presentations/despana/workspace`.
- The server writes `despana-workspace-v1.json` in the character's profile folder,
  compare-and-write by revision. An older write is refused with 412 (`server/despana.rs:270-401`).

The default layout is VellumFE's *"proven Calvix starting layout"* (`layout.js:40-116`,
`layout.test.mjs:89`):
- top: Thoughts and Familiar;
- bottom: Hands, Conditions, Cooldowns and Injuries;
- left: Active Spells, Combat and Vitals;
- right: Map and Tasks;
- centre: Room over Story.

### 1d. The wire it speaks

VellumFE's shared v1 protocol, written for the phone page (`protocol.rs:1`). Each message is an
envelope `{v:1, seq, t, d}`.

The handshake is `auth`, `hello`, `subscribe`, `resume`, then a `snapshot` (full, resume or
gap), then `macros` (`server.rs:1800-1918`).

After that, **each delta replaces one whole slice**:
- `room`, `hands`, `vitals`, `minivitals`, `indicators`, `rt`, `prepared_spell`;
- `entities`, `effects`, `spells`, `inventory`, `inventory_tree`, `injuries`;
- `targets`, `objectives`, `charinfo`, `map_scene`, `map_state`.

There are no patches. The page sends `cmd`, `link_tap`, `macro`, `macro_save`, `macro_delete`,
`map_locations`, `map_view` and `exit_logout` (`session.js:750-851`). Commands are never
acknowledged: a command caught by a closed socket is reported *uncertain* and never resent
(`session.js:858-863`).

### 1e. What it does not do

These gaps were measured in the VellumFE source:
- no Find (`macros.js:7` keeps Ctrl+F for the browser);
- no item drag;
- no graphical doll (the `doll` slice is read and never drawn);
- no settings, highlights or stream editors;
- no login (the overlay links to `/play`);
- no pinch zoom on the map (`map.js:104`);
- *Help* is a label, not a menu (`index.html:33`).

---

## 2. What Hydra has today

### 2a. `cena-web`

`cena-web` has 3,465 lines of Rust in `src/`.

The page is `assets/index.html`, `app.js`, `session.js` and `style.css`: 1,224 lines in all.
- Hydra's own work, *"scoped from VellumFE Despana"* (`style.css:1-16`), and a demo of Despana,
  not Despana (the author, 2026-09-26).
- It shows the hands, the room with a minimap from the atlas, the story with streams you can
  open, the command input with receipts, four vitals and the roundtime.
- It has no links, no command history, no layout, and nothing of the character beyond vitals.

What is better than VellumFE and must survive the port:

| Hydra has | Where |
|---|---|
| A hub page: a card per character, add, quit, reconnect, shut down, and the merged thoughts, speech, logons, deaths and announcements | `socket.rs:231-332`, `app.js:290-358` |
| **Receipts**: a command is answered `sent`, `handled`, `refused` or `uncertain`, one pending at a time, never replayed. VellumFE has no acknowledgement | `socket.rs:360-383` |
| A strict page: an exact `Host`/`Origin` check, a CSP with `script-src 'self'`, no query strings, a pairing token kept per tab in `sessionStorage` | `server.rs:519-554`, `session.js:6-45` |
| Every incoming message checked field by field, colours as `#rrggbb` only | `session.js:56-120` |
| Trigger banners (`alert`) | `hub.rs:142`, `session.js:123-124` |
| The atlas explorer and its live minimap; the experimental hunt setup | `atlas.rs`, `atlas/minimap.mjs`, `hunt_setup.rs` |

### 2b. The wire

Hydra's wire is `cena-ui`'s `WIRE.md`, version 1.
- A `snapshot`, then an `update` that carries **the whole `SessionView`** each time something
  changed (`hub.rs:156-254`).
- `SessionView` is deliberately small: room, hands, four vitals, roundtime, lifecycle, prompt,
  group and map location (`view.rs:294-319`). `plan/49` keeps it *"the remote viewers' format"*
  (`plan/49-gui-widgets.md:141-142`).
- A wire message is capped at 512 KiB (`presentation.rs:24`).

### 2c. The GUI already did most of the model work

Everything a VellumFE module draws is already in Hydra's model, because the egui GUI draws it
too (`cena-gui/src/widget/kind.rs:17-124`, 66 kinds):

| VellumFE module | Hydra's model and the GUI's widget |
|---|---|
| Story, streams | every stream, routed by `<streamWindow>` (`StoryLine`, `Closed`) |
| Room and its lists | `Room`, `Creatures`, `Npcs`, `Objects`, `Players`, `Exits` (`widget/described.rs`) |
| Compass | `Compass` (`widget/room.rs`) |
| Hands / Prepared | `LeftHand`, `RightHand`, `Prepared` |
| Vitals and gauges | the four bars, `Mind`, `Stance`, `Encumbrance`, `FieldExperience` |
| Conditions | 19 `Indicator`s, `Roundtime`, `CastTime`, `Stun` (`widget/status.rs:19-130`) |
| Active Spells, Cooldowns | `Effects(Category)`, four categories (`status.rs:161-190`) |
| Known Spells | `Spellbook` |
| Injuries | `Injuries`, with a Text style (`widget/doll_text.rs`) |
| Combat's targets | the game's `dDBTarget` list (`plan/52` step 7) |
| Tasks, Bounty, Society | `Objectives`, `Society` |
| Map, Local | `Minimap` over `cena_ui::MapScene` (`plan/53`) |
| Nested inventory | `state/inventory_snapshot.rs`, `<inventoryManager>` read whole, used by travel's routines |
| Links and menus | `RunLink`, `object_menu`, `link_command` in `cena-ui` (`object_menu.rs:38,87`), the `_menu` round trip (`cena-gui/src/play/links.rs:1-16`) |
| Hotkeys | `keybinds.toml` and each character's `.keys.toml`, ten macro sets (`plan/52`) |

So the port **is almost all presentation**. The model work is done; what is missing is the
wire carrying it and the page drawing it.

---

## 3. The shape of the port

### 3a. Port the page, keep Hydra's transport

VellumFE's page divides cleanly:

- **Pure modules**: `layout.js`, `workspace-persistence.js`, the camera and hit tests in
  `map.js`, `interactions.js`, `macros.js`, `inventory-tree.js` and `font-scale.js`. Each has
  its own Node tests. These come across **nearly as written**. What changes is only where they
  meet the wire.
- **DOM modules**: `workspace.js`, and `app.js`'s renderers. They come across, **split by
  module**, so no file passes the cap proposed in §5 item 6. `app.js`'s 2,105 lines become a
  file per module family.
- **`session.js`, the reducer**: it is **rewritten** over Hydra's wire. Hydra's own
  `session.js` is its base, because it has the receipts, the checks and the no-replay rule.
  VellumFE's slices are what it reduces.

VellumFE's v1 protocol is **not** adopted. `plan/m4-despana-implementation.md:19` already
settled that there is no compatibility promise with it. Hydra's wire is stricter, and has
receipts.

### 3b. The wire grows slices: version 2

Carrying 16 modules' state as one `SessionView`, resent whole on every change, would send the
spellbook and the effects again at every prompt. Instead, the wire does what VellumFE's does:
**a snapshot, then deltas that each replace one slice**.

- **The slices**: room, hands, vitals, indicators, timers, effects, spellbook, injuries,
  targets, character (gauges, bounty, society), objectives, inventory tree, map, and lines per
  stream.
- **Where they are projected**: in `cena-ui`, each from `GameState`, pure and tested beside
  `SessionView::project` (`projection.rs:25`).
- **How they are sent**: `cena-web`'s pump sends only the slices that changed.
  The hub's cards keep `SessionCard` as they are.

The page and server ship in one binary, so v2 replaces v1 outright. The version number is
bumped so a stale open tab is told to reload rather than misreading.

`plan/57` step 7's theme message fits into v2 as one more message, rather than being a
version step of its own.

### 3c. Where things live

| Piece | Where |
|---|---|
| Slice types and projections, the injury text, the menu labels | `cena-ui` (pure; its toolkit ban, `layering.rs:302-321`, holds) |
| The pump, the routes, the workspace file, the requests answered for one viewer (menus, map views) | `cena-web` |
| The page | `cena-web/assets/`, replacing today's `index.html`/`app.js`/`style.css`; the hub mode kept |
| The workspace file | the character's folder in Hydra's data folder, beside its settings |

The injury text is used by both frontends, so `widget/doll_text.rs`'s rules **move down into
`cena-ui`** and the GUI calls them from there.

**The map.** Today `crate::atlas` runs only with a window (`crates/cena/src/play.rs:171-175`).
With `--web`, it runs too, and each scene goes to the page as the map slice.

---

## 4. Steps

Each step is a commit on branch `despana`, with tests, and each leaves the page working.

0. **The rules first.**
   - The file cap reaches `.js`, `.mjs` and `.css` (§5 item 6).
   - `assets/tests/session.test.mjs` joins CI; today nothing runs it (`grep` of `.github/`
     finds no reference).
   - The CI job gains `node --test` over the ported modules as they arrive.
1. **Wire v2.**
   - The slices in `cena-ui`, each projected from `GameState` with a test from a recorded
     session.
   - The pump sending only what changed.
   - `WIRE.md` rewritten, and its fixture cut again.
   - Today's page moved onto v2 unchanged in look, so step 1 alone changes nothing a player
     sees.
2. **The workspace.**
   - `layout.js`, `workspace.js` and `workspace-persistence.js` and their tests.
   - The server's `GET`/`PUT` of the workspace, revisioned, per character.
   - VellumFE's default layout, minus the modules not yet built.
   - The menu bar's Workspace menu.
3. **The modules**, in this order: Story and every stream (not three), Room, Compass,
   Hands/Prepared, Vitals, Conditions, Active Spells and Cooldowns, Combat, Tasks, Known
   Spells, Injuries.
   - Each is a renderer over its slices, in its own file.
   - Each is shown in the browser smoke test, fed from a fixture.
4. **Interactions.**
   - Links, and the game's menu.
     - The menu's request and its answer go to the one viewer that asked.
     - The rule is the GUI's: a reconnect closes the request (`play/links.rs:12-16`).
   - Exits as buttons; the target buttons.
   - Web addresses opened in a new tab.
   - Command history on Up and Down.
   - Pause and Bottom.
5. **Hotkeys.**
   - The page reads and runs the character's **Hydra keys**, not a set of its own (§5 item 4).
   - A key the browser keeps for itself is reported, as `macros.js` reports it.
   - The Macros & Hotkeys dialog edits through the Keys page's own writer (`keys/write.rs`).
6. **The map.**
   - Local from `MapScene`, drawn on a canvas by `map.js`'s camera.
   - A click sends `.go2 <room>`; Ctrl+click copies the id.
   - Classic follows §5 item 5.
   - The atlas explorer stays as it is.
7. **The nested inventory.**
   - *Show Nested* asks the game for `<inventoryManager>` the way travel's routines already do.
   - The tree is projected from `inventory_snapshot`.
   - VellumFE's refresh states and its 35-second limit carry over.
8. **The finish.**
   - Font scale; the title; the overlay; Exit & Log Out as the hub's quit.
   - The browser smoke test rewritten over the whole workspace.
   - The demo page retired.
   - `WIRE.md`, the architecture page and the glossary brought up to date.
   - `plan/m4-despana-handoff.md`'s stale lines corrected.

After step 8, Despana is VellumFE's, on Hydra. What Hydra could add beyond it is §6.

---

## 5. Questions for the author

1. **Replace the page at `/`, or open Despana at `/despana` beside it?**
   Recommended: replace it. The demo has no users (the author: *"I've never used despana"*,
   `plan/47-m10-gui.md:46`). The hub stays the page a link without a character reaches.
2. **Is the default layout VellumFE's?** It is the "Calvix" layout in §1c. Recommended: yes,
   unchanged. Hydra's streams beyond Thoughts and Familiar start hidden.
3. **Wire v2 with slices (§3b)?** Recommended: yes. The other road, widening `SessionView`,
   resends everything on every change.
4. **Hotkeys: Hydra's keys, or a set of Despana's own?**
   - VellumFE keeps Despana's hotkeys apart from its desktop keys (`despana.md`: *"These
     bindings are Despana-specific"*).
   - Recommended: one set. The same key does the same thing in the window and the page, and
     `plan/52`'s sets and per-character files already hold it.
5. **The Classic map.**
   - VellumFE's Classic view is an annotated image per area, with a rectangle per room.
   - Hydra's map is hydra-mapper's `gs.map`. The images would come from `reference/mapdb`'s
     `map-data`, or not at all.
   - Recommended: Local only in this plan. Classic comes later if you want it, since the
     atlas explorer already covers browsing.
6. **Cap the page's files at 800 lines, as Rust is capped?**
   - Today nothing caps `.js`, `.mjs` or `.css` (`harness.rs:331`), so VellumFE's 2,105-line
     `app.js` would come across whole.
   - Recommended: yes, a test in `file_rules.rs`, adopted in step 0 before the code it governs.
7. **Copying VellumFE's code.**
   - VellumFE is GPL-3.0 (`reference/VellumFE/LICENSE`), and Hydra has no licence file. The
     clone is shallow, so who wrote each Despana file cannot be read from it.
   - If Despana is all yours, copying is yours to decide. If a collaborator wrote it (the M4
     record names other people at work on Hydra's first page, `plan/m4-despana-status.md`),
     say whether the pure modules may be copied, or should be rewritten from their tests.
   - Recommended: copy, and credit at the top of each ported file, as `plan/57` credits Niffy.
8. **Phone.** VellumFE's rules below 1,050 and 700 pixels come across with the CSS.
   `plan/57` expects a separate phone page if a better one is not found. Recommended: nothing
   more here.

---

## 6. Beyond VellumFE's Despana, later

Each of these is something Hydra has and VellumFE's page does not. Each is a step to choose
after step 8, not part of this plan's scope:

- Find; an item carried by drag (`_drag`, `cena-gui/src/carry.rs`); the injury doll as a picture.
- The hunt's panel, and behaviour status and stop (`plan/44` Q11).
- The settings pages. `cena_ui::settings` is already drawn by any frontend (`settings.rs:1-8`),
  and needs only messages to cross the wire.
- The agent's notices (`plan/35`: *"Despana, not a new frontend, carries the player's side"*).
- The trigger editor.
- The GUI's other widgets: Pulse, World events, Blood Points, Aim, the stun timer, Resources,
  Encumbrance detail.
- Themes (`plan/57` step 7), once that plan is built.
- A login in the page. This stays out: `plan/49` keeps a password off every socket
  (`plan/49-gui-widgets.md:594`).
