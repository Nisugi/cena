# 6-magic-effects: lich_repo_mirror survey

Scope: 360 scripts, 163 substantive (capture + data >= 5); 40 read deeply, 123 at capture-line
depth, 197 in the tail by name (21 opened); Cena HEAD 3566f8d at start, a6146e6 at the end
(`git -C E:/Cena rev-parse --short HEAD`; the two commits between add eherbs and skinning and touch no
file cited here except one unrelated line in `state/chunks.rs`), 2026-09-25. `hunt/engine.rs` carries
another session's uncommitted heal edits; `SIGN_RETRY` (60 s, `:58`) and `maintain` are unchanged.

## Top findings

1. **`spells.tsv` loses 98 of effect-list.xml's 628 spell messages, on 43 spells.**
   `crates/cena-model/tools/extract_spells.rb:119` keys messages by type with `to_h`, so only
   the last `start` and last `end` survive; Lich joins all of them (`lib/common/spell.rb:82-85`).
   The survivor is often the wrong one: 535 keeps its renewal line, 916 keeps `You refresh your
   invisibility.`, 999 keeps a refusal. `spells.rs:5` still claims 628 and nothing tests it.
   CONFLICT with the port-data-whole rule. HIGH, M6. (Special ask 1)
2. **The same extractor drops every stacking and casting attribute**: stackable (96 spells),
   refreshable (56), multicastable (83), max (19), persist-on-death (34), real-time (10), and
   spell `stance` (16), `incant` (17), `channel` (15), `cast-proc` (109)
   (`extract_spells.rb:108-135`; counts from the XML). waggle 0.18.2 (`wagglegobble.lic:1059-1095`),
   passive-spellbot and severance decide by them; Lich's cast sets offensive stance from
   `stance` (`spell.rb:762`). Maintain's "recast before they lapse" (`plan/30-m6-hunt.md:198`)
   needs them. CONFLICT, HIGH, M6. (Special ask 2)
3. **No cast-result vocabulary.** Lich's `spell.rb:21-62` has 12 prepare and 26 result lines
   (`Cast at what?`, `Be at peace my child...`, `Your magic fizzles ineffectually.`,
   `[Spell preparation time: N seconds]`...), matched across this pile (waggle, keepmealive,
   aethorvars, smartsorcerer, rcast). Cena matches only `[Spell Hindrance`
   (`travel/routines/casting.rs:126`). Also missing: the non-stack refusal `Your magic clashes
   with that which is already there!` and the society refusals (`symbolz2.lic:103`,
   `gsigns.lic:169`). GAP, HIGH, M6 (Engage runs `incant`; Maintain casts signs).
4. **Barkskin, in Nisugi's own M6 `signs`, has a cooldown Cena does not know and a broken end
   message.** `cab.lic:85-88` (Nisugi) sets 301 s after the bark absorbs a hit and matches
   `...but rapidly crumbles away.` for a cast during it; `spells.tsv` 605 has no cooldown, and
   its `msg_end` `...disintegrating.\.` (from `effect-list.xml:687`) cannot match the real line
   (anchored test, Method). GAP + CONFLICT, HIGH, M6 (INFERRED: Maintain would recast into the
   refusal every `SIGN_RETRY`).
5. **Lich's `combat/defs/messages.rb` (41 defs) is the source for lines this pile keeps
   re-matching**: weapon reaction (`cab.lic:101`, Nisugi), Sanctum transform, arcane reflex, 703
   haze on a creature, bless expired. Cena deferred it on purpose
   (`tools/extract_combat_defs.rb:48-50`, "a later pass, if a consumer wants it"); the pile
   shows the consumers. GAP (deferred), HIGH, M6.
6. **Spell-state hazards a hunt must see**: dispelled
   (`status_report.lic:229`), spellburst warning (`:237`), Condemn and Power Sink rooms
   (`:239-241`, `cab.lic:73-75`), room hazards web/cloud/vine/swarm/vortex/void/tempest and the
   spell per realm that clears each (`badjuju.lic`, `untrammel.lic:40`). Cena has none of these
   lines and no hazard category (`gameobj-data.tsv` categories, Method). GAP, MEDIUM-HIGH, M6.
7. **Sunfist durations in `spells.tsv` disagree with Lich's society table and the wiki**:
   Sigil of Contact 17 min vs 19, Bandages 3 vs 5, Determination 3 vs 5
   (`guardians_of_sunfist.rb:44,71,161`; `wiki_clean/Guardians of Sunfist.txt:132-150`;
   `isigils_new.lic:20,23,31`). Every sigil and mana-sign **cost** in the pile's tables matches Cena
   (`Society.lic`'s spirit column is Cena's cost + 1 throughout, INFERRED to be a threshold).
   CONFLICT, MEDIUM, M6. (Special ask 4)
8. **Spell Sever and spellburst are absent from Cena and from Lich core.** `severance.lic:65-78`
   (Kaetel 1.0.12, 2026-09-03) lists 16 exempt group spells plus 4 short unlocks and a limit of
   2; `spells.tsv` `availability` calls 9 of the 16 `self-cast`/`all`. `burst_calc_new.lic:42-120`
   has an approximate burst formula and 30 exempt spells. GAP, MEDIUM, M6.
9. **Third-person spell lines**: `spellmerge.lic:227-370` (LostRanger 0.7) pins 29 landing and
   wear-off lines on 21 group spells to spell numbers; `rapid-fire`/`shaste`/`lessershroud` add
   515/535/120 on others. Cena's third-person set is Lich's 18 `spell_loss` rows; 2 of 22 tested
   lines match. GAP, MEDIUM, M6 group Maintain and M5.
10. **Mana and resource lines**: MANA PULSE's four replies (Lich `lib/gemstone/mana.rb:15-20`),
    the `mana` report (`manatimer-sf.lic:49-58`), 12 non-pulse mana sources
    (`pulsetimer.lic:126-178`), mana sending (`sendmana.lic:115-126`), and Shadow Essence (Lich
    `infomon/parser.rb:55-61`). `vitals.rs` holds the gauges only. GAP, MEDIUM, M6 rest, M5.
11. **Room-wide spells and wands**: `donoharm.lic:27` lists 26 room-wide attack spells; Cena's
    `area` tag covers 4 of them. `wands.lic` (Starsworn 1.2, from the wiki) maps 52 wands to
    spells; `dazed-and-confused.lic` agrees on its 33. Cena types a wand by noun with no spell
    (`gameobj-data.tsv:80-81`); `plan/30-m6-hunt.md:509` lists wands as not built. PARTIAL/GAP,
    MEDIUM, M6.
12. **The old infomon's special cases map to gaps Cena would reopen if it ever reads spell
    text**: shared up-messages (9903/9907/9913 and 9904/9908/9912 have identical `msg_start` in
    `spells.tsv`), Core Tap charges, cast -> cooldown links (9813->9048, 515->599,
    aspect n->n+1), `spell active <name>` for other characters. PARTIAL, MEDIUM. (Special ask 3)

## Special ask 1: spell messages the scripts match, against `spells.tsv` and `effects.rs`

**What Cena holds.** `spells.tsv` has three message columns: `msg_start`, `msg_end`,
`msg_target_start` (header, `data/spells.tsv:8`). Filled: 273, 255 and 2 rows of 515 (awk count,
Method). **No prep, cast, success, failure, renewal or refusal column exists**; renewal and
refusal lines appear only where effect-list.xml filed them as `start`. `effects.rs` holds no
message text at all: it is fed by the `Active Spells`/`Buffs`/`Debuffs`/`Cooldowns` dialogs
and keys by wire id (`effects.rs:49-58`).

**What reads them at runtime.** Only the cooldown landing: `spells.rs:398-420` compiles
`message_up` for the three group-cooldown spells (211, 215, 219) and `target_start` for the two
target-cooldown spells (140, 506), used by `state/chunks.rs:353`. No code matches `message_down`
(grep for `message_down` outside `spells.rs` finds only `tests/spells.rs`). VERIFIED.

**Finding 1 (CONFLICT with port-data-whole, HIGH).** `tools/extract_spells.rb:119` builds
`messages` with `to_h` keyed by type, so a spell with two `start` or two `end` messages keeps
only the last. MEASURED against the same source file the TSV header names
(`C:/Gemstone/lich-5/data/effect-list.xml`, mtime 2026-09-13): 628 messages (336 start, 290
end, 2 target-start), TSV holds 530, so **98 messages on 43 spells are lost**. `spells.rs:5`
still says "515 spells, 628 messages" and no test pins the count. Lich keeps all of them,
joined with `$|^` (`lib/common/spell.rb:82-85`). The losses are not random: the kept one is
often the renewal or refusal variant, and the scripts wait on the dropped one:

| spell | kept in `spells.tsv` | dropped, and who matches it |
|---|---|---|
| 535 Haste start | `The world stutters around you ... slower speed` (a renewal) | `You begin to notice the world slow down around you.  Strange.` (`shaste.lic:83`) |
| 916 Invisibility start/end | `You refresh your invisibility.`; one of 9 ends | `You become invisible.`, `You (become\|are) visible again.` |
| 999 Core Tap Recovery start | `You are too exhausted to cast Core Tap right now.` (a refusal) | the real cast line `Tapping into the elemental core of Elanthia...`; old infomon special-cases the refusal (`infomonfixspells.lic:1241`) |
| 9075 Meditation end | `The lingering effects of your meditation fade away.` | `Your action interrupts your meditation.`, `Your meditation is interrupted by X.` (`meditate.lic:65-78`) |
| 9009 Raise Dead Cooldown, 9044 Miracle Cooldown | 1 of 17, 1 of 21 start lines | 16 and 20 lines |
| 9060-9074 Meditative Resistance (15 spells) | one end line each | the other |

