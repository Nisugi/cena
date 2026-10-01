# 61. Loot, complete: every command and feature of eloot

**Status: PROPOSED 2026-09-30.** The author: *"let's plan out the rest of it. Hydra's loot
should have all the features and commands of eloot."* Questions for the author in §7.

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
locksmith, incremental tipping and hoarding are off in it, `plan/31` §4).

1. **The standalone commands over what is built.** `loot`, `loot skin`, `loot sell`,
   `loot pool [deposit|return]`, `loot deposit`, `loot box`, the errand machine and
   endings, `stop`, the agent's operations, the help and the reference page. The first
   scripted-game tests of the selling driver. `loot show|set|unset`.
2. **The breakdown** after every round, in a hunt or not, and `loot last`.
3. **The dead keys of the loot**: overflow containers, the `autoclose` list, `loot_keep`,
   `critter_exclude`, `keep_closed`, the group's disks, phasing with 704; `loot ground`;
   `loot reset unskinnable`.
4. **The round's missing pieces that every profile meets**: the bounty checks at the
   furrier and gem shop with the region, *lighten your load*, a full pool, full bags
   during a return, the worker by the room's tag, scarabs and the gem shop's odd nouns,
   the ingot, plinite.
5. **Selling by choice**: `loot sell type <types>`, `loot sell shop <shops>`, `loot sell
   item <names>` (eloot's three `--` forms, in Hydra's words).
6. **The coin hand**, coin bag and gambling kit: found, used after a loot, drained at the
   bank; `sell_share_silvers`.
7. **The town locksmith** with its priority, the gem-bounty override and `case` boxes;
   **incremental tipping**.
8. **Places**: FWI and the trinket, the Hinterwilds, Pinefar's banker, the free-to-play
   ladder.
9. **The rest of the shops**: consignment and `alchemy_mode`, `break_rocks`,
   `dump_herbs_junk`, cursed items (315, the gauntlet), the sell buffs, blood bands,
   `between`.
10. **The hoard**, after `plan/59`'s lockers: the jars' inventory in the model, deposit,
    raid, the bounty's gems, `loot hoard list|deposit|reset|raid`, `loot bounty`.

Each step: the profile keys it makes live, the importer's note for them removed, tests
against a scripted game, `plan/31`'s record and the reference page updated in the same
commit. A branch, `loot-complete`.

## 6. Not ported, and why

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
and `between`; 10 the hoard. §5's numbering is the proposal's; this order governs.

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
