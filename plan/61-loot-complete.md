# 61. Loot, complete: every command and feature of eloot

**Status: APPROVED 2026-09-30** (the author, the questions of §7 answered: *"You can go
ahead and work on the loot plan"*). **Steps 1, 2 and 3 BUILT the same day** on branch
`loot-complete` (§5a, §5b, §5c); **step 4 BUILT 2026-10-01** in three parts (§5d, §5e,
§5f), and **step 5** the same day in three more (§5g, §5h, §5i). The author: *"let's plan out the rest of it.
Hydra's loot should have all the features and commands of eloot."*

`plan/31` ported eloot's share of the hunt: looting, skinning, and a selling round at the
rest. This plan is the rest of the script. It was written from two surveys made the same
day: eloot read whole, and Hydra's port measured against its code.

- **eloot**: `reference/scripts/scripts/eloot.lic`, **8,111 lines, version 2.11.12**
  (`eloot.lic:17`), the clone at `df11fdd` of 2026-09-25. `plan/31` was written against an
  older, shorter file (it gives `Sell` as 1,943 lines; it is 1,964 now, `eloot.lic:5918-7881`,
  and `Hoard` 843, `:4194-5036`), so its line citations have drifted; this plan cites the
  current file.
- **Hydra**: `crates/cena-behavior/src/loot/` and `town/`, 4,294 lines together, the hunt's
  drivers `hunt/drive/loot.rs` and `drive/selling.rs`, and 71 tests in eight planner files
  plus three scripted-game tests of looting. **No test runs the selling driver against a
  scripted game**; none of it has run live.

## 1. The gap in one paragraph

Everything Hydra's loot does, it does **inside a hunt**: `Said::Loot` after a kill
(`hunt/engine.rs:573`) and `Said::Sell` on reaching the resting room (`hunt/rest.rs:57`).
No command loots a room, sells, visits the pool or deposits by itself; `;loot` is the
ledger's reports (`crates/cena/src/loot.rs`), and the agent's operations do not include
any of it (`crates/cena/src/perform.rs:39-41`). eloot is the opposite: seventeen commands
(§2), of which a hunt uses two. So a player standing in town with full bags cannot sell
them, and a player not hunting cannot loot. Beyond the commands, the round is missing the
pieces `plan/31` §4b-4d recorded, three profile keys are carried and never read, and some
thirty `[town]` keys are imported and never read (§3).

## 2. eloot's commands, and Hydra's

eloot's dispatch is `eloot.lic:7883-8111`; its help table `:2705-2762`.

| eloot | Does | Hydra today |
|---|---|---|
| `;eloot` | loot: skin, search the dead, loot the room, coin hand, hands back (`:2501-2523`) | only inside a hunt |
| `;eloot box [id]` | empty the open box in hand, or one on the ground by id (`:7977`) | the pool's returns only (`loot/plan/boxed.rs`) |
| `;eloot ground` | empty every open box on the ground (`:5144-5219`) | no |
| `;eloot skin` | skin the dead (`:5885`) | only as the loot's first phase |
| `;eloot sell` | the whole round (`:7812-7847`), then the breakdown | only at a hunt's rest |
| `;eloot sell alchemy_mode` | the round, keeping `Vars.needed_reagents` and dumping alchemy leftovers | no |
| `;eloot pool` \| `pool deposit` \| `pool return` (`check`, `loot`) | the locksmith pool: both, drop off only, collect only (`:7612-7652`) | inside the round only, both together |
| `;eloot deposit` | bank: coin hand drained, shared, deposited, the keep withdrawn (`:3395-3516`) | inside the round only |
| `;eloot --sellable <shops>` | sell what those shops take (`:6700`) | no |
| `;eloot --type <types>` | sell those types (`:6744`) | no |
| `;eloot --sell <names>` | sell those items (`:6653`) | no |
| `;eloot list` \| `deposit` \| `reset` `gem\|reagent\|alchemy` | the hoard: list it, deposit into it, rebuild its inventory (`:8071`) | no hoard |
| `;eloot raid <type> <item> x<n>` | take from the hoard (`:4992`) | no |
| `;eloot bounty` | take the gem bounty's gems from the hoard (`:4857`) | no |
| `;eloot settings <key> <value>`, `settings help`, `options`, `setup`, `list`, `load` | the settings from the line, their names, the window, the listing, a reload | the *Loot*, *Skinning* and *Selling* pages; **no `loot set`** (`hunt/command.rs:148-153` has `Heal` and `Waggle` only) |
| `;eloot reset unskinnable [creature]` | forget what was learned unskinnable (`:2169`) | hand edit |
| `;eloot test`, `debug`, `debug file`, `start`, `ver` | troubleshooting | `hunt check` says the loot file; the rest have no counterpart and need none (§6) |

## 3. Features: built, missing, and carried but dead

**Built** (`plan/31` §4, confirmed in the code): the two-pass loot with a critter's bag
emptied, Sigil of Determination, stows confirmed by contents, the disk, full bags and a
box in hand as reasons to rest; skinning with its eleven keys; the pawnshop (appraise
limits, ALTER 41 and transmogs, scrolls kept, the gemshop's refusals rechecked), the gem
shop (bulk, then item by item), the furrier (bulk, bundles apart), collectibles, gold
rings, the bank (`deposit all`, the keep, early over 80% encumbrance), the pool (tip,
confirm, returns emptied, the box kept, trashed or dropped), the hands restored.

**Missing from the loot** (eloot §5 of the survey):

| Feature | eloot | Note |
|---|---|---|
| `loot_keep`: names taken whatever their kind | `:5595`, `:5647` | the importer drops it with a note (`loot/import.rs:141-156`) |
| `critter_exclude`: corpses never searched or skinned | `:5714`, `:5891` | dropped with a note |
| overflow containers | `:3958-4033` | `overflow` is in the profile and **nothing reads it** (`loot/plan.rs:441-458`) |
| phasing boxes (704) and unphasing at the pool | `:2975-2996` | `phase_boxes` carried, never read |
| the profile's `autoclose` list | `:3711`, every 30 s | carried, never read; learned only at run time |
| the group's disks for boxes (`use_disk_group`) | `:4035-4066` | dropped silently by the importer |
| the coin hand, coin bag and gambling kit | `:3582-3635`, `:2286` | no model of it |
| cursed items: 315, or the eonake gauntlet | `:3035-3087` | `plan/31` §4d |
| blood bands raised at a corpse | `:5720`, `:2263` | dropped with a note |
| creatures that hand an item over (bramble, skayl, glacei, caedera, golem, elemental): a hand freed first, what lands in it stowed | `:5724-5765` | to check against `loot/plan/hands.rs` |
| plinite plucked from a box | `:5138-5140` | no |
| a gold ingot that fits nowhere sold at the gemshop on the spot | `:7188-7203` | no |
| the bounty heirloom always taken; oblivion quartz at level 100 | `:5648`, `:5674`, `:5664` | to check against `loot/worth.rs` |
| `keep_closed`: bags shut again after | `:3737`, `:7846` | dropped as bookkeeping |
| `track_full_sacks`, `log_unlootables`, `favor_left` | `:7911`, `:2954`, `:3826` | dropped; decide each (§7) |

**Missing from the selling round**:

| Feature | eloot | Note |
|---|---|---|
| the town locksmith, with `locksmith_priority`, `locksmith_when_gem_bounty`, `locksmith_withdraw_amount`, `case` boxes the town cannot open, refusals remembered | `:7221-7332`, `:7661-7771` | carried in `[town]`, never read |
| incremental tipping (`base_tip`, `max_tip`, `alpha_rate`), the pool counted | `:7480-7514` | carried, never read |
| a full pool emptied by returns and filled again; `always_check_pool` | `:7420-7478`, `:7696` | no |
| *lighten your load* answered with a bank trip | `:7445` | no |
| full bags during a return: sell the box's contents, or park the box and sell | `:5384-5567` | no |
| the worker from the room's `meta:boxpool:npc` tag | `:3201-3230` | the seller sees only `nearest(tag)` (`town/plan.rs:193`) |
| a saved box's cursed contents kept out of it | `:7773-7810` | no |
| the furrier's and the gem shop's bounty checks: the bounty's skins and gems sold in the bounty's town when it is in the region, bundles kept whole | `:6319-6489`, `Region` `:2530-2593` | the model has both tasks (`cena-model/src/state/bounty.rs:56-102`); no region |
| consignment (the alchemist), and `alchemy_mode` | `:6614-6651`, `:6516` | no `Buyer` for it in the ledger (`state/ledger.rs:220-226`) |
| `break_rocks` | `:6302-6317` | no |
| `dump_herbs_junk`, `trash_dump_types` | `:6815-6851` | no |
| sell buffs: Glamour and Shroud by the town's race, Assume Aspect | `:7032-7096`, `:7849-7860` | no |
| `sell_share_silvers` | `:3446` | no |
| `sell_deposit_coinhand`, the coin item drained at the bank | `:3398-3440` | no |
| Pinefar's banker | `:3463`, `:3570` | no |
| the free-to-play bank ladder, and no pool for free-to-play | `:3470-3504`, `:7335` | no |
| FWI: `sell_fwi`, the trinket home, notes carried back, no Chronomage there | `:3274-3293`, `:3304-3309` | no |
| the Hinterwilds, which lacks four shops | `:3243-3272` | no |
| `between`: scripts run after the boxes | `:7205-7219` | Hydra's answer is a list of Hydra commands |
| the breakdown: silver by shop, tips and fees, pool counts, what was over the limit | `:6216-6300`, `:6174` | the ledger has the facts; nothing prints them after a round |
| scarabs, and thorns and berries, to the gem shop; clothing tried at both | `:6512-6535` | to check against `town/goods.rs` |

**The hoard** (`eloot.lic:4194-5036`, 843 lines, 34 settings): gems and reagents kept in
jars in a locker (a public one by town, a house one by its rooms and verbs) or a worn
stash; the inventory built from the jars, or from `locker manifest` on a premium account;
everything-but or only-these lists; a gem bounty's gems held back and shaken out; a
locker's debt paid; deposits in the breakdown. **Nothing of it exists in Hydra**, and the
model has no locker. `plan/59` §5a already makes lockers a plan of their own for the
inventory manager; the hoard's reading half (what is in which jar) belongs with that, and
its sending half here.

## 4. The shape

- **One behavior, started two ways.** The planners are pure and already separate from the
  hunt (`loot::Planner`, `town::Seller`); what is the hunt's is only who asks. A standalone
  command runs the same planner as a one-shot errand under the hunt desk's authority, as
  `heal` and `heal stock` do today (`hunt/errands.rs:31-35`, `hunt/desk.rs:191-204`,
  `hunt/drive/errands.rs:69-156`): claimed, named, stoppable by `stop`, ended with its own
  `Ending`, refused while a hunt holds the session. The selling driver already starts from
  wherever the character stands (`hunt/drive/selling.rs:16`).
- **Its own word, and the reports keep theirs** (§7 item 1). Proposed: `loot` is the
  family. `loot` alone loots the room, as `;eloot` does; `loot sell`, `loot pool`, `loot
  deposit`, `loot box`, `loot ground`, `loot skin` are eloot's; the reports stay `loot
  summary`, `recent`, `boxes`, `creatures`, `cap`. Today bare `loot` is the summary
  (`crates/cena/src/loot.rs:91`), which moves to `loot summary` alone.
- **Settings by the line**: `loot show`, `loot set <setting> <value>`, `loot unset
  <setting>`, through the writer `heal set` uses (`cena-behavior/src/settings.rs`), over
  the three tables the pages already show. eloot's `settings`, `options`, `list`, `load`
  and `setup` are these and the pages.
- **A setting is read or it is not there.** Every key the importer carries and nothing
  reads is either built in a step below or dropped from the tables with a note at import,
  so the *Selling* page never shows a switch that does nothing.
- **Routing is the map's.** FWI, the Hinterwilds, Pinefar, the bounty's region and the
  pool's worker are facts about rooms. The seller is given what it needs of them (a room's
  town and tags, whether a route crosses a region's edge) rather than only `nearest(tag)`;
  eloot's hard-coded boundary rooms (`:2562-2577`) become data beside travel's.
- **The breakdown is the ledger's.** A round's facts are already recorded
  (`LootFact::Sold`, `PoolQuoted`, `Deposited`, ...); the breakdown is a report over one
  round's, said at its end and kept as `loot last`.

## 5. Steps

Ordered so that each leaves something usable, the author's own profile first (the town
locksmith, incremental tipping and hoarding are off in it, `plan/31` §4). In the order
the author's answers set (§7), with the bounty trips dropped (§6).

1. **The standalone commands over what is built.** BUILT (§5a).
2. **The breakdown** after every round, and `loot last`. BUILT (§5b).
3. **Places the author sells in**: Mist Harbor by `sell_fwi`, and the shops the
   Hinterwilds lacks. BUILT (§5c).
4. **The dead keys of the loot**: overflow containers, the `autoclose` list, `loot_keep`,
   `critter_exclude`, `keep_closed`, the group's disks, phasing with 704; `loot ground`;
   `loot reset unskinnable`; and `track_full_sacks`, `log_unlootables`, `favor_left`
   (§7 item 4). BUILT, in three parts (§5d, §5e, §5f).
5. **The round's missing pieces that every profile meets**: *lighten your load*, a full
   pool, full bags during a return, the worker by the room's tag, scarabs and the gem
   shop's odd nouns, the ingot, plinite; and step 1's leftovers, `loot pool` banking the
   boxes' silver and `loot box` taking an empty box to a bin. Scripted-game tests of
   `loot sell`, `loot pool` and `loot box`. BUILT, in three parts (§5g, §5h, §5i).
6. **Selling by choice**: `loot sell type <types>`, `loot sell shop <shops>`, `loot sell
   item <names>` (eloot's three `--` forms, in Hydra's words).
7. **The coin hand**, coin bag and gambling kit: found, used after a loot, drained at the
   bank; `sell_share_silvers`.
8. **The town locksmith** with its priority and `case` boxes; **incremental tipping**;
   Pinefar's banker; the free-to-play ladder.
9. **The rest of the shops**: consignment, `break_rocks`, `dump_herbs_junk`, cursed items
   (315, the gauntlet), the sell buffs, blood bands, and `between` as the player's own
   rules (§7 item 3). `alchemy_mode` later (§7 item 7).
10. **The hoard**, after `plan/59`'s lockers: the jars' inventory in the model, deposit,
    raid, `loot hoard list|deposit|reset|raid`, `loot bounty`.

Each step: the profile keys it makes live, the importer's note for them removed, tests
against a scripted game, `plan/31`'s record and the reference page updated in the same
commit. A branch, `loot-complete`.

## 5a. Step 1, BUILT 2026-09-30

- **The errands** (`cena-behavior/src/loot.rs`, `Errand`): `Room`, `Skin`, `Box`, `Sell`,
  `Pool { drop, collect }`, `Deposit`. Each is a one-shot machine (`Hunt::loot_only`,
  `hunt/errands.rs`) the hunt desk starts as it starts `heal stock`: claimed, echoed as
  `loot>`, stopped by `stop`, ended `Ending::Looted`. Refused, and said, when the
  character has no loot profile.
- **The words** (`hunt/command.rs`, `loot_words`): `loot`, `loot skin`, `loot box`, `loot
  sell`, `loot pool`, `loot pool deposit`, `loot pool return` (`check`, `loot`), `loot
  deposit`; `loot help`; `loot show`, `loot set <setting> <value>`, `loot unset
  <setting>` over the loot profile's file, `skin.` and `town.` keys with it. The reports'
  five words are left to the reports, which no longer take bare `loot`
  (`crates/cena/src/loot.rs`).
- **The planners' new modes**: skinning alone, whatever `skin.enable` says
  (`loot/plan/alone.rs`); the round as the pool alone, with or without the drop-off and
  the collecting, or the bank alone (`town::Round`, `Seller::for_round`, `Pool::only`).
- **The driver** (`hunt/drive/loot.rs`, `loot_errand`): the dead here found from the
  state; the box in hand emptied, then kept when the profile sells such boxes, else
  `trash` and, still held, `drop`; the selling round asks for the stow list when it has
  not been read, and says how each errand went.
- **An agent** at `behaviors` may start each (`crates/cena/src/perform.rs`); `;help`,
  `loot help` and the reference page (`crates/cena/src/command_reference.rs`) list them.
- *Tests:* the words (`hunt/command.rs`); three over a scripted game
  (`tests/loot_errand.rs`): `loot` searches the dead, loots the room and ends; `loot
  skin` skins and searches nothing; `loot deposit` walks to the bank, deposits, withdraws
  the kept silver and walks back, **the first run of the selling round's driver against a
  scripted game**. Not yet over a scripted game: `loot sell`, `loot pool`, `loot box`.
- **Left for step 5**, as differences from eloot: `loot pool` does not bank afterwards
  (eloot deposits the box silver, `eloot.lic:7641`); `loot box` does not walk to a bin
  (`save_trash_box`, `:7773`); a missing loot profile is refused, where eloot has defaults.
- **Found, not this plan's**: on `main` at `75095f2a` two split parents are over their
  caps and `ratchet.rs` is red for them: `cena-behavior/src/hunt/drive.rs` (467 of 450,
  since `4eb41db3`) and `cena-gui/src/keys.rs` (459 of 450, since `1d114e2d`).

## 5b. Step 2, BUILT 2026-09-30

- **The breakdown** (`cena-behavior/src/town/breakdown.rs`, `Breakdown`): pure, fed each
  prompt's `LootFact`s by the seller (`Seller::outcome`), so it costs the round no
  command and no `wealth`. Silver by shop (gem shop, pawnshop, furrier); the locksmith
  pool as what its boxes held less its tips and fees, with how many boxes were given and
  came back; a total of the sales and the boxes less the pool's cost; the Chronomage's
  credit apart, since it is not silver; the bank's deposits, notes and withdrawals; what
  was appraised and not sold since, with its figure, or *too valuable for the shop*; and
  how many items the round gave up on.
- **Said at the end of every round**, a hunt's and an errand's
  (`hunt/drive/selling.rs`), in place of the one line that counted what could not be
  sold; kept on the desk's `Reports` for **`loot last`**, until Hydra is closed.
- **Not eloot's**: its *Pool Return* count added into the silver total (`eloot.lic:6231`)
  is not reproduced. `loot box` alone says no breakdown: it is not a round.
- *Tests:* the adding up, the pool's net, what is kept and what a sale takes off that
  list, the grouping of figures, a round with nothing to say (`breakdown.rs`); `loot
  last` as a word; and over a scripted game the bank trip says *deposited 12,340* from
  the game's own line (`tests/loot_errand.rs`).

## 5c. Step 3, BUILT 2026-09-30

The author's own round (§7 item 5: *"I do sell in fwi has hinterwilds doesn't have
pawnshop to sell at"*).

- **Where a shop is** (`cena-behavior/src/town/route.rs`, `shop_room`): the nearest room
  tagged for it by what this walker would pay, as before, with eloot's two rules over it.
  - **`sell_fwi`** (`ELoot.go2`, `eloot.lic:3300-3309`): a shop the Isle of Four Winds
    has is the island's, wherever the round begins; one it lacks is the nearest anywhere.
    The walk there and back is travel's own, by the trinket its settings name
    (`travel/routines/trinket.rs`), and the round ends where it began.
  - **The Chronomage** (`go_sell`, `:7106-7110`): none on the island, so gold rings are
    given before the town is left, **first in the round** when it sells on the island
    (`goods::in_order`), and not at all from the island. eloot gives them only when no
    box sent it to the island's pool first; Hydra always gives them first.
  - **The Hinterwilds** (`shop_unavailable_in_town?`, `:3236-3253`): when the nearest
    town is Coldriver Village (the game's room 7503205) and the round does not sell on
    the island, the pawnshop, the collectibles counter, consignment and the Chronomage
    are passed by, never walked to in another town.
- **The setting** is typed and shown: `town.sell_fwi`, *Sell in Mist Harbor*, on the
  *Selling* page (`town/settings.rs`); it was imported from eloot's file and read by
  nothing.
- **No way to the island** (no trinket named on *Travel*): the round says so once and
  sells where it stands, the Hinterwilds' rule included (`route::reaches_fwi`,
  `hunt/drive/selling.rs`). eloot stops with *set your FWI trinket in go2 setup*.
- *Tests:* the routing over a small map of a town, the island and the Hinterwilds
  (`route.rs`: the island's shop over the town's, the Chronomage, the four shops the
  Hinterwilds lacks); and over a scripted game a profile that sells in Mist Harbor banks
  at the island's bank though the town's is as near (`tests/loot_errand.rs`).
- **Not ported** (the author, the same day: *"I don't think we want the go sell in
  another town for a bounty thing"*): `fwi_return` before a bounty's skins and gems are
  sold in its own town (`:3274`), and the note carried back to a bank off the island
  after it (`deposit_note`, `:3342`). See §6.

## 5d. Step 4a, BUILT 2026-10-01: what is taken and searched, and what is learned

Step 4 is built in three commits: 4a here, 4b the bags (overflow, `keep_closed`,
`track_full_sacks`, the group's disks), 4c the hands, phasing and `loot ground`.

- **`keep`** (eloot's `loot_keep`, `cena-behavior/src/loot/worth.rs`): a thing named in it
  is taken whatever its kind, past every rule that throws a thing out
  (`reject_invalid_loot`, `eloot.lic:5647`), but not past the player's own `leave` nor a
  curse not wanted (`loot_specials`, `:5588-5595`), and it is taken one by one as a
  special (`:5595`), whatever was learned of its name.
- **`leave_creatures`** (`critter_exclude`): a corpse named in it is never searched nor
  skinned (`search`, `:5714`; `skin`, `:5891`), and neither is a child (`:5715`)
  (`LootProfile::leaves_creature`, `loot/plan.rs`).
- **`remember_unlootable`** (`log_unlootables`): the `unlootable` list is read only when
  it is on (`:5655`); and then a thing the game would not let the character hold, of no
  kind the object table knows, is added to it (`ELoot.unlootable`, `:2951-2959`). Off,
  the planner still leaves such a thing for the rest of the hunt.
- **What a visit learns reaches the profile** (`loot/learned.rs`, `Learned`,
  `remember`): a creature that cannot be skinned (as before), a thing that crumbled
  (eloot saves it, `:4149-4153`; not a critter's bandana or robes crumbling as they are
  opened, `:5045-5050`), a thing that could not be held when remembered, and a bag that
  closes itself. The planner collects them (`loot/plan/learn.rs`); the hunt's driver
  hands them on after each visit; the hunt desk writes them in under the file's lock and
  says each kind once (`hunt/keep_learned.rs`, moved out of `desk.rs` at its cap). The
  driver's callback is now `FnMut(&Learned)`.
- **A bag closed by hand is not a bag that closes itself** (`loot/plan/bags.rs`): eloot's
  test (`store_item`, `:4119-4124`): when the game says *It's closed!* and the bag's
  contents are still listed, it closed itself, and is learned and named for the profile;
  not listed, it was closed by hand, and is only opened. Hydra had taken every closed bag
  for one that closes itself. The profile's `autoclose` names are read now: a bag named
  there is opened before anything goes in.
- **`loot reset unskinnable [creature]`** (eloot's `manage_unskinnable`, `:2169-2193`):
  every creature learned unskinnable forgotten, or the one named, in any case
  (`loot::forget_unskinnable`; the binary's `hunt/settings.rs`, `reset_unskinnable`). A
  hunt under way keeps its own list until it ends.
- **The importer** carries `loot_keep`, `critter_exclude` and `log_unlootables`, no longer
  noting them dropped; the *Loot* page shows *Always take*, *Never search* and *Remember
  what cannot be held*.
- *Tests:* the verdict (kept past the weapon rule and a crumbly name, `leave` winning;
  the unlootable list read only when remembered, `tests/loot_worth.rs`); the planner (a
  creature left and a child, a thing kept, a bag closed by hand opened and not learned, a
  bag that closes itself learned once a hunt, a bag named in the profile opened first,
  what could not be held remembered only when asked, `tests/loot_plan.rs`); the profile
  writer and the reset (`loot/learned.rs`, the binary's `hunt/settings.rs`); the words;
  and over a scripted game, a thing that crumbles reaches the profile writer
  (`tests/loot_errand.rs`).

## 5e. Step 4b, BUILT 2026-10-01: the bags

- **Where a thing goes** (`cena-behavior/src/loot/plan/bags.rs`, `Planner::bag_for`): a
  box goes on a disk before any bag, the character's own (`disk`), then with
  `disk_group` (eloot's `use_disk_group`) its group's in the room, its own first
  (`single_drag_box`, `eloot.lic:4035-4066`); **nothing but a box goes on a disk**
  (`:4038`). Then the stow list's bag for the kind, the default, and the overflow
  containers in the profile's order (`single_drag`, `:3958-4007`). A disk that takes no
  more is passed by as a full bag is. Hydra had put boxes in bags first and anything on
  the disk once every bag was full; neither was eloot's.
- **The overflow containers** are found as eloot finds them (`ensure_items`, `:2230`): by
  the name's words among what is worn, `/\b<name>\b/i` over `GameObj.inv`, else by a
  listed container's title (`named_bags`). eloot keeps them as one comma-separated
  string (`set_inventory`, `:2357`); the importer splits it. The selling round reads the
  same bags (`town::goods::selling_bags`).
- **Someone's disk is never loot** (`loot/worth.rs`): read by the model's `Disk::read`, so
  a `rusty iron Duffield coffer` is Duffield's disk, not a box. The rule had read a
  possessive the game never sends, the defect fixed in the model on `main` the same day
  (`74840eec`, `cena-model/src/state/disk.rs`).
- **`keep_closed`**: a bag whose contents are not listed is opened before anything goes
  in, `loot room` too (`open_loot_containers`, `:3869-3889`), and every bag a visit
  opened is closed again before it ends (`:2409`), a disk never. A bag that closes
  itself, learned or named in `autoclose`, is opened before `loot room` as well
  (`:3864-3867`); it had been opened only before a drag.
- **`track_full`** (eloot's `track_full_sacks`, on by default, `:508`): a bag found full
  stays full across the hunt desk's runs, a hunt, a `loot`, the next hunt (`FullBags`,
  `hunt/errands.rs`), until a selling round or a pool trip, which free every bag before
  and after (`:7996`, `:8006`, `:8015`, `:8030`). Hydra never freed one, so a hunt that
  had sold could rest again for a bag it had emptied. Off, each visit tries every bag
  again (`:7911-7914`).
- **The round's bags** (`hunt/drive/selling.rs`): a bag the round sells from whose
  contents are not listed is opened and looked in first, as eloot does on starting
  (`open_single_container`, `:3892-3921`); with `keep_closed` each is closed at the end,
  but not one the ready list keeps a weapon in (`close_sell_containers`, `:3737-3746`).
- **The importer** carries `use_disk_group`, `keep_closed` and `track_full_sacks`; the
  *Loot* page shows *The group's disks too*, *Keep the bags closed* and *Remember full
  bags*.
- *Tests:* the planner (a box on the disk first and nothing else on it; the group's disk
  after the character's own, never a stranger's; someone's disk never taken for a box;
  the overflow bags in the profile's order, found among what is worn; a kept-closed bag
  opened before `loot room` and closed after, the disk left alone; the importer,
  `tests/loot_bags.rs`); over a scripted game, a bag found full skipped by the next `loot`
  until a bank trip frees it, and tried again with `track_full` off; the round's bag
  opened and looked in before it starts and closed after (`tests/loot_errand.rs`).
  Each rule was broken by hand against its tests: 14 mutations, 14 caught. The round's
  two clears were broken together: in these tests either one alone leaves the bags free,
  so neither alone is pinned; the one before matters to a box emptied during the round,
  the one after to a bag that box filled.

## 5f. Step 4c, BUILT 2026-10-01: the hands, phasing and `loot ground`

- **A hand freed** (`cena-behavior/src/loot/plan/hands.rs`): before `loot room`, a thing
  taken one by one or the skinner wielded, one hand must be free and fit to use, a hand
  being unfit with a wound or a scar of rank 3 on its arm or hand (`free_hand`,
  `eloot.lic:3816-3837`). With neither free, the left is freed when the profile favors
  it (`favor_left`) and it is fit, or when the right is not; else the right. An armament
  is stored where the player set it to go, as a trip stores it (`travel/hands.rs`; eloot
  stores a readied weapon, `stow_ready_list`, `:4175-4190`); anything else, or an
  armament that would not store, is dragged into its bag. The step that wanted the hand
  waits, and is sent once the hand is free or after three tries. The hand holding a box
  being emptied is never the one freed. Before, Hydra looted with whatever hands it had,
  and with both full every `loot room` and `loot #id` was refused.
- **Given back** (`return_hands`, `:3923-3944`): before the visit ends, what was put
  away goes back, an armament as a trip takes it back (`remove` when worn, else `get`),
  anything else dragged into the hand it came out of.
- **Neither hand fit** ends the visit, `Left::NoHand`, and the hunt rests for it as *too
  injured* (`Why::Injured`). eloot's own branch for it cannot be reached: a right hand
  that is not fit is always *damaged*, so eloot frees the left and loots anyway.
- **A creature that hands its loot over** (`search`, `:5709-5765`): a tumbleweed, plant,
  shrub, creeper, vine or bush puts it in the left hand, freed first whatever the profile
  says; a skayl, glacei, caedera, golem or elemental in a free one. What lands in that
  hand is dragged to its bag after the search. Before, it stayed in hand.
- **Phasing** (`loot/plan/phase.rs`; `box_phase`, `:2975-2984`): with `phase_boxes`
  (eloot's `loot_phase`, carried and never read until now), a box confirmed in a bag is
  phased, `prepare 704` then `cast at #box`, cast again while armour hinders it (five
  times at most), when Phase is in the spell list and its cost can be paid
  (`cast::ready`). Never a box of enruned or mithril, nor one put on a disk, which eloot
  does not phase either (`single_drag`, `:4002`).
- **Unphased at the pool** (`town/pool.rs`; `box_unphase`, `:2986-2996`): each box in
  hand is looked at before the worker takes it; one that is *shifting* is dropped, which
  it will not be, and comes back whole, found in the hand again by whatever id it has
  then. **Claude's call, for the author:** this is done only when the profile phases
  boxes. eloot looks at every box, which is a `look` per box for every profile.
- **`loot ground`** (`box_loot_ground`, `:5144-5219`): the hands put away as a trip puts
  them away; each box on the ground taken up, emptied by the box planner, and kept or
  thrown out as `loot box` does; a locked box put back where it lay; someone's disk left,
  whatever its noun. The hands are given back at the end. eloot opens a box where it lies
  before taking it up; Hydra takes it up and lets the planner open it, so a locked box
  costs a `drop` more and an open one nothing.
- **Found building it**: eloot's line for a locked box, *It appears to be locked.*
  (`:5151`, `:7270`), was not read as locked (`loot/outcome.rs`), so the box planner took
  a locked box for an empty one, and `loot box` would have thrown it out.
- The importer carries `favor_left`, which it had put under `[town]` where nothing read
  it; the *Loot* page shows *Free the left hand first*. `loot ground` is in `loot help`,
  the reference page and an agent's operations.
- `loot/plan.rs` passed its cap of 600 with this; the planner's words, `Step` and
  `Left`, moved down to `loot/plan/step.rs`.
- *Tests:* the planner (the right stored and given back; `favor_left` freeing the left
  and dragging it back; a right arm too hurt to use; neither hand fit; a free hand
  freeing nothing; the box's hand never freed; a bramble handing its loot to the left; an
  ordinary corpse searched with full hands; a box phased, and cast again when hindered;
  none phased on a disk, of mithril, or without the spell; the importer,
  `tests/loot_hands.rs`); the pool (a phased box unphased and given by its new id, a
  whole one looked at once, `tests/town_pool.rs`); the two new lines
  (`tests/loot_outcome.rs`); over a scripted game, `loot ground`
  (`tests/loot_errand.rs`). Each rule was broken by hand against its tests: 15
  mutations, 15 caught.

## 5g. Step 5a, BUILT 2026-10-01: the pool

Step 5 is built in three commits: 5a here, the pool's own gaps; 5b what a return leaves
that no bag takes (the box set aside while the round sells, the ingot, the shops after the
pool's returns, a box thrown out only once it is known empty); 5c `loot box` to a bin, the
gem shop's odd nouns, and the scripted tests of `loot sell` and `loot box`.

- **The worker the map names** (`find_worker`, `eloot.lic:3204-3211`):
  `meta:boxpool:npc:<name>`, read by `cena_map::Room::pool_worker`; the selling driver
  tells the round each step (`Seller::worker_here`). Every pool on the map names its
  worker: 13 rooms, 12 names, and two of them, a dead halfling pirate and a grimy
  halfling scoundrel, answer to none of eloot's eight words, so those two pools were
  passed by. MEASURED over `reference/mapdb/mapdb.json`; the map Hydra ships carries the
  same twelve. A worker the map names and the room lacks is no worker: a word could find
  somebody else, and the box and its tip would go to them.
- **A full pool** (`handle_full_pool`, `:7420-7427`; `:7404-7407`): the box goes back to
  its bag and stays among those to give; what is ready is collected; once a box has come
  back there is room, and the drop-offs go on. A full pool that gives nothing back takes
  nothing more. Too little silver for the tip still stops the drop-offs for the visit.
- ***Lighten your load*** (`pool_return`, `:7443-7452`): the round goes to the bank,
  deposits, keeps its silver, and comes back to the pool, whose visit goes on where it was
  (`town/plan/bank.rs`, `Seller::bank_between`). Refused again straight after, the
  returns are given up, as eloot gives them up. **A box's coins that would not all fit**
  (`box_loot`, `:5109-5115`) go the same way: the bank, then the box again for the rest;
  the box planner says so (`loot::Emptied::CoinsLeft`). `loot box` does the same by
  itself; `loot ground` stops there, as eloot's does (`:5170-5173`), the box put back
  with the rest of its coins.
- **A plinite the worker hands back** (`box_loot`, `:5138-5140`; `pool_return` takes a
  box or a plinite, `:7455-7464`) is plucked, `pluck #id`, and what came of it put in the
  default bag; a sword already in the other hand stays. Before, a plinite in hand ended
  the returns.
- **`always_check_pool`** (`process_boxes`, `:7696`, `:7714`): the round asks the pool for
  its returns with no box to give, and with drop-offs off (`sell_locksmith_pool`). Typed
  and shown on the *Selling* page, *Always check the pool*; the importer carried it
  already.
- **`loot pool` banks after** (`pool`, `:7626-7648`): a drop-off asks what is carried
  first (`wealth quiet`), and the bank after keeps exactly that, so what the boxes held is
  deposited and the tips paid are evened out. `loot pool return` banks nothing, as
  eloot's does not. A round's tips count as earnings now: a round that only gave boxes
  ends at the bank, as eloot's `silver_deposit` does whenever the silver changed. §5a
  had said `loot pool` did not bank; it did when a box's coins were gathered, keeping
  `sell_keep_silver` rather than what was carried, and it did after `pool return` too.
- **A step the pool repeats five times** ends the visit, as any other step of the round
  does. The pool had been left out of that count, and a box that would not go back in its
  bag sent the same `_drag` until the round's cap of 400 steps.
- **Found building it: the round read the room it had left.** A walk ends on travel's own
  copy of the state; the hunt's copy folded the new room's lines only at its next send.
  So the round, arriving at the pool, looked for the worker among the last room's NPCs and
  passed the pool by; arriving at the Chronomage it looked for the clerk the same way and
  gave no ring. The hunt's own turn drains its stream first, so the hunt was not
  affected; every walk inside an errand was. The driver now folds what the walk's last
  step brought before the walk returns (`hunt/drive/walk.rs`). The defect is on `main`
  too, and not fixed there: the crate review of the same day has work in flight in the
  same driver.
- `town/plan.rs` passed its cap of 800 with this; it is a split parent now, capped at
  700, the sacks sold whole (`town/plan/sack.rs`) and the bank's part in a round
  (`town/plan/bank.rs`) moved down.
- *Tests:* the planner (the worker the map names over eloot's words, and none when the
  named one is not there; a full pool refilled once a return made room, and not when none
  did; *lighten your load* to the bank and back, and given up when refused again; coins
  that would not fit; a plinite plucked and put away; `always_check_pool` with no box and
  with drop-offs off; `loot pool` banking what was carried, `pool return` not banking; a
  pool step repeated five times, `tests/town_pool.rs`); the box planner's coins left
  (`tests/loot_plan.rs`); the map's name (`cena-map`, `room.rs`); and over a scripted
  game, `loot pool` in a small town, the worker the map names chosen over a woman in the
  same room and the bank after keeping the 1,234 carried (`tests/loot_rounds.rs`; the
  errand harness moved to `tests/errand_support/` for it). Each rule was broken by hand
  against its tests: 20 mutations, 20 caught, the walk's fold among them: undone, the
  scripted `loot pool` gives its box to nobody.

## 5h. Step 5b, BUILT 2026-10-01: what a return leaves that no bag takes

- **A box is thrown out only when it is known empty.** The box planner says how a box
  came out (`loot::Emptied`): emptied; locked; coins left; **never listed**, its `look
  in` answered with no listing, which eloot puts back (`box_loot`, `eloot.lic:5096`); or
  **things left**, something the profile wants still in it, every bag that would take it
  full, a drag that never took, or a gold ingot. Before, a box not emptied was taken for
  an empty one, and the pool, `loot box` and `loot ground` threw it out with what it
  held. The crate review of the same day found the never-listed case on `main` (BE-E-5);
  the others were found here. An empty box is listed as one: the game sends its
  `<inv>` header and ` nothing` (`cena-model/src/state/inventory.rs`), which the scripted
  `loot ground` test had not answered with, and now does.
- **A gold ingot that will not fit** says nothing of the bag (`single_drag`,
  `:4141-4146`): too heavy for any, it is left where it is and the bag stays open to the
  rest. Before, each bag it was tried in was marked full for the hunt.
- **A returned box that still holds what no bag would take** (`pool_full_recovery?`,
  `:5384-5388`; `pool_direct_sell_recovery`, `:5455-5491`): the pool's visit stops with
  the box in hand; the round sells, the box's contents among its goods, a gold ingot
  whatever the profile sells (`handle_ingot`, `:7188-7203`), and comes back to the pool,
  where the box is emptied first with the room the sales made, and the returns go on
  (`town/plan/returns.rs`). Once a box: set aside a second time it stays in hand and the
  round says so, as eloot pauses (`pool_sell_recovery`, `:5516-5526`); `loot pool`
  sells nothing, so its box stays in hand at once (`stow_box_item`, `:5363`). A stow to
  free a hand puts the other hand's thing away, never that box. **Claude's call, for the
  author:** eloot sells the box's contents straight from the box, a trip per shop, then
  runs a whole round, then goes back to the pool; Hydra holds the box through the round
  it is already running, which does both in one pass. eloot parks the box on a disk when
  what is left has nowhere to sell (`pool_sell_recovery`, `:5516`); Hydra keeps it in
  hand.
- **The shops follow what the returns brought** (`Sell.sell`, `:7820-7835`:
  `check_items` after `process_boxes`): after each visit to the pool, the shops for
  anything the round has not seen are queued, one visited already again too. Before, the
  round chose its shops before the pool, and what the boxes held waited for the next
  rest.
- **What the hands held when the round began is never for sale** (the crate review's
  BE-E-6): a weapon stowed in a selling bag to free a hand at one shop was sold at the
  next. Those ids are left out of the round's goods, and fetched back at the end as
  before.
- `town/goods.rs`'s `lots` takes the round's goods instead of reading the bags itself,
  so the box held aside and the hands' ids are decided in one place (`Seller::goods`).
- *Tests:* the box planner (a box never listed and one listed empty; a gem no bag takes;
  a gold ingot marking no bag full, the next thing still going in, `tests/loot_box.rs`);
  the round (the shops after the returns; a box set aside, its emerald sold, the box
  emptied on coming back and the returns going on, the shield put away for a hand and
  the box kept; an ingot sold from the box; a box set aside twice staying in hand; the
  sword stowed at the gem shop not sold at the pawnshop, `tests/town_returns.rs`); the
  pool (a box never looked into kept; `loot pool` keeping a box it cannot empty,
  `tests/town_pool.rs`); and over a scripted game, a chest the worker hands back and
  `look in` never lists put back in the pack, and a chest holding an emerald the pack
  refuses kept in hand and said (`tests/loot_rounds.rs`). Each rule was broken by hand
  against its tests: 17 mutations, 17 caught.

## 5i. Step 5c, BUILT 2026-10-01: `loot box` to a bin, and the round run whole

- **`loot box` takes an emptied box to a bin** (`save_trash_box`, `eloot.lic:7786-7791`,
  `:7809`): when `trash` says there is no receptacle here, it walks to the nearest pool's
  room, throws the box out there, and walks back; with no pool on the map, or no
  receptacle there either, it drops the box where it is. `loot ground` drops it in place,
  as eloot's does (`box_loot_ground`, `:5202-5204`). eloot's `go2('locksmith pool')`
  would go to Mist Harbor's pool for a profile that sells there; Hydra takes the nearest,
  a box's bin not being worth the island.
- **A reliquary is never thrown out** (`save_trash_box`, `:7781`): kept whatever the
  profile sells, at the pool and by `loot box`. It had been kept only by a profile that
  sells boxes.
- **The gem shop's odd ones** (`check_items`, `:6523-6526`): a scarab when the profile
  sells scarabs, and a crystallized thorn the object data calls a gem, both go to the gem
  shop, as they already did; now pinned (`tests/town_shops.rs`). The thorn's shop is
  eloot's thorn-and-berry rule, not the object data's: broken, the test fails. Clothing
  both shops buy was already offered at the pawnshop too.
- **Step 1's leftovers** (§5a) are done: `loot pool` banks (§5g) and `loot box` walks to
  a bin (here). A missing loot profile is still refused, where eloot has its defaults; it
  was not in this step.
- **The round run whole over a scripted game**, §5a's *not yet*: `loot sell` sells an
  emerald at the gem shop item by item (the backpack holds a diamond the profile keeps,
  so it is not sold whole), banks, comes home, and says what it came to; `loot box`
  empties a chest and takes it to the pool's bin and back, and banks for coins that will
  not fit and gathers the rest; with `loot pool` and the pool's returns (§5g, §5h), all in
  `tests/loot_rounds.rs`.
- *Tests:* the reliquary (`tests/town_pool.rs`), the gem shop's odd ones
  (`tests/town_shops.rs`), and the three scripted runs above. Each rule was broken by
  hand against its tests: 5 mutations, 5 caught.

## 6. Not ported, and why

- **Going to the bounty's town to sell** (the author, 2026-09-30: *"I don't think we
  want the go sell in another town for a bounty thing"*): `check_bounty_furrier` and
  `check_bounty_gems`' walk to the bounty town's furrier and gem shop (`:6319-6489`),
  the `Region` module that says whether that town is in reach (`:2530-2593`),
  `fwi_return` (`:3274`) and `deposit_note` (`:3342`). The round sells where its own
  rules send it, **and sells a bounty's gems and skins like anything else**: nothing is
  held back for a bounty. The author, asked: *"sell them like anything else. that's what
  the between scripts and the gem hoarding is for. Not here to feed them, here to give
  them the tools they need to fish."* A player who wants a bounty's gems kept writes
  that as a `between` rule (step 9) or hoards them (step 10); the round itself has no
  bounty logic. So eloot's furrier check that keeps bundles whole for a skin bounty is
  not built either, nor `locksmith_when_gem_bounty`, which sends boxes to the town
  locksmith rather than the pool while a gem bounty is open (`:7690`; Claude's reading of
  the same rule, put to the author); the hoard's own `gem_horde_turnin` stays with the
  hoard. §3's table rows for the bounty checks and the region, and
  §4's *Routing is the map's* bullet where it speaks of a region's edge, are superseded
  by this.

- `debug`, `debug file`, the `DebugLogger` (`:268-376`): Hydra's wire log and player log
  already hold what it writes.
- `start`, `load`, `ver`: a preload, a reload and a version for a script that keeps state
  between runs; Hydra reads the profile when the behavior starts.
- `test`: `hunt check` and `loot show` say what it says.
- The GTK window (`:728-1939`): the three pages.
- Urchin guides and `go2 --disable-confirm` (`:3300-3340`): travel's.
- eloot's own defects, not reproduced: `;eloot list <category>` unreachable (`:7974`);
  *Pool Return* counted into the silver total (`:6231`); the *Reagents Hoarded* section
  that never prints (`:6275`); `locksmith_withdraw_amount`'s three defaults (8000, 10000,
  8000); `silence`, never read.

## 7. For the author

**ANSWERED 2026-09-30**, the same day:

1. *"agree"*: `loot` alone loots the room; the report is `loot summary`.
2. *"yes"*: an agent at `behaviors` may start them.
3. `between` is **custom behaviors** between the boxes and the selling: *"say you want to
   keep a stock of 5 blue crystals in reserve and sell the rest, well between scripts is
   where you would put that behavior, or maybe you want to duplicate all the wands on you
   before selling them, ect."* So it is not a list of commands to send: it is a hook where
   the player's own steps run, with the inventory to decide on. Its shape is step 9's to
   design (a reserve rule such as *keep 5 of X* is data; duplicating wands is a behavior),
   and a script through the runner is the general case.
4. *"all 3"*: `track_full_sacks`, `log_unlootables` and `favor_left` are built as eloot
   has them.
5. *"I do sell in fwi has hinterwilds doesn't have pawnshop to sell at."* So FWI and the
   Hinterwilds are the author's own round, and **move up to step 3**, ahead of the dead
   keys; Pinefar and the free-to-play ladder stay in step 8.
6. *"agreed"*: the hoard waits for `plan/59`'s lockers.
7. *"later"*: `alchemy_mode` waits.

The steps as answered: 1 the commands; 2 the breakdown; **3 FWI (`sell_fwi`, the trinket
home, notes carried back) and the Hinterwilds' missing shops**; 4 the dead keys with the
three of item 4; 5 the round's missing pieces; 6 selling by choice; 7 the coin hand; 8
the town locksmith, incremental tipping, Pinefar, free-to-play; 9 the rest of the shops
and `between`; 10 the hoard. §5 is numbered in this order.

The questions as asked:

1. **The word.** `loot` alone loots the room and the summary becomes `loot summary` only
   (recommended); or the reports keep bare `loot` and the behavior takes another word
   (`eloot`, which is in players' fingers but names Lich's script).
2. **The agent.** Should `loot`, `sell`, `pool` and `deposit` join the operations an agent
   at `behaviors` may start (`perform.rs`)? Recommended: yes, they are behaviors like
   `heal stock`.
3. **`between`**: eloot runs Lich scripts after the boxes. A list of Hydra commands (and,
   with a runner, script names) in its place?
4. **The dropped bookkeeping keys**: `track_full_sacks` (Hydra forgets full bags at each
   run today?), `log_unlootables`, `favor_left`. Build each as eloot has it, or name the
   ones you use.
5. **The order after step 1**: the steps follow your profile. Is anything in 6 to 9 wanted
   sooner (the coin hand, FWI)?
6. **The hoard waits for lockers** (`plan/59` §5a). Agreed, or wanted before?
7. **`alchemy_mode`** reads `Vars.needed_reagents`, another script's variable. Hydra has no
   alchemy behavior; a `needed_reagents` list in the loot profile instead?
