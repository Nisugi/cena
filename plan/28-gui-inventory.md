# 28 — The GUI: what VellumFE has, and what the successor should not repeat

**Status: INVENTORY + PROPOSAL (Claude, 2026-09-22).** The inventory is measured and
citable. The design direction rests on two author decisions recorded below; everything
past §5 is a proposal and not yet approved. `plan/12` wins any contradiction.

Author decisions this document rests on (2026-09-22):

- **egui.** *"we will use egui."* Not a re-litigation of the frontend question; it
  settles the toolkit.
- **The window model is free.** *"When I designed vellum I didn't consider we don't
  have to stick to the rigid window structure that wrayth does (the official fe). We
  can make what ever we want. Which was part of the push behind designing cena the way
  we have. Everything is at our fingertips, we just have to figure out how we want to
  use it."*
- **The layout shape (§7d).** Main area plus four drawers, all carrying the same
  grid/snap system, as in Vellum — *"maybe a little less buggy!!"* — **plus a container
  window whose interior is that same grid**, so widgets can be arranged inside one
  window. *"This solves the problem that vellum's window grouping created."*
  The model underneath is **free rects with the grid as a snap target**, pitch
  adjustable; **not** real cells. Widgets inside a container are **bare; the container
  owns the chrome.**
- **Drawer behaviour (§7d.5).** Drawers keep Vellum's per-zone choice to **clip or push**
  the center (`Overlay` vs `Reserve`) and its **full translucency**. New is
  **click-through**, governed by one rule: *"the goal is to be able to see what you're
  clicking."* A click on a transparent drawer's bare backdrop — where no **widget** sits
  — falls through to the center pane; an opaque backdrop stops it, because the user
  cannot see what they would be hitting. **Derived from opacity, not a separate setting.**
  Drops still target the drawer.
- **Why the inventory exists**, in the author's words: *"It's hard to put my finger on
  it. It was vibe coded and not planned properly. That's why I want the inventory so I
  make sure I'm thinking of everything."* So this is a **completeness checklist**, not
  a post-mortem hunting for named mistakes. The failure was the absence of a plan; the
  deliverable is the plan that was missing.
- **Sequencing: plan now, build at M10.** `12` §8's table is unchanged. M5
  (multi-session) and M8 (highlights/customization) land first and *inform* the GUI
  rather than being retrofitted into it.

> **This document does not schedule work.** GUI is `12` §8's **M10**, the last row.
> Writing it now is cheap and the inventory is perishable; building it now is not on
> the table.

---

## 0. The finding that outranks the rest: the author already wrote this plan

`reference/VellumFE/.beads/artifacts/window-system-redesign/spec.md` — **225 lines,
dated 2026-08-05**, not present in Cena's `plan/`. It is a complete redesign with owner
decisions, a seven-phase strangler-fig migration, a risk register, and verification
against 11.4 GB of real logs. It opens:

> *"VellumFE was built template-first: ~55 hardcoded window templates (a 1500-line
> `match`) define what windows can exist, what they bind to, and where they sit. But
> Wrayth is a declaration-first protocol."*

And it names the root cause in one sentence:

> **"Templates conflate three orthogonal concerns: identity (which game feed),
> presentation (which renderer), and default geometry/config."**

That is the answer to *"it was vibe coded."* The diagnosis exists; it was written after
the fact, under the weight of 81,702 lines, and the fix was never finished. VellumFE's
HEAD commit is `c1f7953 refactor(config): the window catalog becomes data, not a
1900-line match` — the same fight one layer down, whose message reads *"data wearing a
function's clothes."*

**The successor starts from that spec's conclusion. It does not re-derive it a third
time.** A copy is preserved at `reference/VellumFE/.beads/artifacts/window-system-redesign/spec.md`;
§7 below records what carries over and what Cena has already made unnecessary.

### 0a. Wrayth itself agrees with the decomposition

The spec's most load-bearing evidence is not an argument, it is the official client's own
behaviour. Lich logs capture Wrayth's layout sync **back to the server** — `<!-- CLIENT
--><stgupd>` blocks — where every window is:

```
<w id=… vis frame="panel|float" location panel="Left|Right" x y width height open detach ts/>
```

organised into `<panels><group id='Left'>` lists of `<dialog/>`, `<stream/>`, `<builtin/>`.

> **Wrayth models a window as binding-id + kind + zone + frame + rect + open/vis** —
> exactly the `WindowBinding` + `ViewKind` + `PlacementHint` + visibility decomposition
> the redesign proposes.

So "don't imitate Wrayth's rigid windows" and "adopt Wrayth's window *model*" are not in
tension. The rigidity was never in the protocol. It was in Vellum's **templates**, which
are a client-side invention the protocol never asked for.

---

## 1. Scale — MEASURED

Measured 2026-09-22 against `reference/VellumFE` at `c1f7953`.

| Fact | Value | Command |
|---|---|---|
| VellumFE total Rust | **324,605** LOC | `find src -name '*.rs' \| xargs wc -l \| tail -1` |
| `src/frontend/gui` | **81,702** LOC, **87** files | `find src/frontend/gui -name '*.rs' \| xargs wc -l` |
| GUI share of codebase | **~25%** | derived |
| Pure layout/placement machinery | **~8,300** LOC, 11 files | §3 |
| `editors/` — UI for configuring the UI | **29** files, ~32 editor surfaces | `ls src/frontend/gui/app/editors` |
| Cena `cena-ui` | **907** LOC | `find crates/cena-ui -name '*.rs' \| xargs wc -l` |
| Cena `cena-web` | **1,246** LOC | same |

The single largest editor, `editors/controller.rs` (4,312 LOC), is behind
`#[cfg(feature = "gamepad")]`.

**A quarter of the client is its GUI, and a quarter of *that* is placing rectangles and
configuring itself.** That ratio, not any individual file, is the thing to not repeat.

---

## 2. What the rigid window model cost

### 2a. One window kind, spelled four times

| Enum | Variants | Where |
|---|---|---|
| `WindowDef` | 37 | `src/config/window_def.rs:17` |
| `WidgetType` | 40 | `src/data/window.rs:23` |
| `WindowContent` | 45 | `src/data/window.rs:182` |
| `TabKey` | 25 | `src/frontend/gui/tab_id.rs:19` |

Four hand-written enums, each with its own fallback arm, kept in sync by hand for every
new window kind. `WidgetType::from_str` **silently defaults unknown types to `Text`**
(`window.rs:88`).

Identity is then decided by **string-sniffing**: `WidgetType::Hand` becomes
`LeftHand`/`RightHand`/`SpellHand` by `name.to_ascii_lowercase().contains("left")`
(`tabs.rs:107-116`); `Progress` becomes `Vitals` iff
`name.eq_ignore_ascii_case("vitals")` (`tabs.rs:85-91`).

### 2b. So `TabKey` is not stable — and that one fact costs 1,130 lines

The author's own comment, `tab_drag.rs:12-22`:

> *"shell keys are not stable across a move: a text window's `TabKey` is the singleton
> `TextMain` iff it subscribes to `main`, so tearing the `main` tab out of `story`
> re-keys `story` to `TextByName{story}` and hands `TextMain` to whichever window now
> carries `main`."*

