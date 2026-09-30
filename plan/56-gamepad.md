# 56 — Gamepads: VellumFE's controller support, ported whole

**Status: PROPOSED 2026-09-30.** The author, 2026-09-30: *"We also want to port the controller
support that vellum has for gamepads. Full port of the controller code in entirety as well as
the controllers editor for it's button binds and wheel menus."*, and of this plan: *"yes a plan
is good."* Nothing is built. §5 holds the questions; §4 the steps, each a commit, on a branch
`gamepad` once the author approves.

---

## 1. What VellumFE has, measured

`reference/VellumFE` at `c1f7953` (2026-09-16). Counted with `wc -l`; found with
`grep -rilE "gamepad|gilrs|controller|joystick|xinput|sdl2|deadzone|radial"`.

| File | Lines | What it is |
|---|---|---|
| `src/frontend/gui/app/gamepad.rs` | 3,023 | the runtime: polling, buttons, sticks, the wheel's state machine, drawing the wheel and the overlay, rumble |
| `src/frontend/gui/app/editors/controller.rs` | 4,312 | the `.controller` editor: Bindings, Wheels (with a painted wheel designer), Rumble, Tuning |
| `src/frontend/gui/app/editors/touch_wheel.rs` | 202 | the phone's touch wheel editor |
| `src/core/app_core/haptics.rs` | 152 | rumble on roundtime's end, a stun, a death |
| `defaults/globals/controller.toml` | 154 | the shipped binds and wheel |

7,843 lines between them. Beside them, the controller's share of shared files:
`src/config/keybinds.rs` (the data model at `:85-170` and `:891-1442`, load and save at
`:2314-2830`, merging at `:3537-3589`), 42 `cfg(feature = "gamepad")` in
`src/frontend/gui/app.rs`, and `execute_macro_keybind` in `global_input.rs:572`. A browser
version for the web and mobile lives in `src/frontend/web/assets/app.js`.

- **The crate.** `gilrs = { version = "0.11", optional = true }` (`Cargo.toml:104`) behind a
  feature `gamepad`, on by default (`:141`, `:144`). No SDL, no XInput. `Gilrs::new()` once at
  start (`app.rs:1086`); a failure is a warning, not an error.
- **No thread.** `poll_gamepad` (`gamepad.rs:152`) runs once an egui frame, before the
  keyboard: it drains button and connection events, reads both sticks of the first pad, and asks
  for a frame every 50 ms while a pad is connected (`:500`).
- **Buttons.** Seventeen names (`gamepad.rs:21-64`): `south east north west`, the d-pad's four,
  `l1 r1 l2 r2 l3 r3`, `select start guide`. A bind's key is up to two held modifiers and the
  button, `l2+r1+south`, matched exactly, never falling back to fewer (`:688-697`). A button bound
  to `controller_modifier` is held to chord and does nothing alone.
- **What a bind does.** An action by name or a macro's text. 30 of the actions table's rows are
  the controller's (`grep -c "scope: ActionScope::Controller" src/config/keybinds.rs` → 30):
  scrolling, targeting, stop travel, an interact mode and its select, menu navigation, the
  modifier, the overlay, the wheel (`controller_wheel:<name>`), and text to speech.
- **Sticks.** The movement stick walks: eight sectors send `n`, `ne` and so on. The aim stick
  scrolls the story on a curve, or in interact mode moves the focus among what is in the room;
  held open, it aims a wheel.
- **Wheels.** A ring of slices, each a label and a command, a colour, a span in degrees, and
  folders of slices within slices, with Back. The stick's angle past the deadzone picks one; a
  dwell commits it; it fires on release, at an edge, or on the stick's return, as the tuning
  says; South fires at once, East backs out. `<target_id>` becomes the focused creature. A
  `portals` wheel is built from the room's exits. The state machine is pure functions
  (`wheel_aim_step` `:1593`, `wheel_south_step` `:1745`), checked against golden vectors
  (`tests/data/wheel_golden.json`) shared with the browser's copy.
- **The overlay.** A legend at the right edge of what each button does, lit while its
  modifiers are held (`render_controller_overlay`, `:2975`).
