# 7-character-healing-crafting: lich_repo_mirror survey

Scope: 284 scripts, 176 substantive; 40 read deeply, 136 at capture-line depth, 108 in the tail by name
(about 45 of those opened or grepped for their messages). Cena HEAD `a6146e6` at the end of the survey (`git -C E:/Cena rev-parse --short HEAD`;
it was 3566f8d at the start, parallel sessions committed `067b52a` and `a6146e6`, the latter being plan/36 Stage 1:
`data/herbs.tsv`, `herbs.rs`, `state/doses.rs`). Uncommitted at the end: `crates/cena-behavior/src/heal/` (plan/36
Stage 2, 668 lines). 2026-09-25.

## Top findings

1. **plan/36's unbuilt stages have their exact line shapes here. GAP, HIGH, M6d.** The Survivalist's kit (Stage 3):
   `The kit contains DOSEs of the following solid herbs: acantha (N), ambrominas (N), ...` and `... TINCTUREs of the
   following liquid herbs: rose-marrow (N), bolmara (N), talneo (N), brostheras (N), wingstem (N), bur-clover (N).`
   (`healme.lic:35-36,288-289`, Alastir v1.2, no licence line; `distiller.lic`). The herbalist (Stage 4): menu rows
   `<d cmd='order N'>name</d>`, `You may order a QUANTITY of this item, ORDER something else, or BUY this item.`,
   `Sold for N silver`, `But you do not have enough silver!`, `Thanks for your patronage` (`useherbs.lic:443-452`,
   Tillmen v0.12; `nesherbalist.lic`, `sfillherbs.lic`). Searches over crates for `DOSEs of the following`,
   `TINCTURE`, `QUANTITY`, `patronage`: nothing. Cena's shop errands send a fixed order number
   (`travel/routines/sword_gorge.rs:52` `order 3`, `giant.rs:92` `order 11`); nothing reads a menu by name.
2. **The game's own statement that an injury blocks an action is not captured. GAP, HIGH, M6 hunt and M6d.**
   `You can't make that dextrous of a move!` (arms/hands), `You can't think clearly enough to prepare a spell!`
   (head/nerves), `You're not in any condition to be searching around!` (`health.lic:63-85`, Alastir). Cena has the
   rules (`character/injured.rs`) but not the lines; `grep dextrous\|think clearly enough` over crates finds only a
   spell message and a creature description; `loot/outcome.rs:80` matches `not in any condition` for searching only.
   The uncommitted heal reply classifier (`heal/reply.rs:26-43`) reads eat/drink replies, not these.