Because identity is derived from *content*, **moving a tab silently renames windows**.
Every move must therefore atomically migrate **ten `HashMap<TabKey,…>` placement maps**
(`rekey_placement_state`, `tab_drag.rs:546-561`), four scalar `Option<TabKey>` fields,
and the detached-viewport map — driven by diffing the entire name→key mapping, because
the move may rename a window *"possibly one no receipt mentions"* (`:19-20`).

**With stable opaque ids, this file does not exist.** A move is one field write.

### 2c. There is no layout tree. There are five parallel placement states.

| State | What it is | Where |
|---|---|---|
| `main_window_rects` | flat free-floating pixel rects | `dock.rs:40-60` |
| `tab_zones` | five fixed named regions | `zones.rs:11-18` |
| `window_anchors` | a DAG of per-edge constraints, Kahn-solved per frame | `window_manager.rs:46-110` |
| `tab_groups` | hand-rolled flex stacks (weights, merged, end_anchored) | `persistence.rs:294-321` |
| `detached_tabs` | OS viewports that **shadow** the above, not replace it | `detached.rs:8-10` |

Plus `sidebar_gap_above` + `migrated_sidebar_zones` as a sixth, legacy, mid-migration
state, and `pending_zones` keyed by *window name* rather than `TabKey` — which exists
precisely because TabKeys don't exist for windows that aren't live (`dock.rs:31-36`).

Every operation must consider all six. `restore_layout_state` (`dock.rs:134-361`, ~230
lines) reconciles them, and includes a **heuristic over four evidence sources** just to
answer *"was this window ever placed?"* — needed only because zone assignment is
mandatory-with-a-default, so the model cannot represent "unplaced."

> The five zones are not a tree either. `default_zone_for_tab_key` and
> `default_zone_for_widget_type` both `_ =>` ignore their argument and return `Center`
> (`zones.rs:510-528`) — an owner decision dated 2026-08-06. The zone enum is five
> constants where a tree wanted nodes.

### 2d. Why `zones.rs` is 2,889 lines

A "zone" is one of five fixed regions — a trivial concept. The file is large because it
is **the whole main render loop plus a hand-rolled window manager**: a press/drag
engagement state machine (`:120-290`, including `should_claim_latch` with nine boolean
arguments), `render_zone_surface` at ~860 lines, z-order management, drop overlays, and
~500 lines of tests.

That state machine exists because **egui gives no window-manager semantics**: the shell
must arbitrate which of N overlapping `Area`s owns a press, and must fight egui's
remembered `desired_size`. It is **entirely absent** in a tiled/tree layout where
hit-testing is unambiguous.

### 2e. Two independent layout engines over one window list

Core cell-space (`TuiPlacement` u16 rows/cols + a conservation-guaranteeing
`distribute_1d` solver, `core/app_core/layout.rs`, 2,455 LOC) and GUI point-space
(`main_window_rects` + anchor solver + canonical-canvas rescale), reconciled **only by
`String` window name**. Two files, two formats, two schemas, two migration strategies
(idempotent healers vs. `schema_version`), and a `merge_loaded` union that exists solely
because either frontend may write the shared catalog.

---

## 3. The bones are good — and this is the important half

The inventory's clearest result is that **VellumFE's data flow is not the problem.**

### 3a. Nothing re-parses text

`WindowContent`'s 45 variants are typed, and **most carry no data at all** — they are
pure view selectors over `GameState` (`Targets` → `room_creatures`, `Quests` →
`objectives`, `Containers` → `managed_inventory`). Only the text family carries buffers.
The GUI never re-tokenizes markup. This is `12` §3a's *one parser, N classifiers*,
already honoured.

### 3b. Renderers are stateless functions

Every pane is rendered by an associated fn taking `&AppCore` (immutable) + `&mut
egui::Ui`. Per-widget view state (scroll, selection, bestiary page, container focus)
lives in **egui's own temp/persisted data**, not in app structs (`widgets.rs:3-4`).

> *"This is why detached viewports and the TUI/GUI/web parity work at all."*

### 3c. One outbound interaction channel

Each renderer returns `Option<GuiLinkClick>`. A click anywhere — creature card, target
row, map room, hotbar button, compass arrow — becomes a `LinkData {exist_id, noun, text,
coord}`. Synthetic commands use a `_direct_` sentinel exist id (`vitals.rs:102`).
**One channel, one vocabulary.**

### 3d. Highlights apply once, in core, before any frontend

`highlight_engine.rs:1-7`: *"applies highlights ONCE during message processing, **before
text reaches any frontend. Text arrives at widgets pre-colored.**"* It is **not** a
render-time pass and it imports no frontend. This is the single most important
architectural fact about that system, and it matches Cena's parse-first rule exactly.

### 3e. Condition resolution is centralized

`core::conditions::resolve_status` / `resolve_hand`, `core::hotbar::resolve_bar`, and
doll variant resolution are consumed **identically** by indicators, dashboards, hands,
hotbars, dolls and creature-card overlays.

> *"That one abstraction carries an enormous amount of the client's expressiveness."*

And it is built from **dropdown-composed structured conditions, not an expression
language** (`hotbars.rs:1-2`) — which is the same answer `CLAUDE.md` gives for
automation. Consistent with the no-DSL-for-now posture, and evidence that it works.

### 3f. Drawing modules never decide

Alerts, creature placement, target filtering and hotbar state all resolve in core, *"so
multiple renderers can't double-fire or disagree."*

### The real coupling, stated precisely

There is **no GUI↔parser coupling**. The coupling is **layout↔widget-type**: geometry
code asking a 40-variant enum what a window *is*, instead of asking the view for a size
policy.

- `zones.rs` has six `ui_state.windows.get(...)` calls inside render/solve loops needed
  only to read `widget_type`.
- `compact_height_cap` (`zones.rs:571-662`) reaches from geometry code into
  `app_core.layout.windows`, matches `WindowDef::Dashboard`, reads `data.cell_count()`
  and parses `DashboardLayout::from_str` — **to compute a height cap**.
- `min_window_width_for` is `if Compass { 48.0 } else { 120.0 }`
  (`window_manager.rs:526-532`), and `dock.rs:373-380` documents that
  `MIN_SNAPSHOT_WIDTH` *"must stay at or below the smallest of those (the compass' 48pt),
  or every snapshot read silently re-inflates a narrower user-set width."*

The *behaviour* is right — a one-row countdown should not stretch. Only its location is
wrong. Expressed as a per-view declared `SizePolicy`, **four hardcoded tables delete**
(two in `zones.rs`, one in `window_manager.rs`, one in `core/app_core/layout.rs:477`).

---

## 4. The under-designed thing, named

From the editor census, and it is the truest sentence in this document:

> **The one thing that is genuinely under-designed: *where* a setting lives.** Settings
> window vs. context menu vs. dedicated editor vs. layout file vs. `appearance.toml`
> vs. sidecar is currently decided ad hoc. **Decide that taxonomy first; a large share
> of the 29 editors exist because the answer was re-litigated per feature.**

The evidence is unambiguous churn, not inference:

- `settings.rs:13-16` records that widget-shaped settings *"moved to the Window Editor
  (editors/windows.rs)"* — **`editors/windows.rs` does not exist on disk.** The doc
  comment points at a module that was removed.
- `menus.rs:180` labels a block *"Window/widget config commands (**the former Window
  Editor fields**…)"* — a whole editor dissolved into the context menu.
- `menus.rs:886` calls `open_settings_editor_at("Targets")` **from** the context menu —
  the round trip made visible.

