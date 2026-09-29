# 52 — Keys bound to actions

**PROPOSED 2026-09-28.** The author: *"Keys bound to actions. It unblocks Find, scrolling,
tab switching, resend, targeting and most of the input rows. I think this is the next
thing we tackle. We need to come up with a list of actions that we should be able to bind
(we can use vellum for inspiration), then we need to come up with logical defaults for
those keybinds. Leaving them fully customizable of course."*

This file is the list and the defaults, measured against what players already press, with
the questions that are the author's (§6). Nothing here is built.

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
| **Wrayth's stock key set** | set 0, *(default)*, in two of the author's own Wrayth exports (`Mnstr.xml` line 19, identical in `NewLayoutWrayth.xml` line 233): 66 keys, 49 of them `{Action}` tokens | **first**: the official client; what most players' fingers know |
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
| Tab | cycle windows | cycle windows | cycle windows (§6 question 3) |
| Esc | not bound; Wizard clears the line; Genie stops scripts | clears Find | closes what is open, else clears the line |
| Shift+Esc | pause the script | — | **stop** (`;stop`): Wrayth's key for "halt the automation" (§6 question 4) |

**Vellum's lessons, kept:** one table of actions is the only source of truth (Vellum has
two default sources that disagree: its code's fallback binds Ctrl+K and its shipped file
does not); an action it cannot perform is refused with a reason, not silently dropped
(`keybinds.rs:24-83`); Ctrl+C, X and V are refused for anything else, since the clipboard
arrives beneath the keys.

## 2. The shape

- **A key does one of three things**: sends lines, **puts text in the input** without
  sending it, or **performs an action**. Wrayth has all three: `\r` sends, a macro without
  it types (`Mnstr.xml`'s `prep 111`), and `{Action}` acts.
- **Several lines, one key.** The author's own F keys send two and three commands
  (`stance off\r610`, `stance off\rraise bow\rweapon barrage`). R10's rule holds per line:
  a key sends a **list** of lines, each one command, in order.
- **Written in the one file**, so a key is bound once:

  ```toml
  [keys]
  F1 = "incant 610"                           # one line
  F2 = ["stance off", "raise bow", "weapon barrage"]  # lines, in order
  F3 = { type = "prep 111 " }                 # into the input, not sent
  "Ctrl+Enter" = { action = "repeat_last" }   # an action
  "Ctrl+F" = { action = "find" }
  "Shift+Esc" = ""                            # a default unbound
  ```

- **Defaults live in the code, and the file holds only the player's changes.** The
  *Keys* page shows every key in effect and whether it is the default or the player's,
  as `plan/50` asked of every setting (*"the value in effect and where it came from"*).
  An empty string unbinds a default. New defaults in a later Hydra reach players who
  never changed that key.
- **An action acts on the play window that has the keyboard.** Scrolling, Find and tabs
  act on **the window in use**: the last window clicked in the play window, the story until
  one is. It is marked, so the player sees what a key will act on.

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
| `next_window` / `previous_window` | make the next window the one in use | **Tab** / **Shift+Tab** |
| `next_tab` / `previous_tab` | the next tab of the window in use | Ctrl+Tab / Ctrl+Shift+Tab |
| `next_unread_tab` | the first tab, anywhere in the play window, with lines unread | none (§6 question 5); Ctrl+U is the text box's |

### Targeting

The game says what may be attacked: the `dDBTarget` list, already read into the model
(`crates/cena-model/src/state/targeting.rs`).

| Action | Does | Default |
|---|---|---|
| `target_next` / `target_previous` | `target #<id>` of the next or last creature in the game's list, wrapping | Alt+] / Alt+[ (Vellum's, commented out in its file) |
| `target_clear` | `target clear` | none; a line does it |

Vellum orders by where each creature is drawn on its creature field. Hydra has no field
yet (`plan/49` Stage G), so the game's order is the one there is (§6 question 6).

### Hydra

| Action | Does | Default |
|---|---|---|
| `stop` | `;stop`, as the top bar's Stop | **Shift+Esc** (Wrayth's *pause script*) |
| `drawer_top` / `_bottom` / `_left` / `_right` | open or shut that drawer (`plan/49` Stage E) | Ctrl+Alt+Up / Down / Left / Right |
| `character_1` … `character_9` | the play window of the hub's first to ninth character | Ctrl+1 … Ctrl+9 |
| `settings` | the settings menu | Ctrl+, |
| `lock` / `arrange` | the top bar's two switches | none |

### Lines bound by default (not actions)

| Key | Sends |
|---|---|
| **Numpad8 2 6 4 9 7 3 1** | `north` `south` `east` `west` `northeast` `northwest` `southeast` `southwest` |
| **Numpad5** / **Numpad0** / **NumpadDecimal** | `out` / `down` / `up` |
| **NumpadAdd** / **NumpadSubtract** / **NumpadMultiply** / **NumpadDivide** | `look` / `info` / `exp` / `health` |

F1 to F12, Alt and Ctrl with letters, and every other numpad chord are left to the player.

## 4. Importing a Wrayth key set

`;keys import <Wrayth settings file>` reads a `<macros>` set (`<keys id name><k key
action/>`), as `;trigger import` reads highlights (`plan/45` Stage 4): a `{Token}` as its
action (`{HistoryPrev}` is `history_back`, `{RepeatLast}` is `repeat_last`, `{PageUp}` is
`scroll_page_up`, `{CycleWindows}` is `next_window`...), text ending in `\r` as lines split
on `\r`, text without it as `type`, and `\x` (clear the line first) dropped, since a sent
line never touches the input. A token Hydra has no action for (`{MacroSet}`, `{Rest}`,
`{ToggleMusic}`), `@` and `\?` are said, not silently lost. The fixture comes from
`Mnstr.xml` or `NewLayoutWrayth.xml`: the committed Wrayth fixture is cut from
`Nisugi3.xml`, which has no `<macros>` (`crates/cena-behavior/tests/fixtures/wrayth.xml`).

## 5. Steps, once §6 is answered

1. The binding's three kinds, lists of lines, defaults in code with the file on top, and
   the *Keys* page showing where each key comes from. The numpad defaults.
2. Sending: `send_or_repeat`, the repeats, the history as actions, `clear_input`.
3. The window in use, and scrolling.
4. Tabs and windows.
5. Find: the bar, its matches, and its keys.
6. Targeting.
7. Hydra's own: stop, drawers, characters, settings.
8. `;keys import`.

## 6. Open, for the author

1. **Several lines, one key, and `type`.** A key sending a list of lines, each one
   command, and a key that only fills the input: yes? A pause between lines (Wrayth's
   `\p`, Vellum's `s1.5`) is left out until asked for.
2. **Defaults in code, the file holding only changes**, and an empty string unbinding
   one: yes? The other way, writing a full file on first run as Vellum does, leaves every
   later default out of every existing file.
3. **Tab** makes the next window the one in use (Wrayth, Vellum), so Page Up, Find and
   Ctrl+Tab act on it. Or should Tab do nothing, and the window in use be only the one
   last clicked?
4. **Shift+Esc stops everything Hydra is doing** on the character. Wrayth's key for it
   pauses a script; stop is the nearest thing Hydra has. Right key, and stop rather than
   pause?
5. **`next_unread_tab`'s key.** Nothing agrees; Vellum leaves it unbound. Leave it so?
6. **Target order**: the game's own list, until a creature field exists?
7. **One set of keys for every character**, as today, or a character's own on top?
   Wrayth has ten sets switched with Alt+0 to Alt+9; nobody has asked for that here.
