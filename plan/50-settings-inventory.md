# 50 — Settings: what Hydra has, what it lacks, and where each lives

**Status: PROPOSED 2026-09-27, for the author.** This is `plan/49` Stage D's first step. It
answers `plan/28` §9 item 3, where the author said: *"we shall discuss this when it's time
taking inventory of all settings we have and need."* Nothing in this document is built. §5
lists the questions to answer before any settings editor is written.

**How it was measured.** Three read-only surveys, 2026-09-27, on branch `gui-widgets` at
`fc52989`:

1. every file Hydra persists;
2. every argument, environment variable and `;` command that configures something;
3. every setting `reference/VellumFE` exposes.

Each item below cites its source. Items marked **VERIFIED** were checked again by hand.
The rest are cited from the surveys and were not re-read.

---

## 1. What Hydra has today

### 1a. Files

`<D>` is the data folder: `CENA_DATA_DIR`, otherwise `data` relative to wherever Hydra
was started (`crates/cena-session/src/character_store.rs:64`; Hydra's own folder since §7
step 7). `<ic>` is
`<instance>_<character>` in lower case.

| File | Scope | How a player changes it | Keys | Where |
|---|---|---|---|---|
| `<ic>.settings.json` | character | **hand edit only** | the command symbol; which player-log feeds are kept | `cena-session/src/settings_store.rs:121` |
| `travel.json` | one file; saved targets per character or global | `;go2 save`/`delete`; the walk writes its memories | saved targets; plus a **free-form string map** of travel settings (`use_urchins`, `get_silvers`, `use_day_pass`, `day_pass_sack`, `buy_day_pass`, `fwi_trinket`, `use_gigas_hwtravel`, `ice_mode`, `key_sack`, and whatever the map file names) | `cena-session/src/travel_store.rs:103` |
| `hunt/global.toml` | every character | **hand edit only** | any hunt profile key | `cena-behavior/src/hunt/chain.rs:46` |
| `hunt/profiles/<name>.toml` | shared by name | `;hunt import`, `;hunt set/unset`, the web setup page | about 113 keys: targets, routines, rooms, stance, rest, flee, loot, wander, react, aim, wand, boons, monitor, unarmed, mstrike, group | `cena-behavior/src/hunt/profile.rs:74` |
| `hunt/characters/<ic>.toml` | character | **hand edit only** | any hunt profile key | `cena-behavior/src/hunt/chain.rs:67` |
| `hunt/heal/<ic>.toml` | character | `;heal set/unset/show` | 10: container, skip_scars, potions, yabathilium, blood_only, buy_missing, stock, split_blood, deposit_coins, distiller | `cena-behavior/src/heal/profile.rs:29` |
| `hunt/waggle/<ic>.toml` | character | `;waggle set/unset/show` | 8: cast_list, start_at, stop_at, refreshable_min, multicast, reserve_mana, bail, skip_not_sharing | `cena-behavior/src/waggle.rs:40` |
| `hunt/keep/<ic>.toml` | character | `;keep add/del/nocast/power` | 3: spells, nocast, power | `cena-behavior/src/keep.rs:34` |
| `hunt/sc/<ic>.toml` | character | `;sc alias/verb/stance/set` | alias, verbs, stance, channel, conserve, safety, stance_all, typed | `cena-behavior/src/spellcaster.rs:41` |
| `hunt/loot/<ic>.toml` | character | `;hunt import-loot` **only**; the hunt appends `unskinnable` | take, leave, defensive, disk, overflow, autoclose, …; `[skin]` 10 keys; `[town]` about 18 keys that are read, and about 15 imported from eloot that nothing reads | `cena-behavior/src/loot/profile.rs:41` |
| `triggers.toml` | every character, with per-character overrides inside | `;trigger …` | switches by category and by kind of response; about 27 keys per trigger | `cena-behavior/src/triggers.rs:84` |
| `keybinds.toml` | every character | **hand edit only**, then the play window's reload button | `numpad`; `[keys]` | `cena-gui/src/keys.rs:30` |
| `layouts/<name>.json` | per character **name** | the GUI, as the player arranges | holders, cells, tabs, grid, follows | `cena-gui/src/layout.rs:313` |
| `layouts/_presets.json` | every character | *Save as preset*, *Forget* | the saved custom windows | `cena-gui/src/layout/preset.rs:26` |
| `roster.json` | every character | a login proven Ready; Launch's Forget and star | character, account, game, favourite | `cena/src/roster.rs:26` |
| the OS keyring (service `hydra`) | account | Launch's *Keep* box; the terminal's offer; *Forget … password* | the password | `cena/src/secrets.rs:28` |
| `sounds/` | every character | the player drops files in | the sounds triggers name | `cena/src/attention.rs:17` |

