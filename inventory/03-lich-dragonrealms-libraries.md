# Lich-5 DragonRealms Libraries — Porting Inventory

**Date:** 2026-09-17
**Scope:** Every file under `E:/Cena/reference/lich-5/lib/dragonrealms/` (34 `.rb` files, 13,033 LOC), plus `lib/common/map/map_dr.rb`, `map_gs.rb` and `map_base.rb` (1,893 LOC), and the game-branching call sites in `lib/common/`, `lib/attributes/`, `lib/gemstone/`, `lib/global_defs.rb`, `lib/lich.rb`, `lib/messaging.rb` and `lib/games.rb`. Consumer counts were measured against `E:/Cena/reference/dr-scripts` (222 `.lic` files).

All line numbers below were obtained with `grep -n` / `sed -n` against the working tree. Paths are relative to `E:/Cena/reference/lich-5/`.

---

## 1. Tree and sizes

`wc -l` over `lib/dragonrealms/` + `lib/common/map/`:

| File | LOC |
|---|---:|
| `lib/dragonrealms/commons/common-items.rb` | 2058 |
| `lib/dragonrealms/commons/common-arcana.rb` | 1373 |
| `lib/dragonrealms/commons/common.rb` | 1093 |
| `lib/common/map/map_base.rb` | 954 |
| `lib/dragonrealms/commons/equipmanager.rb` | 947 |
| `lib/dragonrealms/commons/common-crafting.rb` | 815 |
| `lib/dragonrealms/drinfomon/drparser.rb` | 725 |
| `lib/dragonrealms/commons/common-healing.rb` | 520 |
| `lib/common/map/map_gs.rb` | 520 |
| `lib/dragonrealms/commons/common-moonmage.rb` | 492 |
| `lib/dragonrealms/commons/common-travel.rb` | 424 |
| `lib/common/map/map_dr.rb` | 419 |
| `lib/dragonrealms/creature.rb` | 416 |
| `lib/dragonrealms/commons/slackbot.rb` | 392 |
| `lib/dragonrealms/drinfomon/drvariables.rb` | 372 |
| `lib/dragonrealms/drinfomon/drbanking.rb` | 354 |
| `lib/dragonrealms/commons/common-theurgy.rb` | 332 |
| `lib/dragonrealms/drinfomon/drdefs.rb` | 307 |
| `lib/dragonrealms/drinfomon/drstats.rb` | 292 |
| `lib/dragonrealms/commons/common-healing-data.rb` | 278 |
| `lib/dragonrealms/drinfomon/drskill.rb` | 251 |
| `lib/dragonrealms/commons/common-summoning.rb` | 247 |
| `lib/dragonrealms/commons/common-money.rb` | 210 |
| `lib/dragonrealms/drinfomon/drexpmonitor.rb` | 209 |
| `lib/dragonrealms/drinfomon/startup.rb` | 196 |
| `lib/dragonrealms/custom_substitutions.rb` | 120 |
| `lib/dragonrealms/dependency/settings_config.rb` | 103 |
| `lib/dragonrealms/commons/common-validation.rb` | 102 |
| `lib/dragonrealms/detachable_client_init.rb` | 94 |
| `lib/dragonrealms/drinfomon/drroom.rb` | 85 |
| `lib/dragonrealms/commons/common-money-data.rb` | 75 |
| `lib/dragonrealms/drinfomon/drspells.rb` | 67 |
| `lib/dragonrealms/drinfomon/events.rb` | 40 |
| `lib/dragonrealms/drinfomon.rb` | 29 |
| `lib/dragonrealms/commons.rb` | 15 |
| **Total (DR tree)** | **13,033** |
| **+ `lib/common/map/` (3 files)** | **1,893** |

Two load roots: `lib/dragonrealms/drinfomon.rb:9-19` requires the ten `drinfomon/` files; `lib/dragonrealms/commons.rb:1-15` requires the fifteen `commons/` files. Both are pulled in by `lib/common/gameloader.rb:63-67`, which is reached only from `GameLoader.load!` (`gameloader.rb:88-91`):

```ruby
sleep 0.1 while XMLData.game.nil? or XMLData.game.empty?
return self.dragon_realms if XMLData.game =~ /DR/
return self.gemstone if XMLData.game =~ /GS/
```

That `if XMLData.game =~ /DR/` at `gameloader.rb:89` is the **primary game seam in the entire codebase**: it decides which of two disjoint library sets is ever loaded into the process. Everything else listed in §7 is a secondary branch inside code that both games share.

---

## 2. DRInfomon — DragonRealms character state

DRInfomon is DR's answer to GemStone's `Infomon`, but it is architecturally the opposite. `lib/gemstone/infomon.rb:86` keys a SQLite table per `"#{XMLData.game}_#{XMLData.name}"`; DRInfomon has **no database at all**. Every DR state module is a bare Ruby module holding `@@` class variables, populated by regex matches against raw server text.

Version string: `$DRINFOMON_VERSION = '3.0'` (`drinfomon.rb:4`).

### 2.1 State modules

| Module | File | Storage | What it holds |
|---|---|---|---|
| `DRStats` | `drinfomon/drstats.rb` | 17 `@@` vars (`drstats.rb:6-24`) | race, guild, gender, age, circle, 8 stats, favors, tdps, encumbrance, balance, position, luck |
| `DRSkill` | `drinfomon/drskill.rb` | `@@list` array of `DRSkill` instances (`drskill.rb:9`) | per-skill rank / mindstate / percent / baseline; `@@exp_modifiers`; rested-exp triple |
| `DRRoom` | `drinfomon/drroom.rb` | 7 `@@` arrays (`drroom.rb:6-13`) | pcs, npcs, dead_npcs, room_objs, group_members, pcs_prone, pcs_sitting |
| `DRSpells` | `drinfomon/drspells.rb` | 3 hashes + 3 booleans (`drspells.rb:6-12`) | known_spells, known_feats, spellbook_format, three scrape-in-progress flags |
| `Flags` | `drinfomon/events.rb` | `@@flags`, `@@matchers` (`events.rb:6-7`) | script-registered regex watchers; the DR event bus |
| `DRBanking` | `drinfomon/drbanking.rb` | JSON via `load_accounts`/`save_accounts` (`drbanking.rb:285,292`) | per-town coin balances, passively scraped |
| `DRExpMonitor` | `drinfomon/drexpmonitor.rb` | thread + toggles | real-time exp gain reporting; rewrites EXP window lines in place |

`DRStats` is **not fully independent** — six of its readers delegate straight to `XMLData` (`drstats.rb:181-203`):

```ruby
def self.name;          XMLData.name; end
def self.health;        XMLData.health; end
def self.mana;          XMLData.mana; end
def self.fatigue;       XMLData.stamina; end     # note: DR "fatigue" == GS "stamina" bar
def self.spirit;        XMLData.spirit; end
def self.concentration; XMLData.concentration; end
```

Likewise `DRRoom.exits/title/description` delegate to `XMLData.room_exits/room_title/room_description` (`drroom.rb:32-42`), and `DRSpells.active_spells/slivers/stellar_percentage` delegate to `XMLData.dr_active_spells*` (`drspells.rb:14-32`).

**This is the line Cena has to cut cleanly.** The vitals bars, the room title/description/exits and the active-spell percWindow are genuine XML in DR and arrive through the same `XMLData` object GemStone uses. Everything else on that list is text-scraped.

### 2.2 XML vs text: where DR state actually comes from

`drinfomon/drparser.rb` contains 47 `Pattern::` constants (`drparser.rb:9-91`) and one dispatch method, `DRParser.parse(line)` (`drparser.rb:544-722`), a 25-branch `elsif` chain over **raw text lines**, not XML nodes.