The same knobs were relocated at least twice and the documentation is now stale.

### 4a. Three build-failing parity mechanisms, each with an exemption list

| Guard | Where |
|---|---|
| `HIGHLIGHT_FIELDS` + `HlFrontend` + per-editor coverage manifests | `config/highlights.rs:252-400` |
| `keybind_action_table_is_the_single_source_of_truth` + `EXEMPT_ACTIONS` | `config/keybinds.rs:391-398, 874-890` |
| registry `Config`-shape walker + `EXEMPT_PREFIXES` | `config/registry.rs:8-19` |

`config/highlights.rs:257-261` is candid about why: unlike `registry.rs`, which *drives*
generic rendering, the highlight catalog *"can't render"* — so the guard is a test
instead of a generator.

> Three separate escape-hatch registries is a pattern that appears only when the generic
> mechanism could not be extended to the structured cases. **Make structured config
> renderable from a schema and all three guards, plus their exemption lists, disappear.**

These guards are good engineering — they are a **rule that is enforced** in `05` §0's
sense. The point is not that they are wrong; it is that they are compensating for a
missing generator.

### 4b. "Partial editor over a shared document," implemented by hand four times

`tabs.rs:20-23`, `indicators.rs:5-8`, `dashboard.rs:5-7`, and `skin.rs`'s
comment-preserving save all independently solve *"fields this editor doesn't surface must
survive a save."* That is one missing abstraction — edit-in-place over a document with
preserved unknowns — not four unrelated bugs. `pool.rs:985-1131` uses `toml_edit` for
the same reason.

---

## 5. What to keep — earned knowledge, not accident

These are conclusions Vellum paid for. A rebuild should inherit the **reasoning**, and in
several cases the code.

**Trust and safety**
- **Alertpack content-hash trust gate** (`config/alertpacks.rs:8-27`). Shared rule packs
  inherit `replace` (rewrites text in place) and `redirect` (reroutes streams) — powers
  that can *misrepresent what the game said*. Those stay **inert until the user approves
  that pack's exact contents by content hash**; any edit re-arms the gate automatically.
  *"The mechanism is content-based, not install-channel-based, so there is no trusted
  path to smuggle a change through."* Colour/sound/alert rules load immediately, because
  gating them would make packs useless on arrival and *"train users to click approve
  reflexively."* The user's own rules are never gated: *"the gate is about authorship,
  not about the capability itself."* And the editor's purpose: **"A gate the user cannot
  read is not a gate"** — every sensitive rule rendered in plain language before asking.
- **Alert scoping by room/area/realm** — *"A Reim encounter pack matching its patterns
  while you stand in a Wehnimer's bank produces false positives, and false positives are
  what make people switch an alert system off."*

**Text rendering (generic chrome, worth porting nearly verbatim)**
- **Buffer-anchored selection** — endpoints address `(line uid, char index)` in the
  *stream*, not on screen, so selection survives scrolling, trims and stick-to-bottom
  shifts, and Ctrl+C can copy off-screen lines (`widgets.rs:445-460`).
- **Same-frame scroll anchoring** (`text.rs:1490-1522`) — when the ring buffer drops
  lines off the front, the persisted pixel offset is nudged by the dropped rows' strides
  *before* the `ScrollArea` reads it, so an up-scrolled reader keeps their exact place
  with zero flicker.
- **Split scrollback** — scrolled up, the pane splits into a frozen history pane and a
  live tail pane with a draggable divider. A better answer to the classic scroll-lock
  problem than scroll-lock.
- **Link runs** — consecutive non-link segments accumulate into one `LayoutJob`; links
  flush and render as their own widget with *zero* inter-widget spacing, so punctuation
  does not gain spaces (`text.rs:425-470`).
- **Row-height cache** keyed by wrap width + font + buffer generation, with an
  incremental append path.

**Geometry and assets**
- **Fraction-based anchors survive resize and zoom** (`doll_calibration.rs:4-6`).
- **The 880×470 virtual stage** (`creature_field.rs:21`) — the solver plans on a fixed
  stage mapped uniformly into the widget rect, *"which is what keeps every solver
  guarantee intact under any window size."* One transform for the painted scene and the
  objects on it, so *"resizing the window can no longer make them drift apart."*
- **Sidecar metadata travels with the artwork**, and is embedded in the PNG so *"the
  file travels calibrated"* (`creature_calibration.rs:9-10`).
- **Repaint only when animating** — idle rooms request no frames
  (`creature_field.rs:274-279`).

**Input**
- **Refuse a bind with an explanation rather than letting it silently no-op**
  (`keybinds.rs:24-83`). `reserved_combo_conflict` refuses ctrl/cmd+C/X/V because winit
  synthesizes clipboard events *beneath* the key layer, so binding something else makes
  one key do two things; `keyboard_dead_action_reason` refuses controller-only actions on
  keyboard keys and points at the Controller editor.

**Process**
- **Live previews driven through the real pure transform** (`sorter.rs:3-5`) — *"the
  transform is a pure function, so the preview is always truthful."*
- **`write_atomic` everywhere**: tmp → `.bak` copy → rename (`config/paths.rs:23-39`).
- **Forward-compatible config**: `parse_tolerant` round-trips window entries whose
  `widget_type` this build cannot deserialize, and re-appends them on save
  (`config/layout.rs:400-449`). A layout from another branch is not destroyed.

---

## 6. What Cena has already made unnecessary — VERIFIED 2026-09-22

The redesign spec's wire-verification section lists tags VellumFE's parser did not
handle. **Cena handles every one of them, as typed frames.**

| Vellum gap (spec, 11.4 GB census) | Cena |
|---|---|
| `exposeDialog` ×4,265 — *"the missing U5 bank verb"* | `Frame::Expose { kind, id }` (`parser/thin.rs:87`) |
| `exposeStream` ×12, `exposeContainer` ×2,714 | same variant |
| `deleteContainer` ×7,559 | `Frame::DeleteContainer { id }` (`thin.rs:91`) |
| `closeDialog` | `Frame::CloseDialog { id }` (`frame.rs:224`) |
| `streamBox`, `dynaStream`, `clearDynaStream` | present in the 126-tag table |
| dropped `streamWindow` placement attributes | **`attrs: Attrs` — the whole bag** (`frame.rs:269`) |
| dropped `openDialog` location vocabulary | **`attrs: Attrs`** on `DialogOpen` *and* `DialogPanelOpen` (`frame.rs:343-353`) |

**`PlacementHint` — the concept Vellum had to invent and retrofit — needs no parser work
in Cena. The data is already arriving.** Cena's own measured comment (`frame.rs:265-268`)
records why the attrs bag was right:

> 1,176,686 `<streamWindow>` carrying **15 distinct attribute names**, of which
> [Vellum kept] only three. `location` (1,169,402) and `target` (1,163,813) are on ~99%
> of them, and `ifClosed` (602,513), `resident` (605,219) and `styleIfClosed` (1,353)
> are all live.

`Frame::DialogWidgets` makes the same move deliberately, collapsing Vellum's seven
variants into one keyed by `kind`: *"spinboxes and skins are toolkit concepts; the wire
fact is only 'the game sent an `<upDownEditBox>` with these attributes'."* That **is**
the identity/presentation separation the redesign was fighting for, already applied at
the parser boundary.

### 6a. Two decodes to get right when a dialog renderer is built

