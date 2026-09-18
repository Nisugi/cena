# Lich-5 GemStone IV Game Libraries — Porting Inventory

**Scope and date.** Written 2026-09-17 against the working copy at
`E:/Cena/reference/lich-5`. Covers every file under `lib/gemstone/` (27 top-level
`.rb` files + 8 subdirectories, 702 files total), the GemStone-relevant parts of
`lib/common/` (`spell.rb`, `gameobj.rb`, `inventory.rb`), `lib/magic-info.rb`, the
read-API façades in `lib/attributes/`, and the loader `lib/common/gameloader.rb`
that wires them together. Usage counts against the community corpus come from
`E:/Cena/reference/scripts` (251 `.lic` files). Line counts are `wc -l`; byte
counts are `wc -c`. Paths below are relative to `lich-5/` unless stated.

Everything here is GemStone-only. `GameLoader.load!` (`lib/common/gameloader.rb:87-92`)
branches on `XMLData.game`: `/GS/` loads `GameLoader.gemstone`, `/DR/` loads
`GameLoader.dragon_realms`. None of the files in this document load for DragonRealms.

---

## 0. Load order and topology

`GameLoader.gemstone` (`lib/common/gameloader.rb:15-57`) is the authoritative
dependency order. Notable facts for a port:

| Fact | Evidence |
|---|---|
| 37 explicit `require`s, in a hand-tuned order | `gameloader.rb:17-53` |
| `creature.rb` is deliberately **not** preloaded — `combat/tracker.rb` pulls it | `gameloader.rb:48` (comment), `combat/processor.rb:8` |
| Two background threads are started at load: `ActiveSpell.watch!` and `Infomon.watch!` | `gameloader.rb:54-55` |
| `common_before` loads `Spell` before any GemStone file | `gameloader.rb:9`, called at `:16` |
| `psms.rb` requires all 8 PSM category files at require-time | `gemstone/psms.rb:30-37` |
| `critranks.rb` `Dir.glob`s and `load`s 21 table files at require-time | `gemstone/critranks.rb:22-26`, invoked at `:130` |

The two *push* paths that feed all of this from the game stream are both in
`lib/games.rb`:

```
lib/games.rb:1395  def process_game_specific_data(server_string, stripped_server = nil)
lib/games.rb:1399    Infomon::XMLParser.parse(server_string)          # raw, XML intact
lib/games.rb:1401    Infomon::Parser.parse(line) unless line.empty?   # per stripped line
```

This is a *direct call from the game thread*, not a hook. By contrast
`Combat::Tracker` installs itself as a removable downstream hook
(`combat/tracker.rb:517-563`, `DownstreamHook.add(@hook_id, segment_buffer, persist: true)`).
That distinction matters for Cena: infomon-class parsing is in the hot path and
cannot be disabled; combat tracking is opt-in and off by default
(`combat/tracker.rb:58` `enabled: false, # Disabled by default, user must enable`).

---

## 1. Summary table

Difficulty key: **data-only** = the file is a literal table, port by converting
the serialization; **pure logic** = arithmetic/state, no game I/O, unit-testable
in isolation; **needs game I/O** = sends a command and blocks on the response
(`dothistimeout` / `Lich::Util.issue_command`), so it needs Cena's command
round-trip primitive before it can exist at all.

| Module | File(s) | LOC | What it owns | How it learns it | Rust port difficulty |
|---|---|---:|---|---|---|
| Infomon core | `gemstone/infomon.rb` | 312 | Key/value character-state store, SQLite-backed, per-character table | Written by the parsers; read by everything | pure logic (needs an sqlite crate + async write queue) |
| Infomon cache | `gemstone/infomon/cache.rb` | 55 | Write-through in-memory map with DB read fallback | n/a | pure logic (trivial) |
| Infomon line parser | `gemstone/infomon/parser.rb` | 839 | ~105 named regexes, 99 `when` branches | Parser hook (direct call, `games.rb:1401`) | pure logic, but the largest regex-translation job here |
| Infomon XML parser | `gemstone/infomon/xmlparser.rb` | 635 | NPC death detection, STOW/READY list capture, Group/Overwatch/Claim dispatch | Parser hook on the *raw* XML (`games.rb:1399`) | pure logic; 500 of its 635 lines are two giant `Regexp.union` literals |
| Infomon CLI | `gemstone/infomon/cli.rb` | 83 | `;infomon` user command, `redo!`, `db_refresh_needed?` | Command round-trip | needs game I/O |
| ActiveSpell | `gemstone/infomon/activespell.rb` | 191 | True-up of spell durations against the game's own effect dialogs | Queue-driven thread; reads `XMLData.active_spells` | pure logic |
| Status | `gemstone/infomon/status.rb` | 55 | `bound?`/`calmed?`/`sleeping?`/`muckled?` etc. | Reads Infomon bools ∧ `Effects` dialogs | pure logic (trivial) |
| PSMS façade | `gemstone/psms.rb` | 267 | Name normalization, cost/stamina assess, FORCERT rules, shared failure regexes | Reads Infomon ranks + `XMLData` stamina | pure logic |
| CMan | `gemstone/psms/cman.rb` | 849 | 80 combat maneuvers | Static table + Infomon rank + `dothistimeout` | data-only table + needs game I/O for `use` |
| Feat | `gemstone/psms/feat.rb` | 443 | 33 feats | same | same |
| Shield | `gemstone/psms/shield.rb` | 426 | 33 shield specializations | same | same |
| Weapon | `gemstone/psms/weapon.rb` | 405 | 24 weapon techniques | same | same |
| Armor | `gemstone/psms/armor.rb` | 287 | 11 armor specializations | same | same |
| Warcry | `gemstone/psms/warcry.rb` | 255 | 6 warcries | same | same |
| Ascension | `gemstone/psms/ascension.rb` | 159 | Ascension abilities | same | same |
| QStrike | `gemstone/psms/qstrike.rb` | 865 | QSTRIKE RT-reduction / stamina optimizer | Pure arithmetic over weapon stats + stance | pure logic (the only real *algorithm* in PSMs) |
| Spell | `common/spell.rb` | 954 | Spell model, active tracking, duration math, `cast` | Loads `effect-list.xml`; up/down messages matched by Infomon parser | **hard** — durations are `eval`'d Ruby (see §4.3) |
| SpellRanks | `gemstone/spellranks.rb` | 80 | Per-character circle ranks for *other* characters | `Marshal` file `DATA_DIR/<game>/spell-ranks.dat` | pure logic; the Marshal format must be abandoned |
| Spellsong | `attributes/spellsong.rb` | 190 | Bard song timing | Derived from Spell/Skills | pure logic |
| magic-info | `magic-info.rb` | 313 | `;magic` user command, display toggles | Command round-trip + `Lich.magicinfo_*` settings | needs game I/O (presentation only) |
| CritRanks | `gemstone/critranks.rb` | 133 | Index + lookup over 2,395 crit records | Static data + line matching | pure logic |
| Crit tables | `gemstone/critranks/*_critical_table.rb` (20 files) | 46,106 | The crit data itself | Static data | **data-only** |
| Combat defs | `gemstone/combat/defs/*.rb` (13 files) | 4,506 | Attack/flare/status/outcome/spell pattern catalogues | Static regex data | data-only *if* the regex dialect survives (see §5) |
| Combat processor | `gemstone/combat/processor.rb` | 2,436 | The combat state machine | Fed lines by Tracker | pure logic — the single densest logic file |
| Combat recorder | `gemstone/combat/recorder.rb` | 1,061 | Per-encounter roll-up / attribution | Subscribes to Processor events | pure logic |
| Combat tracker | `gemstone/combat/tracker.rb` | 612 | Downstream hook, buffering, thread pool, settings | `DownstreamHook.add` | needs game I/O (hook plumbing) |
| Combat parser | `gemstone/combat/parser.rb` | 383 | `parse_attack`/`parse_flare`/`parse_status`/… over the defs | Pure function of a line | pure logic |
| Combat async | `gemstone/combat/async_processor.rb` | 115 | Worker-thread queue | n/a | pure logic |
| Combat messages | `gemstone/combat/messages.rb` | 180 | Output formatting | n/a | pure logic |
| Creature model | `gemstone/creature.rb` | 1,018 | `CreatureTemplate` + live `Creature` (HP, stun, amputation, UCS position) | Template files + combat events | pure logic |
| Creature templates | `gemstone/creatures/*.rb` (627 files) | 97,741 | Bestiary: level, AS/DS/CS/TD, areas, treasure, abilities | Static data | **data-only**, with a caveat (§6.2) |
| Armaments | `gemstone/armaments.rb` + `armaments/*.rb` (13) | 180 + 2,942 | AG/ASG naming, weapon DF/AvD tables, armor/shield stats | Static data + index lookups | data-only + pure logic |
| Societies | `gemstone/society.rb`, `societies/*.rb` (3) | 199 + 1,424 | Voln/CoL/Sunfist symbol tables, favor costs, use | Infomon rank + static table + `dothistimeout` | data-only table, but costs are lambdas (§7.1) |
| Bounty | `gemstone/bounty.rb`, `bounty/parser.rb`, `bounty/task.rb` | 41 + 165 + 157 | Bounty task type, requirements, town, progress | Parses `CHECKBOUNTY` round-trip | pure logic + needs game I/O |
| Bank | `gemstone/bank.rb` | 389 | Deposit/withdraw/notes, f2p cap, Pinefar wording | Command round-trip | needs game I/O |
| Currency | `gemstone/currency.rb` | 110 | silver / notes / scrip / marks / dust / tickets | Reads Infomon; `refresh` sends `WEALTH` | pure logic + optional I/O |
| Experience | `gemstone/experience.rb` | 109 | fame, fxp, axp, LTE, deeds, next-ATP math | `XMLData` + Infomon | pure logic (trivial) |
| Wounds | `gemstone/wounds.rb` | 105 | Per-body-part wound severity | `XMLData.injuries` | pure logic (trivial) |
| Scars | `gemstone/scars.rb` | 110 | Per-body-part scar severity | `XMLData.injuries` | pure logic (trivial) |
| Injured | `gemstone/injured.rb` | 195 | Capability predicates over wounds+scars, with cache | Derived from Wounds/Scars | pure logic |
| Effects | `gemstone/effects.rb` | 89 | `Spells`/`Buffs`/`Debuffs`/`Cooldowns` registries | Thin view over `XMLData.dialogs` | pure logic (trivial) |
| Stance | `gemstone/stance.rb` | 172 | Stance bands, send + confirm | Command round-trip | needs game I/O |
| Mana | `gemstone/mana.rb` | 43 | `MANA PULSE` send + confirm | Command round-trip | needs game I/O |
| ReadyList | `gemstone/readylist.rb` | 96 | 9 READY slots + 6 STORE dispositions | Populated by `Infomon::XMLParser` | pure logic |
| StowList | `gemstone/stowlist.rb` | 78 | 14 STOW target containers | Populated by `Infomon::XMLParser` | pure logic |
| Disk | `gemstone/disk.rb` | 60 | Identify/own/locate a wizard disk | Pattern over `GameObj.loot` | pure logic (trivial) |
| Group | `gemstone/group.rb` | 665 | Membership, leader, open/closed, per-spell group cooldowns | `Group::Observer` from XMLParser + command round-trips | pure logic + needs game I/O |
| Claim | `gemstone/claim.rb` | 110 | Room ownership arbitration between characters | Mutex + `XMLData.room_id` + XMLParser arrival hook | pure logic |
| Overwatch | `gemstone/overwatch.rb` | 256 | Hidden-creature tracking per room | `Overwatch::Observer` from XMLParser | pure logic |
| SK | `gemstone/sk.rb` | 79 | User-declared "I have this as an SK item" spell list | `DB_Store` per character | pure logic (trivial) |
| Fog | `gemstone/fog.rb` | 269 | The five ways out of the field, ranked and confirmed | Command round-trip + room-change confirm | needs game I/O |
| Gift | `gemstone/gift.rb` | 54 | Gift of Lumnis pulse counter | Counter driven by pulses | pure logic (trivial) |
| GameObj | `common/gameobj.rb` | 1,945 | The live object registry (npcs/pcs/loot/inv/hands/contents) | XML parser writes it | pure logic; GS+DR shared |
| Inventory | `common/inventory.rb` | 1,754 | Full nested inventory snapshot from the `inventoryManager` stream | Saga extended feed | pure logic; GS+DR shared |
| Attribute façades | `attributes/{char,stats,skills,spells,resources,enhancive}.rb` | 1,035 | The names scripts actually call | Read-through to Infomon / `XMLData` | pure logic (trivial) |

