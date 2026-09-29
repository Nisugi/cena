# 53 — The map in the GUI: tune the layout, bring it in, style it, show it

**Status: APPROVED 2026-09-29, the author's answers in §6**, Claude's two recommendations
among them confirmed (*"I'm ok with your recommendations yep"*). Was PROPOSED the same day. `plan/49` Stage G (*"The map and minimap
... Each gets its own plan when it is next"*, `plan/49-gui-widgets.md:850-853`); the author,
2026-09-29: *"I think next we do the minimap? Let's build a solid plan to implement it.
There is the despana map demo that we implemented ... despana map demo is the styling,
hydra-mapper has our layout engine, which still isn't perfect. So the plan is tuning the
layout engine, implementing it, styling it, presenting it. Learning from vellum."*

Four stages, in the author's order (§4); the nine questions of §5, answered in §6.

**How it was measured.** Four read-only surveys, 2026-09-29: hydra-mapper, Despana's map,
VellumFE's map, and what Hydra has that a map stands on. Each item cites its source. Items
marked **VERIFIED** were checked again by hand for this plan; the rest are cited from the
surveys and were not re-read. Paths into the mapper are `hydra-mapper/...`
(`G:\dev\hydra-mapper`), into VellumFE `reference/VellumFE/...`.

---

## 1. What we have

### 1a. The map: `gs.map`

- **Hydra's own format, and the one file everything reads.** `HYDRAMAP` version 1, written
  and read by `crates/cena-map/src/binary.rs`. The copies at `C:\Users\shawn\hydra-mapper\gs.map`
  and `G:\dev\hydra-mapper\gs.map` are the same file (same MD5, both 2026-09-24 00:43).
- **It is the map Despana's atlas was built from. VERIFIED:** its SHA-256 begins
  `d5e8ac60659b09da`, and so does `source_map_sha256` in
  `crates/cena-web/atlas-data/corpus/manifest.json`
  (`sha256sum gs.map`; `grep source_map_sha256 manifest.json`).
- **33,956 rooms**, after 2,629 deleted from Lich's 36,585 (hydra-mapper commit `70b0d11`;
  `gs.overrides.areas.tsv` has that many rows). The mapper's README still says 36,838.
- **No positions.** A room carries ids, uids, text, tags, exits with their kind, crossing,
  cost and stated bearing (`dirto`), its Lich picture rectangle (`image`), and three
  extensions: `sheet` (the plate and the **area**), `placement` (an anchor uid and a cell
  offset, the editor's hand drags) and `exit_dirto` (`crates/cena-map/src/room.rs:47-156`,
  `crates/cena-map/src/binary/wire.rs:10-53`). Areas and regions are baked in as
  `meta:area:` and `meta:region:` by the mapper's `retag`. **Layout is derived every time**,
  never stored.
- **Loaded once per Hydra, shared by every character:** `CENA_MAP`, no default path ("a
  wrong map is worse than none"), decoded and hashed (`crates/cena/src/map_context.rs:21-47`,
  `crates/cena/src/play.rs:166`); 79 ms to decode in a release build (`plan/21-mapdb.md:925-926`).

### 1b. The layout engine: hydra-mapper

`G:\dev\hydra-mapper` is its own repository (`plan/26-mapper.md:10-25` moved it there), with
three crates: `cena-map-layout`, the pure engine (no I/O, no window); `cena-mapper`, an egui
editor titled "Hydra Mapper"; and `retag`, which bakes `curation/*.toml` into `gs.map`.
**VERIFIED:** the engine is 9,359 lines in 15 files
(`find hydra-mapper/crates/map-layout/src -name '*.rs' | xargs wc -l`).

The pipeline (`hydra-mapper/crates/map-layout/src/pipeline.rs:91-167`):