Not gaps today — Cena has no dialog renderer, and these values ride untouched in `attrs`.
Recorded here so they cannot bite the way they bit Vellum.

1. **`justify` is a bitfield, not an enum.** Corpus census: `2` ×10.5M, `0` ×441K,
   `4` ×393K, `5` ×6, `6` never. Correct decoding: **low 2 bits = alignment
   (0=left, 1=center, 2=right), bit 4 = a flag** — so `align = justify & 3`. Vellum's
   map had 4/5/6 right but let **the majority value `2` (right) fall through to
   default-left**, visibly: *"Level 106"* rendered left instead of centered. Fixed in
   Vellum 2026-08-05.
2. **Percentage coordinates are real.** `minivitals` sends `left='25%' width='25%'`.
   Vellum parsed coords as `i32`/`u16`, so `%` failed to parse and controls **silently
   lost position** — harmless there only because `minivitals` routed to a dedicated view.
   A `Px(i32) | Pct(f32)` coordinate type must land **before** any per-window view
   override ships.
3. Deferred in Vellum for want of captured XML: **vertical align**. `bugDialogBox`
   anchors buttons `align='se'/'sw' top='-5'` — bottom-relative — and Vellum's anchor
   resolver ignores the vertical compass component (s/c treated as n).

### 6b. Also recorded by the spec, and worth keeping

- **`exposeWindow` does not exist on the wire.** The real verbs are
  `exposeDialog`/`exposeStream`.
- **Stream text carries no alignment markup at all.** An exhaustive census of 11.4 GB
  found only `roomName`, `roomDesc` and `""` as style ids. Window-level text alignment
  is a purely client-side choice with no wire counterpart — nothing to honour.
