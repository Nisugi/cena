# lich_repo_mirror: what the old Lich repository's scripts hold that Cena could use

**Scope.** Every `.lic` in `reference/lich_repo_mirror/lib/`: **2,130 scripts, 849,631 lines**,
the user scripts of the old Lich repository (`;repository`), mirrored every 3 hours by
<https://github.com/FarFigNewGut/lich_repo_mirror> (mirror commit `d326ee9`, 2026-09-22).
Compared against Cena at `3566f8d` to `a6146e6` (HEAD moved during the survey; each reviewer
re-checked the files it cited). Date: 2026-09-25.

**The question**, the author's: *"Anything that involves data, capturing or using should be
compared against what we capture and our defs at the very least."* So every script that
**captures** game text, **holds** game data, or **uses** character or game data through Lich's
APIs was compared with Cena's classifiers, data tables, consumers and behaviors.

**Not the same corpus as `07`.** `07-script-corpus-api-census.md` measured
`reference/scripts` (elanthia-online, 234 `.lic`). This mirror shares **2** scripts with it
(`loottracker`, `multi`). So bigshot, eloot, go2 and kswole are not here; they are in `scripts`.
MEASURED from `reference/lich_repo_mirror`:
`comm -12 <(ls lib | grep '\.lic$' | sort) <(ls ../scripts/scripts | grep '\.lic$' | sort)`.

---

## 0. How this was done, and how far to trust it

1. **Triage** (`12-lich-repo-mirror/tools/triage.py`): each script scored for capture lines
   (regex matches, waits, hooks), data lines (literal tables) and Lich API calls, and given a
   domain. **1,123 scripts** have capture + data >= 5; the other 1,007 are mostly short macros.
2. **Nine piles** (`tools/assign.py`), one reviewer each, all on one brief (`tools/BRIEF.md`)
   and one baseline of what Cena captures and defines (`tools/cena-baseline.md`, every model
   module, data table, consumer and behavior with its first doc line).
3. **Each finding has a verdict**: HAVE (Cena has it, cited), PARTIAL (a variant or field
   missing), GAP, CONFLICT (their text or data differs from Cena's, both quoted), N/A. Each
   GAP, PARTIAL and CONFLICT has a value (HIGH for M6-M8, MEDIUM, LOW) and a milestone. A GAP
   is a claim about absence, so the brief required more than one search, and those searches
   are in each report's Method section.
4. **The nine reports are the evidence**: `12-lich-repo-mirror/<pile>.md`, with their comparison
   scripts in `tools/<pile>/`. This file is their summary. It does not restate a number that
   is not in a report or re-measured here.

| Pile | Scripts | Substantive | Read deeply | Capture-line depth |
|---|---:|---:|---:|---:|
| [1 hunting engines](12-lich-repo-mirror/1-hunting-engines.md) | 31 | 20 | 13 + 4 by diff | 6 |
| [2 combat](12-lich-repo-mirror/2-combat.md) | 168 | 79 | 13 | 66 |
| [3 creatures](12-lich-repo-mirror/3-creatures.md) | 145 | 90 | 11 | 79 |
| [4 loot and trade](12-lich-repo-mirror/4-loot-trade.md) | 380 | 214 | 40 | 174 |
| [5 boxes and magic items](12-lich-repo-mirror/5-boxes-magic-items.md) | 104 | 87 | 14 | 92 |
| [6 magic and effects](12-lich-repo-mirror/6-magic-effects.md) | 360 | 163 | 40 | 123 |
| [7 character, healing, crafting](12-lich-repo-mirror/7-character-healing-crafting.md) | 284 | 176 | 40 | 136 |
| [8 bounty, events, travel](12-lich-repo-mirror/8-bounty-events-travel.md) | 364 | 174 | 18 | 156 |
| [9 UI, social, utility](12-lich-repo-mirror/9-ui-social-util.md) | 294 | 120 | ~40 | ~80 |

**Re-checked against the code for this summary.** The claims about code Cena already has were
re-read here, not taken from the reports:

| Claim | Result |
|---|---|
| `invalid_targets` imported with the wrong meaning | **VERIFIED**, fixed in the working tree (§1.2) |
| `maneuvers.rs` says nothing announces a cooldown's end | **VERIFIED** (§1.3) |
| `spells.tsv` loses messages and attributes | **VERIFIED**, and wider than reported (§1.1) |
| "Come back in about a minute" dropped; removal line documented, not read | **VERIFIED** (§1.4) |
| Bank withdraw misses "and then hands you"; appraisals need "silver" | **VERIFIED** (§1.5) |
| Box coins collected in part never ledgered | **VERIFIED** (§1.6) |
| `mindState` attributes dropped | **VERIFIED** (§1.7) |
| The prepared spell (`<spell>`) never stored | **VERIFIED** (§1.8) |
| `creature_message::classify` has no caller | **VERIFIED** (§1.9) |
| `movement.rs:107` requires a one-character name | **VERIFIED**, small effect (§1.10) |
| `profile <other>` overwrites the character's own facts | code path VERIFIED; whether the text reaches it is **UNVERIFIED** (§1.11) |
| `creature_hp.lic` injects `health=`, so Cena's evidence is Lich's estimate | **WITHDRAWN**; the game released creature HP about 2026-09-18 (author, §8.1) |

Everything else is the reviewers', with their evidence. Items marked UNVERIFIED need one
capture or one corpus query, and the corpus needs the author's permission first.

**The scripts belong to their authors.** The mirror is Apache-2.0 for its tooling, not for the
scripts; most carry no licence line. What is recommended for porting is **game facts** (message
text, tables of game data), not code. Each report names the author of every HIGH item.

---

## 1. Fix first: defects in what Cena already has

These are wrong now. None needs a new feature.

### 1.1 `spells.tsv` drops data, as the bestiary did

> **FIXED 2026-09-25** by a companion extractor rather than a rebuilt one (`plan/37` Stage 1):
> `tools/extract_spell_extras.rb` → `spell_extras.tsv` keeps every duration and spell
> attribute, every cost as written, the cast procedure, and every message with its type
> (626: the file's 628 less the duplicated 9052's second copy). The 43 spells are pinned in
> `cena-model/tests/spell_extras.rs`. The Sunfist durations and the Barkskin typo are not
> touched.

`crates/cena-model/tools/extract_spells.rb` reads **5** of the attribute names that
`effect-list.xml` uses, and keeps one message per type. MEASURED against
`C:/Gemstone/lich-5/data/effect-list.xml` (the source its header records, mtime 2026-09-13):

| Lost | Count | Where |
|---|---:|---|
| messages overwritten by `to_h` (one kept per type) | **98 of 628, on 43 spells** | `extract_spells.rb:119` |
| `<duration span=>` (the stacking rule) | 173 durations | not read |
| `<duration multicastable=>` | 100 | not read |
| `<duration persist-on-death=>` | 34 | not read |
| `<duration max=>` | 19 | not read |
| `<duration real-time=>` | 10 | not read |
| `<spell stance=>`, `incant=`, `channel=` | 16, 17, 15 spells | not read |

```
ruby -rrexml/document -e '...' effect-list.xml   # attribute census, tools/ in 6-magic-effects
grep -oE "attributes\['[a-z-]+'\]" crates/cena-model/tools/extract_spells.rb | sort -u
  -> availability cast-type name number type
```

The kept message is sometimes the renewal or refusal line (535, 916, 999, per pile 6).
`spells.rs:5` still says "628 messages", and no test checks it. Nothing fails on an attribute
the extractor does not read. **Same fix as the bestiary** (`port-data-whole`): keep every
message with its type, keep every attribute, fail on one not handled, and have a test pin the
count. waggle decides by the stacking attributes, and Maintain needs them to know whether a
recast adds time. **HIGH, M6.**

Also from pile 6, not re-checked here: Sunfist's Contact, Bandages and Determination are
17/3/3 min in `spells.tsv` against 19/5/5 in Lich's `guardians_of_sunfist.rb`, the wiki and a
script. And Barkskin's end message (605, in the author's own `signs`) carries the typo
`disintegrating.\.` from `effect-list.xml:687`, so it cannot match the real line.