| DR state | Source | Evidence |
|---|---|---|
| health / mana / fatigue / spirit / concentration | **XML** (`<dialogData>` vitals, shared with GS) | `drstats.rb:185-203` delegates to `XMLData` |
| room title / description / exits | **XML** (`<streamWindow>` / room component) | `drroom.rb:32-42` |
| active spells + durations, slivers, stellar % | **XML** (`percWindow` stream) | `xmlparser.rb:644-651`, `:663-670`, `:1014`, `:1024`, `:1028` |
| room players | **Text inside an XML component**, `"Also here: …"` | `drparser.rb:21` `RoomPlayers`, dispatched `:587-591` |
| room NPCs and objects | **Text + `<pushBold/>` markers** inside `'room objs'` component | `drparser.rb:23` `RoomObjs`, dispatched `:592-597`; bold scan in `drdefs.rb:31` `NPC_SCAN` |
| creature combat flags | **XML** `<crtrStatus exist=… hostile=1 immobile=1/>` — **id-keyed but name-less** | `creature.rb:11-14`; `xmlparser.rb:503-511` |
| creature *names* tied to ids | **Text** in the `assess` stream | `creature.rb:18-21`; `xmlparser.rb:571-573` |
| race / guild / gender / age / circle | **Text** from the `INFO` verb | `drparser.rb:12-13`, dispatched `:563-569` |
| 8 primary stats, Favors, TDPs | **Text** `"Strength  : 100"` | `drparser.rb:14` `StatValue`, dispatched `:574-577` |
| encumbrance | **Text** `"Encumbrance : Light Burden"` | `drparser.rb:16`, dispatched `:570-571` |
| luck | **Text** `"Luck : … (2/3)"` | `drparser.rb:17`, dispatched `:572-573` |
| **balance** | **Text** `"[You're solidly balanced…"` | `drparser.rb:18` `BalanceValue`, dispatched `:580-581` |
| **combat position** | **Text**, same status line | `drparser.rb:19` `PositionValue`, dispatched `:582-584` |
| skill ranks + mindstates | **Text inside `<component id='exp …'>`** (three different formats) | `drparser.rb:9-11`, dispatched `:606-631` |
| exp modifiers | **Text** block after a header line | `drparser.rb:27-28`, dispatched `:632-634`, consumed by `check_exp_mods` `:313` |
| rested exp | **Text inside `<component id='exp rexp'>`** | `drparser.rb:36-37`, dispatched `:679-683` |
| known spells / barbarian abilities / thief khri | **Text**, three separate multi-line scrapers | `drparser.rb:363`, `:465`, `:511` |
| inventory item ids | **Text scrape of `<d cmd='get #12345'>` links** during `INV LIST` / `INV SEARCH` | `drparser.rb:56-57`, `:192`, `:544-562` |
| group members | **Text inside `<pushStream id="group"/>`** | `drparser.rb:25-26`, dispatched `:602-605` |
| bank balances | **Text**, passively on every line | `drbanking.rb:25-52`, called from `drparser.rb:715` |
| account name + subscription tier | **Text** from the `PLAYED` verb | `drparser.rb:33-34`, dispatched `:651-664` |
| scheduled server shutdown | **Text** announcement | `drparser.rb:66`, `check_game_shutdown` `:129-134` |

**Bottom line for Cena's parser design:** in GemStone, a character sheet is a database read. In DragonRealms, a character sheet only exists because DRInfomon fires four verbs at login and regex-matches the replies. `DRInfomon.startup_script` (`drinfomon/startup.rb:50-84`) is literally a heredoc of Ruby that issues `info`, `played`, `exp all 0` and `ability`, each wrapped in `Lich::Util.issue_command` with a start pattern and an end pattern. Without those four round-trips, `DRStats.guild` is `nil`.

Two additional mechanics that Cena's parser must reproduce:

1. **Line mutation.** `drparser.rb:618` and `:621` call `line.replace(...)` — DRExpMonitor rewrites the EXP-window line *in place* so the client renders cumulative gains inline. The parser is not a pure observer; it is in the render path.
2. **Statefulness across lines.** Three module-level flags (`@@parsing_exp_mods_output`, `@@parsing_inventory_get`, `@@inventory_partial` — `drparser.rb:95-99`) make the parser a mode machine. The inventory scrape distinguishes COMPLETE (`INV LIST` → `GameObj.begin_inv` then `commit_inv`) from PARTIAL (`INV SEARCH` → upsert, no clear) at `drparser.rb:548-562`, with a safety valve at `:694-706` that discards a staged replacement if a `<prompt>` arrives before the closing `<output class=""/>`.

### 2.3 DR data tables that have no GemStone counterpart

`drinfomon/drvariables.rb` is pure data, 372 lines, and is the DR world model:

- `DR_LEARNING_RATES` — 35 ordered mindstate names, `'clear'` … `'mind lock'` (`drvariables.rb:5-41`). Index into this array *is* the numeric mindstate; `drparser.rb:610` and `:629` do `DR_LEARNING_RATES.index(rate_word)`.
- `DR_BALANCE_VALUES` — 12 ordered balance words (`drvariables.rb:46-59`); `DRStats.balance` is an **index**, defaulting to 8 = `'solidly'` (`drstats.rb:22`).
- `DR_POSITION_VALUES` — 20 phrases → signed −9…+9 (`drvariables.rb:67-88`). Two phrases both map to 9.
- `DR_SKILLS_DATA` — 5 skillsets (Armor/Lore/Weapon/Magic/Survival) totalling 70 skills, plus `guild_skill_aliases` mapping each of 11 guilds' `'Primary Magic'` to its real name (`drvariables.rb:90-186`).
- Three currency zones: `KRONAR_BANKS` (4 towns), `LIRUM_BANKS` (9), `DOKORA_BANKS` (7) (`drvariables.rb:188-190`); `BANK_TITLES` 20 towns → room-title strings (`:192-213`); `VAULT_TITLES` 9 towns (`:215-225`).
- `HOMETOWN_REGEX_MAP` — 28 towns, each with an abbreviation regex (`drvariables.rb:236-264`), unioned into `HOMETOWN_REGEX` (`:270`).
- `ENC_MAP` — 12 encumbrance strings → 0-11 (`drvariables.rb:276-289`), ending with `"It's amazing you aren't squashed!"`.
- `MANA_MAP` — 4 perception tiers each with an ordered adjective list (`drvariables.rb:334-339`), used by `DRCA.parse_mana_message` (`common-arcana.rb:761`).
- `VOL_MAP` — 10 size adjectives → volume integers (`drvariables.rb:344-355`).
- `BOX_WOODS` (11) / `BOX_CONTAINERS` (9) composed into `BOX_REGEX` (`drvariables.rb:325-332`).
- Sigil patterns: 5 primary, 10 secondary (`drvariables.rb:341-342`).

`drvariables.rb:359-370` re-exports eleven of these as **global variables** (`$HOMETOWN_LIST`, `$ENC_MAP`, `$box_regex`, `$MANA_MAP`, `$VOL_MAP`, …) explicitly "for third-party scripts." Cena's Lua layer must decide whether to reproduce that global namespace or force a module path; 222 dr-scripts reference these.

---

## 3. The `commons/` libraries — the dr-scripts support layer

All ten functional commons files use `module_function` (`common.rb:11`, `common-arcana.rb:6`, `common-crafting.rb:6`, `common-healing.rb:6`, `common-items.rb:35`, `common-money.rb:6`, `common-moonmage.rb:6`, `common-summoning.rb:8`, `common-theurgy.rb:6`, `common-travel.rb:6`), so every `def foo` is a public module method called as `DRC.foo`.

Consumer counts — occurrences of `<Module>.` across all 222 files in `E:/Cena/reference/dr-scripts/*.lic`:

| Module | File | LOC | Public methods | Call sites in dr-scripts |
|---|---|---:|---:|---:|
| `DRC` | `commons/common.rb` | 1093 | 52 | **3861** |
| `DRCI` | `commons/common-items.rb` | 2058 | 63 | **942** |
| `DRCT` | `commons/common-travel.rb` | 424 | 14 | 460 |
| `DRCC` | `commons/common-crafting.rb` | 815 | 30 | 434 |
| `DRSkill` | `drinfomon/drskill.rb` | 251 | — | 312 |
| `DRStats` | `drinfomon/drstats.rb` | 292 | — | 385 |
| `DRRoom` | `drinfomon/drroom.rb` | 85 | — | 286 |
| `DRCA` | `commons/common-arcana.rb` | 1373 | 66 | 174 |
| `DRCM` | `commons/common-money.rb` (+`-data`) | 210+75 | 15 | 133 |
| `DRSpells` | `drinfomon/drspells.rb` | 67 | — | 96 |
| `DRCMM` | `commons/common-moonmage.rb` | 492 | 30 | 77 |
| `EquipmentManager` | `commons/equipmanager.rb` | 947 | 30 | 56 |
| `DRCH` | `commons/common-healing.rb` (+`-data`) | 520+278 | 18 | 34 |
| `DRCTH` | `commons/common-theurgy.rb` | 332 | 22 | 19 |
| `DRCS` | `commons/common-summoning.rb` | 247 | 11 | 14 |
| `CharacterValidator` | `commons/common-validation.rb` | 102 | 8 | 3 |
| `SlackBot` | `commons/slackbot.rb` | 392 | 14 | 1 |
| `DRBanking` | `drinfomon/drbanking.rb` | 354 | 24 | 0 (user-facing `;banks`) |

### 3.1 `DRC` (common.rb) — the workhorse

