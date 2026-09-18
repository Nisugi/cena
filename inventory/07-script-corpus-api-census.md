# Script Corpus API Census

**Scope.** An empirical measurement of what the two community `.lic` script corpora
actually call, in order to define the Lua API surface Cena must expose for existing
scripts to be portable. Covered: all 234 `.lic` files in
`E:/Cena/reference/scripts/scripts/`, all 222 `.lic` files in
`E:/Cena/reference/dr-scripts/`, the authoritative global-function list extracted from
`E:/Cena/reference/lich-5/lib/global_defs.rb`, and the Lich-5 definitions of
`get_settings`, `get_data`, `parse_args` (`lib/global_defs.rb`,
`lib/common/setup_files.rb`, `lib/common/arg_parser.rb`) and `Spell` runtime evaluation
(`lib/common/spell.rb`). Date: 2026-09-17.

**Measurement method and its limits.** Counts come from two single-pass Ruby tokenizers
run over the corpora, which strip single-quoted strings, double-quoted strings, inline
`#` comments and (for the global census) regex literals before tokenizing, plus targeted
`grep -n` for evidence. This removes the bulk of false positives from prose and regex
metacharacters but is **not** a Ruby parser. Specific known inaccuracies are flagged in
place with **APPROX**. Every count below is reproducible; every cited line number was
read.

---

## 1. Corpus size

| Metric | GemStone (`scripts/scripts`) | DragonRealms (`dr-scripts`) | Total |
|---|---:|---:|---:|
| `.lic` files | 234 | 222 | 456 |
| Total lines | 177,869 | 70,424 | 248,293 |
| Mean lines/script | 760 | 317 | 544 |
| Median lines/script | 302 | 128 | — |

The task brief said 251 GemStone and 222 DragonRealms scripts. The GemStone repo
contains 251 `.lic` files in total, but 17 of those live under `spec/` as test fixtures
(e.g. `spec/jinx/repos/mirror/assets/sloot.lic`, 2,077 lines). The **shipping** corpus is
the 234 files in `scripts/`, and all counts in this document use those 234. DragonRealms
has 223 `.lic` files, of which 1 is `samples/rezz.lic`; the shipping corpus is the 222
files at repo root.

**Support libraries.** Neither corpus carries a meaningful `lib/` of its own. The
GemStone repo's `lib/` is 10 Ruby files / 998 lines, almost all repo tooling
(`lib/migration/*`, `lib/util/color.rb`) rather than script-facing API. DragonRealms has
no `lib/` at all — **its shared library lives inside Lich-5**, at
`E:/Cena/reference/lich-5/lib/dragonrealms/` (`commons/common.rb`, `commons/common-items.rb`,
`drinfomon/drstats.rb`, and 20+ siblings). DragonRealms instead ships 30 YAML data files
under `dr-scripts/data/` (`base-crafting.yaml`, `base-herbs.yaml`, …) which are loaded
through `get_data`.

### Largest scripts

| GemStone | LOC | DragonRealms | LOC |
|---|---:|---|---:|
| `bigshot.lic` | 10,246 | `combat-trainer.lic` | 6,964 |
| `BlackArts.lic` | 9,184 | `moonwatch.lic` | 2,723 |
| `eloot.lic` | 8,087 | `stack-scrolls.lic` | 2,661 |
| `ebounty.lic` | 3,894 | `trade.lic` | 2,453 |
| `eherbs.lic` | 3,639 | `droughtmans.lic` | 2,324 |
| `madwarrior.lic` | 3,558 | `bescort.lic` | 2,216 |
| `ForgeMaster.lic` | 3,292 | `textsubs.lic` | 1,835 |
| `creaturebar.lic` | 3,155 | `inventory-manager.lic` | 1,676 |
| `alchemy.lic` | 3,091 | `forge.lic` | 1,670 |
| `repository.lic` | 3,035 | `healer.lic` | 1,470 |

The distribution is extremely skewed: on the GemStone side the top 10 scripts are 51,181
lines, 29% of the corpus. `bigshot.lic` alone is 5.8% of the GemStone corpus.

---

## 2. The global function census

`lib/global_defs.rb` (2,370 lines) defines **208 global methods** — this is Lich's entire
script-facing free-function surface. I extracted all 208 names and counted each across
both corpora. Result: **144 of 208 are used at least once; 64 are never used by any
script in either corpus.**

Counts below are call-site occurrences after string/comment/regex stripping. `GSc`/`DRc`
are occurrences; `GSf`/`DRf` are distinct files.