Written but not settings: the character snapshot, the learned-menu cache, the combat
database, the certificate pin, and the logs.

### 1b. Arguments and environment

**Arguments.** None is saved. Each lasts one run and covers every character in it:

- `--character <Name>` (repeatable), `--headless`, `--web`
- `--web-login` (`cena/src/connector.rs:223`)
- `--hunt-setup` and `--pages-attend` (both only with `--web`)
- `--first <cmd>` (`cena/src/travel.rs:59`)
- `--record` / `--no-record`

There is no argument parser: each reader scans for its own word, so a mistyped flag is
ignored without a word.

**Environment variables.**

- `CENA_MAP`: no default, and the map stays loaded until restart.
- `CENA_DATA_DIR`.
- `CENA_LOG_DIR`, `CENA_LOG_LINES` and `CENA_LOG_TIMESTAMPS` (`cena-platform/src/sink/config.rs`).
- `CENA_PASSWORD_<ACCOUNT>`.
- `CENA_HUNTING_CORRECTIONS_DIR`, for developers.

**VERIFIED:** `--record` is **on in a debug build and off in a release build**
(`cena/src/setup.rs:195`, `cfg!(debug_assertions)`).

### 1c. Kept only in memory

- `;sorter on|off` lasts only for the session; it is lost on restart
  (`cena/src/sorter.rs:16`).
- Command history (100 lines) and which play windows are open.
- The size and place of each native window: eframe keeps none.

Despana keeps its map preferences and rest-spot favourites in the browser's storage
(`cena-web/assets/atlas/preferences.mjs:18`).

---

## 2. What the inventory found wrong

Fixed on the way, each in its own commit:

- **A character named Presets would have written its layout over the preset library.**
  Both lived in `layouts/`. The library is now `_presets.json`, a name no layout can take,
  and layouts and the library save atomically (`094b482`).
- **The loot profile's `unskinnable` list was written with a plain write**, not the atomic
  save every other behavior file uses (`74a76e1`).

Still open, and each one bears on the editor:

1. **Five places have no writer but a hand edit.** A settings screen cannot edit them
   until each gets one:
   - `<ic>.settings.json`: **VERIFIED**, `settings_store::save` is called only from tests.
   - `hunt/global.toml`.
   - `hunt/characters/<ic>.toml`.
   - `keybinds.toml`.
   - `travel.json`'s settings.
2. **Travel's settings are an unchecked string map** (**VERIFIED**,
   `travel_store.rs:103`). The code reads eight keys and the map file names more
   (`cena-map/src/cond.rs:211`), so the full list cannot be enumerated from the source.
3. **Some changes are not seen until something reloads them:**
   - the `;sc` profile is held in memory at login (**VERIFIED**, `cena/src/hunt.rs:73`);
   - the player-log feeds are read once a session;
   - keybinds wait for the reload button;
   - the map waits for a restart.
4. **Two characters of one name on two games share one layout**, because a layout is
   keyed by name alone (`cena-gui/src/layout.rs:313`).
5. **The data folder moves with where Hydra was started** (plan/44 Q09).
6. **Whether a setting outlives a restart is not said anywhere** (plan/44 Q15): `;sorter`
   does not; `;sc set` does.
7. **The hunt chain has four layers and only one of them has a command.** The layers are
   the built-in defaults, `global.toml`, the profile and the character's file; only the
   profile has `;hunt set`. No one can see which layer a value came from (plan/44 Q09).

plan/44 Q05, an edit writing defaults over a file that did not read, **is already fixed**.
`keep` and `sc` both refuse, and say why (`cena/src/hunt.rs:636`).

---

## 3. What Hydra lacks that VellumFE has

VellumFE's settings are declared once, in a registry: each scalar with its label,
category, range, scope and frontend (`reference/VellumFE/src/config/registry.rs:1`). Its
Settings window is generated from that registry, and a test fails when a setting is in
neither the registry nor its exemption list (`registry.rs:892`). Its layering is defaults,
then global, then character, per key; each row saves to Character or Global
(`src/frontend/gui/app/editors/settings.rs:373`). That is `plan/28` §7f's *"renderable
from a schema"*, already built once.

