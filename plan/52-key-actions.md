# 52 — Keys bound to actions

**PROPOSED 2026-09-28.** The author: *"Keys bound to actions. It unblocks Find, scrolling,
tab switching, resend, targeting and most of the input rows. I think this is the next
thing we tackle. We need to come up with a list of actions that we should be able to bind
(we can use vellum for inspiration), then we need to come up with logical defaults for
those keybinds. Leaving them fully customizable of course."*

This file is the list and the defaults, measured against what players already press, with
the questions that are the author's (§6). The first seven were **ANSWERED the same day**
(§6), which moved Tab to targeting, left Hydra's own actions without default keys, and
added macro sets per character. Three followed from them, answered too (§6, 8-10): set 0
always active and holding the defaults, Alt and a digit choosing a set, and `stop` left
for later. **APPROVED to build; nothing is built.**

---

## 0. What there is today

- A key sends **one line**, as if typed (`crates/cena-gui/src/keys.rs`, `take`), read from
  `keybinds.toml`'s `[keys]` table and changed on the *Keys* settings page. A line with a
  newline is refused: one command per line (`crates/cena-ui/src/input.rs:83`, the crate
  review's R10).
- **No key is bound out of the box.** With no file there are no bindings, so the numpad
  types digits until a player writes a file (`keys.rs`, `load`).
- A key that types (a letter, digit or mark with no Ctrl, Alt or Cmd) is refused, or it
  could never be typed.
- The command input's up and down walk the history, wired into the input and not
  rebindable (`crates/cena-gui/src/play.rs`, `walk`).
- **The text box already edits.** egui's own text box moves by word, deletes a word
  (Ctrl+W), clears to the start or end (Ctrl+U, Ctrl+K), undoes and redoes, and handles
  the clipboard (the fork's `widgets/text_edit/builder.rs:1345-1360`). None of that needs
  an action, and none is listed below.

## 1. What players already press

Four sources, measured by two surveys on 2026-09-28:

| Source | What it is | Weight |
|---|---|---|
| **Wrayth's stock key set** | set 0, *(default)*, in two of the author's own Wrayth exports: `Mnstr.xml` line 19, 63 keys, 46 of them `{Action}` tokens; `NewLayoutWrayth.xml` line 233, 76 keys, the same 46 tokens (CORRECTED 2026-09-29: this read *"66 keys, 49 of them"* and *"identical"*; 66 is `Mnstr.xml`'s two sets together. MEASURED: `sed -n 19p Mnstr.xml \| grep -o "<k " \| wc -l` and `grep -o "action='{"`, and line 233 of the other with `action="{"`) | **first**: the official client; what most players' fingers know |
| Wizard, the older official client | its built-in keys (`reference/wiki_clean/Wizard _front end_.txt:254-293`) | where Wrayth kept them |
| VellumFE | the author's client: its keyboard actions and their defaults (`reference/VellumFE/src/config/keybinds.rs:419-853`, `defaults/globals/keybinds.toml`) | the names, and the lessons |
| Genie 5 | DragonRealms' client (`reference/Genie5/src/Genie.App/Views/MainWindow.axaml.cs:621-869`) | a tiebreak |

What they agree on (every source that has the key):

- **The numpad walks**: 8 2 4 6 7 9 1 3 the eight directions, 5 `out`, 0 `down`, `.` `up`.
  `+` is `look`; `-` `*` `/` are `info`, `exp`, `health` in Wrayth and Vellum.
- **Up and down** walk the history. **Ctrl+Enter** sends the last command again,
  **Alt+Enter** the one before; **numpad Enter** sends the line, or the last command when
  the line is empty (Wrayth, Wizard).
- **Page Up and Page Down** scroll a page (Wrayth, Vellum, Genie; Wizard alone scrolls a
  line). **Ctrl+Home and Ctrl+End** go to the top and bottom (Wrayth, Genie).
- **Ctrl+F** finds (Vellum, Genie). **F1 to F12 are the player's**: Wrayth binds none in
  its stock set, and the author's own sets fill them with spells and stances.

Where they disagree, and what this file chooses:

| Key | Wrayth | Vellum | Chosen |
|---|---|---|---|
| Shift+PgUp/PgDn | a line | (Alt+PgUp/PgDn a line) | **Wrayth's** |
| Ctrl+PgUp/PgDn | top and bottom | the next and last Find match | **Wrayth's**; Find keeps F3 |
| Ctrl+R | rest mode (anti-idle) | resend the last command | **neither by default**: Ctrl+Enter resends, as Wrayth does |
| Tab / Shift+Tab | cycle windows | cycle windows | **the next and last target** (the author, §6 answer 3) |
| Esc | not bound; Wizard clears the line; Genie stops scripts | clears Find | closes what is open, else clears the line |
| Shift+Esc | pause the script | — | **nothing**: Hydra's own actions get no default key (the author, §6) |

**Vellum's lessons, kept:** one table of actions is the only source of truth (Vellum has
two default sources that disagree: its code's fallback binds Ctrl+K and its shipped file
does not); an action it cannot perform is refused with a reason, not silently dropped
(`keybinds.rs:24-83`); Ctrl+C, X and V are refused for anything else, since the clipboard
arrives beneath the keys.

## 2. The shape

- **A macro is a key bound to one of three things** (the author, §6 answer 1: *"a macro
  is a keybind to send one or more commands ... send to game, fill command input, or
  perform hydra action"*): **send** one or more commands to the game, **fill** the command
  input without sending, or **perform** a Hydra action. Wrayth has all three: `\r` sends,
  a macro without it types (`Mnstr.xml`'s `prep 111`), and `{Action}` acts.
- **Several commands, split as `VellumFE` splits them** (`reference/VellumFE/src/core/app_core/commands.rs:180-238`):
  a send macro's text is cut at each `\r` into commands, each sent in order and each one
  command, so R10's rule holds per command. Empty pieces are dropped. A piece that is
  exactly `s` and a number of seconds (`s1.5`) waits that long before the rest, and waits
  add up; nothing else in a piece is special. Unlike Vellum, a trailing `\r` means nothing:
  the macro's kind, not its last character, says whether it is sent (Vellum's own book
  and code disagree on this, `reference/VellumFE/book/src/customization/keybinds.md:20-24`
  against `src/config/keybinds.rs:131-134`).
- **Written in TOML**, where `\r` in a quoted string is the character itself:

  ```toml
  [keys]
  F1 = "incant 610"                              # send: one command
  F2 = "stance off\rraise bow\rweapon barrage"   # send: three, in order
  F9 = "prep 118\rs1\rcast"                      # send, waiting a second
  F3 = { fill = "prep 111 " }                    # into the input, not sent
  "Ctrl+F" = { action = "find" }                 # a Hydra action
  NumpadAdd = ""                                 # a default unbound
  ```

- **Defaults live in the code** (§6 answer 2), and the files hold only what the player
  added or changed. The *Keys* page shows every key in effect and where it came from, as
  `plan/50` asked of every setting (*"the value in effect and where it came from"*). An
  empty string unbinds a default. New defaults in a later Hydra reach every player who
  never changed that key.
- **Macros are the character's, with a switch to make one global** (§6 answer 7: *"The
  macro is character by default with a global toggle"*). A character's own are kept in
  its own keys file beside its settings; the global ones in `keybinds.toml`, as today.
- **Ten macro sets, 0 to 9, as Wrayth has, and set 0 is always active** (§6 answer 8).
  **Hydra's defaults are set 0's**: every default in §3 is a macro in set 0, which the
  player changes like any other. One of sets 1 to 9 may be chosen, per character, and
  kept; Alt+1 to Alt+9 choose one and Alt+0 goes back to set 0 alone (§6 answer 10). For a
  key press, the first that binds the key wins:

  1. the chosen set, if one is: the character's own macro, then a global one;
  2. set 0: the character's own, then a global one, then Hydra's default.

  The author's example: *"if set 0 has F2 = stance offensive, F4 = stance defensive, and
  set 1 only has F4 = loot, then when you activate set 1 and hit F4 you will loot, but if
  you hit F2 you still stance offensive."*
- **An action acts on the play window that has the keyboard.** Scrolling, Find and tabs
  act on **the window in use**: the window last clicked in the play window, the story
  until one is (§6 answer 3: *"click only for choosing the window"*). It is marked, so the
  player sees what a key will act on. `next_window` and `previous_window` exist for a
  player who wants Wrayth's Tab back, with no default key.

## 3. The actions

Each with its proposed default. **Bold** defaults are what Wrayth ships; the others follow
Vellum, a common convention, or nothing.

### Sending

| Action | Does | Default |
|---|---|---|
| `send_or_repeat` | send the line; on an empty line, the last command again (Wrayth's `{ReturnOrRepeatLast}`) | **NumpadEnter** |
| `repeat_last` | send the last command again | **Ctrl+Enter**, **Ctrl+NumpadEnter** |
| `repeat_second_last` | the one before it | **Alt+Enter**, **Alt+NumpadEnter** |
| `history_back` / `history_forward` | walk what was sent | **Up** / **Down** |
| `clear_input` | empty the input | Esc, when nothing is open to close |

Enter itself sends and is not rebindable: Vellum's TUI ignores a rebound Enter
(`reference/VellumFE/src/frontend/tui/input_handlers.rs:196`), and a key that cannot be
bound should not look bindable.

### Scrolling (the window in use)

| Action | Default |
|---|---|
| `scroll_page_up` / `scroll_page_down` | **PageUp** / **PageDown** |
| `scroll_line_up` / `scroll_line_down` | **Shift+PageUp** / **Shift+PageDown** |
| `scroll_top` / `scroll_bottom` | **Ctrl+Home**, **Ctrl+PageUp** / **Ctrl+End**, **Ctrl+PageDown** |

### Find (new: a Find bar over the window in use)

| Action | Default |
|---|---|
| `find` | Ctrl+F |
| `find_next` / `find_previous` | F3 / Shift+F3, and Enter / Shift+Enter in the bar |
| (closing it) | Esc |

### Tabs and windows

| Action | Does | Default |
|---|---|---|
| `next_window` / `previous_window` | make the next window the one in use | none: a click chooses it (§6 answer 3) |
| `next_tab` / `previous_tab` | the next tab of the window in use | Ctrl+Tab / Ctrl+Shift+Tab |
| `next_unread_tab` | the first tab, anywhere in the play window, with lines unread | none (§6 answer 5) |

### Targeting

The game says what may be attacked: the `dDBTarget` list, already read into the model
(`crates/cena-model/src/state/targeting.rs`).

| Action | Does | Default |
|---|---|---|
| `target_next` / `target_previous` | `target #<id>` of the next or last creature in the game's list, wrapping | **Tab** / **Shift+Tab** (the author, §6 answer 3) |
| `target_clear` | `target clear` | none; a macro does it |

Vellum orders by where each creature is drawn on its creature field. Hydra has no field
yet (`plan/49` Stage G), so the game's order is the one used (§6 answer 6).

### Hydra

No default keys (the author: *"don't give default binds to those hydra actions"*).

| Action | Does |
|---|---|
| `stop` | what the top bar's Stop does, `;stop`: the running behaviors stop (the hunt with its errands, travel, `;foreach` and `;multi`, each registered in `crates/cena/src/commands.rs`, `stops`). Not a script: left for later (§6 item 9) |
| `drawer_top` / `_bottom` / `_left` / `_right` | open or shut that drawer (`plan/49` Stage E) |
| `character_1` … `character_9` | the play window of the hub's first to ninth character |
| `settings` | the settings menu |
| `lock` / `arrange` | the top bar's two switches |
| `macro_set_0` … `macro_set_9` | choose that set for this character; `macro_set_0` chooses none, leaving set 0 alone. **The one exception to no default: Alt+0 to Alt+9**, as Wrayth has them (§6 answer 10) |

### Lines bound by default (not actions)

| Key | Sends |
|---|---|
| **Numpad8 2 6 4 9 7 3 1** | `north` `south` `east` `west` `northeast` `northwest` `southeast` `southwest` |
| **Numpad5** / **Numpad0** / **NumpadDecimal** | `out` / `down` / `up` |
| **NumpadAdd** / **NumpadSubtract** / **NumpadMultiply** / **NumpadDivide** | `look` / `info` / `exp` / `health` |

**Shift and the numpad peers** (the author, 2026-09-28: *"shift + numpad = peer
direction"*), the game's `PEER` for each key that walks:

| Key | Sends |
|---|---|
| Shift+Numpad8 2 6 4 9 7 3 1 | `peer north` `peer south` `peer east` `peer west` `peer northeast` `peer northwest` `peer southeast` `peer southwest` |
| Shift+Numpad5 / Shift+Numpad0 / Shift+NumpadDecimal | `peer out` / `peer down` / `peer up` |

To check when built: on Windows, Shift with a numpad key while NumLock is on reaches a
program as that key with NumLock off, so Vellum cannot tell the two apart
(`reference/VellumFE/defaults/globals/keybinds.toml:158-161`). Hydra reads NumLock from
each press through the fork's hook (`plan/47` step 7); whether Shift survives there is
to be measured, not assumed.

F1 to F12, Ctrl with letters, and every other numpad chord are left to the player.

## 4. Importing a Wrayth key set

`;keys import <Wrayth settings file>` reads its `<macros>`, as `;trigger import` reads
highlights (`plan/45` Stage 4), each Wrayth set into the Hydra set of the same number
(`<keys id name><k key action/>`): a `{Token}` as its action (`{HistoryPrev}` is
`history_back`, `{RepeatLast}` is `repeat_last`, `{PageUp}` is `scroll_page_up`,
`{CycleWindows}` is `next_window`, `{MacroSet}3` is `macro_set_3`...), text ending in
`\r` as a send macro, text without it as a fill, `\p` as `s1`, and `\x` (clear the line
first) dropped, since a sent command never touches the input. A token Hydra has no
action for (`{Rest}`, `{ToggleMusic}`), `@` and `\?` are said, not silently lost. The fixture comes from
`Mnstr.xml` or `NewLayoutWrayth.xml`: the committed Wrayth fixture is cut from
`Nisugi3.xml`, which has no `<macros>` (`crates/cena-behavior/tests/fixtures/wrayth.xml`).
Step 9 cut its own from `Mnstr.xml` (`crates/cena-gui/tests/fixtures/wrayth_macros.xml`).

## 5. Steps, once §6 is answered

1. A macro's three kinds, a send macro split at `\r` with its waits, defaults in code
   with the files on top, and the *Keys* page showing where each key comes from. The
   numpad defaults.
2. Macros per character with the global switch, and the ten sets.
3. Sending: `send_or_repeat`, the repeats, the history as actions, `clear_input`.
4. The window in use, and scrolling.
5. Tabs and windows.
6. Find: the bar, its matches, and its keys.
7. Targeting, on Tab.
8. Hydra's own: stop, drawers, characters, settings. (The sets' keys came with step 2.)
9. `;keys import`.

### Step 1, BUILT 2026-09-28

The author: *"go for it"*. In `crates/cena-gui/src/keys/`:

- **What a key does** (`binding.rs`): `Macro::Send`, `Fill` or `Act`. A send macro is cut
  at each `\r` (a line end too, which a TOML multi-line string holds) into commands, each
  checked as one line, empty pieces dropped; `s` and a number of seconds is a wait, up to
  a minute, so a slip (`s600`) cannot hold a character's commands. Nothing sends unless a
  command is in it. `s` alone is `south`, not a wait.
- **Actions arrive with their steps.** Step 1 has two, `stop` and `settings`, what the top
  bar's buttons ask; the rest are named in the step that makes them do something, so the
  file never takes an action that does nothing.
- **Hydra's defaults in the code** (`defaults.rs`): the numpad walks, Shift with it
  peers, and `+ - * /` are `look`, `info`, `exp`, `health`, 26 keys in all. The file holds
  only changes (`keys.rs`, `Keybinds`), `""` unbinding a default; the writer writes `""`
  when a default is removed or moved away, and deletes the player's line when it is
  restored (`write.rs`).
- **The *Keys* page** (`page.rs`) lists every key in effect, Hydra's among them: its kind,
  what it does, where that came from in a word (*yours*, *Hydra's*, *changed*,
  *unbound*, Hydra's default when the pointer rests on it), and *Restore* and *Remove*.
  A command break is typed `\r`, as Wrayth's and `VellumFE`'s players write it. *Add a
  key* moved above the list, which Hydra's own keys make long.
- **Done by the app** (`crates/cena-gui/src/app/keyed.rs`): commands sent on the window's
  character as if typed, those after a wait kept and sent when due, a frame asked for
  then; a fill puts its text in the command input with the cursor at the end
  (`Play::fill`); an action asks what the window's button asks.
- **To measure live**: whether Shift with a numpad key reaches Hydra as Shift on Windows
  with NumLock on (§3); nothing here can.
- *Tests:* the macro's cutting and kinds, the defaults all ones the file would take, the
  file over the defaults, the writer's `""` and restore, the waits, a fill and an action
  pressed in the app, the page's *Restore* and *Remove*. Eight mutants, all caught.

### Step 2, BUILT 2026-09-29

The author: *"step 2 let's go"*. In `crates/cena-gui/src/keys/`:

- **Two files, ten sets each** (`file.rs`): every character's keybinds file, and a
  character's own, `<instance>_<name>.keys.toml` beside its settings. Set 0 is `[keys]`,
  as before, so step 1's files read unchanged; sets 1 to 9 are `[set1]` to `[set9]`.
  The character's file keeps the set it uses as `set = 3` above its tables; every
  character's keeps `numpad`. Each says what the other holds that it does not.
- **A key press looks in order** (`keys.rs`, `Keys`): the chosen set, the character's
  own and then every character's; then set 0 the same way; then Hydra's defaults. The
  first that binds or unbinds the key has it, so `""` in a character's file unbinds every
  character's key or Hydra's for that character alone, and in a chosen set leaves the
  key doing nothing while the set is in use. The author's example is a test
  (`a_chosen_set_goes_over_set_0`).
- **Alt+0 to Alt+9 choose a set** (`defaults.rs`, `Action::Set`, `macro_set_0` to
  `macro_set_9`), the one action with a key of Hydra's (§6 answer 10), kept in the
  character's file (`KeyChange::Choose`) and shown on the play window's bar (*Set 3*).
  Moved here from step 8: sets that could not be chosen would do nothing.
- **The Keys page** (`page.rs`): on Hydra's own settings, every character's keys; on a
  character's (its *Keys* in the page list, and the play window's *Change the keys...*),
  its own over every character's. Either shows one set at a time (*Keys of*); a
  character's also chooses the set in use. A key added on a character's page is its own
  unless *global* is ticked (the author: *"character by default with a global
  toggle"*), and each key from a file has *global* to move it between the two files
  (`KeyChange::Share`, written as a bind in one and a line taken out of the other).
  *Restore* takes a key's line out of its file, so it does what lies beneath again:
  every character's, or Hydra's.
- **The writer** (`write.rs`) writes a set's table, making it when there is none, and
  writes `""` only in set 0 and only where something beneath the file binds the key; a
  key taken out of a set from 1 to 9 falls to set 0's. A setting above the tables written
  and taken out again leaves the file as it was, so Alt+1, Alt+0 does not pile up blank
  lines.
- **The fork catches the keys of the window with the keyboard** (`app/keyed.rs`): the
  play window that last had it, its character's own among them.
- *Tests:* each set read from its table, the layering and the author's example, the
  writer's sets, a character's `""` and its chosen set, Alt+1 and Alt+0 in a play window,
  a key shared and taken back, the character's page (`tests/settings_keys.rs`, with an
  image). Nineteen mutants: eighteen caught; the one left, writing `set = n` unchecked on
  read-back, is equivalent, since a line above the tables cannot fail to read back.

### Step 3, BUILT 2026-09-29

The author: *"step 3"*. §3's sending table, with Wrayth's keys:

- **Six actions** (`crates/cena-gui/src/keys/binding.rs`): `send_or_repeat`
  (NumpadEnter), `repeat_last` (Ctrl+Enter, Ctrl+NumpadEnter), `repeat_second_last`
  (Alt+Enter, Alt+NumpadEnter), `history_back` and `history_forward` (Up, Down), and
  `clear_input` (Escape), done by the play window on its command input and history
  (`crates/cena-gui/src/play.rs`, `Play::act`). The history is what was typed, as
  `VellumFE`'s is (`reference/VellumFE/src/frontend/gui/app/command_input.rs:46`): a
  repeat is not typed and changes nothing in it, so Alt+Enter sends the same command
  each time until another is typed. Up and Down are no longer wired into the input; a
  player who binds them elsewhere has them elsewhere.
- **Enter alone is bound to nothing** (§3: *"a key that cannot be bound should not look
  bindable"*): the file, the writer and the Keys page refuse it, as they refuse a key
  that types, and say so (`Chord::refused`). Enter with a modifier binds.
- **An action on the input waits for it** (`Action::on_input`, `Play::typing`): the
  history's and `clear_input` act while the command input has the keyboard, or nothing
  does, with nothing open -- the object's menu, a widget's, the list of widgets to add,
  a drop-down, a menu of the bar. Otherwise the key is left to what has it, so Escape
  closes a menu and keeps the line (§3's *"when nothing is open to close"*). A key the
  fork caught waits the same way. The sending ones act whatever has the keyboard.
- **No play window with the keyboard, no keys caught** (`app/keyed.rs`,
  `keys_to_catch`): the fork catches a bound numpad key wherever the keyboard is, so a
  default `NumpadEnter` would have stopped the hub's login form taking one. While the hub,
  the settings or another program has the keyboard, the fork is told to catch none of
  the play windows' keys (Clear, on a Mac, still).
- *Tests:* the repeats, `NumpadEnter` through the fork sending or repeating, Up and
  Escape in a play window, Escape and a caught key left to an open menu, another field
  with the keyboard, Enter alone refused, the fork's keys with and without a window
  (`crates/cena-gui/src/app/tests/sending.rs`). Thirteen mutants, all caught.

### Step 4, BUILT 2026-09-29

The author: *"do steps 4-10"*. The window in use, and §3's scrolling table:

- **The window in use** (`crates/cena-gui/src/play.rs`, `Play::in_use`) is the widget
  last pressed in, by its placed id, standalone, in a custom window's cell or a tab; the
  story until one is, and again once the one pressed is taken away. It is marked with a
  faint amber edge (`play/draw.rs`, `shown`), so the player sees what a key will act on.
- **Six actions**, with Wrayth's keys: `scroll_page_up` and `_down` (PageUp, PageDown),
  `scroll_line_up` and `_down` (Shift with them), `scroll_top` (Ctrl+Home, Ctrl+PageUp)
  and `scroll_bottom` (Ctrl+End, Ctrl+PageDown). A key asks the widget in use by its id
  and the widget scrolls as it is drawn (`widget/split.rs`, `ask` and `asked`).
- **The story and a stream split as the wheel splits them**: back from the newest line a
  page, a line or to the oldest opens the split with its top there; the keys then move
  the top, never past the newest line; forward to the newest, or `scroll_bottom`, closes
  it, as its button does. A key back is scrolled from inside the pane and at once: an
  offset given a pane is not the player's own scroll to egui, and sticking to the newest
  line took it straight back (`egui/src/containers/scroll_area.rs:1280` in the fork).
- **Any other widget that scrolls** -- the Room, the lists, the Hunt panel -- takes the
  same keys through its one scrolling helper (`widget/draw.rs`, `scrolled`).
- *Tests:* the split by keys, from one pane and two; a list scrolled a page, a line, to
  the bottom and the top; a catalog widget taking its key; the story in use until the
  room is pressed, then the room scrolling and the story not (`play/tests/in_use.rs`).
  Ten mutants, all caught.

### Step 5, BUILT 2026-09-29

§3's tabs and windows:

- **`next_tab` and `previous_tab`** (Ctrl+Tab, Ctrl+Shift+Tab) turn the stack the window
  in use is a tab of, round from the last to the first, and the tab shown is in use
  (`layout/custom.rs`, `Layout::turn_tab`). A widget alone turns to itself.
- **`next_window` and `previous_window`**, with no key (§6 answer 3), make the next
  widget showing the one in use, in the order the windows are drawn, round
  (`Layout::showing`).
- **`next_unread_tab`**, with no key (§6 answer 5), shows the first tab not showing that
  counts lines unread, as drawn, and puts it in use.
- **Tab never moves the keyboard off the command input**: egui moves focus on Tab and
  Shift+Tab before any of Hydra's code sees the key (`egui/src/memory/mod.rs:580` in the
  fork), so the input keeps it (`lock_focus`), and Tab is the keys' alone (step 7's
  targeting).
- *Tests:* a stack turned both ways and in use, back from the first tab of three to the
  last, the window after and before, the tab with lines unread
  (`play/tests/tabs.rs`). Five mutants: four caught, and the fifth, a stack of one not
  turned, was equivalent and its guard removed.

### Step 6, BUILT 2026-09-29

§3's Find:

- **The bar** (`crates/cena-gui/src/play/find.rs`) opens on `find` (Ctrl+F) over the top
  right of the window in use when it is a story or a stream, over the story when it is
  not, with the keyboard; Ctrl+F again takes the keyboard back to it. It says *n of m*,
  or *none*; ⬆ and ⬇ step, × closes, and so does Escape in it.
- **What is found** (`widget/find.rs`): the lines the widget draws -- the game's, and
  the echo when it is shown, never a prompt -- that hold what is typed, whatever its
  case. Each is marked, the current one more. The current one counts from the newest:
  the first found is the latest said, and `find_next` (F3, and Enter in the bar) goes
  back to the one before, `find_previous` (Shift+F3, Shift+Enter) forward.
- **Brought into sight**: when the bar moves to a line, the pane the player scrolls is
  brought to it at once, so a story splits as scrolling back splits it and the newest
  lines stay below. A widget is asked by its id each frame, as a key's scroll is, and
  says how many it found back.
- *Tests:* typed, found whatever its case, the latest first and the story split to it,
  F3 and Shift+F3 stepping and stopping at the latest, Escape closing it
  (`play/tests/in_use.rs`), and the bar as drawn (`tests/snapshots/find.png`). Six
  mutants: five caught; the sixth, the lines counted afresh for each pane, changes only
  which line is marked current in the pane that follows the newest, where the current
  one is not in sight.

### Step 7, BUILT 2026-09-29

§3's targeting, on the author's Tab (*"I prefer tab for targetting"*):

- **`target_next` (Tab) and `target_previous` (Shift+Tab)** send `target #<id>` for the
  creature after, or before, the one the game says is targeted, in the order of its own
  `dDBTarget` list (§6 answer 6; `crates/cena-model/src/state/targeting.rs`), round
  from the last to the first; the first, or the last, when none of them is; nothing
  while the game lists none (`play/keyed.rs`, `target`). The line is sent as if typed,
  and echoed.
- **`target_clear`**, with no key, sends `target clear`.
- **Tab stays on the command input**: the input holds Tab (step 5), and a targeting key
  puts the keyboard back on it, since with nothing holding it egui gives Tab to the next
  widget.
- *Tests:* round the list both ways, none targeted, one gone, none listed, a player's
  negative id; the play window's list as the game last sent it; Tab and Shift+Tab
  leaving the keyboard and the line where they were (`app/tests/sending.rs`). Four
  mutants, all caught.

### Step 8, BUILT 2026-09-29

§3's Hydra actions, none with a key (the author: *"don't give default binds to those
hydra actions"*); `stop` and `settings` came with step 1 and the sets with step 2:

- **`drawer_top`, `_bottom`, `_left` and `_right`** open the play window's drawer, or
  shut it (`plan/49` Stage E), kept with the layout.
- **`lock` and `arrange`** are the top bar's two switches: Lock ends Arrange, and Arrange
  does nothing while locked, as the bar's own do.
- **`character_1` to `character_9`** open the play window of the hub's first to ninth
  character, in the order they were started, and give it the keyboard
  (`app/keyed.rs`, `bring`); one past the last is nothing.
- *Tests:* a drawer opened and shut, Lock and Arrange, the second character's closed
  window brought and the ninth nothing, and every action's name read back as itself
  with a label of its own, fifty of them now. Four mutants, all caught.

### Step 9, BUILT 2026-09-29

§4's import:

- **`;keys import <file>`**, typed in a play window, reads a Wrayth settings file's
  `<macros>` into the character's own keys; `;keys import global <file>` into every
  character's (`crates/cena-gui/src/app/import.rs`). It is the window's, not the
  session's, since the keys are the window's: a character running headless has none.
  `;help` names it.
- **Each Wrayth set into the Hydra set of the same number** (`keys/wrayth.rs`): Wrayth's
  key names made Hydra's (`Keypad 8`, `Page Up`, `Alt-Ctrl-E`, `UP`); a token as its
  action, thirteen of them and `{MacroSet}n`; text ending `\r` sent, text without it
  typed into the input; `\r` inside a break, `\p` a second's wait, `\x` dropped. A token
  Hydra has no action for (`{Rest}`, `{ToggleMusic}`, `{Copy}`...), `@` and `\?`, and a
  key that types or is Enter alone, are said by name, never silently lost.
- **What is already so is left**: a key of set 0 already doing what Wrayth has it do is
  not written again, so importing Wrayth's stock set writes only where it differs. Where
  it differs from a key of Hydra's, the answer names the keys. The stock set's Tab is
  `{CycleWindows}`, so importing it makes Tab choose the next window, not target: the
  author's own Tab (§6 answer 3) is one *Restore* away on the Keys page, and the answer
  says so.
- *Tests:* the author's own set (`crates/cena-gui/tests/fixtures/wrayth_macros.xml`, cut
  from `Mnstr.xml`'s `<macros>`), every one of its 66 keys bound or said; waits, breaks,
  entities and what is refused; `;keys import` in a play window into the character's
  keys and every character's, nothing sent to the game. Five mutants, all caught.

## 6. For the author

The first seven were asked 2026-09-28 and answered the same day, the author's words
quoted.

1. ~~Several commands, one key, and filling the input.~~ **ANSWERED**: *"yes, a macro is a
   keybind to send one or more commands. You can take notes from vellum on sending
   multiple lines and how to split them. Yes a keybind should have 3 modes, send to game,
   fill command input, or perform hydra action."* §2 splits as Vellum does, with its wait.
2. ~~Defaults in code.~~ **ANSWERED**: *"yes defaults can live in code"*.
3. ~~Tab and the window in use.~~ **ANSWERED**: *"click only for choosing the window and
   tab for target next, shift + tab for target previous"*. Also: *"I don't like tab for
   picking the window in use, you can click to pick the window in use or change the
   keybind to tab if that's what you want."*
4. ~~Shift+Esc.~~ **ANSWERED**: *"I guess it could stop the behavior"*, and *"don't give
   default binds to those hydra actions"*: `stop` stops the behaviors, with no key.
5. ~~`next_unread_tab`'s key.~~ **ANSWERED**: none.
6. ~~Target order.~~ **ANSWERED**: the game's list.
7. ~~One set of keys or a character's own.~~ **ANSWERED**: *"macros should be per
   character. You have the default set everyone gets, then they can add their own
   macros. The macro is character by default with a global toggle. Then add in macro sets
   like wrayth has. 0-9. Default macros are always active unless one of the macros in
   your macro set overwrites it."*

Three followed from those answers, asked and answered the same day:

8. ~~Set 0.~~ **ANSWERED**: *"set 0 is one of the 10, and always active. Set 0 has all
   the actions we just discussed, plus the numpad keybinds"*. §2 has the order and the
   author's example; Shift and the numpad peer (§3) came with it.
9. **Scripts and `stop`. LEFT FOR LATER** by the author. `stop` does what `;stop` does,
   which reaches no script (`plan/46`'s runner, `plan/51`'s Lich), until it is revisited.
10. ~~Keys for the sets.~~ **ANSWERED**: *"sure alt + number = set"*.

Nothing is open. Step 1 can start.