| Function | GSc | GSf | DRc | DRf | **Total** | **Files** |
|---|---:|---:|---:|---:|---:|---:|
| `respond` | 3,883 | 152 | 366 | 22 | **4,249** | 174 |
| `echo` | 2,567 | 135 | 1,594 | 118 | **4,161** | 253 |
| `fput` | 2,686 | 114 | 548 | 102 | **3,234** | 216 |
| `waitrt?` | 716 | 78 | 406 | 91 | **1,122** | 169 |
| `match` | 872 | 41 | 124 | 20 | **996** | 61 |
| `dothistimeout` | 857 | 83 | 0 | 0 | **857** | 83 |
| `pause` | 176 | 39 | 569 | 138 | **745** | 177 |
| `_respond` | 694 | 74 | 48 | 13 | **742** | 87 |
| `move` | 240 | 39 | 245 | 29 | **485** | 68 |
| `wait_while` | 357 | 67 | 4 | 1 | **361** | 68 |
| `running?` | 307 | 42 | 2 | 2 | **309** | 44 |
| `variable` | 179 | 43 | 111 | 34 | **290** | 77 |
| `start_script` | 242 | 33 | 45 | 20 | **287** | 53 |
| `multifput` | 258 | 26 | 0 | 0 | **258** | 26 |
| `put` | 244 | 53 | 8 | 7 | **252** | 60 |
| `before_dying` | 169 | 103 | 82 | 81 | **251** | 184 |
| `goto` | 241 | 3 | 0 | 0 | **241** | 3 |
| `checkright` | 223 | 30 | 16 | 12 | **239** | 42 |
| `checkleft` | 202 | 30 | 18 | 13 | **220** | 43 |
| `waitcastrt?` | 209 | 38 | 10 | 6 | **219** | 44 |
| `matchwait` | 211 | 5 | 0 | 0 | **211** | 5 |
| `get_settings` | 9 | 4 | 200 | 172 | **209** | 176 |
| `get_data` | 0 | 0 | 191 | 78 | **191** | 78 |
| `wait_until` | 164 | 52 | 1 | 1 | **165** | 53 |
| `parse_args` | 7 | 5 | 139 | 139 | **146** | 144 |
| `waitfor` | 47 | 10 | 82 | 26 | **129** | 36 |
| `cast` | 50 | 25 | 77 | 6 | **127** | 31 |
| `hide_me` | 110 | 14 | 2 | 2 | **112** | 16 |
| `dothis` | 99 | 4 | 0 | 0 | **99** | 4 |
| `empty_hands` | 87 | 32 | 9 | 3 | **96** | 35 |
| `pause_script` | 65 | 23 | 23 | 11 | **88** | 34 |
| `checkrt` | 85 | 11 | 0 | 0 | **85** | 11 |
| `fill_hands` | 75 | 29 | 0 | 0 | **75** | 29 |
| `silence_me` | 74 | 22 | 0 | 0 | **74** | 22 |
| `checkcastrt` | 72 | 10 | 0 | 0 | **72** | 10 |
| `reget` | 34 | 19 | 37 | 24 | **71** | 43 |
| `checkbounty` | 62 | 7 | 0 | 0 | **62** | 7 |
| `checkpaths` | 54 | 5 | 1 | 1 | **55** | 6 |
| `checkpcs` | 54 | 16 | 0 | 0 | **54** | 16 |
| `checkmana` | 54 | 17 | 0 | 0 | **54** | 17 |
| `stop_script` | 10 | 4 | 35 | 20 | **45** | 24 |
| `matchtimeout` | 45 | 10 | 0 | 0 | **45** | 10 |
| `status_tags` | 42 | 19 | 1 | 1 | **43** | 20 |
| `checkroom` | 41 | 10 | 0 | 0 | **41** | 10 |
| `checkmind` | 40 | 8 | 0 | 0 | **40** | 8 |
| `checkstance` | 36 | 8 | 0 | 0 | **36** | 8 |
| `wait` | 26 | 17 | 9 | 3 | **35** | 20 |
| `checkprep` | 25 | 9 | 2 | 2 | **27** | 11 |
| `no_pause_all` | 6 | 6 | 20 | 20 | **26** | 26 |
| `unpause_script` | 15 | 5 | 9 | 4 | **24** | 9 |
| `no_kill_all` | 7 | 7 | 17 | 17 | **24** | 24 |
| `checkspirit` | 23 | 7 | 0 | 0 | **23** | 7 |
| `checkstamina` | 22 | 6 | 0 | 0 | **22** | 6 |
| `percentmind` | 21 | 11 | 0 | 0 | **21** | 11 |
| `send_to_script` | 19 | 5 | 0 | 0 | **19** | 5 |
| `report_errors` | 18 | 3 | 0 | 0 | **18** | 3 |
| `maxmana` | 17 | 7 | 0 | 0 | **17** | 7 |
| `muckled?` | 16 | 10 | 0 | 0 | **16** | 10 |
| `checkdead` | 16 | 8 | 0 | 0 | **16** | 8 |
| `do_client` | 14 | 8 | 0 | 0 | **14** | 8 |
| `get?` | 10 | 8 | 4 | 3 | **14** | 11 |
| `checkstunned` | 11 | 4 | 2 | 1 | **13** | 5 |
| `checkpoison` | 12 | 5 | 0 | 0 | **12** | 5 |
| `checkhidden` | 11 | 5 | 1 | 1 | **12** | 6 |
| `undo_before_dying` | 11 | 11 | 0 | 0 | **11** | 11 |
| `checkgrouped` | 10 | 2 | 0 | 0 | **10** | 2 |
| `checkstanding` | 4 | 2 | 5 | 4 | **9** | 6 |
| `checkdisease` | 8 | 4 | 0 | 0 | **8** | 4 |
| `start_exec_script` | 7 | 6 | 0 | 0 | **7** | 6 |
| `checkhealth` | 6 | 4 | 0 | 0 | **6** | 4 |
| `checkname` | 2 | 2 | 4 | 3 | **6** | 5 |
| `checkkneeling` | 6 | 4 | 0 | 0 | **6** | 4 |
| `percentmana` | 6 | 4 | 0 | 0 | **6** | 4 |
| `walk` | 3 | 3 | 2 | 2 | **5** | 5 |
| `checkwebbed` | 5 | 4 | 0 | 0 | **5** | 4 |
| `checkarea` | 3 | 2 | 0 | 0 | **3** | 2 |
| `checkbleeding` | 0 | 0 | 3 | 3 | **3** | 3 |
| `matchfind` | 2 | 2 | 0 | 0 | **2** | 2 |
| `multimove` | 2 | 1 | 0 | 0 | **2** | 1 |
| `waitforre` | 1 | 1 | 0 | 0 | **1** | 1 |

Movement direction globals (`n`, `s`, `e`, `w`, `ne`, `nw`, `se`, `sw`, `u`, `d`, `o`,
`out`, `up`, `down`, `go2`) are **excluded from the table above as unmeasurable by this
method**. Their raw token counts are large (`s` = 955, `w` = 530, `n` = 460) but
inspection shows this is almost entirely noise: local variable names and block
parameters. Checking for a bare direction as a whole statement (`^\s*s\s*$`) finds only
**9 `s` and 1 `d` occurrences across all 456 files**. **Conclusion: single-letter
movement globals are effectively unused by the corpus** — scripts move via `move
"north"`, `DRCT.walk_to`, or `fput`. Cena does not need to reserve single-letter
identifiers in Lua's global namespace.

### Functions named in the brief that the corpus does not use

Verified by direct `grep` over all 456 files, these have **zero** occurrences:
`matchre`, `matchfind` variants beyond 2 uses, `fetchloot`, `survivepoison?`, `take`
(as a Lich global — the 191 `take` hits are all script-local method definitions or the
game verb inside strings), `toggle_*` (only 38 occurrences across 11 files, and the
top hits `toggle_ambients` / `toggle_che` are **script-local**, not Lich globals; the
real Lich globals `toggle_upstream` = 2, `toggle_unique` = 4, `toggle_echo` = 0).

`checkhealth`/`checkmana`/`checkstamina`/`checkspirit` are far less used than the brief
anticipated (6, 54, 22, 23 total) because the modern idiom is `Char.health` /
`percenthealth` / `DRStats.health`.

### The 64 globals no script uses

`_echo`, `abort!`, `alias_deprecated`, `bin2dec`, `calmed?`, `checkcalmed`,
`checkcutthroat`, `checkfamarea`, `checkfamnpcs`, `checkfampaths`, `checkfampcs`,
`checkfamroom`, `checkfamroomdescrip`, `checkfried`, `checkloot`, `checknotstanding`,
`checkprone`, `checkroomdescrip`, `checksaturated`, `checksilenced`, `checksitting`,
`checkspell`, `count_npcs`, `dec2bin`, `detachable_client_count`,
`detachable_client_primary?`, `detachable_client_register`,
`detachable_client_send_init`, `detachable_client_send_player_id`,
`detachable_client_unregister`, `detachable_clients_close`, `detachable_clients_respond`,
`detachable_clients_snapshot`, `detachable_listener_connected`, `die_with_me`,
`dispatch_client_input`, `echo_off`, `echo_on`, `fb_to_sf`, `handle_detachable_client`,
`hide_script`, `lich_shutdown`, `matchafter`, `matchbefore`, `matchboth`,
`matchfindexact`, `matchfindword`, `maxconcentration`, `noded_pulse`,
`percentconcentration`, `selectput`, `send_scripts`, `silenced?`, `start_scripts`,
`sync_detachable_client_globals`, `timetest`, `toggle_echo`, `unique_get?`,
`unique_send_to_script`, `unique_waitfor`, `unnoded_pulse`, `upstream_get?`,
`upstream_waitfor`, `watchhealth`.

**This is a 31% dead-surface finding and it is directly actionable**: Cena's Lua layer
can omit all 64 on day one without breaking a single existing script. The 14
`detachable_client_*` functions are Lich's newest (5.19→5.21) feature and have no corpus
uptake at all.

---

## 3. Class and module usage

Counted as `Const.method` and `Const[` occurrences after string/comment stripping.

### 3.1 Top namespaces overall