1. Drop non-places (gone rooms, urchin hideouts).
2. **Bearings**: each exit's direction from its kind and command; a scripted exit only when
   every move in it agrees (`direction.rs:198-279`). `go door`, `out` and the like have none:
   **29,798 of 84,867 exits, 35%** (`hydra-mapper/README.md:302-305`). Up and down borrow
   north and south; there are no floors (`lib.rs:17-21`).
3. **Placement**: breadth-first from the room with the most directional exits, "grid rips"
   on collision, bearingless exits as connectors after; hill-climb and compaction; a
   topological placement (`satisfiable.rs`) kept only when it wins outright
   (`positioner.rs:136-270`).
4. Indoor or outdoor, by "Obvious exits" against "Obvious paths" (`classifier.rs:5-8`).
5. Outdoor packing: picture anchors, connector BFS, a strip fallback (`packer.rs:5-8`);
   each building hung beside its street at `TOWN_SCALE` 4 (`interior_shelf.rs:877`).
6. Line routing around rooms (`routing.rs:1-12`).

It is a port of VellumFE's engine (`plan/26-mapper.md:10-25`), whose own spec is
`reference/VellumFE/docs/layout-engine-spec.md`.

**What "not perfect" is, measured by the mapper itself:**

| Problem | Count | Source |
|---|---|---|
| exits drawn against their bearing | 667: **358 on data that could be satisfied** (the solver's fault; its repairs are local), 309 in 25 groups whose exits truly contradict | `hydra-mapper/README.md:161-181, 293-299` |
| exits with no bearing to place by | 35%, a floor no solver moves | `README.md:302-305` |
| lines through rooms | 3,583 → 741 so far | commit `bfd07d4` |
| building rooms under a line not theirs | 5,403 → 4,299 | commit `6a37d23` |
| buildings welded to neighbours by indoor links | 19 groups | `docs/room-classification.md:254-256` |
| areas still undecided | 213 | `docs/room-classification.md:199-208` |
| rooms with no location / needing a walk | 1,357 / 421 | `docs/room-classification.md:396-401` |
| a keep drawn with long connectors | Ta'Illistim Keep, 123 rooms, 23 doors | `README.md:81-84` |
| VellumFE's quality targets | not yet reproduced | `README.md:300-301` |

Also: the shipped `cena-mapper.exe` predates the editor's newer flags (built 2026-09-23
12:22); the `G:` copy's code is two commits ahead of `C:\Users\shawn\hydra-mapper` (a drag
that moves the whole selection); and `G:`'s `gs.overrides.json` holds about a thousand more
lines of hand pins than `C:`'s.

**No run time for the whole map is recorded anywhere.** VellumFE's spec targets under
300 ms per zone on a worker thread, and reports 10 ms to 1.7 s for Wehnimer's in its
JavaScript reference (`reference/VellumFE/docs/layout-engine-spec.md` §1, §9).

### 1c. Despana's map: the styling

`crates/cena-web/assets/atlas/`, planned in `plan/28-despana-map-explorer.md` and
`plan/40-despana-live-map.md`. It draws positions laid out **offline** by the same engine
(`tools/atlas-export`, which pins `cena-map-layout` at `e98f669`), baked into 80 MB of JSON
(`crates/cena-web/atlas-data/`, frozen to the map hash above). The live minimap
(`minimap.mjs`) sits in the character page's Room pane and follows the room the binary
resolves (`MapLocationView { map_sha256, room }`, `crates/cena-ui/src/view.rs:275-298`).

What the GUI takes from it, as design tokens (dark only; `atlas/style.css:1`):