3. **The model drops the experience bar's attributes, and the plan says it reads them. PARTIAL/CONFLICT, MEDIUM.**
   Cena's own fixture has `<progressBar id='mindState' ... field_exp='0' max_field_exp='1324' ascension_exp='0'
   exp='135' until_next='2365' .../>` (`cena-protocol/tests/fixtures/login_burst_full.xml:3`). The frame keeps every
   attribute (`frame/payload.rs:228-235`), but `state/character.rs:460-466` passes only text and value, while
   `plan/27b-model-api-character.md:64` lists `<progressBar>` as a source of `field_experience` and `plan/18:68-69`
   records the attributes as VERIFIED. lich-5 reads all of them plus `fashlonae`, `lumnis` and `rpa`
   (`common/xmlparser.rb:725-740`, whose comment says `lumnis` and `rpa` are sent only while active); so does
   `exp_sagapanel.lic:957-1012`. A live `lumnis` flag is what `Gift`'s doc names as its upgrade trigger
   (`character.rs:173-177`).
4. **`weapons.tsv` carries six lich-5 typos that the wiki and character-planner both contradict. CONFLICT, MEDIUM.**
   Backsword DF (`weapons.tsv:24`: .31/.225/.24/.125/.15; wiki `Backsword.txt:13` and planner: .440/.310/.225/.240/.150);
   bola DF is the backsword row copied (`:71`, and lich-5 `weapon_stats_thrown.rb:46`); AvD typos: halberd ASG8 17
   vs 27 (`:61`), naginata ASG8 27 vs 47 (`:66`), awl-pike ASG11 21 vs 31 (`:60`), lance ASG16 31 vs 41 (`:65`).
   Also `shields.tsv:5` tower evade -0.5 vs 0.54 factor (`shield-size.lic:59`, `wiki_clean/Evade.txt:58-60`).
   Method: `tools/7-character-healing-crafting/weapons_cmp.py` (53 raw diffs over 32 weapons; the rest are planner
   errors the wiki refutes, or blanks).
5. **The `health` command is not read. GAP, MEDIUM, M6d.** Per-location bleed per round and `(tended)`
   (`smartheal.lic:722-730`), poison/disease `Taking N damage per round.  Dissipating N per round.`
   (`formatfix.lic:46`), `Maximum/Remaining ... Points`, damage reduction (`stats.lic:89-94`). Cena has only the
   `bleeding`, `poisoned`, `diseased` flags (`status.rs:199,231,233`).
6. **Other characters' wounds and scars: the 14x3 + 14x3 description table is a GAP, MEDIUM.** Its primary source
   is already in the repo (`reference/wiki_clean/Wound.txt:13-43`); scripts match it off `appraise`/`look`
   (`injurywatch.lic:290` has all wound and scar texts in one regex, Mystienne v0.4.2; `useherbs.lic:705-872`
   maps each to herb types). Needed only for healing someone else, which plan/36 leaves out (escort, deader) and M7
   might want. No "appears to be ..." ladder exists in the pile beyond `appears to be in good shape` and `has no
   apparent injuries`.
7. **Lumnis, Tutelage and RPA. GAP, MEDIUM.** `lumnis info` (`You have N points of doubled experience remaining on
   your Gift of Lumnis.  It is scheduled to refresh in ...`, `has expired for this week`, schedule, donations;
   `xp2.lic:2144-2203`, Mystienne v1.9.4), the end line `The soft feeling of serenity slowly dissipates from your
   mind.` and Tutelage start/end (`tutelog.lic:2875-2877`), and three Lumnis-active phrasings in `experience`
   (`xp2.lic:2057`, `microbar.lic`, `nesstats.lic`). Cena: `Gift { pulses }` only (`character.rs:144-182`).
8. **Experience and ascension extras. PARTIAL/GAP, MEDIUM.** Not in `experience_report.rs:28-60`: `Exp to next TP`,
   `Exp to next ATP`, `PTPs/MTPs: a/b`, `ATPs: n`, `Wisdom of the Ages for ...` (`xp2.lic:2039-2066`). `ascension
   exp` (`Absorption Rate: N%`, `ATPs Earned/Spent/Available`, `xp2.lic:2209-2252`): absent from Cena and lich-5.
   `asc list` prints `the following Ascension Abilities are available` with a Resist/Stat/Skill/Other/Regen column
   (`asclearn.lic:43-51`, elanthia-online v1.1.0), which bears on `psm.rs:336-339`'s UNVERIFIED note; the 75
   ascension mnemonics with max ranks are in `character-planner.lic:934-1028`.
9. **character-planner (special ask 2) holds rules data Cena has none of. GAP, MEDIUM, no milestone (M7 agent,
   M10 planner).** Training cost per profession and skill with ranks per level (370 rows: the per-level skill cap),
   profession growth indices and primes (10), race growth modifiers (13), race stat bonuses (13), CMAN point costs
   and professions (73), experience per level 0-100 (101), plus weapons (100), crits (1602, HAVE in richer form),
   armor crit divisors and 12 test critters. Dreaven/Tgo01, v10, no licence line. It agrees with the wiki where
   checked (`wiki_clean/Bard.txt:4`); `maxcap.lic`'s cost table is stale (29 of 360 rows differ, the wiki sides
   with the planner every time checked).
10. **Death recovery and deeds. GAP, MEDIUM, beside M6d.** Sting potion `750 + 150 x level` and the priestess
    replies (`zombie.lic:186-209`, Tillmen), CON death-penalty via `info` bold (`death_potions.lic:66-71`), deed cost
    `deeds^2 x 20 + gs3_level x 100 + 101` with GS3 bands and rubies (`deedcalc.lic:28-57`, elanthia-online v1.1.0),
    and the temple routines with success/failure lines (`gemdeeds.lic:170-224`, `deed.lic:159-207`), a natural fit
    for `cena-behavior/src/travel/routines/`. Cena has the sting level and deed count only.
11. **Herb tables (special ask 1): Cena's `herbs.tsv` is a superset of this pile's. HAVE, with small PARTIALs.**
    Of 160/175/199 rows in useherbs/useherbs_kf/herbmaster, missing from Cena: `tincture of moss`, 11 KF long names,
    20 FWI dish names; store_doses differ on 5 (eherbs 2.2.1 is newer); the 10 Aldoran healing stones / Mist Harbor
    gems (`herball2.lic:44-54`, `Herbhigh.lic`) are absent. **No table in the pile or in Cena has herb price or HP
    per bite**; the wiki has HP per bite (`wiki_clean/Acantha leaf.txt`).
12. **Crafting (special ask 4): Cena has none, confirmed. GAP, LOW, no milestone.** Only incidental hits
    (`gameobj-data.tsv:3-6,52` alchemy types, travel shop tags, `herbs.rs:203` "Alchemical" source). Tables that
    exist: `alchemy-recipes.lic` 883 rows (111 general, 87 potions, 114 trinkets with rank ranges, professions and
    spells; 139 buy with cost; 191 grind; 19 extract; 3 distill; 129 forage; 87 kill-for-reagent naming the creature),
    29 reagent-equivalence groups, 32 reagent values (Tillmen v0.11, 2019); fletching paints 25
    (`fletching.lic:55-81`); slat symbols ~56 (`slats.lic`, divination, not crafting). Forging, fishing and gardening
    scripts hold captures only (workshop rental, grinder quality, fish weight, 1118 herb growth). Cooking: no script.

## Data tables Cena lacks or differs on
| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| character-planner.lic:38-438 | training cost per profession and skill | 370 (10 professions x 37 skills) | PTP, MTP, max_ranks per level | none: `skills.rs` reads ranks off the wire; searches `ptp_cost`, `training_cost`, `cost_of_skill` find nothing | GAP (the wiki's profession pages hold the same numbers, e.g. `wiki_clean/Bard.txt:4`, Bard TWC 3/2) | MEDIUM (M7 agent, M10 GUI planner) |
| character-planner.lic:440-591 | profession growth indices | 10 | GI per stat x10, Prime 1, Prime 2 | none (`stats.rs` holds values, not growth) | GAP | MEDIUM |
| character-planner.lic:592-762 | race growth-index modifiers | 13 races | per stat x10 | none | GAP (`wiki_clean/Race modifiers.txt` exists) | MEDIUM |
| character-planner.lic:763-933 | race stat bonuses | 13 races | per stat x10 | none | GAP | MEDIUM |
| character-planner.lic:934-1028 | ascension abilities | 87 rows, 75 with a mnemonic | name, mnemonic, base_atp, max_ranks, tooltip | `psm.rs:344` `AscensionTable` is "a marker with no fields yet" | GAP (gives the vocabulary Cena says it is waiting for) | MEDIUM |
| character-planner.lic:1029-1646 | CMANs | 73 | mnemonic, cost per rank (CMAN points), max_ranks, professions, tooltip | `psm.rs` reads ranks off the wire; lich-5 `psms/cman.rb` has stamina cost only | GAP (training cost and profession availability) | LOW |
| character-planner.lic:1647-4369 | weapon damage factor and AvD by ASG | 100 (74 weapons + 21 bolt spells + 4 UCS + net/runestaff) | slash/crush/puncture %, DF and AvD for ASG 1,5-20 | `data/weapons.tsv` (96 rows, from lich-5 `armaments/`) | CONFLICT on 6 weapons, wiki sides with the planner (below); PARTIAL: 6 brawling weapons blank in Cena get damage-type splits here; bolt-spell DF/AvD GAP | MEDIUM (combat numbers, no milestone) |
| character-planner.lic:4370-16547 | critical tables | 1602 (17 types x 9 generic locations x ranks) | damage, message (wiki style), status code (S#, K, A, F, Fv, RT#, Sil, Slo, Slp), wounds | `data/crit_tables.tsv` (2394 patterns, sided) | HAVE (Cena richer); PARTIAL: plasma and disintegration carry a second, bolt-spell damage figure in `( )` on the wiki (`wiki_clean/Plasma critical table.txt:5-9`) that Cena's one `damage` column lacks; 43 planner values differ for that reason | LOW |
| character-planner.lic:17028-17131 | experience needed per level | 101 (levels 0-100) | level -> total exp | none: `grep -rn 7572500` over crates finds nothing; also in advexp, gs3level, percentcapped, percent_to_cap, postcapgoal, stats, tutelog, xp2 | GAP | LOW (Cena reads `Exp until lvl` off the wire and deliberately drops it, `experience_report.rs:34-44`) |
| health-tracker.lic:17-826 | creature max HP from 711 | 478 (227 with a value) | min, max, damage at each of 6 intensities | `data/creatures.tsv` `max_hp` (589 of 627 filled) | HAVE: 223 overlap, 152 inside the tracker's range, 71 differ (mostly 1-3 HP; `striped relnak` 50-52 vs 42, `dark shambler` 199-201 vs 218); 4 tracker-only names | LOW |
| alchemy-recipes.lic:50-3548 | alchemy recipes and reagent sources | 883 | product, steps, type, rank [lo,hi], for (profession), spell, nick, cost | none: `grep -ril "alchem"` over crates finds only gameobj types, travel tags and one herb source marker | GAP | LOW (no milestone) |
| alchemy-recipes.lic:3549-3583 | reagent equivalents | 29 groups | names that count as one reagent | none | GAP | LOW |
| alchemy-recipes.lic:3584-3620 | reagent sale value | 32 | name -> silver | none | GAP | LOW |
| character-planner.lic:1647-4369 vs `data/weapons.tsv` | weapon DF/AvD, the six rows the wiki settles | 6 weapons | see CONFLICT column | `weapons.tsv:24` backsword DF `\|0.31\|0.225\|0.24\|0.125\|0.15` vs planner and `wiki_clean/Backsword.txt:13` `.440/.310/.225/.240/.150`; `:71` bola DF is backsword's row copied (lich-5 `weapon_stats_thrown.rb:46` has the same copy) vs wiki `Bola.txt` `.205/.158/.107/.118/.067`; `:61` halberd AvD ASG8 17 vs 27; `:66` naginata ASG8 27 vs 47; `:60` awl-pike ASG11 21 vs 31; `:65` lance ASG16 31 vs 41 and DF scale .559 vs .550 (each wiki value checked in `wiki_clean/<Weapon>.txt`) | CONFLICT: Cena inherits lich-5 `armaments/` typos; the planner and the wiki agree | MEDIUM (combat numbers; also worth reporting upstream to lich-5) |
| character-planner.lic:1647-4369 vs `weapons.tsv:13,14,17,20,21,22` | damage-type split for 6 brawling weapons | 6 | slash/crush/puncture % | Cena blank (lich-5 `weapon_stats_brawling.rb:177` "Missing Data on Wiki"); planner: hook-knife 50/0/50, jackblade 50/50/0, paingrip 33.4/33.3/33.3, tiger-claw 50/50/0, troll-claw 50/50/0, yierka-spur 33.4/33.3/33.3 | PARTIAL (planner fills blanks; wiki `Troll-claw.txt` confirms Slash/Crush without percentages) | LOW |
| shield-size.lic:30-72 | shield and dodge DS factors | 6 + 4 + 4 + 4 + 6 + 20 | stance modifiers, ranged size bonus, dodge penalty, dodge factor, dodge stance modifier, dodge hindrance by ASG | `shields.tsv:2-5` (size_modifier, evade_modifier) | CONFLICT: tower evade factor 0.54 here and `wiki_clean/Evade.txt:58-60` vs `shields.tsv:5` -0.5 (lich-5 `shield_stats.rb:41`); the other tables GAP | LOW |
| sexual-favors.lic:109-110 | Voln kill favor by level | 101 + 101 | level_factor, globe_offset | `societies/voln.rs` has symbol COST by level, not favor GAIN | GAP | LOW |
| sexual-favors.lic:136-268 | undead level (and TD modifiers) | 116 names | lvl, td_mod per circle | `creatures.tsv` (level, undead, 13 TD columns) | HAVE: 95 of 116 by name, 2 levels differ (`mist wraith` 4 vs 5, `ice troll` 31 vs 29, and Cena does not flag ice troll undead) | LOW |
| etchedstones.lic:44-282 | flat etched stones / bloodrunes | 158 glyphs | glyph, series, name, effect, duration | none | GAP | LOW |
| SpiritBeast.lic:77-311, 591 | spirit beasts | 187 + 8 | location, room uid, cap level, swim, climb, area, rarity; element weakness | none (`gameobj-data.tsv:71` talisman type only) | GAP | LOW |
| betazzherb2.lic:401-450 | bounty herb name -> forage name | 50 | bounty wording, forage noun | none | GAP | MEDIUM (M8 Bounty) |
| deedcalc.lic:28-57 | deed cost | formula + 5 GS3 bands | `deeds^2*20 + gs3*100 + 101`; ruby 4,500 buys 13,500; max 200 | none (grep `deed_cost`, `gs3`: nothing relevant) | GAP | MEDIUM (death recovery) |
| zombie.lic:186 | death's sting potion cost | formula | `750 + 150 x level` silver | none | GAP | MEDIUM (M6d neighbour) |
| experiencecalc.lic:16-99 | field pool and mind states | formula + 8 states | pool max `800+LOG+DIS`, pulse, 8 mind labels at % thresholds | `character.rs:96-99` keeps text and percent only | GAP, formula age UNVERIFIED | LOW |
| stepcost.lic (tail) | Voln step favor cost | formula | `rank*100 + level^2 * ceil(rank/3)*5 / 3` | none (grep `step cost` in `societies/`: nothing) | GAP | LOW |
| poison.lic (tail, Machtig) | rogue poisons | 10 | name, condition, charges, colour, rank | none | GAP | LOW |
| herball2.lic:31-106, herbs.lic, Herbhigh.lic (Docktoer, v0.4 2021) | herb crosswalk by injury and town | 19 injuries x 5-8 sources | Landing, Icemule, Pinefar, Teras ales, Zul, FWI elixir, bloodletting tea, tincture, Mist Harbor healing gems | `herbs.tsv` (`a6146e6`) (247 rows incl. Teras ales `flagon of Bloody Krolvin ale`, KF elixirs, tinctures) | HAVE except the 10 Aldoran healing stones / Mist Harbor gems (grep `sard`, `chrysoprase`, `spinel` in `herbs.tsv`: 0) | LOW |
| (all healing scripts) | herb heal amount, herb price | 0 | - | `herbs.tsv` has no price or HP-per-bite column either | GAP in both; no script in the pile holds them (searched, see Method). The wiki holds HP per bite per herb (`wiki_clean/Acantha leaf.txt`: "+10 health points per bite") | MEDIUM (M6d blood maths) |
| listcapacity.lic (tail) | container capacity words | 11 | word -> pounds | none | GAP | LOW |
| badgex.lic:77-113 | Adventurer's Guild badge ladders | 3 x 11 | material -> level | none | GAP | LOW |
| tradingbonus.lic:54-183, dirty-deeds.lic:316-324 | racial trading bonus by town | ~16 towns | race -> bonus | none | GAP | LOW |
| useherbs.lic:52-212 (and 3 forks, herbmaster) | herbs | 160 / 175 / 199 | name, type, short_name, store_doses | `crates/cena-model/data/herbs.tsv` (committed `a6146e6`) (247 rows from eherbs 2.2.1: adds locations) | HAVE (`a6146e6`); 1 name missing in all (`tincture of moss`); 11 KF elixir long names (`useherbs_kf`); 20 FWI dish names (`herbmaster`); store_doses: woth 2 vs Cena 3, torban 3 vs 4, sticky basal moss 4 vs 7, two tarts 4 vs 10 (eherbs is newer) | LOW |

## Captures Cena lacks or differs on
| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| injurywatch.lic:290, useherbs.lic:705-872, heal2.lic:107-113 | another character's wounds AND scars, per part and rank | `severe head trauma and bleeding from the ears` (head W3), `a mangled right leg` (leg S2), `developed slurred speech` (nerves S1) | none: `grep -rn "severe head trauma\|slurred speech\|mangled"` over crates finds only creature/crit data. The full 14x3 wound and 14x3 scar table is `reference/wiki_clean/Wound.txt:13-43` | GAP | MEDIUM (healing another character; plan/36 leaves escort/deader out) |
| injurywatch.lic:758-779, 1241-1259 | `appraise` output | `You take a quick appraisal of X and find that he has ...`, `You also notice that he has ...`, `He is also quite dead` | none | GAP | MEDIUM |
| injurywatch.lic:297-311 | an empath's full blood restore, 7 themes | `X appears entirely restored.`, `takes on a much healthier tint`, `flush of full health`, `full vigor` | none | GAP | LOW |
| heal2.lic:177-195, iempath.lic:58-59, healbot2025.lic:126-128 | empath `transfer` replies | `You take some`, `You take all`, `loses some of .+ pallor`, `You infuse`, `Nothing happens`, `You cannot transfer` | none | GAP | LOW |
| smartheal.lic:722-730 | the `health` command | `You seem to be in one piece.`, `You have the following injuries:`, per-location bleed per round, `(tended)`, `Remaining Stamina Points:` | `status.rs:199` has the `bleeding` indicator only | GAP | MEDIUM (M6d: blood-first ordering) |
| health-tracker.lic:964-993 | Pain (711) intensity -> % of max HP | `cringes in pain.` 20%, `shudders in pain.` 23%, `twists in great pain!` 26%, `is wracked with painful spasms!` 29%, `shudders and twists in intense pain!` 32%, `contorts in excruciating agony!` 35% | none (grep for the six phrases finds nothing) | GAP | LOW |
| tutelog.lic:2474-2485, 2875-2877; xp2.lic:2144-2203 | `lumnis info` and the Lumnis/Tutelage start and end lines | `You have N points of doubled experience remaining on your Gift of Lumnis. It is scheduled to refresh in ...`, `Your Gift of Lumnis has expired for this week.`, `Your Gift of Lumnis is scheduled to start on ..., in-game time.`, `The soft feeling of serenity slowly dissipates from your mind.`, `A sensation of heightened understanding crests over you...` | `character.rs:144-182` `Gift { pulses }`, whose doc says a start/end time is the upgrade trigger | GAP (the trigger is met) | MEDIUM |
| xp2.lic:2039-2066 | `experience` lines Cena does not read | `Exp to next TP:` (capped), `Exp to next ATP:`, `PTPs/MTPs: a/b`, `ATPs: n`, `strange sense of serenity`, `recent adventures echo powerfully in your mind` (RPA), `You have been experiencing the Wisdom of the Ages for ...` | `experience_report.rs:28-60` (fame, exp, field, asc, deaths, total, sting, LTE, deeds) | PARTIAL | MEDIUM |
| xp2.lic:2209-2252 | `ascension exp` report | `Absorption Rate: N%`, `ATPs Earned:`, `ATPs Spent:`, `ATPs Available:`, `You are not currently Ascended` | none (grep `ATPs Available`, `Absorption Rate` in crates and lich-5: nothing) | GAP | MEDIUM (the author's character is ascension) |
| xp2.lic:2329 | `resource` amounts, cap read from the line | `^(.+?): a/b (Weekly) c/d (Total)` | `standing.rs:501-502` requires literal `/50,000 (Weekly)` and `/200,000 (Total)` | PARTIAL (a different cap reads as no line; UNVERIFIED that other caps exist) | LOW |
| exp_sagapanel.lic:957-1012; lich-5 `common/xmlparser.rb:725-740` | the `mindState` bar's attributes | `<progressBar id='mindState' value='0' text='clear as a bell' ... field_exp='0' max_field_exp='1324' ascension_exp='0' exp='135' until_next='2365' .../>` (VERIFIED in Cena's own fixture `cena-protocol/tests/fixtures/login_burst_full.xml:3`); plus `fashlonae`, `lumnis`, `rpa` when active | the frame keeps every attribute (`cena-protocol/src/frame/payload.rs:228-235`, `attrs`) but `character.rs:460-466` passes only `text` and `value` to the model. `plan/27b-model-api-character.md:64` names `<progressBar>` as a source of `field_experience`, and `plan/18:68-69` records the attributes as VERIFIED | PARTIAL, and the code disagrees with its own plan | MEDIUM (Rule 2.2 loss; `lumnis` also answers `Gift`) |
| health.lic:63-105 | an injury refusing an action | `You can't make that dextrous of a move!`, `You can't think clearly enough to prepare a spell!`, `You're not in any condition to be searching around!` | `injured.rs` has the rules; no line. `loot/outcome.rs:80` matches only `not in any condition` | GAP | HIGH (M6 hunt, M6d) |
| healme.lic:35-36, 288-289; distiller.lic | Survivalist's kit contents | `The kit contains DOSEs of the following solid herbs: acantha (N), ambrominas (N), ...`, `The kit contains TINCTUREs of the following liquid herbs: rose-marrow (N), bolmara (N), talneo (N), brostheras (N), wingstem (N), bur-clover (N).` | plan/36 Stage 3 (not built; grep `DOSEs of the following` in crates and plan: nothing) | GAP, PLANNED | HIGH (M6d) |
| nesherbalist.lic, sfillherbs.lic, useherbs.lic:443-452 | the herbalist's menu and a purchase | `<d cmd='order N'>name</d>` rows, `You may order a QUANTITY of this item, ORDER something else, or BUY this item.`, `Sold for N silver`, `But you do not have enough silver!`, `Thanks for your patronage` | plan/36 Stage 4 (not built) | GAP, PLANNED | HIGH (M6d) |
| formatfix.lic, smartheal.lic | `health`: poison/disease progress | `Taking N damage per round.  Dissipating N per round.` | `status.rs` poisoned/diseased flags only | GAP | MEDIUM (M6d: wait it out or cure) |
| asclearn.lic:43-51 | `asc list` table | header `the following Ascension Abilities are available`; row `mnemonic</d> cur/max ... (Resist\|Stat\|Skill\|Other\|Regen)` | `psm.rs:336-344` `AscensionTable` marker, form UNVERIFIED | GAP (INFERRED from the script, not a capture) | MEDIUM |
| deathlocation.lic:12-34 | death announcements, 12 phrasings | `X just bit the dust!`, `X was just put on ice`, `The death cry of X echoes in your mind` | `cena-ui/src/merge.rs:28` merges the `death` stream by id; no name or cause read | PARTIAL | LOW |
| zombie.lic:196-209; death_potions.lic:182-186 | death's sting potions | `You hand the priestess N silvers.`, `flask with a clear fluid`, `sweet, like an orange` / `feel somehow changed` | sting level HAVE (`vocabulary.rs:39`); the purchase GAP | GAP | MEDIUM |
| gemdeeds.lic:170-224, deed.lic:159-207, coin4deed_z.lic:104 | deed temples | `the world about shimmers and fades` / `He smiles sadly at you`; `Thy offering pleases the Goddess and thy deed has been recorded` / `The Goddess was neither convinced nor pleased`; `feeling you could have given more` | none | GAP | MEDIUM |
| sexual-favors.lic:511, newfavortracker.lic:202 | undead released (Voln kill favor) | `You hear a sound like a weeping child as a white glow separates itself from the X's body` | none (grep `weeping child` in crates: nothing) | GAP | LOW |
| injurywatch.lic:1104-1184 | someone being healed | `X's <part> looks better.`, `X focuses on Y with intense concentration`, `X's <part> wound gradually fades` | none | GAP | LOW (M7) |
| volndeath.lic (tail), xp2.lic:2001 | own death | `It seems you have died, my friend.`, `You are a ghost!` | `status.rs:210` `dead` indicator | HAVE via indicator; text GAP | LOW |
| unpoison.lic (tail) | another character poisoned | `X appears to grow more tired.`, `X begins to look fatigued.`, `X muscles tremble with fatigue.` | none | GAP | LOW |
| character-planner.lic:20128-20136 | `asc milestone` | `your Ascension Milestones are as follows`, one ` Yes` per milestone | none | GAP | LOW |