---

## 2. Infomon — the character-state database

### 2.1 Storage layer

`lib/gemstone/infomon.rb` is a Sequel/SQLite key-value store, one table per
character:

| Property | Value | Line |
|---|---|---|
| Backing file | `File.join(DATA_DIR, "infomon.db")` (`Dir.tmpdir` under CI) | `infomon.rb:27-28` |
| Driver | `Sequel.sqlite` | `infomon.rb:29` |
| Table name | `"%s_%s" % [XMLData.game, XMLData.name]` as a symbol — e.g. `GSIV_Someone` | `infomon.rb:84-87` |
| Schema | `text :key, primary_key: true` / `any :value` / `float :updated_at` | `infomon.rb:114-118` |
| Guard | `context!` raises `"cannot access Infomon before XMLData.name is loaded"` | `infomon.rb:78-82` |
| Migration | If the table exists without `updated_at`, the whole table is dropped and rebuilt | `infomon.rb:105-112` |

Value typing is deliberately narrow:

```ruby
AllowedTypes = [Integer, String, NilClass, FalseClass, TrueClass]   # infomon.rb:141
```

with booleans round-tripped as the strings `"true"`/`"false"` via `_value`
(`infomon.rb:135-139`). Key normalization is `key.to_s.downcase.tr(' -', '_').gsub(/_+/, '_')`
(`infomon.rb:131-133`), so `"Combat Maneuvers"` and `"combat-maneuvers"` are the
same key.

**Concurrency design** — this is the part a Rust port would change:

- A module-level `Queue` (`infomon.rb:33`) receives *SQL strings*, not structs.
- A bare `Thread.new` loop started at require-time (`infomon.rb:260-283`) pops and
  runs them under a `Mutex`.
- `flush(timeout_seconds: 5)` pushes a `Queue` object *as a barrier token*; the
  worker recognizes `item.is_a?(Queue)` and signals it (`infomon.rb:228-243`, `:265-268`).
- `get` writes through the cache and, on a miss, calls `self.flush` first so a
  just-queued write is visible (`infomon.rb:147-173`).

A Rust port would replace this with a channel of typed commands plus a
oneshot-based barrier; the semantics (write-behind, read-your-writes via flush)
must be preserved because scripts depend on `Infomon.set` followed immediately by
`Infomon.get` returning the new value.

`upsert_batch` (`infomon.rb:245-258`) exists purely to keep the `INFO`/`SKILL`/`EXP`
dumps from generating dozens of individual statements; it accepts only Integer and
String and raises otherwise (`infomon.rb:250`).

### 2.2 What state it tracks

Keys are namespaced by dotted prefix. From `Infomon.set(...)` and `upsert_batch`
call sites in `infomon/parser.rb`:

| Namespace | Examples | Source command |
|---|---|---|
| `stat.*` | `stat.race`, `stat.profession`, `stat.gender`, `stat.age`, `stat.strength`, `stat.strength_bonus`, `stat.strength.base`, `stat.strength.enhanced` | `INFO` / `INFO FULL` |
| `skill.*` | `skill.<name>`, `skill.<name>_bonus` — 45 skills listed at `attributes/skills.rb:38` | `SKILL` |
| `spell.*` | `spell.minorspiritual` … one per circle | `SPELL` |
| `experience.*` | `fame`, `long_term_experience`, `deeds`, `deaths_sting`, `ascension_experience`, `total_experience` | `EXP` |
| `society.*` | `society.status`, `society.rank` | `SOCIETY`, plus join/step/resign messages |
| `citizenship` | town name or `'None'` | `CITIZENSHIP` |
| `cman.* shield.* weapon.* armor.* feat.* ascension.*` | `<category>.<short_name>` → rank | `CMAN`/`SHIELD`/… list output and learn/unlearn lines |
| `warcry.*` | 6 keys, e.g. `warcry.bertrandts_bellow` | `WARCRY` |
| `resources.*` | `weekly`, `total`, `type`, `suffused`, `voln_favor`, `covert_arts_charges`, `shadow_essence` | `RESOURCE`, plus per-message deltas |
| `currency.*` | `silver`, `silver_container`, `silver_total`, `notes`, `tickets`, `blackscrip`, `bloodscrip`, `ethereal_scrip`, `soul_shards`, `raikhen`, `aevit`, `gold`, `redsteel_marks`, `gemstone_dust`, `gigas_artifact_fragments` | `WEALTH`, `TICKET` |
| `status.*` | `sleeping`, `bound`, `silenced`, `calmed`, `cutthroat`, `thorned` | Free-text event messages |
| `enhancive.*` | `active`, `pauses`, `stats.item_count`, `stats.property_count`, `stats.total_amount`, plus per-stat/skill value+cap | `INVENTORY ENHANCIVE TOTALS` |
| `che` | Cooperative House affiliation | `PROFILE` |
| `account.*` | `name`, `subscription` | `ACCOUNT` |
| `citizenship`, `infomon.show_durations` | misc | |

Counted by namespace across single-key `Infomon.set('…')` calls in `parser.rb`:
currency 17, society 15, status 12, resources 11, enhancive 3, che 3,
citizenship 2, warcry 1, spell 1, account 1 (plus the batched `stat.*`,
`skill.*`, `experience.*` and PSM writes, which go through `upsert_batch`).

`resources.shadow_essence` is notable as the one key maintained by *arithmetic on
event messages* rather than by reading a report: casts decrement it and kills
increment it, each clamped to `0..5` (`parser.rb:474-500`).

### 2.3 Parser hooks

Two parsers, both called directly from the game thread in `lib/games.rb:1395-1402`.

**`Infomon::Parser`** (`infomon/parser.rb`, 839 lines) sees one stripped line at a
time. Structure:

- `module Pattern` (`:8-163`) — ~105 frozen named-capture regexes.
- A precomputed anchored union gate:
  `AllStart = Regexp.new('\A(?:' + Regexp.union(ALL_LIST).source + ')')` (`:162`),
  with a separate `AllMid` for patterns that can appear mid-line. `parse` returns
  `:noop` immediately unless one matches (`parser.rb:232`). The comment at
  `:158-160` says this scan "was ~half of all server-thread CPU" before the gate.
