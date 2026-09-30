# 15 — What Vellum has that Hydra does not

What VellumFE does for a player that Hydra does not, area by area, and what Hydra already
has. Asked for by the author on 2026-09-28 (*"We need to find out what we are missing that
Vellum has"*) and made the same day. Six read-only agents each surveyed one area of both
codebases.

## How it was made

- **Read, not run.** Nothing was built and neither client was run.
- **What was compared.** VellumFE is `reference/VellumFE` at `c1f7953` (2026-09-16):
  387 `.rs` files, 324,605 lines, measured with `find src -name '*.rs' | xargs wc -l`.
  Hydra is `main` at `a9c8019`.
- **Labels.** Each row is one of:
  - **VERIFIED**: both sides were read.
  - **INFERRED**: Hydra was searched for the feature and nothing was found. The search is
    named in the row where it matters.
- **Paths.** A Vellum path is relative to `reference/VellumFE/src/` unless it starts with
  `defaults/` or `android/`. A Hydra path is relative to the repository root.
- **Where to start.** `inventory/00-vellum-capability-survey.md`, `05` and `06` were the
  leads. Hydra has built a lot since they were written, so every row below was checked
  again against the current code.
- **Status words.**
  - **MISSING**: Hydra does not have it and no plan names it.
  - **PARTIAL**: Hydra has some of it.
  - **PLANNED**: a plan names it, and it is not built.
  - **BY DESIGN**: left out on purpose, with the decision cited.

## The short version

**Four gaps are worth starting with.** None of them is in any plan.

1. **Keys bound to actions.** A Hydra key can only send one line of text
   (`crates/cena-gui/src/keys.rs:262-345`). A Vellum key can also do any of about 60
   actions (`config/keybinds.rs:257-377`). Finding text, scrolling, tab switching, resend,
   targeting and most of the input gaps below wait on this.
2. **Command history kept per character across restarts.** Hydra keeps 100 lines in memory
   only (`crates/cena-gui/src/play.rs:155-161, 297-351`), and Up throws away the line being
   typed.
3. **Find in the story** (Ctrl+F, F3 for next).
4. **Server dialogs read into the model.** The parser emits every dialog frame. The model
   reads a few labels and drops the rest in the catch-all at
   `crates/cena-model/src/state.rs:476-481`. One piece of model work would cover Betrayer,
   the game's own quickbar, the aim timer, and the bounty, combat and event panels.

**Already planned and not built:** themes, fonts and skins (`plan/49` Stage F); the injury
doll, the map in the GUI and the creature field (Stage G); drawers and zones (Stage E);
hotbars; the terminal UI; gamepad.

**Hydra's parser is not the gap.** Its 126 tags cover all 116 of Vellum's
(`crates/cena-protocol/src/tags.rs`), and every frame it drops has its reason written in
`crates/cena-protocol/src/frame.rs:23-111`. The gaps are in what the model does with the
frames, and in the client around them.

## 1. Input, keybinds, commands

| Feature | Vellum | Hydra | |
|---|---|---|---|
| Keys bound to actions, not only text: cursor, history, scroll, search, tabs, copy and paste, toggles | `config/keybinds.rs:257-377`; `defaults/globals/keybinds.toml:86-149` | **MISSING**. A key maps to one line (`crates/cena-gui/src/keys.rs:283-314`) | VERIFIED |
| Command history saved per character across restarts | `frontend/gui/app/command_input.rs:17-40` | **PARTIAL**. Kept in memory for the session only, and Up replaces the line being typed | VERIFIED |
| Grey suggestion from history; Tab completion of commands and window names | `frontend/common/command_input_model.rs:5, 461-469, 564` | **MISSING**. Recommended in `plan/44-user-qol-review.md:184` | VERIFIED |
| Resend the last or second-to-last command (Ctrl+R, Ctrl+T) | `frontend/gui/app/global_input.rs:636-637` | **MISSING** | VERIFIED |
| Find in the story (Ctrl+F, F3 and Shift+F3, Esc) | `frontend/gui/app/search_bar.rs`; `global_input.rs:675-889` | **MISSING** | INFERRED |
| Scroll a window by key (PageUp, PageDown, a line, Home, End) | `config/keybinds.rs:279-284` | **MISSING**. The mouse wheel works | VERIFIED |
| Next tab, previous tab, next unread (`.nexttab`, `.prevtab`, `.gonew`) | `config/keybinds.rs:293-295`; `core/app_core/commands.rs:3211-3219` | **PARTIAL**. Tabs count unread lines; nothing moves between them | VERIFIED |
| Move focus between windows with Tab | `config/keybinds.rs:278` | **MISSING** | INFERRED |
| Keybind profiles, global plus per character (`.savekb`, `.loadkb`, `.kbprofiles`) | `core/app_core/commands.rs:3044-3097` | **MISSING**. One set for every character (`crates/cena-gui/src/keys/page.rs:1-3`) | VERIFIED |
| `<target_id>` and `<target_noun>` in a macro, filled from the current target | `core/app_core/keybinds.rs:136-144` | **MISSING** | VERIFIED |
| Target next or previous in screen order | `config/keybinds.rs:319-320` | **MISSING** | INFERRED |
| Interact mode: move between exits and creatures from the keyboard (F6) | `config/keybinds.rs:323-329`; `frontend/gui/app/interact.rs` | **MISSING** | INFERRED |
| Quick switcher (Ctrl+K); undo and redo a layout change | `frontend/gui/app/quick_switcher.rs`, `layout_undo.rs` | **MISSING** | VERIFIED |
| `.menuof <id>`: open an object's menu by id | `core/app_core/commands.rs:3196` | **MISSING** | INFERRED |
| `.emptyhands`, `.fillhands` as typed commands | `core/app_core/commands.rs:2851-2882` | **PARTIAL**. Inside travel only (`crates/cena-behavior/src/travel/drive/deeds.rs:24-32`) | VERIFIED |
| Typed `.reconnect`, `.launch <char>`, `.reload <what>` | `core/app_core/commands.rs:2370-2428` | **PARTIAL**. The hub reconnects (`crates/cena-ui/src/hub.rs:29`), and keybinds reload from a button | VERIFIED |
| Menu keybinds and their validator | `config/menu_keybind_validator.rs`; `core/input_router.rs` | **MISSING**. Minor: egui's keys and Esc | VERIFIED |
| Phone and web macro buttons (`macros.toml`) | `config/macros.rs:1-20` | **MISSING**. Matters only if Despana needs them | INFERRED |
| Hotbars: buttons that send lines, change look on conditions and show countdowns, with an editor | `config/hotbars.rs`; `core/hotbar.rs`; `defaults/globals/hotbars.toml` | **PLANNED**. Only "buttons that send a line" (`plan/49-gui-widgets.md:198`) | VERIFIED |
| Gamepad: binds, radial wheel, legend | `frontend/gui/app/gamepad.rs`; `defaults/globals/controller.toml` | **PLANNED**, "later" (`plan/47-m10-gui.md:142`) | VERIFIED |
| Pause, Scroll Lock, macOS Clear | bind in Vellum | **PLANNED**. Waits on the egui fork (`crates/cena-gui/src/keys.rs:19-23`) | VERIFIED |

**Hydra has (11):** numpad keybinds with NumLock read; a Keys page that captures a key as it
is pressed; keybinds that send a line; Enter and Up/Down; egui's own editing in the input;
drag-select and copy of story text with links; link clicks (`<d cmd>`, cmdlist `coord=`,
the game's `_menu`, web addresses); dragging items with `_drag`; a changeable command symbol
and `;help`; `;go2` and `;stop`; `;foreach` and `;multi`. It also has `;to`, `;all`, `;sc`
and Lich scripts, which Vellum does not.

## 2. Windows, widgets, layout, look

Vellum has 38 widget types (`data/window.rs:23-78`). Hydra's `Widget` enum
(`crates/cena-gui/src/widget/kind.rs:16-104`) has 38 plain kinds, plus 7 named streams and
any other stream, 4 effect lists and 19 indicators.

| Feature | Vellum | Hydra | |
|---|---|---|---|
| Themes and colour schemes | `frontend/gui/app/theme.rs`; `theme/loader.rs`; `config/colors.rs` | **PLANNED**, Stage F (`plan/49-gui-widgets.md:846`) | VERIFIED |
| Fonts, text size, zoom, a font per window | `frontend/gui/persistence.rs:64-136`; `app/render_settings.rs` | **PLANNED**, Stage F. Sizes are fixed in code today | VERIFIED |
| Skins, skin packs, art frames, window backgrounds | `frontend/gui/skin.rs`; `config/skin_pack.rs` | **PLANNED**, Stage F | VERIFIED |
| Per-window borders, title, see-through background, corner radius | `config/widgets.rs:363-409`; `app/borders.rs` | **PLANNED**, Stage F | VERIFIED |
| Injury doll | `frontend/gui/app/widgets/injury.rs` | **PLANNED**, Stage G. The model has the data | VERIFIED |
| Map, minimap and map explorer in the GUI | `frontend/gui/map_view.rs`; `app/map_explorer.rs` | **PLANNED**, Stage G. Despana has one (`crates/cena-web/assets/atlas/minimap.mjs`) | VERIFIED |
| Creature field (sprite cards) and scenes | `app/widgets/creature_field.rs`; `studio.rs`; `config/scenes.rs` | **PLANNED**, Stage G. Hydra lists creatures by name | VERIFIED |
| Drawers and zones (header, footer, sidebars, click-through) | `frontend/gui/app/zones.rs` | **PLANNED**, Stage E (`plan/49-gui-widgets.md:840`) | VERIFIED |
| The command input as a movable widget | `config/window_def.rs` | **PLANNED** (`plan/49-gui-widgets.md:197`) | VERIFIED |
| Pop a window or tab out into its own OS window | `frontend/gui/app/detached.rs` | **MISSING**. Discussed in `plan/28-gui-inventory.md:702`, and no stage owns it | VERIFIED |
| Scrollback depth set per window | `config/widgets.rs:572` | **PARTIAL**. One fixed cap of 2,000 (`crates/cena-model/src/state/streams.rs:60`); `plan/50-settings-inventory.md:147` names it, and it is not built | VERIFIED |
| Split scrollback: history frozen above while scrolled up | `plan/28-gui-inventory.md:352` | **MISSING** | INFERRED |
| One text window showing several streams | `config/widgets.rs:570` | **PARTIAL**. One stream per window; the Story takes streams whose window is closed | VERIFIED |
| Alert overlay: art, one-shot pictures, edge flash | `frontend/gui/app/alert_overlay.rs` | **PARTIAL**. A text banner (`crates/cena-gui/src/play/draw.rs:157`) | VERIFIED |
| Icons for status indicators | `frontend/gui/app/status_icons.rs` | **PARTIAL**. A coloured box with a word | VERIFIED |
| Stun countdown bar | `config/presets.rs:218` | **PARTIAL**. Stun is an indicator | INFERRED |
| Dashboard and mini vitals widgets | `WidgetType::Dashboard`, `MiniVitals` | **PARTIAL**. Presets of single widgets instead | VERIFIED |
| Tearing a tab out to a free window; joining one by hovering | `frontend/gui/app/tab_drag.rs`, `tab_join.rs` | **PARTIAL**. Tabs live in custom windows only (`plan/49` §1) | VERIFIED |
| Snap options (radius, siblings, centres) | `frontend/gui/app/snap.rs` | **PARTIAL**. Ported without anchors and centre lines; the grid is the one setting | VERIFIED |
| Play window position and size remembered | `window_position/` | **PARTIAL**. Opens at 980x680 (`crates/cena-gui/src/app.rs:269`) | INFERRED |
| Missing-spells watch list, perception window, game dialog panels, bestiary view, Betrayer, spacer | `WidgetType::*` | **MISSING**. Dialog panels are left out on purpose (`plan/49-gui-widgets.md:807`) | VERIFIED |
| Colour emoji; room art; hand icons | `frontend/gui/app/color_emoji.rs`; `config/room_images.rs` | **MISSING** | VERIFIED |
| Lich WebUI panel | `frontend/gui/app/webui_panel.rs` | **MISSING** | VERIFIED |
| Performance monitor | `WidgetType::Performance` | **BY DESIGN** (`plan/50-settings-inventory.md:280`) | VERIFIED |

**Hydra has (15):** the Story and stream windows; the Room and its parts; bars for vitals,
stance, encumbrance, mind and next level; roundtime and cast countdowns; the compass; hands;
effects in four lists; creatures and players; objects, spellbook, reserve and containers;
objectives; tab stacks; presets and named layouts, locking and the grid; timestamps; links
with menus, and trigger colours.

## 3. Highlights, alerts, sound, speech

Hydra's triggers (`plan/45-m8-triggers.md`; the fields a rule accepts are `Raw`,
`crates/cena-model/src/trigger/check.rs:38-64`) against Vellum's highlights and alerts.

| Feature | Vellum | Hydra | |
|---|---|---|---|
| Text-to-speech: a queue, next/previous/next unread, mute, alerts that cut in, speech-only gags, pronunciation, voice/rate/volume, auto-speak for thoughts and speech | `tts/mod.rs`; `config/settings.rs:412-433` | **MISSING** | VERIFIED |
| Countdown timer bars started by a match | `core/alert_timers.rs`; `config/highlights.rs:138-150` | **MISSING**. `flag = {seconds}` sets state and draws nothing | VERIFIED |
| Players' own timed state events: a pattern sets, clears or counts a timer, the time from a capture | `config/highlights.rs:214-235` | **PARTIAL**. `event` covers Hydra's own classifiers only | INFERRED |
| Rich alerts: art, flash, placement, duration, priority, `cancels` | `config/highlights.rs:28-133` | **PARTIAL**. A text banner for 4 s, up to 5 | VERIFIED |
| Volume per rule, master volume, cooldown per sound | `config/highlights.rs:180`; `sound.rs:41-92` | **PARTIAL**. On or off; the cooldown is per trigger | VERIFIED |
| Condition words: roundtime or cast time active; hand contents | `config/conditions.rs:41-42, 187-194` | **MISSING** | INFERRED |
| Vital conditions with any comparison, percent or amount | `config/conditions.rs:248-293` | **PARTIAL**. Fixed at-least and at-most words | INFERRED |
| Effect conditions: active, inactive, time left, a name contained | `config/conditions.rs:20-37` | **PARTIAL**. `expiring "<name>" N`, and `Status` | VERIFIED |
| Scope by area, realm, map tag or room | `config/alertpacks.rs:40-131` | **MISSING**. The guard words name no place | INFERRED |
| Alert packs: files of their own, turned on per pack, trusted by hash | `config/alertpacks.rs` | **BY DESIGN**: one file, no packs (`plan/45` §1). Imported `send` waits for approval | VERIFIED |
| `silent_prompt`: a matched line brings no prompt back | `config/highlights.rs:188` | **MISSING** | VERIFIED |
| A substitution in one window, its colour everywhere | `config/highlights.rs:198` | **PARTIAL**. `stream` limits the whole trigger | VERIFIED |
| Emoji shortcodes in text; custom image emoji; `<vellumImg>` inline images | `core/emoji.rs`; `core/custom_emoji.rs`; `core/inline_image.rs` | **MISSING** | VERIFIED |
| A GUI editor for highlights | `frontend/gui/app/editors/highlights.rs` | **MISSING**. `;trigger` stands in | INFERRED |
| Startup music; controller rumble on a match | `config/settings.rs:369-371`; `config/highlights.rs:182` | **MISSING**. Minor | VERIFIED |

**Hydra has (17):** text, regex, case, a stream filter and fast literal sets; colour,
background and bold on the match or the line, and on capture groups; squelch, substitute
and redirect; flags with a duration; sounds; condition alerts that fire once and re-arm;
cooldowns; categories; per-character rules merged field by field; switches by category and
kind; `;trigger test`; Wrayth import; the sorter; players painted in the room window.
**Hydra has these, and Vellum does not:** OS notifications, `send`, a fixed order when looks
overlap, and Wrayth `<ignores>` imported as squelches.

## 4. The game's state: protocol and model

| Feature | Vellum | Hydra | |
|---|---|---|---|
| Server dialogs: labels, buttons, fields, dropdowns, open, close, expose | `parser/dialogs.rs:489-896`; `core/messages/element.rs` | **PARTIAL**. Parsed; the model reads the `expr` and `encum` labels (`crates/cena-model/src/state/character.rs:534-542`) and the target dropdown and injury radios (`state/targeting.rs:185-206`) | VERIFIED |
| Betrayer panel: blood points, items | `core/messages/element.rs:1945-1990` | **MISSING**. Its labels reach the catch-all | VERIFIED |
| The game's quickbar contents | `parser/dialogs.rs:950-1037` | **PARTIAL**. `switchQuickBar` is a frame nothing reads | VERIFIED |
| Aimed-shot timer | `parser/dialogs.rs:162-170` | **PARTIAL**. `Frame::AimTime`, never read | VERIFIED |
| Another player's injuries (the appraisal popup) | `parser/dialogs.rs:372-420` | **PARTIAL**. Left out of the model on purpose (`crates/cena-model/src/state/vitals.rs:121-170`) | VERIFIED |
| Room picture | `parser.rs:112`; `core/game_art.rs` | **PARTIAL**. `Frame::RoomPicture` (`crates/cena-protocol/src/parser/dispatch.rs:230`), never read | VERIFIED |
| PantheonStatus; LaunchURL | `parser.rs:402, 326` | **PARTIAL**. Parsed, never read | VERIFIED |
| Missing-spells watch list | `core/missing_spells.rs` | **MISSING**. Guard words could express it | INFERRED |
| Elanthian time of day: hour, dawn, day, dusk, night | `core/elanthian_time.rs` | **MISSING** | INFERRED |
| Estimated lag: the machine's clock against the game's | `core/state.rs:2110, 2165` | **MISSING** | INFERRED |
| Compact bounty, one to four lines | `core/bounty_parser.rs` | **PARTIAL**. The task is classified; there is no compact form | VERIFIED |
| Creature effects from effect-list `<effect>` entries | `core/spell_table.rs:47-64, 249-274` | **PARTIAL**. 30 statuses from Lich's combat module; `<effect>` entries are not read | INFERRED |
| Map evidence (`forage sense`, ranger `sense`) kept per room | `core/evidence.rs` | **MISSING** | INFERRED |

**Hydra has (25):** the tag table, mangled `$<` markup, multi-line capture, the stream stack,
own injuries, effects with expiry, vitals, the mind bar, targets and hidden targets,
`crtrStatus`, the room's players, objects and creatures, room meta, world events, the
inventory manager and item view, cmdlist, objectives, group, ready and stow, the sorter,
gameobj classes, bounties, society, profession, citizenship and urchins, currency, the day
pass, movement feedback. Hydra captures only `<inventoryViewItem>` across lines, having
measured none of Vellum's other seven doing so (`crates/cena-protocol/src/parser/view_item.rs:1-14`).

## 5. Maps, travel, game data

| Feature | Vellum | Hydra | |
|---|---|---|---|
| Map and minimap in the GUI | `frontend/gui/map_view.rs`; `core/map_service.rs` | **PLANNED**, Stage G. Despana has an explorer and minimap (`plan/28-despana-map-explorer.md`, `plan/40-despana-live-map.md`) | VERIFIED |
| Room pictures from play.net's art, cached | `core/game_art.rs` | **PARTIAL**. Parsed, never read (§4) | VERIFIED |
| Room art mapped by the player (`room_images.toml`) | `config/room_images.rs` | **MISSING** | VERIFIED |
| Bestiary in the client | `core/bestiary.rs`; `frontend/gui/app/widgets/bestiary.rs` | **PARTIAL**. Loaded (`crates/cena-model/src/state/creature/load.rs`) and shown in Despana; no GUI widget | VERIFIED |
| Creature cards | `core/creature_cards.rs` | **PLANNED**, Stage G | VERIFIED |
| Skill trainer (play.net's skill manager) | `core/skill_trainer.rs` | **MISSING**. `LaunchURL` is only parsed | VERIFIED |
| Inventory paged through `_inventory manager … continue` | `core/inventory_service.rs` | **PARTIAL**. Snapshots and pending pages are parsed (`crates/cena-model/src/state/inventory_snapshot.rs`); nothing sends the request or the `continue` | INFERRED |
| Item moves checked against the hands, one at a time | `core/item_mover.rs` | **PARTIAL**. Drag with `_drag` (`crates/cena-gui/src/carry.rs`); nothing confirms the move | INFERRED |
| mapdb updates downloaded, without Lich | `core/mapdb_update.rs` | **MISSING**. `CENA_MAP` names a converted file, read at start | VERIFIED |
| Jinx asset client | `core/jinx/mod.rs` | **MISSING** | VERIFIED |
| Data packs: Lich's data folder, then a local store, then bundled | `core/data_pack.rs` | **PARTIAL**. gameobj-data is read; the fallback is unconfirmed | INFERRED |
| Ghost rooms: unmapped shops sketched for the session | `core/ghost_rooms.rs` | **MISSING**. Needs the GUI map | VERIFIED |
| Classic map images; curated base maps | `core/classic_maps.rs`; `core/curated_maps.rs` | **PARTIAL**. Despana uses classic coordinates and its own regions | VERIFIED |
| Pathcode mazes: the route from the NPC (the Ranger Guild) | `core/travel/mazes.rs` | **MISSING**. Hydra walks the minotaur maze and the Confluence | VERIFIED |
| UI packs (`.vellumpack`); the palette generator | `core/uipack.rs`; `core/harmony.rs` | **MISSING**. Goes with Stage F | VERIFIED |

**Hydra has (8):** the mapdb graph with pathfinding and locating (`crates/cena-map`); `go2`
with targets; the walk that reacts to the game's text; the Confluence; the minotaur maze;
the Chronomage day pass; stowing and fetching hands; StringProc, `timeto` and urchin gates.
Travel is built and has not run live (`crates/cena/src/travel.rs`).

## 6. Connecting, sessions, platforms

| Feature | Vellum | Hydra | |
|---|---|---|---|
| Playing from a phone on the LAN: web server, bind, pairing | `frontend/web/server.rs:48, 131` | **PARTIAL**. Despana binds this machine only (`crates/cena-web/src/server.rs:318`) | VERIFIED |
| Terminal UI | `frontend/tui/` | **PLANNED** (`plan/23-m4-frontend.md:275, 284`) | VERIFIED |
| Android and iOS apps | `android/`, `ios/` | **PLANNED** as a compile check only (`plan/m4-mobile-verification.md`) | VERIFIED |
| Performance overlay, `.performance dump` | `performance.rs:81` | **MISSING** | VERIFIED |
| Lich WebUI bridge | `webui.rs` | **MISSING**. Only the tag is known | VERIFIED |
| Typed reconnect and launch | `core/app_core/commands.rs:2382, 2421` | **PARTIAL**. The hub does both | VERIFIED |
| Finding Lich from its running process | `process_probe.rs` | **MISSING**. `lich folder <folder>` once (`crates/cena/src/lich.rs`) | VERIFIED |
| Shipped defaults added to a player's files as they grow | `config/defaults_refresh.rs` | **MISSING**. May not matter to Hydra's settings | INFERRED |
| Attaching to a running Lich; being launched by Lich with `--key`; SSH start of Lich | `network.rs:37, 629`; `launcher/` | **BY DESIGN**. The relay replaces them (`plan/51-lich-relay.md`) | VERIFIED |
| DragonRealms | `network.rs:215-223` | **BY DESIGN** (CLAUDE.md, settled decisions) | INFERRED |
| A registry of sessions across processes | `core/session_registry.rs` | **BY DESIGN**. One process holds every character | INFERRED |

**Hydra has (11):** direct login; the launcher with accounts, characters and favourites;
passwords in the OS keyring, then the environment, then a prompt; the roster; reconnect with
Vellum's backoff; several characters in one process with `;to` and `;all`; headless; the GUI
hub; the wire log with timestamps and rotation; the daily player log, which Vellum does not
have; `CENA_DATA_DIR`.
