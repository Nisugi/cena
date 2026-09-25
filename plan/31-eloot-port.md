# 31 — The Loot behavior: eloot, measured and ordered

**Status: Stages 1, 2 and 3 BUILT 2026-09-24.** Stage 3 in the planner: eloot's two passes
(the specials one by one, then `loot room`), a critter's bag opened, looked in and emptied
before it is taken, the three uncommon Hinterwilds names dragged by name, Sigil of
Determination (`incant 9716`) cast once when a search fails on condition, and a stow the
text did not confirm confirmed by the bag's contents (`state.inventory`). The *at your
feet* line needs nothing of its own: a searched corpse's finds land on the floor, which
the planner loots next. Stage 1 is `crates/cena-behavior/src/loot/`
(profile, importer, worth, outcome, planner; 24 tests in `tests/loot_*.rs`, Nisugi's
`eloot.yaml` importing whole with no notes). Stage 2 is the hunt driver's `loot` loop
(`hunt/drive.rs`): the engine says `Said::Loot(corpses)` when the character has a loot
profile, the driver runs the planner step by step through the gate, reads each reply
through the outcome classifier, keeps what it learned for the next room, and hands
`BagsFull`/`BoxInHand` back as the rest reasons `Why::Loaded`/`Why::BoxInHand`
(`hunt/rest.rs`, split from the engine). `;hunt import-loot <eloot yaml>` writes the
character's profile; the desk reads it when a hunt starts and says so either way. Tested
over a scripted game in `tests/hunt_loot_drive.rs`. **Not yet:** the *at your feet* line
after a search, and confirming a stow by the bag's contents (the model has no container
contents; a stow is confirmed by the thing leaving the floor). Stage 3 is next. The author's answers to §7 are recorded
there and folded into §4 and §6. Claude's staging of M6c (`plan/30` §7: *"M6c — eloot. Its
port plan first (`plan/31`), then the halves a hunt calls: loot, sort, box in hand. Town
errands follow in the same plan's order."*). The author's rule that makes eloot part of M6 is
`plan/30`'s: *"We need to make sure we have eloot and eherbs too, not just eohunter."* A hunt
that cannot loot fills its hands and pack and rests on encumbrance.

This document orders the port the way `plan/24` ordered go2: what exists, what the script
does when a hunt calls it, which halves are the hunt's and which are town errands, and the
stages, each ending in something that runs and is tested.

## 0. Measured

