# 2-combat: lich_repo_mirror survey

Scope: 168 scripts, 79 substantive; 13 read deeply, 66 at capture-line depth (every regex
scanned against Cena, GAPs read by hand); 89 in the tail, all name-scanned with capture lines
grepped and 10 opened. Plus `flare_patterns.rb` (in no pile; two pile scripts load it) and
`crittracker.lic` (pile 3; special ask 2 names it). Cena HEAD 3566f8d
(`git -C E:/Cena rev-parse --short HEAD`), 2026-09-25; the working tree has uncommitted changes
on `m6-hunt` (per the session's git status), so line numbers are of the tree as read.

## Top findings

1. **Cena's own fixture refutes a claim in `maneuvers.rs`** (CONFLICT, HIGH, M6).
   `crates/cena-model/src/state/maneuvers.rs:41` says of a maneuver cooldown ending: "Nothing
   says so." `crates/cena-behavior/tests/fixtures/smithy_engage.xml:271` is
   `Volley is ready for use.`, and the line before it is a `Cooldowns` dialog bar with
   `time='00:01:26'` (against `:34`, "the game sends no other statement of one").
   `Combatical.lic:4553-4556` reads `/^(\w[\w\s]+?) is ready for use\.$/` as the cooldown's end;
   at least four scripts in other piles wait for it. `plan/33:109`'s `cooldown "<name>"` guard is the consumer.
2. **The game's refusals of an attack are not recognised** (GAP, HIGH, M6 hunt).
   `You currently have no valid target.  You will need to specify one.` (`osa_attack.lic:58`),
   `You do not currently have a target.` (`osa_attack.lic:29`; also Lich `psms.rb:213`),
   `It looks like somebody already did the job for you.` (`smartwiz.lic:341`),
   `You spin about but don't see anything to hit!` (`smartwiz.lic:365`). No hit in `crates/`
   for any. `Gate::Act` (`crates/cena-session/src/command/verdict.rs:349-356`) checks before
   sending, which cannot catch a kill that lands while the command is in flight.
3. **About 350 flare messages have no Cena pattern** (GAP, HIGH, M6 `;combat`).
   `flare_patterns.rb` (2024-09 to 2026-06) has 324 damaging and 141 non-damaging message
   alternatives; 244 and 109 match nothing in `combat_*.tsv`. Mainly custom flare messaging
   (Fatal Afflares 47, Blessed Standard Reckoning 37, Quinton Manse 11), purified metals, lore-flare
   repeats, weapon and item scripts, Covert Arts poisons, gemstone properties and ensorcell.
   Section "Special ask 3".
4. **`Armor::coverage` disagrees with its own source** (CONFLICT, MEDIUM).
   `crates/cena-model/src/state/armaments.rs:173-180` buckets sub-groups 1-4, 5-8, 9-12 and 13-20.
   `lich-5/lib/gemstone/armaments/armor_stats.rb:373-376` uses the position inside each group
   (torso = 1,5,9,13,17; all = 2,8,12,16,20), and so do `crit-tracking.lic:2217-2235` and
   `qrs.lic:15702-15738`. `tests/armaments.rs:142-153` checks ASG 1 and 20 only, which both rules
   answer the same way. Nothing reads it yet.
5. **Six crit rows are missing because two tables share a message** (PARTIAL/CONFLICT, MEDIUM).
   The clearest is crush/neck/1 `Whiplash!` (damage 2): all four crit scripts have it, and Cena
   records it as unbalance/neck/3 (damage 5, stun 2, `crit_tables.tsv:2219`). Also crush/chest/4,
   puncture hands/7, disruption rank 1 (3 locations), ucs_jab/back/6.
6. **A first sentence that matches another rank** (CONFLICT, MEDIUM, INFERRED impact). Replaying
   Cena's first-match lookahead (`parse/damage.rs:61-79`) over the gswiki texts, a fatal
   slash/back/8 is read as non-fatal rank 3 (`crit_tables.tsv:1428`), and slash/right_arm/3 as
   rank 5 (`:1450`). Crit sentences arrive one per line (VERIFIED for one crit, `arch_kill.xml:9-10`). The corpus would settle it.
7. **A fatal crit pattern names one creature** (CONFLICT, MEDIUM). `crit_tables.tsv:28`
   `^Acid hits the triton assassin full in the eye.` (acid/left_eye/6, fatal), inherited from
   `acid_critical_table.rb:533`; `crit_type_tracker.lic:1607` has `.*?`.
8. **Weapon flares fix the verb's number** (CONFLICT, MEDIUM, INFERRED). Cena has impact, magma
   and water only as plural with `at the <target>` (`flares.rb:136,142`), and fire, lightning,
   disruption, disintegration and grapple only as singular. `flare_patterns.rb:203-326` allows
   both for every element. Cena's own acid, steam and terror rows show that both forms occur.
9. **Reactive weapon techniques** (GAP, MEDIUM, M6). `You could use this opportunity to <X>!`
   and then `weapon <x>` (`reflex.lic:20-25`, `weaponreact.lic:23-30`, 3 more). Nothing in
   `crates/` or `plan/30`/`plan/33`.
10. **Spell results missing from the attack defs** (PARTIAL, MEDIUM). Five of Pain (711)'s six
    tiers, with damage share and RT (`pain.lic:14-39`, `crit_type_tracker.lic:2759-2764`), where
    Cena has only `twists in great pain!` (`combat_attacks.tsv:62`). Also Bone Shatter's killing
    line and Limb Disruption's four breaks.
11. **Lines from the author's own `tracking.lic`** (Nisugi, v1.1.0 alpha) that the later Lich tracker
    and Cena do not carry (GAP, MEDIUM). Ranger-trinket resist and negate (10, `:98-107`), ensorcell
    (5, `:131-135`) and camouflage casting (`:118`). Other hunt-safety lines also have no
    classifier: lurk infestation (`lurkcheck.lic:32-40`), anti-magic dissipation
    (`osacombat.lic:1275-1278`) and room-effect spell objects (`osacombat.lic:3097-3103`).
12. **Calculators (special ask 4).** Cena holds the **inputs**: every roll line is parsed
    (`crates/cena-model/src/state/combat/resolution.rs:103-122`: AS/DS/AvD/d100/endroll,
    CS/TD/CvA, UAF/UDF/MM, SMR, SSR, FS/FD). It also holds DF and AvD by armor
    (`weapons.tsv`), crit divisors by group (`armaments.rs:158`) and skill-rank bonus
    (`spellsong.rs:317`). It holds **no formulas**. The scripts hold redux
    (`calcredux.lic:1252-1278`), AS by style, stat, CMAN, penalties and society
    (`as_audit.lic:696-730,2001-2150`), block and parry (`training-buddy.lic:729-823`), feint RT
    and stance (`feint_calc.lic:44-51`), crit weighting (`crit-tracking.lic:2192-2215`) and bolt
    AvD and DF (`qrs.lic:16003-16070`, 2012). The data check found five AvD typos and a copied bola
    DF row in Lich's `weapon_stats_*.rb`; `weapons.lic` and `crit-tracking.lic` agree with each
    other against it (LOW).

## Special ask 2: the crit scripts against `crit_tables.tsv`

Cena's table is Lich's `critranks/*.rb` (header dated 2024/06/25), 2,394 rows over 21 types
(`crates/cena-model/src/crit.rs:3-6`). Method: every script entry was turned into a sample line
(its regex with wildcards filled, or its literal text), run against every Cena pattern with
Python `re` (all 2,394 compile), and the best hit's type, location, rank, damage, stun and fatal
compared (`tools/2-combat/critcmp.py`, `summary.py`).

| script (author, date) | entries | same | same but side-less location | a field differs | no Cena pattern matches (Cena has the key, text differs) | Cena has no key | not crit rows |
|---|---|---|---|---|---|---|---|
| `crit_type_tracker.lic` (Gibreficul, "5-28-2013 V-02.01", :3) | 2,483 | 1,912 | 207 | 142 | 73 | 1 | 148 |
| `ctt_win.lic` (Gib's 11-27-2011 table + Sam's window, :3-4) | 1,869 | 1,424 | 157 | 98 | 60 | 1 | 129 |
| `crit-tracking.lic` (Tgo01, v11, :14-15) | 390 | 330 | 37 | 11 | 11 | 1 | 0 |
| `crittracker.lic` (Kaelyr, AI-written, gswiki revs 209174/209175/111839, :3,29-32; pile 3, included because the ask names it) | 351 | 281 | 34 | 11 | 24 | 1 | 0 |

**Types.** No script has a crit type Cena lacks. Cena has types no script has: `steam` (121
rows; `ctt_win.lic:15` "Unfinished: Steam"), `vacuum` (97; the 2013 script's vacuum rows match
Cena but carry unparsed labels), `generic` (1). The UCS tables (jab, punch, kick, grapple) are in
`crit_type_tracker.lic` only (added 2013, :7); `ctt_win.lic` is the 2011 table without them.

**Which is newer.** `crit_type_tracker.lic` (2013) is newer than `ctt_win.lic` (2011) and shows it
on the wire: the 2011 table has 44 patterns the 2013 one lacks, most of them multi-sentence
(`/Slash across right eye. Hope the left is working./`), and a committed fixture shows those two
sentences arrive as **two lines** (`crates/cena-behavior/tests/fixtures/arch_kill.xml:9-10`:
`Slash across right eye!` then `Hope the left is working.`). So the 2011 regexes could not match;
2013 rewrote them to one sentence. Of the 142 field differences in the 2013 script, most are
stun counts one round apart or its own `??`/`Unknown` guesses (e.g. `:2051` `Unknown` vs Cena 16).
Cena's source is 11 years newer and agrees with the gswiki transcription (`crittracker.lic`) on the
rows I spot-checked (`Bladder impaled`: gswiki 35 = Cena 35 = `crit-tracking.lic:2110` 35, against
`crit_type_tracker.lic:175` 25 and `calcredux.lic:1066` 25). I treat Cena as current except where
listed below.

**Keys Cena lacks because two tables share one message.** These rows are absent from Lich's
`critranks/*.rb` source itself, not dropped by Cena (VERIFIED: the rank lists parsed from
`crush_critical_table.rb` are neck 0,2-9 and chest 0-3,5-9; `puncture` right_hand 1,8,9;
`disruption` neck 2-9, right_eye 3,4,7; `ucs_jab` back 0-5,7-11; `plasma` abdomen 0-6,8,9).
The likeliest reason is the one `crit.rs:159-170` records for the pair Lich did keep: its table
is a Hash keyed by `Regexp`, so a second identical pattern cannot coexist (INFERRED). The
message is always attributed to the other key:

| missing key (damage) | script evidence | Cena attributes it to | verdict |
|---|---|---|---|
| crush/neck/1 (2) `Whiplash!` | `crit_type_tracker.lic:29`, `ctt_win.lic:73`, `crit-tracking.lic:1768`, `crittracker.lic:75` (gswiki) | unbalance/neck/3, damage 5, stun 2 (`crit_tables.tsv:2219`); `grep -c Whiplash crush_critical_table.rb` = 0 | CONFLICT |
| crush/chest/4 (20, stun 3) `Whoosh. Several ribs driven into lungs.` | `crit-tracking.lic:1804`, `crittracker.lic:111`; `crit_type_tracker.lic:63` labels it "RANK 4-5 ... 20-25" | crush/chest/5 (25, stun 5) (`crit_tables.tsv:259`) | PARTIAL |
| puncture/{right,left}_hand/7 (15, stun 4) `Strike to wrist severs right hand!` | `crit-tracking.lic:2154,2165`, `crittracker.lic:315,326`, `calcredux.lic:1128,1133` | rank 8 (18, stun 5) (`crit_tables.tsv:1348,1351`) | PARTIAL |
| disruption/{neck,right_eye,left_eye}/1 (5) `Throat strike causes X to cough.` / `X's right eye swells suddenly` | `crit_type_tracker.lic:713,724,734` | unbalance rank 1 (1 or 3) (`crit_tables.tsv:2217,2226,2235`) | PARTIAL |
| ucs_jab/back/6 (12, stun 2) `Blow to kidney!` | `crit_type_tracker.lic:2168` | ucs_punch/back/5 (25, stun 3) (`crit_tables.tsv:2133`); ucs_jab/back has ranks 0-5, 7-11 only | PARTIAL |
| plasma/abdomen/7 `X is sliced open neatly by brilliant beam of plasma!` | `crit_type_tracker.lic:1914` | plasma/chest/7 (`crit_tables.tsv:1208`) | PARTIAL (location) |

Value MEDIUM, no milestone: the `;combat` reports (`crates/cena/src/combat.rs`) read crit type
and rank, and `parse/damage.rs:68` takes `tables.parse(..).first()` with no weapon damage type to
break the tie. A crush weapon's rank-1 neck crit is recorded as an unbalance rank-3 with a stun.

**A first sentence that matches another rank's pattern (INFERRED impact).** Crit messages arrive
one sentence per line (VERIFIED for one crit, `arch_kill.xml:9-10`), Cena's patterns are anchored
`^`, and the lookahead takes the **first** of three lines that matches anything
(`crates/cena-model/src/state/combat/parse/damage.rs:61-79`). Splitting the gswiki texts in
`crittracker.lic` into sentences and replaying that rule gives 10 rows whose first sentence is
claimed by a different row (`tools/2-combat/sentences.py`):

| gswiki row (`crittracker.lic`) | first sentence | Cena row that claims it | effect |
|---|---|---|---|
| :429 slash/back/8, 60, **fatal** | `Slash to the X's lower back!` | slash/back/3, 15, not fatal (`crit_tables.tsv:1428` `^Slash (?:to\|along) .*? lower back.`) | a fatal crit recorded as rank 3 |
| :425 slash/back/4, 20 | same | slash/back/3, 15 | rank 4 read as 3 |
| :435 slash/right_arm/3, 8 | `Slash to the X's right arm!` | slash/right_arm/5, 15 (`:1450` `^(?:Deep s\|S)lash to .*? right (?:forearm\|arm).`) | rank 3 read as 5 |
| :445 slash/left_arm/2, 7 | `Slash to the X's shield arm!` | slash/left_hand/1, 1 (`:1456`) | wrong location and rank |
| :457 slash/right_hand/3, 5 | `Quick flick at its weapon hand!` (2nd sentence) | slash/right_hand/5, 8 (`:1470`) | rank 3 read as 5 |
| :385,:396 slash/{right,left}_eye/8, 45 | `Horrifying slash to the X's head!` | slash/head/8, 40 (`:1373`) | location |
| :75 crush/neck/1; :315,:326 puncture hands/7 | see collisions above | | |

The same first sentences appear as the *whole* pattern in `crit-tracking.lic:1972,1982,1992,2006`
and `calcredux.lic:986,1000,1008`, so three independent authors read those lines as the
start of those ranks. Whether the rank-3 and rank-5 alternations in Cena (`(?:Deep s|S)lash`,
`Slash (?:to|along)`) were measured on the wire is for the author: settling it needs the corpus.
Verdict CONFLICT, value MEDIUM (M6's `;combat` numbers; no behavior depends on crit rank).

**A creature name inside a pattern.** `crit_tables.tsv:28` (acid/left_eye/6, damage 30, **fatal**; there is no
right_eye/6 row, so it serves both eyes) is
`^Acid hits the triton assassin full in the eye.  Nothing left but a smoking socket.` It matches
only when the target is a triton assassin, so this fatal crit on any other creature records no
crit and no death from the crit. `crit_type_tracker.lic:1607` has
`/Acid hits the .*? full in the eye!/`. CONFLICT, value MEDIUM. The error is upstream:
`reference/lich-5/lib/gemstone/critranks/acid_critical_table.rb:533` has the same literal.

**Messages no crit table in Cena holds** (all `crit_type_tracker.lic`, 2013):
- Lightning "bonus" (nerves) lines with no Cena row: `You get a sharp whiff of burning hair.` (:1567),
  `Heavy shock sends X to the ground with convulsions.` (:1572, 20, stun 7),
  `Powerful blast sends X up in smoke.` (:1574, 80, fatal). Cena's lightning/nerves has ranks 1-9
  (`crit_tables.tsv`, `awk '$2=="nerves"'`). GAP, LOW.
- Spell results that are not crits, compared in the Captures table below: Bone Shatter's kill line
  (:2744), Limb Disruption's four "snaps in pain" breaks (:2750-2753), Pain (711)'s six tiers
  (:2759-2764).

## Special ask 3 (part): flares, from `flare_patterns.rb`

`lib/flare_patterns.rb` is not a `.lic` and so in no pile; `flaretracker_2.lic:128-154` and
`flarewindow.lic:731-733` load it. It is the most current flare table in the mirror (changelog
1.0.0 2024-09-07 to 1.7.3 2026-06-21, :6-37; no author or license line; `flarewindow.lic:46`
"Author: Codex, ClaudeCode, Phocosoen"). Three hashes: `NODMGFLARE_PATTERNS` (:49-199, 148
entries), `DMGFLARE_PATTERNS` (:201-358, 154), `ATTACK_PATTERNS` (:361-363). Many entries are
alternations of several messages (Fatal Afflares :338 has 47, Blessed Standard Reckoning :335 has 39).

Each alternative was tested against every pattern in `combat_attacks.tsv`, `combat_effects.tsv`,
`combat_results.tsv` and `crit_tables.tsv` three ways: its sample against Cena's regex, Cena's
samples against its regex, and its longest literal run (first or last 28 characters) inside
Cena's pattern text (`tools/2-combat/cenamatch.py`, `flarecmp.py`). Result, placeholders excluded:

| hash | alternatives | no Cena pattern | entries not fully covered |
|---|---|---|---|
| damaging | 324 | 244 | 110 of 154 |
| non-damaging | 141 | 109 | 105 of 148 |

Spot check of the automated verdict: 18 distinctive fragments grepped in the three combat TSVs
(`grep -c -i -- "<fragment>" combat_*.tsv`); 15 absent (e.g. `to the temple causes`,
`scorching blast of golden fire`, `white-blue light`, `Afflicted by your`, `ignites anew`,
`massive ball of flames`, `minuscule blue-white star`), 3 present and then caught by the literal
test (`alchemical fire`, `blast of psychic energy`, `psychic power unravel`). Some residual false
GAPs remain possible where the script's text is truncated mid-phrase.

What Cena has: the standard flare of every element (acid, air, cold, fire, lightning, plasma,
steam, vacuum, water, grapple, impact, unbalance, disruption, disintegration, magma, dispel), the
GEFs, the lore *flourishes* (`*_flourish`), sonic, holy fire and water, briar, somnis, blink,
acuity, and more: 153 `flare` rows (`cut -f1 combat_effects.tsv | sort | uniq -c`).

What it lacks, damaging (`flare_patterns.rb` line, missing alternatives in brackets):
- **Custom and festival messaging for the standard flares**: Fatal Afflares :338 (47), Blessed
  Standard Reckoning :335 (37), Quinton Manse :339 (11), and the second-to-fourth alternatives of
  Acid :203 (3), Fire :234 (3), Vacuum :320 (3), Cold :216 (2), Lightning :251 (2), Impact :248 (2),
  Grapple :241 (2), Magma :256 (2), Unbalance :319 (2), Air, Steam, Water, Disruption,
  Disintegration, Dispel (1 each). A character with a customised flare scores none of its flares.
- **Lore-flare damage-over-time repeats**: Air :206, Earth :231, Fire :236, Religion :289,
  Necromancy :269, Telepathy :312, Manipulation :254-255, Demonology :224-225, Summoning :310.
- **Purified metals** (2026): Pure Drakar :277-278, Eonake :279, Faewood :280, Gornar :281-282,
  Rhimar :283-284, Zorchar :285-286, Sephwir :287, Coraesine :219-220.
- **Weapon and item scripts**: Knockout :249 (10), Greater Black Ora :242 (6), Greater Rhimar
  :243, Globus :239-240, Mana flare :257, Nerve :270, Solar :299 and Nebular :267 weapons,
  Shadowdeath :293-294, Day/Nightbringer :221-223, Chain spear :213, Chronomage dagger :214,
  Mechanical quiver :259, Spirit gauntlet :303, Spore :304-305, Tomes :313-315, Twisted :318,
  Valence :321-322, Transformation lore :316 (7), Phytomorphic :274, Parasitic :275.
- **Spell side effects**: Smite (302) infusion and instant death :297-298, minor elementals
  901/903/904 :261-264, Webbing catches fire :328, Wall of Entropy 603 :329, Elemental Link 1117
  :332-333, Mage Armor fire :334, Ball-spell splash :210, Balefire demon :209, Spirit Slayer 240 :306.
- **Covert Arts poisons** :340-344 (`Afflicted by your X reels as the <colour> poison does its work`).
- **Gemstone properties** :346-355 (Blood Boil, Burning Blood, Ether Flux, Charged Presence (4),
  five Thorns).

Non-damaging (defensive procs, lore benefits, ensorcell, armor augmentations): 105 entries, e.g.
Ensorcell health/mana/spirit/stamina/AS-CS :124-128, Troll heart :60, Kroderine :111-112,
Sidestep :86 (a 2025 rogue feat), Mana armor :103-104, Forest armor :115-118, Cursed armor
:142-144. Cena's `flare` family does carry non-damaging rows (`damaging=0`, e.g. somnis, blink),
so these are in its scope.

Verdict GAP, value **HIGH (M6)**: the `;combat` reports built at M6 (`plan/34` Stage 4,
`crates/cena/src/combat.rs`) attribute damage by flare, and a missed flare's damage lands on the
swing or nowhere. The texts are game messages (facts); `flare_patterns.rb` has no license line.
Lich's own mechanism for adding patterns, `reference/lich-5/lib/gemstone/combat/defs/supplements.rb`
(player YAML), is deliberately not ported and left as an author question
(`crates/cena-model/src/state/combat.rs:94-96`); these rows would go into the shipped TSV instead.

## Special ask 3 (rest): maneuvers, warcries, weapon techniques, UCS

- **Maneuvers.** `osacombat.lic:2791-2831` lists 79 attack options. Cena has an attack
  definition for most of them, but none for 29: the warcries (bellow, growl, cry), chastise,
  crowd press, rain of thorns, dislodge, excoriate, exsanguinate, the six rogue cheapshots, leap
  attack, the three light-wing attacks, mighty blow, trample, pound, shield trample, spin attack,
  spell cleave, staggering blow, true strike, vault kick and voln sleep. Lich's `combat/defs`
  has none of them either (`grep -n -i "bellow\|yowlp\|holler\|footstomp\|eyepoke"
  lich-5/lib/gemstone/combat/defs/*.rb` finds only two spirit-animal flares). The pile holds
  **names** for these, not attack messages. The two message fragments it has are Seanette's
  Shout `let loose an echoing shout` (`osacombat.lic:3961`) and Horland's Holler `a thundering
  holler` / `holler your war cry` (`:3975-3979`). So the texts must come from elsewhere.
- **Cooldowns.** The maneuver-ready line (Top finding 1). The Mug outcomes `You feel like you
  could try that again on X!` / `The X won't fall for that again.` (`mug.lic`, tail) are GAP.
- **Weapon techniques.** The reaction trigger `You could use this opportunity to X!` (Captures)
  is GAP. Assault techniques (barrage, flurry, fury, guardant thrusts, pummel, thrash) are HAVE:
  `osacombat.lic:2025-2110` reply patterns match Cena's `assault` family
  (`combat_effects.tsv:324`). `Distracted, you hesitate` is HAVE. `Pummel may not be activated
  within` is GAP.
- **UCS.** HAVE: `excellent positioning`, `followup <jab|punch|grapple|kick>`
  (`osacombat.lic:1059-1072`, `smartwiz.lic:297-336`) match `combat_effects.tsv:349-351`
  (`ucs position` and `tierup`). The UCS crit tables are all in Cena, and `IC_Crits.lic`'s
  crit-tail phrases sit inside those rows (9 of 12 of its regexes match). Nothing new for UCS.

## Data tables Cena lacks or differs on
| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| `crit-tracking.lic:2217-2235` | crit divisor by armor and **body location** (17 armors x 9 locations) | 17 | chest, abdomen, back, arm, hand, leg, head, eye, neck divisors | `crates/cena-model/src/state/armaments.rs:173-180` `Armor::coverage`: sub-groups 1-4 Torso, 5-8 TorsoAndArms, 9-12 +Legs, 13-20 +Head | CONFLICT: Cena's coverage disagrees with **its own source**, `lich-5/.../armor_stats.rb:373-376` (torso = ASG 1,5,9,13,17; +arms = 6,10,14,18; +legs = 7,11,15,19; all = 2,8,12,16,20). The script agrees with Lich: Light Leather (ASG 5) arms/legs/head at 5, Full Leather (6) arms at 6, Reinforced (7) legs at 6, Double (8) all at 6. The test `tests/armaments.rs:142-153` checks ASG 1 and 20 only, which both bucketings give the same answer. The script also gives an uncovered location the **next lower group's** divisor (chain mail's arms 7, plate's arms 9); `qrs.lic:15702-15738` (2012) states the same rule in words ("Areas of the body not covered are assessed as the next lower Armor Group") and tabulates the same T/TA/TAL/TALH coverage by ASG, so three sources agree on coverage against Cena. (qrs's divisors for cloth and leather, 3 and 5, are older than Lich's 5 and 6, which `crit-tracking.lic` also uses.) Cena has no per-location divisor | MEDIUM (crit and redux maths; nothing reads `coverage()` today: `grep -rn "coverage()" crates` hits only the test) |
| `crit-tracking.lic:905,924,984,1008,1471` | weapon AvD by ASG | 5 cells | AvD | `weapons.tsv:61,66,60,65,13` = `lich-5/.../weapon_stats_polearm.rb:57-59,112-114,46-48,101-103`, `weapon_stats_brawling.rb:92` | CONFLICT. Cena (from Lich): halberd ASG 5-8 `30,29,28,17`; naginata `50,49,48,27`; awl-pike ASG 9-12 `35,33,21,29`; lance ASG 13-16 `53,49,45,31`; hook-knife ASG 9-12 `18,16,13,12`. The script: 27, 47, 31, 41, 14. Every other run in these rows steps by a constant (-1 leather, -2 scale, -4 chain, -6 plate). A second independent table, `weapons.lic:47,48,51,87` (Pukk, from Psinet's `?weapons`), agrees with the script on all four it lists (halberd 27, awl-pike 31, lance 41, hook-knife 14), and on troll-claw ASG 1 = 45 against Cena's 46 (`weapons.lic:95`, `crit-tracking.lic:1578`). So Cena carries five upstream typos from Lich (VERIFIED against two sources) | LOW (MEDIUM if an AvD calculator is ever built) |
| `crit-tracking.lic:707-722`, `calcredux.lic:278,405,531,657` | backsword damage factor | 1 | DF by armor group | `weapons.tsv:24` `\|0.31\|0.225\|0.24\|0.125\|0.15` (= `weapon_stats_edged.rb:57`) | CONFLICT. Two independent scripts give 0.44 / 0.31 / 0.225 / 0.24 / 0.15; Cena's row is those values shifted one group left with 0.125 inserted. `weapons.tsv:71` **bola** carries the identical five values (`weapon_stats_thrown.rb:46`), while `crit-tracking.lic:1677-1690` gives bola 0.205 / 0.158 / 0.107 / 0.118 / 0.067: a copied row in Lich (INFERRED) | LOW |
| `crit-tracking.lic:2192-2215` | crit weighting: raw crit value to the lowest and highest rank it can yield | 10 + 9 | lowest, highest rank | none: `grep -rniE "weighting\|randomi[sz]" crates --include=*.rs` finds only PSM names | GAP | LOW |
| `calcredux.lic:199-830` | DF by weapon for skin/leather/scale/chain/plate, 125 names with aliases | ~620 cells | DF | `weapons.tsv` (96 weapons x 5 groups) | HAVE for 91 of 117 mapped weapons; 26 differ. calcredux is older: its brawling DFs (cestus 0.15, razorpaw 0.15, paingrip 0.15 cloth) are well below Cena's (0.25, 0.275, 0.225), consistent with the later brawling revision (INFERRED). Its backsword agrees with crit-tracking (above) | none beyond backsword |
| `calcredux.lic:832-1170` | **incoming** crit messages (second person, "your") with damage | 299 | text, damage | `crit_tables.tsv` (patterns use `.*?` for the owner, so "your" matches) | HAVE: 193 same damage; 97 are first-sentence fragments that Cena anchors on the second sentence; 9 differ, all rows already listed above (collisions, first-sentence rank) | none new |
| `as_audit.lic:86-256` | AS bonus by spell, with lore/rank scaling formulas (117, 211, 215, 307, 425, 509, 606, 1007, 1107, 1109, 1130, 1606, 1611, 1617) | 14 | num, styles, bonus formula | `spells.tsv` column `bonuses` (e.g. 215 `physical-as 25+(Skills.slblessings/10)`, 307 the Cleric-rank formula) | HAVE | none |
| `as_audit.lic:696-730` | AS from society powers: Voln Courage +rank (max 26), CoL Striking +5 / Smiting +10 / Swords +20, GoS Offense +rank (max 20) | 5 | rank gate, amount | `crates/cena-model/src/state/societies/col.rs:174-240` (the signs, typed `AbilityKind::Offense`, no amount) | PARTIAL | LOW |
| `as_audit.lic:2001-2150` | AS arithmetic: stat enhancive base points to AS by style; CMAN bonus to AS (/2 melee, /4 thrown); health penalty 10/20/30 below 75/50/25%; spirit penalty 20/35/50; stance factor 1.0 to 0.5; Surge of Strength 6+2r (3+r thrown/UAF); Elemental Targeting `min(25+max((r-25)/2,0),50)` (:689) | 7 formulas | | none (`grep -rniE "stance_factor\|health_penalty\|surge" crates --include=*.rs` finds none); skill rank to bonus is HAVE at `crates/cena-model/src/state/character/spellsong.rs:317` `to_bonus` | GAP | LOW |
| `as_audit.lic:1091-1094` | Tactician gemstone AS: Journeyman rank r, Master r+5 | 2 | | none | GAP | LOW |
| `goalstracker2.lic:1150-1560` (Zedarius 0.4.1, fork of Morvik's `goalstracker.lic` 0.4.0) | training cost PTP/MTP per profession per skill; max ranks per profession; ascension skill caps and subcategory; spell-circle and sublore groupings; cost-tier rule (1x ranks 0-100, 2x 101-201, :27-29) | 10 prof x 46 skills, 2 tables; ~80 ascension rows | cost, max, subcategory | `crates/cena-model/src/state/character/skills.rs` (46 skills, ranks and bonus only); `psm.rs:316-344` `AscensionTable` reads the game's list | GAP | LOW (no milestone) |
| `training-buddy.lic:32-65,729-823` (Dreaven/Tgo01, v3) | stance modifiers (shield, block, AS, parry bonus, one- and two-hand parry) for the six stances; shield size modifiers (melee, ranged, ranged bonus); block and parry formulas (cap 60%, scaled by `20 / attacker level`); profession prime stats | 6 + 4 rows, 2 formulas | | `crates/cena-model/src/state/character/stance.rs:104` `band()` (percent bands only); `shields.tsv` `size_modifier`, `evade_modifier` | PARTIAL (shield sizes HAVE; stance modifiers and block/parry maths GAP) | LOW |
| `qrs.lic:16003-16070` (Gahread/Parafulmine, 2012; tail) | bolt spells: AvD by ASG, and DF by armor group with splash and lore | 19 spells x 17 ASG; 19 x 5 DF | AvD, DF, splash, lore | none: `cut -f1 weapons.tsv \| sort -u` has no bolt category; `spells.tsv` has no AvD | GAP (2012 data, INFERRED stale after the spell revisions) | LOW |
| `wood.lic:19-41` (Pukk; tail) | wood materials: usable as staff, shield, bow, arrow; bonus | 22 | 5 | none (`grep -rli carmiln crates` hits only creature attack text) | GAP | LOW |
| `feint_calc.lic:44-51` (Gnomad 0.5; tail) | feint result: RT = clamp((endroll-100)/6, 3, 8); stance change = min(endroll-100, 100) points | 2 formulas | | none | GAP | LOW |
| `osacombat.lic:3097-3103` (Peggyanne 4.3.0, 2026-09-20) | room-effect spells and how to see them in the room: 335 `miniature storm of faintly glowing snowflakes`, 610 plant nouns, 709 arm nouns, 710 `tempest`, 118 `web` | 5 | spell, room-object regex | none: `grep -rli "faintly glowing snowflakes" crates` empty; Cena's `room.rs` holds the objects but no spell-to-object table | GAP | MEDIUM (M6 hunt: "do not recast a room spell that is up") |
| `osacombat.lic:2791-2831` | the combat-maneuver vocabulary a hunt routine offers (79 names) | 79 | name | `combat_attacks.tsv` names | PARTIAL: no attack definition for bellow, growl, cry (warcries), chastise, crowd press, rain of thorns, dislodge, excoriate, exsanguinate, the six cheapshots (eyepoke, footstomp, kneebash, nosetweak, templeshot, throatchop), leap attack, the three light-wing attacks, mighty blow, trample, pound, shield trample, spin attack, spell cleave, staggering blow, true strike, vault kick, voln sleep (loop over the names against `cut -f2 combat_attacks.tsv`) | MEDIUM (M6 `;combat`) |

## Captures Cena lacks or differs on
| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| `Combatical.lic:4553-4556` (Kyrandos 1.3.1, 2026-02); also `keepsurge.lic:9`, `habitual_side_stepper.lic:35`, `Ashborne.lic:2731`, `rogue.lic:1615` (other piles) | a maneuver's cooldown has **ended** | `/^(\w[\w\s]+?) is ready for use\.$/`, e.g. `Surge of Strength is ready for use.` | `crates/cena-model/src/state/maneuvers.rs:41`: "**That it has ENDED.** Nothing says so." **Refuted by Cena's own fixture**: `crates/cena-behavior/tests/fixtures/smithy_engage.xml:271` is `Volley is ready for use.` (VERIFIED). The same fixture's line 270 is a `<dialogData id='Cooldowns'>` bar with `time='00:01:26'`, against `maneuvers.rs:34` "the game sends no other statement of one" (that dialog is read by `effects.rs:418`) | CONFLICT | HIGH (M6: `plan/33:109` guard `cooldown "<name>"`) |
| `osa_attack.lic:58,61`; `osa_attack.lic:29`; `smartwiz.lic:341,365` (Yasutoshi 2.1.0) | an attack the game refused | `You currently have no valid target.  You will need to specify one.` / `You do not currently have a target.` / `It looks like somebody already did the job for you.` / `You spin about but don't see anything to hit!` | none: `grep -rl -i --include=*.rs --include=*.tsv -e "<each>" crates` empty for all four. The hunt's pre-send check is `Gate::Act` (`crates/cena-session/src/command/verdict.rs:349-356`), which cannot see a kill that lands in flight. Lich has the second one in `psms.rb:213` `FAILURES_REGEXES`, whose port `maneuvers.rs:5-10` defers to M6 | GAP | HIGH (M6 hunt engine) |
| `reflex.lic:20-25`, `reactive.lic:15-16`, `weaponreact.lic:23-31`, `psm3weapon.lic:15-17` (tail), `osacombat.lic:6883` | a **reactive weapon technique** is available, and the command it wants | `^You could use this opportunity to ([\w\s]+)!$` then `weapon <name with spaces removed, lowercased>`; `weaponreact.lic:28` sends `Overpower` even in roundtime; `:30` sends nothing while Berserk (9607) is active | none: `grep -rl -i "could use this opportunity" crates` empty; `plan/30`, `plan/33` do not mention reactions (`grep -n -i "reaction\|opportunity"`) | GAP | MEDIUM (M6 hunt, weapon users) |
| `pain.lic:14-39`, `crit_type_tracker.lic:2759-2764` | Pain (711) result tiers, with damage share and RT | `cringes in pain.` 20% 0s; `shudders in pain.` 23% 0s; `twists in great pain!` 26% 3s; `is wracked with painful spasms!` 29% 4s; `shudders and twists in intense pain!` 32% 6s; `contorts in excruciating agony!` 35% 7s | `combat_attacks.tsv:62` `attack pain` has only `twists in great pain!` (= `lich-5/.../defs/spells.rb:94`) | PARTIAL (5 of 6 missing) | MEDIUM (M6 `;combat`) |
| `crit_type_tracker.lic:2750-2753` | Limb Disruption (708) breaking, not amputating | `X's right arm snaps in pain.` (arm/leg, right/left) | `combat_attacks.tsv:125` has only `The X's (right\|left) (leg\|arm\|hand\|eye) explodes!` | PARTIAL | LOW |
| `crit_type_tracker.lic:2744`, `flare_patterns.rb:151` | Bone Shatter (1106) killing blow | `The internal skeletal structure of X implodes inward upon itself, leaving behind no support for (his\|her\|its) body or life.` | 1106's cast (`combat_attacks.tsv:79-80`) and the pearlescent result (`combat_results.tsv:79`) are there; the kill line is not (`grep -c "implodes inward" combat_*.tsv` = 0) | GAP | MEDIUM (a death the tracker does not see) |
| `flare_patterns.rb:203-326`, `flaretracker.lic:719,726` | weapon flares on items with **plural** nouns, and without a target | `\*\* Your .*? release(s)? a blast of vibrating energy`, `expel(s)? a glob of molten magma`, `shoot(s)? a blast of water`, `flare(s)? with a burst of flame`, `emit(s)? a searing bolt of lightning` | `combat_effects.tsv` fixes one verb form per flare: impact, magma, water only plural **and** `at the <target>` (`\*\* Your .+? expel a glob of molten magma at the (?<target>[^!]+)! \*\*`, = `lich-5/.../defs/flares.rb:136,142`); fire, lightning, disruption, disintegration, grapple, air, unbalance only singular. Acid, steam, terror and spring rod carry both, so both forms exist on the wire (INFERRED from Cena's own rows) | CONFLICT | MEDIUM (M6 `;combat`) |
| `tracking.lic:98-107` (**Nisugi**, the author, v1.1.0 alpha) | Resist Nature / ranger trinket partial and full negation | `Your X pulses briefly, deflecting some of the (burning\|frozen\|electrical\|natural\|scalding) damage!` / `The (burning\|...) essence is drawn toward your X, where it is consumed with an abrupt burst of light!` | none: `grep -rn "pulses briefly\|essence is drawn" lich-5/lib/gemstone/combat` and Cena TSVs empty | GAP | MEDIUM |
| `tracking.lic:131-135`, `flare_patterns.rb:124-128`, `flaretracker.lic:697-701` | Ensorcell procs | `You feel healed!` / `empowered!` / `rejuvenated!` / `reinvigorated!` / `energized!` | none (same greps) | GAP | MEDIUM |
| `tracking.lic:118` | a spell empowered from camouflage | `Emerging from the shadows, you draw upon the environs to empower your spell!` | none | GAP | LOW |
| `tracking.lic:96`; `tracking.lic:69` | first-person dodge; missile block variant | `You deftly dodge along the ground, avoiding the onslaught!`; `X tumbles to the side and blocks the missile with Y!` | `combat_results.tsv` has 55 evade rows, not this one; Lich `defs/outcomes.rb:211` has `tumbles to the side and **deflects**` only | PARTIAL | LOW |
| `lurkcheck.lic:32-40` (Lavastene 1.1) | infested by a shambling lurk (fatal without the vat) | `The last of the noxious spittle evaporates from your flesh.` / `Hot and cold wracks your body.` / `Your vision doubles abruptly, then shivers back into place.` / `You fight back the urge to vomit as the world spins around you.` / `Intense nausea tosses your stomach as a headache, intense as a thunderclap, blinds you for a moment.` | none (`grep -rl -i` for each fragment over crates) | GAP | MEDIUM (M6 rest reasons / M6d healing) |
| `as_audit.lic:779-784,1138,1290,1393,1848-1934` (RuseofFools, 1.0.2) | item properties from `recall` and `analyze` | `It provides a boost of (\d+) to (.+?) (Ranks\|Bonus)\.`, `It helps to increase the efficiency of attacks made by the wearer with a bonus of (\d+)\.`, `It may be enchanted up to a bonus of (\d+) by a wizard`, `It has been sanctified \d+ times\..*\+(\d+) AS bonus against the undead\.`, `It is weighted to inflict more critical wounds`, `It has permanent Holy Fire flares\.`, `It is a Blink Weapon\.`, `This is tier (\d+) Mana-Infused Armor` | none: `grep -rl -i -e "As you recall" -e "enhancive item:" crates` empty | GAP | MEDIUM (M6c keep/sell decisions; pile 5 likely has more) |
| `smartwiz.lic:370` | dying with a favor owed | `You mentally give a sigh of relief as you remember that the Goddess Lorminstra owes you a favor.` | none | GAP | LOW |
| `cman_calc.lic:72` | feint outcome | `bounces around distracted by the pain` | none | GAP | LOW |
| `ragetracker.lic:175-207` | Rage Armor stages | `A burning rage awakens within you` ... `The burning rage abates` | none | GAP | LOW |
| `nerve.lic:28-32,88-104` | runestaff affinity and its Muscle Memory CMAN bonus | `The X runestaff shares a (slight\|partial\|moderate\|considerable\|complete) Affinity with you!  This will result in a bonus of (5\|6\|7\|8\|10) to Combat Maneuver Ranks ...` | none | GAP | LOW |
| `ebsteinnosuicide.lic:13-35` | death announcements (the `death` stream) | `X just bit the dust`, `X has gone to feed the fishes`, `X is dust in the wind`, 12 forms | `crates/cena-ui/src/merge.rs:28` merges the `death` stream but nothing classifies who died (`grep -rli "bit the dust" crates` empty) | GAP | LOW |
| `goalstracker2.lic:295-326,458-463` | `asc info` and `skills base` report lines | `Absorption Rate:\s*(\d+)%`, `ATPs Available:\s*(\d+)`, `\((\d+) Mnt converted to Phy\)` | `experience_report.rs:10,50` reads `Ascension Exp:`; `tests/character_skills.rs:235` has Phy-to-Mnt only | PARTIAL | LOW |
| `trackbless.lic:31,143`; `smartwiz.lic:252,270,381-399` | bless on weapons and UAC gear | `You X, but it has no effect!`, `A wave of power flows outward from you towards`, `Your X stops glowing.`, `A faint aura of holy light radiates` | `spells.tsv:161` has the `returns to normal` end line only | PARTIAL | LOW |
| `arena-*.lic`, `midnightarena.lic:17-63`, `duskruin_arenatimer.lic:42-86`, `dr_timer.lic`, `arenatimer_old.lic`, `arenasquat.lic:38-43` | Duskruin and Midnight arena: announcer, round and dodge prompts | `you could try to (roll out of the way\|bob back and forth\|lean out of the way\|back pedal\|prepare to jump\|duck down)`, `An announcer shouts, "FIGHT!"` | none | GAP | LOW (no milestone; events are pile 8's) |
| `osacrewv3.lic`, `osacrewv2.lic`, `damagecontrol.lic`, `osacombat*.lic:4167`, `osamedicalofficer.lic`, `boatswainsmate.lic` | OSA ship crew: sails, capstan, anchor, damage control, crew whispers | `You grab a hold of the main line of the (.*)sail ...`, `Main Deck:  It appears to be`, `You do not currently have a task from the Sea Hag's Roost` | none | GAP | LOW |
| `warrior.lic` (Dreaven/Tgo01 v158), `twarrior.lic` (Tgo01 v87) | Warrior guild tasks (GLD) | `The Training Administrator told you to ...` (about 20 task texts), `\[You have \d+ repetition\(s\) remaining\.\]`, `You need to give your vocal cords a bit of a rest!` | none (`grep -rli "Training Administrator" crates` empty) | GAP | LOW |
| `autoforage.lic:79-124`, `zzherb*.lic:223-246`, `sortforage.lic:44` | foraging results | `You forage briefly and manage to find`, `As you forage around you suddenly feel a sharp pain in your right hand!`, `Glancing about, you notice the immediate area should support specimens of` | `crates/cena-model/src/state/doses.rs`, `herbs.rs` (herb use, not foraging) | GAP | LOW |
| `osacombat.lic:1275-1278` | society power lost in an anti-magic room | `The power from your (sign\|sigil\|symbol) dissipates into the air.`, `Your magic fizzles ineffectually.` | none (`grep -rli "dissipates into the air" crates` hits only two creature death lines, `creature_messages.tsv:6108,6952`; `fizzles ineffectually` no hit) | GAP | MEDIUM (M6 hunt `Maintain` step, `hunt/engine.rs:24`) |

## Script by script
Depth: **D** read deeply (header, data blocks, capture logic); **C** capture-line depth (every
regex extracted by `tools/2-combat/scan.py` and tested against Cena; GAP lines reviewed by hand).

| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| `crit_type_tracker.lic` (D) | 2918 | Gibreficul 2013: names each crit you land | 2,483 regex to label rows, 17 crit types incl. UCS; 711/708/1106 spell results | Special ask 2. Cena current; 6 collision keys, triton-assassin pattern, 3 lightning nerve lines, pain tiers |
| `ctt_win.lic` (D) | 4088 | same table (2011) in a Wrayth window | 1,869 rows, no UCS | older variant of the above; the 2013 table dropped 44 of its patterns, most of them multi-sentence and so unable to match one line |
| `crit-tracking.lic` (D) | 2915 | Tgo01 v11: crit, damage and armor maths per swing | crush/puncture/slash crit table (390); weapon AvD/DF by 17 armors (1,222 cells); crit divisor by armor and body location; crit weighting ranges; creature wound appearance | coverage CONFLICT (Cena's own bug), 5 AvD typos, backsword/bola DF, crush/neck/1 |
| `calcredux.lic` (D) | 1299 | damage reduction (redux) from incoming hits | DF table (5 groups, ~125 names); incoming crit damages (299); formula `redux = 1 - (taken - crit)/(DF x (endroll-100))` | DF older than Cena except backsword; formula GAP LOW |
| `osacombat.lic` (D) | 6927 | Peggyanne 4.3.0 (2026-09-20): automated combat, bigshot-like | attack vocabulary (79), stances, society buffs, anti-magic lines, room-effect spells, UAC chain, hurl recovery, warcry replies | maneuver defs PARTIAL; anti-magic and room-effect GAP MEDIUM (M6) |
| `osacombat_avalon_wizard.lic`, `osacombatv2.lic` (C) | 3350, 961 | older OSACombat builds | subsets of the above plus TARGET/discern lines | variants; nothing new beyond `You discern that you are the origin of` (target command replies, GAP LOW) |
| `Combatical.lic` (D, capture lines) | 4624 | Kyrandos 1.3.1 (2026): ability bar with RT and cooldown tracking | `X is ready for use.`, RT lines, enemy attack verbs | cooldown-ended CONFLICT with `maneuvers.rs:41` (HIGH) |
| `as_audit.lic` (D) | 2343 | RuseofFools 1.0.2: explains your AS | spell AS rules (14), society AS, stat/CMAN/penalty/stance formulas, recall property lines | spells HAVE; formulas and recall lines GAP |
| `training-buddy.lic` (D) | 898 | Dreaven v3: what training buys (AP, block, parry) | stance modifier table, shield sizes, block/parry formulas | PARTIAL, LOW |
| `goalstracker2.lic` (D), `goalstracker.lic` (C) | 1953, 1931 | Zedarius 0.4.1 fork of Morvik 0.4.0: training goals | profession skill costs, max ranks, ascension caps; `asc info` lines | GAP LOW; the fork fixes cost-tier boundaries (:27-33), so it is the newer |
| `tracking.lic` (D) | 541 | **Nisugi** v1.1.0 alpha: the author's early event tracker | misses/evades/blocks (markup), my-evade and resist lines, ensorcell, flares | 76 of its regexes match Cena (`scan.py`); ranger-trinket resist/negate, ensorcell, camouflage cast GAP |
| `flare_patterns.rb` (D; not in any pile) | 363 | flare message table (2024-09 to 2026-06) | 303 entries, ~475 message alternatives | Special ask 3; 353 alternatives have no Cena pattern |
| `flarewindow.lic` (C) | 1278 | Phocosoen 1.0.0: flare window, uses `flare_patterns.rb` | loader, combo counting, non-damage outcome list | outcome list mostly HAVE; `with little effect`, `manages to dodge the licking flames`, `scoffs at the` GAP LOW |
| `flaretracker_2.lic`, `flaretracker.lic` (C) | 944, 952 | predecessors of flarewindow (2024) | 2: uses the .rb; 1: its own inline table | variants; the inline table's gaps are a subset of `flare_patterns.rb`'s; magma/impact verb-number CONFLICT seen here first (:719,:726) |
| `warrior.lic` (C), `twarrior.lic` (C) | 3811, 3518 | Dreaven/Tgo01 v158 and v87: Warrior guild task automation | GLD output, task texts, reps, warcry names | GAP LOW (no guild model); warrior.lic is the newer |
| `UberBarWiz.lic` (C) | 822 | Wizard UI bar (2022) | base64 images, exp labels | N/A |
| `rofl-trivia.lic` (C) | 577 | trivia bot | 249 question/answer strings | N/A |
| `osacrewv3.lic`, `osacrewv2.lic`, `damagecontrol.lic`, `osa_attack.lic`, `osamedicalofficer.lic`, `osa_migrate.lic`, `boatswainsmate.lic` (C) | 2150, 731, 752, 77, 143, 400, 183 | Open Seas Adventure ship crew | sails, capstan, repair, crew whispers, boarding creature arrivals (`osa_attack.lic:4-13`) | GAP LOW; `osa_attack.lic:29,58,61` attack refusals feed the HIGH row |
| `trackbless.lic` (C) | 251 | bless charges on weapons | bless cast and wear-off lines | PARTIAL LOW |
| `combat-window.lic`, `CombatWindow.lic` (C) | 762, 139 | combat lines into a window | cast verbs, hiding line | HAVE (`combat_attacks.tsv:274`, `tests/claim.rs:10`) |
| `sortedskills.lic` (C) | 214 | skills in trainer-spreadsheet order | skill names | HAVE (`crates/cena-model/src/state/character/skills.rs`) |
| `wood.lic` (D) | 330 | Pukk: wood materials | 22 woods x 5 | GAP LOW |
| `garrote.lic` (C) | 337 | Gwrawr: rogue garrote | refusal and result lines | `garrote` attack HAVE; failure texts (`garrote.lic:95,98`) GAP LOW |
| `autoforage.lic`, `forage.lic`, `zzherb.lic`, `zzherb2.lic`, `zzherb3.lic`, `sortforage.lic` (C) | 190, 253, 267, 277, 277, 64 | foraging | forage result and failure lines, forage sense | GAP LOW; zzherb2 is Cidven's fork of Zzentar's zzherb (kneeling, running from NPCs), zzherb3 differs from zzherb2 by 14 diff lines (`diff zzherb2.lic zzherb3.lic \| wc -l`) |
| `SpiritBeastTrack.lic` (C) | 327 | Deathravin 0.8: spirit beast battles | 15 battle lines | GAP LOW |
| `uacswap.lic` (C) | 68 | Hazado: swap blessed/unblessed UAC gear | none matched | N/A |
| `arena-ryjex.lic`, `arena-ryjex2.lic`, `arena-nylis.lic`, `arena-nylis2.lic`, `arena-daiyon.lic`, `arenasquat.lic`, `midnightarena.lic`, `duskruin_arenatimer.lic`, `dr_timer.lic`, `arenatimer_old.lic` (C) | 213-329 | Duskruin and Midnight arena | announcer, dodge prompts, winnings | GAP LOW; the ryjex, nylis and daiyon files are per-character variants of one script; `dr_timer.lic` and `duskruin_arenatimer.lic` differ by 11 diff lines |
| `fsmr.lic` (C) | 227 | Kaetel: reformats SMR/SSR lines | SMR/SSR | HAVE (`combat_results.tsv` `resolution smr`, `ssr`) |
| `smartwiz.lic` (D, capture lines) | 508 | Yasutoshi 2.1.0: UAC stancing | UCS tier-up, bless state, attack refusals | UCS HAVE (`combat_effects.tsv:349-351`); refusals GAP HIGH row |
| `attack.lic` (C) | 71 | Salasin: attack loop | stance-change replies (`You are now in an?`, `Cast Round Time in effect`) | HAVE by design: Cena confirms a stance from the `pbarStance` bar, not the sentence (`crates/cena-behavior/src/stance.rs:9-16`) |
| `playcount.lic` (C) | 279 | bard instrument practice log | `You feel a sense of accomplishment` | GAP LOW |
| `ragetracker.lic` (C) | 278 | Rage Armor tracker (2026) | 5 stage lines | GAP LOW |
| `sebp.lic` (C) | 198 | SpiffyJr: evade/block/parry counter | defensive outcomes | HAVE (`combat_results.tsv:71,179,184,89`) |
| `speechrandomizer.lic` (C) | 75 | speech tones | tone list | N/A |
| `webkill.lic` (C) | 36 | cast at webbed targets | web spells by profession | HAVE (`crates/cena-model/src/state/creature/status.rs:104` `Webbed`) |
| `IC_Crits.lic` (D) | 51 | Hazado: strips UCS crit tail jokes | UCS crit tail phrases | HAVE: 9 of 12 regexes match Cena UCS crit rows (`scan.py`); the tails are part of the same lines |
| `nerve.lic` (C) | 114 | nerve runestaff affinity | 5 affinity lines | GAP LOW |
| `saimtest.lic`, `suntil.lic`, `scalcmm.lic`, `sgrip.lic`, `scrit.lic` (C) | 59, 70, 56, 50, 96 | SpiffyJr utilities: aim stats, wait-until, UCS MM calc, grip swap, crit tracker | UCS attack, grip replies, crit wound tiers | HAVE (`combat_attacks.tsv:10`; `scrit` 11 of 12 match crit rows); grip replies GAP LOW |
| `unlearnforfixskills.lic` (C) | 126 | PSM unlearn helper | cman/shield/armor list rows | HAVE (`psm.rs:73`, generic PSM table) |
| `ebsteinnosuicide.lic` (C) | 40 | one character's death lines | 12 death forms | GAP LOW |
| `xtask.lic` (C) | 148 | rogue partner tasks | partner whispers | N/A |
| `charsnapshot.lic`, `restfull.lic`, `escapestom.lic` (C) | 135, 141, 89 | snapshot INFO/SKILLS; afk rest; leave a room | none new | N/A |
| `wymdi.lic`, `lurkcheck.lic` (C) | 27, 68 | a joke hook; lurk infestation alarm | 5 lurk lines | lurk GAP MEDIUM |
| `cman_calc.lic` (C) | 78 | feint result calculator (replaces `feint_calc.lic`) | feint and knockdown lines | HAVE 2, GAP 1 LOW |
| `gboost.lic`, `gexpboost.lic` (C) | 86, 86 | use an experience boost when the mind is full | `do not have any` | N/A (`diff gboost.lic gexpboost.lic` is 4 lines) |
| `pain.lic` (C) | 47 | Pain (711) tier report | 6 tiers | PARTIAL MEDIUM |
| `rnum2.lic` (C) | 40 | Xanlin: room id in the title | roomName style | HAVE (`crates/cena-protocol/src/text.rs:77`) |

## Tail
89 scripts under the capture+data threshold. All were name-scanned and their capture lines
grepped (`grep -hoE '(=~|waitfor|matchtimeout|matchfind|matchwait|dothistimeout|when) ...'`);
the ten whose names suggested data were opened.

- **Opened, with data or captures worth recording**: `qrs.lic` (17,159 lines, Gahread/Parafulmine
  2012; a quick-reference of puts'd tables: bolt AvD and DF, crit divisors and the coverage rule,
  armor penalties, weighting, mstrike, creature and herb lists; the rows used above are cited
  there; old, so INFERRED stale where spells were revised); `weapons.lic` (Pukk, Psinet's
  `?weapons`: weapon AvD by ASG, damage type, DU/ST, speed; confirms the five Lich AvD typos);
  `feint_calc.lic` (feint RT and stance formulas, above; superseded by `cman_calc.lic`);
  `reflex.lic`, `reactive.lic`, `weaponreact.lic`, `psm3weapon.lic`, `clobber_it.lic` (the
  reactive-technique line, above); `itemprop.lic` (Maze 2021: the weapon/armor enhancement slot
  categories A/B/C as text; GAP LOW, pile 5's domain); `legend.lic` (`A prismatic display of
  color tints the air around you and arcs away, heralding your discovery of a legendary
  treasure!`; `plan/34-loot-ledger.md:164` records `LEGENDARY_ITEM` as deliberately not
  recorded, so a decision, not a gap); `vocabulary.lic` (speech tones, N/A).
- **Small captures Cena lacks** (GAP LOW unless noted): `mug.lic` Mug outcomes `You feel like you
  could try that again on (.*)!` / `The (.*) won't fall for that again.` (MEDIUM for the ledger
  if mugging is a loot source); `flystaff.lic` `Your .*? tears free from your hands and floats
  threateningly in the air around you, darting about as if possessed!` (a weapon lost mid-hunt,
  MEDIUM, M6); `pummel.lic` `Pummel may not be activated within`; `keepshout.lic` `Your surge of
  empowerment fades.`; `shadowdeath.lic` vambrace hunger lines; `brooch.lic` charge lines;
  `ironmanplate.lic` platemail knob; `rats-are-dumb.lic` a sewer-rat familiar line; `rrboot.lic`
  a treasure-room description. (`grep -rl -i --include=*.rs --include=*.tsv` over `crates` is empty for each: `fall for that again`, `tears free from your hands`, `Pummel may not`, `surge of empowerment`, `finished a meal at a large banquet`, `as light as it can get`, `knob on the side of the platemail`, `sewer rat`, `equally fine treasures`.)
- **Stun and unstun helpers**: `unstun`, `unstun2`, `unstunner`, `unstunwand`, `emp-helper`,
  `paladin-helper`, `barkstun`, `stunning`, `stunningog`, `stunshout`, `stunzerk`, `berserk`,
  `timestop` (read the stun indicator or `are stunned for`; HAVE, `combat_effects.tsv` `status
  stunned`).
- **Attack and stance macros**: `xfire`, `shoot`, `electrocute`, `sniper`, `smash`, `cripple`,
  `ezambush`, `pull`, `kneel`, `kneeler`, `staystanding`, `sweep`, `oneshot`, `celerity`,
  `bigtap`, `902`, `702deolinda`, `vbless`, `eyeswitch` (a crit line HAVE at
  `crit_tables.tsv:1287`), `hp`, `dc`, `yelldamage` (damage line HAVE).
- **Experience and resting**: `instant`, `ltb`, `helsfeldpro`, `5hours`, `babytime`, `rejuv`.
- **UI, social, settings, one-liners**: `gentle`, `intel_gentle`, `mood`, `schizophrenia`,
  `stylebold`, `searchrepo`, `star-autoaccept`, `fullhands`, `givesafe`, `grgvars`,
  `newsettings`, `onstart`, `keepalive_new`, `foxfox`, `crystal`, `viscosity`, `borg`, `beast`,
  `bq` (OSA boarders), `juryrig` (OSA hull), `woahnele`, `drizzleback`, `gottago`, `kfatoll`,
  `bigquick`, `slavepen`, `foragewindow`, `ac-wizard`: N/A.

## Method
- Pile and threshold: `awk -F'\t' 'NR>1 && ($3+$4)>=5' pile-2-combat.tsv | wc -l` = 79 substantive
  of 168; lines total 80,836.
- Crit tables: `tools/2-combat/critcmp.py` parses the four crit scripts' tables (regex to label,
  `if line =~` to `cttmsg`, `'re' => {:r,:d,:l,:t}`, and `[rank, dmg, "msg", ...]` rows), builds a
  sample line per entry, and runs all 2,394 Cena patterns (Python `re`, named groups rewritten).
  `summary.py`, `diffs.py`, `keys.py`, `side.py` classify; `sentences.py` replays Cena's
  first-match lookahead over gswiki messages split into sentences.
- Weapons and DF: `weapcmp.py` (crit-tracking's 17-armor rows mapped to ASG 1, 5-20; DF by
  group), `calccrits.py` (calcredux crits and DF), against `weapons.tsv` via
  `armament_aliases.tsv`; disputed rows re-read in `lich-5/lib/gemstone/armaments/*.rb`.
- Every other regex: `scan.py <script>` pulls each `/.../` literal (markup tags stripped), and
  calls it HAVE if (a) its sample matches a Cena combat or crit pattern, (b) a Cena pattern's
  sample (alternations expanded) matches it, or (c) its longest literal run (first or last 28
  characters) is inside a Cena pattern's text; FRAG if a 14+ character literal appears in any
  `.rs` or `.tsv` under `crates/`; else GAP. All 74 remaining substantive scripts were scanned
  (`tools/2-combat/scan/*.out`) and every GAP line read by hand. The scanner also picks up UI
  strings between slashes; those were discarded.
- `flare_patterns.rb`: `flarecmp.py`, the same three tests per alternative.
- Absence claims were each re-checked with a plain grep, e.g.
  `grep -rl -i --include=*.rs --include=*.tsv -e "could use this opportunity" crates`,
  `grep -rn "pulses briefly\|essence is drawn" reference/lich-5/lib/gemstone/combat`,
  `grep -c -i -- "<fragment>" crates/cena-model/data/combat_*.tsv`; each is quoted beside its row.
- Not done: no search of the log archive (the brief forbids it); the corpus would settle the
  crit first-sentence question and the flare verb-number question.
