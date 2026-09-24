# 31 — The Loot behavior: eloot, measured and ordered

**Status: PROPOSED 2026-09-24.** Claude's staging of M6c (`plan/30` §7: *"M6c — eloot. Its
port plan first (`plan/31`), then the halves a hunt calls: loot, sort, box in hand. Town
errands follow in the same plan's order."*). The author's rule that makes eloot part of M6 is
`plan/30`'s: *"We need to make sure we have eloot and eherbs too, not just eohunter."* A hunt
that cannot loot fills its hands and pack and rests on encumbrance.

This document orders the port the way `plan/24` ordered go2: what exists, what the script
does when a hunt calls it, which halves are the hunt's and which are town errands, and the
stages, each ending in something that runs and is tested. Nothing here is built.

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
- The session memory of full bags becomes a **Rest reason** (`Why::Full`) when every bag is
  full and there is no disk, in place of eloot's pause.
- Replay fixture: a cut with a search whose reply names an item at your feet, and one with
  `loot room` taking everything. Both exist in Nisugi's 21 September log (116 searches); cut
  when the stage is reached and named by line.

### Stage 3 — the specials the Hinterwilds need

- Boxes: taken as loot (`box` is in Nisugi's types) and stowed to the box slot; `box_in_hand`
  (bigshot `:2013`: *"force resting mode if box is left in your hand after looting"*) is a
  Rest reason when a box stays in hand because the box bag is full.
- Bags dropped by critters (`bag_loot`, `:4980-5025`) and the three uncommon items.
- The disk as a container (`use_disk`): `wait_for_disk`, and the disk's own full state.
- Sigil of Determination on `not in any condition`.
- Skinning stays out until a profile turns it on (Nisugi's does not).

### Stage 4 — town, in eloot's order (`plan/30`: *"Town errands follow"*)

Sell (1,943 lines), Hoard (840), Region (65), silver and notes: each a trip with `travel`
first and verbs at the counter. They come after M6d, because a hunt that cannot heal cannot
stop resting either, and they bring `plan/20` §0b's sending halves of stash and bank with
them. Not staged further here; §5 tables what they are so they are not forgotten.

## 5. Out of the hunt's share, tabled

| eloot piece | Lines | What it is | When |
|---|---|---|---|
| `Sell` | 1,943 | sell by shop, appraise thresholds (Nisugi: gemshop 14,999, pawnshop 34,999), keep transmogs, gold rings, collectibles, locksmith pool, tips | Stage 4 |
| `Hoard` | 840 | gem and alchemy hoarding in lockers and caches | Stage 4 |
| `Region` | 65 | regional bounty selling | Stage 4 |
| silver deposit, notes, coin hand | ~120 | bank | Stage 4, with `plan/20` §0b bank |
| skinning (`skin`, `skin_obj_types`) | ~130 | skin corpses by weapon | when a profile turns it on |
| the GTK window | 1,213 | settings UI | never: the profile is a file, and `;hunt check` reads it |
| `DebugLogger` | 110 | | never |

## 6. The loot profile

Per character, like eloot's: `<data>/hunt/loot/<instance>_<character>.toml`, imported from
`eloot.yaml` by `;hunt import-loot <path>`, with the hunt profile's `loot.script = "eloot"`
meaning *use it*. Proposed shape, the keys carried named for what they do:

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

## 7. Questions for the author

1. **When does a hunt sell?** bigshot never runs `;eloot sell`; the author does, by hand. Does
   Hydra's hunt need a trigger (bags full, N kills, before resting) or is selling a command
   the player types, as today?
2. **The disk in M6c.** Nisugi's profile uses it. Stage 3 as written; is it wanted in Stage 2
   instead, since a full bag with no disk means resting?
3. **`box_in_hand` as a Rest reason** is my reading of bigshot's tooltip. Right?
4. **The loot profile per character, not per hunt profile.** eloot's is per character; the
   hunt profile's `loot.script` only turns it on. Keep that split?