- `module State` (`:165-203`) — a small state machine with states
  `:ready`, `:goals`, `:profile`, and six `:enhancive_*` states. Illegal
  transitions `fail`, i.e. abort the calling script (`:181-183`).
- `self.parse(line)` (`:222`) — a 99-branch `case line … when Pattern::X`.

Note the coupling to `Spell`: three of the patterns are built by joining every
spell's up/down message at load time —

```ruby
SpellUpMsgs  = /^#{Lich::Common::Spell.upmsgs.join('$|^')}$/o          # parser.rb:~104
SpellDnMsgs  = /^#{Lich::Common::Spell.dnmsgs.join('$|^')}$/o
SpellTargetUpMsgs = (Spell.target_upmsgs.empty? ? /(?!)/ : …)
```

so `effect-list.xml` must be loaded *before* the parser module is evaluated. The
`(?!)` fallback exists because an empty `Regexp.union` matches everything.

**`Infomon::XMLParser`** (`infomon/xmlparser.rb`, 635 lines) sees the raw string
with XML intact. Only 18 patterns, but lines 11-511 are two enormous
`Regexp.union` literals — `NpcDeathPrefix` (`:11`) and `NpcDeathPostfix` (`:84`) —
composed into `NpcDeathMessage` (`:512`), which sets `npc.status = 'dead'` on the
matching `GameObj`. The rest dispatches to other modules rather than writing
Infomon keys: `Group::Observer` (`:571`), `Overwatch::Observer` (`:576`),
`Claim`'s arrival scan (`:580`), `StowList` (`:588-595`), `ReadyList` (`:597-618`),
and `Infomon::Parser::State` reset on every prompt (`:619-620`).

It also carries a multi-line fallback the line parser does not need: if the string
contains an interior newline it uses the full (unanchored) union so inner `^`
anchors still match (`xmlparser.rb:553-559`).

### 2.4 The read API scripts use

Scripts almost never call `Infomon.get` directly — only **1 of 251** corpus scripts
mentions `Infomon.`. They use the façades in `lib/attributes/`:

| Façade | File | LOC | Backed by |
|---|---|---:|---|
| `Char` | `attributes/char.rb` | 151 | Almost entirely `XMLData` passthrough (`char.rb:11-50`) |
| `Stats` | `attributes/stats.rb` | 93 | `Infomon.get("stat.…")`; 10 stats × {value, bonus, base, enhanced} via `define_singleton_method` (`stats.rb:30-50`) plus 10 shorthand aliases (`:52+`) |
| `Skills` | `attributes/skills.rb` | 82 | `Infomon.get("skill.…")`; 45 skills (`skills.rb:38`); also the pure `to_bonus` rank→bonus step function (`skills.rb:9-36`) |
| `Spells` | `attributes/spells.rb` | 76 | Per-circle ranks from Infomon |
| `Resources` | `attributes/resources.rb` | 36 | `resources.*` |
| `Enhancive` | `attributes/enhancive.rb` | 407 | `enhancive.*` |

Corpus usage (files out of 251 containing the token): `Char.` 104, `Stats.` 35,
`Skills.` 29, `Spells.` 18, `Infomon.` 1. **The façade, not the store, is the API
Cena must preserve.** `Skills.to_bonus` is worth calling out as a piece of pure
arithmetic worth porting verbatim: 5/rank to 10, 4/rank to 20, 3 to 30, 2 to 40,
1 thereafter (`skills.rb:12-27`).

### 2.5 Lifecycle

`Infomon.watch!` (`infomon.rb:287-305`) spawns a thread that spins on
`sleep 0.1 until GameBase::Game.autostarted? && XMLData.name && !XMLData.dialogs.empty?`,
then — GemStone only, `XMLData.game !~ /^DR/` — runs `Infomon.redo!` in a
subprocess script if `db_refresh_needed?`, and finally calls `PostLoad.game_loaded!`.
That polling loop is a readiness barrier a Rust port should express as an
awaitable signal instead. (`redo!` and `db_refresh_needed?` live in
`infomon/cli.rb:46` and `:73`, not in `infomon.rb`.)

---

## 3. PSMs — Combat Maneuvers / Shield / Weapon / Armor / Feat / Warcry / Ascension

### 3.1 Shape of the data

Each category is one file holding one frozen class-variable Hash keyed by a
normalized **long name**, with this record shape (`psms/cman.rb:22-36`):

```ruby
@@combat_mans = {
  "bearhug" => {
    :short_name => "bearhug",
    :type       => :concentration,
    :cost       => { stamina: 10 },
    :regex      => Regexp.union(/You charge towards .+ and attempt to grasp .+ in a ferocious bearhug!/,
                                /.+ manages to fend off your grasp!/),
    :usage      => "bearhug"
  },
```

Optional keys seen in the corpus of definitions: `:buff` (an `Effects::Buffs` name
to test for already-active, `cman.rb:733-734`), `"ignorable_cooldown"` (a
string key, not a symbol — `cman.rb:64`), and `:alt_cost_modifier` in the society
tables. `:usage => nil` marks a passive that cannot be commanded (`cman.rb:27`).

| Category | File | LOC | Entries |
|---|---|---:|---:|
| CMan | `psms/cman.rb` | 849 | 80 |
| Feat | `psms/feat.rb` | 443 | 33 |
| Shield | `psms/shield.rb` | 426 | 33 |
| Weapon | `psms/weapon.rb` | 405 | 24 |
| Armor | `psms/armor.rb` | 287 | 11 |
| Warcry | `psms/warcry.rb` | 255 | 6 |
| Ascension | `psms/ascension.rb` | 159 | 0 literal entries — built differently |
| QStrike | `psms/qstrike.rb` | 865 | n/a — calculator, not a table |

(Entry counts by `grep -cE '^\s+"…"\s+=> \{'`; total **187** named techniques.)

### 3.2 Keying and lookup

There are **two** keys per technique and both are accepted:

```ruby
def self.find_name(name, type)                                    # psms.rb:69-73
  name = self.name_normal(name)
  Object.const_get("Lich::Gemstone::#{type}").method("#{type.downcase}_lookups").call
        .find { |h| h[:long_name].eql?(name) || h[:short_name].eql?(name) }
end
```

`<type>_lookups` (e.g. `CMan.cman_lookups`, `psms/cman.rb:680-689`) projects the
table down to `{long_name, short_name, cost}`. Normalization is
`Lich::Util.normalize_name` (`psms.rb:53-55`).

The rank lookup goes to Infomon under the **short** name:

```ruby
Infomon.get("#{type.downcase}.#{seek_psm[:short_name]}")          # psms.rb:123
```

so the Infomon key space and the table's short names are one namespace — the
parser writes `cman.bullrush` at `parser.rb:439` and `PSMS.assess` reads it back.
`CMan[name]` is `PSMS.assess(name, 'CMan')` (`cman.rb:697-699`).

Every category also gets **two dynamically defined singleton methods per entry**,
one for each name (`cman.rb:838-846`), so `CMan.bullrush` and `CMan.bull_rush`
both work. 187 techniques × 2 = ~374 generated accessors across the categories.
A Rust port cannot generate methods; the natural translation is a single
`cman(name) -> u32` plus, for Lua, an `__index` metamethod over the table.

### 3.3 The shared logic in `psms.rb`

| Method | Line | What it is |
|---|---:|---|
| `name_normal` | 53 | delegate to `Lich::Util.normalize_name` |
| `find_name` | 69 | dual-key table lookup |
| `assess(name, type, costcheck, forcert_count:)` | 101 | either affordability or rank; **raises `ArgumentError` to kill the calling script** on an unknown name (`:106-109`) |
| `available?` | 142 | false if `Debuffs` has `Overexerted`, or the name is in `Cooldowns` |
| `can_forcert?` / `max_forcert_count` | 159 / 178 | MOC-rank step function: 0-9→0, 10-34→1, 35-74→2, 75-124→3, 125+→4 |
| `FAILURES_REGEXES` | 208 | 11 shared "the command did not take" lines |
| `command(verb, usage, target, forcert_count:)` | 236 | builds e.g. `"cman bullrush #12345 forcert"`; a `GameObj` or Integer target becomes `#id` |
| `results_regex(name, *patterns, results_of_interest:)` | 259 | union of failures + `"X what?"` + `"X is still in cooldown."` + the technique's own regex |

The FORCERT cost formula is the one non-obvious piece of arithmetic:

```ruby
(cost_amount + (cost_amount * ((25 + (10.0 * forcert_count)) / 100))).truncate < XMLData.public_send(cost_type)
```
(`psms.rb:116`) — i.e. +25% for the first FORCERT and +10% per additional one,
truncated, compared strictly against the live `XMLData.stamina`.

### 3.4 `use` is game I/O

`CMan.use` (`cman.rb:771-795`) is the template every category copies:
`available?` → build command → build results regex → `waitrt?; waitcastrt?`
(skipped under FORCERT) → `dothistimeout usage_cmd, 5, results_regex`, with a
special retry that waits up to 10s for `"You regain control of your senses!"`
after `"You don't seem to be able to move to do that."`.

