# 34 — The loot ledger: loottracker, measured and ordered

**Status: PROPOSED 2026-09-24, author asked for it** (*"it's probably something I want to add
in and fix the bugs"*). The author's own script, `loottracker.lic` (Nisugi, v0.2.1, beta),
ported into Hydra as a **classifier and a ledger**: what a hunt earned, from a searched
corpse to a sold gem, recorded as it happens and reported on demand.

Decisions the author gave on 2026-09-24, before this was written:

- **No history to carry**: *"no history, all new."* The schema is Hydra's to shape.
- **SQLite is already embedded**: the combat recorder (`cena-session/src/combat_recorder/`)
  writes it through `rusqlite`. The ledger goes beside it, in the same crate, on the same
  road; no new dependency, no new crate. The author asked whether combat reading and writing
  fit there too: they do, and §5 says how.
- **One known bug**, the Red Forest: *"in the red forest where the room has 2 uids it
  breaks."* §3 has its cause and its fix, which the port gets by construction.

## 0. Measured

`reference/scripts/scripts/loottracker.lic` is **5,012 lines**. Its parts, by module header
(`grep -nE '^  (module|class)'`):

| Part | Lines | What it is | Port |
|---|---|---|---|
| `Patterns` | 280 | **55 regexes, 37 of them triggers**, over raw XML | the classifier's vocabulary (§2) |
| `Parser` + `XMLHelpers` | 117 | slice on the prompt, dispatch a chunk to a processor by which trigger it holds | the chunk is Hydra's already |
| `Processors` | 1,086 | 18 processors: what each chunk means | the classifier's arms |
| API methods (`record_*`, `update_*`, `classify_item`) | 1,332 | write rows, link boxes to their opening, appraisals and sales to their item | the ledger (§4) |
| `Database` | 358 | six tables and eight migrations | the schema, fresh (§4) |
| `Reports` | 456 | summaries, the monthly loot cap, creatures, boxes, lookups | the reader (§5) |
| `CLI` | 1,138 | `;loottracker` and its subcommands, formatting | `;loot` (§6) |
| hooks, state, run loop | ~250 | a downstream hook buffering lines to the prompt; a 50 ms polling loop | nothing: the session actor delivers chunks |

**Every pattern matches raw XML.** MEASURED: `<a exist=` occurs 76 times in the 280 pattern
lines; eight patterns spend most of their length re-tokenising `<pushBold/><a exist="…"
noun="…">…</a><popBold/>`. Hydra's parser did that once (`plan/12` §3a): a `ChunkLine`
carries its links typed, so a pattern here says only *which line and which link*. This is the
same shortening `containers.rs` records for `stowlist.rb`.

## 1. What exists in Hydra

| piece | where | state |
|---|---|---|
| the chunk, closed on the prompt, with every link typed | `crates/cena-model/src/state/chunks.rs` (`Chunk`, `ChunkLine::links`, `objects`, `bold`) | built (M3); it is what `Lich::DownstreamHook` + a buffer sliced on `<prompt` reimplement |
| a classifier over a closed chunk, stateless, then a recorder | the combat FSM (`state/combat/`) and `cena-session/src/combat_recorder/` | built: `GameState::close_chunk` → `ChunkFacts` → `actor/combat.rs::publish_combat` → the recorder's bounded queue, one chunk one transaction |
| SQLite | `rusqlite`, `combat_recorder/{schema,attack,status,worker}.rs`; one file per character, `<game>_<name>_combat.db` | built |
| object typing (`item_type`) | `state/gameobj.rs` `classify(noun, name)` | built; loottracker's `classify_item` (105 lines, `:3271`) reimplements a corner of it from a hand list |
| the room's id, the wire's | `state.room.id`, the `<nav rm=>` value | built; **this is the Red Forest fix** (§3) |
| who is looting | `state.character.name`, `instance` | built |
| silver, coins, the hands | `state.character`, `hands.rs` | built |
| commands on the line | `crate::commands` in the binary, `hunt/command.rs` as the model | built (M6a) |

So the ledger's input, the chunk with typed links, and its output road, a recorder beside the
combat one, both exist. What does not exist is the **middle**: the classifier that says what a
chunk means for loot, and the tables that hold it.

## 2. The classifier: 55 patterns as one enum

`cena-model/src/state/ledger/` (the crate's naming: the classifier is model, the file is
session). A stateless `fn classify(chunk: &Chunk) -> Vec<LootFact>` over the lines, arms in
loottracker's dispatch order (`Parser.process`, `:816-893`). What each processor extracts,
typed:

| loottracker | Fact | From |
|---|---|---|
| `SearchProcessor` (90) | `Searched { creature, silvers, items, finds }` | `You search the <creature>.`; then `had N silvers on`, `had/carried/left <item>`, `Interesting, … carried`, the klock key/lock `appears on the ground`, gemstone dust, the jewel *at your feet*, an LTE boost |
| `SkinProcessor` (27) | `Skinned { creature, skin }` | `You skinned the <creature>, yielding <skin>` |
| `BoxProcessor` (133) + `LocksmithProcessor` (53) | `BoxOpened { box, silvers, items }`, `BoxReturned { box }`, `PoolQuoted { box, tip, fee }`, `PoolDropped { box, tip, fee }`, `BoxLookedIn { box, items }` | `You gather the remaining N coins from inside <box>`, the charm's swarm, `<inv id=>` lines, `here's your <box> back`, the two pool messages |
| `AppraisalProcessor` (37), `LoresongProcessor` (55), `ShopAppraisalProcessor` (59) | `Appraised { item, value, by: Gem \| Skin \| Loresong \| Shop }` | `You peer intently at`, `You turn the <hide> over`, the two loresong lines, `turns the <item> over in his hands` + `I'll give you N silver`, the jeweler's one-liner, `probably worth about` |
| `SellProcessor` (71), `GemshopProcessor` (86), `FurrierProcessor` (27), `ChronomageProcessor` (26) | `Sold { item, silvers, to: Pawn \| Gemshop \| Furrier \| Chronomage, note: Option<item> }`, `Worthless { item }`, `TooValuable { item }` | `offer to sell`, `hands you N silver coins`, `scribbles out a <chit> for N`, `basically worthless`, the gem shop's four forms, the furrier's two, the halfling's ring |
| `BankProcessor` (28) | `Deposited(N)`, `Withdrew(N)`, `NoteDeposited(N)` | the three teller lines |
| `BountyProcessor` (21) | `Bounty { points, experience, silver }` | `[You have earned …]` |
| `BundleProcessor` (59) | `Bundled { skin, bundle, container, created: bool }` | the three bundle lines |
| `WandDupeProcessor` | `WandDuplicated { donor, copy }` | the 918 line, the gesture, the hand |
| `KlockProcessor` (20), `SpecialFindProcessor` (72) | folded into `Searched.finds` | — |
| `GemshopProcessor`'s shatter | `Shattered { item }` | `Your focused voice causes the <gem> to shatter` |

Every pattern's **item** is a `ChunkLine` link read as `ItemRef { id, noun, text }`, never a
regex over `exist=`. The ten test fixtures loottracker's comments quote as `# Real:` lines are
the classifier's first tests, each cut from the same wire the comment cites; the three replay
fixtures already in `cena-behavior/tests/fixtures/` hold six searches between them.

What stays in the processors and is **not** classification: linking an appraisal or a sale to
the item that was searched up (`update_item_*`, `:2638-2957`), linking a returned pool box to
the box that was dropped (`link_returned_pool_box`, `:2480`), and the two-line waits
(`LoresongProcessor`, `ShopAppraisalProcessor` cache a `@pending_item` across chunks). Those
are the ledger's (§4).

## 3. The Red Forest bug

`current_room_uid` (`:121`) returns `Room.current.uid`, and on a Lich map room that is an
**array** (`map_gs.rb:31`, `attr_accessor … :uid`; `map_base.rb:608`, `uids_add`), since one
map room can carry several game ids. Every `loot_events` row and every pool-drop row stores it
in a `String` column (`:206`, `:272`), so a two-uid room writes the array's text or raises; and
the pool reconciliation `.where(pool_room_uid: room)` (`:2494`) compares the stored text
against the current array, which never matches, so a box returned from the pool in such a room
is never linked to the box that was dropped.

The fix is to record the wire's own room id, the `<nav rm=>` value (`XMLData.room_id`,
`xmlparser.rb:423`), not the map's list. Hydra's `state.room.id` **is** that value, so the
port gets it right without a decision. Recorded here so the reason survives: a map's uid is a
set, and a set is not a key.