`eloot.lic` at `C:\Gemstone\lich-5\scripts` (the author's live install, 2026-09-13) is
**8,029 lines**; the clone at `reference/scripts/scripts/eloot.lic` (`7fd0b97`) is **8,087**.
Line numbers below are the live install's. Its modules, by `grep -nE '^module ELoot'`:

| Module | Lines | Hunt calls it | Notes |
|---|---|---|---|
| DebugLogger | 110 | no | a log file |
| Data | 350 | as tables | the reject lists, the outcome patterns, the category names |
| UI Setup | 1,213 | no | a GTK settings window |
| Profile loading/saving | 274 | as a format | `eloot.yaml` per character |
| Sets Inventory | 279 | yes | finds the containers, the coin hand, the disk, the charm |
| Main methods | 36 | **yes** | `ELoot.loot`: the whole hunt-side cycle, 20 lines |
| Regional bounty selling | 65 | no | town |
| Script utility | 379 | as primitives | `get_command` = send and collect lines until a pattern, retrying on roundtime |
| Game utility | 649 | partly | stance, disk, coin hand, phase; also silver and notes (town) |
| Inventory | 533 | **yes** | free a hand, drag an item into a bag, put a bag's item away, return hands |
| Gem and Reagent hoarding | 840 | no | lockers and caches: town |
| Room looting | 880 | **yes** | search corpses, loot the room, boxes, bags, skins |
| Sells the loot | 1,943 | no | 47 methods: gemshop, pawnshop, furrier, locksmith pool, alchemist, bank |

**The hunt's share is ~1,750 lines of the 8,029**: Main, Inventory, Room looting, and the
parts of Sets Inventory and Game utility they lean on. The other ~6,300 are a settings window
and the town. That is the split this plan builds on.

## 1. What exists in Hydra

| piece | where | state |
|---|---|---|
| the game's own sorter, read | `crates/cena-model/src/state/containers.rs`: `Containers::stow(StowSlot)`, `ready(ReadySlot)`, `stow_checked()` | built (M3): `stow list` and `ready list` and the confirmations that follow `stow set` |
| what is in each hand, by id | `crates/cena-model/src/state/hands.rs` | built; `holds(id)`, `is_empty()` |
| what is on the floor, typed | `crates/cena-model/src/state/room.rs` (`RoomItem`), `crates/cena-model/src/state/gameobj.rs` (`classify(noun, name) -> ObjectTypes`, `is(type)`, `sells_to(shop)`) | built: the port of Lich's `gameobj-data.xml` typing, which is what eloot's `thing.type` reads |
| corpses | `CreatureInstance::corpse()` | built (`d296712`..`02037df`): by flag or by hit points |
| the stand-in | `cena-behavior/src/hunt/engine.rs`, the Loot arm | `loot #id` once per corpse, spaced 15 s while targets stand when `loot.delay` |
| the profile's loot table | `cena-behavior/src/hunt/profile.rs`, `Loot { script, delay, defensive }` | built; `script = "eloot"` is carried and does nothing yet |
| the reading half of stash | `plan/20` §0b | built; the sending half (12 functions, ~460 lines) deferred to here |
| send, then read the reply | `SessionHandle::send_and_await` with a matcher; the write-time `Gate::Act` | built (M6a); the driver already sends the hunt's verbs through it |
| a behavior inside a behavior | `travel_holding`, run by the hunt driver under its own token | built (M6b): the shape Loot takes too |

So the room, the hands, the containers and the object types are already model facts. What
does not exist is anything that *sends*: no drag, no `loot room`, no reading of the game's
answer to either.

## 2. What eloot does when bigshot calls it

bigshot (`bigshot.lic:7855-7887`, `loot()`) runs the loot script once per dead npc and once
more when only floor loot remains: `run_script(@LOOT_SCRIPT, true, looting: true)`, waiting
for it to finish. eloot's whole hunt-side cycle is `ELoot.loot` (`eloot.lic:2483-2506`):

```
stance defensive            if loot_defensive
Loot.skin                   if skin_enable          (Nisugi: off)
Loot.search                 every dead npc
Loot.room                   everything on the floor worth taking
use the coin hand           if one is set           (Nisugi: none)
Inventory.return_hands      what was held before is held again
```

### 2a. `Loot.search` (`:5645-5711`)

For each corpse not excluded by name: `loot #id`, up to three times, reading the reply for
`You search|plunge|break|tentatively`, `not in any condition`, and the *at your feet* line
that names an item the search dropped (`regex_at_feet`), which is then dragged into a bag.
On `not in any condition` with `sigil_determination_on_fail` (Nisugi: on) it casts Sigil of
Determination and tries again. Plant-like and in-hand critters (`tumbleweed`, `vine`,
`golem`, `elemental`, …) need a free hand first and the hand checked after.

### 2b. `Loot.room` (`:5632-5643`) and `loot_regular` (`:5222-5292`)

1. `GameObj.loot` minus **`reject_invalid_loot`** (`:5582-5596`): the fixture names and
   nouns (Data `:633-680`: ranger vines, deity effects, doors, mist, kittens…), anything
   with a negative id, weapons and armor that are not `uncommon` or `clothing`, and what the
   character has learned is unlootable or crumbly.
2. **`loot_specials`** (`:5509-5570`): bags dropped by critters, boxes, three named uncommon
   Hinterwilds items `loot room` does not take.
3. Split the rest by **`should_grab_item?`** (`:5600-5609`): excluded by name → no; cursed
   only if `cursed` is wanted; wanted if its type is in `loot_types`; unwanted if its type is
   in a category not chosen; otherwise yes.
4. **If everything left is wanted: `loot room`** (`loot_all`, `:5161-5200`) — one verb, and
   the game sorts by its own STOW LIST. eloot reads the reply for `There is no loot`, a
   closed container (then learns the bag is an autocloser and opens it), and the *too much to
   carry* line, after which it drags what landed in the hands and loots the room again.
5. **If wanted and unwanted are mixed: item by item.** A stow-list type (`clothing|jewelry|
   gem|herb|skin|wand|scroll|potion|reagent|trinket|lockpick|treasure|forageable|…`) goes by
   `loot #id` (`single_loot`, `:4012-4054`), which the game puts straight into the right
   container; anything else by `_drag #item #bag` (`single_drag` → `store_item`,
   `:3902-3960`, `:4056-4118`), the bag chosen as the stow list's slot for its type, else the
   default, else the overflow containers in order.
6. The reply to a drag is read for: `won't fit` (**the bag is full**: remembered for the
   session in `sacks_full`, and the next bag is tried), `crumbles and decays` (learned as
   crumbly), `It's closed!` (autocloser: open it and retry), `That is not yours`, `Hey, that
   belongs to`, `put something that you can't hold` (unlootable). When every bag is full it
   pauses for the player; the hunt equivalent is a Rest reason.

### 2c. What Nisugi has set (`C:\Gemstone\lich-5\data\GSIV\Nisugi\eloot.yaml`)

| Setting | Value | Reaches the hunt as |
|---|---|---|
| `loot_types` | 18 of the 21 categories; not `cursed`, `herb`, `junk`, `weapon` | the *take* list |
| `loot_exclude` | `black ora`, `urglaes` | the *leave* list, by name |
| `loot_defensive` | true | `Loot.defensive`, already in the profile |
| `use_disk` | true | when a bag is full, the disk is a container too (`wait_for_disk`, `disk_usage`) |
| `sigil_determination_on_fail` | true | recast on `not in any condition` |
| `charm_name` | `fossil charm` | a worn charm found at set-up (`:2377`); its use is in Sell |
| `loot_phase` | false | 704 on boxes: off |
| `overflow_containers` | none | the stow list's default is the only fallback |
| `skin_enable` | unset (false) | skinning: off |
| everything `sell_*`, `locksmith_*`, hoarding | various | town; see §5 |

## 3. The shape

The same two layers as travel and the hunt (`plan/30` §3): a **pure planner** and a **thin
driver**, and the driver runs *inside the hunt's authority* the way a walk does
(`travel_holding`), so the hunt keeps folding its own stream and the Loot arm's `Said::Send
{ loot #id }` becomes `Said::Loot(corpses)`.

**The planner** takes what the model already knows (the room's items typed, the hands, the
stow list, the corpses) plus the loot profile and the session's memory (bags found full, bags
learned to be autoclosers, names learned crumbly), and answers one question at a time: *the
next command*, as `loot #corpse`, `loot room`, `loot #item`, `drag #item #bag`, `open #bag`,
`stance defensive`, or *done*. It is fed each command's **outcome**, read by a classifier
over the reply lines into a small closed set: `Searched`, `NothingHere`, `Stored`,
`WontFit(bag)`, `Closed(bag)`, `Crumbled`, `NotYours`, `Unlootable`, `TooMuch`,
`AtYourFeet(item)`, `NotInCondition`. The whole of eloot's regex vocabulary above is that
enum's `classify`, over `ChunkLine`s rather than raw XML, as `containers.rs` already did for
`stowlist.rb`.

Three-valued where the model is: a room whose items have not been stated, or a stow list never
taught, is `None`, and the planner holds rather than guesses (`plan/30` §3's rule).

## 4. Stages

### Stage 1 — the loot planner, pure

- `cena-behavior/src/loot/profile.rs`: the loot profile (§6) and its import from `eloot.yaml`
  (the bigshot importer's shape: flat YAML, no crate; the town keys carried into notes, not
  lost).
- `cena-behavior/src/loot/worth.rs`: `reject_invalid_loot` and `should_grab_item?` as one
  function over `RoomItem` + `ObjectTypes`, tabled from Data `:633-690`. Tested by table.
- `cena-behavior/src/loot/outcome.rs`: the outcome classifier over reply lines, from the
  `put_regex`/`look_regex`/search patterns (`:541-580`, `:5647`). Tested against the lines
  the three replay fixtures already hold (`You search the …`, `It didn't carry any silver`,
  `had nothing of interest`) and hand-cut lines for the rest.
- `cena-behavior/src/loot/plan.rs`: the planner, `next(&GameState, &Memory) -> Step`, fed
  `outcome(Step, Outcome)`. The order is §2's. Tested tick by tick as the hunt engine is.

### Stage 2 — the driver, and the hunt's Loot arm calls it

- `cena-behavior/src/loot/drive.rs`: `loot_holding(handle, token, joined, plan, …)`, settling
  roundtime, sending through `Gate::Act`, matching each reply with the outcome classifier,
  returning what it learned (`Memory`: full bags, autoclosers, crumbly names) for the hunt
  to keep across corpses and rooms.
- The hunt engine's Loot arm says `Said::Loot` and the hunt driver runs it as it runs a walk;
  `loot.script` unset keeps today's `loot #id`.
- The disk as a container (`use_disk`: `wait_for_disk`, `disk_usage`, and the disk's own
  full state), here rather than in Stage 3 (author, Q2).
- The session memory of full bags becomes a **Rest reason** (`Why::Full`, *"too much
  loot"*) when every bag and the disk are full, in place of eloot's pause; a box left in
  hand that no bag will take is the same reason (author, Q3: *"we don't want to drop it, so
  we head in to rest"*).
- Replay fixture: a cut with a search whose reply names an item at your feet, and one with
  `loot room` taking everything. Both exist in Nisugi's 21 September log (116 searches); cut
  when the stage is reached and named by line.

### Stage 3 — the specials the Hinterwilds need

- Boxes: taken as loot (`box` is in Nisugi's types) and stowed to the box slot; `box_in_hand`
  (bigshot `:2013`: *"force resting mode if box is left in your hand after looting"*) is a
  Rest reason when a box stays in hand because the box bag is full.
- Bags dropped by critters (`bag_loot`, `:4980-5025`) and the three uncommon items.
- Sigil of Determination on `not in any condition`.
- Skinning stays out until a profile turns it on (Nisugi's does not).

### Stage 4 — selling, during the rest (author, Q1)

*"Sells typically happen during the rest, one reason that triggers the rest is too much
loot."* So Sell is not a separate errand but a step of the Rest phase: on arriving at the
resting room with loot to sell, the trip to each shop and back is a walk inside the rest,
before the rest commands. The author brought it forward of M6d on 2026-09-24 (*"finish off
eloot"*), so it is staged here, measured against eloot's Sell (1,943 lines, `:5904-7847`).

**The shape.** The same two layers again. A pure **errand planner** (`cena-behavior/src/
town/`) reads the state -- the sellable bags' contents by the stow list, the hands, the room
-- and the profile's `[town]` table, typed, and answers *the next step*: walk to the nearest
room tagged for a shop (travel's own `Target::Nearest` over the map's tags, as `;go2
gemshop` already resolves), fetch an item to a hand, `sell`, `appraise`, `analyze`, bulk-sell
a sack, read a note, `deposit`, `give` to a clerk, and back to the resting room. The driver
runs it inside the hunt's authority as it runs a loot, walking with travel's driver as the
hunt walks. **The outcomes are the ledger's facts**: the driver's own fold of the stream
queues each prompt's `LootFact`s (`state.take_loot()` on its private `GameState`), so a
sale's silver, an appraisal's figure, a refusal and a note are read by `plan/34`'s one
classifier, never a second time here; the few replies that are not loot facts (*not quite
my field*, *already holding as many boxes*, *You hand your*) are a small text classifier of
their own.

- ~~**4a. The frame, the pawnshop and the gemshop.**~~ **BUILT 2026-09-24**:
  `cena-behavior/src/town/` (`settings.rs` types the `[town]` table, `plan.rs` is the
  `Seller`, `reply.rs` the few non-fact replies), `Said::Sell` and `Phase::Selling` in the
  engine (`hunt/rest.rs`, `wants_to_sell`), the round in `hunt/drive.rs` (`sell`), reading
  each prompt's `LootFact`s from the driver's own fold. Tested in `tests/town_plan.rs` (6)
  and `hunt_engine.rs` (the arrival). **Its leftovers BUILT 2026-09-25**: `return_hands`
  (what the hands held when the round began, a box aside, is fetched back before home, so a
  weapon stowed to free a hand is in hand for the hunt); the jeweler's *not my field* sold
  at the pawnshop instead (`retry_wrong_shop_jewelry_at_pawnshop`), and, when
  `sell_pawn_recheck` is on, what it found too valuable appraised there and put back
  (`recheck_refused_at_pawnshop`), the pawnshop joining the round for either;
  `sell_keep_scrolls` (a scroll read first and kept when a line names a kept spell, `215`
  plain or `215v` vibrant). **Corrected:** the pawnshop analyzes *every* item before
  selling it, as eloot's `pawnshop` does (`eloot.lic:7539-7547`), so ALTER 41 is caught on
  anything; 4a analyzed only what could be a transmog. The Stage 4 tests are split by stage:
  `tests/town_plan.rs`, `town_shops.rs`, `town_pool.rs`, sharing `tests/town_support/`. As staged: `Phase::Selling` between the walk to
  the resting room and the rest commands; `Said::Sell` when a sellable bag holds something;
  `check_items`' shop list from the bags' typed contents (`gameobj`'s `sellable`); the
  pawnshop item by item (appraise first for the profile's appraise types and for
  uncommon/weapon/armor, against `sell_appraise_pawnshop`; over the limit or *too valuable*
  goes to the appraisal container or back to its bag; `analyze` before selling what could be
  a transmog when `keep_transmogs`); the gemshop as one bulk `sell #sack` for a gem sack with
  no excluded gem, the note read, the sack worn again, then the leftovers item by item;
  `sell_exclude`, `bound`, ready-list items skipped. Back to the resting room. **The silver
  is the ledger's**: no `wealth` before and after.
- ~~**4b. Furrier, collectibles, the Chronomage and the bank.**~~ **BUILT 2026-09-25**:
  `town/goods.rs` holds `check_items`' reading (which shop takes an item: a gold ring by
  eloot's 22 names to the Chronomage, a collectible to its counter by either tag, then the
  object data's `sellable`) and each shop's lots; `town/plan.rs` gains `Deposit`, `Give`,
  `Unbundle`, `DepositAll` and `Withdraw`. The furrier's bag sells whole like the gem sack
  (a bag is sold whole at most once a round, which also closes 4a's loop when a bulk sale
  leaves something behind); a bundle comes apart with `bundle remove` and each skin sells,
  a refused skin back to the bag. The bank comes last when the round sold anything or read
  a note, and first whenever encumbrance is over 80% on the way to a shop; a note waiting
  in the default bag is a round by itself. `deposit all`, then `withdraw <sell_keep_silver>
  silver`. Tested in `tests/town_shops.rs`. **Differs from eloot, deliberately:**
  eloot goes to the bank whenever `wealth` differs from the keep figure; Hydra's silver
  figure is only as fresh as the last `wealth`, so a stale one would send every rest to the
  bank, and the round banks on what it earned instead. **Not built:** the furrier's
  skin-bounty check that keeps bundles whole, Pinefar's banker, the coin hand.
  As staged: Bulk `sell #sack` at the
  furrier and bundles unbundled one skin at a time; `deposit #id` at the collectibles
  counter; gold rings given to the Chronomage's clerk (`sell_gold_rings`); `deposit all`
  less `sell_keep_silver` at the bank, notes deposited, and a deposit whenever encumbrance
  passes 80% on the round (`plan/20` §0b's bank sending half, its reply patterns from
  `bank.rb:22-66`).
- ~~**4c. Boxes: the locksmith pool.**~~ **BUILT 2026-09-25**: `town/pool.rs` is the
  pool as the round's first stop (`Shop::Pool`, tagged `locksmith pool`): every box in a
  hand, the selling bags and the disk (`use_disk`) fetched, swapped to the right hand,
  `give #worker <tip>[ PERCENT]`, the quote confirmed with the same give and ` confirm`;
  *already holding as many boxes* or *You don't have that much* stops the drop-off and the
  box goes back to its bag; *already open* empties it on the spot. Then `ask #worker for
  return` until nothing is ready, each returned box emptied by the loot planner's new box
  mode (`loot/plan/boxed.rs`, `Planner::for_box`: `open`, `look in`, `point <charm> at
  #box` or `get coins from #box`, then each wanted thing by the planner's own `take`), then
  kept when it is an empty gold, mithril or silver box the profile sells, else `trash`,
  else `drop`; a locked box goes back to its bag. The worker is found by eloot's eight
  names; the drop, the quote and the return are the ledger's facts. Tested in
  `tests/town_pool.rs` (3) and `tests/loot_plan.rs` (3). **Not built:** the room's
  `meta:boxpool:npc` tag (the round has no map tags, only room ids), incremental tipping
  (`use_incremental_tipping`, off for Nisugi), a full pool emptied by returns and filled
  again in the same visit, *lighten your load* answered with a bank trip, the town
  locksmith, and a box's cursed contents kept out of a saved box.
  As staged: A box in hand before anything else; every box in the
  box bag and the disk to the pool with the standard tip (`sell_locksmith_pool_tip`, or the
  incremental ladder), the worker found by the room's `meta:boxpool:npc` tag or eloot's
  names; the pool's returns asked for and each returned box opened, looked in, its coins
  gathered (the charm first when the profile names one) and its contents taken by the loot
  planner's own `take`. The town locksmith (`sell_locksmith`, off for Nisugi) and the
  `case` boxes it cannot open are recorded, not built.
- **4d. Not ported now**, each named so it is not forgotten: **Hoard** (840 lines; off for
  Nisugi), **Region** (65; regional bounty selling), consignment and alchemy mode, curse
  removal (315), `break_rocks` for breakables,
  `dump_herbs_junk`, FWI routing, the gem-bounty and furrier-bounty checks, the shroud and
  aspect sell buffs, `sell_share_silvers`, and the free-to-play bank ladder.

## 5. Out of the hunt's share, tabled

| eloot piece | Lines | What it is | When |
|---|---|---|---|
| `Sell` | 1,943 | sell by shop, appraise thresholds (Nisugi: gemshop 14,999, pawnshop 34,999), keep transmogs, gold rings, collectibles, locksmith pool, tips | Stage 4 |
| `Hoard` | 840 | gem and alchemy hoarding in lockers and caches | Stage 4 |
| `Region` | 65 | regional bounty selling | Stage 4 |
| silver deposit, notes, coin hand | ~120 | bank | Stage 4, with `plan/20` §0b bank |
| skinning (`skin`, `skin_obj_types`) | ~130 | skin corpses by weapon | **BUILT 2026-09-24**: `cena-behavior/src/loot/skin.rs`, the planner's first phase when `skin.enable` is on; the profile's `[skin]` table carries eloot's five switches, four names and two lists; the driver sends `get`, `kneel`, `skin #id <hand>`, `stow gem`, `stand`. **Gaps closed 2026-09-25:** `skin_bounty_only` skins only the creature a skinning bounty names (the model's bounty status, `TaskKind::Skin`), and nothing without one; a `rotting chimera` learned unskinnable is described and skinned when the scorpion-tailed form shows (`occassional_skinner`); a creature learned unskinnable is written into the loot profile file by the hunt's desk (`loot::remember_unskinnable`), as eloot saves its profile, and said |
| the GTK window | 1,213 | settings UI | never: the profile is a file, and `;hunt check` reads it |
| `DebugLogger` | 110 | | never |

## 6. The loot profile

Per character, like eloot's (author, Q4: *"the eloot settings profile is per character
yes"*): `<data>/hunt/loot/<instance>_<character>.toml`, imported from `eloot.yaml` by
`;hunt import-loot <path>`. There is no choice of looter (author: *"eloot is it by
default"*), so the hunt profile's `loot.script` key is carried from bigshot for the record
and decides nothing: a hunt loots with this profile when the character has one, and with
`loot #id` alone when not. Shape, the keys carried named for what they do:

```toml
take = ["alchemy", "armor", "box", "breakable", "clothing", "collectible", "food", "gem",
        "jewelry", "lockpick", "lm trap", "magic", "reagent", "scroll", "skin", "uncommon",
        "valuable", "wand"]
leave = ["black ora", "urglaes"]
defensive = true
disk = true
sigil_on_fail = true
phase_boxes = false
overflow = []
```

The `sell_*`, `locksmith_*` and hoarding keys import as a `[town]` table carried verbatim
for Stage 4, and `;hunt check` says so.

## 7. Questions for the author — answered 2026-09-24

1. **When does a hunt sell?** *"Sells typically happen during the rest, one reason that
   triggers the rest as you mention is too much loot."* Stage 4 is a step of the rest.
2. **The disk in M6c.** *"Yes"*: Stage 2.
3. **`box_in_hand` as a Rest reason.** *"Yes, if eloot leaves you with a box in hand it can't
   do anything with, we don't want to drop it, so we head in to rest."*
4. **The loot profile per character.** *"There's only one option for a looting script here so
   eloot is it by default. I mean we don't have scripts ... but the eloot settings profile is
   per character yes."* One looter, its profile per character.