`DRC.bput` (`common.rb:102-207`) is called more than any other function in the DR ecosystem. It is **not** a thin `fput`; it is a 105-line retry/recovery state machine:

- Waits for roundtime first (`common.rb:117`, `waitrt?`) unless `ignore_rt`.
- Default timeout 15 s (`common.rb:104`).
- Converts every non-Regexp match argument to a case-insensitive regex (`common.rb:120`).
- Then handles **nine distinct game-failure classes** by fixing the condition and re-sending the command (`common.rb:133-190`): world-frozen, `...wait N` (`WAIT_RESPONSE_PATTERN`, `common.rb:19`), typeahead limit, asleep → `wake`, performing → `stop play`, hiding → `release_invisibility` + `unhide`, stunned → poll `stunned?`, webbed → poll `webbed?`, and seven variants of not-standing → `fix_standing` (`common.rb:552`).
- Returns `result.to_a.first` (`common.rb:194`) — the *matched string*, not a boolean. On timeout it returns `''` after dumping the whole log to the client (`common.rb:199-206`).

Other notable `DRC` members:

- `DRC::Item` (`common.rb:467-529`) — a 17-keyword-argument value class (`common.rb:470`) describing a wearable/wieldable: `leather`, `worn`, `hinders_locks`, `swappable`, `tie_to`, `transforms_to`, `transform_verb`, `lodges`, `ranged`, `needs_unloading`… with `Item.from_text` (`common.rb:509`) to build one from a settings string.
- `FLAVOR_TEXT_PATTERN` (`common.rb:76`) — a **single regex literal that is roughly 6,000 characters long**, enumerating hundreds of DR item-description adjectives and participles ("acid-etched|adorned|affixed|appliqued…"). It is used by `remove_flavor_text` (`common.rb:460`) to reduce a decorated item name to something a `get`/`stow` command can address. This is the single hardest artifact in the DR tree to port faithfully.
- `list_to_array` (`common.rb:318`), `box_list_to_adj_and_noun` (`common.rb:347`), `scroll_list_to_adj_and_noun` (`common.rb:419`), `get_noun` (`common.rb:440`) — DR item-name normalization, all pure text.
- Bard instrument automation: `play_song?` (`common.rb:720`), `clean_instrument` (`common.rb:803`), `tune_instrument` (`common.rb:857`), `do_tune` (`common.rb:892`).
- Script coordination: `pause_all` / `unpause_all` / `smart_pause_all` / `safe_pause_list` / `safe_unpause_list` (`common.rb:917-1022`) — these reach into Lich's `Script` registry and are how DR scripts avoid clobbering each other.
- Output: `atmo` (`common.rb:1054`), `log_window` (`common.rb:1060`), `bold` (`common.rb:1079`), `message` (`common.rb:1088`).
- `CANNOT_STAND_PATTERN` (`common.rb:83`), `COLLECT_MESSAGES` (`:22`), `RETREAT_MESSAGES` (`:51`), `RETREAT_ESCAPE_MESSAGES` (`:43`), `ASSESS_TEACH_*` patterns (`:63-64`), `COMMON_RANGED_WEAPONS_PATTERN` (`:67`), `RACIAL_RANGED_WEAPONS_PATTERN` (`:72`, 21 racial weapon nouns).

### 3.2 `DRCI` (common-items.rb) — 2058 LOC, 63 methods

The largest single file. Everything about moving items: `get_item?`/`get_item_safe?`/`get_item_unsafe` (`common-items.rb:1182,1211,1249`), `stow_item?` + safe/unsafe variants (`:1433,1442,1455`), `wear_item?`/`remove_item?` + variants (`:1348-1420`), `put_away_item?` + variants (`:1705-1736`), `tie_item?`/`untie_item?` (`:1306,1323`), `open_container?`/`close_container?` (`:1771,1785`), `give_item?`/`accept_item?` (`:1811,1856`).

Query side: `in_hands?`, `in_left_hand?`, `in_right_hand?`, `in_hand?` (`:859-897`), `wearing?` (`:806`), `inside?` (`:815`), `exists?` (`:828`), `have_item_by_look?` (`:924`), `container_is_empty?` (`:1539`).

Counting: `count_item_parts` (`:968`), `count_items` (`:1010`), `count_items_in_container` (`:1025`), `count_lockpick_container` (`:1040`), `count_necro_stacker` (`:1066`), `count_all_boxes` (`:1078`).

Gem-pouch automation is a whole subsystem: `check_belt_for_pouch?` (`:1875`), `tie_gem_pouch?` (`:1899`), `remove_and_stow_pouch?` (`:1917`), `swap_out_full_gempouch?` (`:1937`), `fill_gem_pouch_with_container` (`:1993`, `retries: 10`).

`dispose_trash` (`:664`) is a 110-line method with a `retries: 3` parameter and a companion `execute_dispose_command` (`:774`).

**Port note:** almost every method here is `command → wait for one of N response patterns → branch`. The `?`/`_safe?`/`_unsafe?` triads (`get_item?` calls `get_item_safe?` which calls `get_item_unsafe`) encode a retry-and-recover policy that is data, not logic — a strong candidate for a declarative table in Cena rather than three Rust or Lua functions per verb.

### 3.3 `DRCA` (common-arcana.rb) — 1373 LOC, 66 methods

DR magic. `prepare?` (`common-arcana.rb:391`) takes **nine positional arguments plus three keywords** — abbrev, mana, symbiosis, command, tattoo_tm, runestone_name, runestone_tm, custom_prep_message, `custom_invoke_message:`, `custom_spell_prep:`, `retries:`. `cast?` (`:514`) takes six. The cambrinth (mana-storage item) subsystem is 14 methods: `find_cambrinth` (`:613`), `charge?` (`:693`), `invoke` (`:670`), `allocate_cambrinth_charges` (`:1039`), `usable_cambrinth_indexes` (`:1061`), `cambrinth_charge_counts` (`:1071`), `split_cambrinth_charges` (`:1091`), `distribute_cambrinth_charges` (`:1109`), `cambrinth_caps` (`:1133`), `stale_cambrinth_caps?` (`:1141`), `normalize_cambrinth_items` (`:1299`), `charge_cambrinth_items` (`:1309`), `charge_cambrinth_items_repeated` (`:1319`), `charge_cambrinth_items_by_item` (`:1332`), plus `spread_flat_cambrinth_charges` (`:1359`).

Guild-specific entry points: `start_khris`/`activate_khri?`/`kneel_for_khri?` (Thief, `:195-235`), `start_barb_abilities`/`activate_barb_buff?` (Barbarian, `:243-247`), `parse_regalia`/`shatter_regalia?` (Bard, `:739-749`), `update_avtalia`/`invoke_avtalia`/`charge_avtalia`/`choose_avtalia` (`:1237-1268`).

Mana reading is text: `parse_mana_message` (`:761`) against `MANA_MAP`; `perc_mana` (`:777`), `perc_aura` (`:809`), `perc_symbiotic_research` (`:1288`).

`CYCLIC_RELEASE_SUCCESS_PATTERNS` (`common-arcana.rb:8`) is a hand-curated list of per-spell release messages across Ranger, Empath and Bard spell lists.

### 3.4 `DRCT` (common-travel.rb) — 424 LOC, 14 methods

`walk_to(target_room, restart_on_fail = true, retry_depth: 0)` (`common-travel.rb:178-274`) is the DR movement primitive. Notably it **shells out to another Lich script**: `start_script('go2', [room_num.to_s], force: true)` (`:218`), then supervises it with `Flags`:

```ruby
Flags.add('travel-closed-shop', 'The door is locked up tightly for the night', 'You smash your nose', '^A servant (blocks|stops)')
Flags.add('travel-engaged', 'You are engaged')
```
(`common-travel.rb:223-224`) and polls `while Script.running.include?(script_handle)` (`:227`), killing and restarting `go2` when a flag trips (`:228-236`). Unknown-room recovery re-matches against `Map.list` by title+description (`:190-215`) with `MAX_WALK_TO_RETRIES`.

Others: `tag_to_id` (`:275`), `find_empty_room` (`:314`, 7 params incl. `min_mana`, `prioritize_buddies`), `sort_destinations` (`:365`), `find_sorted_empty_room` (`:378`), `time_to_room` (`:383`), `reverse_path` (`:393`), `get_hometown_target_id` (`:404`), plus shop verbs `sell_item`/`buy_item`/`ask_for_item?`/`order_item` (`:82-123`) and `refill_lockpick_container` (`:138`).

### 3.5 `DRCC` (common-crafting.rb) — 815 LOC, 30 methods