### 1.2 `invalid_targets` means "don't count toward fleeing", not "never attack"

bigshot's own settings table: `'flee_count' ... # flee if enemy count is >`, then
`'invalid_targets' ... # but don't count these`, then `'always_flee_from' ... # and always
flee from` (`reference/scripts/scripts/bigshot.lic:3482-3485`). Its only use is the flee count
(`bigshot.lic:8579-8583`). Cena imports it as `ignore`, *"Creatures never attacked"*
(`crates/cena-behavior/src/hunt/import.rs:391`, `profile.rs:82-83`), and `fightable` skips them
(`engine.rs:575-591`). **CONFLICT, HIGH, M6.**

> **FIXED IN THE WORKING TREE 2026-09-25, uncommitted.** The author: port bigshot's meaning,
> *and* keep a never-attack list. `invalid_targets` now imports as `flee.uncounted`, which is
> left out of the count and still fought. `never_attack` is Hydra's own list, never fought and
> not counted, the two ways bigshot uses its learned `untargetable` list
> (`bigshot.lic:8583`, `:8624-8634`). A second difference came out while fixing it: Cena
> counted only creatures its target list names, where bigshot counts its whole hostile roster
> (`bigshot.lic:8579-8591`). The count now follows bigshot. The field was renamed rather than
> reused, so a profile saved with the old `ignore` key is refused by name at load
> (`hunt/chain.rs:19-20`) instead of being read with the other meaning. Tests:
> `hunt_import.rs` `invalid_targets_are_left_out_of_the_flee_count_not_never_attacked`, and
> three in `hunt_engine.rs`. Each engine rule was mutated, and the mutant was caught.

### 1.3 A maneuver's cooldown does announce its end

`crates/cena-model/src/state/maneuvers.rs:41`: *"**That it has ENDED.** Nothing says so."*
Cena's own fixture: `Volley is ready for use.`
(`crates/cena-behavior/tests/fixtures/smithy_engage.xml:271`), and nothing in `crates/` reads
it. `Combatical.lic:4553-4556` reads `^(\w[\w\s]+?) is ready for use\.$` as the end. The
consumer is `plan/33`'s `cooldown "<name>"` guard. **CONFLICT, HIGH, M6.**

> **FIXED 2026-09-25**: `maneuvers::ready_line` and `Maneuvers::note_ready`
> (`crates/cena-model/src/state/maneuvers.rs`). The guard still waits on `plan/33`.

### 1.4 Bounty status

- `Come back in about a minute if you want another task.` returns `None`: `classify_refusal`
  parses the word `a` as a number (`crates/cena-model/src/state/bounty_status.rs:157-161`).
  Five scripts handle this exact line.
- `I have removed you from your current assignment` is in the module's table
  (`bounty_status.rs:24`), and no code reads it.

**CONFLICT, HIGH, M8.**

### 1.5 The selling round: three wordings the ledger misses

> **FIXED 2026-09-25**: the appraisal forms without *silver*, the furrier's *pay you*, the
> surcharged note, the withdraw's *and then* and Lich's other figures (`bank.rb`), in
> `ledger/town.rs`; tested in `cena-model/tests/loot_facts.rs`.

- **An appraisal without "silver"** gives no value, so the planner **keeps the item instead of
  selling it** (`town/plan.rs:481-484`, `:504-507`). eloot's own parser accepts
  `N for it if you want to sell` and `N for this if you'd like`
  (`reference/scripts/scripts/eloot.lic:5910`). Every Cena appraisal pattern needs `silver`
  (`crates/cena-model/src/state/ledger/town.rs:72-76`).
- **The bank's withdraw** is `teller carefully records the transaction,? (?:and )?hands you`
  (`ledger/town.rs:96`). Lich spells it `(?:and then )?hands you` (`lich-5/lib/gemstone/bank.rb:37`),
  as do `bank.lic` and `colmaster.lic`. Cena reads 3 bank reply shapes, and `bank.rb:22-46`
  and `bank.lic:36` have 11, including debt, Teras scrip fees and notes.
