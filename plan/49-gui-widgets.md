# 49 — The GUI, second stage: widgets, custom windows, and what follows

> **STATUS: APPROVED, 2026-09-27.** Every question is answered; the author's decisions of
> the day are quoted in §1, including the stages' order (§4, row 6). Claude's readings are marked as such. `plan/47`
> (M10's first build) is BUILT; `plan/28` stays the inventory of VellumFE's GUI and holds
> the layout decisions this file builds on; this file is what gets built next, and in what
> order. `plan/12` wins any contradiction, except where §1 records the author changing it.

---

## 0. Why this file exists

M10's first build is done: all eleven of `plan/47`'s steps are BUILT (2026-09-27), and it was
by design *"everything the M6 live run needs, and nothing more"* (`plan/47` §3). What comes
after it was named there in one line, with no steps: the launcher tabs; drawers, opacity,
click-through; themes, skins, fonts, settings editors; the map, creature field, injury doll,
effects, gamepad; Despana's full port (`plan/47` §3, **Later**). `plan/28` is the inventory
to draw from (§7d.4, what a GUI owns; §8, VellumFE's census), not an order.

The author asked, 2026-09-27: *"do we have more stages to the gui in the plan or we need to
extend the plan?"* This is the extension.

What it stands on, decided before it and not reopened:

| Decided | Where |
|---|---|
| Identity, presentation and placement kept apart; stable opaque ids, never derived from a name | `plan/28` §7c |
| Free rects with the grid as a snap target; a main area and four drawers; a **custom window** whose inside is the same grid, one level down. *"Widgets inside a container window are bare -- the container owns the chrome."* | `plan/28` §7d, DECIDED 2026-09-22 |
| Chrome follows the widget's parent, never a side table (Vellum's `no_title_tabs`) | `plan/28` §7d.3 |
| Drawers: clip or push, translucency, click-through derived from opacity | `plan/28` §7d.5, settled 2026-09-22 |
| A widget says its own geometry (`SizePolicy`); layout code never asks what a window *is* | `plan/28` §7e |
| The settings taxonomy before any editor; the author: *"we shall discuss this when it's time taking inventory of all settings we have and need"* | `plan/28` §7f, §9 item 3 |
| R1-R4: every active character's story visible; no window mixes two characters' story; each character's state at a glance; shared streams merged once | `plan/29` §5a |

---

## 1. Decisions (author, 2026-09-27)

The author showed Saga's panel list (§3) *"for ideas"*, and said:

> *"One thing that I like .. we said a custom window with a grid that you can add widgets
> into and they're frameless. So this means we should probably make all the widgets
> frameless that get tossed into custom windows. We can make some of those as standalone
> things they can add or make their own, ect?"*

Asked three things in reply, answered the same day:

| # | Asked | Answer | What it means here |
|---|---|---|---|
| 1 | How small is a widget? | *"individual things, a bar for health is a widget, a bar for stamina is a widget, each hands are their own widget, ect."* | One widget per fact a player reads alone. Saga's composite panels (Experience, Loadout) are **presets**: custom windows made of such widgets (§2). |
| 2 | Tabs, as Saga's Story, Thoughts and Announcements | *"tab stack sounds good"* | A tab stack is a cell of a custom window holding several widgets, one showing (§2), not a second grouping mechanism: Vellum's `TabGroup` was that, and it is the buggy part (`plan/28` §7d). |
| 3 | May a widget show another character than its window's -- party vitals? | *"I like the idea but I also don't want to complicate the ui and all that right. so yes but an advanced option/menu/place to access them."* | Yes, behind one Advanced place; a widget follows its window's character unless a player went there (§2). |

The third answer is also a rule for everything below: **the everyday surface stays simple,
and the power goes behind one Advanced door.**

Given this file's first draft, the author answered again the same day:

| # | Asked | Answer | What it means here |
|---|---|---|---|
| 4 | Voln, Mentor, Host: what are they? | *"mentor and host are positions you hold with the game, not for everyone, they're just text streams like thoughts, same for voln, it has it's own thought stream."* | Stream widgets like any other (§3). |
| 5 | The live run first? | *"No m6 live run until I have a proper ui."* | **The M6 live run waits for the GUI** (§4). |
| 6 | The order of the stages | *"sure"* | A, then B-H, as §4 lists them. |
| 7 | A stream widget always follows its window's character? | *"yes"* | DECIDED: R2 holds by construction (§2). |
| 8 | Where the Advanced place is | *"sounds good."* | DECIDED: the bottom of the Add-a-widget list, closed, and one *Advanced* entry in a widget's right-click menu; nowhere else. |
| 9 | Tab stacks only in a custom window? | *"what do you recommend?"*; given §2's recommendation, *"yep custom windows only."* | DECIDED (§2). |
| 10 | Presets shared, layouts per character? | *"presets being our put together windows? Does sharing mean editing a preset edits it for all characters?"*; given §2's answer, *"yep own copy"* | DECIDED: adding a preset places the character's own copy (§2). |

---

## 2. The model

**A widget** draws one thing for one character, and nothing around it: no title bar, no
frame, no border. It is:

- a **kind** from the catalog (§3): the health bar, the right hand, the compass, a stream;
- its **options**, the kind's own: which streams a stream widget shows, where a bar puts
  its words (`crates/cena-gui/src/bar.rs`, `plan/47` step 10);
- its **geometry**, said by the kind (`plan/28` §7e): a bar is short and wide, a hand is
  one line, a stream takes what it is given;
- whose **character** it follows: its window's, unless changed in the Advanced place.

**Two holders**, and only two:

- **A standalone window** holds one widget and gives it chrome: a title and a frame. It is
  what each of M10's five panes is today (`crates/cena-gui/src/play/panes.rs`,
  `pane_window`).
