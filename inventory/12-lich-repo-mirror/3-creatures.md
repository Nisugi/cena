# 3-creatures: lich_repo_mirror survey

Scope: 145 scripts, 90 substantive; 11 read deeply (bestiary, findcreature, creaturewindow,
crittracker, killcounter, safety, explorer, energywings, compass_and_wings, boon_appraise,
sanctumwatch), 79 at capture-line depth, 55 in the tail (14 of them opened, creature_hp and
atlas + `atlas_data.db3` in full). Cena HEAD 3566f8d (`git -C E:/Cena rev-parse --short HEAD`),
2026-09-25. HEAD moved to 067b52a (skinning, cena-behavior) during the survey. None of the
compared files changed (`git diff --stat 3566f8d 067b52a -- crates/cena-model/data
crates/cena-model/src/state/{creature*,room.rs}` is empty). Scratch scripts and JSON results:
`survey/tools/3-creatures/`.

## Top findings

1. **Creature spell-prep lines: 66 of 114 missing.** energywings.lic:234-400 (the same list is
   compass_and_wings.lic:363) carries KSwole's creature spell-prep regexes grouped by hunting area.
   Against Cena's 281 `spell_prep` rows in `data/creature_messages.tsv`, 42 match, 6 match another
   kind, and **66 match nothing** (`tools/3-creatures/prep_cmp2.py`). The gaps cover Atoll, Bowels,
   Citadel, Confluence, Den of Rot, Duskruin, Moonsedge, the Rift, the Hive, Stormpeak and Sailor's Grief.
   A prep line is the only warning before a creature casts. **PARTIAL, HIGH, M6.**
2. **A creature's display name often does not resolve to its template.** Three lists name
   "Arachne priestess", "infernal lich" and "glittering crystal crab" as creatures. Cena holds all
   three, but only as message text inside the templates `arachne_priest`, `lich` and `crystal_crab`.
   `by_name` (`cena-model/src/state/creature.rs:325-330`) matches template names only, so these
   names fall back to 400 HP (`state/creatures/instance.rs:33`) and lose the template's facts.
   "shadowy spectre" and "luminous spectre" resolve only by accident: both adjectives are boon
   adjectives, and `template_max_hp` strips those (`instance.rs:122-137`). The aliases can be
   derived from Cena's own `creature_messages.tsv`. **PARTIAL, HIGH, M6 targeting and M8 Bounty.**