| Namespace | GSc | GSf | DRc | DRf | Total | Files | Nature |
|---|---:|---:|---:|---:|---:|---:|---|
| `DRC.*` | 0 | 0 | 3,806 | 188 | 3,806 | 188 | DR common lib (in Lich) |
| `UserVars.*` | 1,464 | 53 | 464 | 61 | 1,928 | 114 | cross-script persistent vars |
| `GameObj.*` | 1,621 | 108 | 13 | 6 | 1,634 | 114 | game object model |
| `Script.*` | 1,256 | 154 | 133 | 48 | 1,389 | 202 | script introspection |
| `Spell[...]` | 1,276 | 64 | 0 | 0 | 1,276 | 64 | spell table lookup |
| `Room.*` | 826 | 87 | 231 | 49 | 1,057 | 136 | current room |
| `CharSettings[...]` | 896 | 43 | 57 | 6 | 953 | 49 | per-character settings |
| `DRCI.*` | 0 | 0 | 940 | 109 | 940 | 109 | DR inventory lib |
| `Flags.*` | 0 | 0 | 785 | 63 | 785 | 63 | DR trigger flags |
| `XMLData.*` | 505 | 71 | 110 | 19 | 615 | 90 | raw game XML state |
| `Char.*` | 586 | 78 | 1 | 1 | 587 | 79 | character vitals |
| `Room[...]` | 480 | 28 | 31 | 3 | 511 | 31 | room table lookup |
| `Skills.*` | 494 | 24 | 1 | 1 | 495 | 25 | skill ranks |
| `DRCT.*` | 0 | 0 | 460 | 100 | 460 | 100 | DR travel lib |
| `DRCC.*` | 0 | 0 | 433 | 33 | 433 | 33 | DR crafting lib |
| `DRStats.*` | 0 | 0 | 343 | 65 | 343 | 65 | DR character stats |
| `Map.*` | 320 | 38 | 15 | 11 | 335 | 49 | pathfinding/map db |
| `DRSkill.*` | 0 | 0 | 296 | 59 | 296 | 59 | DR skill/exp |
| `DRRoom.*` | 0 | 0 | 268 | 59 | 268 | 59 | DR room contents |
| `Settings[...]` | 262 | 17 | 0 | 0 | 262 | 17 | per-script settings |
| `Log.*` | 259 | 17 | 0 | 0 | 259 | 17 | logging |
| `DownstreamHook.*` | 197 | 47 | 16 | 6 | 213 | 53 | game→client filter |
| `Stats.*` | 213 | 29 | 0 | 0 | 213 | 29 | stat values |
| `DRCA.*` | 0 | 0 | 171 | 48 | 171 | 48 | DR arcana lib |
| `Spells.*` | 138 | 7 | 0 | 0 | 138 | 7 | spell ranks by circle |
| `DRCM.*` | 0 | 0 | 131 | 43 | 131 | 43 | DR money lib |
| `Wounds.*` | 129 | 11 | 0 | 0 | 129 | 11 | wound severity |
| `Scars.*` | 122 | 10 | 0 | 0 | 122 | 10 | scar severity |
| `Vars[...]` | 67 | 10 | 0 | 0 | 67 | 10 | global vars |
| `UpstreamHook.*` | 61 | 29 | 15 | 8 | 76 | 37 | client→game filter |
| `Society.*` | 47 | 8 | 0 | 0 | 47 | 8 | society rank/status |
| `Bounty.*` | 77 | 3 | 0 | 0 | 77 | 3 | bounty task parsing |
| `CMan.*` | 62 | 6 | 0 | 0 | 62 | 6 | combat maneuvers |
| `Feat.*` | 20 | 4 | 0 | 0 | 20 | 4 | feats |
| `Lich.*` | 13 | 6 | 0 | 0 | 13 | 6 | engine internals |
| `Armor.*` | 7 | 2 | 0 | 0 | 7 | 2 | armor PSMs |
| `Shield.*` | 5 | 3 | 0 | 0 | 5 | 3 | shield PSMs |
| `Weapon.*` | 6 | 1 | 0 | 0 | 6 | 1 | weapon PSMs |
| `Infomon.*` | 6 | 2 | 0 | 0 | 6 | 2 | stat parser |
| `Watchfor.*` | 1 | 1 | 0 | 0 | 1 | 1 | async line trigger |
| `PSMS.*` | 0 | 0 | 0 | 0 | 0 | 0 | **never called directly** |