| Token | Value | Where |
|---|---|---|
| background | `#0b1119` (explorer canvas a radial `#132331`→`#0b121b`); minimap inset `#11161b` | `atlas/style.css:1`, `assets/style.css:141-154` |
| panel, line, text, muted | `#111a25`, `#263342`, `#dce5ed`, `#8e9fad` | `atlas/style.css:1` |
| room, in focus | rect, rx 1.4, fill `#497fa3`, stroke `#a2c8df`, 10 px | `atlas-view.mjs:275-276` |
| room, context | circle, fill `#7c95a7`, stroke `#9eb4c3`, 8 px | same |
| room, selected | `#f7f5df`, stroke `#fff2c1`, ring `#fff3c9` r 10 | `atlas-view.mjs:275, 282` |
| **you** (minimap) | ring r 1.8 map units, `#dddcd7`, 2.5 px | `minimap.mjs:132-147` |
| exit, strong / weak | `#6e99b5` 1.35 px, opacity .9 / `#385164` 0.8 px, .65 | `atlas-view.mjs:233-234` |
| connector / stub | dashed 5/4 / two dashed 3/2 stubs, each min(20%, 15 px) | `atlas-view.mjs:235-239` |
| route | `#57f3cb` 2.5 px, halo .3, animated 9/9 dash, an arrow on its longest leg | `atlas-view.mjs:247-255` |
| transition | gold `#ffc778` ring r 9 and "↗" (to another area); `#8db6ce` "↳" (local) | `atlas-view.mjs:303-306` |
| label | 11 px, box text+14 × 21, rx 3, `#181e25`, dashed leader; tried at 18/30/46/66/90 px in NE, NW, SE, SW, N, S; dropped when nothing fits | `atlas-view.mjs:288-297`, `labels.mjs:4-20` |
| place icons | bank `#e5be52`, furrier `#cfaa80`, gem shop `#64dafa`, pawnshop `#f3a25c`, adventurers' guild `#829fff`, locksmith `#c4a0ef`, healer `#f38d99`, herbalist `#91d578`, alchemist `#60d3bd`; by tag or title | `icons.mjs:2-18`, `preferences.mjs:2-127` |
| minimap camera | 80 × 56 map units; recentres only past 24 × 16 from centre | `minimap.mjs:151-155` |

Author decisions recorded there: *"a first minimap, not the full explorer squeezed into a
small pane"*, and a click on the minimap **never walks or sends a command**
(`plan/40-despana-live-map.md:7-12`); the room resolved with no history (`Whence::Nowhere`,
`plan/40:23-24`); a strict map-hash match (`plan/40:32-36`).

### 1d. VellumFE's map: what to learn

VellumFE has an egui minimap and a map explorer over **one shared painter**
(`reference/VellumFE/src/frontend/gui/map_view.rs`), laid out by its own engine from Lich's
data on a worker thread and **cached on disk by a hash of the rooms**, an `ENGINE_VERSION`
forcing a redo (`reference/VellumFE/src/core/layout_engine/cache.rs:1-22`,
`map_service.rs:398-462`). To copy:

- **The painter is a pure function** of a scene with no UI types and a camera the caller
  owns (`map_view.rs:14-28`, `scene.rs:1-5`).
- **Detail by zoom**: labels at ≥ 12 px a cell, connector labels at ≥ 14, room ids at ≥ 20
  (`map_view.rs:253-255`); labels painted last, tried in other places before they would cover
  a room (`:175-213`).