Room-finding by hometown is the dominant pattern: `find_wheel`, `find_anvil`, `find_grindstone`, `find_sewing_room`, `find_loom_room`, `find_shaping_room`, `find_press_grinder_room`, `find_enchanting_room`, `find_empty_crucible` (`common-crafting.rb:158-270`) — each takes `hometown` and several take an `override`. `private_forge_room` (`:784`) and `towns_with_private_forge` (`:791`) encode town-specific data in code.

Recipe handling: `recipe_lookup` (`:270`), `find_recipe` (`:295`), `find_recipe2` (`:308`, adds `discipline`). Tool/material flow: `get_crafting_item` (`:324`), `stow_crafting_item` (`:365`), `crafting_cost` (`:389`), `repair_own_tools` (`:430`), `check_consumables` (`:480`), `get_adjust_tongs?` (`:498`), `count_raw_metal` (`:672`), `create_mechanisms` (`:715`). Enchanting: `order_enchant` (`:585`), `fount` (`:594`), `clean_brazier?` (`:620`), `empty_brazier` (`:634`), `check_for_existing_sigil?` (`:648`), `logbook_item` (`:559`).

### 3.6 `DRCH` (common-healing.rb + common-healing-data.rb) — 798 LOC

Two value classes, `HealthResult` (`common-healing.rb:405-440`) and `Wound` (`:442-510`). `Wound` exposes `bleeding?`, `internal?`, `scar?`, `parasite?`, `lodged?`, `tendable?`, `location`, `type`, `to_h`, `to_s` — DR wounds are a richer model than GS's simple wound/scar ranks.

Four independent text scrapers: `parse_health_lines` (`:40`), `parse_perceived_health_lines` (`:170`), `parse_bleeders` (`:245`), `parse_wounds` (`:280`), `parse_parasites` (`:308`), `parse_lodged_items` (`:329`). Actions: `bind_wound` (`:356`), `unwrap_wound` (`:379`), `skilled_to_tend_wound?` (`:386`), `calculate_score` (`:399`). Empath support: `perceive_health` (`:84`), `perceive_health_other(target)` (`:129`).

`common-healing-data.rb` is one table, `BLEED_RATE_TO_SEVERITY` (`:14-…`), mapping ~26 bleed-rate strings (`'tended'`, `'clotted(tended)'`, `'slight'`, … `'extremely severe(tended)'`) to `{severity:, bleeding:, skill_to_tend:, skill_to_tend_internal:}` (`common-healing-data.rb:15-38`). `skill_to_tend_internal` values (600, 620, 640, 660, 700) are First Aid rank thresholds. **This is pure game-balance data with no GemStone analogue.**

### 3.7 `DRCM` (common-money.rb + common-money-data.rb) — 285 LOC

DR has **three currencies with an exchange rate matrix**; GemStone has one. `common-money-data.rb` holds `DENOMINATIONS` (5 tiers, platinum=10000 copper down to copper=1, `:8-14`), `DENOMINATION_VALUES` (`:17-23`), abbreviation regexes for both denominations (`:27-33`) and currencies (`:37-41`), and `EXCHANGE_RATES` as a nested `from → to → rate` hash (`:48-…`).

Methods: `minimize_coins` (`:16`), `convert_to_copper` (`:30`), `get_canonical_currency` (`:43`), `convert_currency(amount, from, to, fee)` (`:50`), `hometown_currency` (`:58`), `town_currency` (`:63`), `check_wealth` (`:67`), `wealth` (`:71`), `get_total_wealth` (`:78`), `ensure_copper_on_hand` (`:102`), `withdraw_exact_amount?` (`:113`), `get_money_from_bank` (`:135`), `debt` (`:162`), `deposit_coins` (`:167`).

There is a **second, independent copper converter** in `drinfomon/drdefs.rb`: `convert2copper(amt, denomination)` (`drdefs.rb:66-78`) and `convert2plats(copper)` (`:89-99`), and a **third** in `DRBanking.to_copper` (`drbanking.rb:126`). Three implementations of the same conversion is a duplication Cena should collapse to one.

### 3.8 `DRCMM` (common-moonmage.rb) — 492 LOC, 30 methods

Moon Mage divination. Telescope handling (`get_telescope?`/`store_telescope?`/`peer_telescope`/`center_telescope` — `:106-170`), bones (`get_bones?`/`store_bones?`/`roll_bones` — `:174-220`), generic divination tools (`get_div_tool?`/`store_div_tool?`/`use_div_tool` — `:220-274`), summoned moon weapons (`wear_moon_weapon?`/`drop_moon_weapon?`/`holding_moon_weapon?`/`hold_moon_weapon?`/`is_moon_weapon?`/`moon_used_to_summon_weapon`/`moon_weapon_duration` — `:274-374`), and an **astronomy model**: `update_astral_data` (`:385`), `find_visible_planets` (`:394`), `set_planet_data` (`:417`), `set_moon_data` (`:433`), `bright_celestial_object?` (`:450`), `any_celestial_object?` (`:457`), `moons_visible?` (`:464`), `moon_visible?(moon_name)` (`:469`), `visible_moons` (`:475`), `check_moonwatch` (`:481`). `parse_roisaen` (`:374`) parses DR's in-game calendar. `align(skill)` (`:170`), `observe`/`predict`/`study_sky` (`:90-102`).

No GemStone analogue whatsoever.

### 3.9 `DRCS` (common-summoning.rb, 247 LOC) and `DRCTH` (common-theurgy.rb, 332 LOC)

`DRCS` — Warrior Mage weapon summoning: `summon_weapon(_moon, element, ingot, skill)` (`:61`), `get_ingot`/`stow_ingot` (`:81,92`), `break_summoned_weapon` (`:102`), `shape_summoned_weapon` (`:108`), `custom_summoned_weapon_adjectives` (`:154`), `base_summoned_weapon` (`:174`), `identify_summoned_weapon` (`:183`), `turn`/`push`/`pull_summoned_weapon` (`:202-222`), `summon_admittance` (`:232`).

`DRCTH` — Cleric ritual supplies. `CommuneSenseResult` value class (`:49-65`). Supply checks `has_holy_water?`/`has_flint?`/`has_holy_oil?`/`has_incense?`/`has_jalbreth_balm?` (`:67-87`), shopping `buying_cleric_item_requires_bless?`/`buy_cleric_item?`/`buy_single_supply`/`quick_bless_item` (`:91-147`), hand management `empty_cleric_hands`/`empty_cleric_right_hand`/`empty_cleric_left_hand` (`:155-178`), ritual actions `sprinkle_holy_water?`/`sprinkle_holy_oil?`/`apply_jalbreth_balm`/`wave_incense?` (`:185-236`), and `commune_sense`/`parse_commune_sense_lines` (`:282,298`).

### 3.10 `CharacterValidator` (common-validation.rb, 102 LOC)

Not a validator of data — a **cross-character trust list over LNet**, DR's out-of-band player chat network. `initialize(announce, should_sleep, greet, name)` (`:9-24`) finds the running `lnet` script by name (`:13`), pushes messages into `@lnet.unique_buffer` (`:94,98`). `validate(character)` (`:34`) issues `who <character>` over LNet and waits for `confirm` (`:42`). `in_game?` (`:82`) uses `DRC.bput("find #{character}", ...)`. Also carries bankbot helpers (`send_bankbot_balance` `:57`, `send_bankbot_location` `:65`, `send_bankbot_help` `:73`) and `send_slack_token` (`:26`).

**Dependency to flag:** this class hard-depends on a *separate community script* named `lnet` being loaded (`LNET_SCRIPT_NAME = 'lnet'`, `:6`). It degrades gracefully (`:18-21`) but is non-functional without it.

### 3.11 `SlackBot` (slackbot.rb, 392 LOC)

A full Slack Web API client inside Lich: error hierarchy `Error` / `NetworkError` / `ApiError` / `ThrottlingError` with `retry_after` (`slackbot.rb:18-33`), `post(method, params)` (`:343`), `get_dm_channel` (`:387`), `direct_message` (`:124`), `fetch_users_list` (`:214`) with an on-disk cache (`read_users_cache` `:274`, `write_users_cache` `:287`), token discovery `find_token` (`:299`) / `request_token` (`:316`) / `ensure_slack_token` (`:174`), and LNet coupling (`lnet_connected?` `:92`, `lnet_available?` `:158`, `wait_for_lnet_connection` `:201`).

**This does not belong in Cena's core.** It is an outbound HTTP integration with a credential store, shipped inside the game engine. It is the clearest candidate in the tree for "Lua script, not Rust binary."

### 3.12 `EquipmentManager` (equipmanager.rb, 947 LOC, 30 methods)