- **A surcharged note** (Elven Nations): *He scribbles out a City-States promissory note for
  35000 (minus a small 72 silver surcharge) and hands it to you.* (`duskrunner_support.lic:23`)
  matches neither `pawn_note` nor `gemshop_note`, so the sale is lost and `sold` stays false
  (pile 4, tested in Python).

**HIGH, M6 (plan/31 Stage 4a-4b, plan/34).**

### 1.6 Box coins collected in part are never ledgered

`^You can only collect (.*) of the coins due to your load\.` (`tpick.lic:5718`; eloot
`eloot.lic:5089-5094`). Cena's ledger reads only
`^You gather the remaining ([\d,]+) coins from inside ...` (`ledger/boxes.rs:27`); the behavior
sees `You can only collect` and keeps no figure (`loot/outcome.rs:145`). loottracker has the
same gap, so this improves on the source. **PARTIAL, HIGH, M6.**

### 1.7 The experience bar's numbers are dropped

The `mindState` bar carries `field_exp`, `max_field_exp`, `ascension_exp`, `exp` and
`until_next` (Cena's own `crates/cena-protocol/tests/fixtures/login_burst_full.xml:3`), plus
`lumnis`, `rpa` and `fashlonae` while active. Lich reads them all
(`lich-5/lib/common/xmlparser.rb:725-740`). The frame keeps them, but `apply_bar` takes only
the text and the percent (`crates/cena-model/src/state/character.rs:460-466`), while
`plan/27b-model-api-character.md:64` says `<progressBar>` feeds `field_experience`. A live `lumnis` is the upgrade
trigger `Gift`'s doc waits for (`character.rs:173-177`). **PARTIAL/CONFLICT, MEDIUM.**

### 1.8 The prepared spell is parsed and never stored

> **FIXED 2026-09-25**: `GameState::prepared` (`plan/37` Stage 3).

The parser makes `Frame::Spell` (`crates/cena-protocol/src/parser/thin.rs:49`), and nothing in
`cena-model` handles it. `plan/18:33` recorded "parsed, no field holds it" at M2. A caster must
know what is prepared before `prep` or `release`, and three window scripts show it.
**GAP, HIGH, M6** (and M10's spell-prep indicator).

### 1.9 The bestiary's per-creature lines reach nothing

`creature_message::classify` has no caller outside two test files (`grep -rn classify crates`).
So the 281 creature spell-prep lines and the 162 trigger lines ported on 2026-09-24 do not
reach the hunt yet. Pile 3 adds: **66 of kswole's 114 prep regexes** (in `energywings.lic:234-400`,
grouped by hunting area) match nothing in `creature_messages.tsv`. **PARTIAL, HIGH, M6.**

### 1.10 A Lich regex ported with its bug

`crates/cena-model/src/movement.rs:107`: `^[A-z\s-] is unable to follow you\.$`, from
`lich-5/lib/common/move.rb:290`. The class has no `+`, so no real name matches. The effect is
small: `state/movement.rs:274` (from `move.rb:71`) catches the same line as a substring.
Ashborne's is `^([A-Za-z][A-Za-z'\-]*) is unable to follow you\.` (`Ashborne.lic:10912`).
**CONFLICT, MEDIUM** (it matters once groups move together).

### 1.11 `profile <another player>` may overwrite the character's own facts

`absorb_profile` writes the profile's age, society and citizenship into the character's own
`Identity` and `Standing` (`crates/cena-model/src/state/character/profile.rs:245-265`). Scripts
run `profile <name>` on other players (`plist.lic:66`, `deity.lic`). Whether that output
arrives on the stream `absorb_profile` reads is **UNVERIFIED**, and one capture settles it.
**MEDIUM, M6** (snapshot correctness).

### 1.12 Data defects, most inherited from Lich

Pile reports, not re-checked here:

- **`weapons.tsv`**: six values the wiki and `character-planner.lic` both contradict. The
  backsword DF, and the bola DF, which is a copy of backsword's row
  (`lich-5 weapon_stats_thrown.rb:46`). Also AvD for halberd, naginata, awl-pike and lance.
  `shields.tsv:5` has a tower evade of -0.5 where the wiki says a 0.54 factor (pile 7 §4; pile 2 §12).
- **`Armor::coverage`** (`armaments.rs:173-180`) contradicts its own source
  (`armor_stats.rb:373-376`). The test checks only ASG 1 and 20, where both rules agree
  (pile 2 §4).
- **Crits**: six rows are missing where two tables share a message (crush/neck/1 `Whiplash!`
  is recorded as unbalance rank 3). A fatal acid pattern names one creature, the triton
  assassin (`crit_tables.tsv:28`, from upstream). A first-sentence match reads a fatal
  slash/back/8 as rank 3 (INFERRED) (pile 2 §5-7).
- **The creature templates**: 101 `equipment` rows are wound descriptions ("a bruised left
  eye"), and 9 `skin` values are cut-off possessives ("a goblin's"). Port-whole kept them. Tag
  them; do not drop them (pile 3 §8).
- **Skins in Cena's own bestiary are not typed `skin`**: rolton eye, glossy kiramon chitin,
  chipped troll tusk and others. So the town routes them to the pawnshop, not the furrier
  (pile 4 §4).
- **Display names that do not resolve**: "Arachne priestess", "infernal lich" and "glittering
  crystal crab" exist only as message text inside other templates. `by_name`
  (`creature.rs:325-330`) misses them, and they fall back to 400 HP. The aliases can be derived
  from `creature_messages.tsv` (pile 3 §2). **HIGH, M6 targeting, M8 Bounty.**

---

## 2. M6, the hunt

> **FIXED 2026-09-25**, each in `crates/cena-behavior/src/hunt/`: `messages.rb` whole
> (`crates/cena-model/src/state/incident.rs`) and the hunt's answers to it (`react.rs`:
> disarm recovery, weapon reactions, the sanctum snake, item limit, the hive's ground); the
> attack, injury and cast refusals (`replies.rs`); four of the rest triggers (dread, Wall of
> Thorns poison, confusion, and a rest count, `rest.rs`); environmental flight (`flee.clouds`,
> `vines`, `webs`, `voids`, `messages`, `wander.rs`); the contested room, now claimed on entry
> (`engine.rs`); and being swallowed (`react.rs`). Since fixed: the spirit and wound-rank
> rest triggers (`07d7f36`), boons (`d87a4ae`), and Barkskin's lockout
> (`hunt/maintain.rs`). Fixed on the model side, for the hunt to read: the two hazard
> creatures and the 13 hazard objects (`crates/cena-model/src/state/hazard.rs`), familiars,
> companions and summons (`state/creatures/ally.rs`), and kills that leave no corpse and the
> cold wyrm's phases (`state/creatures/prose.rs`). Still open from this table: the hunt
> reading those three, creature statuses in room prose, and flares for `;combat`.

Lines the hunt needs, which the scripts match and Cena does not. All are GAP unless marked.

| What | Where the text is | Value |
|---|---|---|
| **Lich's `combat/defs/messages.rb`** (240 lines, 2026-09-16): 21 events, including self-disarm (6 forms), hazards, rooted, item limit, bless expired, arrow stuck, bonded weapon return, **Swift Justice charges** (`plan/33`'s held `justice` guard), Arcane Reflex and the weapon-reaction prompt. Cena deferred it on purpose (`state/combat.rs:90-93`), and the hunt is the consumer that was waiting | `reference/lich-5/lib/gemstone/combat/defs/messages.rb` | HIGH |
| **Lich's cast replies**: 38 prepare and result lines (sanctuary's "no need for spells of war", "can't think clearly"...). Cena matches only `[Spell Hindrance` (`travel/routines/casting.rs:126`) | `lich-5/lib/common/spell.rb:21-62` | HIGH |
| **Attack refusals**: `You currently have no valid target.`, `You do not currently have a target.`, `It looks like somebody already did the job for you.`, `You spin about but don't see anything to hit!`. `Gate::Act` checks before sending, which cannot see a kill landing in flight | `osa_attack.lic:29,58`, `smartwiz.lic:341,365` | HIGH |
| **Injury refusals**: `You can't make that dextrous of a move!`, `You can't think clearly enough to prepare a spell!`, `You're not in any condition to be searching around!` | `health.lic:63-85` | HIGH (and M6d) |
| **Rest triggers the profile cannot express**: spirit <= 10%, a wound or scar rank >= N per part, stunned, webbed or bound, poisoned or diseased, "your attack has no effect" three times (wrong equipment), a cycle count to stop an overnight run | pile 1 §5 and special ask 2 | HIGH |
| **Environmental flight**: hazard objects (cloud, void, web, vine, sandstorm, swarm, pod), gas clouds forming, ground about to erupt, sanctuary refusals. bigshot's `flee_clouds/vines/webs/voids` are dropped by the importer (`bigshot.lic:3491-3494`). Two hazard "creatures" Cena would attack: wasp nest and shimmering fungus (`creatures.tsv:484,602`). 13 hazard objects have no `gameobj-data` category | `Ashborne.lic:17317-17345`, `creaturewindow.lic:784`, `hazardwindow.lic:97-113` | HIGH |
| **A contested room**: Cena's `engage` needs the claim every tick (`engine.rs:430`), `wander` will not leave while creatures remain (`:571-573`), and `loot` has no claim check (`:286-330`). So another player's arrival stalls the hunt in place, and it still loots. bigshot claims on entry, gates looting on it, and counts a stranger's floating disk as contesting | `bigshot.lic:9392`, `:7808`, `:7091-7099` | HIGH, INFERRED from the tick order |
| **Swallowed**: "The Belly of the Beast" (roa'ter: small edged weapon, `attack wall`) and "Ooze, Innards" (blunt weapon, `kill organ`) | `explorer.lic:918,4352-4390`; `bigshot.lic:9645-9646` | HIGH |
| **Sanctum of Scales**: a sentinel turns your weapon into a snake. `clench` recovers it, and a failed clench bites and infects (cure at `go2 25250`, `clean vat`). None of these is in the sentinel's 31 message rows | `sanctumwatch.lic:37-52` | HIGH |
| **Familiars, companions, summons** are not known to be friendly. Neither Lich's `gameobj-data.xml` nor Cena's copy has the type, and `fightable` skips only `hostile() == Some(false)`. At risk: a profile with an `any` target. UNVERIFIED whether the game's target list already omits them | `recolor.lic:733-757` (128 companion nouns), `xmlpatch.lic:125-209` | HIGH |
| **Disarmed**: `[Use the RECOVER ITEM command while in the appropriate room to regain your item.]` | `disarmbond.lic:8` | MEDIUM |
| **Reactive weapon techniques**: `You could use this opportunity to <X>!`, then `weapon <x>` | `reflex.lic:20-25`, `weaponreact.lic:23-30` | MEDIUM |
| **Barkskin (605)**: a 301 s cooldown after an absorb | the author's `cab.lic:85-88` | HIGH |
| **Creature statuses outside `crtrStatus`**: `frozen`, `held in place`, `entangled`, which only room prose states. Cena reads that clause for players only (`state/room.rs:341-430`) | 8 scripts; `creaturewindow.lic:1486-1503` | MEDIUM |
| **Flares for `;combat`**: 353 of `flare_patterns.rb`'s 465 message alternatives (2024-2026) match no Cena pattern: custom and festival flares, purified metals, Covert Arts poisons, ensorcell. Several weapon flares match only one verb number | pile 2 §3, §8 | HIGH for `;combat` |
| **Boon creatures**: 65 adjectives mapped to 38 traits and 3 tiers. Cena has the adjective list only, to strip names | `boon_appraise.lic:20-121` | MEDIUM |
| **Boss phases and kills that leave no corpse**: Gem Onslaught captain, cold wyrm grounded, airborne or shielded, and implosions ("Rather abrupt decompression causes X to explode") | `creaturewindow.lic:293-294,1470-1474`, `killcounter.lic:223` | MEDIUM |

## 3. M6c, loot and the town (beyond §1.5 and §1.6)

All MEDIUM unless marked; pile 4 and pile 5 have the lines.

- **Items marked unsellable**: `inventory full` shows `(marked)`, and `mark` answers "has been
  (permanently) marked as unsellable". The selling round cannot see a mark
  (`dont-lose-it.lic:97-123`, `colmaster.lic:1070-1078`).
- **Confirmations the pool and pawnshop do not expect**: the pawnshop's "attempt to resell it
  again within the next 30 seconds", and the trash confirmations "throw the item away again
  within fifteen seconds" and "attached to the container ... of significant value". The pool
  treats an unrecognised trash answer as a failure and drops the box next (`town/pool.rs:175-186`).
  Icemule's pawnbroker says "Not my line, really."
- **Furrier and gemcutter variants**: `I'll pay you N silvers for it.` (`appskin.lic:74-75`),
  where `town.rs:72` has only `give|offer`. Plus the gemcutter's longer form.
- **Two-word qualities** (`above average`): the skin appraisal requires `is of \w+ quality`
  (`ledger/town.rs:59`). INFERRED; the gem pattern already allows `.+`.
- **RECALL's value line**, `estimated to be worth about N silvers` (`merchantical.lic:7311`),
  is not the loresong line the ledger reads.
- **The pool worker by map tag**: `meta:boxpool:npc:<name>` and `meta:boxpool:table:<name>`
  (`tpick.lic:6290-6294`). Cena matches eight worker names, and `plan/31:285-286` already
  records this as not built.
- **The town locksmith's return line** (`spellbookscan.lic:67`), for the locksmith that
  `plan/31:295-296` has not built yet.
- **Loot outside a search**: mug results (`mugreport.lic:216-391`), "hasty search scatters",
  and silver or items passed between characters (GIVE/ACCEPT, `X just gave you N coins`), plus
  `EMPTY` into a container, which moves a box's loot without a GET. These matter to
  multi-session, where one's own characters hand things to each other.
- **Jar hoarding** (`plan/31` §4d, not ported): `Inside X you see N portion(s)`, "is full",
  "not a suitable container" (`ezmove.lic:266-449`).
- **Merchant `order`/`buy` replies**: `Sold for N silver`, `But you do not have enough
  silver!`, "no such item", and the catalog's `<d cmd='order N'>` links. Cena's buy errands
  check only for "hands you" and send fixed order numbers (`travel/routines/shopping.rs:314-324`).
  This is shared with M6d's herbalist (§4).

## 4. M6d, healing (`plan/36-eherbs-port.md`)

`plan/36` Stage 1 is built (`herbs.tsv`, `herbs.rs`, `doses.rs`). Pile 7 found that
`herbs.tsv` is a superset of every herb table in the pile, except `tincture of moss`, some
long names, and the ten Aldoran healing stones (`herball2.lic:44-54`). No table anywhere,
Cena's included, has herb **prices** or **HP healed per bite**. The wiki has HP per bite
(`reference/wiki_clean/Acantha leaf.txt`).

What the unbuilt stages need, with the exact text:

- **Stage 3, the Survivalist's kit**: `The kit contains DOSEs of the following solid herbs:
  acantha (N), ambrominas (N), ...` and the `TINCTUREs of the following liquid herbs` line
  (`healme.lic:35-36,288-289`; `distiller.lic`). HIGH.
- **Stage 4, the herbalist**: menu rows `<d cmd='order N'>name</d>`, `You may order a QUANTITY
  of this item, ORDER something else, or BUY this item.`, `Sold for N silver`, `But you do not
  have enough silver!`, `Thanks for your patronage` (`useherbs.lic:443-452`, `nesherbalist.lic`).
  HIGH.
- **The injury refusals** from §2. HIGH.
- **The `health` command**: bleed per body part per round, `(tended)`, poison or disease
  `Taking N damage per round.  Dissipating N per round.` Cena has the flags only
  (`smartheal.lic:722-730`, `formatfix.lic:46`). MEDIUM.
- **Another character's wounds and scars** as `appraise` and `look` describe them: a 14x3 plus
  14x3 table, whose primary source is already in the repo (`reference/wiki_clean/Wound.txt:13-43`).
  `injurywatch.lic:290` holds all of them in one regex. Needed for healing someone else, which
  plan/36 leaves out. MEDIUM.
- **Death and deeds**: the sting potion (`750 + 150 x level`) and the priestess replies
  (`zombie.lic:186-209`); the deed cost (`deeds^2 x 20 + gs3_level x 100 + 101`,
  `deedcalc.lic:28-57`); and the temple routines with their success and failure lines
  (`gemdeeds.lic:170-224`), a fit for `travel/routines/`. MEDIUM.
- **Ashborne's Empath-first town pass**: an empath heals the group before herbs and selling,
  behind a phase barrier (pile 1, special ask 1, #16-#19). For groups.

## 5. Multi-session: group hunting (after solo M6)

**Ashborne.lic** (NecroDeus, 32,527 lines, releases through 2026-09-19) hunts with several
accounts as a group. Each Lich process runs one character, so it coordinates through **26
files in a shared data directory** plus a remote-command channel. Pile 1 §A catalogues every
file: its name pattern, fields, writer, readers, freshness window, and the decision it serves.
It covers member telemetry, follow and ready gates, the assist target, loot election, a danger
broadcast with rescue, spell and interrupt claims, job election, a phased town round and a
repeat-cycle handoff.

In Hydra most of that becomes a read of another session's `GameState`. What remains is **14
group decisions (pile 1 §D, R1-R14)**, none built. Two matter before groups exist:

- **R13: the room claim should count every character this Hydra runs as "with me"**
  (`Ashborne.lic:7114-7131`, `:22196-22215`). Otherwise two of one's own characters contest
  each other's rooms.
- **R14: a follower whose leader vanished defends the room for 30 s, then withdraws**
  (`Ashborne.lic:20985-21050`). This is the author's recorded reconnect design (`plan/30` §4),
  already running in someone else's code.

## 6. M7, the agent

- **No script calls an LLM.** The ChatGPT and Claude mentions are authorship credits.
  lich-agent-bridge (`reference/lich-agent-bridge`) is still the only prior art.
- **`bigshit2.lic` is a prank** with a lesson for `plan/35`. On random rests it hides all game
  output from the player, posts on public channels and strips the character's armor in town
  (`bigshit2.lic:5306-5395`, `:389-390`). A hook that can swallow lines can blind the player,
  which is an argument for the agent protocol's level and denylist design.
- **LNet** (`lnet2.lic`) is a separate chat protocol: TLS to `lnet.lichproject.org:7155`, XML
  framing, and `data` payloads that are **base64 Ruby `Marshal`**. So a Rust client needs a
  Marshal codec, and unmarshalling a peer's data is an injection risk. Peers can request
  spells, skills, stats, location, health and bounty, all of which Cena's model holds. LOW, no
  milestone. M7 is the nearest neighbour.
- **Rules data for "is this report worth building" questions**: `character-planner.lic` holds
  370 training-cost rows, growth indices, race modifiers, a level-to-experience table (0-100)
  and 75 ascension mnemonics (pile 7 §9). Cena has none of it.

## 7. M8

### 7.1 Bounty

Cena's task parser holds up: `bounty.rs`'s 23 patterns classify all 67 complete real bounty
lines quoted in the mirror correctly (pile 8 §1, run in Python). **What is missing is
everything around the task:**

- **The group-bounty assignment**: `... in order to help <leader> take care of a bandit
  problem` (`go2bandits.lic:57`). It is not in Lich's `bounty/parser.rb` either, so a Lich port
  will not bring it. Also `You have completed this portion of your Adventurer's Guild task.`
  HIGH.
- **The `bounty` report's timer**: `You will be eligible for new task assignment in <time>.`
  matches the `None` prefix, and the wait is discarded. The same report has unspent points,
  lifetime points and per-kind success counts. HIGH/MEDIUM.
- **Guild answers and in-task events**: expedite results, four more refusals, the heirloom
  found (`which looks like the heirloom that you are searching for!`), escort ambush, trap and
  follow lines. HIGH/MEDIUM.
- **Escort data**: the pickup-phrase to room table, drop-off rooms, and **87 escort-aware
  crossings outside the mapdb** in the baseline `escortgo2.lic`. `plan/21` §2d counts only
  about 11, those inside it. HIGH/MEDIUM.
- **Bounty to hunting ground**, the data Cena lacks. Ashborne has 531 zones with start rooms
  and 21,839 room ids (`Ashborne.lic:5097-5629`), huntplan has 1,307 spawn lists over 8,810
  rooms, `atlas_data.db3` has 136 areas with start and boundary rooms, and findcreature has
  6,754 distinct rooms for 460 creatures. Cena has creature to area name and uid range only.
  All use Lich map ids, so they join through `plan/21`. All are old, so verify before use.
  HIGH.
- **The skin-quality ladder** (crude < poor < fair < fine < exceptional < outstanding < superb
  < magnificent, `skintracker.lic:27`). The ledger drops the grade, and the bounty keeps
  `quality` as a bare string, so "of at least fine quality" cannot be tested. MEDIUM.
- **Which stream the task arrives on.** Lich reads the `bounty` stream (`xmlparser.rb:1177`),
  and Cena reads main-window lines only. That is safe only if every `bounty` push also prints
  in the main window. **UNVERIFIED**; one corpus query settles it. HIGH.

### 7.2 Customization: highlights first

- **The highlight format, palette and presets.** `highlight-manager.lic` (Mystienne) and
  `stormsync.lic` read and write Wrayth's highlights: `<settings><palette>` with
  `<strings|names|ignores><h text color bgcolor sound line case word/>`, `@N` colours into a
  110-entry default palette (`highlight-manager.lic:216-244`), and the only shipped presets
  (by profession, race and tag, `:137-214`). **Cena consumes the login `<settings>` blob whole
  and keeps nothing** (`cena-protocol/src/frame/vocabulary.rs:452-463`). `research/11`'s R-H1
  asks for import on day one. HIGH.
- **Settings inheritance.** `stormsync.lic` runs a working global, then game, then character
  chain (`GS-Global -> GSIV -> GSIV-<char>`, `:600-680`). That is `plan/12` §6a.2's chain,
  where Cena's `settings_store.rs` is one file per character. PARTIAL, HIGH, as design prior art.
- **The data behind name and channel highlights**: ESP and thoughts (`[Channel] Name thinks`,
  `[Focused]`), `who` and `fame` rosters, named logons and deaths. Cena classifies only the
  `speech` and `whisper` presets (`message.rs:56-77`). MEDIUM.

## 8. Ruled out, or already had

1. **Creature health is the game's, not Lich's estimate (withdrawn).** `creature_hp.lic`
   (Nisugi, 1.0.0, 2026-09-11, installed in `E:\Gemstone\dev\lich-5\scripts\`) does append
   `health=` and `maxhealth=` to `<crtrStatus>` through a DownstreamHook
   (`creature_hp.lic:58-72`). That raised the question of whether `creature/status.rs:38-63`
   measured Lich's estimate. Three things say it did not:
   - The script adds health only when Lich knows the creature's HP from a template
     (`attrs_for`, `:57-61`). The jeweler Etaenia, whose health row `status.rs` cites, has no
     template.
   - `creaturewindow.lic` v1.33.0 (2026-09-21) records the attributes as "confirmed genuine
     server data -- present in the raw pre-hook server_string, i.e. what session logs capture
     before any DownstreamHook runs".
   - The dates fit: `creature_hp` bridged a gap before the game began sending HP (about
     2026-09-19, per `status.rs`).

   **CLOSED by the author, 2026-09-25:** *"creature health was never a question. Your
   confusion comes from it being released officially a week ago."* The game sends creature
   HP on `<crtrStatus>`, and the author's logger records the stream before any hook runs.
   `creature_hp.lic` predates the release. So the question came from reading a script
   written before the game's feature and not knowing the release date, not from the
   evidence.
2. **Wire tags**: none missing. Every tag the scripts match is in `tags.rs`'s 126. The only
   other names, `db` and `stgupd`, go client to server (pile 9 §12).
3. **Move failures**: `movement.rs` already has every one the travel scripts match, and the go2
   forks add none (pile 8 §9).
4. **Creature coverage**: the lists overlap Cena's 627 almost entirely. bestiary has 465 of
   470 in Cena, findcreature 445 of 460, atlas 494 of 670 (most of its extras are Reim NPCs at a
   placeholder level 999). Only lesser fetid corpse and faceless clay being are genuinely
   absent. All three lists agree on two of Cena's blank levels: greater fetid corpse 42 and maw
   spore 33 (pile 3).
5. **Wands**: all 52 wand spells in `wands.lic` agree with `spells.tsv`, and all 52 names
   classify as `wand`. Only the wand-to-spell map itself is missing (pile 5 §8).
6. **Collectibles**: all 93 names classify correctly (pile 4).
7. **The map**: the mirror's `gs_map/gs_map.json` (2026-09-09) is older than
   `reference/mapdb` (2026-09-20). What the mapdb lacks is the OSA ocean (§10) and the elven
   day pass (§10).

## 9. Questions for the author

1. ~~**Does your logger record the game stream before Lich's hooks run?**~~ **Yes** (author,
   2026-09-25). §8.1 is closed.
2. ~~**`invalid_targets`**~~ **Both** (author, 2026-09-25): bigshot's meaning, and a
   never-attack list. Done in the working tree (§1.2).
3. **Corpus checks** that would settle UNVERIFIED items, each one query. **Not run, and the
   author has not run them either** (2026-09-25):
   - Does `profile <another player>` arrive on the stream `absorb_profile` reads (§1.11)?
   - Does every `bounty`-stream push also print in the main window (§7.1)?
   - The full wording of `for it if you want to sell` (§1.5), and a two-word skin quality (§3).
   - `frozen`, `held` or `entangled` in a creature's room prose (§2).
   - A fatal slash/back/8 crit, whose first sentence matches rank 3 (§1.12).

## 10. Data Cena lacks, with no milestone yet

| Data | Source | Size | Note |
|---|---|---|---|
| Traps: 16 kinds, up to 5 message states each, 408-safety class | `tpick.lic:3827-4131,4875-4890`; `sbox.lic:18-148`; `Calipers.lic:19-41` | 69 branches | for a tpick port |
| Lock difficulty: 38 bands of 40 | `tpick.lic:2317-2356`, `box-buddy.lic:73-111` | 38 | **CONFLICT** above 1355 between two lineages |
| Lockpick modifiers and precision words | `tpick.lic:2316,1974-2033`; `reference/scripts/scripts/lockpicks.yaml` | 20 tiers | xcalipers says veniom 2.15, everything else 2.20 |
| Wand to spell | `wands.lic:21-543` | 52 | spells all agree with `spells.tsv` |
| Item properties from RECALL, ANALYZE and INSPECT | `itemdb.lic:421-579` (Drafix 1.7.0); `recallwps.lic` | parser + 14x4 WPS ladder | Cena keeps the raw text only (`inventory_snapshot.rs:145`) |
| Training costs, growth, race modifiers, experience per level, ascension mnemonics | `character-planner.lic:934-1028` and more | 370 + 101 + 75 rows | `maxcap.lic`'s cost table is stale (29 of 360 differ) |
| Alchemy recipes | `alchemy-recipes.lic` | 883 rows | crafting: Cena has none, confirmed |
| GemStone jewel properties by rarity | `generate-gemstone.lic:21-27` | 70 | absent from Lich core too |
| The guild `gld` report, rogue tasks, guild dues | `grguild.lic:2421-2466`, `checkin.lic:28-40` | 11 professions | guild entry puzzles are HAVE |
| The ocean (OSA, the player-ship system) | `osa-map-plugin.lic`, `ocean-go2.lic` | 1,252 and 1,283 rooms | absent from the mapdb |
| The elven day pass (Ta'Illistim to Ta'Vaalor) | `chrono.lic:85` | 1 route | in neither the mapdb nor `day_pass.rs`, which also never reads `raise` |
| Combat formulas: redux, AS by style, block and parry, feint, bolt AvD and DF | `calcredux.lic:1252-1278`, `as_audit.lic`, `training-buddy.lic:729-823`, `qrs.lic:16003-16070` | | Cena parses every roll line (`combat/resolution.rs:103-122`) but holds no formulas |

Lower still, each in its pile report: the Council of Light's 336 Inquisitor answers, the rogue
and warrior guild task texts, festival and event captures (Cena has the event currencies and
item types, `currency.rs`, `gameobj-data.tsv`), and LNet (§6).

## 11. The author's short list (2026-09-25)

> *"this is my short list of scripts to implement functionality from."*

The author's list, located, and set against what Cena has. It spans both clones: ten of these
are in `reference/scripts` (elanthia-online), which this survey did not cover, so they have no
pile findings. Line counts: `wc -l`. "Mirror" is `reference/lich_repo_mirror/lib`, "EO" is
`reference/scripts/scripts`.

| Script | Where, lines | Author, version | What it does | What Cena has | Survey |
|---|---|---|---|---|---|
| `wander` | mirror, 286 | Tillmen 0.6 | walks a hunting ground within boundary rooms; learns which creatures can be targeted | the hunt's Wander step with `boundaries` (`hunt/profile.rs:105-106`); not the learned lists (§1.2) | piles 1, 3 |
| `tpick` (listed twice) | mirror, 6,830 | Dreaven v44 | picks boxes: traps, locks, lockpicks, 403/404/407/408/416, calipers, loresong | the locksmith pool only (`town/pool.rs`); no trap, lock or lockpick data | pile 5 §5-7; §10 |
| `killtracker` | EO, 1,249 | Alastir 2.11 | kills and finds (gemstones, jewels, dust, klocks), submitted to an external tracker | the ledger reads klocks and dust (`ledger/hunt.rs:21-42`); the combat recorder counts kills; no GemStone jewel property list (§10) | not surveyed |
| `dirtydeeds` (`dirty-deeds.lic`) | mirror, 1,803 | Dreaven v20 | deeds from gems (Landing, River's Rest) or wands and lockpicks (Icemule), with appraisal and a deed calculator | the deed count only; the deed formula is in pile 7 §10. `travel/drive/deeds.rs` is travel steps, not temple deeds: one word, two meanings | pile 7 |
| `autoforage` | mirror, 190 | Tillmen 0.2 | forages herbs by map tags. The author: *"but supports survivalist kits unique storage"* | the herb table (`herbs.rs`), and the Survivalist's kit (plan/36 Stage 3, `state/kit.rs`, being built as of 2026-09-25); no foraging | pile 7 |
| `ebounty` | EO, 3,894 | elanthia-online (Deysh, Nisugi, Tysong, Rinualdo) | the Adventurer's Guild tasks. The author: *"especially the stand alone forage feature"* | task parsing (`state/bounty.rs`, 23 patterns, §7.1); no bounty behavior (M8) | pile 8 (as baseline) |
| `brooch`, `brooched`, `broochboost` | mirror, 67 / 298 / 90 | Dantax 2 / Archeth 1.0 / — | the Lumnis brooch: absorb by moon phase, rub or `boost longterm` when the mind is full | nothing (`grep -rli brooch crates` finds none); the mind bar's attributes are dropped (§1.7) | piles 6-8 |
| `kswole` | EO, 394 | elanthia-online | Feat Absorb and Dispel for Kroderine Soul, Mental Dispel, Shield Mind, driven by creature spell-prep lines by hunting area | 281 spell-prep lines, not yet read by the hunt (§1.9); 66 of its 114 area lines missing from the bestiary (pile 3 §1) | pile 3 |
| `child2`, `echild` | mirror, 261; EO, 519 | Drafix; elanthia-online | escorts a lost child to a guard (a bounty) | the task is parsed; nothing escorts (M8) | piles 3, 8 |
| `combo_fury`, and bigshot's UCS routines | mirror, 318 | — | UCS: jab, tier up to 3, aim, hit and miss, stunned and webbed handling | UCS positioning is parsed (`combat/ucs.rs`); `plan/33` (PROPOSED) merges bigshot's `tier1-3` and `ucs*` guards into `position N` | pile 3 |
| `crumbly`, `ectracker` | mirror, 116; EO, 372 | —; Nisugi 1.1.1 | enhancive charges running out | enhancive totals (`character/enhancive.rs`), not charges | piles 5-7 |
| `elore` | EO, 861 | elanthia-online (Nisugi and others) 2.1.2 | loresinging | the ledger reads the loresong value (`ledger/town.rs:63`); no behavior | not surveyed |
| `exptrack` | mirror, 615 | Nisugi 1.0 | experience over time in a database, with reports by date range | the `experience` report is parsed; one database per character exists (`plan/34`); the bar's numbers are dropped (§1.7) | pile 7 |
| `findwarcamp`, `warcampfind` | mirror, 182 / 159 | LostRanger 0.1 / casis | searches for war camps, by Sigil of Location interference or by a `wander` variant | the achievement count only (`character/profile.rs:140`) | pile 8 |
| `fletchit` | EO, 2,144 | elanthia-online | fletches arrows and bolts, with painting and cresting | nothing (crafting, §10) | not surveyed |
| `blue-tracker` | EO, 579 | Nisugi 1.0.2 | GM ("blue") announcements to Discord channels. The author: *"upgrade it"* | the hub merges announcements across characters (M5, `plan/29`); nothing posts anywhere | not surveyed |
| `blackarts` | EO, 9,184 | elanthia-online 1.3.6 | alchemy | nothing; `alchemy-recipes.lic` has 883 rows (§10) | pile 7 |
| `invdb`, `invdb-beta` | mirror, 5,009 / 6,841 | — | inventory, lockers, banks, tickets, Lumnis and resources, searched **across characters** | each character's inventory snapshot, containers and bank; Hydra holds every character in one process, which is the thing `invdb` works around | piles 5, 7 |
| `lumnismon` | EO, 1,712 | elanthia-online (Vailan, Demandred). The author: *"ma display"* | Lumnis tracked per character, shown across them | `Gift { pulses }` only; `lumnis info` is not read (pile 7 §7); the bar's `lumnis` is dropped (§1.7) | pile 7 |
| `ocean-go2`, the OSA scripts, `sailors_grief_swim_fix` | mirror, 37,482; about 30 `osa*`; 273 | —; Tysong 1.0.0 | the ocean map and player ships; Sailor's Grief swim crossings not yet in the mapdb | none of the ocean is in the mapdb (§10); the swim fix is StringProc crossings, which `plan/21` ports as primitives | pile 8 |
| `trollspeak`, `trollhear` | mirror, 145 / 114 | Tillmen 0.1 | speech into and out of troll: letter substitutions | nothing | pile 9 |
| `wanddupe` | mirror, 67 | Caithris | wand duplication | the `wand` type; 52 wand spells agree with `spells.tsv` (§8.5) | piles 5, 6 |

The two ocean maps the author gave are
<https://gswiki.play.net/images/gswiki/thumb/5/5c/Southern_Ocean_-_Higher_Res.png/300px-Southern_Ocean_-_Higher_Res.png>
and <https://gswiki.play.net/images/gswiki/a/a9/Open_Sea_Adventures_%28thumbnail%29.jpg>.