- **Rumble.** Roundtime ending, a stun and a death each a pattern (short, long, double, or the
  player's own); a highlight may rumble too.
- **Tuning.** Which stick moves, the deadzone (50%), dwell, debounce, grace, fire mode,
  trigger thresholds, the least time a wheel stays open.
- **Where it is kept.** `controller.toml`, one for everyone and one per character, merged key by
  key, a wheel replaced whole by the file that last names it (`keybinds.rs:2328`, `:2752`).
  Sections `[controller]`, `[controller_overlay]`, `[controller_rumble]`, `[controller_tuning]`,
  `[[controller_wheel]]`, `[[controller_wheels.<name>]]`, `[controller_wheels_meta.<name>]`.
- **The editor** (`.controller`): Bindings (a form, and *Press a button...* to capture one;
  rows marked everyone's or the character's), Wheels (a list, and a painted designer: drag a
  divider to resize, drag the inner arc for the aim floor, split, merge, move, mirror, even out,
  lock, undo fifty steps), Rumble (events, patterns, Test), Tuning (every field).
- **Tests.** 43 in `gamepad.rs`, 27 in `controller.rs`, 3 in `haptics.rs`.
- **What it says of itself.** Windows and Linux tried on hardware; macOS *"verify on hardware;
  may need a GameController-framework bridge"* (`gamepad.rs:9-12`). Only the first pad's sticks
  are read (`:189`). Three hardware faults worked around: a stick frozen by the driver (seen for
  2,546 frames), phantom axes on reconnect, trigger bounce.

## 2. What Hydra has to build on

- **Actions and macros** (`plan/52`, `crates/cena-gui/src/keys/`): `keys/binding.rs:22`,
  `Macro::{Send, Fill, Act(Action)}`, a `Send` of several lines with waits, and 33 actions:
  scrolling, history, the windows and tabs, Find, targeting next and previous, the drawers,
  Stop, Lock, Arrange, a character. **A controller bind is a Hydra `Macro`**, keyed by
  VellumFE's `mods+button`: most of VellumFE's controller actions already exist here by another
  name, and a wheel slice's command goes out the same way a key's line does.
- **Everyone's and a character's** (`keys/file.rs`, `Whose::{Every, Character}`): the same
  split `controller.toml` has.
- **The settings menu's pages** (`cena_ui::settings`) and the trigger editor's window
  (`plan/54`) are the two shapes an editor takes here.
- **What rumble would listen for**: roundtime's end, the stun timer (`state/stun.rs`, built
  2026-09-30), death, and a trigger's attention.

## 3. How it is carried over

**The code is ported, not rewritten**, as the author asked, and as `CLAUDE.md` says of knowledge
that lives in code. Two things change on the way, both this workspace's rules:

- **No file over 800 lines** (`crates/cena-arch-tests/tests/file_rules.rs`): `gamepad.rs`'s 3,023
  and `controller.rs`'s 4,312 are split by what each part does. The runtime becomes a
  `gamepad/` module in `cena-gui` (polling, buttons, sticks, the wheel's machine, drawing, the
  overlay, rumble); the editor a `gamepad/editor/` of its four tabs and the designer.
- **Hydra's names for Hydra's things.** A bind's value is a `keys::Macro`; the file is Hydra's,
  in its data folder beside `keybinds.toml`; the actions VellumFE has that Hydra has by another
  name are mapped, not duplicated. VellumFE's `controller.toml` is read by an importer, as
  `;keys import` reads a Wrayth key set.

`gilrs` lives in `cena-gui` only, behind a feature `gamepad` on by default. The five core crates
that build for Android and iOS never see it.

## 4. Steps

Each a commit, tested, on the branch.

0. **The pad.** `gilrs` behind the feature, polled once a frame before the keyboard; buttons,
   both sticks, connect and disconnect; the three hardware faults' guards, with VellumFE's tests.
1. **The binds.** `controller.toml`, everyone's and a character's, merged as VellumFE merges them;
   `mods+button` keys and modifier layers; a bind is a `Macro`; the shipped defaults; a
   controller's actions added to `Action` where Hydra has none (the menu's navigation, interact,
   the modifier, the overlay, a wheel).
2. **Doing what a bind says.** A button does its macro; the movement stick walks; the aim stick
   scrolls the story, or moves the focus in interact mode (Hydra's targeting over the game's
   `dDBTarget` list); menu navigation drives an open menu.
3. **Wheels.** The slices, folders and spans; the pure state machine ported with its golden
   vectors (`wheel_golden.json`, the same file, so the two stay one); drawing the wheel; the
   `portals` wheel from the room's exits; `<target_id>` from the focus.
4. **The overlay**, lit by the held modifiers.
5. **Rumble** on roundtime's end, a stun, a death, and a trigger's attention.
6. **The editor**: Bindings with *Press a button...*, Wheels and the painted designer with its
   undo, Rumble with Test, Tuning; opened by `.controller` and from the settings menu.
7. **Import** a player's VellumFE `controller.toml`, everyone's and each character's.
8. **Docs**: the architecture page, the glossary (*bind*, *wheel*, *slice*, *modifier*), and
   `CLAUDE.md`'s row.

## 5. Questions for the author

1. **The browser's gamepads.** VellumFE's web page reads pads through the browser for mobile
   (`app.js`). Despana's page is a demo today: leave the browser's pads for when Despana is
   real? Claude's recommendation: yes, later.
2. **The touch wheel** (`touch_wheel.rs`, the phone's): the same question, the same
   recommendation.
3. **Text to speech.** Nine of VellumFE's controller actions speak; Hydra has no speech yet
   (`plan/50` §6: *later*). Leave them out until it has? Recommendation: yes.
4. **Where the editor lives.** A window of its own, as the trigger editor is, opened by
   `.controller` and from Settings? Recommendation: yes; four tabs and a designer are more than
   a settings page holds.
5. **Interact mode.** VellumFE's aim stick moves a focus among what is in the room. Hydra's
   nearest is targeting round the game's `dDBTarget` list (`plan/52` step 7). Use that, and
   widen it to the room's objects and exits as VellumFE's does? Recommendation: port VellumFE's
   focus whole, over Hydra's room model.
6. **More than one pad.** VellumFE reads the first pad's sticks only. Keep that?
   Recommendation: yes, as ported; a second pad is a later question.