The only `class` (not module) in `commons/` (`equipmanager.rb:12`). Instantiated with `settings` (`:34`), builds `items(settings)` (`:45`) from `DRC::Item` records.

Equipment-set switching: `wear_equipment_set?(set_name)` (`:94`), `wear_items` (`:77`), `wear_missing_items` (`:157`), `remove_unmatched_items` (`:182`), `remove_gear_by(&block)` (`:66`), `return_held_gear(gear_set = 'standard')` (`:437`), `empty_hands` (`:460`).

Weapon handling is the complex half: `wield_weapon?(description, skill)` (`:347`), `wield_weapon_offhand?` (`:308`), `turn_to_weapon?(old_noun, new_noun)` (`:679`) — DR weapons physically *transform* (`DRC::Item` carries `transforms_to`, `transform_text`, `transform_verb`), `swap_to_skill?(noun, skill)` (`:700`), `unload_weapon(name)` (`:791`) for ranged, `stow_weapon(description, transform_depth: 3)` (`:835`) which recurses through transform chains, `stow_by_type` (`:888`).

Retry machinery: `STOW_RECOVERY_PATTERNS` (`:18`), `STOW_HELPER_MAX_RETRIES = 10` (`:29`), `UNTIE_EXHAUSTED_PATTERNS` (`:479`), `stow_helper(action, weapon_name, *accept_strings, failure_patterns:, retries:)` (`:903`), `get_item_helper(item, type)` (`:601`), `verb_data(item)` (`:509` — a ~90-line dispatch from item properties to the right game verbs).

`notify_missing(lost_items)` (`:138`) tells the player when gear went missing mid-session.

---

## 4. DR-specific parsing helpers

### 4.1 `custom_substitutions.rb` (120 LOC)

A user-extension mechanism, not a substitution table. `CustomSubstitutions.resolve(key, defaults, type:)` (`:72-76`) merges the player's own additions from character settings on top of a frozen built-in default list, validating each entry individually and memoizing the result:

```ruby
memoize(key) { (Array(defaults) + validated_additions(key, type)).uniq }
```

Three shapes, `SUPPORTED_TYPES = %i[pairs names regexes]` (`:55`). Validation, timeout and reporting live in the shared `Lich::Common::UserDefs` (`:40`); `REGEX_TIMEOUT_SECONDS` (`:48`) applies a **per-regex evaluation budget** to user-supplied patterns — a ReDoS guard, because players can inject arbitrary regexes into the hot parse path.

Consumers:
- `DRC.box_list_to_adj_and_noun` via `DEFAULT_BOX_SUBSTITUTIONS` (`common.rb:328`) and settings key `:custom_box_woods` / `:custom_box_containers` (`drvariables.rb:322-328`).
- `DRC.scroll_list_to_adj_and_noun` via `DEFAULT_SCROLL_SUBSTITUTIONS_PRE` (`common.rb:368`) and `_POST` (`common.rb:383`), plus `SCROLL_KEYWORD_COLLAPSE` (`common.rb:403`).
- `normalize_creature_names` (`drdefs.rb:209-215`) via `:custom_creature_normalizations` on top of `CREATURE_NORMALIZATIONS` (`drdefs.rb:59`, three entries: `'alfar warrior'`, `'sinewy leopard'`, `'lesser naga'`). The comment at `drdefs.rb:200-202` notes this "runs on the hot NPC-parsing path (`find_npcs` fires on every room update), so it uses a plain substring check rather than compiling a regex per name per call."

Cache is reset at login by `DRInfomon.post_startup_checks` (`startup.rb:104`).

### 4.2 `drdefs.rb` (307 LOC) — room-text parsing

`DRDefsPattern` (`drdefs.rb:8-60`) holds 20 patterns. The player-list pipeline is `extract_pcs(room_players, filter_pattern:, status_pattern:)` (`:129-139`) with three entry points: `find_pcs` (`:144`), `find_pcs_prone` (`:151`), `find_pcs_sitting` (`:158`). It has to strip status prose (`PLAYER_STATUS`, `:12`), parentheticals (`:14`), and handle **both** long-form and short-form posture markers because of a game option: `LYING_DOWN = /who is lying down|\(prone\)/i` (`:21`) and `SITTING = /who is sitting|\(sitting\)/i` (`:25`), each annotated "shown when the HidePostStrings option is on (issue #4529)".

`normalize_trailing_and` (`:116-122`) exists purely because DR's room text has **no Oxford comma**.

NPC pipeline: `find_all_npcs` (`:165`) → `NPC_SCAN` (`:31`) which matches `<pushBold/>…<popBold/>` runs including `" which appears dead"` and `" (dead)"` variants → `clean_npc_string` (`:176-188`) chains `normalize_creature_names` → `remove_html_tags` (`:220`) → `extract_last_creature` (`:230`) → `extract_final_name` (`:238`) → `add_ordinals_to_duplicates` (`:246-266`), which produces `"goblin"`, `"second goblin"`, `"third goblin"` using `$ORDINALS`. `find_objects` (`:299`) hard-codes a workaround for one creature: `GELAPOD` / `GELAPOD_REPLACEMENT` (`:50-51`).

### 4.3 DR's roundtime and status model

DR has **no `Status` object**. `lib/global_defs.rb:1219-1265` defines seven top-level status predicates that are GemStone-only and `fail` loudly in DR:

```ruby
def checksleeping;  return Status.sleeping? if XMLData.game =~ /GS/
                    fail "Error: toplevel checksleeping command not enabled in #{XMLData.game}"
```
— same shape for `sleeping?`, `checkbound`/`bound?`, `checksilenced`/`silenced?`, `checkcalmed`/`calmed?`, `checkcutthroat`/`cutthroat?`.

DR instead derives status from text. `DRC.bput` (`common.rb:172-183`) polls `stunned?` and `webbed?` in loops; `DRC.fix_standing` (`common.rb:552`) and `CANNOT_STAND_PATTERN` (`common.rb:83`) reconstruct posture from failure messages ("unconscious|plummeting to your death|prevents you from standing|…|no room to do much of anything").

Roundtime itself *is* shared: DR uses the same `<roundTime/>` tag and the same `waitrt?` global (33 call sites across `lib/dragonrealms/commons/`). But DR also has a **text-only soft-wait**: `WAIT_RESPONSE_PATTERN = /(?:\.\.\.wait |Wait |\.\.\. wait )(?<seconds>[0-9]+)/` (`common.rb:19`), handled at `common.rb:140-148` by sleeping `seconds - 0.5` then re-issuing. Cena needs both a structured RT timer and this text-derived one.

Balance and position are DR-only and are ordinal indices, not booleans: `DRStats.balance` is an index into `DR_BALANCE_VALUES` (`drparser.rb:581`) defaulting to 8 (`drstats.rb:22`); `DRStats.position` is a signed −9…+9 from `DR_POSITION_VALUES` (`drparser.rb:583`). GemStone's equivalent concept is `Stance`, a percentage.

### 4.4 `creature.rb` (416 LOC)

Built on the shared `Lich::Common::CreatureBase` mixin (`creature.rb:31,43`) — so the registry, room roster and `<crtrStatus>` flag handling are **already abstracted** between the two games. What is DR-specific is the three-stream reconciliation documented at `creature.rb:9-28`:

1. `<crtrStatus exist=… hostile=1 immobile=1/>` — id-keyed, full flag snapshot, **name-less**, batched *after* the room-objs component closes.
2. room-objs bold text — names only, **no exist id**.
3. the `assess` stream — the only feed tying an id to a name, plus assess number, relation and range.

So instances are created id-first and backfilled. Two backfill paths exist in `lib/common/xmlparser.rb`:

- **Positional pairing at prompt time** (`xmlparser.rb:680-688`): `@dr_crtr_ids` and `@dr_room_npc_names` are zipped *only if the counts match* — "on any mismatch (e.g. a bold room entity that emits no crtrStatus) we skip naming rather than risk a shifted mis-pair." Both arrays are cleared every prompt.
- **Assess backfill by id** (`xmlparser.rb:571-573`) → `Creature.feed_assess(entry)` (`creature.rb:321`, instance method `:123`).

DR-only accessors on `CreatureInstance`: `assess_number`, `relation`, `assess_status`, `range` (`:melee`/`:pole`/`:missile`), `target_id`, `target`, `target_number`, `balance`, `conditions`, `enriched_at` (`creature.rb:55-85`). `balance_pattern` (`:232`) is built from `DR_BALANCE_VALUES`, which is why `creature.rb:32` requires `drinfomon/drvariables`. Predicates `off_balance?` (`:185`), `cursed?` (`:203`), `friendly?` (`:220`), `enriched?` (`:177`), and DR's own `valid_target?` (`:168`).