Other defects found on reading, to verify against the wire before fixing:

- `SHOP_APPRAISE_VALUE` is a trigger on its own (`:783`), so a chunk holding only `I'll give
  you N silver` with no cached item records nothing and logs nothing.
- `classify_item` (`:3271-3418`) types by a hand-kept noun list; where it and
  `gameobj-data` disagree, the ledger's `recent <type>` filter and eloot's `loot_types`
  disagree about the same item. The port uses one table.

## 4. The ledger: tables, fresh

In `cena-session/src/ledger/`, beside `combat_recorder/`, on its pattern: a worker thread
with a bounded queue, one chunk one transaction, the server's clock passed in. **One database
per character** with the combat tables and these, so a report that wants kills against loot
per creature is one query; Lich's combat schema untouched.

loottracker's six tables, reshaped with what the author gave (no history) and what the port
learned:

| Table | Holds | Change from loottracker |
|---|---|---|
| `loot_events` | a search or a box opening: source id and name, silvers, **room id as the wire's**, at | `room_uid` gone; `room_id` is `<nav rm=>` |
| `loot_items` | every item that entered: id, noun, name, type (from `gameobj`), source event, searcher; for a box: opened event, pool drop (room, who, fee, tip); for anything: appraisal, loresong, shop offer, sold (value, to, when, where, bonus) | the eight migrations collapsed into the table |
| `skin_events`, `bundle_events` | as loottracker's | — |
| `transactions` | silver in and out by category (sale, deposit, withdrawal, fee, tip, bounty), linked to the item where there is one | `year/month/day/hour` columns gone: derive from `at` |
| `bounty_rewards` | as loottracker's | — |