**Finding 2 (CONFLICT, LOW-MEDIUM).** 605 Barkskin `msg_end` is
`...before disintegrating.\.` (effect-list.xml:687, carried into `spells.tsv`): an unescaped
dot then an escaped one, so it cannot match the one-period game line that
`cab.lic:83` (Nisugi's own script) waits for. VERIFIED with an anchored regex test (Method).
The only such typo among Cena's messages (`typos.py`, Method). 605 is in Nisugi's imported
`signs` (`plan/30-m6-hunt.md:198`).

**Script message coverage, by kind** (checker: every `/regex/` and `matchwait` string in the
scripts, tested against every Cena message pattern; Method):

| kind | example scripts | in Cena? |
|---|---|---|
| own wear-off (first person) | `status_report.lic:59-199` (71 lines) | 69 match `spells.tsv` msg_end; 540's miss is a script typo; 1613's line exists nowhere |
| own landing (first person) | `cab.lic:81`, `lessershroud.lic:75` | mostly HAVE; 535/916/999 dropped as above |
| third-person landing/wear-off on others | `spellmerge.lic:227-370` (29 spell entries on 21 spells), `rapid-fire.lic:112-121` (515), `shaste.lic:113-124` (535), `lessershroud.lic:105-114` (120) | of the 22 single-line entries tested, 2 match (`combat_effects.tsv` spell_loss 911 and the generic `seems slightly different`); Cena's third-person set is Lich's 18 `spell_loss` rows. GAP MEDIUM (M6 group Maintain) |
| prep lines | `infomonfixspells.lic:390-433` (self and others, multicast `make a complex gesture`); `mana-tracking.lic:302` (4 elemental prep lines naming the spell); `dreavening.lic:349`; `Your spell is ready.` / `[Spell is stored]` (`rcast.lic:22`) | none held; GAP MEDIUM-HIGH (M6 cast confirm) |
| cast results and refusals | `wagglegobble.lic:869-909`, `keepmealive.lic:134-179`, `aethorvars.lic:2355-2425`, `smartsorcerer.lic:174-184`, `stupidglobe.lic:10-14`; source set is Lich `lib/common/spell.rb:21-62` (12 prepare + 26 result lines) | Cena matches only `[Spell Hindrance` (`travel/routines/casting.rs:126`) and the outcome table's hindrance/warding rows. GAP HIGH (M6) |
| non-stack refusal | `Your magic clashes with that which is already there!` (`rapid-fire.lic:121`) | GAP |
| society refusals | `The power from your sign dissipates` (`gsigns.lic:169`), `nullifies your symbol`, `power from your symbol dissipates` (`symbolz2.lic:103`), `Repeating the sign has no effect!` (`infomonfixspells.lic:931`) | GAP MEDIUM (M6 Maintain casts signs) |
| spell-state events | dispelled `A pale, flickering nimbus coalesces around you, then vanishes in a brilliant flash!`, two dispel-blocked lines, spellburst `You suddenly feel the essence surrounding you shift and writhe chaotically!` (`status_report.lic:229-237`) | GAP MEDIUM-HIGH (M6) |
| item-lore (loresing/recall) | `freddie_mercury.lic:66-96`, `ssing.lic:52-169` | value line HAVE (`ledger/town.rs:62-63`); the rest GAP |
| song status | `song-manager.lic:276-343`, `play.lic:711` | GAP MEDIUM |

`effects.rs` is not where these belong: it is the dialog's state. The gaps are classifier gaps
(cast result, refusal, third-person) and the extractor loss above.

## Special ask 2: spell data the scripts hold, against `spells.tsv`

**Finding 3 (CONFLICT with port-data-whole, HIGH, M6).** The extractor also drops every
attribute the spell-up scripts decide by. `extract_spells.rb:108-114` packs a duration as
`cast-type, kind, body` and nothing else; the row it writes (`:122-135`) has no column for spell attributes.
MEASURED in the same effect-list.xml (spells carrying the attribute):

| attribute | spells | Lich reader | who uses it in this pile |
|---|---:|---|---|
| `duration span='stackable'` | 96 | `spell.rb:118`, `stackable?` `:392` | waggle (`wagglegobble.lic:1059-1095`), passive-spellbot (`:189-193`), share |
| `duration span='refreshable'` | 56 | `spell.rb:119`, `refreshable?` `:416` | waggle `refreshable_min` (`:45`, `:1077-1090`) |
| `duration multicastable=` | 83 | `spell.rb:120-124`, `:440` | waggle multicast (`:1003-1027`), infomon multicast inference (`infomonfixspells.lic:1362-1390`) |
| `duration max=` (else 250 min) | 19 | `spell.rb:130-134`, `max_duration` `:557` | waggle `stop_before`, passive-spellbot's 250 min cap |
| `duration persist-on-death=` | 34 | `spell.rb:125-128` | reconnect/death handling |
| `duration real-time=` | 10 | `spell.rb:109-113` | |
| spell `stance='yes'` | 16 | `spell.rb:87`, used `:762` (sets offensive before the cast) | casting wrappers |
| spell `incant='no'` | 17 | `spell.rb:68`, used `:698` | casting wrappers |
| spell `channel='yes'` | 15 | `spell.rb:88`, used `:685` | `aethorvars.lic:2316` |
| `<cast-proc>` | 109 | `spell.rb:136` | (Ruby; would be kept verbatim like `Duration::Unknown`) |

Nisugi's `signs` list is Maintain's input (`plan/30-m6-hunt.md:198`); "recast before they
lapse" needs stackable/refreshable/max to know whether a recast adds time.

**Other spell data in the pile, against `spells.tsv`:**

- **Renew costs** (`song-manager.lic:70-81`): 11 of 12 agree with `spells.tsv` `renew_cost`.
  1007 Kai's Triumph Song: script `2+[[Spells.bard-7,0].max,10].min`, Cena
  `2+[[Spells.bard-7,0].max,13].min`. CONFLICT, script older, LOW.
- **Sleep (501) cost**: `mana-tracking.lic:305-330` scales to 13 by caster level; Cena caps at
  5 by `Stats.level`; the wiki copy caps at 5 **by target level**
  (`reference/wiki_clean/Sleep _501_.txt:17-19`). Script stale; Cena's formula uses the
  caster's level where the wiki says target's. CONFLICT, LOW.
- **Multicast cap** `(mana-control ranks / 25) + 1` by profession and circle
  (`wagglegobble.lic:1003-1027`, same in `share.lic:226-250`, `keepmealive.lic:200-220`). GAP,
  MEDIUM (M6 spell-up).
- **Item-cast duration**: `spell.rb:286` activator modifiers tap 0.5, rub 1, wave 1,
  raise 1.33, eat/drink 0, and scroll/invoke substitute Arcane Symbols then halve
  (`spell.rb:318-330`); used by `MIUduration.lic`. Cena keeps the expression only. GAP LOW.
- **Spellburst**: `burst_calc_new.lic:42-54` formula (its author calls it approximate) and 30
  exempt spells; no spellburst concept anywhere in Cena or Lich-5 core (grep, Method). GAP
  MEDIUM.
- **Spell Sever**: `severance.lic:65-78` (Kaetel v1.0.12, 2026-09-03), 16 exempt group spells,
  4 short group unlocks (211, 215, 219, 506), default limit 2. `spells.tsv` `availability`
  cannot stand in for "group spell": 9 of the 16 are `self-cast` or `all` there (605, 620,
  1006, 1007, 1018, 1035, 1125, 1213 self-cast; 1216 all). PARTIAL/CONFLICT-shaped, MEDIUM.
- **Room-wide attack spells**: `donoharm.lic:27` lists 26; `spells.tsv` marks `area` on 5
  (`spells.rs` `is_area`, measured there), overlapping on 4 (410, 435, 909, 912); 518 is area in
  Cena but not in donoharm's list. PARTIAL, MEDIUM (M6 target choice).
- **Bonuses not in effect-list**: `timers_sagapanel.lic:1063-1150` (2026): Surge of Blessings
  +10 AS/+6 CS per stack up to 3, Crushing Dread -5 pAS/-3 CS per stack, Camouflage 608 +30
  AS/+18 spiritual CS, Item Supercharger 9084 half weapon enchant, Brine of the Rusalka +10
  AS/+6 CS per target, Rage +damage AS capped 50 for 30 s, 425 floor for scroll-known casters.
  `spells.tsv` bonuses column is empty for 608, 9084, 1016 (awk, Method). GAP MEDIUM (M10
  GUI, AS math).
- **Stack counts in effect names**: `Surge of Blessings (2)`, `Crushing Dread (3)`,
  `Hypothermia (90) 32 mins`, `Luminous Flight (1..3)`, `Wall of Thorns Poison 1..5`
  (`timers_sagapanel.lic:560-570`, `effectsbar_sagapanel.lic:162,237-239`). `effects.rs` keeps
  the text verbatim (HAVE) but reads no count. PARTIAL LOW.
- **Cast -> cooldown linkage**: `symbolz2.lic:39-43` (9813->9048, 9812->9049, 9819->9050),
  infomon (515->599, 597; 9627->9052 five minutes; aspects n->n+1; 725 with 9725:
  `infomonfixspells.lic:1225-1233, 1318-1322, 1418-1427`). `spells.tsv` has every one of these
  rows but no column linking a cast to its cooldown row. PARTIAL MEDIUM (M6 Maintain).
- **Barkskin cooldown**: `cab.lic:85-87` (Nisugi) sets a 301 s cooldown after the bark absorbs a
  hit; `spells.tsv` 605 has no cooldown and effect-list has no Barkskin cooldown row (grep).
  GAP, HIGH (605 is in Nisugi's M6 `signs`; INFERRED that Maintain would recast into a
  refusal every `SIGN_RETRY`).
- **Wands**: `wands.lic:21-620` (Starsworn v1.2, from the wiki) 52 wands with spell, circle,
  crumbly/treasure/alchemy flags; `dazed-and-confused.lic:261-293` 33 wands, all agreeing
  with wands.lic (diff, Method). Cena: `gameobj-data.tsv:80-81` types a wand by noun, with no
  spell. GAP MEDIUM (`plan/30-m6-hunt.md:509` lists wands as not built).
- **Runes, sigils, infusion**: `fizzfuse.lic`, `infuse.lic`, `infuse_scroll.lic` (scroll
  infusion), `imbed_rods.lic`, `chargeimbed.lic`, `useimbed.lic` (420/517 charges), `juice*`,
  `tears2`, `dechanter`, `chanterguide`, `aura`, `essence` (925/735 enchanting tables). All GAP,
  LOW: no crafting milestone.

## Special ask 3: scripts that fix or extend Infomon's spell tracking

`infomonfixspells.lic` (1.18), `infomoncommageddon.lic` (1.17) and `infomon_backup.lic` (1.14,
Alastir) are the **pre-Lich-5 text infomon** (Shaelun), not patches to today's
`lib/gemstone/infomon/`. Diffs: 1.14 -> 1.17 adds Core Tap charges, the `vocal cords` spelling,
the new CMan header and enhanced-stat capture; 1.17 -> 1.18 changes only the `spell active`
header (`active spells` -> `active effects`) and skips its section lines (`diff`, Method). What
they were patching, and whether Cena has the same hole:

| Lich gap it patched | where | Cena |
|---|---|---|
| Knowing what is up by **matching every spell's up/down message** in the text | `infomonfixspells.lic:372-373, 1210-1440` | Not needed for self: Cena reads the effect dialogs by id (`effects.rs:49-58`), as Lich 5 does. But the dialog lags a cast (`effectsbar_sagapanel.lic:251-257` keeps `Spell[n].active?` for 506/515/1035 "immediately after casting") and never covers other characters; the text route is the only one for both, and Cena matches no up/down text except the 3 cooldown landings. PARTIAL |
| `SPELL ACTIVE` output changed format twice (PSM3, "commageddon") | fixspells `:1462-1500` vs commageddon | Cena parses no `spell active` text. Same format now carries `spell active <name>` for **other** characters (`wagglegobble.lic:945-1000`), with `has spell sharing disabled`. GAP MEDIUM (M6 group spell-up; M5 characters in the same Hydra have their own dialogs) |
| Name drift between the text list and the spell table: `Mage Armor - X`, `Cloak of Shadows - X`, `Raise Dead Recovery` -> `Raise Dead Cooldown` | `:1472-1480`; repeated in `wagglegobble.lic:978-984` | Not an issue for the dialog (ids). Would be for `spell active <name>` text. |
| **Shared up-messages**: Sign of Striking/Smiting/Swords print one line, Warding/Defending/Shields another, so the text cannot say which | `:1261-1284`, resolved from the XML end time or the typed command | Same ambiguity is in `spells.tsv`: 9904/9908/9912 share `You (?:grip\|flex) your ... with renewed vigor!` and 9903/9907/9913 share `Your dancing fingers weave a web of protection around you!` (awk, Method). Harmless today because nothing matches them; a future text consumer must not map them 1:1 |
| Core Tap charges (1-3) by Elemental Lore: Earth ranks, thresholds `[[210,4],[135,3],[60,2],[0,1]]`; the refusal line is not a cast | `:1241-1251`, `:1401-1409` | `spells.tsv` has the four rows (996-999) and **999's only start message is that refusal** (the cast line was dropped, Finding 1). PARTIAL |
| Cast -> linked timers: 515 -> 599 (and 597), 9627 -> 9052 five minutes, 9043 -> 9042, Aspect n -> n+1 cooldown, 725 with 9725, Meditative Resistance family | `:1225-1233, 1318-1322, 1418-1427` | rows exist, linkage not held. PARTIAL MEDIUM |
| Who cast it and how: self, other player, item activator (rub/wave/tap/raise/drink/bite/gobble), invoked scroll, multicast (`make a complex gesture`) -> which duration formula | `:390-433`; formula side in `spell.rb:285-330` | `Duration` keeps self/target forms only (`spells.rs` `CastType`); no activator modifiers, no multicast. GAP LOW-MEDIUM |
| Mana Leech drained-mana timer, Curse of the Star bonus, multicast duration read back from the XML | `:1285-1312, 1339-1390` | not held; LOW |
| Text-only statuses (sleeping, bound, silenced, calmed, cutthroat) | `:1443-1462` | HAVE (`state/afflictions.rs`, from today's `infomon/parser.rb:87-102`). The 1.14 copy's `vocal chords` would never match; `rapid-fire.lic:88`, `shaste.lic:89` and `lessershroud.lic:81` still wait on `vocal chords`, Cena correctly has `vocal cords` (`afflictions.rs:105`) |
| Armor specialization (9501-9509) falling off when armor is removed: `As you remove your .*?, it falls out of alignment.` | `:1437-1440` | GAP LOW (the dialog shows it) |
| Death timers: `...departing in N mins...`, `Thy soul is bound to thy body for an extra N minutes` (pseudo-spell 6666) | `:1201-1209` | GAP LOW (no 6666 row in `spells.tsv`) |

Other scripts extending effect tracking: `spellfade.lic` 1.1.0 (announces ends from the effect
list; aliases `multi-strike cooldown` -> `MStrike Cooldown`, effect `beacon of courage` ->
1608), `unified-effects.lic` (re-merges the four dialogs, which Cena already does in one
collection, `effects.rs:33-47`), `dispel_tracker`, `inactivespells`, `spellactive2`,
`timers_sagapanel`, `effectsbar_sagapanel`. None patches a gap Cena has beyond those above.

## Special ask 4: society and resource scripts, against `societies/` and `vitals.rs`

**Costs: the sigil tables agree with Cena; one sign column is a different quantity.** All 13 sigils
in `Society.lic:106-118` (its full table) and all 15 in `isigils_new.lic:19-34` match
`societies/sunfist.rs:100-292` on mana and stamina, as do the mana-only signs (Warding 1,
Striking 1, Thought 1, Defending 2, Smiting 2, Staunching 1, Deflection 3; `Society.lic:119-125`).
The four spirit costs in `no-spirit-death.lic:37-42` (Swords 1, Shields 1, Dissipation 1,
Madness 3) match `societies/col.rs`. `Society.lic:126-132` gives every spirit sign **one more**
than Cena (Swords/Shields/Dissipation 2, Healing 3, Madness 4, Wracking 6, Darkness 7, against
Cena's 1/1/1/2/3/5/6 and the wiki's 1 for Shields, `reference/wiki_clean/Council of Light.txt:83`).
A uniform +1 reads as "spirit you must have to use it without a spirit death", not a cost
(INFERRED; the script does not say). Not a CONFLICT in the data; worth knowing if a behavior
ports that script's thresholds. Voln favor costs: no script in this pile states one.

**Durations: Cena's own tables disagree with each other.** `spells.tsv` (from effect-list.xml)
has Sigil of Contact 17 min, Bandages 3, Determination 3. Lich's
`lib/gemstone/societies/guardians_of_sunfist.rb:44,71,161` (1140 s, 300 s, 300 s), the wiki
(`reference/wiki_clean/Guardians of Sunfist.txt:132-150`: 19 min, 5 min, 5 min) and
`isigils_new.lic:20,23,31` all say 19/5/5. CONFLICT inside Cena's data, MEDIUM (M6 Maintain
plans recasts from durations). Cena's `societies/` tables hold no duration at all: the Voln
`:duration` lambdas were left out on purpose (`societies/voln.rs:180-183`). Council signs differ
by a constant: `spells.tsv` `1 + Stats.level / 6.0` minutes against Lich
`council_of_light.rb:47` `10 * Stats.level` seconds (60 s apart; Staunching 120 s; LOW).

| topic | scripts | Cena | verdict |
|---|---|---|---|
| Voln favor figure | (none capture it here) | `character/currency.rs:140` `Voln Favor:` | HAVE |
| Voln symbol -> its cooldown effect (9813->9048, 9812->9049, 9819->9050) | `symbolz2.lic:39-43`, `symmanaloop.lic` (`You feel Koar's blessing return to you.`) | cooldown rows HAVE (`spells.tsv` 9048-9050, 9048's end line matches); no link column | PARTIAL MEDIUM (M6 Maintain) |
| Voln refusals and rooms where symbols fail | `symbolz2.lic:45,103` | none | GAP MEDIUM |
| Voln `sym recognition` reply `X is a (member\|Master)` | `raisedead.lic:250` | none | GAP LOW |
| Voln step 24 rune puzzle | `VolnStep24-RuneSolver.lic`, `step24.lic`, `Melgorehn.lic` | none (Cena reads `at step N`, `societies/membership.rs:149`) | GAP LOW |
| CoL recognition before Wracking: the gesture pattern (5 verbs x 5 body parts) and `X acknowledges your sign` | `wagglegobble.lic:814-817`, `aethorvars.lic:1420-1452`, `keepmealive.lic:77-80`, `umadbro.lic:93` | none | GAP LOW-MEDIUM (a spell-up that uses Wracking) |
| CoL pending spirit from dissipating signs | `no-spirit-death.lic`, `wracksafe`, `umadbro`, `signspacer` | timing HAVE (`societies.rs` `CostTiming`); the sum deferred above the model on purpose (`societies.rs` module doc, `pending_spirit_loss`) | HAVE (by design) |
| CoL sign refusals, `Repeating the sign has no effect!` | `gsigns.lic:169`, `infomonfixspells.lic:931` | none | GAP MEDIUM (M6 Maintain) |
| Sunfist mutually exclusive sigils (minor/major bane, minor/major protection) | `isigils_new.lic:51-53` | none | GAP LOW |
| Sunfist sigil effects (+1 AS/DS/TD per rank, Power 50 stamina -> 25 mana, Health 15 HP or half) | `isigils_new.lic:20-34` | cost only | GAP LOW |
| Mana gauge | all | `state/vitals.rs` current/max/percent | HAVE |
| Mana regen: what is and is not a pulse (12 other mana sources with their lines), dead pulse | `pulsetimer.lic:82-178` | none | GAP MEDIUM (M6 rest `rest_till_*`) |
| `mana` report (`Mana gained off/on node: N`, MANA PULSE / MANA SPELLUP) and a regen estimate | `manatimer-sf.lic:49-73` | none | GAP MEDIUM |
| MANA PULSE and its four replies | Lich core `lib/gemstone/mana.rb:15-20` (bigshot, ebounty, eherbs share it) | none | GAP MEDIUM (M6: bigshot's pulse-before-cast) |
| Mana sharing (`send N to X`) success and failure lines | `sendmana.lic:115-126`, `pulsetimer.lic:150` | none | GAP MEDIUM (M5: characters in one Hydra can feed each other) |
| Spell cost paid in stamina (Mental Acuity feat: stamina >= 2 x mana cost) | `wagglegobble.lic:872-876` | none | GAP LOW-MEDIUM |
| Shadow Essence (sorcerer, 0-5) and its 8 lines | `effectsbar_sagapanel.lic:331-340`; source Lich `infomon/parser.rb:55-61` | none (`standing.rs` holds resource weekly/total, suffused, Covert Arts charges) | GAP MEDIUM (M6, sorcerers) |
| Weekly/total resource and suffused amount | `calcsongofluck.lic:41-51` | `character/standing.rs:491-516` | HAVE |
| Spirit and stamina gauges | `sstamina`, `staminamina`, `staminaburstup` | `state/vitals.rs` | HAVE |

## Data tables Cena lacks or differs on
| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| (Lich's own `effect-list.xml`, read by every spell script here) | spell messages | 628 (98 lost) | start/end per spell, all variants | `data/spells.tsv` msg_start/msg_end; `tools/extract_spells.rb:119` | CONFLICT (extractor keeps last per type) | HIGH, M6 |
| (same) | stacking and cast attributes | 96 stackable, 56 refreshable, 83 multicastable, 19 max, 34 persist, 10 real-time, 16 stance, 17 incant, 15 channel, 109 cast-proc | per spell/duration | none; `extract_spells.rb:108-114, 122-135` | CONFLICT (dropped) | HIGH, M6 |
| `effect-list.xml:687` via `spells.tsv` 605 | Barkskin end message | 1 | regex | `spells.tsv` 605 msg_end | CONFLICT with `cab.lic:83` (typo `.\.`) | LOW-MEDIUM, M6 |
| `cab.lic:85-87` (Nisugi) | Barkskin cooldown after absorb | 1 | 301 s | `spells.tsv` 605 (no cooldown), no cooldown row | GAP | HIGH, M6 (605 in Nisugi's signs) |
| `severance.lic:65-78` (Kaetel 1.0.12) | Spell Sever exempt spells, limit | 16 + 4, limit 2 | spell numbers | `spells.tsv` `availability` (9 of 16 disagree) | GAP / CONFLICT-shaped | MEDIUM, M6 |
| `burst_calc_new.lic:42-120` (Gibreficul, Lavastene) | spellburst formula, 30 exempt spells, half-cost circles by profession | 30 + 10 circles | numbers | none (no spellburst anywhere) | GAP | MEDIUM, M6 |
| `donoharm.lic:22,27` (Tgo01 v4) | offensive spells; room-wide attack spells | 109; 26 | spell numbers | `spells.rs` `is_area` (5; 4 overlap) | PARTIAL | MEDIUM, M6 |
| `wagglegobble.lic:1003-1027` | multicast cap | 7 profession groups x circle | `(ranks/25)+1` | none | GAP | MEDIUM, M6 |
| `lib/common/spell.rb:286-330` (Lich core; used by `MIUduration.lic`) | item/scroll activator duration modifiers | 8 activators | multiplier, substitute skill | `spells.rs` `Duration::Derived` (expression only) | GAP | LOW-MEDIUM, M6 |
| `wands.lic:21-620` (Starsworn 1.2, from the wiki) | wand -> spell | 52 | name, spell, number, circle, crumbly, treasure, alchemy, notes | `gameobj-data.tsv:80-81` (wand by noun, no spell) | GAP (and `dazed-and-confused.lic:261-293`, 33 rows, all agree) | MEDIUM, M6 |
| `timers_sagapanel.lic:1063-1150` (2026) | bonuses the effect list does not carry | 8 effects | AS/CS/DS per stack or per target, caps, seconds | `spells.tsv` bonuses (empty for 608, 9084, 1016) | GAP | MEDIUM, M10 (and AS math) |
| `effectsbar_sagapanel.lic:124-157,228-249` | Debuffs/Buffs dialog names | 32 + 20 | name | `effects.rs` (open text) | PARTIAL (untyped) | LOW-MEDIUM, M6 guards |
| `symbolz2.lic:39-43`; `infomonfixspells.lic:1225-1233,1318-1322` | cast -> cooldown effect | 3 + 5 kinds | spell number pairs | `spells.tsv` rows exist, no link | PARTIAL | MEDIUM, M6 |
| `isigils_new.lic:19-34,51-53` | sigil durations, effects, exclusive pairs | 15, 2 pairs | duration, effect | `spells.tsv` 97xx durations; `sunfist.rs` (costs only) | CONFLICT on 9703/9706/9716 (Cena 17/3/3 min vs 19/5/5 in Lich `guardians_of_sunfist.rb:44,71,161`, wiki, script) | MEDIUM, M6 |
| `Society.lic:126-132` | spirit per sign | 7 | cost + 1 throughout | `col.rs` (cost), wiki | different quantity (INFERRED), not a conflict | LOW |
| `song-manager.lic:70-81` | bard renew costs | 12 | formula | `spells.tsv` renew_cost | 11 agree; 1007 cap 10 vs Cena 13 (CONFLICT, script older) | LOW |
| `mana-tracking.lic:305-330` | Sleep (501) cost | 13 bands | level -> cost | `spells.tsv` 501 (cap 5 by caster level); wiki cap 5 by target level | CONFLICT (script stale; Cena's variable is the caster's level) | LOW |
| `spellmerge.lic:227-370` (LostRanger 0.7) | third-person group-spell landing/wear-off lines | 29 entries, 21 spells | spell, line, merged form | `combat_effects.tsv` spell_loss (18 rows) | GAP (2 of 22 tested held) | MEDIUM, M6 |
| `freddie_mercury.lic:66-109`, `ssing.lic:52-169` | item-lore result patterns; charge words -> counts | 28 + 31; 10 | pattern, capture | none | GAP | MEDIUM (pile 5 overlap) |
| `pulsetimer.lic:126-178` | non-pulse mana-gain lines | 12 | line -> source | none | GAP | MEDIUM, M6 rest |
| `manatimer-sf.lic:49-73` | `mana` report and regen estimate | 5 lines, 1 formula | | none | GAP | MEDIUM, M6 rest |
| `badjuju.lic:55-150`, `untrammel.lic:40`, `aethorvars.lic:1583-1599`, `osahealthmonitor.lic:245` | room hazards and the spell that clears each | 7 hazards x realm | noun -> spell | none (no hazard category in `gameobj-data.tsv`) | GAP | MEDIUM, M6 |
| `reorder-spellbook.lic:78-317` (Zedarius 1.0.0) | one-line spell effect summaries | 209 | number -> text | `spells.tsv` bonuses (typed, 89 rows) | GAP | LOW, M10 tooltips |
| `whatis.lic:769-1487` (Luxelle 1.6b) | GemStone jargon | ~717 | term, meaning, wiki link | none | GAP | LOW, M7 |
| `dazed-and-confused.lic:33-260` | collectibles, Simucoin token effects | 95, 102 | name -> note | `gameobj-data.tsv:15` (collectibles HAVE) | PARTIAL | LOW |
| `dechanter.lic`, `juice.lic`, `tears2.lic`, `teartracker.lic`, `chanterguide.lic`, `aura.lic` | 925/735 enchanting tables | ~50 materials each, tiers, aura colours | | none | GAP | LOW (no milestone) |
| `cscalculator.lic:19-60`, `specialization.lic`, `calcsongofluck.lic:53-80` | player CS by profession; armor spec bonuses; Song of Luck item tiers | | | none | GAP | LOW |
| `rol_new.lic:17-170` | Rings of Lumnis trivia | 76 | question -> answer | none | N/A (event) | LOW |

## Captures Cena lacks or differs on
| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| `wagglegobble.lic:869-909`, `keepmealive.lic:134-179`, `smartsorcerer.lic:174-184`; source Lich `lib/common/spell.rb:21-62` | prepare and cast results (38 lines) | `^You already have a spell readied!`, `^Your spell(?:song)? is ready\.`, `^Cast at what\?$`, `^Be at peace my child, there is no need for spells of war in here\.$`, `^Your magic fizzles ineffectually\.$`, `^You do not currently have a target\.$`, `^\[Spell preparation time: \d seconds?\]$` | only `[Spell Hindrance` (`cena-behavior/src/travel/routines/casting.rs:126`) | GAP | HIGH, M6 (Engage and Maintain cast) |
| `rapid-fire.lic:121`, `shaste.lic:83`, `lessershroud.lic:114` | non-stacking refusal | `Your magic clashes with that which is already there!` | none | GAP | MEDIUM, M6 |
| `status_report.lic:229` | dispelled | `A pale, flickering nimbus coalesces around you, then vanishes in a brilliant flash!` | none | GAP | MEDIUM-HIGH, M6 (recast trigger) |
| `status_report.lic:232-234` | dispel blocked | `The raw elemental energy surrounding you takes on a watery look as it absorbs the magic.`, `A snow white haze distorts the air around you, confounding .* spell!` | none | GAP | LOW |
| `status_report.lic:237` | spellburst warning | `You suddenly feel the essence surrounding you shift and writhe chaotically!` | none (no spellburst anywhere) | GAP | MEDIUM, M6 |
| `status_report.lic:239`, `cab.lic:73-75`, `condemn.lic` | Condemn room effect start/end | `The pungent stench of decay fills the air as mist rises from the (?:floor\|ground) around you!` / `The scent of the grave around you fades away.`; `Skeletal appendages burst` | none | GAP | MEDIUM, M6 (hunt hazard) |
| `status_report.lic:241`, `powersink.lic` | Power Sink room effect | `Your thoughts scatter as you struggle to prepare any magical incantations.`, `That spell isn't ready to be cast yet` | none | GAP | MEDIUM, M6 |
| `rapid-fire.lic:91`, `status_report.lic:319` | anti-magic room (as the scripts read it) | `Your magic fizzles ineffectually` | none | GAP | MEDIUM, M6 |
| `cab.lic:101` (Nisugi); Lich `combat/defs/messages.rb:161` | PSM weapon reaction offered | `You could use this opportunity to <d cmd='WEAPON (\w+ #\d+)'>` | none; Cena deferred messages.rb (`tools/extract_combat_defs.rb:48-50`) | GAP (deferred) | HIGH, M6 (Nisugi's own hook) |
| `cab.lic:68-70`; Lich `messages.rb:96` | Sanctum of Scales creature transform (unholy quickness) | `The (.*)'s form twists and mutates, sprouting scales and cold eyes as it transforms into ...!` | none (deferred as above) | GAP | MEDIUM, M6 |
| `cab.lic:91-96`; Lich `messages.rb:155-156` | arcane reflex / arcane prowess flares | `^Vital energy infuses you, hastening your arcane reflexes!` / `^Nature's blessing of vitality departs...` | announce HAVE (`combat_effects.tsv:10`), end line none | PARTIAL | LOW-MEDIUM |
| `aethorvars.lic:3898-3902`; Lich `messages.rb:148-149` | 703 Corrupt Essence haze on a creature | `is suddenly surrounded by a blood red haze`, `blood red haze dissipates from around` | self lines HAVE (`spells.tsv` 703); creature mark none | GAP (deferred) | LOW-MEDIUM |
| `aethorvars.lic:3914` | 704 Phase outcome | `becomes momentarily insubstantial and appears lighter.` / `...but quickly returns to normal.` | none | GAP | LOW |
| `cab.lic:88` | Barkskin cast while cooling | `A knobby layer of bark begins to form on you, but rapidly crumbles away.` | none | GAP | HIGH, M6 (605 in Nisugi's signs) |
| `rapid-fire.lic:112-121`, `shaste.lic:113-124`, `lessershroud.lic:105-114` | third-person 515/535/120 on others | `X appears considerably more powerful.` / `X suddenly appears less powerful`; `X begins moving faster than you thought possible.` / `X returns to normal speed`; `X suddenly looks a lot more powerful..` | 120 wear-off HAVE (`combat_effects.tsv` spell_loss 120) | PARTIAL | MEDIUM, M6 |
| `spellmerge.lic:227-370` | third-person group spells (211, 215, 219, 307, 310, 419, 506, 611, 1006, 1007, 1018, 1035, 1125, 1213, 1216, 1605, 1609, 1613, 1617, 1618) | `X appears less confident.`, `The brilliant aura fades away from X.`, `X looks empowered.` ... | 911 and the generic line HAVE | GAP (20 of 22) | MEDIUM, M6 |
| `infomonfixspells.lic:390-433` | prep lines, self/other, multicast, item activation, scroll invoke | `^You (?:gesture\|make a complex gesture\|sing a melody\|...)`, `^Sparks begin to fly between the .+ and your fingers...`, `^([A-Z][a-z]+) (?:gestures\|makes a complex gesture\|...)` | `combat_attacks.tsv` cast rows match some `gestures` forms (checker hit `attack/cast`) | PARTIAL | MEDIUM, M6 |
| `mana-tracking.lic:302` | elemental prep lines naming the spell | `^You intone a phrase of elemental power while raising your hands, invoking (.*)\.\.\.` and 3 more | none | GAP | MEDIUM, M6 |
| `wagglegobble.lic:945-1000` | `spell active <name>` for another character | `has the following active effects`, `  Name ..... HH:MM:SS\|Indefinite`, `has spell sharing disabled` | none | GAP | MEDIUM, M6 |
| `song-manager.lic:276-343`, `play.lic:711,1175` | `song status` report; `stop` replies | `Your current renewal cost is (\d+) mana.`, `You are not singing any songs.`, `(\d{4}) Song Name` rows, `will renew in a moment\|shortly\|...`, `You stop singing X.`, `You are not singing that song.` | `spellsong.rs` has formulas only; `renew_cost` summing deferred (`spellsong.rs:34-37`) | GAP | MEDIUM, M6 (bard) |
| `symbolz2.lic:103`, `gsigns.lic:169`, `infomonfixspells.lic:931` | society refusals | `The sense of calm here nullifies your symbol`, `The power from your (?:symbol\|sign) dissipates into the air`, `Repeating the sign has no effect!` | none | GAP | MEDIUM, M6 |
| `wagglegobble.lic:814-817` | CoL recognition exchange | `^You (?:touch\|scratch\|rub\|tap\|point to) your (?:right\|left) (?:eyebrow\|nostril\|earlobe\|shoulder\|cheek) with your ...`, `<a ...>Name</a> acknowledges your sign` | none | GAP | LOW-MEDIUM |
| `pulsetimer.lic:82-178` | mana sources other than the pulse | `You feel mana pulse within the area, but it does not reach you`, `An invigorating rush of mana pulses through you`, `You feel a surge of mana flow through you`, `As you concentrate on your sigil, you feel exceptionally tired...`, `Suddenly, a small bolt of energy arcs between the two of you.  You gain` | 516 leech line HAVE (`spells.tsv` 9516) | GAP (11 of 12) | MEDIUM, M6 rest |
| `manatimer-sf.lic:49-58` | `mana` report | `^\s+Mana gained (off\|on) node:\s+(\d+)`, `MANA PULSE`, `You have used the MANA SPELLUP ability` | none | GAP | MEDIUM |
| `effectsbar_sagapanel.lic:331`; Lich `infomon/parser.rb:55-61` | Shadow Essence | `^Accumulated Shadow (?:E\|e)ssence: (\d)`, gain, cap, 5 sacrifice lines | none | GAP | MEDIUM, M6 |
| `ensorcell_tracker.lic:19`, `cab.lic:137-145` | ensorcell flare outcomes | `You feel (healed\|empowered\|rejuvenated\|reinvigorated\|energized)!` | announce HAVE (`combat_effects.tsv:41-42`) | PARTIAL | LOW-MEDIUM |
| `meditate.lic:65-78` | meditation outcomes | `You are not able to enter a meditative trance...`, `You are too injured to meditate.`, `Your meditation is interrupted by X` | 9075 start HAVE; 2 of 3 ends dropped (Finding 1) | PARTIAL | LOW |
| `play.lic:1851-1860` | 1004 Purification Song on a gem | `shatter into thousands` / `smoother and more pure in color` / `becomes more perfect` / `cannot be purified` | shatter HAVE (`ledger/town.rs:64`) | PARTIAL | LOW-MEDIUM (selling) |
| `infomonfixspells.lic:1437` | armor specialization falls off | `^As you remove your .*?, it falls out of alignment\.$` | none | GAP | LOW |
| `badjuju.lic`, `untrammel.lic:40-60`, `stupidglobe.lic:2-12` | room hazards appearing | objects `web`, `cloud`, `vine`, `swarm`, `vortex`, `void`, `tempest`; `A sphere of intensely blue radiance burns in the air` | room objects are held (`state/room.rs`) but not classed as hazards | PARTIAL | MEDIUM, M6 |
| `status_report.lic:131` vs `effect-list.xml:641` | 540 wear-off | script has one space after `focus.` | Cena has the game's two | CONFLICT (script) | none |
| `infomon_backup.lic`, `rapid-fire.lic:88`, `shaste.lic:89`, `lessershroud.lic:81` | cutthroat end | `vocal chords` | `afflictions.rs:105` `vocal cords` (Lich 5's spelling) | CONFLICT (scripts stale) | none |

## Script by script
| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| aethorvars.lic | 4720 | Aethor's personal all-in-one library (no author/version line): container sorting, looting, selling, `spell_NNN` wrappers for every circle, CoL wracking, dispel logic | 691 capture lines; 475 of them are `prepped?`/`checkprep` tests or `GameObj`/`Char.name` name filters (grep counts in Method). Real game text: CoL recognition (`:1420` `acknowledges your sign`), dispel outcomes (`:1479`), 703 Corrupt Essence outcomes (`:3898`), 704 Phase outcomes (`:3914`), 218 Spirit Servant arrival (`:2934`), bard-song squelch list (`:1684`), spellsong renew replies (`:2355`), room hazards web/cloud/swarm/vine/vortex and the spell that clears each (`:1583-1599`) | Mostly N/A (personal plumbing). Messages: GAP for 704/218/CoL-recognition/renew replies (LOW-MEDIUM); dispel outcomes (`:1479`): `bolt of energy leaps` and `blinks and looks around in confusion` HAVE (`combat_effects.tsv`); `elemental aura`/`hazy film coats` are unpinned wear-offs that Lich and Cena leave as residue by design (`spell_losses.rb:20`, `tests/combat_attack_lines.rs:403-416`) |
| whatis.lic | 1596 | Luxelle v1.6b (2017): GemStone jargon dictionary | `lookups` table `:769-1487`, ~717 rows `term\|meaning\|wiki link` (incl. every spell `NNNN / name` with circle and category) | N/A for the model; spell names already in `spells.tsv`. Glossary could seed M7 agent context: LOW |
| nuckystuff.lic | 1471 | tdusk v1.19 (Fulmen, 2020): Duskruin arena auto-fighter, misfiled here | per-profession arena routines; `Spell[n].affordable?/active?/timeleft`, timer pseudo-spells 597/598/599/9005/9607; package-empty reply `:1218` | N/A for this pile (event script). Timer ids it reads are all in `spells.tsv` (HAVE) |
| play.lic | 2275 | Air: Shattered bounty + hunt driver around bigshot and the Nexus bots | 30-row `hunting_areas` (`:26-55`, creature level/undead/box/flee flags + rooms); `song status` renewal cost (`:711`); `stop <song>` replies (`:1175,:1677,:2170`); 1004 Purification Song outcomes (`:1851-1860`); loot/xp boost activation lines (`:1446,:1745`); bounty lines | Hunting/bounty parts belong to piles 1/8. Magic: renewal-cost line GAP, `stop` replies GAP, 1004 outcomes PARTIAL (only shatter held) |
| infomonfixspells.lic | 1592 | Old infomon 1.18 (Shaelun, upload Scarswyrth): text-based skills/spells/society tracker | builds `spell_up_msgs_re`/`spell_dn_msgs_re` from every spell's `msgup`/`msgdn` (`:372-373`); caster/multicast detection (`:390-433`); special cases `:1210-1440`; `spell active` text parser `:1462-1500` | Superseded by the XML effects dialog in Lich 5 and in Cena; see Special ask 3 for the gaps it patched |
| infomoncommageddon.lic | 1590 | Same, v1.17 | diff vs fixspells: only the `spell active` header text and section lines (`:1462-1484`) | variant of the above |
| infomon_backup.lic | 1560 | Same, v1.14 (Alastir) | diff: no Core Tap charges, `vocal chords` misspelling, older CMan header | variant of the above |
| timers_sagapanel.lic | 3181 | Saga panel v1.17 (2026-09-07, RuseofFools with AI help): active spells/buffs/debuffs/cooldowns with bonus totals | bonus labels (`:868`), stack-count names `Surge of Blessings (N)`, `Crushing Dread (N)` (`:1071-1082`), non-effect-list bonuses (Camouflage 608, Item Supercharger 9084, Brine of the Rusalka, Rage, Mystic Resonance, 425 scroll floor `:1063-1150`), Invoker schedule (`:479-495`) | bonus facts GAP (MEDIUM, M10 GUI / M6 AS math); stack counts in effect names PARTIAL |
| effectsbar_sagapanel.lic | 1557 | Saga panel v1.7.0 (2026-08-28): effects/vitals icon bar | Debuffs-dialog name vocabulary (`:124-157`, 32 names incl. `Wall of Thorns Poison 1..5`), buff names (`:228-249`), armor-support group 9504-9510 (`:205`), Shadow Essence via `resources.shadow_essence` (`:331`) | Debuff names: Cena holds them untyped (PARTIAL); Shadow Essence GAP, MEDIUM (M6, sorcerers; Lich `infomon/parser.rb:55-61`) |
| dazed-and-confused.lic | 338 | Dreaven v2: annotates item names with what they do | `all_info` 247 rows: 95 collectibles, 102 Simucoin items, 33 wand->spell, 13 enchant potions, 4 misc (`:33-300`) | collectibles HAVE (`gameobj-data.tsv:15`); wand->spell GAP (MEDIUM, M6 hunt wands); Simucoin tokens LOW |
| dechanter.lic | 315 | Claudaro v1.0.1 (2018): 925 enchanting reference | enchant difficulty `:24`, temper times `:49`, 50+ material modifiers `:63`, potions `:135`, water-lore table `:170` | GAP, LOW (no enchanting milestone) |
| dreavening.lic | 878 | Dreaven: helper for a player-run mass spell-up event | other-caster prep aura `Mirage-like distortions surround X as X prepares a spell...` (`:349`); meditation start/end (`:782-784`, noting effect-list does not track meditation); `spell active` duration lines for 13 spells (`:742-766`); mana-bread names (`:314`); group open/closed (`:797-817`) | Event-specific; `spell active` text GAP (see waggle); rest N/A |
| wagglegobble.lic | 1305 | waggle 0.18.2 (elanthia-online, Tillmen; 2022-03-09): the canonical spell-up | reads `Spell#stackable?/refreshable?/time_per/max_duration/multicast` for every decision (`:1059-1095`, `:1157-1281`); `spell active <name>` parser for other characters (`:928-1000`, incl. `has spell sharing disabled`, name fixes `Mage Armor - `, `Cloak of Shadows - `, `Raise Dead Recovery`); cast refusals (`:903-909`); `release` replies (`:869`); multicast cap by profession and circle (`:1003-1027`); CoL recognition (`:814-817`); mental-acuity stamina cost x2 (`:874`) | the stacking attributes are exactly what `spells.tsv` drops: CONFLICT with port-data-whole, HIGH (M6 Maintain). `spell active <target>` GAP MEDIUM (M6 group) |
| waggle_spell_active.lic | 1307 | waggle 0.17 (Tillmen, 2021-08-12) | same logic, older header | variant of wagglegobble |
| waggle4clerics.lic | 1230 | waggle 0.14b modded by Santa (2021-01-31) | adds "cast 320 first, saves >= 20% mana" (`:header`, claim UNVERIFIED) | variant |
| juice.lic | 302 | Elkiros v1.1 (2020): 735 Ensorcell cost/chance reference | tier costs `:29` (10k-30k), material modifiers `:34`, roll descriptions | GAP, LOW (no crafting milestone) |
| juice2.lic | 308 | Ponclast's alternate of juice | same data plus skill scrapes | variant |
| tears2.lic | 363 | Alfryd v2.0: 925 tears/essence reference | `sense` essence line `:276`, tier data | GAP, LOW |
| status_report.lic | 388 | Alastir: status monitor on AlastirLib | 71 self wear-off lines mapped to spell numbers (`:59-199`); dispelled / dispel-blocked lines (`:229-234`); spellburst warning (`:237`); room hazards Condemn and Power Sink (`:239-241`); thorn-poison stages (`:206-254`); cast failures (`:266,:319`) | wear-offs: 69 of 71 match a `spells.tsv` msg_end (checker, Method); 540's miss is the script's own typo (one space where the game sends two, `:131` vs `effect-list.xml:641`); 1613's line (`:193`) is in neither `spells.tsv` nor `effect-list.xml`, GAP LOW (which spell it belongs to UNVERIFIED); dispel/burst/room-hazard lines GAP (MEDIUM-HIGH, M6) |
| monitor.lic | 366 | older copy of status_report | lacks the dispel/burst/hazard block | variant |
| burst_calc_new.lic | 166 | Gibreficul, modified by Lavastene: spellburst estimate | formula `:43-54` (self-described as approximate), 30 burst-exempt spells `:42`, per-profession half-cost circles `:61-120` | GAP (Cena has no spellburst anywhere), MEDIUM (M6 for multi-circle spell-ups) |
| burst_calc.lic / jbc.lic / azburst.lic / burst_calc_no_trust.lic | 167-181 | variants of the above (Greminty 2018 update in no_trust) | same table | variants |
| burst.lic | 239 | Alastir from Maodan v1.0 (2023-04-14) | sever logic with 15 exempt group spells `:22` | variant of severance |
| severance.lic | 350 | Kaetel v1.0.12 (2026-09-03): stop spells at risk in Spell Sever zones | 16 sever-exempt group spells and 4 short group unlocks (`:65-78`), default limit 2 (`:61`) | GAP. The exempt list disagrees with `spells.tsv` `availability`: 9 of the 16 are `self-cast` or `all` there (CONFLICT-shaped, MEDIUM, M6) |
| sever.lic | 147 | LostRanger 0.0.1 (2022): same idea | exempt list | variant |
| song-manager.lic | 602 | Dreaven v18: keeps bard songs up | `song status` report (`:276-343`: renew-time phrases, `Your current renewal cost is N mana.`, per-song rows); renew-cost table (`:70-81`) | status report GAP (MEDIUM, M6 bard Maintain); renew costs match `spells.tsv` except 1007 (CONFLICT, LOW) |
| jlsing.lic | 820 | Jara: loresinging verse driver | loresing outcomes (`:96-306`) | value line HAVE (`ledger/town.rs:62-63`); progress/fail lines GAP, LOW |
| freddie_mercury.lic | 433 | v0.6.0: loresing + inspect an item into CSV | `REGEX` table of 28 item-lore patterns (`:66-96`) and `CHARGE_MAPPING` words to counts (`:98-109`) | GAP, MEDIUM (magic-item facts; pile 5's area) |
| share.lic / sharetest.lic / osashare.lic | 889-929 | Dreaven v5: group spell-up party with help-share | casts-to-250 per circle (`:217-224`), multicast by profession (`:226-250`), group lines | multicast formula GAP (same as waggle); group lines HAVE (`state/group.rs:168-223`) |
| help-share.lic / help-osashare.lic | 138-141 | companions of share | OOC whisper protocol between scripts | N/A |
| staymounted.lic | 952 | Ruseoffools v1.3.21: keep a summoned mount | `You are currently riding` (`:282`) | N/A to this pile (mounts), LOW |
| character.lic | 282 | Alastir: misc event hook (clench, notes, harpoon) | same lines as status_report `:101-232` | covered above |
| arena-cleavet.lic | 418 | Nylis: arena dodge-cue responder | cue phrases (`:50`) | N/A (arena event) |
| oc_heist.lic | 338 | Alastir 1.0: Ophidian Cabal heist event | event lines | N/A (event) |
| tarot.lic | 110 | card meanings | 77 strings | N/A (flavor) |
| rol_new.lic | 350 | Alastir, edited Khylynnia: Rings of Lumnis trivia auto-answer | 76 question/answer pairs (`fput "answer` count) | N/A (event trivia), LOW |
| ttap.lic | 473 | whisper-driven spell bot | whisper keywords (`:32-455`) | N/A |
| itemnotes.lic | 236 | item notes and activation | activator verbs, `Maybe you have to wear it first`, `You lose control` (`:149`) | GAP, LOW |
| badjuju.lic | 153 | Glaves v2.2: dispel room hazards | hazard nouns cloud/void/tempest/swarm/vine/web/vortex and the spell per realm that clears each (119 spirit; 505, 417, 912 elemental) (`:55-150`) | GAP: Cena has no room-hazard category (`gameobj-data.tsv` categories listed in Method). MEDIUM (M6 hunt) |
| arenawizard.lic | 444 | Elvidor v0.7: Duskruin arena wizard | arena lines, stomp reply `:290` | N/A (event) |
| sigilz.lic | 463 | GoS sigil keeper 2.0.0 (EO; Ifor Get, SpiffyJr, Tillmen, Hailye) | uses `Spell[]`, society rank; no own table | HAVE (`societies/sunfist.rs:100-292`) |
| Society.lic | 132 | per-character sigil/sign keep-up | cost rows `[name, rank, number, mana, stamina, spirit]` (`:4-55`, full table `:106-132`) | sigils and mana signs match `sunfist.rs`/`col.rs`; spirit signs are uniformly Cena's cost + 1 (INFERRED: spirit needed, not cost). HAVE |
| 918wands.lic | 545 | RuseOfFools with AI help: Duplicate (918) treasure wands | cast/sell replies | N/A to this pile (wizard trade), LOW |
| chanterguide.lic | 306 | 925 enchant helper | `sense` workshop line, cast-at-item enchant readout: aura colour -> base enchant 5..50, recognition word -> modifier, step words (`:22-85`) | GAP, LOW (crafting) |
| imbed_rods.lic | 219 | 420 imbed helper | imbed success/fail lines (`:8-15`) | GAP, LOW |
| itemagic.lic | 447 | Juspera 0.1.0: magic-item manager | crystal/amulet names | LOW |
| mana-tracking.lic | 482 | Tgo01 v7: mana spent per spell | 4 elemental prep lines naming the spell (`:302`), release/cast-RT closers (`:332`), Sleep cost table by level up to 13 (`:305-330`) | prep lines GAP (MEDIUM, M6 cast confirm); Sleep table CONFLICT with `spells.tsv` 501 and the wiki (`Sleep _501_.txt:19`, capped at 5): script stale, LOW |
| osahealthmonitor.lic | 257 | OSA crew health monitor | status words, room hazards by name (`:245`) | hazards GAP (as badjuju) |
| raisedead.lic | 311 | Jara: raise-dead service | `appraise <player>` max line (`:164-170`); Voln `sym recognition` reply `X is a (member\|Master)` (`:250`) | GAP, LOW |
| keepmealive.lic | 237 | Nisch from Tillmen: spell keep-up | same refusal and CoL lines as waggle | variant of waggle |
| miu.lic | 225 | magic-item use helper | activator replies | LOW |
| cscalculator.lic | 240 | Gibreficul (2014): player casting strength by profession | per-profession circle pairs and bonuses (`:19-60`) | GAP, LOW |
| spellmerge.lic | 1077 | LostRanger v0.7 (2020-05-03): merges third-person spell wear-off lines | `EFFECTS` table: 29 third-person group-spell landing and wear-off entries on 21 spells (`:227-370`) | 2 of the 22 single-line entries tested match Cena (`combat_effects.tsv` spell_loss 911, generic); rest GAP, MEDIUM (M6 group Maintain) |
| masscomm.lic / automassies.lic | 343 / 494 | Vailan (2025 / 2022): mass blur/guard/colors announcer | spell-name aliases 911/419/611 | N/A |
| Tysong-duskattack.lic | 78 | Tysong 1.3: arena caster routine | arena | N/A |
| alastirlib.lic | 489 | Alastir 1.0.0 library | clench reply `:159` | LOW |
| teartracker.lic | 377 | Claudaro: 925 static-essence accumulation (Nov 2019 system) | 7 accumulation tiers, saturation, overwhelm, earthnode/anchored pulse (`:111-349`) | GAP, LOW (crafting) |
| necrojuice.lic | 92 | Elkiros 2020: 735 necrotic energy build-up | 13 build-up lines (`:24-72`) | `resource` amounts HAVE (`character/standing.rs:498`); the sense lines GAP, LOW |
| ensorcell_calc.lic | 253 | Xanlin v5: 735 cost calculator | formulas only | LOW |
| ensorcell_tracker.lic | 33 | Gibreficul: ensorcell flare outcomes | `You feel (healed\|empowered\|rejuvenated\|reinvigorated)` (`:19`) | flare announce HAVE (`combat_effects.tsv:41-42`); the resource outcome lines GAP, LOW-MEDIUM (M6 ledger/vitals) |
| ssing.lic | 301 | SpiffyJr 1.0: loresing an item | 31 item-lore result matches (`:52-169`: weight, charges, enhancive boosts, persistence, spell contained, resistances, capacity) | GAP (as freddie_mercury), MEDIUM |
| star-sing.lic / SitNSing.lic / sonezz.lic | 237 / 108 / 53 | loresing drivers; SitNSing has the offer/accept trade lines | loresing verses, `Click ACCEPT` offer lines | value line HAVE; rest GAP LOW |
| pushover.lic | 559 | push notifications on matched text | user patterns | N/A |
| kpop.lic | 204 | Kaldonis 2.0: pop boxes/gates with 407/304/1207/1604 | WL graveyard and IMT rolaren gate opening lines (`:121`) | Cena already opens these gates (`travel/routines/bronze_gate.rs`, `rolaren_gate.rs`); box popping belongs to pile 5 |
| umadbro.lic / signspacer.lic / gsigns.lic / keepsigns(tail) | 255 / 341 / 311 | CoL sign keepers (Veni 0.3; Rinkidinkicus 1.2.0 2026-04-02; Gwrawr 1.2) | CoL recognition, `The power from your sign dissipates` (`gsigns:169`), spirit signs +20 AS/+20 DS/+15 TD (`signspacer:42-45`) | sign costs HAVE (`societies/col.rs`); the refusal line GAP LOW |
| no-spirit-death.lic / wracksafe(tail) | 71 | Dreaven v1: refuse a sign that would spirit-death you | pending spirit per active dissipating sign: Swords 1, Shields 1, Dissipation 1, Madness 3 (`:37-42`) | costs HAVE and agree (`col.rs`); the summing is Lich's `pending_spirit_loss`, deferred by Cena on purpose (`societies.rs` module doc) |
| isigils_new.lic / isigils_with_bandages.lic | 378 / 169 | Sunfist sigil keeper | `SIGIL_DATA` 15 rows with mana, stamina, **duration** and effect (`:19-34`); mutually exclusive pairs minor/major bane and protection (`:51-53`) | costs HAVE; durations: see CONFLICT on 9703/9706/9716 (Data table); exclusivity GAP LOW |
| symbolz2.lic | 180 | Voln symbol keeper 2.0.6 (EO; Ifor Get) | symbol -> cooldown effect map 9813->9048, 9812->9049, 9819->9050 (`:39-43`); refusal lines `nullifies your symbol`, `power from your symbol dissipates` (`:103`); 3 rooms where symbols fail (`:45`) | costs HAVE (`societies/voln.rs`); the cast->cooldown link is only implied by names in `spells.tsv` (PARTIAL); refusals GAP (MEDIUM, M6 Maintain) |
| VolnStep24-RuneSolver.lic / step24.lic / Melgorehn.lic | 142 / 187 / 81 | Voln advancement step 24 rune puzzle | rune positions, cab and tunnel lines | GAP, LOW (society task) |
| raisedead.lic | 311 | covered above | | |
| fizzfuse.lic / infuse.lic / infuse_scroll.lic | 475 / 252 / 387 | scroll infusion (Fizzleworth 1.1; NotAramund 0.2) | infuse result lines (`fizzfuse:280-324`) | GAP, LOW |
| donoharm.lic | 256 | Tgo01 v4: refuse harmful spells at players, helpful at creatures | 109 offensive spell numbers (`:22`), **26 room-wide attack spells** (`:27`), group lines | room-wide list vs `spells.tsv` `area`: 4 overlap (410, 435, 909, 912); 22 room-wide spells lack `area`; `spells.tsv` also marks 518 area. PARTIAL, MEDIUM (M6 target choice) |
| cab.lic | 149 | **Nisugi** v1.0: combat counters for his archery script | barkskin proc and 301 s cooldown (`:85-87`), Condemn start/end (`:73-75`), Sanctum transform (`:68`), bear trap (`:63`), weapon reaction `You could use this opportunity to` (`:101`), arcane reflex (`:91-96`), five flares, ensorcell outcomes | flares HAVE (`combat_effects.tsv`); barkskin absorb HAVE (`combat_results.tsv`); reaction, sanctum, arcane reflex are pinned in Lich's `combat/defs/messages.rb:96-161`, which Cena deferred (`tools/extract_combat_defs.rb:48-50`): PARTIAL, HIGH (M6). Condemn end, bear trap GAP |
| manabattery.lic / sendmana.lic / mana_balance(tail) | 188 / 133 | mana sending between characters (Oweodry 2013; Vailan 2023) | send success `Suddenly, a small bolt of energy arcs between the two of you` and failure (`sendmana:115-126`) | GAP, MEDIUM (M5 multi-session mana sharing) |
| passive-spellbot.lic | 324 | Tillmen 0.2: unasked spell-up | stackable cap 250 min (`:193`), other-caster gestures, `Your spell misfires.` (`:219`) | uses the stacking attributes Cena drops (see waggle) |
| partner3.lic / tap.lic / tap2.lic / ttap | 108 / 152 | player-to-player cue scripts | social cue lines | N/A |
| scrollcast.lic / scrollbuff.lic / star-book.lic / 1b2b3b.lic | 310 / 135 / 1001 / 88 | scroll and spellbook invocation | `You suddenly feel enlightened`, `takes hold in your mind` (scroll read), `You don't have enough mana` | GAP, LOW-MEDIUM (M6 scroll spell-up) |
| triage.lic | 943 | LostRanger 0.1.5: cleric dead-body window | appraise lines | LOW |
| useimbed.lic / chargeimbed.lic / imbed(tail) | 214 / 132 | Tillmen: imbed charge tracking, 517 charge | `You feel terribly drained!`, `The pulsating orb quickly implodes` (`chargeimbed:115-129`) | GAP, LOW |
| pulsetimer.lic | 261 | Caithris 0.2: mana regen pulse timer | 12 non-pulse mana-gain sources (`:126-178`): manna bread, 516, Sign of Wracking, mana received, Song of Unravelling, Rift, MANA PULSE, Symbol of Mana, Sigil of Power, level-up; dead pulse `You feel mana pulse within the area, but it does not reach you` | GAP (special ask 4), MEDIUM (M6 rest) |
| manatimer-sf.lic | 116 | Nomada 0.1: mana regen timer | `mana` command report lines (`:49-58`), regen estimate `mc0/10 + mc1/20 + mc2/20 + 15% max` (`:73`) | GAP, MEDIUM (M6 rest) |
| mana-tracking.lic / companion-mana-tracking.lic | 482 / 185 | covered above / its companion | | |
| enchantnotes.lic / enchantcheck.lic / essence.lic / aura.lic | 261 / 251 / 53 / 38 | 925 enchanting aids (Jymamon 2019, Vailan 2020) | aura colour/intensity -> enchant value (`aura:28`), essence sense lines | GAP, LOW |
| osacrewlite.lic / osastowaway.lic | 197 / 146 | OSA ship crew | Sea Hag's Roost task lines | N/A to this pile (pile 8) |
| itemmagic.lic / miu / brooched.lic | 156 / 225 / 298 | magic-item and Lumnis brooch use | brooch charges `(\w+): N out of N` (`brooched:96`) | LOW |
| getspells.lic / unified-effects.lic / spellactive2.lic / 1140solace.lic | 218 / 227 / 144 / 106 | effect display; SpiffyJr keeper; 1140 Solace healer | `spell active` text (`getspells:128`, `1140solace:21`); `dialogData id='(Cooldowns\|Debuffs\|Buffs\|Active Spells)'` (`unified-effects:146`); 1140 refusals `You are still recovering from your last Solace` (`:81`) | dialog HAVE (`effects.rs`); `spell active` text GAP; 1140 lines GAP LOW (M6d heal) |
| rapid-fire.lic / shaste.lic / lessershroud.lic | 147 / 156 / 140 | one template: keep 515/535/120 up on self and named others | third-person up/down for 515, 535, 120 (`rapid-fire:112-121`, `shaste:113-124`, `lessershroud:105-114`); `Your magic clashes with that which is already there!`; cast blockers (web, exertion, stun, armor, nerves, cutthroat); `Your magic fizzles ineffectually` read as anti-magic room | 120 down HAVE (`combat_effects.tsv` spell_loss 120); the rest GAP. The 535 self-cast start line these wait on is one of the 98 messages the extractor drops. MEDIUM-HIGH (M6) |
| untrammel.lic | 127 | Oweodry 2012 (from Subzero's webkill): clear webs | web dispellers `[209, 906, 417, 119]`, 209 for a webbed player, opaline dust fallback (`:40-80`) | room-hazard GAP (as badjuju), MEDIUM (M6) |
| spellfade.lic | 315 | 1.1.0: announce effects that ended | aliases `multi-strike cooldown` -> `MStrike Cooldown`, `beacon of courage` -> 1608 (`:38-43`) | HAVE in effect (Cena keys by id); LOW |
| bardmana.lic | 63 | "DR-Bardmana" (`:3`): DragonRealms mana-perception words | 21 intensity words | N/A (DragonRealms, deferred by settled decision) |
| rcast.lic / lp.lic | 156 / 141 | prepare-and-cast helpers; 116 Locate Person | `Your spell is ready.(   [Spell is stored])?` (`rcast:22`), `already have a spell readied` | GAP (Lich `spell.rb:21-33` prepare lines), HIGH with the cast-result set (M6) |
| rogue-lmas-sense.lic | 72 | rogue guild LMAS SENSE task | room-condition phrases | LOW (pile 7) |
| startup-manager.lic / fixit.lic / flextape.lic / invmanager.lic / xdrop.lic | | Lich plumbing | none | N/A |
| tcbeast.lic / pokemon.lic | 165 / 69 | spirit beast capture (Tysong) | detect/capture/bound lines (`tcbeast:33-145`) | GAP, LOW |
| gospaladin* / 1604.lic / 1640rant.lic | 185 / 115 / 33 | paladin helpers; 1604 Consecrate outcomes | consecrate success/fail lines (`1604:54-71`) | GAP, LOW |
| animate.lic / animate_refresher.lic / animatekeeper.lic | 154 / 294 / 296 | necromancer 730 Animate Dead upkeep | `sacrifice channel` refresh, capacity line | Shadow Essence use GAP (see effectsbar row) |
| meditate.lic | 84 | Kaldonis 1.2: forced meditation | 5 meditation outcomes (`:65-78`) | 2 of 3 Meditation (9075) end lines are among those the extractor drops; start HAVE |
| smartsorcerer.lic / spellcaster.lic / voodoo.lic | 277 / 418 / 190 | casting wrappers (Yasutoshi 2014; Selandriel; Tillmen 2.2) | `Cast at what?`, `It looks like somebody already did the job for you.`, cast RT regex | GAP (cast results), HIGH (M6) |
| 506celerity.lic / cel.lic | 73 / 67 | Gwrawr: group Celerity with target cooldown tracking | uses gwrawrmon cooldown feed | HAVE (`state/cooldowns.rs`, 506 is one of the five cooldown spells) |
| tboost.lic | 88 | Tysong 1.0: REIM mana boost | REIM status report lines (`:37-54`) | GAP, LOW (event) |
| ghoul_2022.lic / ghoul.lic / ghoul_ci.lic / CI_GHOUL.lic | 55-123 | GHOUL bingo event | caller lines | N/A (event) |
| Anchor_1020.lic / short-haste-message.lic / empheal.lic / raise.lic / pray2.lic / unlock.lic / curseditemhandler.lic / reevoke330or735.lic / wanddupe.lic / eblade-gsf.lic / snitchstiches.lic / stupidglobe.lic / specialization.lic / calcsongofluck.lic / spell_check.lic / reorder-spellbook.lic | 33-1473 | single-purpose helpers | notable data only: curse items (`curseditemhandler:9`, HAVE in `gameobj-data.tsv:16-17`); 209 one-line spell effect summaries (`reorder-spellbook:78-317`, LOW, M10 tooltips); 1216 and 1609 end when you leave the group (`spell_check:18`, LOW); armor specialization bonus tables (`specialization`, LOW); Song of Luck item formula and tiers (`calcsongofluck:53-80`, LOW); `Roundtime changed to N seconds.` (`short-haste-message:34`) | as stated |

## Tail

197 scripts with capture + data < 5, by name, grouped (opened where the name suggested data):

- **Keep one spell or ability up** (cast when down, often with a hard-coded character name):
  spells, 650boost, beseech, tangleweed, powpow, uberspells, uberspells_no_lumnis, yatk, ybatk,
  MMDisp, autonode, crookedintent, msreliever, rapid, rapid2, hasteme, keep_spells, 203, rally,
  servant, wizshield, adrenalin, auto_spells, bless, bread2, familiarkeeper, mal, parasite,
  shroud, smart_spellup, tremor, tremors, wol, zap, evokerer, flesh, powersong, rebless,
  shieldup, wallup, wizbot, 502, leech, porcupine, power, smastery2, surgehot, 320, 506, keep213,
  tethereal, 1608, 1708, 515, celeryme, earthen, keep1625, keep_spell_active, lajespur, mana,
  pray, stomp2, stompy, storm, tap2win, tappy, xxx306, 1615isdope, 1625, 203_rest, 9009,
  flameaura, keepbeacon, keepbless, nfury, provoke, som, spelltap, steam, xtonis, 1030_loop,
  225ClericFogger, PremSong, Wiz-Meteor, aspect, unendingaspect, restspell, spellospells, wbread,
  staminaburstup, gsing, loresong, manashare. N/A (they add nothing beyond `Spell[]` calls).
- **Society keep-ups**: keepsigns (Tillmen), sigilpower, ezsigns, societypowers (Cait),
  staminamina, sstamina, symmanaloop (`You feel Koar's blessing return to you.` = 9048's end,
  HAVE), wrack (Drafix), wracksafe (Levvia 2016, pending-spirit check, see ask 4). Covered by
  ask 4.
- **Group and mass spell-up bots**: massies, gospaladin2, gospaladinfinal, masscolors,
  helpfolks, medwaggle, waggleroom, shareroom, autoinvite, invite, whisperbuffs,
  rubmeforabless, blessexample, mcom, mana_balance (equalises mana across several characters,
  capped at 300: a multi-session idea, M5, LOW). N/A otherwise.
- **Displays and timers**: 1202check, activespells, inactivespells (Tillmen), dispel_tracker,
  spelltimer, song-timer (LostRanger 2017), mana_report, mana2, manapool, promptinfo, 203_info,
  readycheck, spellnum, spellnumbers, goals, keepthoughts, string_manager; **timeminder**
  (Vailan 2024-01-28: Invoker schedule and the 16 spells the Invoker grants,
  `timeminder.lic:36`; same schedule in `timers_sagapanel.lic:479-495`; GAP LOW);
  **sfclock** (Elanthian weekday names from the real date, `sfclock.lic:37-60`; Cena's
  `state/clock.rs` has no calendar names; GAP LOW, M10); **MIUduration** (reads Lich's activator
  modifiers, see ask 2).
- **Squelch and cosmetic**: shutup (Selema 2.0, **162 third-person spell-message fragments**,
  `shutup.lic:6-170`: another source for the third-person gap, MEDIUM with spellmerge),
  spell_squelch (Alastir), sholler, cutiepie, losespell, short-haste-message (substantive list).
- **Hazard reactions**: condemn (`Skeletal appendages burst`), powersink (`That spell isn't
  ready to be cast yet`), infected (cure poison/disease), anti-trollsblood (drop 1125 cast on
  you). Covered in Captures.
- **Reference data**: **circle3** (5,363 lines, "Magic circle database" by Ross/Arrot: prose
  descriptions per spell that state stacking in words, "not cumulative, but may be refreshed";
  LOW, superseded by effect-list attributes once ported); **wands** (Starsworn 1.2, 52 wands,
  in the Data table: MEDIUM); dump-spell-data (LostRanger research uploader, N/A).
- **Plumbing and misc**: waitfor, wait_for, wait_then_execute, halt, pauseforspells, relocate,
  reverse_locate, locker, armoires-are-stupid, chalk, chalk2, fragment, runerrand, scribe, ps,
  pickupweapon, habitual_side_stepper, xhouse, allnpcs, claim, 1601room, candycomb, crystals,
  bogbuddy, dajesus, diss, beastbelly, tcpet, imbue_amulet, aniwand, ezprice, checkstaff,
  miutwo, warning, vault, fastcc, ImplodeTheRoom, gateway, burge, spiritservantgiverunestaff,
  star-cannibalwatch, thumb-ring, constant404, imbed, enchant (925 helper stub),
  spritearmor (Ralkean: sprite armor reserve, `analyze`/`touch`), wutang (a Dreavening
  trigger). N/A. (`burst` falls in the tail by count but is covered in Script by script.)
- **Not GemStone**: bardmana ("DR-Bardmana", `bardmana.lic:3`): DragonRealms, N/A by settled
  decision.

## Method

Scratch tools are in `survey/tools/6-magic-effects/`: `ex.py` (header lines and unique capture
lines per script), `msgcheck.py` (every `=~ /.../` literal and quoted `matchwait`/`dothistimeout`
string on the given lines, turned into a sample sentence and tested against every pattern in
`spells.tsv` msg columns, `combat_{attacks,effects,results}.tsv` and `creature_messages.tsv`,
else a literal grep of `crates/**/*.rs`), `cg.sh` (fixed-string grep of Cena crates and
`reference/lich-5/lib` per fragment), `typos.py` (message-pattern anomalies), and outputs
`sr.tsv`, `todo_msgs.txt`, `todo_heads.txt`.

**Checker limits, stated so its misses are not read as facts.** It tests one sample per
alternative, so a fragment built from `#{}` interpolation or heavy regex reads NONE; those were
judged by hand. It first allowed a missing trailing `.`/`!`, which made the broken 605 pattern
look like a match; the anchored test below is what caught it. Python's `str.splitlines()`
splits on the TSV's `\x1e`/`\x1f` separators, which first hid every multi-duration row; the
checker splits on `\n` only.

Counts and the commands behind them:

- Pile: `awk -F'\t' 'NR>1 && ($3+$4)>=5' pile-6-magic-effects.tsv | wc -l` -> 163 substantive;
  `... <5` -> 197 tail; 360 rows.
- `spells.tsv` fill: `grep -v '^#' data/spells.tsv | awk -F'\t' 'NR>1{n++; if($11!="")a++; if($12!="")b++; if($13!="")c++} END{print n,a,b,c}'` -> 515 273 255 2.
- effect-list.xml (the file `spells.tsv:3` names, mtime 2026-09-13): Python `ElementTree`
  counts of `message` by type -> start 336, end 290, target-start 2; messages beyond the first
  per (spell, type) -> 98 on 43 spells; spells carrying `span=stackable` 96, `refreshable` 56,
  `multicastable` 83, `max` 19, `persist-on-death` 34, `real-time` 10, `stance` 16,
  `incant` 17, `channel` 15, `cast-proc` 109; `cost` elements with `cast-type`: 0 (so costs
  lose nothing).
- Runtime consumers of messages:
  `grep -rn "message_up\|message_down\|target_start" --include=*.rs crates | grep -v src/spells.rs`
  -> only `tests/spells.rs`; `spells.rs:398-420` and `state/chunks.rs:353` are the landing path.
- Barkskin: `python -c "import re; print(bool(re.fullmatch(r'The knobby layer of bark on you creaks and twists briefly before disintegrating.\.', 'The knobby layer of bark on you creaks and twists briefly before disintegrating.')))"`
  -> False. `typos.py` over all messages -> only 605 (the other hits are `...` ellipses).
- status_report: `python msgcheck.py status_report.lic 55 260` -> lines 59-199: 71 fragments,
  69 match `spells.tsv`, 2 NONE (540 typo; 1613).
- aethorvars noise: `grep -cE "matchtimeout|matchwait|waitforre|DownstreamHook|Regexp.new|%r\{|when +/|=~ */|dothistimeout" aethorvars.lic`
  -> 691; of those, `grep -cE "prepped\? =~|checkprep =~|GameObj|\.noun =~|\.name =~|Char.name =~"` -> 475.
- Wands: regex extraction of both tables and a dict diff -> wands.lic 52, dazed 33, 0 disagreements.
- donoharm vs `area`: numbers from `donoharm.lic:27` looked up in `spells.tsv` column 3 -> only
  410, 435, 909, 912 carry `area`; `awk -F'\t' '$3~/area/'` -> 410, 435, 518, 909, 912.
- Sever vs availability: 20 numbers looked up in `spells.tsv` column 4.
- Sigil durations: `grep -n "long_name:\|duration:" lib/gemstone/societies/guardians_of_sunfist.rb`
  and the wiki table rows at `Guardians of Sunfist.txt:132-150`, against `spells.tsv` 9703-9719.
- Infomon variants: `diff infomoncommageddon.lic infomonfixspells.lic`,
  `diff infomon_backup.lic infomoncommageddon.lic`.

Absence claims, and the searches behind each (all `grep -rn` or `grep -rlF` over
`E:/Cena/crates --include=*.rs --include=*.tsv`, plus `reference/lich-5/lib` where stated):

- cast results: `fizzles ineffectually`, `already have a spell readied`, `You do not know that spell`,
  `don't have any mana`, `casting at nothing but thin air`, `Spell preparation time`,
  `You can only evoke certain spells`, `Cast at what?`, `Be at peace my child` -> none in Cena
  (all present in `lich-5/lib/common/spell.rb`).
- spellburst: `grep -rn -i "spellburst\|spell burst"` over crates, plan and `reference/lich-5/lib` -> none.
- Spell Sever: `grep -rn -i "spell sever\|severance\|spellsever"` over crates and `reference/lich-5/lib` -> none (bare `sever` hits are severed limbs and severity: `crit.rs`, `herbs.rs`, `afflictions.rs`).
- hazards: `gameobj-data.tsv` category list (`cut -f1,2 | sort -u`) has no hazard/web/cloud
  category; `grep "web\b\|cloud\|vine\|swarm\|vortex\|tempest" gameobj-data.tsv` finds only
  unrelated names.
- Shadow Essence: `Accumulated Shadow`, `violently shatter the bond`, `sacrifice your victim` -> none
  (Lich `infomon/parser.rb:55-61` has them).
- mana: `Mana gained`, `MANA SPELLUP`, `full magical power again`, `Mana Control abilities`,
  `invigorating rush of mana`, `surge of mana flow through you`, `small bolt of energy arcs between`,
  `mana pulse within the area` -> none.
- song status: `renewal cost is`, `You are not singing any songs`, `not singing that song` -> none.
- spell active text: `has the following active effects`, `has spell sharing disabled`,
  `following active effects` -> none (the only `You currently have the following` hit is the
  bank's `amounts on deposit`, `state/bank.rs:343`).
- third person: `appears considerably more powerful`, `suddenly appears less powerful`,
  `moving faster than you thought possible`, `returns to normal speed`,
  `magic clashes with that which is already there`, `world slow down around you` -> none.
- society refusals: `nullifies your symbol`, `power from your symbol dissipates`,
  `power from your sign dissipates`, `Repeating the sign has no effect`, `acknowledges your sign` -> none.
- messages.rb port: `grep -rn "weapon_reaction\|sanctum_transform"` -> none; the only `Messages`
  hits are the speech module (`state/message.rs`).
- item lore: `provides a boost of`, `charges remaining`, `contains the spell`, `is magic resistant` -> none.
- Invoker: `Invoker` -> none in crates (Lich `infomon/activespell.rb:8` mentions it).

Not done: no log-archive search (brief), no network, no `cargo`. Coverage: **40 scripts read
deeply** (logic and data blocks: the three infomon versions, three waggles, status_report,
song-manager, severance and burst_calc, the two Saga panels, cab, spellmerge, Society,
isigils_new, symbolz2, mana-tracking, pulsetimer, manatimer-sf, donoharm, badjuju, untrammel,
rapid-fire, no-spirit-death, share, freddie_mercury, jlsing, dazed-and-confused, dechanter,
juice, dreavening, aethorvars, whatis, play, nuckystuff, meditate, spell_check, wands), **123 at
capture-line depth** (`ex.py` + `msgcheck.py`), **197 tail by name**, 21 of them opened.