The doc comment at `creature.rb:84-85` is a warning Cena must honor: "Assess fields (range/balance/conditions/relation/target) are 'pull' snapshots and can go stale — poll assess before relying on one."

### 4.5 `dependency/settings_config.rb` (103 LOC)

Pure data: `TRANSFORM_CONFIG` (`:14-100`), a seven-phase declarative pipeline consumed by a shared `SettingsTransformer` (the transformer itself is not in the DR tree):

| Phase | Key | Purpose |
|---|---|---|
| 1 | `empty_data_type` (`:16`) | default empty values |
| 2 | `spell_data_type`, `waggle_set_keys`, `spell_map_enrich_name_keys`, `spell_map_enrich_data_keys`, `spell_list_keys`, `single_spell_keys`, `offensive_spells_key`, `battle_cries_key` (`:19-36`) | spell enrichment |
| 3 | `composed_lists` (`:39-51`) | builds `lootables` from `lootables`+`box_nouns`+`gem_nouns`+`scroll_nouns` with `loot_additions`/`loot_subtractions` |
| 4 | `uservars_fallback` (`:54-69`) | 14 settings that fall back to `UserVars`, each with `mode: :default` or `:append`; plus `global_overrides` mapping `hometown` → `$HOMETOWN` (`:71-73`) |
| 5 | `hometown_lookup_keys` (`:76-83`) | 15 room-id keys resolved per hometown |
| 6 | `denylists` (`:86-91`) | blocks room 5713 from `safe_room`/`safe_room_id` |
| 7 | `legacy_migrations` (`:94-99`) | 4 rules, e.g. `append_if_flag` `'pouches'` to `appraisal_training` when `train_appraisal_with_pouches` |

This file is a clean spec and should port to Cena almost verbatim as a config table.

### 4.6 `drbanking.rb` (354 LOC) and `drexpmonitor.rb` (209 LOC)

**DRBanking** parses banking *passively* — `DRBanking.parse(line)` is called unconditionally from `DRParser.parse` (`drparser.rb:715`) on every single server line. Seven patterns (`drbanking.rb:25-52`) cover deposit portion/all-teller/all-jar, withdraw portion/all, balance check, and no-account. `current_bank_town` (`:164`) resolves the town by matching the room title against `BANK_TITLES`. Persistence is JSON per character (`load_accounts` `:285`, `save_accounts` `:292`), exposed to the player as `;banks`, `;banks all`, `;banks reset` (`client_commands.rb:277-280`). `total_wealth` (`:112`) and `total_wealth_all` (`:118`) aggregate across towns and characters.

**DRExpMonitor** is a thread (`start` `:44`, `stop` `:83`, `active?` `:97`) that is auto-started at `drinfomon.rb:26`:

```ruby
DRExpMonitor.start if Lich.display_expgains
```

`Lich.display_expgains` (`lich.rb:1054-1070`) defaults to `true` **except when `$frontend == 'genie'`** (`lich.rb:1065`) — "Genie has built-in exp tracking." That is a frontend-conditional behavior Cena inherits. `format_briefexp_on` (`:157`) and `format_briefexp_off` (`:167`) produce the replacement strings that `drparser.rb:618,621` splice into the live line.

---

## 5. Travel and mapping: `lib/common/map/`

`map_base.rb` (954 LOC) holds the shared pathfinding (`MinHeap` at `:13`, `TagList` at `:77`, `MapBase::ClassMethods` at `:155`, `InstanceMethods` at `:692`). `map_gs.rb` (520) and `map_dr.rb` (419) are two sibling `class Map` definitions — **only one is ever loaded**, decided at `gameloader.rb:18` vs `:61`.

DR-specific in `map_dr.rb`:

- Three extra room attributes not present in GS: `genie_id`, `genie_zone`, `genie_pos` (`map_dr.rb:31`, constructor `:40`, serialized via `json_extra_fields` `:70-72`), plus a lookup `by_genie_ref(zone_id, node_id)` (`:132`). These exist for interop with the Genie frontend's own map format.
- `match_current(_script)` (`map_dr.rb:225-267`) — **the script argument is ignored** (leading underscore), unlike GS's `match_current(script)` (`map_gs.rb:183`). DR matching is a two-pass linear scan over `@@list`: exact title+description+paths, then a fallback with a description regex built by splitting on sentence boundaries (`map_dr.rb:245`). GS's version at `map_gs.rb:183-309` is 126 lines to DR's 42.
- **Fog handling**, DR-only: `foggy_exits = XMLData.room_exits_string =~ /^Obvious (?:exits|paths): obscured by a thick fog$/` (`map_dr.rb:231`, `:283`) — when foggy, the paths check is skipped entirely.
- `peer_disambiguation_tag?(room)` (`map_dr.rb:171`) and `resolve_matched_room(room, honor_peer_tags:)` (`:204`) — DR rooms that are textually identical require a manual `peer` command to tell apart. `match_current` passes `honor_peer_tags: false` (`:242`) while `match_fuzzy` passes `true` (`:295`), so a room needing a peer resolves to `nil` when no script is running.
- `map_gs.rb` has `fuzzy_room_id` (`:130`), `get_location` (`:135`), `locations` (`:461`), `images` (`:468`) — **none of which exist in `map_dr.rb`**. DR has no location/image concept.

Map data files are game-scoped on disk: `File.join(DATA_DIR, XMLData.game)` (`map_base.rb:429`, `:442`, `:626`).

---

## 6. Startup and lifecycle

`DRInfomon.watch!` (`startup.rb:30-44`) spawns a thread that blocks on `sleep 0.1 until GameBase::Game.autostarted? && XMLData.name && !XMLData.name.empty?` (`:34`), then calls `startup` (`:46`) → `ExecScript.start(startup_script, { quiet: true, name: 'drinfomon_startup' })` (`:47`).

`startup_script` (`:50-84`) is a **heredoc of Ruby source compiled and run as a Lich script**, precisely so that `fput` blocks until the game responds. It issues four `Lich::Util.issue_command` calls (`info`, `played`, `exp all 0`, `ability` — `:53-66`), then a one-time `flag MonsterBold on` check gated on `UserVars.dependency_setflags` (`:71-80`). The `ability` verb is polymorphic: magic guilds → `spells` output, Barbarians → berserk list, Thieves → khri trees, each parsed by a different scraper.

`startup_complete?` (`:24`) is the public readiness flag; `startup_completed!` (`:86-90`) sets it, fires `PostLoad.game_loaded!` and runs `post_startup_checks` (`:97-108`), which warns about **23 obsolete `.lic` filenames** (`DR_OBSOLETE_SCRIPTS`, `:137-143`) — the files these very libraries replaced when they moved into core Lich — and about `scripts/custom/` files shadowing curated ones (`warn_custom_scripts` `:177`).

`DRINFOMON_CORE_LICH_DEFINES` (`drinfomon.rb:6`) lists 17 names that community scripts test with `defined?` to decide whether they are running against core-Lich DRInfomon or the old standalone scripts. `DRINFOMON_IN_CORE_LICH = true` (`drinfomon.rb:8`) and `Lich::Common::CORE_DR_STARTUP = true` (`startup.rb:194`) are the same sentinel idea.

---

## 7. GemStone vs DragonRealms divergence

### 7.1 Every game branch point

`grep -rn "XMLData\.game" --include=*.rb lib/ main.rb` returns **108 hits**. Excluding scope-string uses (`"#{XMLData.game}:#{XMLData.name}"`, which are game-*labeled* rather than game-*branching*), the genuine behavioral branches are:

