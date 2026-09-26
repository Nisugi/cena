# 5-boxes-magic-items: lich_repo_mirror survey

Scope: 104 scripts, 87 substantive, 17 tail (by the pile's capture + data heuristic); plus 19
special-ask scripts outside the pile (918wands, wands, wanddupe, aniwand, unstunwand, enchant,
enchantcheck, enchantnotes, ensorcell_calc, ensorcell_tracker, lore, loresong, loresing,
autolore, mylore, loglore, lore_perm_bonus, tclore, and spa.lic from `reference/scripts` for
`lockpicks.yaml`). **Read deeply: 14** (tpick, box-buddy, Calipers, xcalipers, recall,
recallwps, wands, 918wands, enchant, enchantcheck, ensorcell_calc, ensorcell_tracker,
lore_perm_bonus, sbox's data block); **capture-line depth: 92** (all other pile scripts and the
rest of the special asks, by the regex-diff tool in Method); **tail: 17**, each opened. Cena
HEAD 3566f8d (`git -C E:/Cena rev-parse --short HEAD`), 2026-09-25 (the baseline was cut at
9d37985). HEAD moved to a6146e6 while this ran (067b52a skinning gaps, a6146e6 eherbs); re-checked:
no file cited here changed its cited lines (`git diff 3566f8d a6146e6 -- crates/cena-behavior/src/loot/outcome.rs
plan/31-eloot-port.md` adds a skinning outcome and a table cell only). Cena files were read from the working tree.

The short answer: outside the locksmith pool **drop-off** (built, `town/pool.rs`) and the
ledger's box lines (`ledger/boxes.rs`), Cena holds **no** box, lock, trap or lockpick facts at
all, and it holds item properties (recall, analyze, inspect) only as raw text. The pile is rich
in exactly those tables, several scripts disagree on them, and tpick is already named in
`plan/30-m6-hunt.md:579-580` as a future port.

## Top findings

1. **HIGH, M6 (ledger). A box's coins collected in part are never ledgered.** tpick waits on
   `^You can only collect (.*) of the coins due to your load\.` and `^You gather (.*) of the
   coins?` (`tpick.lic:5718, 5728`); eloot on the first (`eloot.lic:5089-5094`). Cena's ledger
   reads only `You gather the remaining N coins from inside ...` (`ledger/boxes.rs:27`); the
   behavior sees `You can only collect` but keeps no figure (`loot/outcome.rs:145`). PARTIAL.
   loottracker has the same gap, so Cena matches its source; this is an improvement, not a
   regression.
2. **HIGH, M6 4c (pool). The pool worker by map tag.** tpick finds the worker and the table from
   room tags `meta:boxpool:npc:<name>` and `meta:boxpool:table:<name>` (`tpick.lic:6290-6294`);
   Cena matches eloot's eight worker names (`town/pool.rs:29-38`). PARTIAL, and already recorded
   as not built (`plan/31-eloot-port.md:285-286`).
3. **MEDIUM, M6 (ledger), INFERRED. Skin appraisals with a two-word quality may be dropped.**
   `ledger/town.rs:59` requires `is of \w+ quality`; the quality ladder has two-word steps
   (`above average`, `below average`, `very cheap`: `appraiser.lic:77-84`), and the ledger's own
   source records a real gem line `of above average quality` (`loottracker.lic:584`). The gem
   pattern already uses `.+`; the skin one does not. UNVERIFIED for skins; the corpus would say.
4. **MEDIUM, M6 (hunt). A disarmed character is not noticed.** `[Use the RECOVER ITEM command
   while in the appropriate room to regain your item.]` (`disarmbond.lic:8`) has no counterpart:
   Cena has a *creature* dropping its weapon (`combat_effects.tsv:199`) and no guard or plan
   entry for the character (greps in Method). GAP.
5. **MEDIUM, tpick port. The trap table: 16 traps, up to five states each.** Detect, disarmed,
   already disarmed, glowing from 408, set off, with the component consumed
   (`tpick.lic:3827-4131`, 69 branches), 416's different detect lines (`:4827-4866`), the
   408-safety class of each trap (`:4875-4890`), sbox's one-row-per-trap data shape
   (`sbox.lic:18-148`, SpiffyJr) and Calipers' consequences, 22 rows with six scarab kinds
   (`Calipers.lic:19-41`, Deathravin). Cena: the components as item type `lm trap` only
   (`gameobj-data.tsv:38`). GAP.
6. **MEDIUM, tpick port, CONFLICT. The lock difficulty table.** 38 descriptors in bands of 40
   (`tpick.lic:2317-2356`; ranges in `box-buddy.lic:73-111`, which shows tpick's numbers are band
   tops). Two script lineages disagree above 1355: tpick, box-buddy, lockmaster and
   pickerassistant have *unbelievably complicated* then *masterfully intricate*; Calipers,
   xcalipers and box.lic list *masterfully intricate* twice (`Calipers.lic:17`), which shifts
   the top three bands by 40 (Pick.lic has their order, without the duplicate). Cena: nothing. The duplicate is INFERRED to be the error.
7. **MEDIUM, tpick port, CONFLICT. Lockpick modifiers and the LMAS APPRAISE words.** tpick's 20
   tiers (`tpick.lic:2316`) agree with `lockpicks.yaml` on every shared material (the yaml adds
   brass). xcalipers says **veniom 2.15** (`xcalipers.lic:169`) against 2.20 everywhere else. The
   precision words disagree three ways: `somewhat accurate` is Mein 1.85 in tpick
   (`:1974-2033`) but Vultite 1.8 in Calipers and xcalipers, and tpick maps no word to vultite at
   all. Cena: nothing.
8. **MEDIUM, M6/M8. Wand to spell, and charges without numbers.** `wands.lic:21-543` (Starsworn,
   cites gswiki) maps 52 wands to their spells: **all 52 spell numbers and names agree with
   `spells.tsv`** and all 52 names classify as `wand` in `gameobj-data.tsv`
   (`tools/5-boxes-magic-items/wands_cmp.py`), so only the map itself is missing. testme's
   ten-word charge ladder (`testme.lic:194-212`: one ... almost innumerable) is missing too. GAP.
9. **MEDIUM, M6 selling / M7 / M8. Item properties are held raw, not typed.** RECALL, ANALYZE,
   INSPECT text is kept in `inventory_snapshot.rs:145` and parsed nowhere. The cleanest source to
   port a RECALL classifier from is `itemdb.lic:421-579` (Drafix, v1.7.0: enchant cap, last
   enchanter, material, ensorcell and sanctify counts, resistances, enhancive charges); recall.lic,
   testme.lic and merchantical.lic add flares, WPS, TD/DB and the value line; recallwps gives
   the WPS adjective -> CER ladder (14 steps x 4 families; two typos in the script). PARTIAL.
10. **MEDIUM, M6 (ledger). RECALL's value line is a different line.** `estimated to be worth
    about ([\d,]+) silvers` (`merchantical.lic:7311`) is not the loresong's `it's worth about`
    that the ledger reads (`ledger/town.rs:63`). GAP.
11. **MEDIUM, M6 4c follow-on. The town locksmith's return line.**
    `turns h(?:is|er) attention to .*hands your .+ back to you\.` (`spellbookscan.lic:67`,
    elanthia-online) is what the not-yet-built town locksmith (`plan/31-eloot-port.md:295-296`)
    will need; Cena reads only the pool's `here's your (.+) back` (`boxes.rs:30`). GAP.
12. **MEDIUM, M7/M8. Handing things between characters, and EMPTY.** The GIVE/ACCEPT offer lines
    (`SitNPick.lic:22-65`) and `You try to empty the contents of your .* into your .*`
    (`epop.lic:324`) have no counterpart (`grep -rn "Click ACCEPT\|empty the contents" crates` ->
    0); EMPTY moves a box's loot without a GET, so the ledger's item links would miss it
    (INFERRED).

Also recorded, lower: a CONFLICT between loottracker's `TOWN_RACIAL_BONUS` (+5 only) and
loot-be-gone2's town/race table (-25 ... +5, `loot-be-gone2.lic:1148-1235`), moot because the
author ruled trading bonuses out (`plan/34-loot-ledger.md:225-230`); `loottracker.lic` in the
mirror is the ported file (identical but for the final newline); 918wands' duplicate-result patterns were written by an
assistant and are guesses (`918wands.lic:58-80`).

## Data tables Cena lacks or differs on

### Group A: tpick and the lockpick data

`tpick.lic` is v44 by Dreaven (`tpick.lic:14-15`), no license line; plan/30 already names it
"for later, not M6 ... Not yet read" (`plan/30-m6-hunt.md:579-580`). Cena holds **no**
lock-difficulty, lockpick-modifier or trap table: `grep -rIl` over `crates` (`*.rs`, `*.tsv`,
`*.toml`) finds 0 files for `thief-lingo`, `tumblers`, `primitive lock`, `putty`; the only
`lockpick`/`scarab`/`calipers`/`plinite` hits are item *types* in `gameobj-data.tsv` and the
`Lockpick` stow slot (`containers.rs:104`). `reference/lich-5` has none of these tables either
(`grep -rln "thief-lingo\|scarab wedged\|primitive lock\|level of precision"` -> 0 files).

| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| tpick.lic:2317-2356 | lock difficulty descriptors: `"a primitive lock" => 35` ... `"an impossibly complex lock" => 1515`, plus `"You cannot even estimate its difficulty beyond being out of your league" => "IMPOSSIBLE"` | 38 + 1 | descriptor, difficulty (steps of 40) | none (searches above) | GAP | MEDIUM (tpick port, named for later) |
| tpick.lic:2316, 5198-5282 | lockpick tier modifiers: Detrimental 0.80, Ineffectual 0.90, Copper 1.00, Steel 1.10, Gold 1.20, Silver 1.30, Mithril 1.45, Ora 1.55, Glaes 1.60, Laje 1.75, Vultite 1.80, Mein 1.85, Rolaren 1.90, Accurate 2.00, Veniom 2.20, Invar 2.25, Alum 2.30, Golvern 2.35, Kelyn 2.40, Vaalin 2.50 | 20 | tier, modifier | none; `reference/scripts/scripts/lockpicks.yaml` (17 rows) | GAP (Cena); see CONFLICT row below vs the yaml | MEDIUM |
| tpick.lic:2316 vs lockpicks.yaml:1-17 | the two modifier tables | 20 vs 17 | | lockpicks.yaml | CONFLICT (coverage only): every shared material **agrees** (copper 1.00 ... vaalin 2.5). tpick adds detrimental 0.80, ineffectual 0.90, mein 1.85, accurate 2.00 (non-material precision tiers); the yaml adds `brass: 1.00`, which tpick lacks (tpick has `Repair Brass` wire, `tpick.lic:415, 2283`, but no brass pick tier). The yaml cites gswiki `Lockpicks` (`spa.lic:1277`) | MEDIUM |
| tpick.lic:1974-2033, 2166 | LMAS APPRAISE precision word (`It seems to have an? (.*) level of precision and (?:is\|has) (.*)\.`) -> tier: detrimental, ineffectual, very inaccurate (copper), inaccurate (steel), somewhat inaccurate (gold), inefficient (silver), unreliable (mithril), below average (ora), average (glaes), above average (laje), somewhat accurate (mein), favorable **and** advantageous (both rolaren), accurate, highly accurate (veniom; invar when strength `incredibly strong`), excellent (alum; golvern when `astonishingly strong`), incredible (kelyn), unsurpassed (vaalin) | 18 words | precision, strength tiebreak, tier | none | GAP. Note: no word maps to vultite; `favorable` and `advantageous` both map to rolaren. INFERRED that one of them is vultite's (1.80) word and this is a tpick bug; UNVERIFIED | MEDIUM |
| tpick.lic:661 (tooltip) | Lock Mastery tricks and ranks: spin 1, twist 10, turn 20, twirl 30, toss 40, bend 50, flip 60, random 60 | 8 | trick, LM ranks | `psm.rs` holds PSMs, not guild skills; grep `ptrick` -> 0 | GAP | LOW |
| tpick.lic:432-449, 3827-4131 | trap types: Scarab, Needle, Jaws, Sphere, Crystal, Scales, Sulphur, Cloud, Acid Vial, Springs, Fire Vial, Spores, Plate, Glyph, Rods, Boomer (+ No Trap) | 16 | per trap: detect / disarmed / already disarmed / 408-glowing / set off messages; component consumed (putty, cotton, vial) | `gameobj-data.tsv:38` (`lm trap`: the trap *components* as items) | GAP for the messages; HAVE for the component item type | MEDIUM |
| tpick.lic:4875-4890, 4320-4336 (water lore) | 408 safety class per trap: safe to skip (Needle, Jaws, Plate); 408-safe (Crystal, Springs); 408 may trigger (Scarab, Sphere, Scales, Acid Vial, Fire Vial, Spores, Boomer, Cloud, Rods); 408 always triggers (Sulphur); Glyph unopenable | 16 | trap, class | none | GAP | MEDIUM |
| tpick.lic:2359-2368, 6577-6588, 3812, 6002-6041 | the picking maths: pick lore = min(level/2 + LP bonus/10 + dex bonus + MnE ranks/4, LP bonus); disarm lore likewise on DT; LM focus lore = 2*LM ranks + dex/2; total pick = (skill+lore)*2.5; 404 when trap diff + 40 > disarm skill; water-lore rot bonus = floor((sqrt(1+8*ElWater)-1)/2), corrosion capped at 10% of base, lasts 60 s | 6 formulas | | `skills.rs:196` has `DisarmingTraps` as a skill only | GAP | LOW (a policy, not a fact; tpick's own estimate) |
| tpick.lic:4649 | rogue guild toolbench wire price list, read off `read sign`: `N.) a thin <material> wire  <cost>` | per town | order, material, cost | none | GAP | LOW |
| tpick.lic:6290-6294 | map tags `meta:boxpool:npc:<name>` and `meta:boxpool:table:<name>` locate the pool worker and table | 2 tags | | `town/pool.rs:29-38` uses eloot's 8 worker names; `plan/31-eloot-port.md:285-286` records the tag as **not built** ("the round has no map tags") | PARTIAL (known, recorded) | HIGH (M6 4c; the pool's worker by tag, not name) |

### Group B: the other box scripts, where they add to or contradict tpick

| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| Calipers.lic:17 vs tpick.lic:2351-2355 (also box-buddy.lic:107-111) | the order of the top five lock descriptors. tpick, box-buddy, lockmaster, pickerassistant, old-tpick: absurdly difficult (1355), **unbelievably complicated (1395), masterfully intricate (1435)**, absurdly complex (1475), impossibly complex (1515, box-buddy `1480+`). Calipers, xcalipers (`:210-211`), box.lic (`:225-226`): absurdly difficult, **masterfully intricate, unbelievably complicated, masterfully intricate** (listed twice), absurdly complex, impossibly complex, so every band from 1360 up is shifted by 40; Pick.lic (`:78`) has the same order but stops at unbelievably complicated, without the duplicate | 38 vs 39 | descriptor, band | none | CONFLICT between the two script lineages (command: `grep -lE "impossibly complex\|unbelievably complicated" *.lic`, 14 files). The duplicate in the Calipers lineage looks like the error (INFERRED); the corpus would settle it (ask the author first) | MEDIUM |
| box-buddy.lic:73-111, Calipers.lic:123,168 | lock bands as ranges: `"a primitive lock" => "5-35"` ... `"an absurdly complex lock" => "1440-1475"`, `"an impossibly complex lock" => "1480+"` | 38 | descriptor, min, max | none | GAP; confirms tpick's single values are each band's **top** (tpick 315 = box-buddy 280-315); spa.lic's own sample `a simple lock (-295 ...)` (`spa.lic:1244`) falls inside it | MEDIUM |
| Calipers.lic:42-61 vs tpick.lic:1974-2033 vs xcalipers.lic:85-104 | precision word -> multiplier | 18 | | none | CONFLICT (three-way): `somewhat accurate` is Vultite 1.8 (Calipers, xcalipers) but Mein 1.85 (tpick); `advantageous` is 2.0 (Calipers, xcalipers) but Rolaren 1.90 (tpick); `accurate` is 2.2 (Calipers, xcalipers) but 2.00 (tpick); `highly accurate` 2.25 (Calipers) / 2.2 (xcalipers) / 2.20 or 2.25 by strength (tpick). The Calipers reading also fixes tpick's missing vultite word (above) | MEDIUM |
| xcalipers.lic:106-198 | off-the-shelf lockpicks: name, multiplier, strength index, LM ranks to make, price, precision word, 18 rows (copper, brass, steel, gold, ivory, silver, mithril, ora, glaes, laje, vultite, rolaren, veniom, invar, alum, golvern, kelyn, vaalin) | 18 | 6 | none | GAP; and CONFLICT on **veniom: 2.15** here (`:169`, `:200`) against 2.20 in tpick (`:2316`), box-buddy (`:65`) and lockpicks.yaml (`:12`) | MEDIUM |
| Calipers.lic:62-63, xcalipers.lic:79-82 | lockpick strength words (12: flimsy ... unsurpassed) and condition words (Calipers 6: miserable ... excellent; xcalipers 7, adding `broken`) | 12 + 7 | ordinal | none | GAP; CONFLICT on `broken` (minor) | LOW |
| Calipers.lic:64-81 | per-material standard pick: precision, strength, condition, where sold (town codes WL SH IM TI TV TE), materials that make it | 17 | 6 | none | GAP | LOW |
| Calipers.lic:19-41 | trap consequences: name, failure effect, self damage, hits others, LM mastery ranks, safe on fail, 408-safe, notes, detect regex; 22 rows incl. the picking-contest test trap and **six scarab kinds** (blue: disease; green: poison; opalescent: spell, huge CS; onyx: death, lingers; red: bleeds; translucent: Evil Eye 1000+ CS) | 22 | 9 | none | GAP; its `408` column says Rods `No` where tpick treats Rods as "408 might set off" (`tpick.lic:4881`); partly CONFLICT, UNVERIFIED which is right | MEDIUM (a hunt that keeps boxes wants to know which trap kills the room) |
| sbox.lic:18-148 | trap table as data: type, detect, success, inert, component (vial, crystal, jaws, needle, sphere), tools (knife for scales, lockpick for sphere), 17 types incl. `contest` | 17 | 6 | none | GAP; the cleanest shape to port (one row per trap) | MEDIUM |
| lockpick-maker.lic:182-216, 291-308 | rogue guild toolbench order numbers and costs per lockpick material (copper 26 / 80 ... vaalin 41 / 100000), and edging material costs (copper 20 ... rhimar 5000) | 16 + 18 | material, order, cost | none | GAP (one town's menu, INFERRED) | LOW |
| rogue.lic:744-1304 | rogue guild training tasks: `The Training Administrator told you to <task>` (28 distinct task texts, `grep -oE "Training Administrator told you to [^/\|.]*" rogue.lic \| sort -u`), rank messages, task completion | 28 | task text | `grep -rn "Training Administrator" crates` -> 0 | GAP | LOW (no guild behavior on the roadmap) |

### Group C: merchantical, recall, lore, enchanting, ensorcelling (special ask 2)

Cena **holds** the raw text of `recall`, `analyze`, `inspect` and `look` when the game sends
them as `<inventoryViewItem>` sections (`inventory_snapshot.rs:145-148`,
`cena-protocol/src/frame/payload.rs:471-486`) but **parses** none of it: `grep -rIl` over
`crates` finds 0 files for `As you recall`, `imparts a bonus`, `persist after`,
`crumble into dust`, `estimated to be worth`. What it reads today is `analyze`'s transmog and
`ALTER 41` lines (`town/reply.rs:76-80`), the loresong value (`ledger/town.rs:62-63`) and the
enhancive **totals** (`character/enhancive.rs`, a different command).

| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| recall.lic:40-257, 304 (Alastir, 2021, no license) | RECALL vocabulary: enchant `It imparts a bonus of \+(.*) more than usual`; defender `It helps defend its wielder with a bonus of`; TD `It protects against magical attacks with a bonus of`; temporary `When its enhancement has degraded away ...`; weighting/padding; resists `It is (.*) resistant \((.*)\%\) to (.*) attacks`; sighted; holy; flares `It has been infused with the power of (.*)` (8 kinds listed in comments `:107-114`); mana flare `infused with mana \(\+(.*)\)`; acuity `(\+(.*) AS\/\+6 CS)`; enhancive boosts `It provides a boost of (.*) to (.*) (Base\|Bonus\|Ranks)`, Max Health/Mana, Health/Stamina/Mana Recovery; persist/crumble/disintegrate; imbeddable; `It could be activated by (waving\|rubbing\|tapping\|raising) it`; charges (`It has N charges`, `a few`, `a fair amount`, `a lot`, `does not have any charges left`); training requirement `may not be used by adventurers who have not trained (.*) times`; materials glaes, veil iron; plus ~15 boilerplate lines in `:304` | ~40 | property, figure | `inventory_snapshot.rs:145` (raw text only) | PARTIAL (held raw, not typed) | MEDIUM (M6 selling: eloot's keep rules key on these; M8 highlights) |
| merchantical.lic:684-735 (Kyrandos, v1.6.0 2026-09-18) | RECALL -> typed fields: weight (`It is a small item, under a pound` = 0.1, `It appears to weigh about ([\d.]+) pound`), enchant (4 phrasings), charges, persistent, disintegrate, spiked, enhancive, crumbly, imbedded spell (`currently imbedded with (?:the )?(.+?)\s+spell`), object type by text then noun (`:612-667`) | 10 fields | | none | GAP | MEDIUM |
| merchantical.lic:7301-7311 | RECALL value and lock state: `estimated to be worth about ([\d,]+) silvers`; `must reveal the entire loresong\|you have not yet unlocked` | 2 | text | `ledger/town.rs:63` reads LORESONG's `it's worth about ([\d,]+) silvers` only | PARTIAL: the recall form of the value is a different line and is not ledgered as an appraisal | MEDIUM (M6 ledger: plan/34 appraisals) |
| merchantical.lic:343-523 | player-shop furniture capacity catalog: `'rough oak crate' => [10, 'in']` ... by shop (Barter/Trade Alley, Cort's Emporium, Turpin's) | 167 `=>` lines in the block (`awk 'NR>=343 && NR<=523' merchantical.lic \| grep -c "=>"`, includes comments' neighbours; INFERRED about 150 rows) | name, capacity, preposition | none | GAP | LOW (player shops, no milestone) |
| merchantical.lic:305-340 | noun families: WEAPON, JEWELRY, MAGIC_ITEM, SHIELD, CLOTHING, CONTAINER as regexes | 6 | | `gameobj-data.tsv` types `weapon`, `jewelry`, `magic`, `clothing`, `container`; `armament_aliases.tsv` (707 lines) | PARTIAL: Cena's are Lich's own tables, fuller; merchantical's are hand lists. Not worth porting | LOW |
| merchantical.lic:5033-5063 | pawnshop sale terminal and refusal phrases (`hands it back`, `not interested`, `worthless`, `wouldn't give`, `find a buyer`, `shakes.*head`, 25k confirm `resell it again`, note `scribbles out ... hands it to you`) | ~17 | | `town/reply.rs:64-71` (six refusals), `ledger/town.rs` (sales, notes) | PARTIAL: `find a buyer`, `wouldn't give`, `shakes ... head`, `resell it again` are not in `reply.rs` (grep of each -> 0); these are guessed alternations in merchantical, UNVERIFIED as real lines | LOW |
| recallwps.lic:13-131 (Giantphang) | weighting/padding adjective -> CER: lightly 1-2, fairly 3-4, somewhat 5-6, decently 7-8, heavily 9-10, very heavily 11-13, exceptionally 14-15, masterfully 16-20, superbly 21-25, expertly 26-30, phenomenally 31-35, fantastically 36-40, incredibly 41-45, wonderously 46-50; four families (padded to lessen damage, padded against critical blows, weighted for critical wounds, weighted for damage) | 14 x 4 | adjective, CER min, max | none (`grep -rn "wonderously\|CER" crates` -> 0) | GAP. Two typos in the script: `lightly padded padded against` (`:41`) never matches, and `lightly weighted ... critical wounds` appears where `lightly weighted ... damage` should (`:97`) | MEDIUM (WPS is an armament fact; `armaments.rs` has the stat tables it modifies) |
| enchantcheck.lic:47-203 (Vailan, 2020) | 405 Elemental Detection aura -> enchant: colour red 5, orange 10, yellow 15, green 20, blue 25, indigo 30, violet 35, copper 40, silver 45, gold 50; shade faint -4, muted -3, hazy -2, soft -1, vibrant 0; the ayan'eth potion ladder (<20 undiluted green 10,000; <25 blue 25,000; <30 indigo 50,000; <35 violet 75,000; <40/45/50 dilute copper/silver/golden, event only; 5 casts per potion below 40, 1 above) | 10 + 5 + 7 | | `spells.tsv` 405 has no message fields | GAP | LOW (a wizard service; the colour ladder also decodes 405 for any caster) |
| enchantcheck.lic:218-220, enchantnotes.lic:116, 215, enchant.lic:10-23 | enchanting lines: `You recognize the ? ? aura surrounding it as indicating a ...`; `seems to have been prepared for enchanting, and may be enchanted up to a bonus of N`; `You notice no aura indicating enchantment.`; temper `should be ready to (enchant\|enchant in about)? N to M (days\|hours)`; potion poured `...slowly coalesce into (a pair of )?small softly glowing runic symbols`; `suffuse estimate` -> `The service does not require any additional suffused energy.`; 925 channel `Success!` | 7 | | none | GAP | LOW |
| ensorcell_calc.lic:77-151, 177-251 (Xanlin, v5 2020) | ensorcell (735) difficulty: per-material modifier by item type (weapon, shield, armor, bow, runestaff; 72 materials, adamantine -500 ... mithril +20 ... zelnorn -999); script penalties (energy -250, sigil -200, sprite -250, karma -200); enchant penalty `(trunc(enchant/3))^2` (GM Estild, 2020-02-23); tier penalty `-50 - 50*(tier-1)`; CER^2; flares -100; enhancive -200; sanctified -200; caster bonus (sorc ranks x2 to level+1, WIS, INT, EMC/SMC, AS/10, MIU/10, workshop +20). Cites gswiki `Research:Ensorcell_(735)_Formula` | 72 + 4 + 8 | | none (`grep -rIl -i material crates` finds only combat/crit text) | GAP. The material table is the one reusable fact (materials also drive lockpicks and forging) | LOW |
| ensorcell_tracker.lic:19-28 | ensorcell flare payoff: `You feel (healed\|empowered\|rejuvenated\|reinvigorated)` (health, mana, spirit, stamina) | 4 | | `combat_effects.tsv:41-42` has the `ensorcell` flare **announce** (`Necrotic energy from your .+? overflows into you!`) but not these payoff lines (`grep -rn "empowered\|rejuvenated" crates/cena-model/data` -> only spells.tsv:435, unrelated) | PARTIAL | LOW (combat recorder completeness) |
| mylore.lic:1612-1632, loresang.lic:219-270, lore.lic:88-105 | loresing outcomes: `As you sing, you feel a faint resonating vibration`; `but it simply resonates with what you previously learned.`; `You feel as though you have reached the end of`; `Your song is weak, and without sufficient power`; `has more to share, but your song simply wasn't powerful enough to glean more.`; `falters and fades`; `permanently unlocked`; `You open your mouth, but find yourself unable to sing` | 8 | | `ledger/town.rs:62-63` (start and value only) | PARTIAL | LOW (bard only) |
| lore_perm_bonus.lic:22-33 (Gnomad) | loresong permanence bonus: bard ranks to level x1; ranks past level, MnE telepathy, AUR and INF bonus x1/2; MIU, EMC, MMC x1/4 | 1 formula | | none | GAP | LOW |
| spellbookscan.lic:67, 82-83 (elanthia-online, v1.2.0 2026-09-10) | the **town** locksmith handing a box back, and a spell specialization book: `turns h(?:is\|er) attention to .*hands your .+ back to you\.`; `You analyze (?:a\|an\|the\|some\|your) (.+?) and sense that the creator has provided`; `add (\d+) spell specialization ranks? (?:to casting\|for) the spell (.+?) \((\d+)\)` | 3 | text | `boxes.rs:30` has the **pool's** return only; `plan/31-eloot-port.md:295-296` records the town locksmith as not built | GAP | MEDIUM (M6: when the town locksmith lands, its return line is this) |

### Group D: wands and scrolls (special ask 3)

| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| wands.lic:21-543 (Starsworn "with assistance from Claude AI", v1.2; cites gswiki `Wand#List_of_wands`) | wand -> spell: name, spell, spell number, crumbly, treasure, alchemy, circle, notes | 52 | 8 | `spells.tsv` (the spells), `gameobj-data.tsv:80` (`type wand noun ^(baton\|rod\|wand)$`) | GAP for the wand->spell map. **Checked against Cena: all 52 spell numbers exist in `spells.tsv` and every name agrees (0 mismatches); all 52 names classify as `type:wand`** (`tools/5-boxes-magic-items/wands_cmp.py`). Seven also classify `uncommon` by material (mithril, rainbow glaes, bronze, drake, rowan, gnarled yew, grooved witchwood), which is Lich's behaviour too | MEDIUM (M6/M8: a hunt that fires wands, bounty/heal with 1106/1108; the table is small and clean) |
| gameobj-data.tsv:57 vs wands.lic | which wands go to consignment | 6 | | `sellable consignment name` lists faceted topaz wand, slender mithril wand, wavy grey crystal wand, shadowy dark crystal wand, slender azure rod, slender crimson rod | HAVE (Cena's sell routing already covers the alchemy wands wands.lic marks `alchemy: true`) | - |
| 918wands.lic:58-80 (author "Claude Code, RuseOfFools") | 918 Duplicate results: `grab the copy\|splits into two, and slowly separates`; `not well suited for duplicating\|cannot be duplicated`; `puff of smoke\|is destroyed\|disintegrat\|consumed in\|explodes`; `must be holding the object to be duplicated in your right hand` | 4 | text | `spells.tsv` 918 (no messages) | GAP, and **low trust**: the alternations are guesses written by an assistant (`disintegrat\|consumed in\|explodes`); only `not well suited for duplicating` and `puff of smoke` are confirmed by an independent script (`wanddupe.lic:46, 54`, Caithris: `feeling a little lighter\|disappears into a puff of smoke`) | LOW |
| 918wands.lic:84-86 | society mana abilities by spell number: Sign of Wracking 9918 (CoL), Sigil of Power 9718 (GoS), Symbol of Mana 9813 (Voln) | 3 | | `societies/col.rs`, `sunfist.rs`, `voln.rs` | HAVE: `col.rs:290` (9918), `sunfist.rs:271` (9718), `voln.rs:301` (9813), and `spells.tsv:380,395,425` (`grep -rn "9918\|9718\|9813" crates/cena-model`) | - |
| scrollbuff2.lic:235-357, scroll.lic:8-23 | scroll handling: `You rummage through .+ and remove .+ with .+ scrawled upon it`; `but can't seem to locate anything with that spell on it`; INVOKE `As you begin your invocation the ink leaps from the page and seeps into your skin.`; `and gesture to invoke the`; `That spell is not on the`; `Invoke from what` | 6 | | `town/reply.rs:115-122` reads a scroll's `(215) Frenzy` line for the keep list; nothing for RUMMAGE or INVOKE (`grep -rn "scrawled\|ink leaps\|Invoke from" crates` -> 0) | GAP | LOW (scroll buffing is not on the roadmap) |
| librarian.lic:66-68 | scroll read: `^\((\d+)\)\s+(.+?)(?: in vibrant ink)?$`; grimoire slot `^(\d+)\.\s+(?:The (.+?) \((\d+)\) spell with (\d+) charges remaining\.\|This section is blank\.)$` | 2 | | `reply.rs:115-122` (`vibrant: has("vibrant")`) | HAVE for the scroll line; GAP for grimoires | LOW |
| readscrolls2.lic:104 | in XML the scroll's spell line carries the number as the link's noun: `noun="([0-9]+)">([^<]+)<` | 1 | | `reply.rs` reads the flattened text | INFERRED HAVE (the flattened text keeps `(215) Frenzy`); UNVERIFIED that the link noun is the spell number, which would be a Rule 2.2a fact worth a fixture | LOW |

### Group E: looting, inventory and item-information scripts

| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| loottracker.lic (Nisugi, v0.2.1) | the ledger's source | 5,013 lines | | `plan/34-loot-ledger.md:4,20` ports `reference/scripts/scripts/loottracker.lic` | HAVE: the mirror's copy is the **same file** (`diff` of the two after `tr -d '\r'` differs only in the final newline) | - |
| loot-be-gone2.lic:1148-1235 (Alastir, v0.14) vs loottracker.lic:49-60 | shop price adjustment by town and race: trading bonus `(Trading bonus + INF bonus)/12`, then per town a race list of -25 ... +5 (e.g. Ta'Illistim/Ta'Vaalor: Dark Elf -25; Dwarf, Half-elf, Half-krolvin -15; Human -10; Elf +5) | 12 towns | town, race, adjustment | the ledger reports the cap and does **not** compute bonuses (`ledger/report.rs:17-18`; author's decision `plan/34-loot-ledger.md:225-230`) | CONFLICT between the two scripts: loottracker's `TOWN_RACIAL_BONUS` has only +5 favoured races (e.g. Landing: Giantman, Halfling, Half-Elf, Dark Elf, Forest Gnome +5), loot-be-gone2 has Landing Dark Elf/Forest Gnome **-5** and Half-krolvin -25. loot-be-gone2's Landing branch is dead code (`elisf` typo, `:1158`) and it spells `Aeoltoi` (`:1169`). Recorded, not a port candidate: the author ruled the arithmetic out | LOW (decided) |
| testme.lic:194-212, 1284-1302 (Dreaven, v60) | charge-count words, ten steps: one, a few, several, a fair amount, quite a few, a lot, more than your average dwarf could count, more than your average giantman could count, a huge number, almost innumerable | 10 | ordinal | none (`grep -rIlF "charges remaining" crates` -> 0) | GAP | MEDIUM (a wand/item's charges without a number; pairs with the wand table above) |
| testme.lic:103-1413 | the item-test vocabulary across LORESING, INSPECT, ASSESS (warrior), RECALL, APPRAISE, ANALYZE: flares by element (12 words), acuity, mana flare, anti-magic, TD `+(\d+) Target Defense`, DB `+(\d+) Defensive Bonus`, enchanted by, enhancive boosts and training restriction, charges ladder, persist/crumble/shatter, sighted, holy/blessed, merchant alteration (lighten, deepen pockets, `(\d+) difficulty ... to modify`), Voln armour `This item is currently Tier (\d+) \(of (\d+)\) with (\d+) \(of (\d+)\) spider\(s\)\.`, armour coverage by group (soft/rigid leather, chain, plate x 4 coverages, `:784-829`), wear locations, container capacity `can store an? (.*) with enough space` / `with a maximum interior capacity of`, loresong purpose `From the pitch of the vibration you determine that the purpose ...`, gem chambers, appraise with weight and quality `about (.*) silvers, and is of (.*) quality` | ~150 patterns | | `inventory_snapshot.rs:145` (raw text); `armor.tsv` (`armor_group`, `armor_sub_group`: the ASG that fixes coverage, not the INSPECT text) | PARTIAL (held raw) and GAP (typed) | MEDIUM (M6 selling keep rules; M8 highlights; M7 an agent asking "what is this item") |
| itemdb.lic:421-579 (Drafix, v1.7.0) | RECALL, typed, the most recent and cleanest set: `empowered and can be charged for an additional (\d+)`; `It seems to have (.+?) enhancive charges? remaining\.`; `It may be enchanted up to a bonus of (\d+)`; `last enchanted by (.+?)\.`; `It is predominantly crafted of (.+?)\.`; `It is an? (\w+) project \((\d+) difficulty\) for an adventurer to modify\.`; `It is magic resistant\.`; `It (?:is\|appears to be) resistant \((\d+)%\) to (.+?)\.`; `It has been ensorcelled (\d+) times?\.`; `It has been sanctified (\d+) times?\.`; `It has a permanently unlocked loresong by ...`; INSPECT slot `You determine that you could (?:wear\|pin\|attach\|clip\|tie\|fasten) (.+?)\.`; spell specialization ranks | 25 | property, figure | none typed (`grep -rIlF` for `ensorcelled`, `sanctified`, `predominantly crafted`, `enhancive charges` -> 0 each) | GAP | MEDIUM (the best single source to port a RECALL classifier from; author, version and anchors are all present) |
| isharpie.lic:759-880 (Kyrandos) | RECALL WPS phrasing variants: `is (\w+(?: \w+)?) padded to lessen the damage`, `... padded against critical`, `... weighted to inflict`, `... sighted`; `exactly how much requires assessment`; `unknown (scripted) benefit`; `rechargeable` | 10 | | none | GAP (overlaps recallwps/testme) | LOW |
| inventory-buddy.lic:578-689 (Dreaven) | INV and locker and bank lines: `^You are carrying nothing at this time.`; `You are holding nothing at this time.`; locker `Looking in front of you, you see the contents of your locker in (.*?)\:` / `Thinking back, you recall the contents of your locker in (.*?)\:` / `There are no items in this locker.`; `You currently have no open bank accounts\.`; bounty points `(.*?)/50,000 \(Weekly\)\s+(.*?)/200,000 \(Total\)`; currencies `You take a moment to recall the alternative currencies you've collected...` | 9 | | `bank.rs:341-345` (`opens_account` reads `...amounts on deposit` and `...have an account`, not `no open bank accounts`); `standing.rs` has `(Weekly)`; locker lines: 0 hits | PARTIAL (bank: the no-account answer leaves the bank model unknown rather than known-empty); GAP (locker manifest) | MEDIUM (M7: an agent asking what a character owns; M6 bank) |
| invdb-beta.lic:667, 4992, 3225 (Xanlin) | locker capacity `Your locker is currently holding (\d+) items? out of a maximum of (\d+)`; oremonger storage `ores you have stored`; LOCATION `You carefully survey your surroundings and guess that your current location is (.*?) or somewhere close to it\.` | 3 | | none (`grep -rIlF "locker is currently holding\|survey your surroundings"` -> 0) | GAP | LOW |
| gtk3-sloot.lic:1174-1213 (same in shlooter, slootM) | ammunition bundles: `Individual projectiles from this bundle will have a (?:show\|long) of "(.*)"\|Each individual projectile will be "(.*)"`; `a strength of (\d+) and a durability of (\d+)` | 2 | | none | GAP | LOW (archers; M6 hunts do not gather arrows yet: `grep -n -i "arrow\|ammo" crates/cena-behavior/src/loot/*.rs` finds none) |
| loot.lic:1012 (Tillmen, v0.26), gtk3-sloot.lic:2166-2170 | special search yields: `You plunge your hand into .*? withdraw (?:your arm to find )?a .*? (\w+) (?:in your grasp\|at the cost of freezing your hand)!`; `withdraw a (?:cold blue gem\|fiery red gem)`; `you withdraw your arm to find a pungent piece` | 3 | | `loot/outcome.rs` (`You plunge` -> `Searched`) | PARTIAL: the search is recognised, the gem it yields is not ledgered as found (`grep -rn "cold blue gem\|pungent piece" crates` -> 0) | LOW |
| loot-be-gone2.lic:797, shlooter.lic:695-701 | disk arrival: `^Your disk arrives, following you dutifully\.`; `^A small circular container suddenly appears` | 2 | text | `disk.rs` (names, not arrival; `grep -rn "Your disk arrives" crates` -> 0) | GAP | LOW (the room feed shows the disk anyway) |
| gemguard.lic:69-127 (Mystienne, v0.21.2) | protected gems: 47 nouns (`aetherstone` ... `wyrdshard`) and 106 names (`aster opal` ... ) never sold | 153 | | `gameobj-data.tsv` `valuable` and `gem` types | N/A: a player's keep list, not a game fact | - |
| gemguard.lic:57-67 | gem LORESING and 704 and chrism tests: pure orb `resonate with your voice.*draw power from you\|pulses strongly with the rhythm`; chrism `holy receptacle\|restorative magic\|grants a quicker recovery to the deceased`; chrism blessing `spiritual bond\|too crude to accept your blessing\|blows away in the form of a fine powder` | ~12 | | none | GAP | LOW |
| shatventure.lic:181-215, 240-246, 524, 620, 999 | Shattered bounty and boost lines: `Minor Loot Boosts:\s*(.*)`, `Major Loot Boosts:`, `Bounty Boosts:`, `You have activated a Major Loot Boost.`; debt `In the back of your mind you remember you owe a debt`, `You heard the clerk!  Pay off that debt first.` | 7 | | `bounty.rs` has the task texts (e.g. `:300`, `:390`); boosts and debt: 0 hits | PARTIAL | LOW (Shattered only) |
| gibs_pickpocket_helper.lic:201 | pickpocket success: `^You reach into ([A-Z][a-z]+)'s (.*?)\sand pull out (.*?)\.$` | 1 | text | none | GAP | LOW |
| huntsuggest.lic:104 | hunting areas by level: read at run time from a Google Sheet (`SHEET_URL`), not held in the script | - | | `creature_areas.tsv` (1,395 lines) | N/A (no data in the mirror) | - |

### Group F: forging and crafting

| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| gforge.lic (Mikefence, v1.92 2025-01-05), forge-perfects.lic (Moredin, Tillek) and its two copies | forging product names, slab-cutter lines (`You've just set the slab-cutter to cut your .* into a (\d+)lb\. piece and a (\d+)lb\. piece`), material weight `you determine it would be necessary to have (\d*) pounds of`, supply-shop order menus, forging rank `In the skill of forging - .* you are a master` | ~130 new patterns | | none | GAP | LOW (no crafting milestone). `fixed-forge-perfects` and `forge-perfects-fixed` differ from `forge-perfects` only in a `multifput` call, a dropped `redo` and one constant (`left = 25` vs `26`) (`diff` after `tr -d '\r'`) |
| chrismforge.lic:158-248 (Mystienne) | chrismable gem nouns and names, chrism quality and fullness notes, chrismarium requirements | ~8 tables | | none | GAP | LOW |
| smelter.lic, townsmith.lic | smelting crucible and town smith lines | 15 + 16 | | none | GAP | LOW |

## Captures Cena lacks or differs on

### Group A: tpick

| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| tpick.lic:5712-5733 | coins from a box, the partial forms | `^You can only collect (.*) of the coins due to your load\.`; `^You gather (.*) of the coins?`; `^You cannot hold any more silvers\.` | ledger `boxes.rs:27` reads only `^You gather the remaining ([\d,]+) coins from inside ...`; the **behavior** side has the first and third (`loot/outcome.rs:145`) but emits no silver figure | PARTIAL: a partial gather is never ledgered, so `;loot` under-counts box silver when the character is loaded | HIGH (M6, plan/34 ledger) |
| tpick.lic:5702 | fossil charm | `You summon .* They locate a pile of (.*) coin` | `boxes.rs:28-29` (`^You summon a swarm of .+ from your .+ reclaiming them`, `locate a pile of ([\d,]+) coins, reclaiming them`) | HAVE | - |
| tpick.lic:6378-6405 | pool returns (customer side) | `We don't have any boxes ready for you`; `here's your .* back`; `"You need to lighten your load first."` | `reply.rs:106-110`, `boxes.rs:30` | HAVE | - |
| tpick.lic:3795 | trap difficulty from DETECT | `\(.*\-(\d+)\)\.` (e.g. `(-123).`) | none | GAP | MEDIUM |
| tpick.lic:3821-3825 | box under examination, and a failed disarm | `You carefully begin to examine (.*) for traps\.\.\.`; `Having discovered a trap on the.* you begin to carefully attempt to disarm it\.\.\.` | none | GAP | MEDIUM |
| tpick.lic:3827-4131 | per-trap messages, 16 traps, up to 5 states each (detect, disarmed, already disarmed, 408-disarmed, set off), e.g. Scarab `Peering closely into the lock\, you spy an? [a-zA-Z]+ (.*) scarab wedged into the lock mechanism\.`; Needle `Using a bit of putty from your.*\, you manage to block the tiny hole in the lock plate\.`; Glyph `You notice some spiderweb\-like scratches on the lock plate ...` | 69 `elsif` branches (`sed -n 3827,4131p tpick.lic \| grep -cE "elsif line =~ /"`), several with alternations | none (`grep -rIl "tumblers\|putty"` -> 0) | GAP | MEDIUM |
| tpick.lic:4132-4145 | no trap; box already open; no putty | `You discover no traps\.`; `Um, but it's open\|There is no lock on that`; `You figure that if you had some sort of putty` | none | GAP | MEDIUM |
| tpick.lic:4827-4866 | 416 Piercing Gaze detection, one line per trap (different text from manual DETECT), and a failed cast | e.g. `You see a cord stretched between the lid and case\.` (Scales); `You gaze at the.*but your vision is obscured\.` | `spells.tsv` row 416 has no message fields (instant spell) | GAP | LOW (416 is a sorcerer box-popping tool) |
| tpick.lic:2517-2555, 4807-4810 | 407 Unlock results | `vibrates slightly but nothing else happens\.`; `corroded the lock` (water lore); `suddenly flies open\.`; `is already open\.`; pop variant `Suddenly\, part of the.*face breaks away and a pair of gleaming jaws snap shut before the lockplate` | `spells.tsv` 407: no messages | GAP | LOW |
| tpick.lic:4697-4726, 4206 | 408 Disarm results | `...The.*pulses once with a deep crimson light\!` (disarmed); `...vibrates slightly but nothing else happens\.` (failed); `...the entire.*explodes in a deafening\, fiery detonation\!` (set off); `The runes on the scarab go still\.` (scarab) | `spells.tsv` 408: no messages | GAP | LOW |
| tpick.lic:5548-5560 | 704 Phase on a box: glyph test | `resists the effects of your magic` (glyph) vs `appears lighter\|then stabilizes\|but quickly returns to normal` | `spells.tsv` 704 has self-cast messages only | GAP | LOW |
| tpick.lic:3406-3600 | lock picking outcomes | roll `^You make .* attempt \(d100(?:\(open\))?\=(\d+)\)\.`; opened `...\(\-(\d+) thief\-lingo difficulty ranking\)\.  Then\.\.\.CLICK\!  It opens\!`; read `About a \-(\d+) difficulty lock \(in thief\-lingo\)\.`; bent `...end up bending the tip\!`; broken `^\* SNAP \*  Crud\!  You broke your .* in the attempt\!`; gnomish bracer save `Tiny .* shoot out of your .* to steady your .*` | none | GAP | MEDIUM (tpick port) |
| tpick.lic:5583-5663 | calipers and loresong lock measure | `\-(\d+) in thief\-lingo difficulty ranking`; any lock descriptor; trapped `You place the probe in the lock and grimace as something feels horribly wrong` (every soul golem box, per the script's comment); `As you start to place the probe in the lock` (undetected trap); `but your song simply wasn't powerful enough` (loresong failed); `has already been unlocked\|isn't even closed` | none | GAP | MEDIUM |
| tpick.lic:2583, spa.lic:1191-1192 | calipers calibration | tpick: `You're good, but you're not that good.` / `You should leave them alone.`; spa: `They practically glow with calibration!` / `Those calipers could not be more perfectly calibrated.` | none | GAP (two scripts, two message sets; both are "done") | LOW |
| tpick.lic:2728-2738 | LM wedge | `suddenly splits away from the casing\|Why bother`; `What do you expect to wedge it with`; `Please specify an appropriate skill.` | none | GAP | LOW |
| tpick.lic:5525-5540, 3346-3395 | plinite extraction | `It looks like it would be.*\(\-(\d+)\)\.`; `You promptly discover that the core has already been removed\.`; `...merely needs to be PLUCKed from the tip of the shard\.`; `you push just a little too hard and rupture the core\!` | `gameobj-data.tsv:49` (type only) | GAP | LOW |
| tpick.lic:5074-5105, 5469-5481 | locksmith pool, **picker** side: job offer with tip, critter and level; the wait ladder (10 s / 1 / 2 / 10 min, rest your mind, no jobs); payment | `says\, \"Ah\, here we are\.  The client is offering a tip of (.*) silvers? and mentioned it being from [a-zA-Z]+ (.*) \(level (\d+)\)\.`; `"That's some quality work.  Here's your payment of (.*) silvers?."`; `Too tough for ya, eh?` | Cena is the customer only (`town/pool.rs`); ledger has no picker income | GAP | MEDIUM (a picker's income; the job line also names the critter and its level) |
| tpick.lic:4972-4989 | locksmith's container stock | `lump of squishy white putty with about (\d+) pinch`; `(\d+) little ball`; `(\d+) vials? of liquid`; `unlimited amounts` | none | GAP | LOW |
| tpick.lic:2411-2420 | GLANCE for a mithril/enruned box in hand | `You glance down to see.*(mithril\|enruned\|rune-incised)`; `You glance down at your empty hands\.` | `hands.rs` reads hands from `<left>`/`<right>` frames, not GLANCE | N/A (the frames already carry the name) | - |
| sbox.lic:158, Pick.lic:179, box-buddy.lic:390 | trap difficulty as DETECT prints it | `It looks like an? (.*) trap \((?:about )?-(\d+)\)` | none | GAP (tpick reads only the number, `tpick.lic:3795`) | MEDIUM |
| Pick.lic:262 | a roll that opens | `^You make an outstanding attempt \(d100\(open\)=([0-9]+)\)\.$` | none | GAP | LOW |
| Pick.lic:273 | a lock too mangled to pick | `^The .+ appears to have been utterly mangled by some incredible force\.` | none | GAP | LOW |
| gpick.lic:675 | acid vial set off while picking | `^As you poke around inside the lock mechanism, you hear the sound of glass shattering.*fused into a lump of useless metal` | none | GAP | LOW |
| gpick.lic:613-614, deadpool.lic:110, box.lic:502 | pool worker hands a job (plinite or box) | `The (case\|trunk\|chest\|box\|strongbox\|coffer\|plinite) is set up on the table for you`; box.lic reads it with the item's `<a exist=>` | none | GAP (picker side) | MEDIUM |
| deadpool.lic:34-37 | scarab disarm by rune, safe vs unsafe; picking it up | `^\*scritch scritch\*  If that had been any easier, you could have done it blindfolded\.$`; `^Deciphering the runes is relatively simple, and you're pretty sure you were successful\.$`; `^You feel like you probably rendered the .* scarab harmless, but can't be sure\.`; `^You are unable to handle the additional load .* scarab would add\.$` | none | GAP | LOW |
| pop.lic:98-131, 177-212 | 416 detection variants tpick lacks (~20 lines) | scarab colours (`miniature sky-blue glaes`, `glimmering opalescent`, `tiny onyx`, `tiny translucent`), `A crimson glow surrounds the needle.`, `The tube appears to be filled with a greenish powder.` | none | GAP | LOW |
| checkbox.lic:68-73 | 704 Phase on a box, a third success form | `becomes somewhat insubstantial and appears lighter` | none | GAP | LOW |
| discard_trap_components.lic:23-85 | removing trap components as a rogue (7 lines) | `You remove a slender steel needle from`, `...a pair of small steel jaws from`, `...a small dark crystal from`, `...a clear glass vial of`, `...a green-tinted vial filled with`, `...a thick glass vial filled with`; refusal `As the trap is currently set, that would likely result in the loss of a few fingers.` | `gameobj-data.tsv:38` holds these items as type `lm trap` | PARTIAL (the items are typed, the extraction lines are not read) | LOW |
| SitNPick.lic:22-65, old-tpick.lic:3102 | handing a box between players: `^([A-Z,a-z]+) offers you a.+(trunk\|coffer\|strongbox\|box\|chest)\.  Click ACCEPT ...  The offer will expire in 30 seconds\.$`; `^You accept ([A-Z,a-z]+)'s offer and are now holding ...`; `has accepted your offer`; `Your offer to ... has expired`; `has declined the offer` | 6 | `grep -rn "Click ACCEPT" crates` -> 0 | GAP | MEDIUM (M7/M8: group looting, give and accept between a player's own characters) |
| SitNPick.lic:84, sbox.lic:838 | calipers already calibrated | `^You can't help but think that the calipers are as finely-tuned as you are possibly going to get them\.  They practically glow with calibration!$` | none | GAP | LOW |
| repair-lockpicks.lic:78, 153 | lockpick nouns, and the repair result | lockpick materials incl. ivory and nouns `lockpick\|hook\|key`; repair `cooling rapidly to form a tight bond\|refuses to break free` (tpick has `refuses to work free`) | `gameobj-data.tsv:39-40` (`lockpick` is noun `lockpick` plus three names) | PARTIAL: `hook` and `key` lockpicks are not typed `lockpick` in Cena, INFERRED from the pattern | LOW |
| xboxempty.lic:394, epop.lic:322-324 | coins from a box, and box to container | `You gather the remaining coin.`; `^You try to empty the contents of your .* into your .*, (and everything falls in quite nicely\|but nothing comes out)\.$` | `boxes.rs:27` (gather), none for EMPTY | PARTIAL: EMPTY-into-container moves loot without a GET, so the ledger's item links miss it (INFERRED) | MEDIUM (M6 ledger) |
| tpick.lic:2959-2972, 6577-6582 | rogue guild status (GLD) | `You have (\d+) ranks in the Lock Mastery skill\.`; `You are a Master of Lock Mastery\.`; task text `At least (\d+) more should have a plated lock ...`; `You have (.*) repetitions? remaining` | none (`grep -rn "Lock Mastery" crates` -> 0) | GAP | LOW (see `rogue.lic` below for the full guild vocabulary) |

### Groups C to E: captures not already in the tables above

| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| loottracker.lic:584-594 (the ledger's source), appraiser.lic:77-84 | appraisal **quality** is often two words: the source's own real gem sample reads `is a rare gemstone of above average quality`; the loresong ladder is very cheap, poor, below average, average, above average, fair, fine, good, exceptional, outstanding, superb, magnificent | `is of (?<quality>\w+) quality` (skin, loottracker `:594`) vs `of (?<quality>.+?) quality` (gem, `:589`) | `ledger/town.rs:59` ports the skin form as `is of \w+ quality and worth approximately ([\d,]+) silvers[.!]` | PARTIAL, INFERRED risk: if a skin is ever appraised `above average` or `below average`, the whole line fails to match and the appraisal is lost. UNVERIFIED that the furrier uses two-word qualities (the corpus would say; ask first). Quality and rarity are captured by neither Cena nor loottracker's storage | MEDIUM (M6 ledger; the fix is `.+?` as the gem form already has) |
| disarmbond.lic:8 | a bonded weapon knocked away | `^\[Use the RECOVER ITEM command while in the appropriate room to regain your item\.\]$` | `combat_effects.tsv:199` has a **creature** dropping its weapon (`status disarmed`); nothing for the character (`grep -rn "RECOVER ITEM" crates` -> 0; plans: `grep -n -i "disarm" plan/30-m6-hunt.md plan/33-guard-vocabulary.md` -> 0) | GAP | MEDIUM (M6 hunt safety: a disarmed character should stop attacking; disarm-no-more.lic exists because bigshot does not notice) |
| reimtraps.lic:7-16 | Reim's ethereal traps, each with its dodge: blowgun and bow (`lean`), hands from the ground and chain at the legs (`jump`), chain at the head (`duck`) | `A slender ethereal blowgun appears from the shadows nearby, aimed right for you!` and four more, matched with `==` | none | GAP | LOW (one event zone) |

## Script by script

Depth: **R** read in full or nearly (GUI blocks skimmed by grep), **C** capture lines and data
blocks read, **D** diffed against a family member. Lines are `wc -l`.

| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| tpick (R) | 6831 | Dreaven v44: pick boxes solo, ground, pool (picker), for others; pop with 416/407/408; plinites; drop/pickup at pool | 69 trap branches, lock table, pick table, precision words, pool picker vocabulary, 403/404/407/408/416/704 results; `Skills`, `Spell[]`, `Effects`, `GameObj`, map tags | the richest box source; Cena has only the pool **customer** side and the partial-gather gap |
| tpick2 (D) | 6361 | tpick v33-beta1 | 4 regexes not in tpick (all `active effects` / a combined disarm check) | variant; lock table identical but for whitespace |
| test-tpick (D) | 5938 | pre-release tpick | 5 regexes not in tpick (active effects, scale weapon) | variant |
| old-tpick (D) | 6447 | tpick before v1 of the rewrite | 25 not in tpick: per-material wire regexes, INFO stats lines, `You cannot even estimate ... out of your league` | variant; lock values as an array, same numbers (`old-tpick.lic:203`) |
| merchantical (C) | 8397 | Kyrandos v1.6.0 (2026-09): recall/loresong catalogue, player shop, batch sales | RECALL parser, obj type by text and noun, furniture capacities, sale/refusal phrases, analyze transmog | GAP (recall typed); value line `estimated to be worth about` not ledgered |
| gforge (C) | 2809 | forging perfects | forging and slab-cutter lines, supply menus | GAP, LOW |
| invdb-beta (C) | 6841 | Xanlin: inventory database (SQLite) | locker capacity, ores stored, LOCATION, account info | GAP, LOW |
| gemguard (C) | 7725 | Mystienne: gem review, purify, chrisms, lapidary | keep lists, loresing/704/chrism result words | N/A (keep lists) and GAP LOW |
| rogue (C) | 2384 | Dreaven v51: rogue guild tasks | 28 task texts, reps, rank lines | GAP, LOW |
| loot-be-gone2 (C) | 1797 | Alastir: looter and seller | town/race price table, gem bounty text, sale lines | CONFLICT with loottracker's racial table; author ruled bonuses out |
| shlooter (C) | 2162 | sloot variant | disk, ammo bundles, special searches | mostly HAVE via eloot port; ammo GAP LOW |
| chrismforge (C) | 4498 | Mystienne: chrism making | chrism tables | GAP, LOW |
| loot (C) | 1195 | Tillmen v0.26: looter | skinning, special searches, arrows | PARTIAL (special search yields) |
| testme (C) | 1692 | Dreaven v60: every item test | ~150 item-property lines incl. charge ladder, WPS, INSPECT coverage, Voln tier | GAP (typed item properties), MEDIUM |
| gtk3-sloot, slootM (D), slootbeta (C) | 2204, 2068, 1901 | SpiffyLoot family | slootM adds nothing to gtk3-sloot (0 new regexes); slootbeta adds settings/equipment lines | as shlooter |
| loottracker (D) | 5013 | Nisugi v0.2.1: the ledger's source | | HAVE (identical to the ported copy) |
| sbox (C) | 2104 | SpiffyJr: box handling | trap table as data (17 types), trap difficulty line | GAP, MEDIUM (cleanest trap shape) |
| itemdb (C) | 2491 | Drafix v1.7.0: item database from recall | 25 typed recall lines | GAP, MEDIUM |
| upickbot4 (C) | 688 | Gibreficul: Shattered pickbot | group/ignore lists, special gems | N/A logic |
| forge-perfects, fixed-forge-perfects, forge-perfects-fixed (D) | ~1215 each | Moredin/Tillek forging | three near-identical copies | GAP, LOW |
| foreachlich4 (C) | 2236 | Nagrad: do X for each item | locker count line, give/accept | utility; LOW |
| lockpick-maker (C) | 1103 | Timbalt: LM create picks | toolbench order numbers and costs | GAP, LOW |
| gpick (C) | 935 | Gwrawr: picking | pool job types, acid set-off line, 407 results | GAP (picker side) |
| box-buddy (R) | 668 | Dreaven v6: picking info window | lock **ranges**, modifiers, trap messages (copy of tpick's) | GAP; confirms band tops |
| gemhoarder, gemhoarder2, reagenthoarder (C) | 744, 749, 670 | Caithris: locker hoarding | locker wardrobe/chest lines | LOW |
| shatventure (C) | 1224 | Shattered bounty runner | loot/bounty boosts, debt | PARTIAL/GAP LOW |
| huntsuggest (C) | 2899 | NecroDeus: hunting-area picker | data from a Google Sheet at run time | N/A |
| lockmaster, Pick, pickerassistant (C) | 322, 568, 240 | calipers readers | lock tables, trap-type readers, roll and mangled-lock lines | same tables; order CONFLICT noted |
| pop, epop, checkbox (C) | 369, 343, 141 | 416/407/408/704 popping | 416 scarab colours, 704 third success form, empty-to-container | GAP LOW |
| inventory-buddy (C) | 734 | Dreaven: inventory/locker DB | INV empty, locker manifest, no bank accounts, bounty weekly | PARTIAL (bank), GAP (locker) |
| isharpie (C) | 1198 | Kyrandos: item notes | WPS phrasing variants | GAP LOW |
| scrollbuff2, scroll (C) | 405, 31 | invoke scrolls | RUMMAGE and INVOKE lines | GAP LOW |
| box (C) | 1882 | box handling (XML aware) | pool worker `is setup ... ASK ... ABOUT CHECK`, accept offers, calipers calibrated | GAP |
| townsmith, smelter (C) | 248, 359 | crafting | | GAP LOW |
| loresang (C) | 1155 | Kyrandos: loresong export | loresing outcome words | PARTIAL LOW |
| recall (R) | 315 | Alastir: keep/sell by recall | ~40 recall property lines | PARTIAL (held raw), MEDIUM |
| transmog_inventory (C) | 1556 | Mara (with Claude): transmog list | 0 regexes new | N/A |
| PoolParty-OG, poolparty_new, poolpartyhw, deadpool (C) | 334, 707, 751, 257 | pool picking loops | trash receptacle nouns, scarab rune disarm lines | GAP (picker side) |
| Calipers (R), xcalipers (R) | 193, 438 | Deathravin / LostRanger: measure and suggest | trap consequences, precision/strength/condition words, OTS pick table, lore formula (GM Naijin 2020-03-09) | GAP; CONFLICTs with tpick |
| recallwps (R) | 137 | Giantphang: WPS adjective -> CER | 14 x 4 | GAP, MEDIUM (two typos) |
| treasure (C) | 564 | Alastir: gem hoarding | open/close lines | LOW |
| xboxempty, boxempty, boxemptysilvery (C) | 481, 27, 27 | Xanlin: empty boxes | `You gather the remaining coin.` | HAVE (gather) |
| container-window, dye_search, watch-locker (C) | 999, 1957, 163 | container views, dye shop search, locker watch | XML container regexes | N/A (Cena has the container model) |
| enhrecalls, recall-enhancives, recallrecord, recaller (C) | 298, 148, 254, 124 | recall collectors | enhancive boost and loresong boundary lines | PARTIAL (raw held) |
| rogue-lmas-appraise, -makepick, -repair, -makelock (C) | 120, 114, 133, 105 | rogue guild LM task helpers (no author line) | LMAS APPRAISE full line (condition, precision, strength, `You could probably handle ...`) | GAP LOW |
| repair-lockpicks, lockpickget (C) | 167, 30 | pick repair | `refuses to break free` variant, `hook`/`key` picks | PARTIAL LOW |
| charm (C) | 130 | Zedarius (with Claude Opus 4.7): swarm charm | box nouns incl. casket, crate | HAVE (`boxes.rs:28-29`) |
| locksmithpool, boxbuster (C) | 181, 230 | pool drop and bounty boxes | pool return, EMPTY into container | HAVE / PARTIAL |
| librarian (C) | 1116 | Bhuryn: grimoires | scroll line with `in vibrant ink`, grimoire slots | HAVE / GAP LOW |
| SitNPick, pickother (C) | 95, 230 | picking for others | GIVE/ACCEPT offer lines | GAP MEDIUM (M7) |
| adrop, poolbox (C) | 105, 200 | drop magic items; Indigo Pools game | | N/A |
| deedgemappraise, orbappraise (C) | 102, 50 | Alastir: appraise gems for deeds/orbs | gem noun shortenings; `give you ([\d,]+) silvers for it if you want to sell.` | PARTIAL: `ledger/town.rs:72` reads `I(?:'ll\| will) (?:give\|offer) you ([\d,]+) silver`; deedgemappraise waits on `I'll give you [0-9]+ for it` with no `silvers` (UNVERIFIED which form the game sends) |
| discard_trap_components (C) | 95 | remove trap components | 7 extraction lines | PARTIAL LOW |
| spellbookscan (C) | 583 | elanthia-online v1.2.0: spell-book drop rate | town locksmith return, spec-book analyze | GAP MEDIUM |
| scrollbrowser, readscrolls2 (C) | 407, 117 | list scrolls | `Describe what`, XML spell link | HAVE (scroll line) |
| gibs_pickpocket_helper (C) | 221 | pickpocket log | success line | GAP LOW |
| **Outside the pile (special asks)** | | | | |
| wands (R) | 613 | Starsworn: wand table | 52 wands | GAP (map), all spells agree with `spells.tsv` |
| 918wands (R) | 545 | 918 duplicate and sell | guessed result patterns | GAP, low trust |
| wanddupe, aniwand, unstunwand (C) | 67, 29, 41 | wand utilities | duplicate result | LOW |
| enchant, enchantcheck, enchantnotes (R) | 41, 251, 261 | enchanting | 405 aura ladder, potion ladder, temper | GAP LOW |
| ensorcell_calc, ensorcell_tracker (R) | 253, 33 | ensorcell maths and flares | material table, penalties, flare payoffs | GAP / PARTIAL LOW |
| lore, loresong, loresing, autolore, mylore, loglore, lore_perm_bonus, tclore, star-lore (C) | 368 ... 35 | loresinging | outcome words, permanence formula | PARTIAL LOW |

## Tail

The 17 non-substantive scripts (capture + data < 5), each opened because every name here
suggests game data:
- **Hands and weapons:** `disarm-no-more` (Tgo01 v13; watches the hands, no text captured),
  `disarmwatch`, `kdisarm`, `disarmbond` (the RECOVER ITEM line, tabled above).
- **Lockers:** `get-locker-id`, `lockerswap`: locker ids by XML, no new text.
- **Recall and lore:** `recall2` (`As you recall the bard's song ...`, covered by recall.lic),
  `star-lore`, `loreloop`, `stopappraise`, `appraise`: command loops.
- **Appraisal:** `appraiser` (Aethor Whiteaxe; the 13-word loresong quality ladder, tabled above).
- **Scrolls:** `readpawnscrolls` (`\((\d+)\) (.*?)$`, the same line `town/reply.rs:115-122` reads).
- **Picks:** `lockpickget`, `locksmith` (7 lines): fetch commands.
- **Event:** `reimtraps` (tabled above). **Game:** `picklebob` (a minigame).

## Method

Output of every command below is in `survey/tools/5-boxes-magic-items/` where it was saved.

- Scope: `awk -F'\t' 'NR>1 && ($3+$4)>=5' pile-5-boxes-magic-items.tsv` -> 87 substantive, 17 tail.
- Newest tpick: header versions (`tpick.lic:15` v44; `tpick2.lic:16` v33-beta1; `old-tpick.lic:31`
  v1 of the old line). Read `tpick.lic` 1-890 and 1974-6831 in full; 890-1973 is the GTK settings
  window, grepped for `=~|dothistimeout|waitfor|matchtimeout` (all hits are settings plumbing).
- Families: every regex literal extracted by `tools/5-boxes-magic-items/regexes.py` (a regex over
  `=~`, `when`, `waitforre`, `matchtimeout`, `dothistimeout`, `Regexp.new`, `%r{}`), each
  alternation normalised to its words, and a script's patterns listed only when not already in a
  base set. Runs: base `tpick` for the family and box-buddy; base `tpick,box-buddy,Calipers(,sbox,
  gpick,pop)` for the other box scripts; base eloot (`reference/scripts/scripts/eloot.lic`),
  loottracker and tpick for the looters; `merchantical,recall,loresang` for lore scripts. Outputs:
  `boxB1.txt`, `boxB2.txt`, `merch.txt`, `recall.txt`, `lore.txt`, `lootE1.txt`, `lootE2.txt`,
  `invE3.txt`, `miscE4.txt`, `forgeF.txt`. Limitation: patterns built at run time
  (`#{...}`) and `.match?` calls are not expanded, so deadpool's and merchantical's constants
  were read by hand.
- The lock table across 14 scripts: `grep -lE "impossibly complex|unbelievably complicated" *.lic`
  then `grep -noE "(absurdly difficult|masterfully intricate|unbelievably complicated|absurdly
  complex|impossibly complex)[^,]{0,20}"` per file.
- Wands: `tools/5-boxes-magic-items/wands_cmp.py` parses the 52 rows of `wands.lic`, looks each
  spell number up in `crates/cena-model/data/spells.tsv` and classifies each name with
  `gameobj-data.tsv`'s own patterns (`wands_cmp.txt`: `spell mismatches=0`).
- loottracker identity: `diff <(tr -d '\r' < reference/scripts/scripts/loottracker.lic)
  <(tr -d '\r' < reference/lich_repo_mirror/lib/loottracker.lic)` -> only the final newline.
- Absence claims, all over `E:/Cena/crates` (`--include=*.rs --include=*.tsv`, some also `*.toml`):
  `thief-lingo`, `thief.lingo`, `tumblers`, `primitive lock`, `putty` (0 files each);
  `scarab`, `lockpick`, `calipers`, `plinite` (only `gameobj-data.tsv`, `containers.rs`, tests);
  `trap`/`disarm` in `*.rs` (only skills, movement, combat statuses, prose); `As you recall`,
  `imparts a bonus`, `persist after`, `crumble into dust`, `estimated to be worth`,
  `ensorcelled`, `sanctified`, `predominantly crafted`, `enhancive charges`, `charges remaining`,
  `locker is currently holding`, `survey your surroundings`, `Click ACCEPT`, `offers you`,
  `empty the contents`, `scrawled`, `ink leaps`, `Invoke from`, `Training Administrator`,
  `RECOVER ITEM`, `Your disk arrives`, `cold blue gem`, `pungent piece`, `owe a debt` (0 each).
  The Lich reference was checked too: `grep -rln "thief-lingo\|scarab wedged\|primitive
  lock\|level of precision" reference/lich-5` -> 0.
- Cena counterparts read: `ledger/boxes.rs` (all 131 lines), `town/pool.rs` (353), `town/reply.rs`
  (137), `loot/outcome.rs` (parts), `ledger/town.rs` (its patterns, by grep), `inventory_snapshot.rs`, `bank.rs`,
  `gameobj-data.tsv` (types for box, lm tool, lm trap, lockpick, plinite, scarab, wand, scroll,
  consignment), `spells.tsv` rows 205, 403, 404, 407, 408, 416, 704, 918, 1006, 1035, 1408,
  1707, 1710, 9718, 9813, 9918.
- Not done: the log corpus was not searched (brief). The two open questions it would settle are
  the top-band lock order and skin appraisal qualities.