These are VellumFE's settings that Hydra has no counterpart for, grouped. Which of them
Hydra needs is §5 question 7.

| Group | VellumFE's keys | Stage it belongs to |
|---|---|---|
| Text and fonts | zoom, text size, title font and bar, density, corner radii, font | F (themes, skins, fonts) |
| Colours and themes | the palette, UI colours, spell colours, themes | F |
| Command line | minimum length for history, suggestions from history, command echo | D |
| Scrollback and time | buffer size, timestamps and where they go | D |
| Sound | enabled, volume, cooldown between sounds | D |
| Speech (text to speech) | enabled, rate, volume, which streams, voice, gags, substitutions | D, if wanted |
| Target list | the status abbreviations, excluded nouns, boss, challenging and dead colours | D or F |
| Stream routing | fallback, routes, room in main | largely answered by Stage B's stream widgets |
| Logging | on, folder, lines per file, timestamps | D; environment variables today |
| Window snapping | on, radius, to siblings, bounds or centres, the grid, guides | D; Hydra has only the grid |
| Keybinds editor | the menu keys, the user keys | D |
| Sorter | on, counts, bold labels, sort order, rules | D (`;sorter` has only on/off) |
| Performance monitor | about 16 toggles | probably not |
| Phone, map, travel, game art | as in `reference/VellumFE/src/config/settings.rs` | the map belongs to G |

**What not to copy** (from the survey):

- In VellumFE, saving one setting rewrote unrelated files: colours, highlights, and the
  keybinds without their `[app]` and `[menu]` sections (`src/config/io.rs:643`).
- The theme and the appearance were each kept in two places and copied by hand.

Hydra's rule is already the cure: one owner per value (Rule 4.3,
`crates/cena-arch-tests/tests/single_owner.rs`).

---

## 4. A proposed shape, for the author to accept or change

These are Claude's proposals. None is built.

1. **Keep the files; put one editor over them.** plan/44 Q09 reached the same view. The
   files are already typed, checked and atomic, except where §2 says otherwise. A
   migration would buy nothing a player sees.

2. **The same writer for a command and the screen.** The commands already check what they
   write:
   - `;heal set` and `;waggle set` read the change back through their typed profile before
     saving (`cena/src/hunt/settings.rs:195`, `:201`);
   - `;hunt set` reads the profile as the character would run it, and puts the old text
     back if it no longer loads (`cena/src/hunt/settings.rs:129`).

   A settings screen should call those same paths, not one of its own. Then a file has one writer, and the command and the screen cannot
   disagree.

3. **The schema is the typed structs, with one table of keys each.** Heal and waggle
   already carry one: `KEYS` at `cena-behavior/src/heal/profile.rs:76` and
   `cena-behavior/src/waggle.rs:76`. Widened with a label, a kind, a default and a range,
   that table is what the editor draws from. A behavior's table lives beside its struct.
   An architecture test can then hold what VellumFE's registry test holds: a field
   outside its table fails the build.