| File:line | Branch | What differs |
|---|---|---|
| `lib/common/gameloader.rb:89-91` | `=~ /DR/` vs `/GS/` | **The root seam.** Loads two disjoint library sets (`:15-57` vs `:59-70`) |
| `lib/common/gameloader.rb:18` / `:61` | — | `map_gs.rb` vs `map_dr.rb` |
| `lib/attributes/char.rb:139` | `=~ /^GS/` | `Char.citizenship` returns nil in DR |
| `lib/attributes/char.rb:147` | `=~ /^GS/` | `Char.che` returns nil in DR |
| `lib/common/account.rb:31` | `=~ /^GS/` | account parsing differs |
| `lib/common/client_commands.rb:192-193` | `:gs` / `:dr` gate | `game_matches?` gates whole commands |
| `lib/common/client_commands.rb:255` / `:263` | `/^GS/` elsif `/^DR/` | help text: GS gets `;infomon …`/`;sk`; DR gets `;display flaguid`, `;display roomid` |
| `lib/common/client_commands.rb:273` | `/^DR/` | DR-only commands: `;display expgains`, `;display inlineexp`, `;display exp-status`, `;banks`, `;banks all`, `;banks reset`, `;banks reset all` |
| `lib/common/xmlparser.rb:505` | `/^DR/` | `<crtrStatus>` → DR applies flags immediately id-first and queues the id; GS buffers in `@pending_crtr_status` |
| `lib/common/xmlparser.rb:554` | `/^GS/` | `GameObj.begin_inv` / `begin_reserve` on pushStream — **GS only** |
| `lib/common/xmlparser.rb:571` | `/^DR/` | assess stream backfills DR `Creature` by id |
| `lib/common/xmlparser.rb:603` / `:608` | `/^GS/` elsif `/^DR/` | room subtitle parsing: GS strips a `- <uid>` suffix; DR treats `<nav rm=…>` as primary UID and the subtitle marker as fallback |
| `lib/common/xmlparser.rb:680` | `/^DR/` | prompt-time positional pairing of crtr ids to bold names |
| `lib/common/xmlparser.rb:1054` / `:1058` | `/^DR/` | `GameObj.remove_inv_item` when an item enters a hand — DR only |
| `lib/common/xmlparser.rb:1068` | `/^DR/` | `<a>`-tagged room creature → DR `Creature.register` vs GS `Creature.register` |
| `lib/common/xmlparser.rb:1089` | `/^DR/` | bold room-objs text with no `<a>` tag → captured into `@dr_room_npc_names` |
| `lib/common/map/map_base.rb:429,442,626` | scope | map data dir is `DATA_DIR/<game>` |
| `lib/common/update/file_updater.rb:216` | `!~ /^GS|^DR/` | refuses to update for unknown games |
| `lib/common/update/file_updater.rb:221` | `/^GS/` | `effect-list.xml` is GS-only data |
| `lib/common/update/script_sync.rb:47` | `config[:game_filter]` | per-script game filter for the curated-script updater |
| `lib/gemstone/infomon.rb:295` | `!~ /^DR/` | Infomon DB refresh explicitly skipped in DR |
| `lib/global_defs.rb:395` | `=~ /GS/` | (GS-only top-level behavior) |
| `lib/global_defs.rb:1219-1265` | `=~ /GS/` ×14 | 7 status predicates `fail` in DR: `checksleeping`/`sleeping?`, `checkbound`/`bound?`, `checksilenced`/`silenced?`, `checkcalmed`/`calmed?`, `checkcutthroat`/`cutthroat?` |
| `lib/lich.rb:1034` | `=~ /^DR/` | `display_room_mono` defaults **on** in DR, off elsewhere |
| `lib/lich.rb:1064-1065` | `!= ""` + `$frontend` | `display_expgains` defaults on except on Genie |
| `lib/messaging.rb:23` / `:25` | `/^GS/` elsif `/^DR/` | allowed stream windows: GS `["familiar","speech","thoughts","loot","voln"]`; DR `["familiar","speech","thoughts","combat"]` |
| `lib/common/gui/utilities.rb:93-97` | string match | login GUI maps `"dragonrealms"`→DR, `"dragonrealms the fallen"`→DRF, `"dragonrealms prime test"`→DRT |
| `lib/common/authentication/login_helpers.rb:44-45,425-445,505-517` | `--dragonrealms`/`--dr` | CLI instance resolution; default `'DR'` |
| `lib/games.rb:942,950-951` | non-empty | `set_game_instance(XMLData.game)` |
| `lib/common/session_lifecycle.rb:194` | non-empty | session naming |
| `lib/dragonrealms/drinfomon/drparser.rb:660` | `== 'DRX' \|\| == 'DRF'` | **intra-DR branch**: DRX (Platinum) and DRF (The Fallen) imply premium |

Instance codes observed: `GS`-prefixed and `DR`, `DRX`, `DRF`, `DRT` — so the game identifier is not binary, and `XMLData.game =~ /^DR/` is the real test everywhere except `drparser.rb:660`.

Notably, **no hits for `$fake_stormfront` gate DR vs GS behavior**; the fake-tag path in `xmlparser.rb` (`@send_fake_tags`, e.g. `:1055`, `:1059`, `:656`) is frontend-conditional, not game-conditional.

### 7.2 What Cena must abstract

| Concern | GemStone | DragonRealms | Abstraction Cena needs |
|---|---|---|---|
| **Character state store** | `Infomon` — SQLite, keyed `GS_<name>`, survives restart (`infomon.rb:86`) | `DRStats`/`DRSkill` — in-memory `@@` vars, rebuilt every login by four verbs (`startup.rb:53-66`) | One state store with a *declared* persistence policy per field. Do not inherit "DR state is ephemeral." |
| **How state arrives** | XML feeds + Infomon parser | XML for vitals/room/spells; **text regex for stats, skills, balance, position, encumbrance, wounds, inventory ids, banking** (§2.2) | A parser that treats "structured event" and "matched text line" as two producers feeding one state model — not two parallel systems |
| **Parser purity** | observer | **mutates the line** (`drparser.rb:618,621`) | An explicit line-transform stage between parse and render |
| **Parser statefulness** | mostly per-line | 3 mode flags + 6 multi-line scrapers with start/end patterns and a prompt-based safety valve (`drparser.rb:95-99, 694-706`) | A first-class "scrape session" concept: begin pattern, end pattern, timeout, abort-on-prompt, commit-or-discard |
| **Creature identity** | one `<a exist noun>` tag carries id + name together | id and name arrive on **different streams** and are reconciled positionally or via assess (`creature.rb:9-28`, `xmlparser.rb:680-688`) | Already partly solved by `CreatureBase`; Cena needs the reconciliation to be a named, testable component, not inline parser state |
| **Status flags** | `Status.sleeping?/bound?/silenced?/calmed?/cutthroat?` | none — `fail`s (`global_defs.rb:1219-1265`); derived from failure text in `DRC.bput` (`common.rb:133-190`) | Capability query, not an exception. `status.sleeping()` should return "unsupported" in DR, and DR's text-derived equivalents should populate the same slots where they exist |
| **Balance / stance** | `Stance` (percentage) | `DRStats.balance` = index 0-11 into a word list; `DRStats.position` = signed −9..+9 (`drvariables.rb:46-88`) | Separate "engagement quality" concept; do not force DR's two axes into GS's one |
| **Roundtime** | `<roundTime/>` | `<roundTime/>` **plus** text `"...wait 3"` (`common.rb:19,140-148`) | One RT clock with two producers |
| **Currency** | single | three currencies × five denominations × exchange matrix, town-scoped (`common-money-data.rb:8-…`, `drvariables.rb:188-190`) | Currency must be a typed value with a zone, not an integer |
| **Wounds** | wound/scar ranks | `Wound` objects with bleeding rate, internal flag, parasite, lodged item, tendability threshold (`common-healing.rb:442-510`, `common-healing-data.rb:15-38`) | Superset model; GS is the degenerate case |
| **Map matching** | 126-line `match_current(script)`, has locations + images (`map_gs.rb:183,461,468`) | 42-line `match_current(_script)`, fog bypass, peer-disambiguation tags, genie ids (`map_dr.rb:225,231,171,31`) | Room-identification strategy as a pluggable pipeline of predicates; `location`/`image` optional |
| **Stream windows** | 5 allowed | 4 allowed, different set (`messaging.rb:23-26`) | Per-game stream capability table |
| **Client commands** | `;infomon …`, `;sk` | `;banks*`, `;display expgains`, `;display inlineexp`, `;display flaguid`, `;display roomid` (`client_commands.rb:255-281`) | Command registry with a game predicate — the `game_matches?(gate)` shape at `client_commands.rb:189-195` is already the right design; Cena should generalize it to all capabilities |
| **Defaults** | mono off, expgains off | mono **on** (`lich.rb:1034`), expgains **on** unless Genie (`lich.rb:1064-1065`) | Per-game default table, frontend-aware |

**Where the seam should be.** Lich's current seam is `require`-time (`gameloader.rb:89`) plus ~30 scattered runtime `=~ /^DR/` checks. For Cena the load-time split is the right instinct — the DR and GS libraries genuinely share almost no logic — but the runtime checks inside `xmlparser.rb` (eight of them, `:505` through `:1089`) show where a single shared component was retrofitted for two games. Those eight are the ones to design out: the XML parser should emit game-neutral events and let a per-game reconciler decide what a `<crtrStatus>` without a name means.

---

## 8. Summary table

Port difficulty: **Low** = mechanical translation, mostly data. **Medium** = logic plus game-protocol knowledge. **High** = intricate retry/recovery semantics, huge regexes, or cross-component state. **Defer** = should not be in the Rust core.