`PSMS.command` and `PSMS.results_regex` were deliberately extracted so a script can
send and confirm itself (`psms.rb:222-234` comment). **That split is exactly the
seam Cena needs**: the table plus `command`/`results_regex` are portable data +
pure logic; only the `dothistimeout` call needs the engine's round-trip primitive.

### 3.5 QStrike is different

`psms/qstrike.rb` (865 lines) is not a table — it is an optimizer that computes
how many seconds of roundtime QSTRIKE can buy without going stamina-negative.
Inputs it reads: weapon category speed multipliers (`SPEED_MULTIPLIERS`
`{two_handed: 1.5, polearm: 1.5, ranged: 2.5}`, default 1.0, `:29-35`), Striking
Asp stance rank discounts (`{1 => 2/3, 2 => 1/2, 3 => 1/3}`, `:39-43`),
`BASE_COST = 10` (`:46`), `MAX_REDUCTION = 8` (`:49`), and MSTRIKE's own costs
(`{open: {base: 20, per_speed: 3}, focused: {base: 30, per_speed: 4}}`, `:55-58`).
It has its own settings persistence (`load_settings`/`save_settings`, `:121`/`:137`)
and a hand-rolled weapon-stats cache (`hand_cache_key`/`valid_cache?`/`cache_cost`,
`:597`-`:620`). Pure arithmetic; ports cleanly and benefits most from Rust.

---

## 4. Spells, spell ranks, magic-info

### 4.1 Data model and where the data lives

`Lich::Common::Spell` (`common/spell.rb`, 954 lines) is **not** fed by a Ruby table.
It parses `DATA_DIR/effect-list.xml` with Ox (`spell.rb:183-238`), and if the file
is absent it downloads it from
`https://raw.githubusercontent.com/elanthia-online/scripts/master/scripts/effect-list.xml`
(`spell.rb:187-189`), falling back to `Script.run('repository', 'download effect-list.xml --game=gs')`.

The copy checked in at `spec/fixtures/effect-list.xml` is **2,750 lines / 233,408
bytes / 517 `<spell>` elements**. Representative element:

```xml
<spell availability='all' name='Spirit Warding I' number='101' type='defense'>
   <duration cast-type='self' span='stackable' multicastable='yes'>(Spell[101].known? ? 120 : 20) + Spells.minorspiritual</duration>
   <duration cast-type='target' span='stackable' multicastable='yes'>20 + Spells.minorspiritual</duration>
   <cost type='mana'>1</cost>
   <bonus type='bolt-ds'>10</bonus>
   <message type='start'>A light blue glow surrounds you\.</message>
   <message type='end'>The light blue glow leaves you\.</message>
</spell>
```

`Spell#initialize` (`spell.rb:64-161`) maps this to: `@num`, `@name`, `@type`,
`@no_incant`, `@availability` (`all`/`group`/`self-cast`), `@bonus` hash,
`@msgup`/`@msgdn`/`@target_msgup` (message texts **joined with `'$|^'` into a
regex fragment**, `:84-89`, `:148-151`), `@stance`, `@channel`, `@cost`
(by type, then `'self'`/`'target'`), `@duration` (by cast-type, carrying
`:duration`, `:stackable`, `:refreshable`, `:multicastable`, `:max_duration`
default `250.0`), `@cast_proc`, `@persist_on_death`, `@group_cooldown` /
`@target_cooldown`, and `@circle` derived from the number's leading digit(s)
(`:159`).

`Ox.load` is called with `skip: :skip_none` specifically because the message
bodies are regex source and whitespace collapsing would break the double-space
after a period (`spell.rb:212-216`).

### 4.2 Active-spell tracking

Three layers, and they are not the same thing:

1. **`Effects::{Spells,Buffs,Debuffs,Cooldowns}`** (`gemstone/effects.rb:40-43`) —
   thin `Registry` views over `XMLData.dialogs.fetch(dialog, {})` (`effects.rb:11-13`).
   These hold the *game's own* expiry timestamps. `active?` is
   `expiration(effect).to_f > Time.now.to_f` (`:27-29`). 89 lines total, trivial to port.
2. **`Spell#active?`** — `(self.timeleft > 0) and @active` (`spell.rb:380-382`),
   where `timeleft` decays locally: `@timeleft - ((Time.now - @timestamp) / 60.0)`,
   calling `putdown` and returning 0 when it crosses zero (`spell.rb:354-366`).
3. **`ActiveSpell`** (`infomon/activespell.rb`, 191 lines) reconciles 1 against 2.
   `update_spell_durations` (`:121-163`) drops spells the dialog no longer lists
   (minus an ignore list `["Berserk", "Council Task", "Council Punishment", "Briar
   Betrayer", "Rapid Fire Penalty"]` at `:130`, and a reject of
   `/^Aspect of the \w+ Cooldown|^[\w\s]+ Recovery/` at `:134`), then sets
   `spell.timeleft` from the dialog, clamping anything over 300 minutes to
   `600.01` (`:154-158`).

`get_spell_info` (`activespell.rb:21-89`) is a hardcoded alias table mapping the
game's dialog labels onto spell names: `Mage Armor|520 - …` → `Mage Armor`,
`CoS|712 - …` → `Cloak of Shadows`, `Enh. Strength` → `Surge of Strength`,
`Enh. Dexterity|Agility` → `Burst of Swiftness`, `Empowered` → `Shout`,
`Multi-Strike` → `MStrike Cooldown`, `Next Bounty Cooldown` → `Next Bounty`,
`Resist Nature|620 …` → `Resist Nature`. Nine special cases, all of which must be
carried over verbatim.

`ActiveSpell.watch!` (`:189`) is a `Queue`-driven worker thread; `request_update`
(`:165`) enqueues, `block_until_update_requested` (`:174`) pops and *clears* the
queue so bursts coalesce.

### 4.3 Duration math — the hardest single porting problem in this document

The duration in `effect-list.xml` is **a Ruby expression string, evaluated**:

```ruby
def time_per(options = {})
  formula = self.time_per_formula(options)
  ...
  result = proc { eval(formula) }.call.to_f          # common/spell.rb:338-346
```

`time_per_formula` (`spell.rb:286-337`) does *textual substitution* on that string
before evaluating it — swapping `Spells.minorelemental` etc. for
`SpellRanks['Caster'].minorelemental.to_i` when the caster is someone else, and
applying activator modifiers `{'tap'=>0.5,'rub'=>1,'wave'=>1,'raise'=>1.33,'drink'=>0,'bite'=>0,'eat'=>0,'gobble'=>0}`
(`:287`) for item activations, with `/2.0` applied when the formula references a
spell circle rather than a skill.

Some formulas in the shipped data are not arithmetic at all. Spirit Barrier (102)'s
target duration is an entire program that re-reads the scrollback:

```
if (history = reget) and (history = history.reverse) and (line = history.find { |l| l =~ /^The air thickens…/ }) and history[history.index(line)+2] =~ /^\s*CS: …/; ($1.to_i - 100)/60.0; else; 0.25; end
```
(`spec/fixtures/effect-list.xml:16`)

**Implication for Cena.** There is no way to "port" this to Rust directly. Either
(a) the duration expressions are rewritten as data (a small arithmetic DSL, which
covers the overwhelming majority but not the `reget`-scraping outliers), or (b)
the expressions are evaluated in the embedded Lua VM, which is the natural home
for them given the Lua decision — the substitution targets (`Spells.*`, `Skills.*`,
`SpellRanks[…]`, `Spell[…].known?`, `reget`) are all things Cena will expose to
Lua anyway. Option (b) preserves the existing `effect-list.xml` as an upstream
data feed; option (a) forks it. This is a decision the porting plan must make
explicitly.

`Spell#known?` (`spell.rb:464-522`) is separate and is pure logic: derive the
circle from the spell number, take `[circle ranks, XMLData.level].min`, and answer
`(@num % 100) <= ranks`. Circles 97/98/99 (Sunfist/Voln/CoL) use `Society.rank`
instead; 1700 is true only for Wizard/Cleric/Empath/Sorcerer/Savant; `SK.known?`
short-circuits to true (`:465`).

### 4.4 SpellRanks

`gemstone/spellranks.rb` (80 lines) is a per-*character-name* record of circle
ranks — used to compute durations of spells cast by **other** players. It
persists with `Marshal.dump`/`Marshal.load` to
`DATA_DIR/<game>/spell-ranks.dat` (`spellranks.rb:14-19`, `:36-42`), with
back-compat patches for circles added in 2012 and 2013 (`:20-22`).
Marshal is Ruby-specific; Cena must re-serialize (and needs a migration path or
must accept dropping other-caster rank history).

### 4.5 magic-info

`lib/magic-info.rb` (313 lines) is `Lich::Util::Magicinfo`, the `;magic` user
command: show active spells, `;magic set <spell#> <mins>`, `;magic clear`, and
four display toggles persisted through `Lich.magicinfo_circle` / `_bonus` /
`_gift` / `_messages` (`magic-info.rb:6-9`, dispatch table at `:11-42`). It owns
no state of its own; it is presentation over `Spell.active`. In Cena this is a UI
concern, not a library — VellumFE would own the rendering and it should not be
ported as a Rust module.

---

## 5. Combat — crit tables, definitions, and the tracker

### 5.1 CritRanks: 2,395 records across 20 tables