- **Cull** to the visible cells and one more (`:245-250`).
- **Glide** to the new room over a quarter second (`map_compass.rs:102-112`).
- **Indoors, only this building** (`map_compass.rs:58-67`).
- **Hold the last room rather than guess** (`map_service.rs:831-832`: a wrong room "would walk
  the map away from the player").
- **Long links become stubs**: a stub arrow labelled with the far room past 8 cells, nothing
  past 30 (`scene.rs:20-29`, *"spaghetti"*); mazes left out (`map_service.rs:421-440`,
  *"spiderweb"*).

To avoid: the minimap asks egui about **every visible room every frame**
(`map_view.rs:499-504`), where one hit-test at the pointer would do; the minimap has **no pan
or wheel zoom**, which its own book calls a gotcha (`book/src/widgets/map.md:109-111`); and
none of its painting has a test.

### 1e. What Hydra has for it

- **The current room**: `cena_behavior::travel::room_of(map, state, Whence)`, the game's
  number first, then the text, `None` on doubt (`crates/cena-behavior/src/travel/drive.rs:280-304`;
  *"0 wrong over 36,838 rooms"*, `plan/24-travel.md:18`). With memory of the last room
  (`Whereabouts::locate`, `crates/cena-agent/src/scripts/local.rs:93-111`) it settles rooms
  that read alike.
- **Walking to one**: `;go2 <map id>` (`crates/cena-behavior/src/travel/command.rs:7, 28`),
  sent by a widget as `Clicked::Send` with the character's own symbol, which moves off `;`
  while Lich runs (`crates/cena-session/src/command/handle.rs:350`).
- **A widget**: `Widget::draw_with(ui, &Seen, id, &Chosen)` over the character's snapshot
  (`crates/cena-gui/src/widget.rs:38-124`), its own settings page off its right-click
  (`crates/cena-gui/src/play/options.rs`), its settings saved with the layout
  (`crates/cena-gui/src/layout.rs:72-85`). There is **no map kind yet**
  (`crates/cena-gui/src/widget/kind.rs:17-105`).
- **The graph**: `cena-gui` may depend on `cena-session` and `cena-ui` only. **VERIFIED:**
  `crates/cena-arch-tests/tests/layering.rs:153`. The GUI is handed no map today
  (`crates/cena/src/play.rs:389`).
- **The author's earlier word on the look** (`plan/21-mapdb.md:466-470`, **VERIFIED**): the
  map panel based on Genie's: *"dark canvas, grid squares, colour-coded rooms, region labels,
  a hover card (map + room number, colour meaning, aliases, exits), route highlight, a
  pulsing 'you are here', left-click pin path, right-click walk."* And floors are required
  (`plan/21-mapdb.md:581-587, 609-620`), where the engine has none.

---

## 2. The shape proposed

```text
gs.map --decode--> Map ---------> room_of (the current room, per character)
                    |
                    +--layout, per area, on a worker; cached by hash--> Scene (cena-ui)
                                                                          |
                             the binary hands the GUI a scene provider    v
                                                        cena-gui: Map widget, one painter
```

- **Layout runs in Hydra, live, not baked.** One engine for the mapper, Despana and the GUI;
  a map the author curates in the mapper shows as it is, with no 80 MB re-export. Per area,
  lazily, on a worker thread, the way VellumFE does; cached on disk under Hydra's data folder
  by the map's hash, the overrides' and the engine's version.
- **A `Scene` in `cena-ui`**: rooms at cells, edges by kind (directional, connector, stub,
  continuation), labels, place categories, transitions. Plain data, no egui, no engine types,
  so the painter is testable and Despana could later draw the same scene live (`plan/40`'s
  open item, "live native scenes for arbitrary map revisions").
- **The binary joins them**, as it does for Despana (`cena_web::MapProjection`): it depends on
  `cena-map-layout`, builds scenes, and hands the GUI a closure. `cena-gui`'s edges do not
  change.
- **One painter** in `cena-gui` for the minimap now and the map window later.

---

## 3. The stages

### Stage 1 — Tune the layout engine (in hydra-mapper)

The engine is tuned where it lives, measured by the mapper's own counts (`stats.rs`), with
pictures (`svg.rs`) for the author to judge. **What tuned means is the author's four rules**
(§6 item 1), over the whole map:

- **exits drawn with their direction**, never against it;
- **no line drawn through a room**;
- **each building under its own street's line**;
- **exits with no direction placed well**: the 35% (`go door`, `out`, climbs) put where
  they read naturally, not flung out as long connectors.

1. **A baseline, measured.** Time the whole map and each area on `gs.map`, release build;
   the four rules as counts, per area and in total (the first three are the table in §1b;
   the fourth needs a count of its own: connector length and crossings); a picture of each
   of the author's usual places to judge by eye. Rebuild `cena-mapper.exe`.

   **BUILT 2026-09-29** on hydra-mapper's branch `tune-layout` (`19d7df5`):
   `cena_map_layout::quality` counts the four rules on a drawn scene, by the engine's own
   tests (the positioner's signs; routing's clearance, 0.4 of a cell); `cena-mapper
   --quality [map]` lays out every area baked into the map as the window draws it
   (`meta:area:`, the store's corrections and pins), measures and times it, and writes
   `<map>.quality.tsv`. **The baseline**, `gs.map` `d5e8ac60`, release:

   | | count |
   |---|---|
   | areas, rooms | 207, 31,787 (rooms with no baked area are not in it) |
   | time to lay out every area | 1.0 s; the slowest, the Landing's town, 102 ms for 2,596 rooms |
   | rule 1, exits against their direction | 720 |
   | rule 2, lines through rooms | 704 |
   | rule 3, building rooms under a line not theirs | 351 |
   | rule 4, pairs joined only without a direction | 10,799: 10,609 lines (**2,144 crossing another**; median 1.4 cells, p90 8.2), 154 stubs, 36 not drawn |

   102 of the 207 areas are clean on rules 1-3; the rest gather in the big towns, the
   Landing's town worst on every rule (45, 186, 133), then Solhaven, Ta'Vaalor and
   Ta'Illistim. These are not the README's 667, 741 and 4,299, which were one-off counts of
   the solver's groups and of a narrower "line not theirs"; these count what is drawn. The
   time settles §2: laying out an area live in Hydra costs at most a tenth of a second.
2. **A regression gate.** The four counts as a test: no change may make any of them worse,
   in total or in any area. Deterministic output, as VellumFE's `tests/layout_engine.rs`
   asserts.

   **BUILT 2026-09-29** (`05f3a0d`): `cena-mapper --quality-check [map]` measures again
   against five counts (the first three rules, directionless exits not drawn, directionless
   lines crossing another), leaving the table alone. A command, not a `cargo test`, because
   `gs.map` is not in the repository; its comparison is tested. Run twice it is identical in
   every area: the layout is deterministic. **Changed the same day by the author's choice**
   (`6431701`): it refuses (exit 1) only a count worse **in total**, and lists each area
   that traded one count for another; the event areas (`special-*`) are measured and not
   gated.

   **Since the baseline, 2026-09-29**, each step through the gate:

   - The positioner's repairs (`329c70c`, `e70bb72`, `55acc53`): a group whose bearings
     cannot all hold moves only what they force (`place_near`, up to 10% longer), and a
     group whose exits truly contradict sets the least-trusted aside as connectors and
     honours the rest.
   - `cena-mapper --compare <area>` (`e788b32`) draws an area by the engine and by the
     author's hand side by side (`<map>.<area>.engine.svg`, `.hand.svg`), with the four
     rules and agreement with Lich's picture for each. Hinterwilds taught the next three.
   - **The author's rulings from it.** Compactness is not a rule: the engine's own objective
     (shortest lines, empty rows and columns removed) is what packed the pits, the shops and
     the long lines together, and a smaller sheet is no longer reported as better. Indoors is
     not a building: no signal in the data tells the Pits of the Dead from a pub (Lich's
     picture draws both, room by room; the name, the forage tags and the terrain each fail
     on one or the other, measured), so *"We leave all indoor rooms without terrain off the
     map. their room that leads to an outside room, shows up on the map as a dot like it does
     now, the rest of them are hidden"*. And a connector is a free joint: *"yellow connectors
     should be stretchy, they should stretch until their whole group is free and clear"*,
     and *"should also curve around to find empty space"*.
   - **Indoor rooms without terrain hidden** but for their doorways (`b7132d9`,
     `cena_map_layout::hidden`), laid out apart so what they join is still packed together;
     an area with no outdoor room is drawn whole.
   - **Stretchy, curving connectors** (`ac4cc39`, `cena_map_layout::stretch`,
     `routing::curve_connectors`).

   | Rule, events aside | baseline gate | now |
   |---|---|---|
   | exits against their direction | 454 | 247 |
   | lines through rooms | 597 | 48 |
   | building rooms under a line not theirs | 291 | 35 |
   | directionless exits not drawn | 34 | 3 |
   | directionless lines crossing another | 1,876 | 437 |

   Rooms drawn fell from 31,787 to 19,444, which is the rule, not a loss. Laying out every
   area takes 3.5 s against 0.9 s; the slowest, Solhaven, 550 ms. Next, by the Hinterwilds
   pictures: a part of a group hanging on one compass exit slides along it until its
   directionless lines are clear (Berserker's End's stairs, which no bend can clear); compass
   lines stretched far past a step by empty columns kept across a whole group (Ojandhaart,
   Eldurhaart's grove); the positioner's objective, which still minimises length.
3. **Directions** (rule 1): the 358 drawn against their bearing on data that could be
   satisfied, by a repair that is not local (the README's own diagnosis). The 309 in groups
   whose exits truly contradict are data: curated, or drawn as the least wrong.
4. **Lines through rooms** (rule 2): the 741, in routing.
5. **Buildings under the right lines** (rule 3): the 4,299, in the shelf, and the 19 groups
   welded by indoor links.
6. **Exits with no direction** (rule 4): placed by what else the data says -- the stated
   bearing (`dirto`), the exit back the other way, the Lich picture's rectangle, a door's
   side, the room's own words, the building a door leads into -- and by keeping the
   connector short and uncrossed when nothing says.
7. **The hand pins into `gs.map`** (§6 item 3): the mapper writes the editor's
   `gs.overrides.json` pins into the map's `placement` extension, so Hydra reads the map
   alone.
8. **The author's list.** What still looks wrong in the pictures, one area at a time: a rule
   in the engine, or a fact about one place in `curation/`.

Done when the four counts are as low as the data allows, and the author says the map reads
right.

### Stage 2 — Bring it into Hydra

1. **The dependency** (§5 question 2), and `cena` → `cena-map-layout` in `ALLOWED_EDGES`
   (`crates/cena-arch-tests/tests/layering.rs`), with its reason.
2. **`Scene` in `cena-ui`**, and the conversion from the engine's output, tested on a small
   extract (the fixtures pattern of VellumFE's `tests/fixtures/layout/`).
3. **The scene service in the binary**: an area laid out on first need on a worker thread;
   the disk cache; the room → area index; from `gs.map` alone, the pins baked in (§6
   item 3). Its time measured against Stage 1's baseline.
4. **The current room per character**, kept by the binary for every widget of that
   character: `room_of`, with memory under the guard of §6 item 4, and a revision the widget
   recentres on.
5. **Hand it to the GUI**: `Sessions::attach` gains the provider (`crates/cena/src/play.rs:389`).

### Stage 3 — Style it: the minimap widget

1. **`Widget::Map`** in the catalog (`widget/kind.rs`), "Minimap" in Add a widget: a widget
   of its own, which a custom window can hold beside others (§6 item 8).
2. **The painter**, a pure function of scene, camera and style: rooms, edges by kind,
   transitions, place icons, you; culled; detail by zoom; labels last, placed by Despana's
   rules (`labels.mjs` ported, with its tests).
3. **Despana's tokens** (§1c) as the style, in one place (§5 question 5).
4. **The camera**: centred on you with Despana's dead zone and VellumFE's glide; indoors, the
   building alone; wheel zoom and drag pan (VellumFE's gotcha), a return-to-you on
   double-click; zoom kept per widget.
5. **Hover**: a card with the room's title, number, area, exits (the Genie card of §1e), from
   one hit-test at the pointer.
6. **Clicks** (§6 item 6): **left-click** shows the route to the room, as Despana draws it;
   **right-click** walks there (`;go2 <id>`, with the character's own symbol), perhaps a
   button later; **Shift+left-click** shows the room in the story as Lich's `;map` does on
   a shift-click (`reference/scripts/scripts/map.lic:2025-2033`: `respond room`, which is
   `#<id> (u<uid>):`, the title with its location, the description and the paths,
   `reference/lich-5/lib/common/map/map_gs.rb:126-128`), as Hydra's notice.
7. **Its settings page**: zoom, what to show (numbers, labels, icons, transitions), and the
   colours.
8. **Tests**: kittest images of the minimap on a fixed scene, as the other widgets have; the
   camera and the hit-test as units.

### Stage 4 — Present it: the map window

1. **A map window**, the explorer, in its own viewport as VellumFE's is: the area, pan and
   zoom, search by name or number, a route preview in Despana's route style, and *Walk
   here*.
2. **Areas and regions**: moving between areas by their transitions; a region overview.
3. **Despana on the same scenes** (optional): its minimap drawing the live scene instead of
   the baked atlas.

**Later, not in this plan:** floors (§6 item 7: curated, never computed), ghost rooms for
places the map lacks
(VellumFE's cartography mode), and editing the layout from Hydra (the mapper does that).

**Floors, measured for their own plan** (the author, 2026-09-29: *"What constitutes a floor
change? I think x amount of rooms on the same floor. How many rooms is the right amount
though?"*; hydra-mapper `examples/floors.rs`, `d1237a4`). A level is the rooms joined
without going up or down; 2,435 explicit up/downs join two levels, 833 of them to a level
of one room. A room count alone does not stop a stack: Sapphire Gate and Whistler's Pass
North, a mountain road, is 27 floors at one room and still 4 at thirty. Counting only
up/downs between indoor rooms (1,150) drops it, and leaves at 3 rooms 72 floors, tallest 7;
at 5, 51, tallest 6 (the Landing's coastal cliffs). Claude's proposal, not decided: a floor
is an indoor level reached by up/down that would be drawn **over** the level below; room
count only trims (3 is the knee); a stack over three floors is listed for the author. Not
counted yet: 2,108 stairs, ladders and hatches with no stated direction (`plan/21` §3e's
order of trust).

---

## 4. Order

Stage 1 first, as the author put it: a painter over a layout that reads wrong is wasted.
Stage 2 can begin beside Stage 1 once §5 question 2 is answered, since its seams do not wait
on the layout's quality. Stage 3 after Stage 2. Stage 4 last. A commit per step; Stage 1's
commits are in hydra-mapper, the rest in Hydra.

---

## 5. Questions for the author

1. **Which areas define "tuned"?** Proposed: the towns you use, the hunting grounds of
   `ojandhaart`, and Wehnimer's Landing; and Stage 1 is done when those read right to you.
2. **Where the engine lives for Hydra.** Proposed: a git dependency on
   `Nisugi/hydra-mapper`'s `cena-map-layout`, pinned by rev, with a `[patch]` in Hydra's root
   `Cargo.toml` pointing the engine's `cena-map` at Hydra's own `crates/cena-map`. Without
   it, the engine's `cena-map` (from git, `hydra-mapper/Cargo.toml:23`) and Hydra's are two
   crates, and their `Map`s do not mix. The other way: move the engine into Hydra's
   workspace, reversing `plan/26`.
3. **The overrides.** The editor's hand pins are in `gs.overrides.json` beside the map. Does
   Hydra read that file too, or does the mapper bake the pins into `gs.map` (its `placement`
   extension already holds drags) so Hydra reads the map alone?
4. **The current room: memory or none?** Despana chose none (`plan/40:23-24`). The GUI
   could keep the last room, as scripts and travel do, and settle more rooms that read
   alike; proposed: with memory, as travel.
5. **Dark only?** Despana's palette is dark; VellumFE takes its colours from egui's theme.
   Proposed: Despana's as the style, every colour on the widget's settings page.
6. **What a click does.** Despana: never walks (`plan/40:7-8`). Your Genie note: left-click
   pins a path, right-click walks (`plan/21:470`). VellumFE: a click walks. Proposed:
   left-click selects and previews the route, right-click opens a menu with *Walk here*.
7. **Floors.** You required them (`plan/21`); the engine has none, up and down borrow
   north and south. Proposed: not in this plan; a later one.
8. **The minimap's first placement**: a widget of its own in Add a widget (proposed), or
   inside the Room widget as Despana's is.
9. **Which hydra-mapper copy is the one.** `G:\dev\hydra-mapper` is newer in code and pins;
   proposed: it is, and `C:\Users\shawn\hydra-mapper` is retired once `CENA_MAP` points at
   `G:`'s `gs.map`.

---

## 6. The author's answers, 2026-09-29

1. **Tuned.** *"tuned to me means exits drawn right not against their direction, lines not
   drawn through rooms, buildings sitting under the right lines, exits without a direction
   being placed in a smart way, I know you ai's can figure that out!"* Four rules over the
   whole map, not a list of areas: Stage 1 is written around them.
2. **The engine.** *"yeah we can do a dependency for now, and it will probably get pulled
   into hydra once it's how we want it."* A git dependency, with the `[patch]` of §5
   question 2.
3. **The pins.** *"the mapper should write them into gs.map"* (Stage 1 step 7).
4. **The current room**, asked for Claude's suggestion and why. **CLAUDE'S RECOMMENDATION,
   CONFIRMED BY THE AUTHOR: with memory, guarded.** Memory decides only one step of the ladder
   (`crates/cena-map/src/locate.rs:1-40`): after the game's number and the text have both
   failed, 333 rooms are still ambiguous, and where the character was settles 91 of them. It
   can never overrule the game's number. Despana chose none because its view is sampled:
   *"Coalesced observations do not supply reliable step-by-step history"*
   (`plan/40-despana-live-map.md:23-24`). Two moves between two looks, and "the room it just
   left" is the wrong room. The guard: the binary keeps one tracker per character, and
   uses where it was only when the move count (`GameState::arrivals`) has gone up by
   exactly one since it last looked (the same count means still there); by more, no
   memory. So the gain is real and small, and the risk Despana named is shut out.
5. **The look**: *"that works to start out, but what all are our options?"* Two separate
   choices. **Where the colours come from:** (a) Despana's fixed dark palette; (b) egui's
   own theme, as VellumFE does, so the map turns light when the window does (Hydra sets no
   theme of its own today, and egui follows the system's); (c) Despana's palette as the
   defaults, every colour changeable on the widget's settings page, as the bars' are. **What
   a room's colour means:** nothing but where you are and what is selected (Despana); its
   **terrain or climate**, which the map carries for each room (`crates/cena-map/src/room.rs:83-88`,
   VellumFE tints by terrain); its **area**; the **kind of place** (Despana's icons: bank,
   shops, healer...); or Genie's colour-coded rooms (`plan/21-mapdb.md:466-470`). **Start,
   Claude's recommendation, confirmed by the author:** (c) with Despana's meaning, plus the
   place icons; the other meanings as a choice on the settings page later.
6. **Clicks.** *"I did like the way despana showed the route on left click, right click to
   travel is fine for now, might change to a button. shift + left click should display the
   room in the story window like ;maps does."* (Stage 3 step 6.)
7. **Floors.** *"floors are a touchy subject, they have to be curated otherwise you end up
   with 14 floors going down a mine"*; and not in this plan (*"yep"*).
8. **Placement.** *"it'll be a widget, that can be placed in a custom widget."*
9. **The copy.** *"g:\dev is the right one yes."* `G:\dev\hydra-mapper` is the mapper and
   its `gs.map` the map; `C:\Users\shawn\hydra-mapper` is retired once `CENA_MAP` points at
   `G:`'s.