| Module | File | LOC | Purpose | GS analogue | Port difficulty |
|---|---|---:|---|---|---|
| `DRInfomon` (loader) | `drinfomon.rb` | 29 | requires the 11 drinfomon files; auto-starts exp monitor | `lib/gemstone/infomon.rb` (loader part) | Low |
| — (data) | `drinfomon/drvariables.rb` | 372 | 16 frozen tables: learning rates, balance, position, skills, banks, hometowns, enc, mana, vol, sigils; 11 global aliases | partial (`Skills`, `Stats`) | Low — but it is the DR world model |
| `DRParser` | `drinfomon/drparser.rb` | 725 | 49 patterns + 31-branch text dispatch + 6 multi-line scrapers + inventory mode machine | `Infomon::Parser` | **High** |
| `DRStats` | `drinfomon/drstats.rb` | 292 | 17 char attributes; 6 delegate to XMLData; 12 guild predicates; `GUILD_MANA_TYPES` | `Stats` + `Char` (DB-backed) | Low |
| `DRSkill` | `drinfomon/drskill.rb` | 251 | 70 skills w/ rank, mindstate, percent, baseline; exp modifiers; rested exp | `Skills` (DB-backed) | Medium |
| `DRRoom` | `drinfomon/drroom.rb` | 85 | pcs/npcs/dead/objs/group/prone/sitting; 3 XMLData delegates | `GameObj` room roster | Low |
| `DRSpells` | `drinfomon/drspells.rb` | 67 | known spells/feats + 3 scrape flags; 3 XMLData delegates | `Spells`, `ActiveSpell` | Low |
| `Flags` | `drinfomon/events.rb` | 40 | script-registered regex watchers; DR's event bus | `Flags` does not exist in GS tree | Low |
| `DRBanking` | `drinfomon/drbanking.rb` | 354 | passive coin-balance scraping per town, JSON-persisted; `;banks` | `lib/gemstone/bank.rb` | Medium |
| `DRExpMonitor` | `drinfomon/drexpmonitor.rb` | 209 | thread; real-time exp gains; **rewrites EXP lines in place** | `lib/gemstone/experience.rb` | Medium |
| `DRInfomon` startup | `drinfomon/startup.rb` | 196 | login thread; 4-verb state bootstrap as generated Ruby; obsolete-file warnings | `Infomon.watch!` + sync | **High** (codegen + ordering) |
| `DRDefs` | `drinfomon/drdefs.rb` | 307 | room text parsing: PCs, NPCs, objects; 20 patterns; ordinal naming | inline in `xmlparser.rb` for GS | Medium |
| `DRC` | `commons/common.rb` | 1093 | `bput` retry engine; `DRC::Item`; 6000-char flavor-text regex; bard songs; script pause/unpause; output | none (GS scripts use raw `fput`/`waitfor`) | **High** |
| `DRCI` | `commons/common-items.rb` | 2058 | 63 item verbs: get/stow/wear/put/tie/give + safe/unsafe triads; counting; gem pouches | none | **High** (volume) |
| `DRCA` | `commons/common-arcana.rb` | 1373 | magic: prepare/cast/charge/invoke; 14-method cambrinth allocator; khri, barb buffs, regalia, avtalia | `lib/gemstone/psms.rb`, `spell.rb` | **High** |
| `EquipmentManager` | `commons/equipmanager.rb` | 947 | equipment sets; wield/offhand; transforming weapons; ranged unload; stow retry helper | `lib/gemstone/readylist.rb`, `armaments.rb` | **High** |
| `DRCC` | `commons/common-crafting.rb` | 815 | 8 room-finders by hometown; recipes; tools; consumables; enchanting/sigils; private forges | none | Medium (mostly data) |
| `DRCH` | `commons/common-healing.rb` | 520 | `HealthResult`/`Wound`; 6 text scrapers; bind/unwrap; Empath perceive | `wounds.rb`, `scars.rb`, `injured.rb` | Medium |
| `DRCH` data | `commons/common-healing-data.rb` | 278 | `BLEED_RATE_TO_SEVERITY`: 26 rates → severity + First Aid thresholds | none | Low |
| `DRCMM` | `commons/common-moonmage.rb` | 492 | telescope/bones/div tools; moon weapons; planet+moon visibility; roisaen calendar | none | Medium |
| `DRCT` | `commons/common-travel.rb` | 424 | `walk_to` supervising the `go2` script via `Flags`; empty-room search; shop verbs | `go2` / `Map.findpath` direct | **High** (cross-script supervision) |
| `SlackBot` | `commons/slackbot.rb` | 392 | Slack Web API client, token store, user cache, LNet coupling | none | **Defer** — not core |
| `DRCTH` | `commons/common-theurgy.rb` | 332 | Cleric supplies, blessing, sprinkle/incense rituals, commune sense | none | Medium |
| `DRCS` | `commons/common-summoning.rb` | 247 | Warrior Mage weapon summon/shape/break; ingots | none | Medium |
| `DRCM` | `commons/common-money.rb` | 210 | 3-currency conversion, exchange w/ fee, bank withdraw/deposit, wealth | `lib/gemstone/currency.rb` | Medium |
| `DRCM` data | `commons/common-money-data.rb` | 75 | denominations, abbreviation regexes, exchange-rate matrix | none | Low |
| `CharacterValidator` | `commons/common-validation.rb` | 102 | LNet-backed cross-character trust list; bankbot DMs | none | **Defer** — depends on external `lnet` script |
| `Creature` (DR) | `creature.rb` | 416 | 3-stream id/name/assess reconciliation on shared `CreatureBase` | `lib/gemstone/creature.rb` (single-stream) | **High** |
| `CustomSubstitutions` | `custom_substitutions.rb` | 120 | validated user extension of built-in name/regex/pair lists, memoized, ReDoS-budgeted | none | Medium |
| `SettingsConfig` | `dependency/settings_config.rb` | 103 | 7-phase declarative settings transform spec | GS equivalent not in this tree (UNVERIFIED) | Low |
| detachable client | `detachable_client_init.rb` | 94 | DR detachable-client bootstrap | `lib/gemstone/detachable_client_init.rb` | Low (paired file exists) |
| Map (DR) | `lib/common/map/map_dr.rb` | 419 | DR room matching: 2-pass linear, fog bypass, peer tags, genie ids | `map_gs.rb` (520) | Medium |
| Map (shared) | `lib/common/map/map_base.rb` | 954 | MinHeap, TagList, pathfinding, JSON I/O | shared already | Medium |

---

## 9. UNVERIFIED

- **`lib/dragonrealms/detachable_client_init.rb` (94 LOC)** — listed and counted but not read line-by-line. To verify: read it alongside `lib/gemstone/detachable_client_init.rb` and diff, to confirm whether the DR/GS split there is real or vestigial.
- **`SettingsTransformer` itself** — `dependency/settings_config.rb` is only the config; the consumer that interprets the 7 phases was not located in this pass. To verify: `grep -rn "TRANSFORM_CONFIG\|SettingsTransformer" lib/`.
- **A GemStone counterpart to `SettingsConfig`** — I did not confirm whether `lib/gemstone/` has an equivalent transform spec, so the "GS analogue" cell for that row is unverified. To verify: `grep -rn "TRANSFORM_CONFIG" lib/gemstone/ lib/common/`.
- **Exact public-method counts for `DRSkill`, `DRStats`, `DRRoom`, `DRSpells`, `DRBanking`, `DRExpMonitor`** — the summary table leaves these blank because they are attribute accessors rather than an API surface in the `commons/` sense; I counted methods only for the `module_function` libraries. To verify: `grep -c "def self\." on each.
- **Whether `EXCHANGE_RATES` is complete for all 3×3 currency pairs** — I read `common-money-data.rb:48-50` (the `'dokoras'` row's opening) but not the full literal. To verify: `sed -n '48,75p' lib/dragonrealms/commons/common-money-data.rb`.
- **`DRCA` cyclic-spell coverage** — `CYCLIC_RELEASE_SUCCESS_PATTERNS` (`common-arcana.rb:8`) was read only through line 20 (Ranger/Empath/Bard sections). The full guild coverage is unconfirmed.
- **`dr-scripts` call-site counts** are `grep -o "\bMODULE\."` occurrence counts across `dr-scripts/*.lic`, not distinct-symbol counts; they measure coupling weight, not API surface. They also do not distinguish a call from a comment mention.
- **`lib/common/creature/creature_base.rb`** — referenced by `creature.rb:31` and quoted from `creature_base.rb:28`, but not inventoried here; it is shared infrastructure and belongs in the Lich-infrastructure inventory.