`gemstone/critranks/` holds 21 files (20 data tables + `generic_critical_table.rb`,
57 lines, which is the documented template). Every table is a Ruby file that
mutates a shared hash: `CritRanks.table[:slash] = { :head => { 0 => {...}, 1 => {...} } }`
(`critranks/slash_critical_table.rb:13-15`). The index is therefore
**type → location → rank → record**.

Each record has a fixed 18-field shape (`slash_critical_table.rb:16-32`):

`:type, :location, :rank, :damage, :position, :fatal, :stunned, :amputated,
:crippled, :sleeping, :dazed, :limb_favored, :roundtime, :silenced, :slowed,
:wound_rank, :secondary_wound, :regex`

| Table | LOC | `:regex` records |
|---|---:|---:|
| `ucs_kick_critical_table.rb` | 2,933 | 153 |
| `ucs_punch_critical_table.rb` | 2,843 | 148 |
| `ucs_jab_critical_table.rb` | 2,805 | 146 |
| `ucs_grapple_critical_table.rb` | 2,767 | 144 |
| `lightning_critical_table.rb` | 2,673 | 139 |
| `slash_critical_table.rb` | 2,501 | 130 |
| `fire_critical_table.rb` | 2,500 | 130 |
| `impact_critical_table.rb` | 2,463 | 128 |
| `disintegrate_critical_table.rb` | 2,387 | 124 |
| `cold_critical_table.rb` | 2,387 | 124 |
| `steam_critical_table.rb` | 2,330 | 121 |
| `non_corporeal_critical_table.rb` | 2,310 | 120 |
| `crush_critical_table.rb` | 2,235 | 116 |
| `puncture_critical_table.rb` | 1,950 | 101 |
| `disruption_critical_table.rb` | 1,931 | 100 |
| `acid_critical_table.rb` | 1,919 | 98 |
| `vacuum_critical_table.rb` | 1,874 | 97 |
| `plasma_critical_table.rb` | 1,798 | 93 |
| `unbalance_critical_table.rb` | 1,779 | 92 |
| `grapple_critical_table.rb` | 1,721 | 89 |
| **Total (20 tables)** | **46,106** | **2,393** |

(plus 2 in `generic_critical_table.rb`, giving the 2,395 the index builds.)

`critranks.rb` itself is 133 lines and does three things: `init` loads the files
with `load` rather than `require` so hot reload works (`critranks.rb:22-26`, with
the comment explaining why); `create_indices` (`:85-109`) buckets every pattern by
**the leading literal word of its anchored regex**, keeping non-anchored patterns
in an always-checked residual list — the comment at `:80-84` states this exists so
`parse` tests "the handful of patterns that could possibly match a given line
instead of all ~2400"; and `fetch(type, location, rank)` (`:120-128`) does the
validated 3-level `dig`.

**Port assessment: the cleanest win in this document.** 2.86 MB of Ruby that is
pure literal data, with a fixed field schema, no interpolation, and no code. It
converts to JSON/TOML/a binary table mechanically. Only the `:regex` field needs
care (see §5.3). The 133-line index-and-lookup module is trivial Rust.

### 5.2 Combat definitions — *not* pure data

`gemstone/combat/defs/` is 13 `.rb` files (4,506 LOC / 288,331 bytes) plus one
167-line example YAML. Def counts by `*Def.new` constructor calls:

| File | LOC | Defs |
|---|---:|---:|
| `flares.rb` | 449 | 100 |
| `attacks.rb` | 837 | 93 |
| `spells.rb` | 369 | 55 |
| `statuses.rb` | 518 | 30 |
| `outcomes.rb` | 482 | 22 |
| `spell_losses.rb` | 140 | 15 |
| `assaults.rb` | 120 | 5 |
| `sequences.rb` | 77 | 4 |
| `supplements.rb` | 822 | 4 |
| `messages.rb` | 240 | 1 |
| `damage.rb` | 121 | 0 (pattern list) |
| `ucs.rb` | 128 | 0 (pattern list) |
| `pattern_gate.rb` | 203 | 0 (machinery) |

Structs are declared inline, e.g. `AttackDef = Struct.new(:name, :patterns)`
(`defs/attacks.rb:11`).

Two things stop these being plain data:

1. **Markup-tolerance interpolation.** `pattern_gate.rb:31-32` defines
   ```ruby
   MK_PRE  = '(?:<pushBold/>)?(?:<a [^>]*>)?'
   MK_POST = '(?:</a>)?(?:<popBold/>)?'
   ```
   and the def patterns interpolate them, e.g.
   `/You swing your .+? at (?<target>.+?)'s#{MK_POST} .+? and connect!/`
   (`defs/attacks.rb:26`). The comment at `:22-30` says this covers "46 def kinds
   proven markup-unsafe against 11.5GB of real logs." So the defs are regex
   *templates*, not regex literals.
2. **`PatternGate`** (`defs/pattern_gate.rb`, 203 lines) derives a literal-substring
   pre-filter from each pattern set at load time. Its header documents the
   measurement: a naive `Regexp.union` costs "~0.5-1ms per non-matching line"
   versus "~7us per line" for the literal-fragment gate. Any Rust port that drops
   this and just tries every regex will be 100× slower on the hot path. Rust's
   `regex::RegexSet` or the `aho-corasick` crate is the natural equivalent and is
   strictly better than what Ruby can do here.

`defs/supplements.rb` (822 LOC) lets a user add their own defs from
`DATA_DIR/combat/defs.yaml`, compiled into the same Structs, re-read on mtime
change, validated through `Lich::Common::UserDefs` (shared with DragonRealms'
CustomSubstitutions). A shipped example is `defs/supplements.example.yaml`
(167 lines / 7,075 bytes). **This is a user-extension point Cena must keep** —
and in Cena it is a natural Lua-table or TOML feature rather than a YAML→Struct
compiler.

### 5.3 A regex-dialect warning that applies to §5.1 and §5.2 both