- **Claim exact ids only.** The dialog id inventory includes per-entity ids like
  `injuries-10154507` (*"Zoleta's Injuries"* — another player's doll). Anything else
  must fall through to a generic renderer.
- **Clamp game hints.** The game itself sends viewport-busting sizes —
  `espMasterDialog height='2100'`.
- **Wrayth settings exports exist on this machine** at
  `E:\Saved Files From Latest Reinstall Yay\Gemstone\SIMU\Wrayth\` (`Nisugi3.xml`,
  `NewLayoutWrayth.xml`, `Mnstr.xml`, `YepCock.xml`): full `<settings client=>` blobs
  with 61 `<w>` entries plus highlights, presets and palette. **Test fixtures**, and the
  source for a future "import Wrayth settings" feature.
- **Skin names on the wire are pure client-side lookups** — no pixels or colours ever
  cross the network (verified against `storm.skn`, a 9.3 MB OLE compound document).
  Lookup must be **case-insensitive**: the wire sends `healthBar`, the skin table says
  `HealthBar`. Simutronics' art is never extracted or shipped; Hydra supplies its own
  keyed by the same names.

---

## 7. Proposal — the frame the GUI should sit on

**Everything in this section is a proposal.** It is the design direction the inventory
points at, stated so it can be argued with.

### 7a. The principle Cena already has, generalized

`cena-ui`'s `Closed` enum (`crates/cena-ui/src/view.rs`) ships the wire's *declaration*
(`ifClosed`/`styleIfClosed`) rather than a resolved destination, and says why: the hub
encodes **one** message and broadcasts it to every viewer, so a per-viewer answer would
mean re-encoding per client.

> **The server states facts. The viewer decides where they land.**

That is the right rule for free-form windows, and it is already implemented for streams.
The proposal is that it generalizes: **the session owns facts; a frontend owns
placement.**

### 7b. The tension to resolve first

`SessionView` (`crates/cena-ui/src/view.rs`) is a **fixed struct with eight named
fields**. It is a far smaller vocabulary than Wrayth's, and its discipline is excellent —
`Option` means *unobserved* rather than zero, `HandView::{Unknown,Empty,Holding}` are
three distinct answers, `project()` never reads a clock.

But it is still a **fixed, server-decided vocabulary**. If a pane is to be *any view over
the typed model*, then `project()` returning one hardcoded shape is what will fight the
GUI: every new pane widens the struct and revs the wire contract.

**This is the first thing to decide, and it is a real fork:**

| Option | Cost |
|---|---|
| Keep `SessionView` fixed; widen it per pane | Simple, typed, versioned. But the wire contract churns with the UI, and a native GUI pays a projection tax for data it could read directly. |
| A query/subscription vocabulary over `GameState` | Panes become free. But it is a second API surface, and `05` §−1's rule of three says do not build it for one frontend. |
| **Split by transport** — native reads `GameState` directly; `SessionView` stays the *wire* DTO for remote viewers | Matches the crate graph (`cena-gui` would sit beside `cena-web`, depending on `cena-ui` + `cena-session`). Native pays no projection tax; the wire stays narrow and versioned. |

The third is the recommendation, and it needs the author's call. It is consistent with
`cena-gui-is-primary`'s note that the architecture stays frontend-agnostic while the GUI
leads, and with the measured fact that `cena-ui` depends on `cena-model` alone.

Third option is the one - Nisugi

### 7c. Windows: what replaces the four enums

Three things Vellum conflated, kept separate:

| Concern | What it is | Vellum's mistake |
|---|---|---|
| **Identity** | which game feed — a `WindowBinding`-shaped `Dialog \| Stream \| Container`, or a local id | derived from the window's *name* and *content* |
| **Presentation** | which renderer — a `ViewKind` | fused into identity as `widget_type` |
| **Placement** | where it sits | a fourth enum plus five parallel stores |

Plus the rules the spec already settled:

- **Two buckets, classified by who authors the *structure*:** game-declared (the XML
  declares content *and* structure) vs. local catalog (everything we invent, whether or
  not it reads game state).
- **The claiming rule:** when the game declares a binding a local view presents better,
  one `DEDICATED_VIEWS` table claims it. *"The binding is game-declared, the presentation
  is ours."* **One** table, so the must-agree invariant is true by construction rather
  than enforced across three functions.
- **Stable opaque ids.** Not derived from name, content or type. This is what deletes
  `tab_drag.rs`.

### 7d. Layout — DECIDED 2026-09-22

The author settled the shape:

> *"Vellum's gui has a main area, then it has a 'drawer' on each side, the top and
> bottom. The main area has a grid and snap system, so do the drawers. That's all going
> to be the same, maybe a little less buggy!! One big change is a new custom window.
> It's a window with a grid that allows you to place and arrange widgets inside the
> window. This solves the problem that vellum's window grouping created."*

And the model underneath, asked directly: **free rects, with the grid as a snap target**,
grid pitch adjustable as in Vellum. Not real cells. Widgets inside a container window are
**bare — the container owns the chrome.**

So: keep the main area + four drawers, keep free placement with snapping, and **add a
container window whose interior is the same grid/snap system one level down.**

#### Why the container window is the right target

Vellum's `TabGroup` (`persistence.rs:294-321`) is
`{ members, horizontal, merged, end_anchored, weights }` — a hand-rolled mini flex
layout: a stack of slots, where `merged` puts two members in one slot and `weights`
splits leftover space after fixed bars take their natural height.

**It is the only nested layout in Vellum, and it is bolted onto a flat rect store.** That
is the structural reason it is buggy: it is a second layout system with no relationship
to the first, so every operation special-cases it — `set_tab_zone` special-cases groups
(`zones.rs:682-692`), `rekey_placement_state` migrates them, `restore_layout_state`
reconciles them.

A container window is not a variant of grouping. **It is the thing grouping was
approximating badly** — the same engine one level down, instead of a parallel mechanism.
That deletes a whole row from §2c's table of five placement states, and it deletes it by
*unification* rather than by removal, which is why it does not cost the user anything.

#### What this decision keeps, and what it therefore owes

Free placement keeps the things a tree would have removed, and they must be budgeted
rather than discovered:

| Kept | Cost |
|---|---|
| overlapping windows | press arbitration — Vellum's engagement latch (`zones.rs:120-290`, ~170 lines) exists because egui gives no window-manager semantics and must be told which of N overlapping `Area`s owns a press |
| free rects | z-order management (`zones.rs:870-978`) |
| snap permanence | the anchor system — `EdgeRef`/`AxisAnchoring`, Kahn-solved per frame (`window_manager.rs`); without it a drawer-splitter drag moves the pane edge but not the windows docked to it |
| "unplaced" not representable | §2c's four-evidence-source *"was this ever placed?"* heuristic — **fix this directly by making placement an `Option`, not by inference** |

`snap.rs` is the thing to port, including its field-report reasoning: sibling *centers*
are deliberately excluded as candidates because *"they sit a few px from real edge targets
and make the engaged line flip while dragging (beta.21 field report)"* (`snap.rs:31-34`).

#### 7d.1 The adjustable grid — a Saga complaint, answered by a feature Vellum already has

The author: *"one complaint people have had from vellum is the snap seems to be 2 spots
instead of 1, so adjustable grid fixes that?"* — then, correcting a misreading of it:
**"The grid complaint was about saga not vellum. Saga's grid isn't adjustible."**

So the complaint is feedback *received from* Vellum's users *about Saga*, whose grid pitch
is fixed. **The author's original answer is correct: an adjustable grid is the fix, and
Vellum already implements it.** `SnapParams.grid` is resolved per drag frame from
`settings.snap_grid` (`snap.rs:554`), with `snap_move_sizes_to_grid` beside it. Hydra
inherits the feature; there is nothing to repair.

> **A CORRECTION WORTH RECORDING, 2026-09-22.** This section first read the complaint as
> being *about Vellum* and argued the author's proposed fix was wrong. The preposition
> carried the meaning — *"from vellum"* meant **from Vellum's users**, not *about Vellum*
> — and a whole diagnosis was built on the wrong subject. The rule this breaks is `05`
> §−2's: the attribution of a report is a fact like any other, and it was assumed rather
> than checked. One clarifying question would have cost nothing.

#### 7d.2 An unattributed Vellum snap defect, found while investigating the above

**This is a real finding with no reported symptom attached to it.** It surfaced while
diagnosing 7d.1's complaint under the wrong assumption, and it survives the correction —
but it is now UNVERIFIED against user reports, and must not be described as explaining
one. Recorded because it is cheap to keep and expensive to re-derive.

**MEASURED cause.** During a `Translate` gesture three moving positions compete — `lo`,
`hi`, and the center (`snap.rs:168-173`) — and **each sibling contributes two
candidates**, its min edge and its max edge (`snap.rs:278-296`). So dragging window B
toward window A's border puts two candidates within a few points of each other:

- B's `lo` → A's `hi`: **abut** (B sits flush against A)
- B's `lo` → A's `lo`: **align** (B's left edge lines up with A's left edge)

Both are `SnapGuideKind::Sibling`, so `kind_priority` cannot separate them — it only ranks
`Bound > Sibling > Center > Grid` (`snap.rs:58-65`). The winner is decided by raw nearest
distance with a 0.001 epsilon (`snap.rs:200-207`), so **which of the two lands is decided
by a sub-pixel of pointer travel.**

That the code permits this is VERIFIED from the source above. That any user has *noticed*
it is UNVERIFIED — no report is attached to it, and the one report that prompted the
investigation turned out to be about a different client. It may well be invisible in
practice: both landing positions are plausible, and a user who wanted abutment and got
alignment may simply drag again without ever calling it a bug.

**The shape is familiar, which is the argument for keeping the note.** The beta.21 field
report — sibling centers making *"the engaged line flip while dragging"* (`snap.rs:31-34`)
— is the same candidate-tie class with a different pair, and it *was* noticed, and it was
fixed by **exclusion rather than a new setting**. A second instance of a class that
produced one real field report is worth a test before it is worth a fix.

**If it is ever confirmed**, the candidate fixes are:

1. **Hysteresis on the engaged target** — once a guide engages it keeps winning until the
   pointer leaves by `radius + margin`. Cures flip-on-a-pixel for the whole class.
2. **Split the priority** — abut and align are different intents; rank abut above align
   so near-ties resolve identically every time.
3. *(optional)* **Suppress near-coincident candidates** within ~2pt, keeping the
   higher-priority one. Generalizes the beta.21 fix rather than re-applying it per pair.

**Do not build these speculatively** (`05` §−1). The right first move is a test that pins
current behaviour at a near-tie, so the question becomes answerable from a report instead
of from a reading.

#### 7d.3 Container chrome must follow position, not a side table

Vellum already has a per-window chrome toggle, and it is in the wrong place:
`no_title_tabs` is a `HashSet<TabKey>` living in the **dock snapshot** (`dock.rs:88`,
`zones.rs:845-849`) — chrome as a property of *placement state*.

For a container window that inverts: **"bare" should follow from the widget being inside a
container**, not from membership in a set. Otherwise dragging a widget in means
remembering to add it to `no_title_tabs` and dragging it out means remembering to remove
it — a fifth thing to keep in sync on every move, which is exactly the §2b failure that
costs Vellum 1,130 lines.

Chrome is a function of the widget's parent. One less map to migrate.

### 7d.4 What a GUI owns — the full list

The author asked: *"What else does a gui own? window layout is one and I think we have a
good idea there. opacity adjustments for the windows and drawers. windows need to be able
to detach. What else am I missing?"*

The useful cut is not "what features exist" but **which state is frontend-local and which
belongs to core**, because that is the split Vellum got right in most places and wrong in
a few, and the wrong ones are where the churn came from (§4).

**The test, stated once:** state is GUI-owned if a *headless* session would not have it
and would not miss it. A second viewer attached to the same session should be able to
disagree about it. If two viewers must agree, it belongs to core.

> **The rule is already proven in the codebase.** Sound is **core**-owned: highlights
> fire `SoundTrigger`s from `highlight_engine`, and `grep` finds `sound::` referenced
> from `core/app_core/state.rs` and **not from the GUI at all**. A headless session still
> plays alerts. Contrast `zoom_factor`, which no headless run has and no second viewer
> should inherit.

#### The list

**1. Layout** — §7d. Rects, zones, drawers, containers, detach.

**2. Window chrome, per window.** Vellum's `TabSettings` (`persistence.rs:64-136`) is the
measured list and it is longer than expected: `font_primary`, `font_secondary`,
`text_size`, `accent_color`, `corner_radius`, `skin_frame`, `frame_scale`,
`background_image`, `title_bar_height`, `title_bar_align`, `wrap_text`, `copy_behavior`,
`map_zoom`, `custom_title`. **Fourteen per-window knobs**, almost all `Option<T>` falling
back to a global.

**3. Global appearance.** `GuiUiSettings` (`persistence.rs:329-460+`): `zoom_factor`,
`text_size`, `title_font_size`, `title_bar_height`, `title_bar_align`,
`effects_bar_height`, **`density`** (spacing scale — *"lower = denser (Wrayth-like)"*),
`bar_corner_radius`, `window_corner_radius`, `auto_contrast_bar_text`, `vitals` layout,
`active_theme`, `doll_image`, `doll_grayscale`, `status_icons`, `compass_set`,
`default_frame`, `default_background`, `zone_separators`, plus the snap block
(`snap_enabled`, `snap_radius`, `snap_to_siblings`, `snap_to_bounds`, `snap_to_centers`,
`snap_grid`, `snap_move_sizes_to_grid`).

**4. Opacity, and the drawer behaviour it belongs to.** See §7d.5 — two of the three
behaviours the author named are already built in Vellum; the third is new and is the
interesting one.

**5. Detach.** Real OS windows via egui multi-viewport,
`ctx.show_viewport_immediate` per detached tab (`detached.rs:34`). The trap is recorded at
`detached.rs:8-10`: `tab_zones` and `main_window_rects` **keep their entries while a tab
is detached**, so reattach is just removing the key from `detached_tabs`. That makes
detach a *fifth parallel placement state* (§2c) rather than a move. In a tree it is "this
subtree renders into viewport N"; with free rects it stays a shadow, and the shadowing
must be deliberate.

**6. Fonts.** `FontRef::{SystemDefault, Custom(path)}` loaded at startup, per-window
primary/secondary overrides, plus font enumeration and validation. Genuinely GUI-local —
a web viewer resolves fonts differently and headless has none.

**7. Input — the largest thing not on the author's list.** Keyboard dispatch is a
**layered** resolver (`config/keybinds.rs:197-201`): app shortcuts first, then menu
keybinds while a menu has focus, then game keybinds. Around it: macro dispatch,
Windows numpad capture (which is why VellumFE runs a **fork** of egui —
`Nisugi/egui` branch `numpad-support`), tab-completion interception, and
`reserved_combo_conflict` refusing ctrl/cmd+C/X/V because winit synthesizes clipboard
events *beneath* the key layer.
**The binding table itself is core** (`ACTIONS`, the single source of truth, with a
build-failing parity test). **The dispatch is GUI.** That split is right and should be
kept.

**8. Selection, clipboard, search.** Buffer-anchored selection (§5), `copy_behavior` per
window, Ctrl+F with a match cursor. Purely viewer-local — two viewers of one session
should be able to select different text.

**9. Scroll and scrollback position.** Per window, including the split-scrollback divider
fraction and the stick-to-bottom state. Note `text.rs:1477-1489`: scroll state is keyed by
renderer id, so moving content between a tab and a window requires *adopting* the previous
id's state. **With stable ids (§7c) that adoption step disappears.**

**10. Theme and skin.** Theme→`Visuals` mapping, the skin registry, frame/background
resolution, gradient borders. Note the recorded decision to re-make deliberately:
live-manifest skins were **removed** in favour of inert presets over an appearance store
(`skin.rs:7-9`) — *"worth deciding up front in the rebuild rather than refactoring into
it."*

**11. Images and textures.** The image store, texture cache, room-art floats, emoji
overlay painting. A GPU-side concern with no headless counterpart.

**12. Frame pacing.** When to request a repaint. `creature_field.rs:274-279` is the model:
50 ms **only while something is animated**, so idle rooms request no frames. On a
3–25-session client this is not a nicety.

**13. Window-manager semantics egui does not provide.** Press arbitration across
overlapping `Area`s, z-order, the engagement latch, resize grabs, drag overlays
(`zones.rs:120-290`, `:870-978`). This is the cost of free placement, itemised in §7d.

**14. OS integration.** Main viewport geometry restored at launch
(`MainViewportState`), detached viewport geometry, and the per-character layout file
location.

#### What a GUI does NOT own — the boundary that keeps this honest

These look like frontend concerns and are not. Vellum places all of them in core, and
that is why its TUI, GUI and web frontends agree:

| Concern | Owner | Evidence |
|---|---|---|
| Highlight matching and colouring | core | *"before text reaches any frontend. Text arrives at widgets pre-colored."* (`highlight_engine.rs:1-7`) |
| Sound and TTS | core | `sound::` is referenced from `core/app_core/state.rs`, **never from the GUI** |
| Condition resolution | core | `core::conditions::resolve_status`, consumed identically by six widget families (§3e) |
| Creature placement | core | `core/creature_cards/solver.rs`; the GUI only maps the stage |
| Target filtering | core | `is_valid_target` is *"canonical on Creature so the TUI/GUI/web lists stay in sync"* |
| Alert firing and de-duplication | core | *"one line mentioning a creature five times is still one warning"* |
| **Per-window widget options** | **core** | looked up from the shared `layout.toml` `WindowDef` **by name at render time**, ~12 sites — *"widget config is not GUI-local"* |

The last row is the subtle one and it cuts against the grain: *what a vitals window
displays* is core config, while *what it looks like* is GUI config. Vellum draws that line
and it is a good line — but §4's churn is what happens when nobody writes it down, which
is why §7f says decide the taxonomy first.

#### Three the author did not name, likely to be missed

- **Frame pacing (12)** — invisible until 25 sessions are open, then decisive.
- **Input dispatch (7)** — the largest omission, and the reason Vellum forked egui.
- **Multi-session shell** — not in Vellum's list at all, because Vellum is one session
  per process. **Who owns the session switcher, per-session indicators, and "which session
  is focused"?** That state is GUI-local by the test above, has no VellumFE precedent to
  port, and lands in **M5**, before the GUI is built. Flagged here so M5 does not
  accidentally decide it by default.

### 7d.5 Drawers: reserve-or-overlay, translucency, and click-through

The author (2026-09-22):

> *"drawers should have the option to clip or push the center area over, same as vellum,
> drawers should be able to go fully translucent, clicking in the drawer where there is
> no window and it is translucent should click through."*

**Two of the three already exist in Vellum and port as-is. The third is new, and it
inverts a rule Vellum deliberately established.**

#### Already built — port them

**Clip or push** is `ZoneDisplayMode` (`zones.rs:294-305`), per zone, persisted in
`ShellLayoutSnapshot` as `header_mode`/`footer_mode`/`left_sidebar_mode`/
`right_sidebar_mode`:

- **`Reserve`** — *"carves its slice out of the center pane (classic behavior: center
  windows displace/squeeze while the zone is open)"* → the author's **push**.
- **`Overlay`** — *"floats the zone above the center like a Wrayth-style drawer — the
  center layout never moves; the covered strip is occluded until the drawer closes"* →
  the author's **clip**.

`Center` is structurally `Reserve` and `set_zone_mode` on it is a no-op (`zones.rs:389-397`),
pinned by a test. Serde default is `Reserve`, keeping pre-mode layouts byte-compatible.

**Full translucency** is the four `*_opacity` fields (`zones.rs:326-340`), `1.0` solid to
`0.0` *"fully see-through so the drawer reads as a HUD over the center pane."* Explicitly
**ignored in `Reserve` mode**, *"where there is nothing behind the zone to reveal"* — a
correct restriction to keep.

One detail worth porting with it, because it is the kind of thing found only by looking
(`zones.rs:1435-1440`): the backdrop fill **and the zone's inner edge stroke** both scale
by the opacity, because *"fading only the fill would leave a hairline floating in space."*

#### Click-through — NEW, and it inverts `zone_for_pointer`

Vellum's rule is the opposite, deliberately. `zone_for_pointer` (`zones.rs:1095-1110`)
makes a drawer **win over Center** whenever the pointer is inside both:

> *"Overlay drawers overlap the center rect; a shell zone hit must win over Center so
> drops target what the pointer visually sits on."*

and it is pinned by `zone_for_pointer_prefers_drawer_over_center` (`zones.rs:2478-2499`).

The author's request does not contradict that rule so much as **narrow its domain**. The
precise condition is a conjunction of three facts, and all three must hold:

1. the zone is in **`Overlay`** mode (in `Reserve` there is nothing behind to click), and
2. its opacity is **below some threshold** — the author says *"it is translucent"*, and
3. the pointer is over **no window in that drawer** — bare backdrop only.

Then the hit falls through to whatever the center pane has under it.

**Why this is coherent rather than a special case:** at opacity 0 a `Reserve`-less drawer
is *visually absent*. Vellum's rule says "target what the pointer visually sits on" — and
when the backdrop is invisible, what the pointer visually sits on **is the center pane**.
Click-through is that same principle applied to a case Vellum never had, because Vellum's
drawers could not disappear. It generalizes the rule instead of excepting it.

#### The governing rule, in the author's words

> **"the goal is to be able to see what you're clicking. if the drawer is fully opaque
> without a window and you click then you don't know what you're clicking on."**
> — author, 2026-09-22

This is the rule, and the three sub-behaviours are consequences of it rather than
independent settings. **It is a statement about the user, not about a rendering state**,
which is why it decides the cases cleanly:

| Backdrop | Can the user see the target? | Click |
|---|---|---|
| transparent | yes — the center pane is visible through it | **passes through** |
| opaque | no — the drawer hides whatever is beneath | **stops at the drawer** (there is nothing meaningful to hit) |

> **A CORRECTION, 2026-09-22.** This section first proposed an explicit per-zone
> `click_through: bool`, reasoning that behaviour should not change as a side effect of
> dragging an opacity slider. **That was backwards.** Under the rule above, changing
> opacity is *exactly* when the behaviour should change, because opacity is what decides
> whether the user can see the target. A separate flag would permit "opaque drawer,
> clicks pass through" — the incoherent state the rule exists to forbid. **Derive it from
> opacity; do not add a setting.** The general lesson is `05` §−1's: a second knob that
> can contradict the first is not configurability, it is an unrepresentable-illegal state
> made representable.

Threshold, not exact zero, follows from the same rule: a drawer at 0.05 hides nothing, so
the user can see the target, so the click should pass. The cut-off is wherever the
backdrop stops obscuring — a tuning value, not a design question.

#### Settled with the author

- **"No window" means no widget** — *"the things we can move around that display ui
  elements."* So the drawer's bare backdrop is click-through territory; a widget's rect
  is not, whatever its frame art does. This is simpler than the frame-art question raised
  earlier and supersedes it: the test is widget occupancy, not pixel coverage.
- **Drops target the top-most container (the drawer)**, even where a click would pass
  through. Dropping is a deliberate placement act with a visible drop overlay; clicking is
  interaction with content. Same pointer position, different verb, different answer — so
  `zone_for_pointer`'s existing drawer-wins-over-Center rule (`zones.rs:1095-1110`) stays
  exactly as written for drops, and click-through is a **separate** resolution path.
  **Do not share one hit-test function between the two**; that is the most likely way to
  get this wrong.
- **Resolve at press time only.** A drag that *starts* in the center and passes under a
  translucent drawer must not be stolen mid-gesture. Vellum's engagement latch (§7d,
  `zones.rs:120-290`) already owns "who captured this gesture"; click-through is decided
  once, at press, and held for the gesture's duration.

**Consequence for §7d's cost table:** click-through is a fifth thing the press-arbitration
layer must know about, alongside z-order, the engagement latch and overlapping `Area`s. It
is a real cost of free placement, not a free addition — but it is a small one, and it
lands in code that already exists to arbitrate presses.

### 7e. Geometry policy belongs to the view

A per-view declared `SizePolicy { intrinsic_height, min_size, fixed_axes }` returned **by
the view**, replacing four hardcoded per-widget-type tables. Geometry code must never ask
what a window *is*.

### 7f. Settings: decide the taxonomy before writing an editor

Per §4, this is the thing to settle first. A rebuild should be able to answer, for any
knob, *where does this live and why* — before the first editor exists. And structured
config should be **renderable from a schema**, which retires three parity guards and
their exemption lists.

### 7g. Consolidations the census identified

| Consolidation | Payoff |
|---|---|
| Schema-driven editors for structured config | retires 3 parity mechanisms + exemption lists |
| One calibration framework parameterized by sidecar kind, with auto-derived first pass | ~2,500 LOC → under 900; editor becomes a *correction* tool |
| One `Trigger → Effects[]` model | removes the highlight/alert dual personality (§7h) |
| One wheel model with an input-source parameter | replaces three radial-menu implementations |
| One `ScopedTomlStore<T>` | replaces >1,000 LOC of repetitive per-config-kind persistence |

### 7h. Highlights are a `Trigger → Effects` system wearing the wrong name

`HighlightPattern` has **26 user-facing fields** and can, from one match: colour, bold,
squelch, suppress the prompt, redirect to another window (two modes), rewrite text
(optionally per-window), play a sound at a set volume, rumble a gamepad, set or clear a
timed status, and raise an overlay alert with banner, art, screen flash, countdown,
cancel list, condition gate and re-arm delay. A rule can fire **with no pattern at all**,
edge-triggered on a condition.

The reuse decision was deliberate and well argued (`config/highlights.rs:42-46`): alerts
inherit *"the same editors, the same per-stream filtering, the same fast-parse matching
path. **No new trigger infrastructure exists or should exist.**"* But the consequence is
visible: two places must special-case *"empty pattern must never compile to a regex,
because an empty regex matches every line"*, and the engine's hash function must
serialize an `AlertSpec` to a `String` because it is not `Hash`.

**The honest factoring is `Trigger (text match | condition) → Effects[]`, with
highlighting as one effect among many.** That removes the empty-pattern special case and
the serialize-to-hash workaround, and lets an editor stop pretending a condition alert is
a pattern. Note this lands at **M8**, not M10 — highlights are `12` §8's customization
milestone, which is a second reason the GUI benefits from coming after it.

Two pieces of that engine to keep regardless:
- **Two automata, not a flag** — `ascii_case_insensitive` is builder-wide in
  aho-corasick, so case-insensitivity is a *separate automaton* with its own
  pattern→rule map. Rules opt in individually.
- **The staleness guard** — every field the engine reads must be hashed, *"or an edit
  touching only that field never rebuilds the engine and keeps serving the stale value."*
  Pinned by `test_hash_tracks_every_consumed_field`.

---

## 8. The full feature census

Three agents read all 87 GUI files on 2026-09-22. The per-feature detail is preserved in
the session scratchpad; this is the checklist the author asked for, so nothing is
forgotten.

### 8a. GemStone-specific — a rebuild must re-encode these

Vitals dual percent-only / value-max feeds · Mind state · Encumbrance · Spirit ·
roundtime/casttime semantics and server-time-offset clock discipline · `<crtrStatus>`
flags including boss tiers (AscensionBoss/MiniBoss) and challenging · Lich
`valid_target?` semantics · injury doll part table (back and nervous system have **no**
front-silhouette spot) · 7-level injury/scar severity ladder · GS4 indicator id
vocabulary · managed-inventory relation vocabulary (`righthand`/`lefthand`/`worn`/
`atfeet`/`reserved`/`room`/`player`) · Saga weight accounting with the 0.1 lb floor and
deep-container rule · `openDialog` anchor-grid protocol, autosend, `%id%` spinbox
substitution, cross-id content pairs (`espMasterDialog`/`espMasterData`) ·
server-pushed quickbars · room components as separate streams · the dot-command
vocabulary · Lich WebUI component protocol · Saga `<objectives>` · Betrayer blood points ·
GS4 experience/ascension · **DragonRealms experience fields** · eAccess login and Lich
attach · the exist-id/noun link model · bundled bestiary codex · mapdb, room uids,
terrain/climate sense words, ghost rooms.

### 8b. Generic chrome — reusable or re-derivable

Text virtualization and row-height cache · buffer-anchored selection · split scrollback ·
search bar and match cursor · command history, tab completion, bound edit ops · quick
switcher · theme→`Visuals` mapping, font enumeration and validation · image store and
texture cache · colour and custom emoji overlay painting · gradient borders and skinned
chrome · tab-move toast · performance sparkline · alert overlay geometry · gamepad wheel
*mechanism* · detached viewports.

### 8c. Panes, by group

**Text/stream** — main text renderer (`text.rs`, 2,719 LOC, the heart of the client) ·
tabbed text windows with per-tab buffers keyed by *stable* tab id · room window ·
perception window.

**Character status** — vitals/minivitals · single progress bar · countdowns
(roundtime/casttime/custom) · active effects (spells/buffs/debuffs/cooldowns) · injury
doll · status indicators · dashboard (indicator grid) · GS4 experience · DR experience ·
encumbrance · betrayer · hands (left/right/spell) · missing-spells watchlist · quests.

**Room/map** — mini-map · shared map renderer · map explorer · compass · room items.

**Combat/creatures** — **creature field** (the flagship novel feature: cards on a
perspective floor, solver-placed, condition-driven overlays with Pulse/Flicker/Orbit
animations, per-part wound overlays, mounted pairs where clicking above the saddle line
picks the rider) · targets list · players in room · bestiary browser.

**Inventory** — containers window · single container · inventory/reserve/spells windows.

**Input** — command input · search bar (Ctrl+F) · quick switcher (Ctrl+K) · interact
mode · context menus · server dialogs · resident dialog panels · quickbar · hotkey bars ·
global keyboard input.

**Theming** — theme · skin runtime (`skin.rs`, 4,009 LOC) · skin ops and window chrome ·
per-window colours · colour emoji · custom emoji · image store.

**Other** — multi-account widget · launcher · Vellum Studio (art authoring) · gamepad
(3,023 LOC, *"by far the most elaborate controller support I'd expect in a MUD client"*).

### 8d. The 29 editors

controller (4,312, behind `#[cfg(feature = "gamepad")]`) · hotbars · colors · settings ·
creature_calibration · highlights · custom_windows · doll_sets · indicators ·
frame_calibration · room_images · launcher · keybinds · alertpacks · jinx · tabs ·
skill_trainer · known_windows · doll_calibration · creature_field · themes (hosts two
surfaces) · sorter · packs · scenery_calibration · dashboard · hand_icons · menu_keybinds ·
touch_wheel.

**The opaque names decoded** — *packs*: UI sharing via `.vellumpack`. *alertpacks*:
shareable highlight rules plus the trust gate. *jinx*: asset package manager, a native
egui client with no `jinx.lic` and no GTK — **this is how art reaches the image pool
pre-calibrated**. *sorter*: categorized container looks, a pure function over the line's
own `<a exist noun>` segments with no cache-freshness dependency, re-feeding generated
lines through the normal flush path so highlights apply to each. *dashboard*: a grid of
custom status indicators whose ids are toggled by highlight `set_status`/`clear_status` —
**a highlight rule lights a dashboard cell**. *skill_trainer*: "Skill Goals", which
scrapes and drives the play.net web skill manager with **no browser**, via the one-time
authenticated `LaunchURL` hmac that `GOALS` emits.

**The four calibrators are legitimate, not workarounds.** They author per-image geometry
that cannot be derived from pixels — where the feet are, height in *feet*, nine-slice
seams, where a wound dot belongs — feeding a 2,000-line constraint solver whose rules
include *permanence* (a unit's square is decided on arrival and never touched),
separation by **contact span** (*"the part of each card that actually rests on the floor;
a tail or thrown-forward paw overhangs it harmlessly"*), an occlusion cap, and a **fall
envelope** (*"an arrival must reserve room for the prone pose it isn't in yet"*). The
cost is fan-out, not existence: four near-identical click-to-place editors sharing only
`CalibrationOutcome`.

### 8e. Half-finished, and worth deciding up front

- **Creature-card art** is the incomplete part; sprite plumbing works (P4/P5). Studio
  exists specifically to produce the art.
- **macOS gamepad is untested** — *"may need a GameController-framework bridge"*
  (`gamepad.rs:12`).
- **Bestiary is bundled-data-only**; no live-observation merge.
- `render_missing_spells_content`, `render_betrayer_content`,
  `render_dr_experience_content` are flat label lists — the least-developed panes.
- **Containers has no auto-refresh by explicit owner decision** — not a gap, but a
  decision to re-confirm.
- **Legacy live-manifest skins were removed** in favour of inert presets over an
  appearance store (`skin.rs:7-9`). *"Worth deciding up front in the rebuild rather than
  refactoring into it."*

---

## 9. Open questions for the author

1. **§7b — the `SessionView` fork.** Native reads `GameState` directly while
   `SessionView` stays the wire DTO, or one projection for all frontends?  Yes
2. ~~**§7d — free placement or a layout tree?**~~ **DECIDED 2026-09-22:** free rects,
   grid as a snap target, main area + four drawers + a container window. See §7d.
   The adjustable grid (§7d.1) is settled too — it answers a **Saga** complaint and
   Vellum already implements it. §7d.2's candidate-tie defect is recorded but
   **unattributed**; it wants a pinning test, not a fix.
   > Note for M5: `cena-gui-is-primary` records the native-renderer rationale as **3–25
   > characters, so 3–25 live panes with long scrollback.** Free placement at that scale
   > keeps the engagement latch and z-order arbitration that a tree would have removed
   > (§7d's cost table). This is a budgeted cost, not an unexamined one — but multi-session
   > is still where it gets tested.
3. **§7f — the settings taxonomy.** Where does a knob live? This is the one the inventory
   says to decide *first*. Hmm, we shall discuss this when it's time taking inventory of all settings we have and need.
4. **Does the TUI carry forward?** `12` §8's M10 says "TUI/GUI". If it does not, §2e's
   second layout engine and one of the two persistence formats never need to exist. TUI will likely be rebuilt in the future, but it's not on the menu until like M15+
5. **Is the creature field in scope?** It is the flagship novel feature and the single
   largest body of game-specific geometric work, and its art is unfinished.  Creature field is in scope but it may evolve before we get to putting it in here, we are exploring ways to make it better.
