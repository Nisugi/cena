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

   > **CHANGED 2026-09-28.** The first layout's Room window is now the one **Room** widget
   > (`crates/cena-gui/src/widget/described.rs`). It draws the room as Wrayth does, and its
   > own page picks the parts, at the author's asking (`plan/50` §7 step 8 records it). The
   > custom window of parts is still what the tests of arranging cells lay out
   > (`Layout::with_room_parts`), and a layout saved before keeps it.

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

   **BUILT 2026-09-27** (`crates/cena-gui/src/layout/preset.rs`). Hydra ships four:
   *Vitals* (the four bars stacked), *Vitals row* (side by side, Saga's bar across the
   bottom), *Loadout* (each hand, then the two clocks) and *Room* (its six parts, the
   description too). They head the Add-a-widget list, above the kinds, found by the same
   typing; a click places a copy, and with the Advanced place showing another character,
   its widgets follow that one, but a story, which stays its window's own -- a party's
   vitals in one click. A right-click on a custom window offers *Save as preset...*, under
   a name typed (not an empty one); it goes into **one library every character adds from**,
   `_presets.json` beside the layouts (a name no character's layout can take), over one of
   the same name, and a saved preset can be
   forgotten from the list. Placing one gives its widgets ids of their own, so **what is
   placed is the character's own copy** (§1 row 10): nothing done to the copy reaches the
   preset, and nothing saved over the preset reaches a copy placed before. The library
   belongs to the app, which every play window asks to keep or forget one
   (`Asked::SavePreset`, `Asked::ForgetPreset`); one it cannot write says so in the list.

   Tests: Hydra's presets named once and the row one row; a copy with ids of its own,
   untouched by the preset saved over later; a preset placed for another character, its
   story its own; the library kept in its file, kept over, forgotten, of another version
   unread, and unwritable saying so; the list placing a preset, for another character too,
   and narrowed by typing; a custom window saved from its menu, not under an empty name,
   and forgotten; and the app keeping and forgetting through a play window, in the file.
   Sixteen mutants: fourteen caught at once, two once a test was written for them (the app
   forgetting, and an empty name).

   **Stage A is BUILT**, steps 1-7, 2026-09-27: every panel a widget, standalone and custom
   windows, arranging, tab stacks, the Add-a-widget list with the Advanced place, and
   presets. Stage B, the catalog widget by widget, is next.

### Stage B — the catalog, widget by widget

Each widget whose data the model already holds (§3), in small steps, cheapest first.
A widget whose data the model does not hold is **not** built here: its model work is
listed and goes first, in the crate that owns the fact.

Claude's order, cheapest first, set when Stage A was done (2026-09-27):

1. **What the character is and has**, read off facts the model keeps as fields: stance,
   encumbrance, mind and the next level as bars; level, training points, the experience
   numbers, the prepared spell, society, resources, objectives. An Experience preset.
2. **Status indicators** (nineteen, a widget each, and an Indicators preset) and the
   **effects lists** (Active Spells, Buffs, Debuffs, Cooldowns; a preset of the four as
   one tab stack, as Saga's panel has them).
3. **A compass** of the room's exits, and **the combat list**: friends and foes in the room
   with their statuses, as Saga's Combat panel.
4. **The game's streams**: thoughts, speech, arrivals, deaths, announcements, familiar,
   Voln, and any other the character has received (§3); the story no longer showing a
   stream's lines while a widget of that stream is open.
5. **The spellbook**, a **container**, and **the reserve** (Saga's R1-R3).
6. **What the model does not keep yet**: World Events and the pulse timer, stored where
   they are parsed and dropped; and Loot, whose standing list is a question (§3).

   **Step 1 BUILT 2026-09-27** (`crates/cena-gui/src/widget/character.rs`). Twelve kinds:
   *Stance*, *Encumbrance* and *Mind* and *Next level* as bars, each labelled in the
   game's words (*"Stance: defensive (100%)"*, which says its percent already, so the bar
   does not say it twice) and plain while unknown (*"Stance ?"*); *Encumbrance, in words*;
   *Level*; *Training points*; *Experience*, the numbers the game has told, grouped as it
   writes them; *Prepared spell*; *Society* with its rank; *Resources* (the profession's
   resource against its caps, suffusion, Covert Arts charges, shadow essence); and
   *Objectives*. Hydra's presets gain *Experience* (level, mind, next level, training
   points, the numbers). The catalog's kinds moved to `widget/kind.rs` in step 6 took
   them as a variant and a line in each table, and the dispatch in `widget/draw.rs` stays
   one arm a kind, allowed its length by name as `cena-model`'s container classifier is.

   Tests: every kind said as the game said it, and each saying what it does not know yet;
   numbers grouped. Objectives are tested empty only: their type is `cena-protocol`'s,
   which the GUI does not depend on, and a test is no reason to add the edge. Ten
   mutants: nine caught, the tenth malformed (it did not build), its line asserted.

   **Step 2 BUILT 2026-09-27** (`crates/cena-gui/src/widget/status.rs`). Each of the
   nineteen status indicators is a widget of its own, as the author asked: lit in its
   colour when on (the dangers warm, the postures cool), outlined when off, asking when
   the game has not said -- which a screen reader hears too (*"Stunned: yes"*). Each of the
   game's four lists of effects -- Active Spells, Buffs, Debuffs, Cooldowns -- is a widget:
   a bar each, full as the game last said, its time left beside its name (*"Rapid Fire
   1:59"*), or *"None."*. A kind may now hold one of these (`Widget::Indicator`,
   `Widget::Effects`), so the list of kinds is made, not written (`Widget::all`), and the
   indicators have a group of their own in the Add-a-widget list. Hydra's presets gain
   *Indicators* (the nineteen, three a row) and *Effects* (the four as one tab stack, as
   Saga's panel has them). A window with an effect counting down is drawn again each
   second, as the clocks are each quarter of one (`App`, `clocks_run`), and not at all
   while nothing counts.

   Tests: an indicator on, off and never told; effects with and without time left, and
   lists with none; seconds as a clock; the two presets whole; a window's next frame for
   clocks, effects and neither; and an image of indicators and effects
   (`tests/snapshots/status.png`). Nine mutants, all caught.

   **Step 3 BUILT 2026-09-27** (`crates/cena-gui/src/widget/room.rs`). *Compass*: the
   room's ways out as a rose -- the eight directions and out in a three-by-three, up and
   down beside it -- each lit when the room has it. **A click on a lit one goes that way**,
   sent as if typed (a widget may now hand its window a line to send,
   `Asked::Send`); never from a compass following another character, which would move
   this one. *Combat*: who is fighting here, as Saga's panel -- *FRIENDLY*, the character
   with its stance and whatever the game says is on its side, then *FOES*, each with its
   statuses in words and a thin bar of its health where the server states it, dead ones
   said so. A creature the game has not called friendly is a foe. The list is drawn from
   a small view of each creature (`Fighter`), read off the model's; the test draws the
   view, since building the model's creature takes `cena-protocol`'s types, which the GUI
   does not depend on.

   Tests: the compass lit where the room goes, a click on a lit way going and on a dark
   one not, another character's going nowhere, a click in a play window sent as typed;
   friends apart from foes with their statuses, the dead, and none; and an image of both
   (`tests/snapshots/room.png`). Twelve mutants, all caught.

   **Step 4 BUILT 2026-09-27** (`crates/cena-gui/src/story/streams.rs`,
   `widget/draw.rs`). A widget of one of the game's streams, by its id: *Thoughts*
   (`thoughts`), *Speech*, *Arrivals* (`logons`), *Deaths* (`death`), *Announcements*,
   *Familiar*, *Voln* (the Order's own thoughts), and **any other the character has
   received**, offered in the Add-a-widget list under its id with a capital -- so a
   mentor's or a host's stream appears for the characters that hold the position and
   nobody else (§1 row 4), and Hydra never had to be told its id. The story keeps every
   stream's own lines, painted by the triggers as its own are, even one the game drops from
   the story (speech, main's copy); a line the game sends to the story is marked with its
   stream, and **the story leaves it out while a widget of that stream is open** in the
   window, showing or a tab behind another -- the story and the stream never say one line
   twice. A stream is its character's story: it never follows another (§1 row 7). Hydra's
   presets gain *Streams*, one tab stack of thoughts, speech, arrivals, deaths and
   announcements, each tab counting what came since it showed. A widget kind may now hold
   a name, so it is cloned, not copied, and its name may be made (`Widget::name`).

   Tests: each stream kept, speech too, the story's lines marked; a stream keeping its
   newest 500; a stream's widget showing its lines, counting them, named or capitalised;
   the story giving a thought up to an open Thoughts widget; the list offering a stream
   received (a mentor's); a stream's menu offering no following; and the Streams preset.
   Ten mutants: eight caught at once, two once tests were written for them (a stream
   allowed to follow, and one kept without end). The preset tests moved to
   `layout/preset/tests.rs` as `layout/tests.rs` neared its cap.

   **Step 5 BUILT 2026-09-27** (`crates/cena-gui/src/widget/lists.rs`). *Spellbook*: the
   spells the game lists, each by number and name under its circle, as its Spells window
   does. *Reserve*: what the character keeps in reserve, numbered as Saga's R1-R3, or that
   nothing is, or that the game has not listed it. *Containers*: each container the game has
   shown, a heading with its count that opens to what it holds -- every container rather
   than one chosen, so the widget needs no option and no editor to choose it. Hydra's
   *Loadout* preset gains the prepared spell and the reserve, as Saga's panel has them. Each
   is drawn from a plain list read off the model where it is drawn, and tested so.

   Tests: spells under their circles, none listed; the reserve numbered, empty and unknown;
   containers closed with their counts and opened; each unknown before the game has said.
   Seven mutants, all caught.

   **Step 6 BUILT 2026-09-27, but for Loot.** The model work went first, in `cena-model`
   (`crates/cena-model/src/state/world.rs`): `GameState` keeps what `<pulse>` and
   `<worldEvent>` say, frames it parsed and dropped (`state.rs`, the `_ => {}` arm), read as
   `VellumFE` reads them (`reference/VellumFE/src/core/messages/element.rs:2398`, `:2490`) --
   a pulse bounds when the *next* comes, on the server clock at arrival; an event lapses its
   minutes after it came. A reconnect forgets the pulse, timed from its connection, and keeps
   the world's events; both are in the state's equality, as facts the server stated.
   `cena-session` re-exports the module (`cena_session::world`). Then the widgets: *Pulse
   timer*, a bar filling toward the next pulse (*"Next mana pulse in 23-52s"*, *"Pulse
   due"*), the window drawn each second while it counts; and *World events*, each with its
   realm and time left. What each says is worked out apart from drawing it, so the tests
   hold the clock still.

   **Loot is not built: its question is the author's** (§5 item 7). The same for Saga's
   *Combat Actions*, *Almanac* and *Badge*, whose content is not known here (§3, UNKNOWN).

   Tests: the model's pulse due and overdue and clockless, an event lapsing and pruned and
   bounded, the state keeping both and a reconnect forgetting the pulse; the pulse bar's
   sayings; events with their time left and none; the window drawn each second while a pulse
   counts. An image test from step 2 counted on the wall clock and flaked when a second
   turned; its bar no longer shows time. Twenty mutants; four survived the first run, each a
   test whose input never reached the distinction: an event pruned only long after it lapsed,
   not at its second; no event without a realm checked for none; a plain pulse, so dropping
   the mana flag changed nothing; and both event lists drawn in one place, so *"No world
   events."* moving from one to the other still showed once. All twenty caught.

   **Stage B is BUILT**, but for those four: 66 kinds of widget (36 plain, 7 named streams
   and any other received, 4 effect lists, 19 indicators), and eight presets.

   **Added by the author 2026-09-27: the Game state widget** (*"a GameState panel. Where you
   can see in real time everything in GameState?"* ... *"Talk about troubleshooting!!"*).
   BUILT the same day (`crates/cena-gui/src/widget/state.rs`), in the Hydra group; 67 kinds
   now, 37 of them plain.
   - *Everything, with nothing to keep up to date.* It shows the model as the model prints
     itself (`GameState`'s `Debug`, `crates/cena-model/src/state.rs:131`). No field is left
     off, and a field added to the model appears with no change here.
   - *A tree.* It is closed below the model's 34 top fields. A branch holding one value is
     one line: `id: Some("8213304")`.
   - *A filter* finds any line, whatever its case, and shows the path to it.
   - *Copy* takes the whole print, for a report.
   - *Cost.* MEASURED: the committed fixtures folded together print 592 KB, 15,676 lines, in
     6 ms in a debug build. So it is printed at most twice a second, not every frame.
   - *Mutants:* six, all caught.

### Stage C — the launcher tab

The hub's third tab, *"incorporate the things lich's launcher can do"* (`plan/47` §1 row 6):
saved logins (today's Start list, from the roster), a manual login, accounts, favourites,
game and instance. Lich's launcher is the reference
(`reference/lich-5/lib/common/gui/saved_login_tab.rb`, `manual_login_tab.rb`,
`account_manager_ui.rb`, `favorites_manager.rb`, `game_selection.rb`).

Claude's steps, set when Stage B was done (2026-09-27), from what Lich's launcher does and
what Hydra already keeps -- the roster (`crates/cena/src/roster.rs`: character, account,
game, no secret) and the password ladder (`crates/cena/src/secrets.rs`: the OS keyring,
then the account's environment variable, then a prompt at a terminal):

1. **The requests.** A hub may ask the binary to log a character in by what was typed --
   account, password, game, character, and whether to keep the password -- to forget a
   character from the roster, and to forget an account's kept password. The password
   travels in the process only, from the window to the binary, and is never printed:
   its `Debug` says it is hidden. Despana's page cannot send it; the web's wire has no
   such message. A login proven `Ready` joins the roster, as a terminal login does, and
   its password goes to the keyring **only when the player ticked the box**, never
   otherwise and never on a failed login. The binary tells the window the whole roster,
   each character with whether its account's password is kept.
2. **The Launch tab**, the hub's third: the roster's characters, each started with a click
   when its password is kept, or with the password typed there (and the box); a new login
   typed whole; the accounts whose passwords are kept, each forgotten with a click.
3. **Favourites**: a star on a character, kept in the roster, puts it first in the list,
   as Lich's favourites do.

**Stage C BUILT 2026-09-27, all three steps in one commit**, since the tab is what the
requests are for. *The requests* (`crates/cena-ui/src/hub.rs`): `HubRequest` gains `Login`,
`Forget`, `ForgetPassword` and `Favourite`; a `Login`'s `Password` prints as
`Password(hidden)`, and the window's `RosterCard` carries whether a password is kept, never
one. Despana's wire has no message that makes a `Login`, so a password never crosses a
socket. The binary answers them (`crates/cena/src/play.rs`): a window login is tidied as a
terminal one is (`ask::from_window`) and marked `secrets::Source::Window { keep }`; once it
is proven `Ready` it joins the roster, and its password goes to the keyring
(`secrets::keep`) only when `keep`. The table re-offers the roster when a login changes it
with no request to answer (a `Notify` that `Proven::on_ready` and the terminal's yes both
ping). Forgetting a password forgets the keyring's (`secrets::forget`); one in the account's
environment variable is the player's own and stays, and the answer says so. A character is
named to the binary as `GAME:Name`, which the roster reads as that one character whatever
other game has one of the same name.

*The Launch tab* (`crates/cena-gui/src/launch.rs`), the hub's third: the roster, each
character with its game by name and its account; *Start* when its password is kept (the
binary's offered list, so the Live tab no longer starts characters), *Playing* when it
plays, and otherwise a masked password field, a *Keep* box and *Log in*; *Forget* on each.
Below, a new login typed whole -- account, password, character, and the game from a list --
and the accounts whose passwords are kept, each forgotten with a click. A password typed
leaves the tab once asked for; the rest of a new login stays for a second try. The games
are Lich's launcher's (`game_selection.rb:12`), kept beside `DEFAULT_GAME_CODE` in
`cena-platform`'s game namespace (Rule 3.4) and re-exported; the list starts on that
default. *Favourites*: a star on each character, kept in the roster (`favourite`, absent from
an older file), lists it first; a later login keeps the star it does not know of.

Tests: a password never printed, and masked to a screen reader; the roster starred first
then by name, each row's three states; each request by its roster name; a new login refused
until whole, its game chosen, its names trimmed; the roster's star outliving a login and
forgetting one of two same-named characters; a window login tidied and kept only when
ticked; the default game one the launcher offers; the tab rendered. Twenty-four mutants, all
caught, after four holes were closed on the way to writing them: no closed character on the
roster, so one counted as playing passed; no character offered by `GAME:Name`; no account
without a kept password, to be left off the list; and the binary's cards built inline in
`offer()`, where nothing could reach them (now `roster::Entry::card`). Typing in a test
reaches a field only once it is focused, as a player's does.

**Not tested here, and why.** The binary's answers to the four new requests run on the
session table, which needs a live connector; `secrets::keep` and `secrets::forget` write the
real OS keyring; and the re-offer after a login proven `Ready` needs a login. Each is a few
lines over tested pieces, and each is the author's to see live: log a character in from
Launch with the box ticked, see it join the roster with its password kept, then forget it.
`cena-session`'s facade sat at its 120-line cap (`file_rules.rs:337`); the one line the
games' re-export needed was found by joining two re-exports of one module.

**REVISED BY THE AUTHOR 2026-09-27, after running it.** Quoted:

> *"the panels shouldn't stretch across the whole screen like this, they should probably be
> the width of the 4 bars there by default no auto stretch, then manually adjustible by
> dragging a side. They should tile or grid on the panel depending on window size."*
>
> *"I prefer not launched to come back and that's where your characters are listed that are
> saved and well, haven't been launched. You can favorite a card and that pushes them to the
> top of the grid."*
>
> *"then new login will have the spot to put in your login credentials, minus character.
> Game needs to default to Prime and not test. So when they enter their credentials and hit
> login it will bring up a list of the characters they have on that account. There will be
> buttons beside each character. Add/Forget  Favorite  Play. Adding will just add them to the
> not launched panel as saved characters. There will be a log out button next to the Log in
> button that terminates the connection."*
>
> *"also get red flashes like this when the login window changes, clicking on shutdown,
> launching the first character."*

So Stage C gains four steps:

4. **No red flashes.** Debug builds box in red any widget whose id changed while its place
   did not. That is egui's `warn_if_rect_changes_id`, on under `debug_assertions`. A child
   area's ids come from its parent's running count, even when it has a name of its own. So
   a line appearing above (the answer, the shut-down question, a new card) renumbers
   everything below. Each region and each card gets an id of its own.
5. **Cards in a grid.** A card is as wide as its four bars by default and does not stretch.
   Dragging a side sets the width for every card. Cards tile across the tab, as many to a
   row as fit.
6. **Not launched**, the third tab, replacing Launch. It holds the roster's characters that
   are not on the table, as cards in the same grid, starred ones first. Below them are the
   new login and the kept passwords.

   > **CORRECTED by the author after running it, 2026-09-27:** *"There's suppose to be 4
   > tabs... not launched and whatever we had before, login? are separate tabs."* The new
   > login, its list of the account's characters, and the kept passwords are a fourth tab,
   > *New login*; *Not launched* holds only the cards. Claude had read *"then new login
   > will have the spot..."* as a part of the Not launched tab. The fourth tab's name is the
   > author's own phrase for it, and can change.
7. **An account's characters.** The new login asks for the account and password, not a
   character, with the game defaulting to Prime. *Log in* asks the login service for that
   account's characters on that game: `K A M F G P C`, stopping before `L`
   (`crates/cena-platform/src/eaccess/handshake.rs:170`). Each character is listed with
   *Add* (or *Forget* once added), a star, and *Play*. *Add* puts it on the roster without
   playing. *Log out* forgets the account's password in the window and the list. A kept
   password is saved at *Log in*, since the service has then proved it. Claude's call, for
   the author to confirm: the service's connection closes once the list arrives, because
   *Play* logs in afresh, as a reconnect must. So *Log out* ends the window's hold on the
   password, not a socket.

Superseded by the revision: the paragraph below, which excluded the listing, and step 2's
Launch tab name.

**Steps 4 and 5 BUILT 2026-09-27** (`57f37db`, `crates/cena-gui/src/hub.rs`).

- *No red flashes.* Everything below the hub's header has an id of its own, so a line
  appearing above no longer renumbers the streams panel under it.
- *The test hears egui's own warning* (`log`, by the thread that drew it). Before the fix
  it failed on exactly the four boxes of the author's screenshot. Its logger is boxed and
  handed to `log`, not a `static` of ours (Rule 5.2).
- *Cards in a grid.* A card is as wide as its four bars (312), and dragging its side sets
  every card's width, no narrower than 160. Cards tile as many to a row as fit. The rows
  are counted rather than left to egui's wrapping, which cannot wrap what it has not yet
  measured.
- *Mutants:* five, four caught. A card's own id survives, because egui's check also needs
  a widget's parent unchanged and each card is its own parent. The id is kept so a card's
  widgets keep their state as others come and go.
- *Not persisted yet.* The card width lasts the run; keeping it is a Stage D setting.
  *(Kept since Stage D step 2, `window.toml`.)*

**Steps 6 and 7 BUILT 2026-09-27.**

- *Not launched* (`crates/cena-gui/src/launch.rs`) holds the roster's characters that are
  not on the table, live or closed. They are cards in the same grid, starred first, then by
  name, and counted on the tab.
- *Log in by account.* It asks for the account and password, with the game on Prime. The
  login service lists the account's characters (`cena_platform::list_characters`:
  `K A M F G P C`, no `L`, over the same scripted server the handshake's tests use). The
  binary hands the window a `Listing`.
- *Each listed character* has *Add* (*Forget* once on the roster), a star (which adds it
  starred if it is not yet on the roster), and *Play*, which logs in with the password
  held since *Log in*. One on the table says *Playing*. *Log out* lets go of the account,
  its list and its password.
- *Keeping the password.* It goes to the keyring at *Log in* when *Keep the password* is
  ticked, once the service has proved it.
- *Where the answers live.* The binary's answers to the roster and password requests moved
  out of `play.rs`, which sat at its cap, into `crates/cena/src/launcher.rs`.
- *Mutants:* fifteen, across the tab, the binary's answers, the parser of `C` and the
  listing's conversation. All fifteen are caught, after one hole was closed while writing
  them: no test ticked the account login's *Keep the password*, so a login that ignored
  it would have passed.
- *Not tested here:* the listing against the live service, the keyring, and the binary's
  answer on the live table. Each is the author's to see.

Not in Stage C: asking the login service which characters an account has, as Lich's
manual tab does (`manual_login_tab.rb`). It needs a new exchange with the live service,
which no test here may make; the character is typed by name, and a wrong one is refused at
login with the service's own reason.

### Stage D — settings

First the inventory of every setting, with the author (`plan/28` §9 item 3); then editors
rendered from a schema, not written by hand (`plan/28` §7f, §7g); and one front door in the
hub for help and settings. The user quality-of-life review of 2026-09-26
(`44-user-qol-review.md`, not yet committed) names the gaps this closes: no screen to set up
healing or edit a hunt profile (its Q01, Q03), no front door (Q04).

**The inventory is taken** (2026-09-27): [`50-settings-inventory.md`](50-settings-inventory.md),
PROPOSED, every setting Hydra has measured and cited, what VellumFE has that Hydra lacks, a
proposed shape, and ten questions for the author (§5). No editor is written before they are
answered. Two defects it found are fixed: a character named Presets would have written its
layout over the preset library (`094b482`), and the loot profile's learned `unskinnable` list
was not written atomically (`74a76e1`).

**BUILT 2026-09-27**, in the eight steps the author's *"stage d!"* set going, each with its
own record in `plan/50` §7:
1. one settings menu, over pages the binary builds from each behavior's key table;
2. Hydra's own *Window* and *Keys* pages;
3. the character's *General*, *Player log* and *Recording* pages, with `--record` retired;
4. *Travel*, typed, with the map's own settings;
5. the loot profile as *Loot*, *Skinning* and *Selling*;
6. a page per hunt profile, every setting shown with where it came from;
7. layouts by game and name, and a fixed data folder;
8. a widget's right-click opening the menu at its page.

The pages are not a schema-rendered editor in the sense §7f meant. They are each behavior's
own key table, drawn by one generic menu, and a hunt profile's own table read whole.

### The author's notes after running Stage D, 2026-09-28

Each BUILT the same day, each with its tests:

1. **Arrange is on the top bar.** *New custom window* turns it on and the switch was inside
   the Layout menu, so the author was left in it with no sign why. It is a switch on the
   play window's top bar now, lit while it is on (`74d4986`).
2. **The grid shows only once a window moves.** *"clicking on a window brings up the grid.
   Grid should only show once a drag of a window has started either to relocate or
   resize."* A press alone shows nothing. The grid and guides come once a pressed window
   has moved or changed its size, and go when it is let go
   (`crates/cena-gui/src/play/holders.rs`, `guiding`).
3. **A window lock.** *"There also probably needs to be a window lock you can toggle to
   prevent dragging them windows."* *Lock* on the top bar keeps every window where it is:
   none is let go by a press, none is movable or resizable, and Arrange cannot be turned
   on. It is kept with the character's layout (`Layout::locked`). The layout's keeping
   moved into `crates/cena-gui/src/layout/kept.rs` to make room.
4. **Links, clickable, with their menus.** Relayed by the M7b session, in the author's
   words: *"we need to implement links using the preset highlights, clickable with their
   menus popping up."* The author, on the cost to Despana's wire: *"if despana quits
   working thats ok ... Don't let despana limit you."* Built as `VellumFE` does it
   (`frontend/gui/app.rs`, `resolve_link_dispatch`; `core/app_core/state/menus.rs`):
   - A run keeps its link through `cena_ui::painted` (`StyledRun::link`, a `RunLink`),
     which dropped it before, and a link run is never merged into the text beside it.
     Added within wire version 1, as `color` was; Despana ignores it (`a2e4a35`).
   - The story and a stream draw a link in `VellumFE`'s link colour, `#477ab3`, unless a
     trigger or a preset coloured it or it is bold (a creature keeps its own). The
     pointer shows a hand over one (`crates/cena-gui/src/text.rs`, `linked`).
   - A command link (`<d>`) sends its command as typed. An object link whose `coord=`
     names a command sends that. Any other object link asks for the object's menu,
     `_menu #<id> <n>`, sent without an echo. The menu the game answers with `id=<n>`
     pops up where the click was, labelled for the object by the model's dictionary
     (`cena_ui::object_menu`), a category a level down under its name, closed. An entry
     chosen sends its command; Escape or a press elsewhere closes it; an answer to
     another request is not shown (`crates/cena-gui/src/play/links.rs`).
   - A web address opens in the browser. Not tested: egui's opening it.
   - **The model's menu was wrong, and a live menu shows it.** `@` was filled with the
     `<mi noun=>`. The wiki says `@` is the object clicked
     (`reference/wiki_clean/Wrayth protocol.txt:28`), and the author's own menu sends
     `ask @ about %` with `noun="amplify"`: the item's noun is `%`. Fixed in `cce1b98`.
   - The dictionary's 45 `_dialog` entries open dialogs of Wrayth's own, and are left
     out, as `VellumFE` leaves them out.

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
- ~~A bar's options saved~~ **BUILT 2026-09-28**: a bar widget's look (fill direction, text
  place and words, colour, overlay) on its own page in the settings menu, saved with the
  layout (`plan/50` §7 step 8, as the author corrected it).
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
7. **Loot** (Stage B step 6): the model empties its loot queue each prompt into the ledger
   (`plan/34`), so no standing list is kept. Should a Loot widget show the last things
   looted this run (the model keeping, say, the last fifty), or the ledger's totals, which
   the binary would hand the window? And what do Saga's *Combat Actions*, *Almanac* and
   *Badge* show? Asked 2026-09-27; not built until answered.

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