Item ids are the game's `exist` ids, which **repeat across sessions**: loottracker's unique
index on `(item_id, item_noun)` (`:403`) is a guard against double-recording within a
session and a hazard across them. The ledger keys an item by `(session, exist id)` and links
by row id.

## 5. Reading: the first reports, and combat's

`Reports` (456 lines) is SQL. The ledger's reader is the first **reading half** over the
character's database, and it is where combat's reports (`cstats`) go too when they come;
`plan/20` §0b's "reading half / sending half" split holds for a database as for a port.

In loottracker's order of use: `summary` (today, month, last N hours), `cap` (the monthly
loot cap, estimated against realised, with the town's racial and trading bonuses,
`TOWN_RACIAL_BONUS` and `calculate_trading_bonus`, `:47-175`), `recent [n] [type]`,
`boxes`, `creatures`, `creature <id>`, `box <id>`, `wands`, `lost`.

## 6. Stages

1. **The classifier** (`cena-model`): the enum and `classify`, tested on the `# Real:` lines
   and the three replay fixtures; the hunt's own pieces first (search, box, skin, bundle,
   bounty), then the town's (sales, appraisals, bank). Ends with every one of loottracker's
   55 patterns accounted for, as a fact or as deliberately dropped with the reason.
2. **The ledger** (`cena-session`): the tables, the worker, the item linking that the
   processors did in Ruby, fed from `publish_combat`'s sibling once per prompt. Ends with a
   session's loot in the database and `plan/12` §7.2's replay producing the same rows twice.
3. **The reader and `;loot`**: `summary`, `recent`, `boxes`, `creatures`, then `cap`.
4. **Combat's reports** on the same reader, after.

## 7. Questions for the author

1. **Cross-character proxy** (`;loottracker proxy`, `:4795`: appraisals sung by another
   character credited to the looter). Keep it, or drop it until multi-session makes it a
   session-to-session fact rather than a setting?
2. **The wand duplication stats** (918): hunt-relevant enough to stay in Stage 1, or town?
3. **`cap`'s bonuses** use race and Trading skill from the character. Hydra has both in
   `state.character`; is the loot-cap formula still `TRUNC((INF_BONUS + TRADING_SKILL_BONUS)
   / 12)`, max 28%, as the script says?