Four high-count entries in the raw data are **not Lich API** and are excluded above:
`TextSubs` (1,587 hits, 1 file — it is `dr-scripts/textsubs.lic`'s own class), `ELoot`
(1,321 / 2 files), `EBounty` (596 / 1), `EHerbs` (392 / 1), `BlackArts` (382 / 1),
`Util` (576 / 7). These are single-script internal namespaces. They inflate any naive
class census and were checked individually.

**Notable negatives.** `PSMS` is called **zero** times despite being the umbrella module
— scripts go straight to `CMan`/`Feat`/`Armor`/`Shield`/`Weapon`. `Watchfor` has exactly
**one** use in the entire corpus, `scripts/echild.lic:395`. `Infomon` is called directly
only 6 times (3× `Infomon.sync`, 2× `Infomon.db_refresh_needed?`, 1× `Infomon.get`) —
scripts consume Infomon's output through `Char`/`Stats`/`Skills`/`Spell`, not through
`Infomon` itself. **These three can be omitted from the Lua surface.**

### 3.2 Method-level breakdown (format: `method` — calls / files)

**`Script`** (1,389 calls): `current` 649/112 · `running?` 253/84 · `run` 227/64 ·
`exists?` 46/29 · `start` 42/19 · `self` 32/16 · `kill` 26/16 · `pause` 24/12 ·
`running` 20/9 · `list` 18/13 · `unpause` 16/12 · `paused?` 16/10 · `hidden` 10/6.
`Script.current` is the single most-called class method in the corpus. It is used mainly
for `Script.current.vars` (script arguments, 527 calls / 97 files) and
`Script.current.name` (387 / 62).

**`GameObj`** (1,634): `right_hand` 451/64 · `left_hand` 372/56 · `npcs` 171/40 ·
`loot` 164/38 · `inv` 134/31 · `pcs` 73/28 · `containers` 73/23 · `room_desc` 53/18 ·
`targets` 52/14 · `type_data` 32/6 · `new` 22/11 · `load_data` 9/7 · `sellable_data` 8/3.
Hands are 50% of all `GameObj` traffic.

**`XMLData`** (615): `game` 226/51 · `room_count` 86/8 · `room_title` 70/22 ·
`injuries` 44/10 · `room_exits` 28/6 · `name` 28/13 · `active_spells` 16/4 ·
`room_id` 14/5 · `server_time` 11/3 · `current_target_id` 11/4 · `next_level_text` 10/3 ·
`room_description` 9/5 · `bounty_task` 6/1 · `prepared_spell` 4/3.
`XMLData.game` (37% of the total) is used almost exclusively for game-type branching
(`"GSIV"` vs `"DR"`), not for state. Only ~10 distinct fields matter.

**`Char`** (587): `name` 278/57 · `stamina` 52/5 · `mana` 35/9 · `prof` 34/15 ·
`race` 31/1 · `level` 21/11 · `percent_health` 17/6 · `spirit` 13/4 · `health` 13/5 ·
`exp` 12/3 · `percent_stance` 9/2 · `percent_encumbrance` 9/2 ·
`total_wound_severity` 8/3 · `stance` 8/2.

**`Room`** (1,057 incl. bracket): `Room.current` 989/133 — this is the whole API.
`Room.list` 45/16 · `Room.ids_from_uid` 13/2 · `Room.current_or_new` 7/2 · `Room.tags` 3/2.

**`Map`** (335): `list` 96/24 · `ids_from_uid` 87/15 · `current` 74/8 · `dijkstra` 29/13 ·
`uids_add` 7/1 · `new` 7/1 · `uids` 5/1 · `save_json` 5/2 · `reload` 5/4 · `tags` 4/2 ·
`estimate_time` 4/2.

**`Spell`**: bracket access dominates completely — `Spell[...]` 1,276 calls / 64 files vs
`Spell.active` 26/10, `Spell.list` 9/2, `Spell.unlock_cast` 6/5, `Spell.lock_cast` 6/5,
`Spell.load` 4/3. The Lua binding must model `Spell` as an indexable table, not a module.

**`Skills`** (495) is a flat field-per-skill namespace with no methods: `elfire` 38/7 ·
`elwater` 32/7 · `elearth` 20/7 · `elair` 20/8 · `smc` 18/7 · `slblessings` 17/10 ·
`emc` 17/7 · `trading` 16/7 · `mmc` 14/7 · `magicitemuse` 13/6 · `arcanesymbols` 13/6 ·
`slreligion` 12/7 · `slsummoning` 11/7 · `mltransference` 11/8 — and ~40 more.
Same shape for `Stats` (`prof` 105/20, `level` 30/11, `race`, `dex`, `aur`, `dis`, …),
`Wounds`/`Scars` (`right` 25/4, `left` 23/4, `head` 10/4, `nerves` 8/3, `neck` 8/4,
`torso`, `nsys`, `rleg`, `rhand`, …) and `Spells` (per-circle rank: `wizard` 11/4,
`sorcerer` 11/4, `minorelemental` 12/5, …). In Lua these are all plain read-only tables.

**Settings family.** `CharSettings[...]` 953/49 · `Settings[...]` 262/17 ·
`Vars[...]` 67/10 · `GameSettings[...]` 19/2. Method form is rare:
`Settings.save` 39/15, `CharSettings.to_hash` 26/19, `Settings.load` 9/6,
`CharSettings.save` 8/6. **Bracket access is 5:1 over method access.** `UserVars` is the
opposite — it is accessed as `UserVars.<name>` (dynamic attribute), e.g.
`UserVars.tdusk` 216/2, `UserVars.op` 135/9, `UserVars.moons` 47/5. `UserVars.op` is
`bigshot.lic`'s profile blob shared with 8 other scripts.

**Hooks.** `DownstreamHook.remove` 119/52 · `DownstreamHook.add` 88/51 ·
`UpstreamHook.remove` 38/25 · `UpstreamHook.add` 29/27. The near-equal add/remove counts
reflect the near-universal idiom of pairing a hook with `before_dying`, e.g.
`scripts/` usage `DownstreamHook.add('room_check', current_room)` followed by
`before_dying { DownstreamHook.remove('room_check') }`. `DownstreamHook.add` takes a
**Proc that receives and returns the line**, which is why `proc {` appears 752 times.

### 3.3 DragonRealms modules

`DRC` (3,806 calls / 188 of 222 DR files — 85% of the DR corpus):
`bput` 1,603/160 · `message` 1,048/107 · `wait_for_script_to_complete` 290/78 ·
`right_hand` 225/66 · `left_hand` 219/58 · `fix_standing` 96/35 · `retreat` 47/17 ·
`safe_unpause_list` 34/14 · `release_invisibility` 34/16 · `beep` 30/20 ·
`log_window` 29/5 · `hide?` 23/7.

**`DRC.bput` is the single most important DR call.** It is send-and-match-response in one
(a blocking `fput` + `matchwait` over a list of patterns), and 160 of 222 DR scripts use
it. Nothing on the GemStone side is equivalent — GemStone scripts hand-roll
`fput` + `waitfor`/`matchtimeout`.

`DRCI` (940 / 109): `put_away_item?` 152/51 · `exists?` 128/37 · `stow_hands` 111/41 ·
`get_item?` 102/32 · `dispose_trash` 95/36 · `in_hands?` 65/24 · `stow_item?` 41/15 ·
`get_item_if_not_held?` 30/18 · `in_left_hand?` 24/6 · `get_item_safe` 19/2 ·
`get_item` 19/10 · `stow_hand` 15/8.

`DRCT` (460 / 100): `walk_to` 334/96 — 96 of 222 DR scripts call `DRCT.walk_to`. Then
`order_item` 66/18 · `buy_item` 29/12 · `dispose` 10/4 · `sort_destinations` 6/5.

`DRCC` (433 / 33): `stow_crafting_item` 176/28 · `get_crafting_item` 92/28 ·
`check_for_existing_sigil?` 24/2 · `check_consumables` 23/7 · `find_recipe2` 14/7.

`DRCA` (171 / 48): `crafting_magic_routine` 19/10 · `cast_spell` 19/12 ·
`release_cyclics` 18/8 · `check_discern` 13/9 · `do_buffs` 11/8 · `cast?` 11/7 ·
`prepare?` 9/7 · `harness_mana` 6/5.

`DRCM` (131 / 43): `ensure_copper_on_hand` 77/33 · `wealth` 16/5 · `deposit_coins` 8/7 ·
`convert_to_copper` 7/7 · `withdraw_exact_amount?` 5/5 · `minimize_coins` 5/4.

`DRSkill` (296 / 59): `getxp` 176/41 · `getrank` 79/38 · `getmodrank` 16/3 · `list` 9/4.

`DRStats` (343 / 65): `circle` 52/17, then a wall of profession predicates —
`thief?` 37/14 · `trader?` 26/11 · `barbarian?` 24/8 · `necromancer?` 19/7 ·
`moon_mage?` 17/7 · `paladin?` 13/10 · `empath?` 13/9 · `cleric?` 12/11 ·
`warrior_mage?` 8/6 — plus vitals `health` 34/9, `mana` 23/6.

`DRRoom` (268 / 59): `room_objs` 108/19 · `npcs` 58/23 · `pcs` 57/22 · `title` 29/14 ·
`pcs_prone` 5/1 · `dead_npcs` 5/2 · `exits` 2/2.

`DRSpells` (93 / 23): `active_spells` 76/23 · `known_spells` 15/5 · `slivers` 2/1.

`Flags` (785 / 63): `add` 283/63 · `reset` 270/55 · `delete` 231/55. `Flags` is the DR
equivalent of a named regex trigger set — `Flags.add(name, *patterns)` then poll
`Flags[name]` (466 bracket reads / 62 files).

`DRC.bput`'s signature (`lich-5/lib/dragonrealms/commons/common.rb:102`) explains why it
dominates: it takes a message plus varargs matches, defaults `timeout` to 15s
(`common.rb:104`), calls `waitrt?` implicitly unless `ignore_rt` (`common.rb:117`), and
**coerces every string match to a case-insensitive Regexp** at `common.rb:120`
(`matches.map! { |item| item.is_a?(Regexp) ? item : /#{item}/i }`). Cena's Lua equivalent
must reproduce all four behaviours or 160 DR scripts change semantics silently.

---

## 4. Ruby-language features the corpus depends on

This section drives the Ruby→Lua translation cost estimate. Counts are occurrences /
distinct files, over 456 files.

### 4.1 Blocks, procs, higher-order functions — **unavoidable**

| Feature | GSc | GSf | DRc | DRf | Total | Files |
|---|---:|---:|---:|---:|---:|---:|
| brace block `{ \|x\| … }` | 4,014 | 180 | 1,214 | 140 | 5,228 | 320 |
| `do \|x\| … end` | 1,534 | 123 | 567 | 116 | 2,101 | 239 |
| `x = proc { … }` | 738 | 82 | 14 | 8 | 752 | 90 |
| `&:symbol` shorthand | 269 | 63 | 92 | 38 | 361 | 101 |
| `lambda` / `->()` | 79 | 8 | 16 | 4 | 95 | 12 |
| `Proc.new` | 72 | 10 | 0 | 0 | 72 | 10 |
| `yield` | 52 | 21 | 6 | 2 | 58 | 23 |
| `block_given?` | 14 | 6 | 0 | 0 | 14 | 6 |

**363 of 456 files (79.6%) use blocks.** This is the single most pervasive Ruby feature
in the corpus. Lua closures cover the semantics, but there is no `yield`/`block_given?`
equivalent and no implicit block parameter — every block-taking API must become an
explicit function argument. The `&:symbol` idiom (361 uses) has no Lua analogue and must
be expanded to `function(x) return x:method() end` by hand or by the translator.
**Difficulty: mechanical but high-volume.**

### 4.2 Regular expressions — **the hardest single dependency**

| Feature | GSc | GSf | DRc | DRf | Total | Files |
|---|---:|---:|---:|---:|---:|---:|
| `=~` | 5,782 | 187 | 665 | 123 | 6,447 | 310 |
| `(?:…)` non-capturing | 2,056 | 117 | 120 | 32 | 2,176 | 149 |
| `$1`..`$9` backrefs | 1,643 | 93 | 23 | 6 | 1,666 | 99 |
| `.gsub` / `.sub` | 850 | 101 | 151 | 44 | 1,001 | 145 |
| `case` / `when /regex/` | 566 | 50 | 716 | 79 | 1,282 | 129 |
| `!~` | 672 | 70 | 41 | 20 | 713 | 90 |
| lookahead/lookbehind `(?=`,`(?!`,`(?<` | 352 | 55 | 134 | 30 | 486 | 85 |
| named captures `(?<name>…)` | 286 | 41 | 113 | 23 | 399 | 64 |
| `Regexp.union` / `.escape` / `.new` | 328 | 75 | 41 | 21 | 369 | 96 |
| `.scan` | 78 | 39 | 55 | 25 | 133 | 64 |
| `$~` / `MatchData` | 6 | 5 | 0 | 0 | 6 | 5 |

**310 of 456 files (68%) match with `=~`.** This is the critical finding of section 4.
**Lua's built-in patterns are not regular expressions** — they have no alternation, no
non-capturing groups, no lookaround, no named captures, no `\b`. The corpus uses all of
those: 2,176 non-capturing groups, 486 lookarounds, 399 named captures. Lua's patterns
cannot express any of them.

**Consequence for Cena: the Lua layer must ship a real regex engine** (e.g. bind Rust's
`regex` crate, or `fancy-regex` for the 486 lookaround uses, which `regex` does not
support). This is not optional and it is not a nice-to-have — without it roughly
two-thirds of the corpus cannot be ported at all. It also implies `$1`..`$9` (1,666 uses)
needs an equivalent: Ruby's implicit post-match globals must become explicit capture
returns, which is a mechanical but invasive rewrite of 99 files.

### 4.3 Metaprogramming — **light, and that is good news**

| Feature | GSc | GSf | DRc | DRf | Total | Files |
|---|---:|---:|---:|---:|---:|---:|
| `.send()` / `__send__` / `public_send` | 250 | 41 | 19 | 7 | 269 | 48 |
| `respond_to?` | 85 | 24 | 44 | 15 | 129 | 39 |
| `const_get` / `const_set` / `const_defined?` | 124 | 17 | 0 | 0 | 124 | 17 |
| `method_missing` | 22 | 20 | 3 | 2 | 25 | 22 |
| `define_method` | 15 | 6 | 0 | 0 | 15 | 6 |
| `instance_variable_get`/`set` | 14 | 6 | 1 | 1 | 15 | 7 |
| `eval(...)` | 34 | 5 | 11 | 2 | 45 | 7 |
| `instance_eval` | 3 | 2 | 0 | 0 | 3 | 2 |
| `class_eval` / `module_eval` | 0 | 0 | 0 | 0 | **0** | 0 |

**Only 9 files in 456 use `eval`, and only 22 define `method_missing`.** Metaprogramming
is far lighter than the size of the corpus would suggest. Lua metatables cover
`method_missing` (`__index`) and `respond_to?` cleanly. This materially lowers the
translation risk — but see §5, because the `eval` uses that do exist are concentrated in
the largest and most-depended-on scripts.

### 4.4 Monkey-patching of core classes — **effectively zero**

Searching all 456 files for `class String` / `class Array` / `class Hash` /
`class Integer` / `class Float` / `class Numeric` / `class Object` / `class Symbol` /
`class Range` / `class Time` at any indentation returns **0 files**. Reopening of *Lich*
classes is almost as rare — 4 sites total: `scripts/puritan.lic:28` (`class Char`),
`scripts/spa.lic:86` (`class Char`), `scripts/sk.lic:45` (`module Games`) and
`scripts/sk.lic:47` (`class Spell`).

**This is the best news in this document.** A common fear in Ruby→Lua ports is that the
corpus has silently redefined core semantics; it has not. Cena does not need an
open-class mechanism.

### 4.5 Concurrency

| Feature | GSc | GSf | DRc | DRf | Total | Files |
|---|---:|---:|---:|---:|---:|---:|
| `sleep` | 1,035 | 141 | 13 | 6 | 1,048 | 147 |
| `Thread.new` | 82 | 37 | 0 | 0 | 82 | 37 |
| `Thread.kill/exit/current/list` | 23 | 6 | 0 | 0 | 23 | 6 |
| `Mutex` | 5 | 3 | 0 | 0 | 5 | 3 |

**No DragonRealms script uses threads at all.** On the GemStone side 37 files do, led by
`scripts/jinx.lic` (10 `Thread.new`: lines 1919, 1955, 1970, 1986, 1999, 2023, 2171, …),
`scripts/bigshot.lic` (7: lines 678, 698, 6102, 6654, 6761, 6788), `scripts/notify.lic`,
`scripts/healbot_core.lic` and `scripts/cluster.lic` (5 each). Only 3 files use a `Mutex`,
which means the threaded scripts are largely relying on the GIL for safety — a Rust
runtime with real parallelism would expose latent races. Cena should give Lua scripts
cooperative coroutines, not OS threads, and port the 37 threaded scripts individually.

### 4.6 Exceptions

`rescue` 622 / 104 files · `begin` 391 / 98 · `raise` 185 / 56 · `retry` 103 / 45 ·
`ensure` 71 / 36. Lua's `pcall`/`error` covers `begin`/`rescue`/`raise`. **`retry`
(103 uses / 45 files) has no Lua equivalent** and must be rewritten as an explicit loop.
`ensure` (71 / 36) needs a `<close>` variable or a manual `pcall`-and-cleanup wrapper.

### 4.7 Strings, symbols, serialization

| Feature | Total | Files | Lua difficulty |
|---|---:|---:|---|
| string interpolation `#{}` | 21,896 | 410 | mechanical (`..` or `string.format`) |
| symbols `:foo` | 20,815 | 267 | map to strings; **APPROX**, see below |
| `%w[]` / `%i[]` word arrays | 453 | 118 | mechanical |
| heredocs `<<~` / `<<-` | 102 | 32 | mechanical |
| `YAML` | 187 | 70 | needs a YAML lib in Rust |
| `JSON` | 60 | 17 | easy |
| `Struct.new` | 53 | 25 | plain tables |
| `OpenStruct` | 60 | 25 | metatables — **and see §4.8** |
| `Marshal` | 28 | 10 | must be replaced entirely |

**APPROX — the symbol count.** The 20,815 figure counts `:foo` tokens after string
stripping, but it still catches some ternary `? :` and scope-resolution contexts. The
top hits are unambiguous real symbols used as hash keys and Gtk arguments (`:steps` 891,
`:product` 884, `:fill` 526, `:padding` 518, `:expand` 376, `:type` 326, `:rank` 299,
`:name` 231). Treat the magnitude as reliable and the exact number as ±10%.

Symbols vs strings matters because Ruby distinguishes `h[:x]` from `h["x"]` and Lua does
not. Collapsing symbols to strings is correct in almost every case, but any script that
stores *both* in one hash will silently merge keys. **UNVERIFIED:** I did not test for
hashes keyed by both a symbol and the identical string. Detecting that needs an AST pass
over the corpus, not grep.

### 4.8 `OpenStruct`, and the DragonRealms script contract

The DR corpus has a near-universal script skeleton, and it is built on `OpenStruct`.
`get_settings` is called by **172 of 222 DR scripts** and `parse_args` by **139 of 222**.
A representative head, `dr-scripts/athletics.lic:5-29`:

```ruby
class Athletics
  def initialize
    @settings = get_settings
    @athletics_options = get_data('athletics').athletics_options
    @performance_pause = @settings.performance_pause
    ...
    arg_definitions = [[
      { name: 'wyvern', regex: /^wy.*/i, optional: true, description: '...' },
      ...
```

Both `get_settings` and `get_data` return an `OpenStruct`
(`lich-5/lib/global_defs.rb:69` and `:79`, delegating to
`lich-5/lib/common/setup_files.rb:78` and `:119`). The settings object is built by
merging `base.yaml`, `base-empty.yaml`, resolved `include` files and per-character files
in that order (`setup_files.rb:93-111`), with a "union_keys" pass that array-unions
selected keys instead of overwriting (`setup_files.rb:95-101`). `parse_args`
(`lib/global_defs.rb:90` → `lib/common/arg_parser.rb:25`) also returns an `OpenStruct`.

**This is a concrete, bounded porting requirement**: Cena must reproduce (a) dot-access
over a YAML-derived table, (b) the multi-file merge order, (c) the `union_keys` semantics,
and (d) the `arg_definitions` regex-based argument parser. Get those four right and 172
of 222 DR scripts keep their entire configuration layer unchanged. Get them wrong and
essentially the whole DR corpus breaks.

### 4.9 Cross-script coupling

`require` appears 252 times / 99 files, but **`require_relative` is used exactly 0
times** — no script loads another script as a library. Cross-script communication instead
happens through four channels:

1. **`UserVars` / `Vars`** — 1,928 calls / 114 files.
2. **Ruby globals `$foo`** — 111 files. Top: `$clean_lich_char` 437, `$debug_mode_ct` 413,
   `$lich_char` 283, `$killtracker` 182, `$sbounty` 179, `$frontend` 92,
   `$data_dir` 51, `$lich_dir` 26, `$script_dir` 12.
3. **`start_script` / `Script.run` / `send_to_script`** — 165 files start, kill or pause
   another script by name.
4. **`DRC.wait_for_script_to_complete`** — 290 calls / 78 DR files.

`$frontend` (92 uses) is read to branch on client type: `'stormfront'` 40+1,
`'profanity'` 20, `'genie'` 2, `'suks'` 1, `'saga'` 1. **Cena must expose a `$frontend`
equivalent and decide what value it reports**, or ~40 scripts take the wrong branch.

`$_SERVERBUFFER` (6 uses / 4 files) and `$_CLIENTBUFFER` (4 / 2) are the raw stream
buffers; usage is rare enough to port case-by-case.

---

## 5. The hard cases

Ranked by how much of the corpus they block, not by size alone.

### 5.1 `scripts/bigshot.lic` (10,246 lines) — **user-supplied Ruby as configuration**

The hardest single file, and it is load-bearing: **19 scripts reference `bigshot` or
`UserVars.op`** (`BlackArts.lic`, `bsprofiles.lic`, `combo.lic`, `disarmed.lic`,
`ebounty.lic`, `ecleanse.lic`, `invoker.lic`, `kswole.lic`, `manaleech.lic`,
`mechfire.lic`, `perfume.lic`, `sammu.lic`, `sbounty.lic`, `sbounty-bigshot.lic`,
`sspell.lic`, `tdusk.lic`, `treim.lic`, `webdispel.lic`).

The blocker is that bigshot stores **Ruby expression strings in its user profile and
`eval`s them at runtime**:

- `bigshot.lic:8887` — `return eval(@WOUNDED_EVAL) if @WOUNDED_EVAL`
- `bigshot.lic:8938` — `result = eval(@BOUNTY_EVAL)`

`@BOUNTY_EVAL` is assigned from user config at `bigshot.lic:3854` (`@BOUNTY_EVAL =
bounty_eval`) and is a serialized profile key (`bigshot.lic:2726`). So a user's saved
settings contain arbitrary Ruby that the script executes to decide whether to keep
hunting. **No translator can convert this**: the Ruby is data, not source. Porting
bigshot means either shipping a Ruby expression evaluator, redesigning the profile format
into a declarative predicate DSL, or accepting that every existing bigshot profile breaks.
It also carries 328 `Gtk` references and 7 `Thread.new` (lines 678, 698, 6102, 6654,
6761, 6788).

### 5.2 `scripts/bsprofiles.lic` (2,007 lines) — **evaluates another script's source**

Worse in kind, smaller in blast radius. It reads `bigshot.lic` off disk and `eval`s
fragments of it:

- `bsprofiles.lic:199` — `data = eval(literal, TOPLEVEL_BINDING, bigshot_path)`, where
  `literal` is extracted by regex from bigshot's source at `bsprofiles.lic:196`
  (`text[/^    @@categories = (\{.*?^    \})/m, 1]`).
- `bsprofiles.lic:575` — `eval(bs_setup_src, binding, BSProfiles.bigshot_path, ...)`,
  which evaluates bigshot's entire `Bigshot::Setup` class body into the current binding
  because "the class exists; otherwise evaluate it from bigshot.lic in this script's
  namespace" (comment at `bsprofiles.lic:570-572`).

This is cross-script source injection with explicit binding control. Lua has
`load`/`setfenv`-style equivalents, but reproducing Ruby's `TOPLEVEL_BINDING` and lexical
`binding` semantics is not a translation — it is a rewrite. **Recommendation: do not port;
redesign as a data file bigshot and bsprofiles both read.**

### 5.3 `scripts/jinx.lic` (2,686 lines) — **runtime class synthesis + 10 threads**

- `jinx.lic:177` — `def self.method_missing(method, *args)`
- `jinx.lic:1812` — `log_class.send(:define_method, :out) do |msg, label: :debug|`
- `jinx.lic:1828` — `log_class.send(:define_method, :mono) do |text|`
- 10 × `Thread.new` (lines 1919, 1955, 1970, 1986, 1999, 2023, 2171, …)

It builds classes at runtime and drives them from ten concurrent threads. `define_method`
maps to assigning closures into a metatable, which is tractable; the thread count is the
real problem under a Rust runtime that does not have Ruby's GIL.

### 5.4 `scripts/0net.lic` (1,781 lines) — **TLS networking inside a game script**

- `0net.lic:331` — an embedded `OpenSSL::X509::Certificate` PEM literal (a pinned root CA)
- `0net.lic:340-345` — `OpenSSL::X509::Store.new`, `SSLContext.new`, `OP_NO_SSLv2`,
  `VERIFY_PEER`
- `0net.lic:346` — `Timeout.timeout(10) { TCPSocket.open(hostname, port) }`
- `0net.lic:347` — `OpenSSL::SSL::SSLSocket.new(socket, ssl_context)`
- `0net.lic:207` — `def method_missing(method, *args)`
- `Thread.new` at `0net.lic:1230` and `:1244`

This is a chat/IPC network client embedded as a game script, with a hard-coded pinned
certificate. Cena would have to expose TLS sockets to Lua, and the pinned CA means the
port must keep working against the same server. **This is a policy decision, not just an
engineering one** — do community Lua scripts get raw socket access?

### 5.5 The GTK cluster — **51 GemStone scripts with desktop-only GUIs**

`Gtk` appears 2,990 times across 55 files. The heaviest: `bigshot.lic` 328,
`sloot.lic` 311, `ebounty.lic` 270, `boon.lic` 255, `eloot.lic` 248, `BlackArts.lic` 132,
`map.lic` 122, `creaturebar.lic` 119, `armor.lic` 117, `sbounty.lic` 115,
`bsprofiles.lic` 112, `calibrate_creaturebar.lic` 111, `mechfire.lic` 91,
`fletchit.lic` 78, `uberfletch.lic` 75.

**Cena must run on mobile**, and GTK does not. These 51 scripts do not have a
translation — they have a UI redesign. `Gtk.queue` (273 uses / 53 files) is the
thread-marshalling idiom and would map to whatever Cena's UI-thread dispatch is, but the
widget trees themselves have to be re-expressed against VellumFE's frontend abstraction.
Only 4 DR scripts touch GTK, so the DR corpus is essentially mobile-ready and the
GemStone corpus is not.

### 5.6 The large parser scripts — big but **mechanical**

`BlackArts.lic` (9,184 lines, 253 `=~`, 132 `Gtk`) and `eloot.lic` (8,087 lines,
307 `=~`, 248 `Gtk`) are the second and third largest GemStone scripts. Apart from their
GUIs they are conventional: regex over game text, tables, blocks. They are expensive in
person-hours and cheap in risk — **provided Cena ships a real regex engine** (§4.2).
Likewise `dr-scripts/combat-trainer.lic` (6,964 lines, 334 `DRC.`/`DRCI.`/`DRCT.` calls)
is the largest DR script but uses no threads, no `eval` and no GTK; it is a pure
consumer of the DR common library and ports as easily as that library does.

### 5.7 Summary of blockers

| Blocker | Scripts affected | Tractability |
|---|---:|---|
| GTK desktop GUI | 55 | Redesign required; blocks mobile |
| Real regex needed (lookaround/named/non-capturing) | ~310 | Solved by binding a Rust regex crate |
| `Thread.new` | 37 | Port individually to coroutines |
| `$1`..`$9` implicit match globals | 99 | Mechanical rewrite |
| `retry` keyword | 45 | Mechanical rewrite to loops |
| `eval` of user/foreign Ruby | 9 (2 severe) | Not translatable; redesign |
| `method_missing` | 22 | Metatable `__index` |
| Raw TLS sockets | 1 | Policy decision |

---

## 6. Verdict — the Lua API surface, ranked by corpus usage

"Scripts broken" = number of the 456 corpus files that contain at least one use of the
capability, measured file-by-file with string/comment stripping. A script is counted once
per capability regardless of how many times it calls it. **APPROX** throughout: a file
that merely mentions a capability in dead code is still counted, so these are upper
bounds on breakage — but they are the right upper bounds for planning, because a missing
API is a load-time or first-call failure either way.

### Tier 0 — without these, nothing runs (>60% of corpus)

| # | Capability | Scripts broken | % | Notes |
|---:|---|---:|---:|---|
| 1 | `echo` / `respond` / `_respond` / `DRC.message` — write to client | **380** | 83.3% | Trivial to implement; universally required |
| 2 | Closures passed to APIs (blocks) | **363** | 79.6% | Lua-native; `&:sym` needs expansion |
| 3 | `fput` / `put` / `multifput` / `DRC.bput` — send to game | **314** | 68.9% | `DRC.bput` needs match+timeout+implicit `waitrt?` |
| 4 | **Real regex** (`=~`, `!~`, captures, lookaround) | **310** | 68.0% | **Must bind a Rust regex crate; Lua patterns insufficient** |
| 5 | `pause` / `sleep` | **294** | 64.5% | Must be coroutine-yielding, not blocking |

### Tier 1 — core engine surface (20–45%)

| # | Capability | Scripts broken | % | Notes |
|---:|---|---:|---:|---|
| 6 | `Script.*` introspection (`current`, `running?`, `run`, `exists?`) | **202** | 44.3% | `Script.current` alone = 649 calls |
| 7 | `DRC.*` common library | **188** | 41.2% | 85% of the DR corpus; `bput` is 1,603 of 3,806 calls |
| 8 | `before_dying` / `undo_before_dying` — cleanup hook | **184** | 40.4% | Pairs with hook removal; needed for correctness not just convenience |
| 9 | String interpolation | **184** | 40.4% | Mechanical translation |
| 10 | `get_settings` + `get_data` (YAML→OpenStruct, merge + union_keys) | **176** | 38.6% | 172 of 222 DR scripts; see §4.8 |
| 11 | Roundtime waits (`waitrt?`, `waitcastrt?`, `checkrt`, `checkcastrt`) | **174** | 38.2% | Blocking semantics must match Lich exactly |
| 12 | Start/kill/pause other scripts | **165** | 36.2% | `start_script`, `Script.run`, `kill_script`, `pause_script` |
| 13 | Movement (`move`, `walk`, `goto`, `DRCT.walk_to`) | **152** | 33.3% | `DRCT.walk_to` alone = 96 DR files |
| 14 | Hand inspection (`checkleft`/`checkright`/`GameObj.*_hand`/`DRC.*_hand`/`empty_hands`) | **146** | 32.0% | 451+372 `GameObj` hand calls |
| 15 | `parse_args` + `arg_definitions` | **144** | 31.6% | 139 of 222 DR scripts |
| 16 | `Room.current` and room state | **138** | 30.3% | `Room.current` = 989 calls |
| 17 | `DRSkill` / `DRStats` / `DRRoom` / `DRSpells` | **127** | 27.9% | DR character/room state |
| 18 | `UserVars` / `Vars` — cross-script persistent vars | **125** | 27.4% | Dot-attribute access, dynamic keys |
| 19 | Script arguments (`Script.current.vars`, `variable[]`) | **121** | 26.5% | GS side; DR uses `parse_args` instead |
| 20 | `waitfor` / `wait_while` / `wait_until` | **116** | 25.4% | |
| 21 | `GameObj.*` — object model (hands, npcs, loot, inv, pcs, containers) | **114** | 25.0% | ~10 fields cover 95% of use |
| 22 | Ruby globals `$foo` — cross-script IPC | **111** | 24.3% | Incl. `$frontend` (92 uses, branches on client type) |
| 23 | `DRCI.*` — DR inventory | **109** | 23.9% | 12 methods cover it |
| 24 | Exceptions (`rescue`/`raise`/`ensure`/`retry`) | **102** | 22.4% | `retry` (45 files) has no Lua equivalent |
| 25 | `DRCT.*` — DR travel | **100** | 21.9% | `walk_to` is 334 of 460 calls |
| 26 | `Char` / `Stats` / `Skills` / `Society` | **91** | 20.0% | Flat read-only tables |
| 27 | `XMLData.*` | **90** | 19.7% | ~10 fields; `XMLData.game` is 37% of use |

### Tier 2 — substantial but bounded (10–20%)

| # | Capability | Scripts broken | % | Notes |
|---:|---|---:|---:|---|
| 28 | `require` of stdlib/gems | **78** | 17.1% | Needs a Lua stdlib mapping decision |
| 29 | `Settings` / `CharSettings` / `GameSettings` (bracket-indexed) | **75** | 16.4% | Bracket access 5:1 over methods |
| 30 | `DownstreamHook` / `UpstreamHook` (proc filters on the stream) | **71** | 15.6% | The proc receives and returns the line |
| 31 | `Spell[...]` table + `Spell.active` | **70** | 15.4% | **Carries an `eval` hazard, see below** |
| 32 | `Flags` (DR named trigger sets) | **64** | 14.0% | `add`/`reset`/`delete` + bracket read |
| 33 | **`Gtk` GUI** | **55** | 12.1% | **No mobile path; redesign required** |
| 34 | Vitals (`checkmana`/`percenthealth`/`maxmana`/…) | **52** | 11.4% | Many aliases over few values |
| 35 | `Map.*` pathfinding (`list`, `ids_from_uid`, `current`, `dijkstra`) | **50** | 11.0% | |

### Tier 3 — long tail (<11%), port opportunistically

`define_method`/`.send()` 48 · `YAML` 43 · `Thread.new` 37 ·
`match`/`matchwait`/`matchtimeout` legacy FSM 30 · `Struct`/`OpenStruct` 25 ·
`method_missing` 22 · `JSON` 15 · `eval` 9.

The legacy `match`/`matchwait` goto-FSM (30 files) is worth calling out: it is a
StormFront-era idiom concentrated in very few files. Counting only statement-position
uses, `matchwait` appears in exactly **4** files (`ForgeMaster.lic`, `crits.lic`,
`disarmed.lic`, `eg_docent.lic`) and `goto` in exactly **2** (`ForgeMaster.lic`,
`MyFletch.lic`). `ForgeMaster.lic` is the archetype (`ForgeMaster.lic:289-293`:
`match "BUYAPRON", "for more options"` … `matchwait`). Porting those few scripts by hand
is cheaper than implementing a goto-label FSM in Lua.

### Explicit non-requirements (measured, not assumed)

Cena's Lua layer can **omit all of the following** without breaking any corpus script:

- The **64 unused globals** listed in §2, including all 14 `detachable_client_*`.
- **`PSMS`** — 0 direct calls. Expose `CMan` (62), `Feat` (20), `Armor` (7), `Shield` (5),
  `Weapon` (6) instead.
- **`Watchfor`** — 1 use in 456 files (`scripts/echild.lic:395`).
- **`Infomon`** as a script-facing API — 6 direct calls; scripts read its output via
  `Char`/`Stats`/`Skills`/`Spell`.
- **Single-letter movement globals** (`n`,`s`,`e`,`w`,`u`,`d`,`o`,…) — 10 real
  statement-position uses across the entire corpus.
- **Open classes / core-class monkey-patching** — 0 files.
- **`require_relative`** — 0 uses; no script loads another as a library.
- **`class_eval` / `module_eval`** — 0 uses.

### The three decisions this census forces

1. **Ship a real regex engine bound into Lua.** 310 of 456 scripts (68%) use `=~`, and
   they use 2,176 non-capturing groups, 486 lookarounds and 399 named captures that Lua
   patterns cannot express. `fancy-regex` covers the lookarounds; the `regex` crate alone
   does not. This is the highest-leverage item in the entire port.

2. **Reproduce the DR settings contract exactly** (`get_settings`, `get_data`,
   `parse_args`, YAML merge order, `union_keys`, OpenStruct dot-access —
   `lich-5/lib/common/setup_files.rb:78-120`, `lib/common/arg_parser.rb:25`). It is a
   small, well-specified surface that 172 of 222 DR scripts depend on completely.

3. **Decide the GTK story before promising corpus portability.** 55 GemStone scripts —
   including 4 of the 5 largest — have desktop-only GUIs with no mobile path. The DR
   corpus (4 GTK files) is essentially mobile-ready; the GemStone corpus is not. Any claim
   that "existing scripts run on Cena" needs this qualified.

### One hazard that is not in any table

`Spell[...]` is used 1,276 times across 64 files, and Lich's own `Spell` implementation
**`eval`s its data at runtime**: `lich-5/lib/common/spell.rb:344`
(`result = proc { eval(formula) }.call.to_f`), `:678` (`proc { eval(@cast_proc) }.call`),
`:877` and `:910`. Spell costs, durations and cast procedures are stored as *Ruby
expression strings* in the spell data files. So even scripts that never write `eval`
themselves are transitively depending on a Ruby evaluator through `Spell`. Cena must
either translate that formula language into something Lua-evaluable or reimplement the
spell data as declarative values. **UNVERIFIED:** I did not enumerate how many distinct
formula strings exist in Lich's spell data or how complex they are — that needs a pass
over the effect-list XML referenced at `spell.rb:193`, which is fetched from GitHub at
runtime and is not present in this checkout.

---

## Reproducibility

The three measurement passes used are:
1. Global census — tokenize each line after stripping quotes/comments/regex literals,
   match against the 208 names extracted from `lich-5/lib/global_defs.rb`.
2. Class census — scan for `Const.method` and `Const[` after the same stripping.
3. Breakage census — per-file boolean match of 42 capability regexes over the stripped
   body of each of the 456 files.

Known limitations, restated:

- **`=begin`/`=end` block comments are not stripped**, and 430 of 456 files contain at
  least one. Most are the short repository docstring header seen at
  `dr-scripts/athletics.lic:1-3`, so the inflation is small, but counts for any
  identifier that also appears in documentation prose are upper bounds.
- Multi-line method calls are counted at their first line only, so call counts for
  APIs commonly written across several lines are slight undercounts.
- A capability named only in dead or unreachable code still counts toward "scripts
  broken", making §6 an upper bound.
- Identifiers that collide with common English words or block parameters — notably the
  single-letter movement globals — were excluded from the tables rather than reported as
  measured, with the reasoning shown inline in §2.
- All per-method breakdowns in §3 count textual `Const.method` occurrences; a script that
  aliases a constant (`X = GameObj`) would be missed. Spot checks found no such aliasing,
  but it was not exhaustively verified.