The crit tables and combat defs together carry roughly **2,400 + 330 regexes**,
written in Ruby's Onigmo dialect. Most are simple, but the corpus uses named
captures `(?<target>…)`, lazy quantifiers, `Regexp.union`, and — critically —
`^`/`$` as *line* anchors (Ruby's default), where Rust's `regex` crate treats them
as *text* anchors unless multi-line mode is set. Several parse paths in Lich
depend on the Ruby meaning: `xmlparser.rb:553-559` explicitly falls back to the
unanchored union when a string contains an interior newline "so inner-line `^`
anchors (e.g. death messages) still match." A mechanical conversion must set
`(?m)` or normalize to one-line-at-a-time feeding, and must audit for backreferences
and lookbehind (which `regex` does not support) — `UNVERIFIED:` I did not audit
all ~2,730 patterns for `regex`-crate incompatibility; doing so needs a script
that compiles each one against the Rust crate and reports failures.

### 5.4 The runtime path

| Component | File | LOC | Role |
|---|---|---:|---|
| `Tracker` | `combat/tracker.rb` | 612 | Owns the `DownstreamHook` (`:517-563`), the buffer, the thread pool, settings persistence via `DB_Store` |
| `AsyncProcessor` | `combat/async_processor.rb` | 115 | Worker queue |
| `Processor` | `combat/processor.rb` | 2,436 | The state machine: `SEEKING_ATTACK → SEEKING_DAMAGE → SEEKING_CRIT` (`:5`) |
| `Parser` | `combat/parser.rb` | 383 | `parse_attack` (`:44`), `parse_damage` (`:229`), `parse_flare` (`:237`), `parse_outcome` (`:243`), `parse_resolution` (`:249`), `parse_sequence_start`/`_end` (`:254`/`:258`), `parse_assault_start`/`_end` (`:264`/`:268`), `parse_swing_weapon` (`:281`), `parse_status` (`:286`), `parse_spell_loss` (`:298`) |
| `Recorder` | `combat/recorder.rb` | 1,061 | Subscribes to emitted events, rolls up per-encounter attribution |
| `Messages` | `combat/messages.rb` | 180 | Output formatting |

Default settings (`tracker.rb:57-69`): `enabled: false`, `track_damage/wounds/statuses/ucs: true`,
`emit_attacks: false`, `max_threads: 2`, `buffer_size: 200`, `fallback_max_hp: 350`.
`max_threads <= 0` means process inline on the hook thread (`tracker.rb:504`).

Events go out over `Lich::Common::Events.emit("combat.#{type}", payload)`
(`processor.rb:190`, `:199`), gated on `Tracker.settings[:emit_attacks] ||
Lich::Common::Events.any_for?('combat.attack')` (`:77`) so the expensive blob is
only built when someone is listening. **That pub/sub seam is exactly how Cena
should expose combat to Lua**: the Rust side owns the state machine, Lua
subscribes to `combat.*` events.

`processor.rb` is the single hardest file to port here — not because of size but
because of accumulated adjudication rules, many dated in comments
(e.g. `:24-35` on unowned DoT ticks, "owner ruling 2026-09-06"; `:36-45` on
spell-releasing flares and a specific log, "Dreadt log 2026-09-11 594-613";
`:46-49` on `ROLL_BEFORE_ROUND_ASSAULTS`). These are behavioural decisions
recorded nowhere else. A rewrite that does not carry them forward will produce
subtly wrong attribution. Constants that encode them include
`POSITION_STATUSES` (`:19`), `SPELL_RELEASING_FLARES` (`:45`),
`ROLL_BEFORE_ROUND_ASSAULTS` (`:49`), `UNOWNED_TICK_ATTACKS` (`:51`),
`SUMMARY_DAMAGE_ATTACKS` (`:55`).

---

## 6. Creatures

### 6.1 The model

`gemstone/creature.rb` (1,018 lines) holds two classes:

**`CreatureTemplate`** — ID-less static reference data. Attributes
(`creature.rb:14-18`): `name, url, picture, level, family, type, undead, boss,
boss_type, otherclass, areas, bcs, max_hp, speed, height, size,
attack_attributes, defense_attributes, treasure, messaging, special_other,
abilities, alchemy, equipment`, plus tri-state `blood/bones/limbs/witherable/
sympathy/muggable/sleepable` where **nil means uncatalogued, not false**
(`:37-42`, explicit comment). `BOON_ADJECTIVES` (`:20-26`) is a 66-word list of
prefix adjectives (`adroit`, `afflicted`, … `wispy`) compiled into `BOON_REGEX`
(`:127`) and stripped before lookup, so "a shimmering water wyrd" resolves to the
`water wyrd` template.

**`Creature`** — the live, ID-bearing instance. Its methods (`creature.rb:340-620`)
are the combat-state surface: `set_ucs_position`/`set_ucs_tierup`/`ucs_expired?`
(`:387`-`:446`), `smite!`/`smote?`/`clear_smote` (`:407`-`:433`),
`add_stun_estimate`/`stun_rounds`/`stunned_for` (`:468`-`:505`),
`amputate!`/`amputated?`/`amputated_parts` (`:517`-`:536`),
`add_injury`/`injured?`/`injured_locations` (`:538`-`:568`),
`mark_fatal_crit!`/`fatal_crit?` (`:554`/`:559`),
`add_damage`/`max_hp`/`current_hp`/`hp_percent`/`low_hp?` (`:569`-`:612`).
`max_hp` falls back to `Combat::Tracker.fallback_hp` when no template exists
(`:581-583`). It extends `lib/common/creature/creature_base.rb`.

### 6.2 The templates — 627 files, 97,741 lines, 2.41 MB

`gemstone/creatures/` contains 628 files: 627 creature templates plus
`_creature_template.rb` (170 lines / 7,101 bytes, the documented schema, skipped
at load — `creature.rb:85`).

Each template file is **a bare Ruby Hash literal, `eval`'d**:

```ruby
def self.load_template_data(file_content, path)
  data = binding.eval(file_content, path, 1)         # creature.rb:137-139
  raise "Template must return a Hash, got #{data.class}" unless data.is_a?(Hash)
  data
end
```

Sample (`creatures/water_wyrd.rb:1-40`): `schema_version: 3, name: "water wyrd",
noun: "wyrd", url:, level: 35, family: "Elemental", type: "Hybrid", undead: false,
blood: nil, bones: false, limbs: true, …, max_hp: 262, height: 5, size: "medium"`,
then `areas:` and `attack_attributes:` with per-attack AS values
(`{name: "Trident", as: 197}`), bolt spells, warding spells with CS, offensive
spells, maneuvers, special abilities.

**The porting caveat.** These files are *not* JSON-equivalent as written, because
they contain live Ruby objects:

- **Ranges**: `uids: [305023..305030, 305032..305038]` (`water_wyrd.rb:31`) and
  `as: (212..237)`, `cs: (171..199)` (`:48`, `:54`). A Range has to become
  `{min, max}` or `[lo, hi]` in any serialization format.
- The largest templates are substantial: `tattooed_gigas_berserker.rb` 327 lines,
  `brawny_gigas_shield-maiden.rb` 303, `behemothic_gorefrost_golem.rb` 299,
  `hooded_figure.rb` 295, `ithzir_seer.rb` 294.

Load is fault-tolerant per file: a `rescue StandardError, ScriptError` skips a
broken template rather than aborting the whole load (`creature.rb:113-118`, with
the comment noting `ScriptError` is not a `StandardError` so `SyntaxError` needed
catching explicitly). Collisions on the normalized lookup key are warned about
only under `$creature_debug` (`:108-110`).

**Port assessment: data-only with a one-time transform.** Write a Ruby script that
`eval`s each file and dumps JSON/TOML with ranges expanded to objects; after that
Cena never needs a Ruby evaluator. The 1,018-line model class is straightforward
Rust.

Checked, and the transform is safe: grepping all 627 files for `->`/`lambda`
returns 2 hits and both are the literal string `"-->"`
(`creatures/algae_draped_merrow_oracle.rb:69`,
`creatures/kelp_tangled_coral_golem.rb:65`), not code. No Regexp literals appear
in any template. **Ranges are the only non-JSON construct**, used in 591 of the
627 files. Schema is versioned in-band: 610 files declare `schema_version: 3`,
the other 17 declare none — a transform must treat "absent" as its own case
rather than assuming 3.

---

## 7. The remaining GemStone modules

### 7.1 Societies

`gemstone/society.rb` (199 LOC) is the read façade — `membership`/`status`
(`Infomon.get("society.status")`, `:19`), `rank` (`:39`), `task`. The three
society files under `gemstone/societies/` carry the ability tables:

| Society | File | LOC | Abilities |
|---|---|---:|---:|
| Order of Voln | `societies/order_of_voln.rb` | 540 | 24 symbols |
| Council of Light | `societies/council_of_light.rb` | 477 | 20 |
| Guardians of Sunfist | `societies/guardians_of_sunfist.rb` | 407 | 20 |

Record shape (`societies/order_of_voln.rb:22-31`): `rank, short_name, long_name,
type, cost_modifier, cost, duration, summary, spell_number` — plus
`alt_cost_modifier` / `alt_cost_reason` where a condition changes the price
(`:37-39`, the non-magical discount on Symbol of Blessing).

**Unlike creature templates, these tables are not plain data.** `cost`, `duration`
and `summary` are frequently **lambdas over live state**:

```ruby
cost:     ->(s) { OrderOfVoln.calculate_cost(s[:cost_modifier]) },
duration: -> { Society.rank * 2 },
summary:  -> { "Bless weapons … up to 2x rank (+#{(Society.rank * 2)}) for Level + 2x Rank (#{Stats.level + (Society.rank * 2)}) swings." },
```
(`order_of_voln.rb:36-43`). They are resolved at access time via `Society.resolve`
(documented at `:17`). So the society tables split into a static part (rank,
names, type, modifiers, spell numbers) and a computed part that in Cena is either
Rust closures or, more naturally, Lua functions in a data table.

Voln favor cost sourced from the wiki formula, referenced in the comment at
`order_of_voln.rb:11`.

### 7.2 Bounty

Three files, 363 LOC total: `bounty.rb` (41), `bounty/parser.rb` (165),
`bounty/task.rb` (157).

`Parser::TASK_MATCHERS` (`bounty/parser.rb:24+`) is a 23-entry hash of regexes
covering the full bounty lifecycle — assignment, in-progress, and completion — for
each task family: `none, bandit, bandit_assignment, creature, creature_assignment,
cull, dangerous, dangerous_spawned, escort, failed, gem, gem_assignment, guard,
heirloom, heirloom_assignment, heirloom_found, herb, herb_assignment, rescue,
rescue_assignment, rescue_spawned, skin, skin_assignment, taskmaster`.
`Bounty::KNOWN_TASKS = Parser::TASK_MATCHERS.keys` (`bounty.rb:7`).

Composable sub-patterns: `HMM_REGEX` (`:5`), `LOCATION_REGEX` (`:6`),
`GUARD_REGEX` (`:7-21` — a 13-alternative union naming each town's specific guard
NPC), `CONCOCTION_REGEX` (`:22`), `TASK_MAYBE_REGEX` (`:23`).

`Task` (`bounty/task.rb`) is a predicate façade: `bandit?`, `creature?`, `cull?`,
`dangerous?`, `escort?`, `gem?`, `heirloom?`, `herb?`, `rescue?`, `skin?`,
`search_heirloom?`, `loot_heirloom?`, plus `critter`/`location`/`count` accessors
(`:13-80`).

`Bounty.current` is `Task.new(Parser.parse(checkbounty))` (`bounty.rb:9-11`) —
a command round-trip. `Bounty.lnet(person)` (`:14-29`) fetches another
character's bounty over LNet. Class methods `status, type, requirements, town,
any?, none?, done?` are `define_method`'d to delegate to a fresh `current`
(`:31-38`) — meaning **each of those accessors issues a game command**. A naive
port that calls three of them in a row sends three CHECKBOUNTYs. Cena should
cache.

### 7.3 Armaments

`gemstone/armaments.rb` (180 LOC) is the façade over 13 data files (2,942 LOC /
150,608 bytes):

- `armor_stats.rb` (697), `shield_stats.rb` (279), `weapon_stats.rb` (416, the
  dispatcher), and 11 per-category weapon files: `blunt, brawling, edged, hybrid,
  natural, polearm, ranged, runestave, thrown, two_handed, unarmed`.

Static maps in the façade: `AG_INDEX_TO_NAME` (5 armor groups: Cloth, Soft
Leather, Rigid Leather, Chain, Plate — `armaments.rb:12-18`) and
`ASG_INDEX_TO_NAME` (20 subgroups, Robes through Augmented Plate — `:25-46`).

Weapon record shape (documented template at `armaments/weapon_stats_edged.rb:26-37`):
`:category, :base_name, :all_names, :damage_types {slash, crush, puncture, special},
:damage_factor` (a 6-element array indexed nil/Cloth/Leather/Scale/Chain/Plate),
`:avd_by_asg` (a 21-element array indexed by ASG 1-20), `:base_rt`, `:min_rt`.
Armor record shape (`armaments/armor_stats.rb:19-30`): `:type, :base_name,
:all_names, :armor_group, :armor_sub_group, :base_weight, :min_rt,
:action_penalty, :normal_cva, :magical_cva`, plus a 20-element spell-hindrance
array documented at `:12-18`.

API: `Armaments.find/valid_name?/names/categories/type_for/category_for`
(`armaments.rb:82-172`); `WeaponStats.find/list/categories/damage_summary/
aliases_for/compare/search/weapons_in_category/names/is_grippable?/category_for/
pretty/pretty_long/valid_name?` (`armaments/weapon_stats.rb:63-407`).

**Port assessment: data-only for the 13 stat files, pure logic for the lookups.**
Positional arrays keyed by AG/ASG index port directly to fixed-size Rust arrays.
This is the second-cleanest win after the crit tables.

### 7.4 Bank and currency

`gemstone/bank.rb` (389 LOC) is entirely command round-trip: `here?`, `account`
(→ `{balance:, max:}`), `deposit`, `deposit_note`, `withdraw(n, note:)`. Its
`Pattern` module (`:21-52+`) unions the teller's replies, including a separate
`WITHDRAW_RESULT` because the debt notice arrives *before* the real answer
(`:33-35` comment), `DEBT`, and `NO_ACCESS`. It carries the free-to-play balance
cap and the Pinefar depository's different wording (header comment `:5-10`).

`gemstone/currency.rb` (110 LOC) is the read side over Infomon's `currency.*`
keys, with optional `refresh:` that sends `WEALTH` / `WEALTH ALL` through
`Lich::Util.issue_command(…, silent: true, quiet: true)` (`:25`). The comment at
`:17-20` documents a subtlety Cena must reproduce: the response is parsed on the
game thread *before* hooks run, so suppressing it from the front end does not
suppress it from Infomon.

### 7.5 Experience, wounds, scars, injured

| Module | LOC | Notes |
|---|---:|---|
| `experience.rb` | 109 | Mostly `XMLData` passthrough (`fxp_current`, `exp`, `axp`, `until_next`); `fame` and `lte` from Infomon. The only real logic: `next_atp = 50_000 - (axp % 50_000)` (`:38`) and the three percent helpers (`:42-52`). |
| `wounds.rb` | 105 | 16 body parts with alias sets (`BODY_PARTS`, `:12-28`), methods generated by `define_method`; reads `XMLData.injuries[part]['wound']` |
| `scars.rb` | 110 | Identical structure, reads `['scar']` |
| `injured.rb` | 195 | Groups (`eyes, arms, hands, legs, feet, head_and_nerves`, `:7-14`) plus a mutex-guarded cache keyed on an `injury_fingerprint` — because `XMLData.injuries` is mutated in place and so cannot be its own cache key (`:30-34` comment) |

All four are trivial Rust. The body-part enum (16 variants) should be a single
shared type, since `Wounds`, `Scars`, `Injured` and the crit tables' `:location`
field all name the same anatomy.

### 7.6 Effects, stance, mana

- **`effects.rb`** (89 LOC) — covered in §4.2. `Registry.new("Active Spells")`,
  `"Buffs"`, `"Debuffs"`, `"Cooldowns"` (`:40-43`); `Effects.display` (`:45-86`)
  is presentation and belongs to the frontend in Cena.
- **`stance.rb`** (172 LOC) — `NAMES` (6 stances, `:25`), `BANDS` mapping each to
  a percent-of-defense range (`offensive 0..0` … `defensive 81..100`, `:30-38`),
  `ACCEPTED` / `DECLINED` reply patterns and their union `CONFIRM` (`:40-55`).
  `Stance.change` sends and confirms; `Stance.safest` returns `'guarded'` during
  cast RT else `'defensive'`. The header comment (`:5-9`) notes this was
  consolidated from four separate copies in `Spell#cast`, bigshot, ebounty,
  eloot and ecleanse — evidence that Cena should provide it as a primitive, not
  leave it to scripts.
- **`mana.rb`** (43 LOC) — `MANA PULSE` with a 4-alternative `PULSE_RESULT` union
  (`:15-20`). `Mana.pulse(spell)` short-circuits if the spell is unknown or
  already affordable (`:33-36`).

### 7.7 ReadyList and StowList

Both are pure registries **written by `Infomon::XMLParser`**, never by their own
parsing:

- `readylist.rb` (96 LOC): 9 slots — `shield, weapon, secondary_weapon,
  ranged_weapon, ammo_bundle, ammo2_bundle, sheath, secondary_sheath, wand`
  (`:6`) — plus 6 store dispositions (`:7`). Accessors generated per key
  (`:31-39`). Populated at `infomon/xmlparser.rb:597-618`.
- `stowlist.rb` (78 LOC): 14 containers — `box, gem, herb, skin, wand, scroll,
  potion, trinket, reagent, lockpick, treasure, forageable, collectible, default`
  (`:6`). Populated at `infomon/xmlparser.rb:588-595`. `valid?(all:)` re-checks
  each stored `GameObj` id is still in `GameObj.inv` (`:45-52`).

Both have a `checked?` flag set only after the game's list output is seen, so
"unknown" and "empty" are distinguishable — a distinction worth keeping.

### 7.8 Disk, Group, Claim, Overwatch

- **`disk.rb`** (60 LOC) — `NOUNS` is an 11-word list (`cassone chest coffer
  coffin coffret disk hamper saucer sphere trunk tureen`, `:4`); `is_disk?` is a
  capitalized-name-plus-noun regex (`:6-8`); `Disk.mine` is
  `find_by_name(Char.name)`. `method_missing` forwards to the underlying
  `GameObj` (`:47-49`). Trivial.
- **`group.rb`** (665 LOC) — members, leader, `:open`/`:closed` status, `checked?`.
  Fed by `Group::Observer` from `infomon/xmlparser.rb:571-574`. It also owns
  **per-spell group cooldowns**, keyed spell-number → member-name →
  expiry (`:24`, `:168`, `:180-235`), fed by the `<cooldown type='group'>` /
  `type='target'` elements `Spell#initialize` reads (`common/spell.rb:138-147`).
  That is the one place where the spell data model and group state are coupled.
- **`claim.rb`** (110 LOC) — arbitrates which of the user's own characters "owns"
  a room. Uses a module-level `Mutex` named `Lock` (`:5`) that is *locked by one
  method and unlocked by another* (`claim_room` unlocks at `:17`), which is a
  pattern Rust will not permit; the port needs an explicit state machine or a
  condition variable. Consumers: `Infomon::XMLParser`'s `Also_Here_Arrival`
  branch, gated on `Lich::Claim::Lock.locked?` (`xmlparser.rb:581`).
- **`overwatch.rb`** (256 LOC) — tracks which room had creatures hide in it
  (`@@hidden_targets` holds a room id; `hiders?` compares to `XMLData.room_id`,
  `:38-41`). Fed by `Overwatch::Observer` from `xmlparser.rb:576-579`.

### 7.9 SK, Fog, Gift

- **`sk.rb`** (79 LOC) — a user-maintained list of spell numbers the character has
  as SK (self-knowledge) items, persisted through
  `DB_Store.read/save("#{XMLData.game}:#{XMLData.name}", "sk_known", …)`
  (`:8-30`), with a migration from the old `vars["sk/known"]` location (`:11-17`).
  Its whole purpose is to make `Spell#known?` return true (`common/spell.rb:465`)
  and to floor durations at 10 minutes (`common/spell.rb:345`).
- **`fog.rb`** (269 LOC) — the five ways out of the field:
  `METHODS = %i[spirit_guide symbol_of_return travelers_song sigil_of_escape familiar_gate]`
  (`:35`), mapped to bigshot's legacy numbering 1-5 (`NUMBERS`, `:38`) and to
  spell numbers `{spirit_guide: 130, travelers_song: 1020, familiar_gate: 930}`
  (`:40`). Constants: `RIFT_ROOM = 2635` (`:22`), `CONFIRM_TIMEOUT = 8` /
  `TRAVELERS_SONG_TIMEOUT = 60` (`:27-28`), `MAX_ROOM_UID = 1_000_000_000` — above
  which the id is xmlparser's MD5 stand-in for a room with no UID and is not
  unique (`:33`, with the explanation at `:30-32`). Confirms on the room actually changing, not on a message.
  Needs game I/O.
- **`gift.rb`** (54 LOC) — Gift of Lumnis: a pulse counter, `remaining` is
  `([360 - @pulse_count, 0].max * 60).to_f` seconds (`:24-26`) and
  `restarts_on` is `@gift_start + 594000` (6.875 days, `:28-30`). Trivial.

### 7.10 GameObj and Inventory (shared with DragonRealms)

Both live in `lib/common/` and serve both games, so they are only partly this
document's business; a fuller treatment belongs with the Lich infrastructure
inventory.

- **`common/gameobj.rb`** (1,945 LOC) — the live object registry. Class-level
  arrays for `@@loot, @@npcs, @@pcs, @@inv, @@room_desc, @@fam_*` plus
  `@@right_hand`, `@@left_hand`, `@@contents` (`:25-39`), a mutex-guarded
  `@@index` (`:76`, `:85`), and a full **staging set** (`@@staging_*`, `:115-140`)
  so a room's objects swap atomically rather than flickering. Getters return
  copies (`registry_or_nil`, `.dup` — `:653-683`). It also loads
  `DATA_DIR/gameobj-data.xml` for item type/sellable classification, merging
  `DATA_DIR/gameobj-custom/gameobj-data.xml` over it (`:1424-1470`). **The most
  used API in the corpus: 119 of 251 scripts reference `GameObj`.**
- **`common/inventory.rb`** (1,754 LOC) — a read-only snapshot of the Saga
  extended feed's `inventoryManager` stream: the *entire* nested item tree with
  per-item `weight`, container `in_encum`, `closed`/`locked` state — none of which
  `GameObj` carries (`:10-19`). Paginated responses are reassembled by
  `Assembly` (`:761`). Classes: `Item` (`:131`), `Snapshot` (`:529`),
  `Handler` (`:653`), `Assembly` (`:761`). Committed snapshots mirror into
  `GameObj.contents` (`:33-41`). Explicit freshness contract: it does not track
  later get/put/loot changes; callers judge staleness from `last_updated`/`age`
  (`:26-31`).

---

## 8. What the community scripts actually use

Files out of 251 in `E:/Cena/reference/scripts` containing each token. This is the
compatibility surface that matters most, since these scripts (or Lua ports of
them) are what will run on Cena.

| Token | Files | Token | Files |
|---|---:|---|---:|
| `GameObj` | 119 | `Feat.` | 4 |
| `Char.` | 104 | `Experience.` | 4 |
| `Spell[` | 71 | `Bounty.` | 4 |
| `Stats.` | 35 | `CritRanks` | 3 |
| `Skills.` | 29 | `Claim.` | 3 |
| `Spell.` | 25 | `Shield.` | 2 |
| `Effects::` | 20 | `ReadyList` | 2 |
| `Spells.` | 18 | `Gift.` | 2 |
| `Society.` | 12 | `Disk.` | 2 |
| `Wounds.` | 11 | `Bank.` | 2 |
| `Scars.` | 10 | `Warcry.` | 1 |
| `Group.` | 9 | `SK.` | 1 |
| `CMan.` | 8 | `Infomon.` | 1 |
| `StowList` | 6 | `CreatureTemplate` | 1 |
| `Combat::` | 5 | `Armaments` | 1 |

Zero hits: `Stance.`, `PSMS.`, `Overwatch`, `Injured.`, `Fog.`, `Currency.`.
Those six are all recent consolidations (their header comments say so — e.g.
`stance.rb:5-9`, `fog.rb:6-8`) that postdate most of the corpus.

Two conclusions for the porting plan:

1. **Priority order is clear.** `GameObj`, `Char`, `Spell[...]`, `Stats`,
   `Skills`, `Effects`, `Spells` cover the overwhelming majority of script
   contact. A Cena that ships only those seven, correctly, already runs most of
   the corpus's data access.
2. **The state store is invisible.** One script in 251 touches `Infomon`
   directly. Cena is free to replace SQLite+Sequel with anything — the
   contract is the `Char`/`Stats`/`Skills`/`Spells` façades, not the store.

---

## 9. Data vs. code

### 9.1 Static data that can ship as-is (after a format transform)

| Asset | Files | Lines | Bytes | Transform needed |
|---|---:|---:|---:|---|
| Crit tables | 20 | 46,106 | 2,856,915 | Literal hashes → JSON/TOML. Regex strings need dialect review (§5.3). |
| Creature templates | 627 | 97,741 | 2,411,905 | `eval` → JSON/TOML; Ranges → `{min,max}` (591 files); handle 17 files with no `schema_version` |
| Armaments stats | 13 | 2,942 | 150,608 | Literal hashes + positional arrays → JSON/TOML |
| PSM tables | 7 | 2,824 | 144,933 | Literal hashes; `:regex` fields need dialect review; `qstrike.rb` is excluded (it is code) |
| Spell data | 1 | 2,750 | 233,408 | Already XML; **duration/bonus fields are Ruby expressions** (§4.3) — not portable as data without a decision |
| Society tables | 3 | 1,424 | 56,175 | Split: static fields port; `cost`/`duration`/`summary` lambdas do not |
| Combat defs | 13 | 4,506 | 288,331 | **Not pure data** — patterns interpolate `MK_PRE`/`MK_POST` and are assembled with derived `PatternGate`s (§5.2) |
| Supplements example | 1 | 167 | 7,075 | Already YAML |

**Clean data subtotal** (crit tables + creature templates + armaments, the three
with no embedded code): **660 files, 146,789 lines, 5,419,428 bytes (5.17 MB).**

**Data with embedded code** (spell XML + societies + combat defs + PSM tables):
**24 files, 11,504 lines, 722,847 bytes** — each needs a per-asset decision about
whether the embedded expressions become Rust closures, a small DSL, or Lua.

### 9.2 Logic that must be rewritten

| Group | Files | Lines | Bytes |
|---|---:|---:|---:|
| `lib/gemstone/*.rb` (27 top-level) | 27 | 5,332 | 203,595 |
| `lib/gemstone/infomon/` | 6 | 1,861 | 129,513 |
| `lib/gemstone/combat/*.rb` (non-defs) | 5 | 4,787 | 263,587 |
| `lib/gemstone/bounty/` | 2 | 322 | 14,614 |
| `lib/gemstone/creature.rb` (counted in the 27 above) | — | 1,018 | 36,790 |
| `lib/attributes/` | 7 | 1,035 | 36,210 |
| `lib/common/` GS-relevant (`spell.rb`, `gameobj.rb`, `inventory.rb`) + `magic-info.rb` | 4 | 4,966 | 217,961 |
| `lib/gemstone/psms/qstrike.rb` (counted in PSM tables above but is code) | 1 | 865 | 32,661 |

**Logic subtotal (de-duplicated; `qstrike.rb` is counted once, in the PSM row of
§9.1, and not again here): 51 files, 18,303 lines, 865,480 bytes.**

### 9.3 The ratio

| | Lines | Bytes | Share of bytes |
|---|---:|---:|---:|
| Static data (clean) | 146,789 | 5,419,428 | 77.3% |
| Data with embedded code | 11,504 | 722,847 | 10.3% |
| Logic to rewrite | 18,303 | 865,480 | 12.4% |
| **Total** | **176,596** | **7,007,755** | |

**The headline for the porting plan: ~87% of the bytes under `lib/gemstone/` are
data, and 77.3% is data with nothing embedded in it.** The actual Rust rewrite is
18,303 lines, and half of that is concentrated in four files —
`combat/processor.rb` (2,436), `combat/recorder.rb` (1,061), `creature.rb`
(1,018) and `common/spell.rb` (954) — plus `common/gameobj.rb` (1,945) and
`common/inventory.rb` (1,754) which are shared with DragonRealms and should be
budgeted against the infrastructure inventory rather than this one.

### 9.4 Distribution of difficulty across the rewrite

| Difficulty | Modules | Approx. lines |
|---|---|---:|
| Pure logic (no game I/O) | Infomon store+cache+parsers, PSMS façade, QStrike, Spell model (minus durations), ActiveSpell, Status, Effects, CritRanks index, all 6 Combat logic files, Creature model, Experience, Wounds, Scars, Injured, ReadyList, StowList, Disk, Overwatch, Claim, Gift, SK, attribute façades, GameObj, Inventory | ~16,000 |
| Needs game I/O (command round-trip) | 7 PSM `use` paths, Bank, Stance, Mana, Fog, Bounty `current`, Society use, Currency `refresh`, Infomon CLI, magic-info, Group commands, Combat Tracker hook | ~2,300 |
| Unresolved (needs a design decision) | Spell duration/bonus expressions (§4.3); society cost/duration/summary lambdas (§7.1); combat def pattern assembly (§5.2) | ~900 |

The "needs game I/O" column is not hard work *per module* — it is a single
primitive (`send command, collect lines until one matches this regex-set or the
timeout expires`) used two dozen times. Building that primitive well, with the
roundtime waits (`waitrt?`, `waitcastrt?`) and the quiet/silent variants Lich has
accumulated, unblocks all of it at once.

### 9.5 Open items

- `UNVERIFIED:` Whether every one of the ~2,730 regexes in the crit tables and
  combat defs compiles under Rust's `regex` crate. Needs a mechanical check:
  extract each pattern, feed it to `regex::Regex::new`, report failures. Expect
  trouble only if any use backreferences or lookaround.
- `UNVERIFIED:` The exact `DB_Store` schema that `SK`, `Combat::Tracker` settings
  and `QStrike` defaults persist into. I read their call sites but not
  `lib/common/db_store.rb` (78 lines) — that belongs with the infrastructure
  inventory.
- `UNVERIFIED:` Whether `effect-list.xml` is versioned or hashed upstream, which
  determines whether Cena can keep consuming the community file or must fork it.
  `Spell.load` only checks file existence (`common/spell.rb:186`).
- `UNVERIFIED:` `attributes/enhancive.rb` (407 LOC) is the largest attribute
  façade and I inventoried only its Infomon key namespace, not its API surface.