4. **Four scopes, named on every row:**
   - **Hydra**: one per install. The data and log folders, sounds, and keybinds unless §5
     says otherwise.
   - **Account**: the kept password, nothing else.
   - **Character**: its own files, its layout, its command symbol.
   - **Shared by name**: hunt profiles, presets, and the triggers file with its
     per-character overrides.

   Where a value is layered (the hunt chain; a trigger's `for.<Name>`), the row shows the
   value in effect **and where it came from**, and offers *use the inherited value* apart
   from setting one.

5. **Say whether a change outlives a restart, and when it takes effect.** Every row says
   *saved* or *this session*, and *now* or *next login*. This covers plan/44 Q15 and §2
   items 3 and 6.

6. **One front door.** A *Settings* button on the hub for Hydra's settings, and one on
   each play window's top bar for that character's. Pages by domain:
   - General: the symbol, history, echo.
   - Text and streams.
   - Sounds.
   - Keys.
   - Heal.
   - Hunt.
   - Spells: keep, waggle, sc.
   - Loot and selling.
   - Travel.
   - Triggers.
   - Logging and data.

   plan/44 Q04 asks for exactly this.

7. **The hunt profile is too large for one form.** Its 113 keys are better as a page per
   table, with *check* beside *save*, as `;hunt check` reads it (plan/44 Q03, Q06, Q08).

---

## 5. Questions for the author

1. **The four scopes in §4.4.** Are they right? In particular, are keybinds one set for
   every character, or one set with overrides per character, as VellumFE has?
2. **Travel's settings.** Should they be a typed list of the keys the code reads, with the
   map file's own keys kept as a free list? Or should they stay one free string map with
   no checks?
3. **The command symbol and the player-log feeds.** Can these go on the character's
   General page? Today nothing writes that file.
4. **`;sorter`.** Should it be saved per character, as VellumFE saves `sorter.enabled`?
5. **Recording.** Should `--record` become a setting, and should both builds share one
   default? If so, which?
6. **Layouts.** Keyed per character name, as now, or per game and name?
7. **§3's list.** Which of VellumFE's groups does Hydra want? Speech and the performance
   monitor are the doubtful ones.
8. **Hunt profiles.** Should the settings window edit them page by page, as §4.7 says?
   Or should they stay files plus `;hunt set`, with a read-only view of the merged chain
   and where each value came from?
9. **The front door.** One Settings button on the hub and one per play window, as §4.6
   says?
10. **The data folder.** Should it stop depending on where Hydra was started? The
    candidates are a fixed folder under the user's profile, or one the launcher remembers.

---

## 6. The author's answers, 2026-09-27

Quoted as given. *Reading* marks Claude's understanding where the answer leaves a choice.

1. **Scopes: *"yes"*.** The four scopes stand as §4.4 proposes. *Reading:* keybinds are
   one set for every character (the Hydra scope, as proposed), with no per-character
   overrides unless the author says otherwise.
2. **Travel's settings: *"checked"*** (asked again in plain words first). The keys the code
   reads become a typed list, each with its kind. The map file's own keys are kept as a
   free list beside it.
3. **The command symbol and the player-log feeds: *"yes"*.** They go on the character's
   General page, and that file gets a writer.
4. **`;sorter`: *"sure and persistent"*.** Saved per character, and kept across restarts.
5. **Recording: *"recording should be a setting, it should be per thing, combat, loot,
   whatever else we decide to record."*** One switch per kind of record, not one for all,
   and not a command-line flag. Today combat and loot share one switch and one database
   (`crates/cena/src/setup.rs:195`); they come apart. Asked for the default: *"off by
   default, turn on stay on until turned off."* So each kind is off until the player turns
   it on, and it stays on across restarts. That is one default for both builds.
   *Reading:* per character, like the other behavior files.
6. **Layouts: *"game and name"*.** A character on Prime and one of the same name on
   Shattered or Test each keep their own layout. A layout saved under the name alone is
   taken as the first one for that name, so nothing arranged is lost.
7. **VellumFE's groups: *"performance monitor can be left out, not sure what the speech
   group is?"*** The performance monitor is out. On speech (text to speech), once
   explained: *"leave tts out for now, we will add it keep it in mind."* It is deferred,
   not refused.
8. **Hunt profiles: *"both sounds good"*** (asked again in plain words first). First a
   read-only view of the chain in effect, each value with where it came from. Then the
   pages, one per table, with *check* beside *save*.
9. **The front door: *"yeah one main settings button to get to the main settings menu. if
   we add right click context menu options to open settings for widgets/windows then they
   would open the same main settings menu to the correct spot for that setting."*** One
   settings menu. Every other way in, such as a widget's right-click, opens that same menu
   at the setting's place. It is never a second editor.
10. **The data folder: *"yeah the data folder should be fixed, ideally we will have an
    installer and updater right? And the data folder would go there with that stuff."***
    Fixed, not where Hydra was started. The installer and updater are `plan/49` Stage H.
    *Reading:* the data belongs in the player's own application-data folder, not beside
    the program, because an update replaces the program's folder. `CENA_DATA_DIR` stays as
    an override.
11. **A setting the author added, 2026-09-27:** *"there should also be a config that
    closes the characters play window when their session closes. It should be off by
    default so the window stays open though."* When a character's session closes, its
    play window closes too if this is on. It is off by default, so the window stays open
    with the character's last state. *Reading:* one setting for all of Hydra.

The hub's own changes, from the same session, are in `plan/49` Stage C's revision. Among
them, the card width is a setting to keep once there is somewhere to keep it (Hydra's
scope).

---

## 7. The steps, set when the author said *"stage d!"* (2026-09-27)

Claude's order, each step shippable and committed on its own:

1. **The menu, and the first pages.**
   - *The description.* The pages a frontend draws are described in `cena-ui`: a page, its
     rows, each row's kind, its value in effect, where that came from, whether it is saved,
     and when it takes effect.
   - *The one menu.* A Settings button on the hub and on each play window opens it. A
     character is picked by roster name (`GAME:Name`), running or not.
   - *The writer.* The binary builds the pages from each behavior's key table and applies a
     change through the same writer its `;` command uses (`cena/src/hunt/settings.rs`).
   - *The first pages:* Heal and Waggle, then Keep and Spellcaster.
   - *A test* that holds each table to its profile's fields, as VellumFE's registry test
     does.

   **BUILT 2026-09-27.**
   - *The description:* `crates/cena-ui/src/settings.rs`. A row says whether its page's
     file sets it (*Use default* puts it back) or it is at its default.
   - *The tables:* each profile's `TABLE` beside its struct, the key's label, help and kind
     (`crates/cena-behavior/src/settings.rs`, `shown`). Heal's and waggle's tests, and
     `crates/cena-behavior/tests/keep.rs` and `crates/cena-behavior/tests/spellcaster.rs`,
     hold each table to its struct's fields.
   - *The writer:* `change` in `crates/cena/src/hunt/settings.rs`, which `;heal set` and
     `;waggle set` now call too. The binary builds the pages and applies a change
     (`crates/cena/src/pages.rs`); a file that does not read is shown with why and never
     written over.
   - *The menu:* `crates/cena-gui/src/menu.rs`, in its own native window. A value is
     checked against its kind before anything is sent, and written as the command would
     type it. A map (spellcaster's aliases, verbs, stances) is shown, not edited: its
     command changes it.
   - *Seen at once:* the spellcaster profile, which a typed spell number reads, is read again
     when its file changes (`crates/cena/src/hunt/caster.rs`), so a change from the menu
     does not wait for a restart.
   - *Checked:* 15 mutants over the menu, the pages, the writer, the tables and the reload,
     all caught.
2. **Hydra's own page**, kept in the window's own file: the card width, and closing a play
   window when its session closes (§6 item 11, off). Also the **Keys** page: keybinds for
   every character, edited with a key pressed rather than typed.

   **BUILT 2026-09-27.**
   - *Where the menu opens:* the hub's *Settings* opens Hydra's own pages, as §4.6 proposed;
     a play window's opens its character's. The picker reaches either from the other.
   - *The Window page:* `window.toml` in the data folder (`crates/cena-gui/src/own.rs`),
     drawn as any other page and changed by the window itself, no behavior being behind it.
     The card width is read at start, kept when a drag of a card's side lets go, and can
     be typed. *Close a play window when its session closes* is off by default. When on,
     it closes the window once, when the session closes; a window opened again from its
     card stays open.
   - *The Keys page:* `crates/cena-gui/src/keys/page.rs`. *Add a key*, press it, then
     type the line it sends; a bound key's own button, pressed, waits for another key to
     move the line to. Escape stops the wait. A key that types, or one already bound, is
     refused and said.
   - *The numpad:* while the page waits, the window opens the fork's channel to every
     numpad key and hands the press to the page, so a numpad key is never taken for its
     digit. Its *NumLock on too* switch is `numpad = "always"`.
   - *keybinds.toml gets a writer:* `crates/cena-gui/src/keys/write.rs`. It changes the
     file a line at a time, so a player's comments and order are kept, and reads the
     result back before saving. A file it cannot change in place (an inline `[keys]`
     table, say) is refused, not mangled. A change binds at once, with no reload.
   - *One atomic text write:* `cena_session::store::save_text`. The JSON stores, the
     behaviors' TOML and both of these files go through it.
   - *Checked:* 29 mutants over both pages, the writer and the window's handling, all
     caught once a test wrote a chord holding Shift and Alt: swapping their names had
     passed.
3. **The character's General page:** the command symbol, the player-log feeds, the sorter
   (saved), and recording per kind (off until turned on, then kept on). The settings file
   gets its writer, and `--record` and `--no-record` retire.

   **BUILT 2026-09-27.**
   - *Three pages over the file:* `<instance>_<character>.settings.json`, the store's own
     file (`crates/cena/src/general.rs`), shown before the behaviors' pages. Each page says
     when a change takes effect:
     - *General*: the command symbol and container-look sorting. Both reach a running
       character at once.
     - *Player log*: a switch per feed, the text feeds on and the readouts off by default,
       and any stream the file names. Read at the next login.
     - *Recording*: combat and loot, each off until turned on. Read at the next login.
   - *One writer:* each section is read and written whole through
     `cena_session::settings_store`, so a section this build does not know rides along.
     A file that cannot be trusted is shown with why and never written over. A symbol must
     be one mark, not a letter, a digit or a space.
   - *`;sorter` saved:* it writes the `sorter` section through the same writer, and a
     character starts sorting as it was left (`crates/cena/src/sorter.rs`).
   - *Recording:* `crates/cena/src/setup.rs` reads the `record` section when the character
     starts, opening the combat recorder, the loot ledger, both or neither; the ledger no
     longer needs the combat recorder to be on. `--record` and `--no-record` decide nothing
     and are said to be retired when given. **This changes a debug build's default**:
     recording was on in debug builds and is now off until turned on, the author's one
     default for both builds.
   - *Checked:* 15 mutants over the pages, the writer, `;sorter`, the recording switches and
     the retired flags. All were caught once a test put a player log feed back to its default:
     that had passed. Not tested: the hub handing a changed symbol or sorter to a running
     character (`Table::handle_of` in `crates/cena/src/play.rs`). It is one lookup over what
     `Kept::take` does, and `Kept::take` is tested.
4. **Travel**, typed: the keys the code reads, and the map's own keys as a free list.

   **BUILT 2026-09-27.**
   - *The page:* `crates/cena/src/travel_page.rs`, after the character's own pages. First the
     nine settings travel's code reads, each with its kind. They are named once, in
     `crates/cena-behavior/src/travel/settings.rs` (`TABLE`), and the code reads them by
     those names now, not by strings of its own. Then the map's own settings as words, and
     any other the file holds. A switch is written `true` or `false`, as go2 keeps it. The
     next trip reads a change.
   - *The map's own settings, listed:* §2 item 2 said the full list *"cannot be enumerated
     from the source"*. It can from the loaded map: `Map::setting_names`
     (`crates/cena-map/src/map.rs`) reads each exit as it is written, finding every guard's
     `setting`, `setting_is_set` and `wearing_named_by`, every `moves_from_setting`, and
     each `{setting:...}` placeholder. A crossing added later is covered without the scan
     knowing its shape. The binary finds them once, the first time the menu asks
     (`map_context::settings`).
   - *A trip no longer writes the settings back:* a trip read the settings when it started
     and saved them, with its memories, when it ended. A change made from the menu between
     the two would have been put back. `travel_store::save` now writes the memories and
     the last room only, and `travel_store::set_setting` writes one setting at a time, under
     the store's lock (`crates/cena-session/src/travel_store.rs`).
   - *Checked:* 15 mutants over the scan, the store, the page and two of the named keys, all
     caught. Not tested: the binary finding the map's settings once (`map_context::settings`),
     a lookup over what `Map::setting_names` does.
5. **Loot**, the loot profile's page.

   **BUILT 2026-09-27.**
   - *Three pages over one file.* The loot profile (`hunt/loot/<ic>.toml`) was written only by
     `;hunt import-loot`. It is now three pages, each a table beside its struct, through the
     writer `;heal set` uses (`crates/cena/src/hunt/settings.rs`, `profiles`):
     - *Loot*: its own ten settings (`crates/cena-behavior/src/loot/profile.rs`, `TABLE`).
     - *Skinning*: `[skin]`'s eleven (`SKIN_TABLE`).
     - *Selling*: the seventeen `[town]` keys the selling round reads, under eloot's own
       names (`crates/cena-behavior/src/town/settings.rs`, `TABLE`). The disk is not among
       them: it is Loot's own `disk`.
   - *Dotted keys:* `settings::shown` finds a dotted key's value (`skin.enable`), and the
     writer already set one.
   - *Values in effect:* `LootProfile::to_toml_whole` writes the profile out whole for the
     pages. Skinning is shown even when off. Selling is shown as the round reads it, through
     `Town::to_table`, `Town::from_table`'s inverse, which a test holds to it by a round trip.
     A limit that is none is left out, and shown unset.
   - *Not shown:* the `[town]` keys the importer carries from eloot that nothing reads. They
     stay in the file as they were.
   - *Checked:* 10 mutants over the dotted lookup, the whole profile, the town's table and
     the three pages, all caught once the town's round trip used towns whose switches differ:
     with every switch on, two swapped names would have read back the same.
6. **Hunt:** first the chain in effect, each value with where it came from; then its pages,
   one per table, with *check* beside *save*.

   **BUILT 2026-09-27**, both halves in one step.
   - *Where each value came from:* the chain names its levels (`hunt::chain::Level`: built in,
     global, the profile, the character's file). `chain::origins` lists every setting in
     effect, by its dotted name, with the level that set it last
     (`crates/cena-behavior/src/hunt/chain.rs`).
   - *A page per profile:* `crates/cena/src/hunt_pages.rs`. It shows every setting in
     effect, and where each came from in the column that said *default* (`Row::from`, new
     in `cena_ui::settings`). The rows come from the profile's own table, so a setting
     added to the hunt appears without being listed. Each value's kind comes from the
     value itself. An empty list's kind is found by trying one number against the profile's
     real type (`chain::holds_numbers`): a list of rooms asks for numbers, a list of
     commands for words.
   - *Check beside save:* a change is written into the profile through `;hunt set`'s own
     writer, refactored to return what it did (`hunt::settings::edited`). It loads the
     profile as this character would after saving. A change that makes the profile
     unreadable is put back. A change that saves but would stop the hunt running is saved,
     and why it would not run is said with it.
   - *What the character's own file sets* is shown, not changed. That file wins over the
     profile and has no writer yet.
   - *Claude's call, for the author to change:* one page per profile, with a heading over
     each table's settings, not a page per table. A profile has about fifteen tables, and a
     page for each, for each profile, would make the menu's list of pages longer than the
     rest of it together. The headings come from the keys (`rooms.hunting` is under
     *rooms*), and appear only on a page whose settings are in more than one table.
   - *Checked:* 15 mutants over the levels, the listing, the kinds, the writer's check and
     the menu's headings. All were caught once a test showed a page in one table has no
     heading: a heading over Skinning's `[skin]` had passed.
7. **Layouts by game and name**, a name-only layout taken as the first. Then **the fixed
   data folder**, the player's own application-data folder, the old `data` copied into it
   once and left with a note.

   **BUILT 2026-09-27.**
   - *Layouts by game and name:* a play window's layout is `layouts/<instance>_<name>.json`,
     so Nisugi on Prime and a Nisugi on Shattered each keep their own. A layout saved under
     the name alone, as they all were, is taken as the first, and saving keeps the game's own
     from then (`crates/cena-gui/src/layout.rs`). The window learns the game when the binary
     seats the character (`Sessions::attach` takes it).
   - *The data folder is fixed:* `CENA_DATA_DIR`, otherwise `data` in Hydra's own folder in
     the player's application data (`character_store::data_dir`, `app_dir`):
     - `%APPDATA%\Hydra` on Windows;
     - `~/Library/Application Support/Hydra` on macOS;
     - `$XDG_DATA_HOME/hydra` or `~/.local/share/hydra` elsewhere.

     It is not beside the program, which the installer's updates (Stage H) will replace.
   - *The one-time copy:* when Hydra starts, the old `data` where it was started is copied in
     once (`character_store::settle`, `move_in`). The copy is made whole under its own name
     and then renamed, and the old folder is left as it was with a note, `MOVED.txt`, saying
     where its data went. Until the copy is made the old folder is still the one read, so a
     copy that fails never starts a player with nothing.
   - *Not moved:* the logs. `CENA_LOG_DIR`'s default is still `logs` where Hydra was
     started; §6 item 10 named the data folder.
   - *Checked:* 11 mutants over the folder, the copy and the layouts. All were caught once a
     test showed a play window taking its game's layout; the window's game could have been
     dropped. That test's first form looked for a widget's title, which is labelled only
     while arranging, and so passed either way.
8. **The ways in:** a widget's right-click opens the one menu at its setting.

   **BUILT 2026-09-27.** Every step of Stage D is built.
   - *A widget's right-click* offers *Settings...* where a setting governs it. It opens the one
     menu on the widget's character, at that page (`Widget::settings_page`,
     `crates/cena-gui/src/widget/kind.rs`):

     | Widget | Page |
     |---|---|
     | the story | *General* |
     | health | *Heal* |
     | the hands and the containers | *Loot* |
     | the room and its exits and compass | *Travel* |
     | the hunt panel and the stance | the first hunt page |
     | the active effects | *Keep* |
     | the spellbook and what is prepared | *Spellcaster* |
     | combat | *Recording* |

     Any other widget has no setting of its own and offers none.
   - *The Keys menu* on a play window's top bar has *Change the keys...*, which opens
     Hydra's *Keys* page.
   - *At the page:* the menu opens at a page by its id, or by the start of one, `hunt:`
     being the first hunt page (`Menu::open_at`).
   - *Claude's call, for the author to change:* which widget opens which page.
   - *Checked:* 9 mutants over the mapping, the menu item, the Keys menu, the page opening
     and the app's wiring, all caught.

   > **CORRECTED by the author, 2026-09-28**, after running it: *"on health bar takes you to
   > the heal behavior settings? It should take you to settings to edit that bar, is it
   > horizontal or vertical, is the text inside it or outside it, is it just a percent, or
   > just current/max, or text label plus numbers, or any combination, what color is it?
   > what overlay am I using (if I want one)."* The table above is retired.
   >
   > A widget's right-click *Settings...* now opens the one menu at **that widget's own
   > page** (`crates/cena-gui/src/play/options.rs`). The pages come from the play window's
   > layout, listed after the character's other pages, and a change is made in the layout
   > and saved with it.
   >
   > A bar's page (the four vitals and the pulse) holds its `Look`:
   > - which way it fills;
   > - where its text goes;
   > - whether it says its label, current/max and percent, in any mix;
   > - its colour;
   > - an overlay: any PNG in the data folder's `overlays`, stretched over it.
   >
   > The menu gained a choice row and a colour row (`RowKind::Choice`, `RowKind::Color`). A
   > widget with no settings of its own offers no *Settings...*. A framed, nine-slice overlay
   > needs each image's border sizes, and waits for skins (`plan/49` Stage F).
   >
   > Checked: 8 mutants over the pages, the menu and the app's wiring, 7 caught once a test
   > saved a change with the layout. Not tested: the colour button itself asking, egui's
   > own picker, which the harness cannot drive. It is one line, `if picked.changed()`.
   >
   > **The room's own page, BUILT 2026-09-28.** The author, of the same menu: *"on room it
   > takes you travel ... it should take you to pick which streams show in the room window
   > (title, description, room objects, room players, room exits)"*. They also asked that it
   > be displayed as Wrayth draws it:
   >
   > - `[Rawknuckle's, Watering Hole] (7503251)`;
   > - the description, with *You also see ...* run on after it;
   > - *Also here:* when anyone is;
   > - the exits.
   >
   > They asked as well for *"the option to take the objects and break it up into creatures
   > / objects based on the pushBold wrapping it"*.
   >
   > It is a widget of its own, **Room** (`crates/cena-gui/src/widget/described.rs`), and the
   > first layout's Room window holds it in place of the custom window of parts. Its page has
   > a switch for each of title, description, objects, creatures, players and exits, and
   > *Creatures apart*. With that last switch on, the description stands alone, followed by a
   > *You also see:* line and a *Creatures:* line. The game's bold decides which is which: in
   > the joined sentence a bold run is drawn in the creatures' colour, and apart the lines come
   > from the model's objects and creatures, which the same bold sorted.
   >
   > What each widget shows is kept in the layout by the widget's id (`Layout::rooms`, beside
   > `looks`), and goes with the widget or its window.
   >
   > A layout saved before this keeps its custom window of parts; *Lay out afresh*, or the Room
   > widget added from the list, gives the new one.
   >
   > **A fixture corrected on the way.** The GUI's test snapshot named the room
   > `[Rawknuckle's, Watering Hole]`, brackets and all, which the wire never sends: the model
   > keeps the subtitle less its leading ` - ` (`crates/cena-model/src/state/room.rs:519`). The
   > Room widget adds brackets and would have drawn two pairs, with every test green.
   >
   > Checked: 14 mutants over the joining, each switch, the title's number, the creature
   > colour, the page, putting a part back and removal. 12 were caught at first. The two
   > survivors were real gaps, *objects* off while apart, and a custom window removed with a
   > Room widget in it; each now has a test that catches it.