3. **`creature_hp.lic` injects `health=`/`maxhealth=` into `<crtrStatus>`.** The script is by
   **Nisugi**, v1.0.0, 2026-09-11. It works through a DownstreamHook (creature_hp.lic:62-72, :143)
   and draws on Lich's damage estimate. Cena's health evidence (`creature/status.rs:39-80`: "2
   files of 127", "331 of 331", `health="-10"`) came from logs. If those logs were written after
   the hook, some rows are estimates. creaturewindow.lic:118-129 claims the attribute is in the
   pre-hook stream, and the jeweler row cannot be injected. **Evidence risk, UNVERIFIED, HIGH, M6:**
   confirm the capture point before the hunt trusts `health`.
4. **Hunting areas exist as data.** Two sources hold it:
   - `atlas_data.db3` (beside atlas.lic in the mirror; Jymamon, 2016) has **136 areas**, each with
     a start room, plus **393 boundary rooms** and 300 area-creature links.
   - findcreature.lic has **18,204 room refs over 6,754 distinct rooms** for 460 creature names.

   Cena has only uid ranges per creature (`data/creature_areas.tsv`). A start room and boundaries
   are what a bigshot-style hunt profile needs; a creature's rooms are what a cull bounty needs.
   Both use Lich map ids, joinable through `plan/21`. Both are old, so verify before use.
   **PARTIAL, HIGH, M6 hunt setup and M8 Bounty.**
5. **Sanctum of Scales item theft.** A lithe veiled sentinel turns your weapon into a snake: you
   `clench` it back, and a failed clench bites and infects you, with a cure at `go2 25250`,
   `clean vat` (sanctumwatch.lic:37-52). None of these lines is among the sentinel's 31 rows in
   Cena. **GAP, HIGH, M6.**
6. **Swallowed by a creature.** A roa'ter swallow drops you into the room "The Belly of the Beast",
   where you need a small edged weapon and `attack wall` (explorer.lic:918, :4352-4390). Current
   bigshot adds the ooze case, "Ooze, Innards", with a blunt weapon and `kill organ`
   (`reference/scripts/scripts/bigshot.lic:9645-9646`). Nothing in `plan/30`, `plan/33` or
   cena-behavior handles either. **GAP, HIGH, M6.**
7. **Creature statuses beyond `crtrStatus`.** Eight scripts test `frozen`, `held in place` or
   `entangled`: utilitybeltx, parasitex, fistoffury, coup, linmurdusk, child2, jdisable and star-nap. creaturewindow.lic:1486-1503 (2026-09-22) says "held" and "frozen" appear only in
   the room prose ("that appears X"), which carries no `crtrStatus` flag. Cena reads that clause
   for players only (`state/room.rs:341-430`), and its 13-flag `Status` has neither word.
   **PARTIAL, MEDIUM, M6.**
8. **Two defects Cena inherits from Lich's templates.** Port-whole kept both.
   - **101 `equipment` rows are wound descriptions** ("a bruised left eye" x42, "a completely
     severed left foreleg"...) over 45 creatures, e.g. `lich-5/.../cinder_wasp.rb:79-83`.
   - **9 `skin` values are truncated possessives** ("a goblin's", `goblin.rb:117`). atlas_data.db3
     has the full names.

   **CONFLICT, MEDIUM, M6.** Tag them; do not drop them.
9. **Boss and kill lines Cena lacks.** None of these is in Cena. **GAP, MEDIUM, M6.**
   - Gem Onslaught captain flees or dies (creaturewindow.lic:293-294). The template has 0 message rows.
   - The cold wyrm's grounded, airborne and shielded phases (creaturewindow.lic:1470-1474).
   - Implosion kills that leave no corpse: "Rather abrupt decompression causes X to explode" and two
     variants (killcounter.lic:223).
10. **Boon creatures.** boon_appraise.lic:20-121 maps 65 adjectives to 38 traits and 3 tiers.
    boondetector.lic:173 parses APPRAISE. Cena has the adjective list only, to strip names
    (`instance.rs:38`), and `plan/30-m6-hunt.md:208` leaves boons out. **PARTIAL, MEDIUM, M6.**
11. **Room hazards** (creaturewindow.lic:784). 13 hazard objects, such as windy vortex, black void
    and spiraling ghostly rift, have no category in `gameobj-data.tsv`. **GAP, MEDIUM, M6.**
12. **Smaller gaps, all MEDIUM:**
    - Aim-failure replies: "You cannot aim that high!", "does not have a head!", "is already
      missing that!" (coup, bardcombo, bigshot). M6.
    - Spell-vs-creature-class exclusions (child2.lic:153-166). M6.
    - 114 rogue and warrior guild task texts (rogues, sguild, xrogue). No milestone.

**The overlap asked for, by name** (case-folded exact match; `tools/3-creatures/*_cmp.py`):
- **bestiary:** 465 of 470 in Cena, with 9 level and 3 undead conflicts.
- **findcreature:** 445 of 460, with 35 level conflicts.
- **atlas:** 494 of 670. Its 176 extras are mostly Reim NPCs and Grimswarm roles at a
  placeholder level 999.
- **Cena has creatures none of them list:** 162 of 627 are not in the bestiary, which dates from 2015.
- **Levels:** where Cena's is blank, all three lists agree on two: greater fetid corpse 42 and maw
  spore 33.
- **HP and nouns:** none of the lists carries HP. Their nouns are the name's last word, so there is
  nothing to compare.

**The crtrStatus feed:** Cena's `Status`/`Classification` covers every flag the scripts test
except `frozen`, `held` and `entangled` (finding 7). `gone` is the departure and roster inference
(`state/departure.rs`, `creatures.rs`). **gameobj-data:** the only more complete classification in
the pile is creaturewindow's hazard list. Scripts also test a `companion|familiar|pet` type. It
exists in neither Lich's nor Cena's gameobj-data, so Cena's `hostile` flag from `crtrStatus` is the
better test.

## Data tables Cena lacks or differs on
| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| findcreature.lic:208-8000 | creature -> hunting rooms, by nearest town | 462 `if critter_to_find ==` blocks, 460 names, **18,204 room refs, 6,754 distinct rooms** | name, level, `available_areas` (town), per-town Lich room-id arrays, undead/non-corporeal flags | `data/creature_areas.tsv` (uid ranges, 1,394 rows, 128 area names) | **PARTIAL**: Cena has areas as uid *ranges* from templates; findcreature has the actual *room ids* per creature. 445 of its names are in Cena. It gives rooms for 6 of the 43 Cena creatures that have **no** area row: greater fetid corpse, greater vruul, ice elemental, maw spore, spectral black warhorse, treekin sapling. Room ids are Lich map ids, joinable through the mapdb (`plan/21`). Author Nugt, v0.6, no license line. Data is old (pre-2020; INFERRED from its creature set) | **HIGH, M8 Bounty** (a cull/dangerous task names a creature; this answers *where*), MEDIUM for M6 hunt setup |
| findcreature.lic, bestiary.lic, atlas_data.db3 (together) | **display names that are not template names** | at least 5: "Arachne priestess" (Cena `arachne_priest`), "infernal lich" (Cena `lich`, level 110 in both), "glittering crystal crab" (Cena `crystal_crab`; findcreature level 6 vs Cena 8), "shadowy spectre" and "luminous spectre" (Cena `spectre`) | name -> template | `state/creature.rs:325-330` `by_name` matches the template name only; `state/creatures/instance.rs:117-137` `template_max_hp` retries with a boon adjective stripped | **PARTIAL**: each variant already appears as message text in Cena's own `creature_messages.tsv` (e.g. `arachne_priest death "The Arachne priestess exhales a final curse and dies."`, `lich arrival "An infernal lich strides in..."`), but no name resolves to its template. "shadowy"/"luminous spectre" resolve **by accident**, because both words are boon adjectives and get stripped (and `gameobj-data.tsv:87` then excludes "shadowy spectre" from boons). A name miss falls back to 400 HP (`instance.rs:33`) and loses the template's undead/level/attack facts. The variant names can be derived from the message table Cena already holds | **HIGH, M6** (targeting and the hunt's per-creature facts), M8 Bounty (a task names the displayed creature) |
| findcreature.lic:7444; atlas_data.db3 | faceless clay being | 1 | level 85 (findcreature) / 79 (atlas), Landing | none (`grep -ril "faceless\|clay being" data` empty; no Lich template: `ls lich-5/lib/gemstone/creatures \| grep -i clay` empty) | **GAP** | LOW |
| findcreature.lic (35 rows) | creature level | 35 conflicts of 445 shared | level | `data/creatures.tsv` col 4 | **CONFLICT**, mostly findcreature being stale. Examples: aivren 75 vs Cena 86, steam dervish 70 vs 84, writhing icy bush 27 vs 36, albino tomb spider 16 vs 8, spectre 18 vs 14. Cena's values come from Lich's templates (`lich-5/lib/gemstone/creatures/*.rb`). Full list in `tools/3-creatures/findcreature_cmp.json` | LOW (Cena's source is newer); worth a spot check for the 4 where Cena's level is **blank**: greater fetid corpse 42, maw spore 33, spectral black warhorse 35, treekin sapling 75 |
| bestiary.lic:41-3060 | creature database (Krakiipedia/gswiki, 2015) | 470 active + 21 commented | name, url, level, undead, corporeal, locations | `data/creatures.tsv`, `creature_areas.tsv`, `creature_lists.tsv` (`otherclass`) | **HAVE for 465 of 470 names** (exact match, case-folded). Cena has 162 the bestiary lacks (it is 2015). **5 bestiary-only**, none a clean gap: Lesser fetid corpse (32, Miasmal Forest, `bestiary.lic:1474`) is in no Cena file (`grep -ril "lesser fetid" data` empty); Shadowy spectre (14, Plains of Bone, `:2256`) and Luminous spectre (34, Oteska's Haven, `:1589`) exist only as **message text inside Cena's `spectre` template** (`creature_messages.tsv`: "A shadowy spectre just arrived.", "A luminous spectre floats {direction}."), and a level-34 luminous spectre folded into a level-14 spectre looks like a template error (UNVERIFIED); "Luminescent arachnid" (`:1583`) = Cena `luminous_arachnid`, "Minotaur magi" (`:1679`) = Cena `minotaur_magus`. Author Oweodry, no license line | MEDIUM, M6: see the display-name finding below |
| bestiary.lic (9 rows) | level | 9 conflicts | level | `data/creatures.tsv` | **CONFLICT/fill**: Centaur ranger 23 vs 25, Forest trali 46 vs 44, Forest trali shaman 44 vs 46 (swapped?), Ithzir herald 93 vs 92, Wind wraith 61 vs 63; and **4 where Cena's level is blank** and bestiary has one: Farlook 77, Greater fetid corpse 42, Ki-lin 28, Maw spore 33 (findcreature agrees on fetid corpse 42 and maw spore 33) | LOW; the blanks MEDIUM (M6: a hunt profile's level band reads this column) |
| bestiary.lic (3 rows) | undead flag | 3 conflicts | undead | `data/creatures.tsv` col 11 | **CONFLICT**: Darken bestiary false / Cena true; Firephantom false / true; Ridge orc **true** / false. Cena follows Lich's templates, which say `undead: true` with otherclass "non-corporeal undead" for darken and firephantom (`lich-5/lib/gemstone/creatures/darken.rb:10,21`, `firephantom.rb:10,21`) and `undead: false` for ridge orc (`ridge_orc.rb:10`). INFERRED the bestiary is wrong on all three | LOW |
| bestiary.lic (15 rows) | locations for creatures Cena has no area for | 15 | locations | `creature_areas.tsv` (no row) | **PARTIAL**: bestiary names a region for 15 Cena creatures with no area row, e.g. Black urgh and Spotted velnalin "Yander's Farm", Cook's assistant and Greater vruul "The Broken Lands", Ice skeleton "Glatoph", Wild hound "Orcswold", Nightmare steed "Darkstone Castle" | MEDIUM, M8 Bounty |
| bestiary.lic (region names) | location vocabulary | 231 shared creatures list a region Cena's areas lack | locations | `creature_areas.tsv` col 2 | **Different grain, not a conflict**: bestiary uses wiki regions ("Foggy Valley" 25x, "Icemule Environs" 23x, "Lava Flows", "Marshtown", "Wehnimer's Environs"); Cena uses map sub-areas from uid ranges. 47 creature-area pairs are missing where the name *is* in Cena's vocabulary, e.g. Arachne servant/Major spider "Spider Temple" (Cena: Lower Trollfang only), Krag dweller/yeti "Wehntoph", Pale crab "Sea Caves" | LOW |
| crittracker.lic:61-480 | crush, puncture, slash crit tables transcribed from gswiki | 351 rows | type, location, rank, damage, message, stun/knockdown/amputation/fatal, wound | `data/crit_tables.tsv` (2,394 rows, from Lich `critranks/`) | **HAVE**: 325 of 351 messages match a Cena pattern. Of the 26 misses most are wiki wording vs wire wording. Two real: **crush neck rank 1 "Whiplash!"** is absent from Cena *and* from Lich's `critranks/crush_critical_table.rb:206-262` (ranks 0, 2..9 only; the same text is unbalance neck, `unbalance_critical_table.rb:263`, so it may be deliberately ambiguous); crush right hand rank 5 damage 5 (wiki) vs 8 (Cena). Author "Kaelyr" (AI-generated), no license | LOW, pile 2's domain |
| creaturewindow.lic:784 | room hazards (loot objects that are dangerous) | 13 names | name regex | none: `grep -rn "boltstone\|windy vortex\|spiraling ghostly rift" crates` finds only the boltstone *trap outcome* in `cena-model/src/movement.rs:180`; `gameobj-data.tsv` has no hazard category (`cut -f2 \| sort -u`) | **GAP**: acidic cloud of mist, glimmering boltstone apparatus, pale hovering runestone, cloud, unearthly silvery blue globe, spiraling ghostly rift, sandstorm, vine, black metal bomb, black void, windy vortex, web, whirlwind (excluding disks). Author "Claude Code / Codex / RuseofFools", v1.35.0 2026-09-22, no license | MEDIUM, M6 (a hunt should see a hazard in the room) |
| nairreim.lic:93-95 (and arshreim, fyreim) | Reim NPCs split into commons and bosses | 3 lists | name regex; common nouns; boss nouns | `gameobj-data.tsv:114` `realm:reim` | **HAVE** (Cena's list is larger: adds Sapper Lord/Lady, Raid Leader, ethereal barbarian...). **PARTIAL**: no boss/common split | LOW |
| atlas.lic:1-60 + **`atlas_data.db3`** (in the mirror, not in any pile) | creature + hunting-area database | 670 creatures, **136 areas** (region, start room), **393 boundary rooms**, 300 area-creature links (188 creatures placed); version row `2016.11.22.01` | creature: level, undead, noncorp, hated, name, skin; area: region, start room id, boundary room ids | `creature_areas.tsv` (uid ranges, no start room, no boundaries) | **GAP for areas**: a start room and boundary rooms per hunting area is exactly bigshot's `hunting_room_id` + boundaries, which Cena's hunt profile needs and Cena's data has no table for. For creatures: 494 names shared, 176 atlas-only: Reim NPCs and Grimswarm roles at a placeholder level 999, plus arachne priestess 26, faceless clay being 79, infernal lich 110, shadowy spectre 14 and the luminescent/magi renames. Of those, only **faceless clay being** is absent from Cena entirely; the others are display names of Cena templates (see the display-name row below). Author Jymamon, 2016-2017, no license line | **HIGH, M6 hunt setup and M8 Bounty** (areas; data is 2016, room ids are Lich map ids, verify before use) |
| atlas_data.db3 `creatures.skin` | skin names | 9 fixes | skin | `creatures.tsv` col 39 `skin` | **CONFLICT (Cena defect, inherited)**: 9 Cena rows carry a truncated possessive as the skin: caedera "a caedera's", cave nipper, cobra, goblin, mountain lion, plains lion, polar bear, spectre, water moccasin (`awk -F'\t' 'NR>1 && $39 ~ /'"'"'s$/' creatures.tsv \| wc -l` = 9). The source is Lich's template (`lich-5/lib/gemstone/creatures/goblin.rb:117` `skin: "a goblin's "`). atlas has the whole names ("goblin skin", "cobra skin", "polar bear skin", ...). Also 3 skins where Cena is blank: lesser faeroth "mottled faeroth crest", lesser ghoul "ghoul nail" (krag dweller "brilliant purple opal" is a gem, not a skin) | MEDIUM, M6 skinning / M8 furrier bounty |
| atlas_data.db3 (29 rows) | level | 29 | level | `creatures.tsv` | **CONFLICT/fill**, small deltas (e.g. deathsworn fanatic 95 vs 98, winged viper 62 vs 60); it agrees with bestiary/findcreature on the blanks: farlook 77, ki-lin 28, maw spore 33, treekin sapling 78 (findcreature 75). Its `undead` flag disagrees with Cena on 16, including ghost and crazed zombie as not undead, so that column is not trustworthy | LOW |
| boon_appraise.lic:20-121 | boon adjective -> boon trait -> tier | 65 adjectives, 38 traits, tiers 1-3 | adjective, trait (e.g. "glorious"/"illustrious" -> cheat death, "resolute"/"unflinching" -> crit death immune, "rune-covered"/"tattooed" -> magic immune), tier | `state/creatures/instance.rs:35-60` `BOON_ADJECTIVES` (66, the list only, to strip names); `gameobj-data.tsv:86-87` type boon | **PARTIAL**: Cena knows the adjectives, not what they mean. Every one of the script's 65 adjectives is in Cena's list; Cena's extra is the typo `tattoed` (ported from Lich). gameobj-data's boon regex has 6 the script lacks (enraged, gory, putrid, scintillating, slime covered, stately) and lacks 3 it has (diseased, luminous, wispy). No author line; v1.0 | MEDIUM, M6 (`plan/30-m6-hunt.md:208` lists bigshot's boons as out of scope; this is the table they would need) |
| energywings.lic:234-400 (same list in compass_and_wings.lic:363) | **creature spell-prep lines by hunting area** ("KSwole-style", ~20 areas) | 114 regexes | area comment, creature noun, prep text | `creature_messages.tsv` kind `spell_prep` (281 rows) | **PARTIAL**: 42 match a Cena spell_prep row, 6 match only another kind (attacks, triggers), **66 match nothing** (`tools/3-creatures/prep_cmp2.py`, filling `{pronoun}` four ways). Missing include Atoll siren and magus/warden, Bowels elder and jarl, Citadel herald, Confluence (rumbles, sparks, sizzles, molten appendage), Den of Rot incubus/inciter/vereri/vision, Duskruin arena (7), Moonsedge grotesque/knight/vampire/conjurer, Rift (seraceris, csetairi, warlock, avenger/crusader, vaespilon, cerebralite, siphon, master, lich), Hive thrall and strandweaver, Stormpeak fiend and stormcaller, Sailor's Grief oracle, merrow, buccaneer, mass, kelpie. Author "Claude Code / Codex / RuseofFools", v1.5.4 2026; list credited to KSwole | **HIGH, M6**: a prep line is the warning before a creature casts; bigshot-style hunts react to it, and Cena's bestiary cannot see two thirds of these |
| child2.lic:153-166 (child2_maaghara.lic:155-168, star-nap.lic:14) | spells a creature class ignores | 7 rules | spell -> name regex | `creature_lists.tsv` `immunities` (18 rows, per creature) | **PARTIAL**: class rules Cena has no table for: 410 not on glacei, griffin, grifflet, elemental, wraith, cold guardian; 501 not on glacei, corpse, wraith, elemental, ghost (and vvrael, construct per star-nap); 505 not on glacei, elemental, wraith; 709 not on glacei, griffin, grifflet, elemental; 706 not on glacei, construct, elemental, boar; 201 not on grimswarm, construct. Heuristics from play, author Drafix | MEDIUM, M6 (spell choice in a hunt profile) |
| prioritytargets.rb (loaded by madtarget.lic:1) | kill-first list | 113 names | name | `gameobj-data.tsv` type grimswarm | **PARTIAL**: 102 are Grimswarm roles in caster-first order; Cena's grimswarm regex lacks the roles **bodyguard** and **rogue** (3 races each). The last 5 are Sanctum creatures (deathsworn fanatic, lithe veiled sentinel, shambling lurk, patchwork flesh monstrosity, pale scaled shaper). The order is policy | LOW |
| log_search_no_time.lic:29-60 | creatures whose search can yield a gemstone | 31 name fragments (Hinterwilds, Sanctum, Hive) | name | none as a flag | **GAP (small)**; all 31 are in `creatures.tsv` | LOW |
| rogues.lic, sguild.lic, xrogue_alpha.lic | rogue and warrior guild task texts ("The Training Administrator told you to ...") | 114 distinct task patterns across the three (`grep -ho "told you to [^/]*/" ... \| sort -u \| wc -l`) | task text -> task kind | none: `grep -rn "Training Administrator" crates` empty; Cena has warcries only (`state/character/vocabulary.rs:254`) | **GAP** | MEDIUM, no milestone (guild training) |

## Captures Cena lacks or differs on
| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| creature_hp.lic:10-16, :62-72, :143 | **a script that writes `health=`/`maxhealth=` INTO `<crtrStatus>`** via DownstreamHook, from Lich's damage-tracking estimate, and replays cached tags after each hit | `%( health="#{creature.current_hp}" maxhealth="#{creature.max_hp}")` appended to every `<crtrStatus exist=...>` whose creature has a template HP | `cena-model/src/state/creature/status.rs:39-80` ("health appears in 2 files of 127, both 2026-09-19 or later"; "331 of 331" hostile rows carry health; a dead creature reads `health="-10"`) | **EVIDENCE RISK (UNVERIFIED)**. Author **Nisugi**, v1.0.0 2026-09-11. If the logs behind status.rs were written *after* DownstreamHooks with this script running, some of those rows are Lich estimates, not server data. A negative health is exactly what `max_hp - damage` produces. Against that: creaturewindow.lic:118-129 (v1.33, 2026-09-21) says it confirmed health "present in the raw pre-hook server_string", and the jeweler row (no template, so no estimate) cannot be injected. Cena talks to the server directly, so only server-sent attributes will ever reach it. Nothing in Cena mentions this script (`grep -rn creature_hp plan crates` empty) | **HIGH, M6**: check which side of the hook the health-era logs were captured on before the hunt relies on `health` |
| creaturewindow.lic:1488-1503 | NPC status from room prose, beyond `crtrStatus` | `that (?:is\|appears) X`; `when /frozen\|immobilized\|terrified/ then "frozen"`, `when /held/i then "held"`, `/unconscious\|slumber\|sleeping/`, `/prone\|lying down\|knocked to the ground/` | `state/room.rs:341-430` reads the clause **only for `room players`** (`items_with_status(body, false, true)`); room objs pass `with_status=false` (`room.rs:355`); creatures take status from `crtrStatus` (`creature/status.rs:96-127`, 13 statuses) | **PARTIAL**: the script says (v1.35, comment at :1486-1490) "held" and "frozen"/"terrified" appear in room prose and in no `crtrStatus` flag. Cena's `Status` has no `held` or `frozen` (`grep -n "held\|frozen" creature/status.rs combat/status.rs` empty). Lich reads the NPC clause at `common/xmlparser.rb:1096-1101`. UNVERIFIED that the game writes "held"/"frozen" | MEDIUM, M6 |
| creaturewindow.lic:293-294 (comment :264-292) | Gem Onslaught captain flees or dies | `/battle-worn Empyrean captain.*?portal.*?(?:disappearing from sight\|vanishing\|wink(?:s\|ing)? away\|dives? into it)/`; `/battle-worn Empyrean captain.*?(?:collapses\|fading away with a final shimmer\|leaving nothing behind)/` | `creatures.tsv` has `battle_worn_empyrean_captain` (level 120, HP blank); `creature_messages.tsv` has **0** rows for it (`grep -c battle_worn creature_messages.tsv`) | **GAP**: boss flee and death lines, with the note that the flee text is randomised per encounter and death is two lines ~35 s apart | MEDIUM, M6 |
| creaturewindow.lic:1470-1474 | cold wyrm boss phases | `cold wyrm plummets toward the ground.*radiating wall of devastation` (grounded), `cold wyrm's muscles bunch and she launches herself into the air` (airborne), `Corruscations of color play along a silver-scaled cold wyrm's scaled hide.*disrupting the attack` (shielded) | `creature_messages.tsv` has the wyrm's attacks and stun_breaks but none of these three (`grep -i "plummets toward the ground\|launches herself\|Corruscations" creature_messages.tsv` empty) | **GAP**: a boss mechanic: airborne and shielded phases change what attacks work | MEDIUM, M6 |
| creaturewindow.lic:2080-2134 | Gem Onslaught status | `There is no Onslaught currently active\.`, `There is currently an Onslaught active somewhere around (.+?)\.`, `active for ...`, `Codices found this week: \d+/\d+` | none (`grep -rn "Onslaught" crates` empty) | **GAP** (event) | LOW |
| killcounter.lic:223 | Implosion (720) kills: the creature leaves no corpse | `Rather abrupt decompression causes X to explode`, `Blast disperses the X into a fine mist`, `X inverts as the intense vacuum rips it to shreds` | none: `grep -rn "abrupt decompression\|intense vacuum" crates` empty; "into a fine mist" only in `spells.tsv:273` (Blink); spell 720 row has no messages (`spells.tsv:177`) | **GAP** | MEDIUM, M6 (a kill with no body: the loot planner must not wait for one; the ledger's kill count) |
| killcounter.lic:195 | wind wraith death | `A X releases a groan of mingled ecstasy and relief as X fades away` | `creature_messages.tsv` wind_wraith death rows | **HAVE** | |
| killcounter.lic:220 | immolation | `Wisps of black smoke swirl around X and s?he bursts into flame!` | `combat_effects.tsv`, `combat_results.tsv` | **HAVE** | |
| killcounter.lic:228, :337 | bandit nouns | `thief\|bandit\|brigand\|mugger\|rogue\|highwayman\|marauder\|thug\|robber\|outlaw` | `gameobj-data.tsv:11` `type bandit` (same ten nouns, race-qualified) | **HAVE** | |
| findcreature.lic:54, :8012 | someone hiding; another player's disk or coffin | `obvious signs of someone hiding`; `loot.name =~ /disk\|coffin/` | `state/claim.rs:120`; `state/disk.rs:40` | **HAVE** | |
| explorer.lic:918-920, :4352-4390 (also current `bigshot.lic:9635-9660`) | swallowed by a creature: escape | room title `The Belly of the Beast` -> small edged weapon, `attack wall`; bigshot adds `Ooze, Innards` -> blunt weapon, `kill organ` | none: `grep -rn -i "swallow\|innards\|belly" plan/30-m6-hunt.md plan/33-guard-vocabulary.md crates/cena-behavior/src` finds only "swallowwort" (`loot/worth.rs:31`) | **GAP** in the hunt. explorer's dagger-noun list `alfange\|basilard\|bodkin\|cinquedea\|dagger\|dirk\|knife\|kozuka\|ice pick\|misericord\|parazonium\|pavade\|poignard\|pugio\|scramasax\|sgian achlais\|spike\|stiletto\|tanto` (`explorer.lic:4371`). Author Alastir (bigshot fork 4.11.2, 2022) | **HIGH, M6**: a swallowed character stalls the hunt; pile 1 should confirm from bigshot |
| safety.lic:161, :165, :219 | Hinterwilds creature defences | `The evanescent shield shrouding (.*) flares to life and thickens to create a substantial buffer around (?:him\|her\|it)\.`; `Numerous grotesque limbs in varying states of decay suddenly burst out of the (.*)!` | absorb variant **HAVE** (`combat_results.tsv` intercept rows 40-41); "thickens ... substantial buffer" is matched by no data row: `grep -rl "substantial buffer" crates` finds it only in two fixtures (`cena-model/tests/fixtures/combat/tackle.txt:5`, `cena-behavior/tests/fixtures/smithy_engage.xml`), which shows it is real wire text Cena already receives; the intercept rows (`combat_results.tsv:85-86`) cover only "absorbs the ..." | **PARTIAL** | MEDIUM, M6 (Hinterwilds; it blocks a maneuver the way "absorbs" blocks a blow) |
| safety.lic:117-126 | bard unravel (1013) outcomes | `You feel your song resonate around the .*, pulling at the threads of mana within\.`, `You gain \d+ mana!`, `The silvery tendril continues to wend its way away from the`, `...as if it had entered a vast empty chamber\.`, `The serpentine thread stretching between you and the (.*) fades`, `Your concentration on unravelling the threads of mana is broken\.` | none (`grep -rl "threads of mana\|serpentine thread\|vast empty chamber" crates` empty) | **GAP** (spell outcome; pile 6 may own it) | LOW |
| sanctumwatch.lic:37-52 (snakewatch.lic is the same without the walk) | Sanctum of Scales: a lithe veiled sentinel turns your item into a snake | `Striking with a serpent's unsettling quickness, a lithe veiled sentinel lashes out and grabs your (?:.*)\.  Vile (?:.*) energy lances down (?:his\|her) arm and into the (?:.*), kindling it into an unholy semblance of life.  The (?:.*) form twists and mutates, sprouting scales and cold eyes as it transforms into a (.*)\!` -> `clench <noun>`; failure `You try to reach for a (?:.*) but your interference only enrages it!  Striking like lightning, (?:.*) bites down on your (?:.*)\!`; infection `The flesh around the wound feels hot and cold at the same time, heavy with infection.` -> `go2 25250`, `clean vat` | `creature_messages.tsv` has 31 lithe_veiled_sentinel rows and none of these (`grep -i "serpent's unsettling quickness\|interference only enrages\|heavy with infection" *.tsv` empty) | **GAP**: the hunt loses its weapon (or other item) and must CLENCH it back; an infection has a cure room | **HIGH, M6** (Sanctum hunts) |
| coup.lic:45, :283; bardcombo.lic:276-280; combo_fury.lic:294-298; fisticuffs.lic:952 (and `bigshot.lic:5524, :6565`) | an aimed attack at a part the creature lacks | `You cannot aim that high!`, `X does not have a head!`, `does not have a (?:right\|left) (?:leg\|arm)!`, `is already missing that!` | none (`grep -rn "cannot aim that high" crates` empty); `creatures.tsv` col 14 `limbs` is blank for 298 of 627 | **GAP**: the reply that says an aim failed and why | MEDIUM, M6 (aimed attacks; pile 1 should confirm from bigshot) |
| aim.lic:41-55; fire.lic:78-113 | a creature's wounds read by LOOK, to choose where to aim | `X appears to be dead.`, `X appears to be in good shape.`, `X has (.+)\.`; `severe head trauma and bleeding from .+ ears`, `snapped bones and serious bleeding from .+ neck`, `blinded left eye`, `severed left leg` | Cena tracks a creature's wounds from crits (`state/creatures/body.rs`), not from LOOK; no classifier for the LOOK reply | **PARTIAL** | LOW |
| (found while checking aim.lic and fire.lic) | **Cena's `equipment` list holds wound descriptions** | "a bruised left eye" x42, "a bruised right eye" x31, "a completely severed left foreleg" x5, ... 101 rows over 45 creatures | `data/creature_lists.tsv` list `equipment` (`awk -F'\t' '$2=="equipment" && ($3 ~ /severed\|blinded\|bruis/)' creature_lists.tsv \| wc -l` = 101 rows over 45 creatures; a wider pattern with "wound" also catches one "leather-wound ruddy steel sledgehammer") | **CONFLICT (Cena defect, inherited)**: the wiki scrape behind Lich's templates took a LOOK-at-creature wound line for gear, e.g. `lich-5/lib/gemstone/creatures/cinder_wasp.rb:79-83`. Port-whole keeps them; a consumer of `equipment` (loot worth, disarm) would read "a bruised left eye" as an item | MEDIUM, M6: tag or filter, do not drop |
| terror_watch.lic:186, :255 | fear on a creature | `X looks at you in utter terror!` (terrified begins); `gathers (?:itself\|herself\|himself) and shakes off the fear` (ends) | the end **HAVE** (`combat_effects.tsv:293`); the begin not found (`grep -rn "looks at you in utter terror" crates` empty; five other terrified-add rows at `combat_effects.tsv:230-235`) | **PARTIAL** | LOW, M6 |
| log_search_no_time.lic:81 | sybil kill leaves a gemstone to infuse | `The fallen sybil's power lingers in the area, ready for the claiming.  Quickly, take your gemstone and INFUSE it while you still can!` | none (`grep -rn "fallen sybil\|ready for the claiming" crates` empty) | **GAP** | LOW, M6 loot |
| log_search_no_time.lic:24 | feeder item announcement | `*** A glint of light draws your attention to your latest find! ***` | deliberately not recorded (`plan/34-loot-ledger.md:163-165`) | **HAVE by decision** | |
| otf_suppress.lic:4-24 | Ithzir buff wear-offs, third person | 20 lines, e.g. `The wall of force disappears from around X.` | 18 of 20 in `spells.tsv` / `combat_effects.tsv` (grep per fragment); missing `seems to lose an aura of confidence` | **HAVE** (one missing) | LOW |
| boondetector.lic:173-174 | boon adjectives from APPRAISE | `The X is \w+ in size and about ...`, `The X appears to be (.*)\.` | none (no APPRAISE-creature classifier) | **GAP** (pairs with the boon table above) | MEDIUM, M6 |
| sacrifice_decide.lic:46-50; reap.lic:17-29 | sorcerer soul assessment (Sacrifice / animate) | `enticingly frail`, `susceptible to manipulation`, `bond .* is (?:stalwart and formidable\|firmly bound\|indomitable)`, `You think .* is suitable for animation`, `Accumulated Shadow essence:\s*(\d+)` | none found (`grep -rn "enticingly frail\|Shadow essence" crates` empty) | **GAP** (spell outcome; pile 6 may own) | LOW |
| vial.lic:124-190 | harvesting blood from a corpse | `not enough blood`, `still alive`, `already have some .* in your vial`, `filled with a dark crimson fluid` | `creatures.tsv` col 12 `blood` (bool) only | **GAP** | LOW |
| utilitybeltx.lic:37, parasitex.lic:23, fistoffury.lic:184, coup.lic:279, linmurdusk.lic:17 | more creature status words | `frozen`, `held in place`, `entangled`, `tangled`, `asleep`, `lying down` | `Status` has 13 flags (`creature/status.rs:96-127`); `lying down` is prose for prone; `tangleweed` is a combat status (`combat_effects.tsv`) | **PARTIAL**: same finding as the creaturewindow row above; eight scripts independently test `frozen`, `held in place` or `entangled` (also child2.lic:148, jdisable.lic:8, star-nap.lic:15) | MEDIUM, M6 |
| safety.lic:270-451 | per-creature disabler policy for the Hinterwilds, by profession | disciple, disir, draugr, valravn, mutant, golem, shield-maiden, skald, warg, angargeist, shaper, sentinel, lurk, fanatic, berserker, cannibal, hinterboar, wendigo -> hamstring / feint / unravel 1214 / warcry / stomp | none | **GAP** as policy, not fact. Author Alastir, 2025-11-23 | LOW (profile data a user would write, M6) |

## Script by script
| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| findcreature.lic | 8094 | walk a creature's rooms by nearest town (Nugt, v0.6) | 462 creature blocks with level, town, room-id arrays; hiding-player hook; group check | room table **PARTIAL/HIGH M8**; 35 level conflicts; 15 names not in `creatures.tsv`: 1 real gap (faceless clay being), the rest display variants, escapes (`n\'ecare`) or typos (`darkwode`, `shrivled`, `etheral`) |
| bestiary.lic | 3064 | creature database from Krakiipedia/gswiki (Oweodry, 2015) | 470 creatures: level, undead, corporeal, locations, url | 465 **HAVE**; 1 real gap (lesser fetid corpse); 9 level, 3 undead conflicts; 4 blank-level fills |
| creaturewindow.lic | 4406 | live creature/bounty window (v1.35.0, 2026-09-22) | status words beyond crtrStatus, hazards, Empyrean captain flee/death, cold wyrm phases, Onslaught status, bounty text (pile 8), turn-in NPC names | hazards **GAP**; boss lines **GAP**; held/frozen **PARTIAL**; bounty parsing is Cena `state/bounty.rs` (pile 8) |
| crittracker.lic | 1139 | own melee crit tracker from gswiki tables (AI-generated) | 351 crush/puncture/slash rows | **HAVE** (325/351); crush neck r1 "Whiplash!" absent in Lich and Cena |
| killcounter.lic | 450 | kill counts (Cait) | attack verbs, wind wraith death, immolation, implosion kills, bandit nouns | implosion **GAP**; rest **HAVE** |
| safety.lic | 460 | Hinterwilds disabling by profession (Alastir, 2025-11-23) | 1015/1030/1013 outcomes, evanescent shield "substantial buffer", Grasp limbs, per-creature maneuver table | shield-thickens **PARTIAL**; unravel **GAP** LOW; policy table LOW |
| explorer.lic | 5128 | bigshot 4.11.2 fork (Alastir, 2022) | 236 regexes, 136 not in current bigshot (mostly old CMAN verbs); roa'ter swallow escape; dagger nouns; ammo recovery | swallow escape **GAP HIGH M6** (bigshot has it too) |
| energywings.lic | 1007 | wing pin automation (v1.5.4) | 114 creature spell-prep regexes by area; claim/Dueling Sands gating | 66 preps **GAP HIGH M6** |
| compass_and_wings.lic | 1134 | energywings fork + Leybound Compass (2.0.1, 2026-08-29) | same 114-regex prep list (`:363`) | same as energywings |
| utilitybeltx.lic | 1205 | wing pin + cirrose parasite (Hexbane) | `PRONE_STATUS` incl. frozen, held in place, entangled; `Effects::Cooldowns` names | status words **PARTIAL**; magic items pile 5 |
| parasitex.lic | 246 | cirrose parasite vissi (Hexbane) | same `PRONE_STATUS` | as above |
| boon_appraise.lic | 165 | appraise boon creatures, show traits | adjective->trait->tier table | **PARTIAL MEDIUM M6** |
| sanctumwatch.lic | 67 | Sanctum sentinel item-to-snake, clench, vat | 3 lines | **GAP HIGH M6** |
| snakewatch.lic | 63 | same, no walk to the vat | same | same |
| coup.lic | 361 | coup de grace / ambush aim (Gwrawr, v1.5) | aim failures, disabled-status words, coup replies (`injured enough to be susceptible`, `aware of your intentions`) | aim failures **GAP MEDIUM M6** |
| bardcombo.lic, combo_fury.lic, sbinnacombo.lic | 299, 319, 221 | UCS combo scripts | positioning, `Strike leaves foe vulnerable to a followup (.*) attack!`, aim failures | UCS **HAVE** (`combat/ucs.rs`, `combat_effects.tsv`); aim failures **GAP** |
| fisticuffs.lic | 1005 | UCS tier-up (Bhuryn) | positioning, spin kick opening, stun/web replies | UCS **HAVE**; `You could use this opportunity to Spin Kick` not checked further, LOW |
| fistoffury.lic | 418 | UAC stance/attack | generic death words, `PRONE_REGEX` incl. frozen, held in place | status words **PARTIAL** |
| smartmonk.lic | 513 | monk UAC (Yasutoshi, 6.1.0) | followup jab/punch/grapple/kick, `You are immobilized with sheer terror!`, Lorminstra favor | **HAVE** (terror: `combat_effects.tsv:231`) |
| archery.lic | 385 | ranged hunt (Nisugi) | construct avoidance, appendage and escort exclusions, flying | **HAVE** (`creatures/instance.rs:550-560`, gameobj escort/passive npc) |
| aim.lic, fire.lic | 104, 122 | aim by the creature's LOOK wounds | wound text, "appears to be dead" | **PARTIAL LOW**; led to the `equipment` defect |
| sfire.lic | 173 | archery (spiffyjr) | eye crits | **HAVE** (`crit_tables.tsv`) |
| mechfire_orig.lic, mechbowreload.lic | 275, 92 | mechanical crossbow | loading lines (`You flip open the stock ...`, `can only hold up to five bolts`) | **GAP** LOW (weapon verbs, not creature) |
| partner4.lic, partner2.lic | 199, 30 | partner signalling by emote | friend emotes | N/A |
| ao-combat.lic | 230 | bandit hunting (Daedeus) | bandit nouns, caster nouns, herb cures by wound | bandit **HAVE**; herbs: Cena `data/herbs.tsv` (pile 7) |
| terror_watch.lic | 372 | watch eerie cry, SSR, fear TTL | cry, SSR, terror begin/end | begin **PARTIAL**, end **HAVE** |
| star-nap.lic (tail), child2.lic, child2_maaghara.lic | 22, 262, 301 | escort child with disablers (Drafix) | spell-vs-class exclusions; Swift Justice (`You sense that your surroundings are calm`, `There is no justice`); Maaghara root | class immunities **PARTIAL MEDIUM**; justice **GAP** LOW |
| miserable.lic | 41 | map fixes for the Miserable Forest (Kaldonis) | wayto StringProcs | map (`plan/21`), N/A here |
| official2.lic (tail) | 49 | walk a Sunfist official | noun official | escort **HAVE** |
| abyss.lic | 279 | Arena of the Abyss (Alastir) | arena announcer lines, oculoth reward | **GAP** LOW (event) |
| ebonarena.lic | 269 | Ebon Gate arena (Fulmen) | same announcer family, vathor heal, `regurgitates` reward | **GAP** LOW (event) |
| si_heist.lic | 257 | Solhaven heist event | pouch loot nouns, listen/track replies | **GAP** LOW (event) |
| gtot.lic | 97 | event door knock | knock replies | N/A, event |
| high_ds.lic | 82 | wave-based casting (Tysong) | AS/DS line, element by name (crimson/fire vs ice/snow) | N/A |
| log_search_no_time.lic | 143 | search logs for gemstone finds (paths name Nisugi) | search, feeder, key, dust, jewel, idol, sybil lines; 31 gemstone creatures | mostly **HAVE** (`state/ledger/hunt.rs:49`); sybil **GAP** |
| tracker.lic | 345 | loot per creature into SQLite (Tristes, 2018) | search, decay words, dialogData | **HAVE** (ledger, `plan/34`) |
| vial.lic | 302 | blood harvest | vial replies | **GAP** LOW |
| sacrifice_decide.lic | 76 | sorcerer sacrifice readiness | assessment phrases | **GAP** LOW |
| calc-td-offset.lic | 44 | measure a creature's TD with 417 | `CS: ... - TD: ... + CvA` line | **HAVE** (`combat/resolution.rs`) |
| scalcflares.lic | 114 | flare counter (spiffyjr) | 3 flare lines | **HAVE** (`combat_effects.tsv`) |
| sadamantine.lic | 115 | incoming attack vs a defence | `hits you, but .* shatters into a thousand`, `attack bounces harmlessly` | **GAP** LOW |
| scancritters.lic | 135 | PEER to scan adjacent rooms (Kynilir) | double-bold creature links in PEER output | **GAP** LOW (no PEER classifier found: `grep -rn "You peer" crates` empty) |
| targetid.lic, target_numbers.lic | 216, 275 | tag creatures in output | `<pushBold/>...<a exist>` shape | N/A (display) |
| roomcreature.lic | 40 | announce arrivals | familiar window line, bandit nouns | **HAVE** |
| cfind.lic | 296 | creature finder on huntplan (Mertyn) | hiding, disks (`bassinet\|cassone\|chest\|coffer\|coffin\|coffret\|disk\|hamper\|saucer\|sphere\|trunk\|tureen`) | disks **HAVE** (`state/disk.rs:40`); huntplan is pile 1 |
| findnpc.lic | 170 | wander until an NPC is found | targeting reply | N/A |
| madtarget.lic | 87 | priority targeting from prioritytargets.rb | 113 names | **PARTIAL** (grimswarm bodyguard, rogue) |
| sbounty-bandit-example.lic | 102 | bandit bounty example | npc type bandit/aggressive | **HAVE** |
| stomp.lic | 29 | Tremors when a target is up | status words | **HAVE** |
| turnbrooch.lic | 100 | brooch colours | `currently set to (\w+)` | pile 5 |
| arshreim.lic, fyreim.lic, nairreim.lic | 594, 582, 579 | Reim wave attacker (Tysong treim 2.3/2.4, per-character copies) | Reim NPC name list, boss/common nouns, Reim status report, group, OOC whisper protocol | names **HAVE** (`gameobj-data.tsv:114`); boss split **PARTIAL** LOW; `nairreim` is the newest (2.4, adds clear_title) |
| xrogue_alpha.lic, xrogue_teach.lic, rogues.lic, sguild.lic | 3526, 3423, 3785, 3745 | rogue (and warrior, sguild) guild task automation (Xanlin v51/v50; Tgo01; spiffyjr) | 114 task-assignment patterns, audience/partner emotes, lock mastery, calipers | **GAP MEDIUM**, no milestone. xrogue_alpha is v51 of xrogue_teach (v50); the diff is audience handling and one task regex |
| grinsawl.lic | 2850 | grinsawl card capture/foil (Zedarius, 0.5.19) | card, tome, foil, capture-failure lines | N/A to creatures; event item, LOW |
| 735.lic, 735_w_energy.lic | 376, 416 | Ensorcell success odds (Tgo01 v33; Dantax fork) | 9 difficulty phrases, workshop check, kill tracking via `status =~ /dead/` | **GAP** LOW (spell 735 odds; pile 6) |
| wannabesig.lic | 2046 | free signature verbs (Ycelacie, 1.3) | 1,957 verb->emote lines | N/A (social) |
| verbwindow.lic | 2159 | verb window (1.8.1, 2025) | `signature` list parse `<d cmd="signature view X">`, race/culture verb tables | N/A (social); LOW for M8 macros |
| signature.lic, resay.lic, rp.lic | 520, 783, 967 | verb helpers, speech check | verb help parsing | N/A |
| pupper.lic, butterfly.lic, zalood.lic | 992, 299, 161 | client-side toys | roomDesc rewrite | N/A |
| totcandy.lic | 670 | Halloween candy bag | bag replies | N/A (event) |
| phase.lic | 167 | phase containers | grab/remove replies | N/A |
| rested.lic | 328 | log out when rested | mind states | N/A (Cena has mind state in `state/character.rs`) |
| ndaily.lic | 110 | Nyessem daily quest | `You need to find (\d+) more` | N/A (event) |
| test.lic | 35 | personal heal test | death, "suddenly looks much better", `Your arms are forced down to your sides!` | N/A |
| queue.lic, MAcomm.lic, crosscharcom.lic, crosscharcom2.lic, gwrawrmon.lic | 80-208 | multi-character messaging | whispers, sockets | N/A (Cena is one process: `plan/29`) |
| copy_script_settings.lic, plat_to_prime_copy.lic, test_server_copy.lic | 470, 259, 259 | settings/mapdb copy between instances | instance names `GSF\|GSIV\|GSPlat\|GST` | N/A |
| armortap.lic, emptylocker.lic, honor_among_thieves.lic | 49, 48, 68 | tap-to-armor, locker, room notice (Nisugi) | tap and locker replies | N/A |

## Tail

55 scripts with capture + data < 5, scanned by name and first lines; 14 opened.

- **Opened, with findings above**: creature_hp (the health injection, read in full), atlas + nesatlas (and `atlas_data.db3`, read in full), boondetector, otf_suppress, star-nap, eattack (a user-maintained non-corporeal list, default `["non-corporeal"]`), reap, jdisable (maneuver immunities by name: no sweep on tegu, crab, warcat, dobrem, burgee, beetle, worm, mammoth, ursian, lion, lizard, bog spectre; no trip on spectre; no vine on glacei, shaman, spectre, shade: **PARTIAL** LOW, Cena has body type in `creatures.tsv` but no maneuver eligibility), runaway (flee from lunatic, warlock, csetairi unless a caedera is present), rgem (the GEM real-items help text; N/A), official2, fofgames, getgem.
- **Weapon-skill pickers**: swordmaster, swordshieldmaster, shieldmaster, snikt. N/A.
- **Single-spell or single-maneuver loops**: lullabye, 410noprone, 502slow, 703, bane, talisman, talisman_sleep, transcend, manna203, reactive2, setshot, linmur, linmurdusk, ashlam (generic death words `and dies\.\|motionless\.\|in a heap\.\|and finally go out\.`), ai-unstun, tinytarget (small-creature name words), vtarget. Their status words (frozen, entangled, lying down) are counted in the status finding.
- **Group helpers**: better_pull, easy_pull, autostand, tapheals, taparmor, tapfriend, armorreinforcement, support, add, wta_accept, go2esp. N/A.
- **Movement and misc**: dragstop, switch_target, moveall, table, order, waypoint, expbrief, targetwin, ssword. N/A.

## Method

All scratch is in `survey/tools/3-creatures/`. The mirror is `E:/Cena/reference/lich_repo_mirror/lib`
and Cena's data dir is `E:/Cena/crates/cena-model/data`.

- **Headers and capture lines**: `dump.py <scripts>` wrote each script's header plus every line
  matching `=~ /`, `!~ /`, `waitforre`, `matchtimeout`, `matchwait`, `DownstreamHook`, `when /`,
  `Regexp.new`, `%r{`, `.match(` to `dump/<script>.txt`.
- **bestiary overlap**: `bestiary_cmp.py` parses `creature_database` (470 active entries, 21
  commented), case-folds names against `creatures.tsv`, and compares level, undead and areas.
  Output: `bestiary_cmp.json`. `area_cmp.py` does the location-vocabulary comparison.
- **findcreature overlap**: `findcreature_cmp.py` parses every `if critter_to_find == "..."`
  block (462) for `critter_level`, `available_areas` and `*_rooms = [...]`. Output:
  `findcreature_cmp.json`. The room counts (18,204 refs, 6,754 distinct) come from the same script.
- **atlas**: `atlas_cmp.py` opens `atlas_data.db3` read-only (`sqlite3`, `mode=ro`). It gives the
  table counts from `select count(*)` and compares name, level, undead and skin with `creatures.tsv`.
- **crit tables**: `crit_cmp.py` parses crittracker's `TABLES` (351 rows). It substitutes
  `[target]` and tests each message against Cena's `crit_tables.tsv` patterns of the same type.
  Its "fatal differs" output is a parser artifact ("Fatal" vs `\bF\b`) and was ignored.
- **spell preps**: `prep_cmp2.py energywings.lic` extracts the `Regexp.union` members. It tests
  each against every `creature_messages.tsv` row, filling `{pronoun}` as her, his, its and him,
  and `{target}` as "you".
- **Absence claims, each with the grep that found nothing** (paths under `E:/Cena/crates` unless
  stated):
  - hazards: `grep -rn "boltstone\|windy vortex\|spiraling ghostly rift"`, plus
    `cut -f2 gameobj-data.tsv | sort -u`
  - Empyrean captain messages: `grep -c battle_worn creature_messages.tsv` gave 0
  - wyrm phases: `grep -i "plummets toward the ground\|launches herself\|Corruscations" creature_messages.tsv`
  - Implosion: `grep -rn "abrupt decompression\|intense vacuum"`
  - Onslaught: `grep -rn "Onslaught"`
  - swallow: `grep -rn -i "swallow\|innards\|belly"` over plan/30, plan/33 and cena-behavior/src,
    and `grep -rn -i "belly of the beast\|kill organ\|attack wall"` over crates and plan
  - Sanctum: `grep -i "serpent's unsettling quickness\|interference only enrages\|heavy with infection" *.tsv`
  - aim failures: `grep -rn "cannot aim that high\|injured enough to be susceptible"`
  - guild tasks: `grep -rn "Training Administrator\|guild task"`
  - held/frozen: `grep -n "held\|frozen" creature/status.rs combat/status.rs`
  - NPC prose status: `grep -rn "that appears\|that is stunned\|(dead)"` over crates, then read
    `room.rs:341-430`
  - creature_hp: `grep -rn creature_hp plan crates`
  - bard unravel: `grep -rlE "threads of mana|serpentine thread|vast empty chamber|silvery tendril"`
  - sybil: `grep -rn "fallen sybil\|ready for the claiming"`
  - terror begin: `grep -rn "looks at you in utter terror"`
  - missing creatures: `grep -ril "lesser fetid" data`, `grep -ril "faceless\|clay being" data`,
    and `ls lich-5/lib/gemstone/creatures | grep -i ...` for each
- **Things first taken for gaps, then found in Cena** (listed so nobody re-raises them):
  - bandit nouns: `gameobj-data.tsv:11`
  - appendage exclusion: `creatures/instance.rs:550-560`
  - Reim names: `gameobj-data.tsv:114`
  - someone hiding: `claim.rs:120`
  - disks: `disk.rs:40`
  - feeder item: not recorded, by decision (`plan/34:163-165`)
  - dangerous-creature bounty: `bounty.rs:355`
  - evanescent shield absorb: `combat_results.tsv` intercept rows
  - Arachne priestess, infernal lich, shadowy/luminous spectre: message text inside other
    templates (`grep -i ... creature_messages.tsv`)
- **Counts about Cena's own data**:
  - wound rows in equipment: `awk -F'\t' '$2=="equipment" && ($3 ~ /severed|blinded|bleeding|trauma|bruis|scar|wound|broken|missing/)' creature_lists.tsv | wc -l` gave 102, one of which is a sledgehammer
  - truncated skins: `awk -F'\t' 'NR>1 && $39 ~ /'"'"'s$/' creatures.tsv | wc -l` gave 9
  - blank levels: `awk -F'\t' 'NR>1 && $4==""' creatures.tsv | wc -l` gave 30
  - creatures with no area row: 43 (inline Python over `creature_areas.tsv`)
- **Committed fixtures instead**: a search of Cena's own `*.xml`/`*.txt` fixtures for the NPC
  prose clause (`grep -rhoE "</a><popBold/></b>? that (is|appears) [a-z ]+"`) finds "flying
  around" (12), "stunned" (20), "dead" (8) and "lying down" (2). It finds **no "held" and no
  "frozen"**, so finding 7 rests on the scripts' say-so.
- **Not done**: no log-archive search (the brief forbids it). So "held"/"frozen" in room prose,
  and the capture point of the health-era logs, stay UNVERIFIED.