## Script by script
| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| character-planner.lic (Dreaven/Tgo01, v10) | 20240 | GTK character planner: stats by level, TPs, CMAN/ascension planning, combat simulator | DATA: training cost per profession x skill (370 rows, `:38-438`), profession growth indices + primes (10, `:440`), race growth indices (13, `:592`), race stat bonuses (13, `:763`), ascension abilities (87 rows, 75 mnemonics, `:934`), CMANs with per-rank cost and professions (73, `:1029`), weapon DF/AvD by ASG (100 incl. 21 bolt spells and 4 UCS, `:1647`), crit tables (1602 rows, `:4370`), armor crit divisors (`:16548`), 12 test critters (`:16842`), level->experience (101 rows, `:17028`). CAPTURES: `experience`, `asc milestone`, `asc info`, `cman info`, `skills base` (`:20119-20200`) | GAP (training costs, growth, race bonuses, level table, ascension vocab); CONFLICT (weapons: planner and wiki agree against Cena on 6 weapons); PARTIAL (crit AS/bolt damage) |
| health-tracker.lic (Tgo01, v13) | 1028 | creature max-HP database from Pain (711) damage | DATA: 478 creatures, 227 with an HP range (`:17-826`). CAPTURE: 711's six pain intensities -> 20/23/26/29/32/35% of max HP (`:964-993`) | HP: HAVE (Cena `creatures.tsv` max_hp covers 223 of 227; 152 inside the tracker's range, 71 differ, mostly by 1-3); 711 intensity table: GAP, LOW |
| alchemy-recipes.lic (Tillmen, v0.11, 2019) | 3821 | the recipe data behind the alchemy scripts | DATA: 883 `:product` rows: 111 general, 87 potions, 114 trinkets (with `:rank [lo,hi]`, `:for` profession, `:spell`, `:nick`), 139 buy (with `:cost`), 191 grind, 19 extract, 3 distill, 129 forage, 87 kill-for-reagent (creature named), 3 misc (`:50-3548`); 29 equivalence groups (`:3549`); 32 reagent opportunity costs (`:3584`) | GAP (no crafting in Cena), MEDIUM/LOW, no milestone |
| alchemy_NewMeasure.lic (Tillmen, mod Thorndar, v0.19 2022) | 3077 | alchemy guild automation | CAPTURES ~239: guild `gld` status (`You have N ranks in the General Alchemy skill`, `You have N repetitions remaining`, `You are a Master of ...`), cauldron light/boil/simmer/infuse/channel, forage success/failure, grind, herb doses (`^The .*? has N bites left`) | GAP (crafting); the doses line is HAVE in `crates/cena-model/src/state/doses.rs` (`a6146e6`) |
| alchemy_noalchemy.lic (Maze, 2021) | 3086 | variant of alchemy_NewMeasure without cauldron work | same as above; 491 changed lines, mostly header and removed cauldron steps (`diff`) | variant |
| useherbs.lic (Tillmen, v0.12 2020) | 1430 | eat herbs by wound; stock herbs; heal an escort | DATA: `$known_herbs` 160 rows (name, type, short_name, store_doses). CAPTURES: `look <escort>` wound descriptions, 46 lines (`:696-872`), purchase (`Sold for N silver`), `order` menu | herbs: HAVE (`crates/cena-model/data/herbs.tsv` (committed `a6146e6`), 1 name missing: `tincture of moss`; 5 store_doses differ); escort wound text: GAP, MEDIUM |
| useherbs_kf / useherbsk / useherbskf.lic | 1447 / 1432 / 1451 | useherbs forks (Kraken's Fall elixirs; Giantphang, Machtig) | +11 KF elixir names in `_kf` not in Cena (`some anise-infused aloeas stem`, `tiny scarlet glass bottle filled with bolmara elixir`, ...) | variants; PARTIAL (KF long names) |
| herbmaster.lic (Tillmen, v0.7 2015) | 1357 | older useherbs | 199 herb rows, 20 names not in Cena (Four Winds "restaurant" herbs: `paprika-sprinkled acantha leaf`, `cumin-rubbed piece of sovyn clove`, ...) | PARTIAL, LOW (2015 data, may be retired) |
| injurywatch.lic (Mystienne, v0.4.2) | 1638 | empath triage window: appraise everyone in the room | CAPTURES: `appraise` (empath and non-empath forms, `:758-779`), the full wound+scar description regex `WOUND_SCAN` (`:290`), 7 empath heal "themes" for full blood restore (`:297-311`), `X's <part> looks better`, `X focuses on Y with`, `X's <part> wound gradually fades` (`:1104-1184`), `quite dead` | GAP, MEDIUM (another character's injuries; plan/36 leaves escort and deader out) |
| smartheal.lic (Vailan, 2020.05.26) | 1323 | empath self-heal by bleed rate and priority | CAPTURE: `health` command (`You seem to be in one piece.`, `You have the following injuries:`, `You have the following scars:`, per-location bleed per round, `(tended)`, `Remaining Stamina Points`, `:722-730`) | GAP, MEDIUM (per-part bleed rate; Cena has only the `bleeding` indicator, `status.rs:199`) |
| heal.lic / heal2.lic (Tayre, Veneyeth) | 216 / 220 | empath heals a target then self | CAPTURES: appraise, `transfer` replies (`You take some`, `You take all`, `entirely restored`, `pallor`, `Nothing happens`, `want`), area->spell 1102/1103/1104/1105 with costs (`heal2.lic:28-40`) | spell costs HAVE (`spells.tsv` 1102-1105 carry the same 2/7, 3/8, 4/9, 5/10 formulas); transfer replies GAP, LOW |
| Healbot2020.lic / healbot2025.lic (Daedeus, v0.75/0.76) | 360 / 364 | answer heal requests in a room | CAPTURES: heal-request speech regex (`:232`), room arrivals (`just limped in`, ... `:319`), taps, bard gem shatter, trap injuries (`severely perforated`, `sickly greenish hue`, `yellowish tint`, `:332-350`), `some of .+ blood loss`, `You infuse` | GAP, LOW (M7 at most) |
| iempath.lic (Monotonous, v1.0 2010) | 312 | AFK empath | same appraise and transfer vocabulary; `loses some of .+ pallor`, `You cannot transfer` (`:58-59`) | GAP, LOW |
| herball2.lic / herball.lic / herbs.lic (Sissi, Soliere) | 108 / 52 / 37 | printed herb crosswalk | DATA (as text): injury x town grid (Landing, Icemule, Pinefar, Teras, Zul, FWI elixir, bloodletting tea, tincture), 10 Aldoran healing stones (FWI backroom 16349), misc forageables (`herball2.lic:31-106`) | herbs HAVE (Cena's table has Teras ales, KF elixirs, tinctures); Aldoran stones: GAP, LOW |
| tutelog.lic (Mystienne, v1.7.10) | 3214 | Lumnis and Tutelage pulse log | CAPTURES: `lumnis info` (`You have N points of (quintupled...doubled) experience remaining`, `refresh in`, `Gift of Lumnis has expired`, `Fash'lo'nae's Tutelage with N day(s)`, `:2474-2485`), Lumnis end `The soft feeling of serenity slowly dissipates from your mind.` and Tutelage start/end (`:2875-2877`), RPA line | GAP, MEDIUM: answers the upgrade trigger `character.rs:173-177` records for `Gift` |
| maxcap.lic (Tgo01, v3) | 590 | PTP/MTP to max every skill | DATA: skill costs, 360 rows (`:63-`) | STALE: differs from character-planner on 29 of 360 rows, and the wiki sides with the planner every time checked (e.g. Bard Spell Aiming: maxcap 3/10, planner 4/1, `wiki_clean/Bard.txt:4` 4/1). Do not port from maxcap |
| auto-level.lic (Dreaven, v48) | 1801 | newbie auto-leveller to 20 | CAPTURES: tutorial sprite, message-runner addresses, `(.*) takes.*your blood loss.`, `(.*) takes your .* damage.` | GAP, LOW (tutorial quest) |
| SpiritBeast.lic (Deathravin, v0.91 2023) | 925 | spirit beast catalogue and fights | DATA: 187 beasts (location, room uid, cap level, swim, climb, area, rarity, `:77-311`), element weakness (8, `:591`), legendary room uids | GAP, LOW (Cena: only the talisman type, `gameobj-data.tsv:71`) |
| invdb.lic (Xanlin, v0.3.9) | 5009 | cross-character inventory SQLite | CAPTURES: `Your locker is currently holding N items out of a maximum of M`, locker manifest refusals, `You carefully survey your surroundings and guess that your current location is ...`, `Account Type\|Subscription` | GAP, LOW (inventory, not this pile's subject) |
| sexual-favors.lic / sexual-favors2.lic / ssexual-favor.lic (Tillmen, v0.34 / 0.33 / 0.5) | 1074 / 1073 / 958 | Voln favor tracker | DATA: kill-favor `level_factor` and `globe_offset` by level (`:109-110`), 22 deities' Divine Wrath end lines (`:483`), undead TD->level. CAPTURES: undead release `You hear a sound like a weeping child as a white glow separates itself from ...`, every Voln symbol's use line | symbol COSTS HAVE (`societies/voln.rs`, 98 values checked against the wiki); favor GAIN formula and the release line GAP, LOW |
| newfavortracker.lic (Caithris, v0.5) | 518 | Voln favor tracker | CAPTURES: `You are a member in the Order of Voln at step N.`, 20 symbol use lines (`:135-202`) | step: HAVE (`societies/membership.rs`); symbol use lines GAP, LOW |
| stat-maximizer.lic (Dreaven, v5) | 1011 | best stat total at cap | DATA: the same profession and race growth tables as character-planner (`:38`, `:190`). CAPTURE: `info start` -> `Level N Stats for X, Race Prof` (`:999-1002`) | GAP, MEDIUM (with the planner's tables) |
| etchedstones.lic / bloodrunes.lic (Ordim, v0.03) | 542 / 356 | read flat etched stones and bloodrunes | DATA: 158 glyph -> series, name, effect, duration (`etchedstones.lic:44-282`); bloodrunes is the older 45-glyph cut | GAP, LOW |
| saladbar.lic (Tillmen fork, Cait) | 975 | herb healing, herbs on a bench | same vocabulary as useherbs | variant |
| herbheal.lic / herbhealHURSHALFIX.lic (Baerden) | 476 / 481 | bench-then-shop herb healing | herb noun list (`:68`) | variant, LOW |
| thunder.lic (Dreaven, v18) | 1376 | server for ;struck multi-character control | bounty text regexes (`:1145-1180`) | bounty belongs to pile 8; N/A here (Cena is multi-session natively) |
| defense_calc.lic (Gibreficul, v2.3 2014) | 550 | DS calculator | CAPTURES: `Your careful inspection ... allows you to conclude that it is (a small\|medium\|large\|tower shield)` (`:143-172`) | LOW; shield sizes are in `shields.tsv` |
| back-in-black.lic (Dreaven, v2) | 359 | room info panel (critters, herbs, magic) | reads Lich map tags | N/A (Cena map) |
| betazzherb2.lic (Zzentar, Tsalinx v5.3) | 794 | forage a bounty herb | DATA: 50 bounty-name -> forage-name translations (`:401-450`). CAPTURES: forage replies (`You forage briefly and manage to find`, `sharp pain in your right hand`, `fairly certain this is where it can be found`, `forage more than 15`, `:714-762`) | GAP, MEDIUM (M8 Bounty herb tasks) |
| exp_sagapanel.lic (Claude Code/Codex/RuseofFools, v1.23.0) | 1596 | Saga experience panel | CAPTURES: the `expr` dialog's `mindState` bar ATTRIBUTES `field_exp`, `max_field_exp`, `ascension_exp`, `exp`, `until_next`, `fashlonae` (0/1/2) (`:957-1012`); `experience` incl. `Exp to next TP ... Exp to next ATP`, `PTPs/MTPs: a/b ATPs: n` (`:1129-1150`); `lumnis info` incl. `available uses`, `last used a Lumnis scheduling option`, `Because your account is free` (`:1185-1241`) | PARTIAL, see Top findings: Cena's frame keeps these attributes but `character.rs:463-466` reads only text and value |
| fletching.lic (unnamed, draft) | 1299 | make, paint and crest arrows | CAPTURES: shop `order` menu, quiver contents XML, `bundle`, fletching steps | GAP (crafting), LOW |
| fletch-ranks.lic (v0.31) | 561 | rank up fletching | CAPTURES: `artisan skill` -> `In the skill of fletching, you are an? (.*) with (\d+) ranks.` (`:117-120`), pare/nock/glue/fletch step replies (`:291-449`) | GAP (crafting), LOW |
| joust22.lic (Demandred, v3.0) | 859 | Rumor Woods jousting | `sunburst marker ... It currently has N entries left`, `quest transport rumor` | GAP, LOW (event) |
| stats.lic (Dreaven, v5) | 496 | character facts from many commands | CAPTURES: `asc info` porter/regenhealth ranks, `stamina` `Stamina gained per pulse`, `mana` `Mana gained off/on node`, `health` `Physical/Magical Damage Reduction: N%`, `profile` House list + `No MHO affiliation`, `society task` `You currently possess N prestige points`, `gld` `You currently have N ranks out of a possible M for your training`, `info start` `This character was created on ...` (`:58-166`) | Houses HAVE (`vocabulary.rs:497-` `Che`, 17); the rest GAP (grep `prestige`, `Damage Reduction`, `gained per pulse`, `was created on`: nothing), MEDIUM |
| healme.lic (Alastir, v1.2) | 487 | eat herbs from a Survivalist's kit | CAPTURES: `look in` kit -> `The kit contains DOSEs of the following solid herbs: acantha (N), ambrominas (N), ...` and `... TINCTUREs of the following liquid herbs: rose-marrow (N), ...` (`:35-36`, `:288-289`); `buy` -> `hands you` | GAP today; PLANNED in plan/36 Stage 3. The exact line shape is here. HIGH (M6d) |
| service_queue.lic | 360 | empath service queue | appraise; transfer replies `meets with your standards of conduct`, `gradually fades`, `You must heal your own`, `You strain to do the transfer, but it doesn't work` (`:95-99`) | GAP, LOW |
| dirty-deeds.lic (Dreaven, v20) | 1804 | buy deeds with gems/wands/lockpicks in Landing, Icemule, River's Rest | DATA: race trading bonus (`:316-324`), town deed item lists. CAPTURES: bank balance, gem dealer quotes, `Long-Term Exp:` | deeds: GAP, LOW (see deeds group below) |
| tradingbonus.lic (Gibreficul, 2014) | 231 | trading bonus by town, race and citizenship | DATA (as code): per town, which races get a racial bonus or malus (`:54-183`); `location` command | GAP, LOW |
| struck.lic / thunder.lic (Dreaven) | 390 / 1376 | ;thunder multi-character client | bounty regexes | N/A |
| CrapsDealer.lic (Dreaven, v5) | 1421 | craps game | payout table | N/A |
| random-verb.lic (Dreaven, v1) | 310 | random emote | verb lists with race/profession limits (`:68-102`) | N/A (LOW: race-only verbs) |
| uberbarmagic.lic / uberbarmagic2.lic (Elkiros) | 353 / 355 | Stormfront status bar | `resource`: `Essence:`, `Necrotic Energy:`, `Motes of Tranquility:`, `Weekly Kills to LKP:`, `Lore Knowledge Points:` (`:52-68`) | HAVE (`vocabulary.rs:189-230` has all resources incl. `Lore Knowledge`); `Weekly Kills to LKP` looks like an older format, UNVERIFIED |
| gs4tools.lic (v0.3.2) | 833 | misc toolbox | society `at step N`, `Voln Favor:` | HAVE (`membership.rs:57`, `currency.rs:140`) |
| shield-size.lic (Tillmen, v0.1) | 215 | shield vs dodge DS by size and stance | DATA: shield stance modifiers (6), ranged size bonus (4), dodge shield penalty (4) and factor (4), dodge stance modifier (6), dodge armor hindrance by ASG (20) (`:30-72`) | CONFLICT: tower shield evade factor 0.54 here and on the wiki (`wiki_clean/Evade.txt:58-60`, "46% (tower)") vs Cena `shields.tsv:5` evade_modifier -0.5 (from lich-5 `shield_stats.rb:41`); the rest GAP, LOW |
| csv_enhancive.lic (v1.2.1) | 664 | export enhancives to CSV | CAPTURES: `INVENTORY ENHANCIVE TOTAL DETAIL` per-item lines `^\s{4}([+-]\d+):\s+(.+)$`, `INVENTORY ENHANCIVE LIST` (`You are wearing the following enhancive items:`), `INVENTORY LOCATION USAGE` `(a/b F) (c/d T)` (`:161-345`) | totals HAVE (`enhancive.rs`); per-item attribution PARTIAL (`enhancive.rs:276-277` leaves it to the caller); list and location usage GAP, LOW |
| inkweeks.lic / inkweeks2.lic (Elkiros, v3.2) | 387 / 395 | monk tattoo inkweek helper | DATA: roll-description table, location chart; `tattoo menu` | GAP, LOW |
| badgex.lic (Hexbane, v2.0) | 388 | read an Adventurer's Guild badge | DATA: three 11-step material ladders -> badge levels (`:77-113`) | GAP, LOW (pile 8) |
| zombie.lic (Tillmen) | 441 | death recovery: wait out death, buy sting potions, log off below a deed floor | DATA: sting potion cost `750 + 150 x level` (`:186`). CAPTURES: old-layout `experience` (`Level: N Deeds: N`, `Experience: N Death's Sting: X`, `:155-170`), priestess `give` replies (`You hand the priestess N silvers.`, `... but she only takes`, `You count your coins and realize that you don't have that much money!`), `drink my flask` (`:196-209`) | Death's Sting HAVE (`vocabulary.rs:39`, 7 levels); the recovery flow GAP, MEDIUM (M6d neighbour; plan/36 names `deader` as not ported) |
| death_potions.lic (Gibreficul) / death_potions_2.lic (Xanlin, v2.4) | 230 / 328 | buy potions until sting is None and CON penalty 0 | CAPTURES: `info` CON with the bolded death penalty (`death_potions.lic:66-71`), `flask with a clear fluid`, `sweet, like an orange` vs `feel somehow changed` (`:182-186`) | GAP, MEDIUM; the CON penalty reading is PARTIAL against `stats.rs` (it reads bold, but no "death penalty" meaning is attached, UNVERIFIED) |
| deathlocation.lic (Hazado, v0.4) | 39 | add the area to death announcements | CAPTURES: 12 death-cry phrases on the death stream: `just got squashed`, `has gone to feed the fishes`, `just bit the dust`, `just turned ... last page`, `was just put on ice`, `just punched a one-way ticket`, `is going home on ... shield`, `just took a long walk off of a short pier`, `is dust in the wind`, `is six hundred feet under`, `death cry of ... echoes in your mind`, `just gave up the ghost` (`:12-34`) | PARTIAL: Cena merges the `death` stream by id (`cena-ui/src/merge.rs:28`) but reads no name or cause from it; LOW |
| deathlog.lic / deathbot_ignore.lic | 60 / 184 | log what killed you; squelch deathbots | attack lines aimed at you; death stream | LOW |
| deed.lic / deeds.lic (Gwrawr) / deedcalc.lic (elanthia-online, v1.1.0) / gemdeeds.lic (Demandred) / coin4deed_z.lic / ruby-deed.lic (Tillmen) / rrshellsdeeds.lic (Nagrad) | 217 / 204 / 328 / 391 / 145 / 91 / 206 | get deeds | DATA: deed cost `deeds^2 x 20 + gs3_level x 100 + 101`, GS3 level from experience in 5 bands, ruby 4,500 buys 13,500 of deed value, max 200 (`deedcalc.lic:28-57`). CAPTURES, per temple: Landing pool (`touch pool` -> `the world about shimmers and fades` / `He smiles sadly at you`, then flower/seed/`plant seed`), the drawer temple (`close drawer` -> `feeling you could have given more`), the chime temples (`ring chime with mallet` -> `Thy offering pleases the Goddess and thy deed has been recorded` / `The Goddess was neither convinced nor pleased`), Illistim `turn tome to illistim` (`gemdeeds.lic:170-224`, `deed.lic:159-207`) | deed COUNT HAVE (`experience_report.rs`, `Deeds:`); cost formula and temple routines GAP, MEDIUM (Cena's travel routines are the natural home, `cena-behavior/src/travel/routines/`) |
| forge.lic (v1.0.0) | 366 | forging loop | CAPTURES: workshop rental `rentals are 300 silver`, `stare handle-glyph`, `turn grinder` quality replies (`:42-139`) | GAP (crafting), LOW |
| sheal.lic (SpiffyJr, v1.0) | 382 | empath heal via imprint | CAPTURES: `imprint check` family (`Through your spiritual connection you sense X...`, `Your connection to X is too weak`, `You must wait about N more minutes before attempting to imprint X again`, `:65-92`) | GAP, LOW |
| HealBot.lic (Auryana, v0.75) | 119 | oldest healbot | appraise, heal-request speech | variant of healbot2025 |
| ucharge.lic (Demandred, 2024.12.14) | 590 | wizard 517 item charging | `You sense that .+ with N charges`, `You estimate you're skilled enough to push this item to a maximum of N charges` (`:388-391`) | pile 5; GAP, LOW |
| drfarm.lic (anonymoose420) | 189 | Duskruin arena farming | `Deeds: N`, arena entry | LOW |
| health.lic (Alastir) | 150 | react to injuries that block an action | CAPTURES: the game's refusals: `You can't think clearly enough to prepare a spell!` (head/nerves), `You can't make that dextrous of a move!` (arms/hands), `You're not in any condition to be searching around!` (head), `You hear a loud *POP* come from your muscles!`, `You are a ghost!`, `slices deep into your vocal cords!` (`:47-105`) | cutthroat HAVE (`afflictions.rs:218`); the three refusal lines GAP (grep `dextrous`, `think clearly enough`: nothing in crates; `loot/outcome.rs:80` has only `not in any condition`), HIGH (M6 hunt and M6d: the wire's own statement that an injury blocks an action) |
| formatfix.lic | 229 | reformat `health`, `info`, `cman` output | CAPTURES: `health` poison/disease `Taking N damage per round.  Dissipating N per round.`, `(Maximum\|Remaining) (Health\|Spirit\|Stamina) Points:`; `cman` `(.*), Rank N (CM points needed: N)`; `info` stat rows with bolded values (`:1-229`) | poison/disease rate GAP, MEDIUM (M6d: how long until poison/disease wears off); stats HAVE (`stats.rs`) |
| 925.lic (Tgo01, v2) | 97 | Wizard 925 enchant check | CAPTURES: old `experience` layout (`Mental TPs:`, `Physical TPs:`, `(N Phy converted to Mnt)`), `Your mind ...`, familiar `You sense understanding from your X.`, workshop `this room is a magical workshop` | LOW |
| 2lumnis1cup.lic (Nugt) | 173 | reschedule the Gift of Lumnis | CAPTURES: `boost info`: `Fast Exp. Absorption Boosts:`, `Random Magical Crystals:`, `Minor/Major Loot Boosts:`, `Enhancive Boosts:`, `Bounty Boosts:`, `Urchin Guide Access:`, `Urchin Runner Uses:`, `Encumbrance Boosts:`, `Guild Boosts:`, `Instant Mind Clearers:`, `Death's Sting Reducers:`, `Bounty Task Waivers:`, `Item Superchargers:` (`:36-60`) | GAP (grep `boost info`, `Loot Boosts`: nothing), LOW |
| lumnisdonate.lic | 524 | Lumnis donations | CAPTURES: `Your Gift of Lumnis is scheduled to start on Xs at HH:MM, in-game time.`, both `scheduled to refresh in` layouts, donation count; `wealth` silver lines | Lumnis GAP (see tutelog); silver HAVE (`currency.rs:103-131`, incl. `silver stored within your`) |
| gpriches.lic (Giantphang, v0.4) | 125 | currency window | CAPTURES: event currencies (bloodscrip, ethereal scrip, tickets, shards, raikhen), redeem lines, `You currently have N unspent bounty points.` | currencies HAVE (`currency.rs:56-185`); redeem/receive lines GAP, LOW |
| tran.lic / tran2.lic | 193 / 189 | empath heal by appraise | appraise parse, `That only works in the Gladiator Arenas.` | variants, LOW |
| complete-info.lic (Dreaven) | 258 | log all character info | CAPTURES: `info`, `experience`, `asc` (`Available Ascension Ability Points: N`, `You know absolutely nothing about Ascension Abilities.`), `bank` (`You currently have the following amounts on deposit:`, `inter-town bank transfer options available`) | experience/bank HAVE (`bank.rs:343`); ascension points GAP, MEDIUM |
| funexp.lic (Gibreficul) | 139 | fame vs exp stats | older `experience` layout (`Exp. until next`, `Mental TPs`, `Physical TPs`, `N Phy converted to Mnt`) | superseded layout; LOW |
| exptrack.lic | 615 | experience session tracker | reads the `expr` dialog | LOW |
| uberbarv_d.lic / uberbarv / uberbar | 309 / 264 / 174 | status bars | `Field Exp:`, `Exp to next ATP:`, `Voln Favor:`, resource names | covered above |
| recount.lic | 172 | time-to-kill tracker | target and death link parsing | pile 2 |
| max_lock.lic / maxlock2.lic / maxlock3.lic / lockformulae.lic / lockranges.lic / autolockranges.lic | 127 / 242 / 245 / 97 / 223 / 55 | max lock and trap by lockpick | DATA: lockpick modifiers; `You are a Master of Lock Mastery.` / `You have N ranks in the Lock Mastery skill.` (`maxlock2.lic`) | pile 5 (boxes); LOW here |
| slats.lic | 493 | carving slats (wood and symbol types) | DATA as echo: 46 wood types, symbol types | crafting GAP, LOW |
| pickpocket.lic | 769 | pickpocketing | CAPTURES: 10 steal outcomes (`You reach into X's pockets and pull out N silvers.`, `notices your actions and turns suddenly`, `still recovering from your last larceny attempt`, room-odds lines) | GAP, LOW |
| whatstats.lic / statperfect.lic | 142 / 404 | stat growth calculator | race/prof growth data (`whatstats.lic`, 23 data lines) | same tables as character-planner |
| high_type.lic | 1119 | link colouring by GameObj type | N/A | - |
| enhwearing.lic | 208 | list worn enhancives | `wearing the following enhancive item`, `Inspecting that may not be a sound idea`, `unlocked loresong` | enhancive list GAP, LOW |
| gardener.lic | 445 | empath 1118 herb growing | `Some X suddenly appears at your feet.`, `plenty of healing items like your X already there` | GAP, LOW |
| growfish.lic | 265 | pet fish feeding | N/A | - |
| allmine.lic | 447 | guard All Mine items | N/A | - |
| askwhirlin.lic | 202 | skill summary | `skills` table rows | HAVE (`skills.rs`) |
| consolidateherbs.lic (after Tillmen's bundle_herbs) | 109 | bundle herbs out of the lootsack | CAPTURES: `Carefully, you combine`, `If you add anything more to this bundle`, `You do not have anything to bundle!`, `You carefully pour`, `You can't pour any more` | HAVE (`state/doses.rs:71,102`, `a6146e6`); the two refusals PARTIAL, LOW |
| UBW-Theme-{Dude,Sexy,Wrayth,SF}.lic | 71 each | uberbar colour themes | N/A | - |
| asclearn.lic (Tysong, elanthia-online, v1.1.0 2025-10-01) | 106 | learn every ascension ability | CAPTURES: `asc list` -> header `the following Ascension Abilities are available`, rows `mnemonic</d> cur/max ... (Resist\|Stat\|Skill\|Other\|Regen)` (`:43-51`); `asc learn X` -> `You have chosen to learn rank` / `You do not have enough points available.` | answers `psm.rs:336-339`'s UNVERIFIED question, INFERRED from a maintained script's regex rather than a capture: the `are available:` form comes from `asc list`. GAP, MEDIUM |
| mural.lic (Drafix) | 68 | Mural of Deities puzzle | 103 verses -> 20 deities | HAVE: all 103 found verbatim in `cena-behavior/src/travel/routines/mural_of_deities.rs` (which has 108) |
| nesstats.lic (Nesmeor) | 559 | exp and bounty-point stats | `accumulated a total of N lifetime bounty points`, task-eligibility lines, Lumnis-active `You feel the eyes of Lumnis upon you.` | bounty lines pile 8; Lumnis line GAP (see tutelog), LOW |
| microbar.lic (Brute) | 78 | minimal bar | Lumnis-active `You feel a strange sense of serenity and find that you are able to reflect on recent events with uncommon clarity and understanding`, RPA line | GAP (see tutelog) |
| enhancive_detector.lic | 202 | infer enhancives by diffing `info`/`skills`/`health`/`mana` | `health` `Maximum (Health\|Spirit\|Stamina) Points` Normal vs Enhanced, `mana` `Maximum Mana Points: a b`, `Mana gained off node`, `You have used the MANA SPELLUP ability` | GAP, LOW |
| inspectarmor.lic (Takoa, 2015) | 70 | armor coverage from `inspect` | `(cloth\|leather\|scale\|chain\|plate) armor that covers the (.*).` | coverage is derived in `armaments.rs:21-24`; the inspect line GAP, LOW |
| calcbattlestandard.lic (Sethius, 2024) | 169 | battle standard calc | `info` header `Gender: X Age: N Expr: N Level: N`, skill rows, resource weekly | HAVE (`stats.rs`, `skills.rs`, `standing.rs`) |
| link.lic | 110 | empath link tracker | `you concentrate on establishing an empathic link`, `...but the link fails`, `You sense your empathic link with` | GAP, LOW |
| sfillherbs.lic / nesherbalist.lic / iherbalist.lic / buyallherbs / rrherbs.lic (Xanthras) / scanherbs.lic (Zangow) | 129 / 252 / 216 / 66 / 327 / 65 | buy, fill and scan herbs | herbalist `order` menu rows `N. (a\|some) name`, `You may order a QUANTITY of this item, ORDER something else, or BUY this item.`, `Thanks for your patronage`, dose lines `has N bites left`, `(a\|has a\|have) X (bites?\|doses?) left` | doses HAVE (`state/doses.rs`); herbalist menu and purchase lines PLANNED (plan/36 Stage 4), HIGH (M6d) |
| rrbundle.lic / bundleall.lic | 189 / 152 | bundle herbs | `(\w*) doses left`, `Carefully`, `If you`, `Try as`, `You can't` | HAVE/PARTIAL as consolidateherbs |
| arena-stats.lic / heist_tracker.lic / drfarm.lic | 136 / 115 / 189 | Duskruin arena and heist | arena entry/exit lines, `You steal N bloodscrip`, `Ophidian Cabal` lines | GAP, LOW (event) |
| autonet.lic / amunet.lic (Oweodry) | 121 / 91 | keep the ESP net on | `The power from your sign dissipates`, `You sense that your attunement to the minds of others has ceased.` | LOW |
| paladinaura.lic (Demandred, 2025.03.09) | 95 | paladin aura switch | `You must wait N seconds before switching auras.` | pile 6 |
| citizenship.lic (Oweodry, 2014) | 175 | check citizenship eligibility | `you do seem to be eligible for full citizenship`, `You currently have full citizenship ...` | citizenship HAVE (`standing.rs`); eligibility GAP, LOW |
| make_voln_iron.lic | 164 | Voln iron task | furnace steps | LOW |
| junktrawl.lic (Hexbane) / bluegreen.lic / enhancivesorter / enhancive_shopper / enhancive_watch / enhancive.lic | 357 / 100 / 51 / 135 / 20 / 109 | enhancive shopping and charging | `You are wearing the following enhancive items:` | LOW |
| asclearn-adjacent: ascinfo.lic / AscCalc.lic | 169 / 203 | ascension summary and calculator | `Ascension Milestones` | GAP, LOW |
| xfletch.lic | 199 | fletching | split/pare/nock replies | crafting GAP, LOW |
| picking_exp.lic / service_sales.lic (peggyanne, v3.4.1) / professionresource_sagapanel.lic / gs3level.lic (Xorus) | 96 / 850 / 230 / 364 | box exp predictor; service adverts; resource panel; GS3 level | reads existing data; `gs3level` holds a GS3 level table | LOW |
| distiller.lic | 101 | distil solid herbs to tinctures in a Survivalist's kit | `contains DOSEs of the following solid herbs:(.*?)\.`, `TINCTUREs of the following liquid herbs:(.*?)\.`, `^(.*?) \((\d+)\)$`, the 20 kit herbs (`:1-101`) | PLANNED (plan/36 Stage 3), HIGH (M6d) |
| experiencecalc.lic | 101 | predict pulses and mind state | DATA: field pool max `800 + LOG + DIS`, pulse `28 + LOG/5 + pool/200` plus 10% long-term, 8 mind states at 100/90/75/62/50/25/1/0% of the pool (`:16-99`) | GAP (`character.rs` keeps the mind text and percent, no vocabulary or pool), LOW; formula age UNVERIFIED |
| lte_smart.lic | 148 | use ROL brooch, instant mind clears, LTE boosts at 100% mind | `resulting in the ability to instantly absorb N experience`, `Field Exp:` | GAP, LOW |
| crumbly.lic | 116 | warn when an enhancive runs out | `enhancive magic will be depleted soon`, `hidden item on your person is reaching its end soon` | GAP, LOW |
| unlearn.lic | 33 | unlearn an ascension ability | `You have chosen to unlearn`, `You decide to unlearn`, `But you don't have any ranks in X!`, `You already have an unlearning selection` | GAP, LOW |
| famecheck.lic (Alastir) | 89 | fame gain in the arena | `Your personal fame is N.` | fame HAVE from `experience` (`experience_report.rs`); this second source GAP, LOW |
| ci_fishing_weight.lic | 136 | fishing weight | `You carefully examine the X and determine that the weight is about N pounds.`, hook-set and line-tension lines | crafting/fishing GAP, LOW |
| grimcount.lic | 79 | Grimswarm camp counter | shroud ripple lines, `You sense the presence of N hated enemies nearby.` | GAP, LOW (event) |
| pooled-essence.lic (Maodan, v1.0) | 38 | wizard essence pool SENSE -> percent | DATA: 7 words -> ranges (`faint` <10% ... `saturated` 95-100%) | GAP, LOW (pile 6) |
| imprintcheck.lic / imprint_heal.lic / heal_tracker.lic / shealwatch.lic (SpiffyJr) | 83 / 49 / 82 / 145 | empath imprint and heal-by-link | `Through your spiritual connection you sense X...`, `(He\|She) is in good shape` | GAP, LOW |
| whatlevel.lic / whatlevel2.lic / xp_commas.lic / xp.lic / fe_test.lic | 117 / 83 / 382 / 393 / 60 | reformat `experience` | `Exp (?:until lvl\|to next TP)`, `Level: N Fame: N`, `Long-Term Exp:` | HAVE except `Exp to next TP` (see xp2) |
| mxp.lic | 286 | equalise exp across a group by ascension absorption rate | `asc` absorption rate setting | GAP, LOW |
| mystic_calc.lic (Greminty) / tattoolist.lic / inkweeks | 78 / 103 | mystic tattoo odds; tattoo list | wiki-derived factors | LOW |
| wikibeast.lic / level1.lic / runlist.lic / prettynum.lic / agidex.lic / volnrestore2.lic / lumnis_grift.lic / deeds_landing.lic (Gibreficul) / calcsanctify.lic (Liandru) / escape_voln_hermit.lic / roll-tracker.lic / alchemydistill.lic | small | utilities | `calcsanctify`: `(Use SKILLS BASE to display unmodified ranks and goals)`; `roll-tracker`: `CS: +N ... d100: +N ==`, `d100 == 1 FUMBLE!`; `alchemydistill`: guild distil task; `lumnis_grift`: donation count | LOW; roll lines are pile 2 (Cena `combat/resolution.rs`) |
| xp2.lic (Mystienne, v1.9.4) | 2963 | EXP window | CAPTURES: every `experience` line incl. `Exp to next TP`, `Exp to next ATP`, `PTPs/MTPs: a/b`, `ATPs:`, `Your mind ...`, Lumnis-active, RPA, Tutelage, `Wisdom of the Ages for ...` (`:2008-2066`); full `lumnis info` incl. schedule and donations (`:2144-2203`); `ascension exp` (`Absorption Rate`, `ATPs Earned/Spent/Available`, `:2209-2252`); `resource` generic caps (`:2327-2340`) | experience core HAVE (`experience_report.rs`); the extra lines GAP, MEDIUM; resource PARTIAL (Cena hard-codes `/50,000` and `/200,000`, `standing.rs:501-502`) |

## Tail
108 scripts with capture + data < 5. Grouped by name; the ones marked (opened) were read.

- **Healing loops, no new text:** selfhealall, healself, taptoheal, herb, herbs2, labelherbs, makeherbs, buyallherbs,
  COLnobleed (opened: CoL `sign of clotting` when any wound >= 2), sunfist-health (opened: `sigil of health` below
  65%), poisoncheck (opened), recovery (opened: `boost enhancive reco`). Herb lists: herbs, herball (opened, same
  crosswalk as herball2; HAVE in `herbs.tsv`), Herbhigh (opened: adds Mist Harbor healing gems and Zul drinks).
- **Poison and death:** poison (opened: 10 rogue poisons, see data table), unpoison (opened: three "another
  character is poisoned" lines), deathopen, deathvambraces (opened: vambrace charge replies `completely satisfied`
  ... `burning hunger`), dead_keepalive, volndeath (opened: `It seems you have died, my friend.`), volnrestore.
- **Deeds, one-temple variants of the deed group:** cashdeeds, deedgetincash_landing, jdeed, jfreedeed, tdeed,
  rrdeed, 10deeds, deednobank, gems2deedstv, gems2deedswl.
- **Experience and Lumnis wrappers:** uberbarv, uberbar, postcapgoal, oldredux, redux, loud-exp-pulse (reads
  `nextLvlPB`), XPMonitor, noxp, getexp, percentcapped, pulse, pulse_do, rest, statperfect, lumnis_stat_boost,
  lumnis_exp_boost (`boost experience`), lumnis_reminder (`You must wait six months from`), lumnis_average
  (`lumnis contest current` table).
- **Locks:** lockpicks, maxlock, lockranges, autolockranges, locks (pile 5).
- **Voln and societies:** volnfavor_sagapanel, stepcost (opened: step favor formula, see data table), voln-answers
  (opened: 11 Voln quiz questions and answers), summation_seed (opened: summoning seed rank bonuses), realkgate,
  surge, keepsurge.
- **Crafting and fishing:** dforge, hforge, fuck_forging (`You nod, satisfied with the piece you've created.`),
  forge_loop (`Your bronze bar is too small to cut in two!`), smithy (Duskruin smithy costs), gfish, ci_fish,
  ci_support_fishing (both 3 lines: "You shall not FISH!").
- **Lore and reference printouts:** stones (opened: 58 Aldoran scrying-stone meanings), listcapacity (opened: 11
  container-size words to pounds), lore_perm_bonus (opened: bard loresong bonus formula), mysticalc,
  focusvsspells, collectible_type_data (opened: collectible name regexes, pile 4).
- **UI, timers and one-liners (N/A):** enhancive, enhancive_watch, profiler, duck-and-weave, DontForget, afkchecker,
  hexmal, briar, drone, hardcore, idleguard (`Today is`; Cena's idle warning is `state/idle.rs`), rift-entry-timer,
  summon, cleangear, getcmranks, myklian, purr, stupidrocks, Phelnotes, ReplaceLinksWithMyName, alfred-chrism,
  bundle, juntrain, ls, failsafe, jinfo, laugh, login-documentation, remember, transfer.

## Method
- Pile and substantive count: `awk -F'\t' 'NR>1 && ($3+$4)>=5' pile-7-character-healing-crafting.tsv | wc -l` -> 176;
  tail `($3+$4)<5` -> 108.
- Headers and capture lines for every substantive script: `tools/7-character-healing-crafting/dump.py` (regex
  `=~ */|waitforre|matchtimeout|matchwait|waitfor|DownstreamHook|when +/|Regexp.new|%r\{|dothistimeout|...`), then
  `compact.py` for the regex literals.
- Data comparisons, all in `tools/7-character-healing-crafting/`: `weapons_cmp.py` (planner vs `weapons.tsv`, 100 vs
  85 distinct names, 53 diffs), `crits_cmp.py` (1602 planner rows vs `crit_tables.tsv`, 831 matched, 43 damage
  diffs all plasma/disintegration), `hp_cmp.py` (health-tracker vs `creatures.tsv` max_hp), `herbs_cmp.py`
  (useherbs family vs `herbs.tsv`), `cost_cmp.py` (planner vs maxcap training costs, 29 of 360 differ). Wiki spot
  checks: `grep -E "^<Weapon> \| AG AsG" reference/wiki_clean/<Weapon>.txt`; `Evade.txt:58-60`;
  `Plasma critical table.txt:5-9`; profession pages for costs.
- Row counts: `grep -c ':product =>' alchemy-recipes.lic` -> 883, and per section by line range; `grep -c '"mnemonic"'`
  in the planner's ascension block -> 87 (75 real); `grep -cE '"PTP" =>'` -> 370; `awk ... grep -c ":lvl"` -> 116
  undead names; etchedstones `grep -c "elsif server_string"` -> 158.
- Absence searches (all over `E:/Cena/crates`, `*.rs` and `*.tsv`, unless noted): `severe head trauma|slurred
  speech|mangled|old battle scar|muscle spasms` (only crit/creature data); `cringes in pain|wracked with painful
  spasms|invoking Pain`; `dextrous|think clearly enough|Dissipating|damage per round`; `DOSEs|TINCTURE|QUANTITY|
  patronage`; `fashlonae|until_next|max_field_exp` (only fixtures and the `ascension_experience` field); `lumnis
  info|Gift of Lumnis|Tutelage|echo powerfully|Wisdom of the Ages`; `Weekly|Exp to next TP|to next ATP|ATPs
  Available|Absorption Rate|Shadow Essence` (also over lich-5 lib); `asc info|asc milestone|Ascension Milestones`;
  `prestige|Damage Reduction|gained per pulse|was created on|kit contains|MHO`; `7572500` (level table);
  `ptp_cost|training_cost|cost_of_skill|growth_index|Prime 1`; `alchem|fletch|forging|cooking|fishing|garden|
  cauldron|tincture|artisan|work order|gld`; `boost info|Loot Boosts`; `bit the dust|feed the fishes|priestess`;
  `weeping child`; `deed_cost|gs3`; `sard|chrysoprase|spinel` in `herbs.tsv`; `It seems you have died|look
  fatigued`; `step cost` in `societies/`; plan/ for `DOSEs of the following` and `field_exp`.
- Healing-description search over the whole pile: `(appears|seems) to be .*(shape|wounded|injured|...)|in good
  shape|no apparent (injuries|wounds)` -> only `appears to be in good shape` and `no apparent injuries/wounds`.
  Herb price and heal-amount search: `per bite|heals? [0-9]+|hp per|:heal(s|_amount)? *=>|:cost *=> *[0-9]` over
  the pile -> only the three alchemy scripts.
- Not searched: the log archive (brief). HEAD re-checked at the end because parallel sessions committed during the
  survey (3566f8d -> 067b52a -> a6146e6).