- **A custom window** holds a grid of widgets, bare, with one frame and an optional title
  for the whole. Inside, widgets are placed by the same snapping as the play window's
  (`crates/cena-gui/src/snap.rs`), one level down: free rects, the grid a snap target.

**Chrome follows the parent** (`plan/28` §7d.3). A widget has no "framed" setting: dragged
into a custom window it loses its frame, dragged out it gets one back.

**A tab stack** is a cell of a custom window holding several widgets, one showing, with a
tab for each and the unread count of a stream not showing (Saga shows *THOUGHTS 2*).
**DECIDED (§1 row 9): only there.** A player drops a
widget on a standalone window's title and the two become tabs; what Hydra keeps is a custom
window of one cell, which looks exactly as the tabbed window should. So there is one tab
mechanism, one thing saved, and one set of drag rules. The alternative, tabs on a
standalone window as well, is two ways of holding several widgets, which is Vellum's
`TabGroup` beside its layout again (`plan/28` §7d, *"a second layout system with no
relationship to the first"*).

**A preset** is a window already put together, as the author put it: a custom window
Hydra ships, such as Saga's Experience or Loadout rebuilt from single widgets, or one a
player saved. It is not special code. **DECIDED (§1 row 10): adding a preset places the
character's own copy.** Editing that window changes that character's window and
nothing else, and the preset changes only when a player saves over it, which changes no
window already placed from it. Presets live in one library every character adds from;
each character's layout is its own. A preset that edits every character at once would
make every edit a question (*this character, or all of them?*), which is the
complication the author asked to avoid (§1 row 3). A player who wants one layout
everywhere copies a whole layout to another character in one action.

**Ids.** A placed widget gets an opaque id when it is placed (`plan/28` §7c), so two of one
kind can coexist: a second story widget showing only thoughts. Today's layout is keyed by
a closed `Pane` enum of five (`crates/cena-gui/src/layout.rs`) and cannot say that.

**Following another character** (party vitals): chosen only in the Advanced place. A
stream widget always follows its window's character (DECIDED, §1 row 7), so R2 (no window
mixes two characters' story) holds by construction; any other widget may follow another.

**Saved.** A character's layout stays in `layouts/<name>.json` in the data folder
(`plan/47` step 6), gaining its holders and widgets as a new version. A version-1 file is
converted once, its five panes becoming five standalone windows where the player left
them; today `Layout::load` drops a file of another version and fits afresh
(`crates/cena-gui/src/layout.rs`, `load`), which would throw the player's layout away.
Presets are one library beside the layouts.

---

## 3. The catalog

Saga's panels as the author's screenshots show them (2026-09-27), in Saga's three groups, each
mapped to what Hydra's model holds. MEASURED 2026-09-27 by reading the model, with the cited
lines checked. **The list may be incomplete:** it came in five scrolled screenshots, and two
seams (after *Spellbook*, after *Encumbrance*) may hide entries. UNVERIFIED.

The GUI reads `GameState` directly (`plan/28` §7b, the author's third option), so a widget
needs no projection: `SessionView` (`crates/cena-ui/src/view.rs:237`) carries far less than
the model holds, and stays the remote viewers' format.

**READY**: the model holds it. **MODEL FIRST**: parsed and dropped, or never read; the model
work goes first, in `cena-model`. **GUI**: a widget with no game fact behind it.
**UNKNOWN**: what Saga shows is not known here; to ask.

### Streams

One widget kind, whose option is which stream ids it shows. The model keeps every stream
the game sends, by id (`crates/cena-model/src/state/streams.rs:119`, `GameState::stream`),
so a stream widget can show any id, including one no one listed. **The Add-a-widget list
offers every stream this character has received**, under its known name where there is
one and its id where not: a stream that comes only with a position the game gives (the
author: Mentor and Host, §1 row 4) appears for the characters that hold it, and Hydra
never has to be told its id.

| Saga | Stream id | State |
|---|---|---|
| Story | the main stream | READY: M10's Story |
| Thoughts, Speech, Arrivals, Deaths, Announcements | `thoughts`, `speech`, `logons`, `death`, `announcements` | READY; the hub merges the same five (`crates/cena-ui/src/merge.rs:34`) |
| Familiar | `familiar` | READY, as any id is |
| Spellbook | `Spells`, read into typed rows (`crates/cena-model/src/state/known_spells.rs:52`) | READY, as a list |
| Room | the `room` window | READY: M10's Room, split below |
| Voln | `voln`, the Order of Voln's own thoughts (`reference/lich-5/lib/common/markup.rb:213`) | READY, as any id is |
| Mentor, Host | streams for those the game makes mentors or hosts (§1 row 4); no id in Lich, VellumFE or Hydra | READY: offered when received, as above |

### Info panels

| Saga | Model | State |
|---|---|---|
| Active Spells, Buffs, Debuffs, Cooldowns | `effects`, by the game's four categories (`crates/cena-model/src/effects.rs:447`) | READY, a widget each |
| Encumbrance | level, percent and blurb (`crates/cena-model/src/state/character.rs:230-234`) | READY: a bar and a line |
| Stance | stance and its percent (`character.rs:226-228`) | READY: a bar |
| Objectives | `objectives` (`crates/cena-model/src/state.rs:159`) | READY |
| Society | society and rank (`crates/cena-model/src/state/character/standing.rs:51`, `:58`) | READY |
| Resources | resource, suffused, Covert Arts charges, shadow essence (`standing.rs:69-77`) | READY |
| Container | `inventory` and `containers` (`state.rs:193`, `:258`) | READY; which container is the widget's option |
| Loot | the loot queue is emptied each prompt (`state.rs:293`); the ledger keeps it (`plan/34`) | MODEL FIRST: a standing list, or the widget reads the ledger |
| World Events | `Frame::WorldEvent` is parsed and dropped (`state.rs:494`) | MODEL FIRST |
| Combat Actions | the combat tracker hands its facts on, keeping no list | UNKNOWN |

### Graphics

| Saga | Model | State |
|---|---|---|
| Vitals | the four vitals (`state.rs:157`) | READY: **four widgets**, a bar each (§1 row 1) |
| Roundtime | roundtime and cast time (`state.rs:155`, `:185`) | READY: two widgets |
| Loadout | each hand, the prepared spell, the reserve (`state.rs:146-148`, `:188`, `:207`) | READY: **a widget each**; Loadout is a preset |
| Experience | level, mind, training points, experience (`character.rs:79`) | READY: a widget each; Experience is a preset |
| Indicators | nineteen, from standing to diseased, each known or not (`crates/cena-model/src/status.rs:192`) | READY: a widget each; Indicators is a preset |
| Compass | the room's exits (`crates/cena-model/src/state/room.rs:206`) | READY |
| Combat | the creatures in the room with their statuses and health, friend or foe (`crates/cena-model/src/state/creatures.rs:161`) | READY; M10's Room pane lists them by name only |
| Health | the injuries and scars (`character.rs:221`) | READY data; the doll's art is Stage G |
| Map, Minimap | room ids (`room.rs:163`); the map is `cena-map`; Despana's minimap is web-side | Stage G |
| Pulse Timer | `Frame::Pulse` is parsed and dropped (`state.rs:494`) | MODEL FIRST |
| Command | the play window's one input (`plan/47` §1 row 2) | GUI: placed like a widget, never two |
| Hotbar | buttons that send a line, as keybinds do (`crates/cena-gui/src/keys.rs`) | GUI |
| Almanac, Badge | none found | UNKNOWN |

### Hydra's own

Not Saga's: Hydra's messages and the Hunt panel (M10), and the room split into its title,
description, players, objects and exits, each a widget.

---

## 4. Stages

**The order is the author's (§1 row 6), and the M6 live run waits for the GUI:** *"No m6
live run until I have a proper ui"* (§1 row 5), and it goes *"when I feel the GUI is
done"* (§5 item 6): the author's call, not a stage's.

### Stage A — widgets and holders (the foundation)

Everything after it adds widgets, so the model goes first and every widget is written once.

1. **The widget.** A closed catalog in `cena-gui`, one enum of kinds with their options,
   each drawing into the space it is given and saying its geometry. An enum, not a trait:
   every kind lives in one crate, and a trait object buys nothing here (`plan/05` §−1).
2. **M10's panes become widgets.** Story, Hydra's messages and Hunt as they are; Vitals as
   four bars; the Room split as §3 says; from the top bar, roundtime, cast time and each
   hand. The top bar keeps who, the connection, Stop and its menus.
3. **Standalone windows** hold them, framed, as today; the layout keyed by id, a
   version-1 file converted.

   **Steps 1-3 BUILT 2026-09-27, in one commit**, since step 1's catalog had nothing to
   draw it until step 2. The catalog is `crates/cena-gui/src/widget.rs`: seventeen kinds
   (the story, a bar per vital, each hand, roundtime and cast time, the room's name,
   description, creatures, objects, players and exits, Hydra's messages, the hunt), each
   drawing itself bare (`widget/draw.rs`), saying the size it would like, and saying what it
   does not know rather than drawing nothing (*"Creatures: unknown"*, *"Right: ?"*). A
   one-line widget stays one line in a narrow cell, cut short with an ellipsis and whole on
   hover; the rest scroll. So nothing spills out of its cell, and a clip meant to stop that
   was removed as untestable. The room's description is styled by one function the web view
   shares (`cena_ui::room_description`), moved down from its projection.

   The layout is version 2 (`crates/cena-gui/src/layout.rs`): windows (holders), each a
   standalone window around one placed widget or a custom window of cells, every window and
   widget with an id of its own. The first layout is the story on the left and, down the
   right, custom windows Vitals (four bars), Loadout (each hand, then the two clocks side by
   side) and Room (its five parts), with Hunt and Hydra standalone; in a short play area the
   first three give way so Room and Hydra keep a window's smallest height. **Version 1 is not
   converted: no file of it exists.** The M10 run's layouts were in the M10 worktree's data
   folder, which its removal deleted, and the real data folder has none (checked
   2026-09-27), so a converter would convert nothing; a version-1 file is not read, and a
   fitted layout takes its place. The top bar keeps who, the connection, the menus and Stop;
   its hands and clocks are widgets now, and the Layout menu's *Lay out afresh* replaces
   *Fit the panes afresh*.

   A custom window's cells follow it when it is resized (`layout/custom.rs`): across, each
   scales with the inside's width, so a row of bars stays as wide as the window and two
   clocks stay halves; down, a cell on the bottom edge keeps to it, and a line stays a line.
   Its cells are first laid for an estimate of the inside (the frame and title bar take 12
   by 44 points, measured in a rendered play window) and kept to the real one when drawn.
   The play window's windows keep M10's mechanics unchanged (`play/holders.rs`, formerly
   `panes.rs`).

   Tests: every widget drawn and found as a screen reader finds it, known and not; a
   one-line widget in a narrow cell; the first layout tiling the area in order, in a short
   area too, every id its own; a custom window's rows laid, shared when they ask too much,
   and kept to a resized inside; the play window found by its windows' titles, a drag, a
   resize, a shared edge, *Lay out afresh*, and a small move in open space staying where it
   was put. Thirteen mutants: eleven caught; a window snapping to its own starting edges,
   so a nudge sprang back, caught once a test was written for it; and the clip, which
   survived and was removed for one-line widgets whose ellipsis a test holds.
4. **Custom windows.** Made from a menu; a widget dragged in or out, its chrome following;
   the grid inside.

   **BUILT 2026-09-27.** A widget in a custom window is bare, with no title bar to take
   hold of, so the play window's Layout menu gains **Arrange** (`crates/cena-gui/src/play/arrange.rs`).
   On, each cell is dimmed under its outline and its widget's name, which it also says to a
   screen reader, and takes the pointer: dragged from inside it moves, from near an edge
   that edge moves (one edge per axis, the pointer showing which), and it lands snapped to
   the inside's edges, the other cells and the grid, its snap lines drawn in its window,
   and always inside it: a move slid back in, a resize cut at the edge. Let go outside its
   window, the widget leaves, its name following the pointer until then: into the custom
   window beneath, or into a standalone window of its own, framed; its window's last widget
   out takes the window with it. A standalone window carried, not resized, onto a custom
   window joins it, bare. With Arrange off none of this happens, so no press in play
   rearranges anything (§1 row 3); a play window opens with it off. *New custom window* in
   the same menu makes an empty one, saying how to fill it, and turns Arrange on. Claude's
   design: the plan named the step, not how a bare widget is taken hold of. The layout's
   side is `layout/moves.rs`; a press on a cell while arranging lets go of no window, so a
   window's own grid is not shown for a cell's gesture.

   Tests: the cells named only with Arrange on, from the menu; no widget leaving its window
   with it off; a cell moved and snapped to the inside's bottom, moved partway out and slid
   back, resized by its edge, resized past its window and kept in it; a widget dragged out
   into its own window, the drag rendered with its name at the pointer
   (`tests/snapshots/arrange.png`); a window dropped on a custom window joining it only when
   arranging, and one resized over it never; a new custom window empty and arranging; the
   arranged layout kept by name; the layout's release, join and take, and a press's edges
   and pointers. Twenty mutants: fourteen caught at once; four once a test was written for
   them (a resize counted as dragging out, a resized window joining, a tab stack's shown
   tab left past its end, a move not slid back in); a filter keeping a widget from being
   released into its own window found vacuous and removed, with its twin in joining; and a
   repaint after a layout change removed, since the tests show a window settles without it.
5. **Tab stacks**, with unread counts.

   **BUILT 2026-09-27.** A cell holding more than one widget draws a strip of tabs, one
   line tall, across its top: a click on a tab shows its widget, with Arrange on or off. A
   tab not showing says what its widget has said since it last showed -- *"Hydra 2"*, as
   Saga's *THOUGHTS 2* -- for the widgets that count what they say (the story counts the
   game's lines, Hydra's messages count themselves; the rest count nothing). A widget first
   seen as a hidden tab counts from then, and a widget drawn showing, anywhere, is read up
   to now: one function draws every showing widget and marks it read
   (`crates/cena-gui/src/play/draw.rs`, `shown`).

   Making and unmaking them is arranging's (step 4), under one rule for everything let go:
   **over the middle half of a widget, it joins that widget's tab stack; anywhere else, it
   takes a place of its own.** The middle half, not the whole, so a full custom window can
   still be rearranged; the widget it would join lights up while it is over it. A
   standalone window carried onto another's title bar stacks with it, the two becoming one
   custom window of one tab stack where the other was, as §2 decided. A tab dragged alone
   leaves its stack; a stack's body drags the whole stack, which, let go in the open, keeps
   together as a custom window of that one stack, still showing what it showed. A tab let
   go on empty space in its own window takes a cell of its own there.
   (`layout/moves.rs`, `Taking`, `stack_onto`, `stacks_at`.)

   Tests: a click switching, with Arrange on too; counts for a hidden tab, none for one
   showing even on the frame a line arrives, and counting only what came after being read;
   the story's counters; stacking by a title, only when arranging; a tab onto a widget in
   another window, a cell onto a widget's middle and not its side, a tab back to empty
   space; a stack let go in the open; a cell stacked onto itself losing nothing; and two
   images, a stack with its tabs (`tests/snapshots/tabs.png`) and the widget a cell would
   join lit up mid-drag (`stack.png`). Twenty-one mutants: seventeen caught at once; four
   once a test was written for them (reading a widget while it shows, a lone cell's
   reading, a tab let go on empty space, and a showing tab's count on the frame a line
   arrives). Reading had been done in three places, which is why two survived; it is done
   in one now. And one design change came of a failing test, not a mutant: stacking had
   been anywhere over a widget, which made a full custom window impossible to rearrange.
6. **Add a widget.** Every kind in one searchable list (Saga's *"Find a panel..."*), grouped
   as §3; the Advanced place at the bottom of it, closed, with following another character.

   **BUILT 2026-09-27** (`crates/cena-gui/src/play/menu.rs`). The Layout menu's *Add a
   widget...* opens the list: typing narrows it, its groups are §3's (Streams, Info panels,
   Graphics, Hydra's own; a group with nothing in it yet is left out), and each kind already
   shown says so. A click adds it in a standalone window of its own, set down and right of
   the last so several added stay apart. At the bottom, closed, **Advanced**: *Show for*
   this character or another running one. Chosen, what is added follows that character,
   its window titled whose (*"Stamina (Baelor)"*) and its content named (*"Baelor SP ?"*);
   a story cannot be added so (§1 row 7).

   A **right-click** on a window or a widget in one opens its menu: *Remove* the widget (a
   standalone window goes with it; a custom window stays, even empty); for a custom window,
   *Rename...* and *Remove window*; and, for a widget that is not a story, one **Advanced**
   entry, closed, with the same *Show for*. Escape, or a click outside, closes it. That is
   the whole of the Advanced place, as §1 row 8 has it.

   Following is kept once, in the layout (`Layout::follows`, by widget id), not on each
   widget, and saved with it; removing a widget or its window forgets it. A widget drawing
   for another character draws from that one's snapshot and hunt, never its story, which
   no other window shows; a story named in the file as following someone shows its own
   lines regardless. Following a character not running says so (*"Lorwyn is not
   running."*). The window gathers the other characters each frame (`App`), and the
   catalog's kinds moved to `widget/kind.rs` so the facade stays small as Stage B adds to it.

   Tests: the list found by typing, its groups and what is shown, a click adding; the
   Advanced place closed, adding for another character, a story refused; the right-click
   menu removing a window, one widget of a custom window, renaming and removing a custom
   window, following from its Advanced entry, none for a story, closing on Escape and on a
   click outside; a follower of lines naming whose; a character not running; a story that
   stays its own; and the layout's add, remove, rename and follow, following saved.
   Twenty-one mutants: twenty caught at once; the twenty-first, a story allowed to follow,
   once its test followed a character not running, where that is seen.
7. **Presets.** A Vitals row and a Loadout from the widgets that exist; saving a custom
   window as a preset.

### Stage B — the catalog, widget by widget

Each widget whose data the model already holds (§3), in small steps, cheapest first.
A widget whose data the model does not hold is **not** built here: its model work is
listed and goes first, in the crate that owns the fact.

### Stage C — the launcher tab

The hub's third tab, *"incorporate the things lich's launcher can do"* (`plan/47` §1 row 6):
saved logins (today's Start list, from the roster), a manual login, accounts, favourites,
game and instance. Lich's launcher is the reference
(`reference/lich-5/lib/common/gui/saved_login_tab.rb`, `manual_login_tab.rb`,
`account_manager_ui.rb`, `favorites_manager.rb`, `game_selection.rb`).

### Stage D — settings

First the inventory of every setting, with the author (`plan/28` §9 item 3); then editors
rendered from a schema, not written by hand (`plan/28` §7f, §7g); and one front door in the
hub for help and settings. The user quality-of-life review of 2026-09-26
(`44-user-qol-review.md`, not yet committed) names the gaps this closes: no screen to set up
healing or edit a hunt profile (its Q01, Q03), no front door (Q04).

### Stage E — drawers

A main area and four drawers, each clipping or pushing, with translucency and click-through
derived from opacity (`plan/28` §7d.5, settled). A widget or custom window lives in the
main area or a drawer (Saga's *Main* column).

### Stage F — look

Themes, skins, fonts, per-window appearance (`plan/28` §7d.4 items 2, 3, 6, 10).

### Stage G — the graphic widgets that are projects of their own

The map and minimap, the injury doll, and the creature field (in scope, still changing:
`plan/28` §9 item 5). Each gets its own plan when it is next.

### Stage H — packaging

`plan/47` §5, **Packaging, later**: no console behind a double-click on Windows, an `.app`
bundle on macOS, a `.desktop` entry on Linux.

### Carried from M10

- Pause, Scroll Lock and macOS's Clear: the egui fork's key hook widened, the author's to
  make (`plan/47` step 7).
- A bar's options saved: a widget's options (Stage A) are saved with the layout.
- CI's first run on macOS and Linux (`plan/47` step 9), which needs a pull request.

---

## 5. Open, for the author

1. ~~**The order.**~~ **DECIDED** (§1 row 6).
2. ~~**A tab stack lives only in a custom window.**~~ **DECIDED** (§1 row 9).
3. ~~**A stream widget always follows its window's character.**~~ **DECIDED** (§1 row 7).
4. ~~**Adding a preset places a copy.**~~ **DECIDED** (§1 row 10).
5. ~~**Where the Advanced place is.**~~ **DECIDED** (§1 row 8).
6. ~~**Which stage makes the UI proper**, so the M6 live run can go?~~ **DECIDED, the
   author, 2026-09-27:** *"When I feel the GUI is done."* No stage is the gate; the author's
   judgment is. (Claude had recommended A and B.)

---

## 6. What the rules require

- **Layering.** All of Stage A is `cena-gui`. A widget that needs a fact the model lacks
  gets it in the crate that owns the fact, moved down, never computed in the GUI.
- **File caps.** Widgets get their own module directory from the first commit, so
  `crates/cena-gui/src/play/draw.rs` shrinks rather than grows; a split parent is capped
  at birth.
- **`missing_docs`**, clippy at `-D warnings`, rustdoc links.
- **The glossary**: *widget*, *standalone window*, *custom window*, *tab stack*, *preset*;
  *pane* retired or defined as the old word for a standalone window.
- **Nothing is run live by Claude.** Every check is a test or a kittest image.
