# 24 — The Travel behavior: the walker

**Status: PROPOSED 2026-09-21.** Claude's staging of work the author has asked for ("I guess
we have the travel behavior to deal with"). The design it stages is `plan/21` §4 — read that
first; this document does not restate it, it orders it.

> **A NOTE ON MILESTONES.** `plan/12` §8 puts the first real behavior at **M6**, after M4
> (a real frontend) and M5 (multi-session). The author has chosen to start Travel now, with
> the map side of `plan/21` just finished. Stages 1–3 below need nothing from M4 or M5: they
> are a pure state machine and its tests. **Stage 4 is where it meets the session**, and is
> the natural place to pause if M4 should come first.

## 1. What exists

| piece | where | state |
|---|---|---|
| the map, and a pathfinder priced per walker | `cena-map` (`Map::routes`, `priced_for`) | built; 3,800/3,800 distances agree with an independent key |
| where am I | `cena-map::locate` | built; 0 wrong over 36,838 rooms |
| every upstream crossing and cost, ported | `cena-mapdb-convert` | **0 unported**, ratchet holds it |
| the walker's facts | `cena_map::Walker` | a plain struct; nothing fills it yet |
| the vocabulary the walker must run | `cena_map::{Action, Cond, Routine}` | 40-odd actions, 27 routines named, **none executed** |
| how a behavior is written here | `cena-behavior::look` | an `async fn` over `SessionHandle`, holding the authority, cancellable at every await |
| what Vellum's walker learned the hard way | `research/mapdb-inventory/vellum-executor-semantics.md` | digested, with line citations |

## 2. The shape — two layers, and why

`plan/21` §4.0 already decides it: **the walk is a pure state machine**, `tick(facts, events)
-> commands`, as Vellum's is (`executor.rs:787`). No sockets, no clock, no sleeps: time and
randomness are passed in. That is what makes `plan/12` §7.2 criterion 7 (deterministic
replay) hold for travel, and it is what makes the walker testable without a game.

Around it sits a thin **driver**: the `async fn travel(handle, cancel, …)` in the mould of
`look`, which claims the authority, feeds the machine frames and model facts, sends what it
asks for through the shared queue, and on *every* exit path — arrival, failure, **user
stop** — runs the machine's cleanup (hands back, stance back, language back, key back).
Vellum skips that on a user stop (`mod:65-68`); `plan/21` §4.0 makes it a requirement.

So nearly all of the work, and all of the risk, is in the pure layer, where it can be
tested by table.

## 3. Stages

Each ends with something that runs and is tested. Rooms-opened order, as the porting was.

### Stage 1 — the trip, over plain exits (pure)
`cena-behavior::travel`: plan with `Map::routes` and `priced_for(&walker)`; walk the path
one exit at a time; **arrival is "the located room is the one expected"**; landing anywhere
else replans; a trip ends `Arrived` or `Failed(why)`. Crossings: `Command` and
`PassThrough` only. Bans live in the trip and are added to the pricing, as
`route.rs` says they would be. *Tests: by table, over the 24-room fixture.*

83,270 of 84,867 exits are this. It is most of the walking and none of the difficulty.

### Stage 2 — move recovery (pure, and the hard part)
`plan/21` §2c: "the hard part is not the primitives; it is move recovery." Lich's `move`
and Vellum's `recover_from_feedback` are the same ~20 failure classes. Here each is a
**stateless classifier over frames** (`plan/12` §3a) — never a second substring scanner —
plus the reaction table in the machine. The **command gate** comes with it, because every
one of its rules is a recorded live bug in Vellum: stamp each send; retry what was *sent*,
never the map's text; attribute outcomes FIFO; an orphan window after abandoning a send;
a `<nav>` beats a failure line that raced it; **never ban on lag** (only if the walker
never left the first room); cap every loop.
*Tests: each reaction, from the lines Lich's `move` already matches (§5): they are the
game's own text.*

> **BUILT 2026-09-21.** The lines are named in `cena-model/src/movement.rs`: Lich's ladder,
> 27 patterns, **in Lich's order** -- which decides ("appears to be closed, perhaps you
> should try again later?" is a shut shop, not a door to open, and only the order says so).
> Ruby's one lookahead is kept by hand. The remedies and budgets are Lich's too
> (`travel/recovery.rs`: `MAX_REMEDIES` 3, `MAX_ROLLS` 20). Vellum's lessons are tests:
> arrival beats a failure line that raced it (lines wait for the tick, so a room change is
> always looked at first); what is sent again is what was *sent*; an orphan window after an
> exit is given up; silence resends, and **bans only from the trip's first room**.
> "You can't go there" is kept as `Trip::wrong_for_the_map()` -- the trip only goes round
> it; whether the map changes is someone else's call.
>
> **Left for stage 3, and said so in the code:** `HandsFull` and the thread-climb's hands
> (emptying and giving back is `plan/21` §4.5), and Sigil of Resolve for `TooInjured`.
> Until then those exits are given up for the trip, not retried blind.

### Stage 3 — the step interpreter (pure)
`Crossing::Steps`: guards asked **when the step is reached**, against a `Walker` refreshed
from the model; the actions, in the order the converter's census says they are used
(`Move`, `Put`, `Pause`, `Cast`, `Await*`, hands, then the long tail). The walk-long
promises: never end with hands stowed, a stance or language changed, or a key out of its
sack. Seeded randomness for `MoveAnyWhile`/`WanderWhile`. `Replan`.

> **BUILT 2026-09-21** (`travel/steps.rs`, `travel/mover.rs`). **A plain exit is the
> one-step list `[move]`**, so there is one way across and move recovery lives inside it.
> Two ways of sending: a *move*, which must change the room and has Lich's ladder behind it;
> and an *exchange*, a command and the game's answer (the next prompt, a room change, or
> three seconds), which waits out `...wait N` and is otherwise not second-guessed. **Every
> loop in the vocabulary is exchanges with a question between them** -- until the room
> changes, until there, while a fact holds, until the game says so -- so ten actions are
> one mechanism. Loops stop at 50 turns (upstream's own bound where it has one).
>
> What the trip cannot spell without facts it should not hold is a **`Deed`** for the
> driver: hands, stance, casting, memories, waiting for followers.
>
> Three things found by building it, each now a test:
> - **What one crossing owes, the next inherits.** Upstream climbs a ledge with empty hands
>   and fills them at the top of the *next* climb.
> - **A crossing is finished before the trip says it has arrived.** Otherwise the door at
>   the journey's end is never closed and locked behind the walker.
> - **Whatever ends the trip, what it changed is put back first**, and `Trip::owed()` gives
>   a driver the same list for a user's stop -- the case Vellum skipped.
>
> Stage 2's two deferred remedies are in: full hands are emptied for the move and given
> back when it lands; Sigil of Resolve is cast for a walker too injured to climb, if known.
>
> **Not run yet, and priced shut so the pathfinder goes round them** (18 of 2,915 steps
> exits): `speak`/`restore_speech` (2), `take_out`/`put_back` (2), `ask` (1),
> `order_by_name` (4), and commands carrying `{item:…}` (9). They need the game's replies
> parsed or an item looked up by name -- stage 4 facts.

### Stage 4 — the driver, and filling `Walker` from the model
The `async fn`, the authority, cancellation, and `Walker::from(&GameState, &profile)`.
Needs decisions that are the author's: **where the travel profile lives** and what its
defaults are; **where memories persist** (per character, across logins — `plan/21` §4.4);
how Hydra gets `hydra.map` (`plan/21` §6, open). First live walk, author present.

> **IN PROGRESS 2026-09-21. Three pieces; the first is built.**
>
> **4a, built -- `travel/facts.rs`: `walker_from(state, notes, now_server)`.** Pure, so it is
> tested without a game. **Unknown stays unknown**: a status the game never reported is not
> `false`, and no effect ever listed is "not told", not "nothing is up". Filled: settings
> and memories (the travel file), profession, race, gender, level (the number in the
> model's verbatim `Level 100`), posture and the `stunned`/`hidden`/`invisible` flags,
> exits, what the room shows, encumbrance, and active spells at the game's clock.
> **Not yet**, tabled in the file: skills and known/affordable spells (need the skill and
> spell tables joined), society and citizenship (the model does not hold them), worn items,
> and the flags pre-flight works out. Not-yet is safe and not free: an exit priced on an
> unknown fact is impassable, so the walker goes round it.
>
> **A FINDING FOR 4c: a behavior has no live read of `GameState`.** `look` and `sync` never
> needed one. What a subscriber gets is `(Snapshot, Receiver<Event>)` -- the state once,
> then frames. So the driver keeps its **own** `GameState`, folds each `Event::Frame` into
> it with `GameState::apply`, and hands frame text to `Trip::heard` and prompts to
> `Trip::prompted`. That is the same thing a frontend does, and needs no new session API.
>
> **4b, next -- the travel file**, `<instance>_<character>.travel.json`, in `cena-session`
> beside `character_store.rs` (the only crate that may hold model types and touch the
> filesystem, `layering.rs`). **4c -- the `async fn`**: claim the authority, locate the
> room (`cena_map::locate` from the model's room), tick, send, do the deeds, and give back
> what is `owed()` on every way out including a stop.

> **4b, BUILT 2026-09-21** -- `cena-session/src/travel_store.rs`. The snapshot's rule is
> refuse-and-resync; this file's is the opposite, for the opposite reason: a missing file is
> an empty one, and a file that cannot be read is refused **and left alone**, because a
> person can mend a file and cannot mend a lost memory.
>
> **4c IS BLOCKED ON ONE RULING -- two rules contradict, and `plan/12` is authoritative.**
>
> - `plan/12` §4.3: a stopped behavior "may run cleanup, but **cleanup cannot send
>   commands** -- it releases resources only. Otherwise 'stop' becomes 'send more'."
> - `plan/21` §4.0 (a proposal, and mine): never end a trip with items still stowed,
>   "**including on a user stop**, which Vellum skips."
>
> Giving a shield back *is* sending commands (`get #id`). Both cannot hold. Until ruled,
> the driver follows `plan/12`: on a stop it sends nothing, and **returns what is owed**
> (`Trip::owed()`) so whoever stopped it can say "your shield is in your cloak" -- and the
> player, or a follow-up action they choose, gives it back. On arrival and on failure the
> trip still puts things back itself, since those are not preemption.
>
> Also settled by reading, not asked: a move is sent with `send_now` -- "fire and verify
> later" (`verdict.rs`, author 2026-09-18) -- which is exactly the trip's shape, since it
> verifies by the room changing. Casting follows Lich (`spell.rb:686-780`): untargeted is
> `incant N`; at a target it is `prepare N` then `cast <target>`; a society power is its
> own name as a verb (`sigil of resolve`). And the integration test needs a **scripted
> source** -- one whose reply depends on the command -- which `cena-platform`'s
> `AnsweringSource` (one fixed reply) is not.

> **4c, BUILT 2026-09-21** -- `travel/drive.rs` (the `async fn`), `travel/hands.rs` (the
> commands for the deeds, pure), `tests/travel_drive.rs`.
>
> **THE RULING** (author, 2026-09-21), which settles the contradiction above: *"yeah cleanup
> cannot send commands, I like don't stop with items stowed ... I would say one get and then
> stops, because you may not even be able to hold a shield with where you are."* So a stop
> sends **one command per stored item and nothing else**: no retry, no waiting to see, no
> stance. What was not seen to come back is returned (`Travelled::still_stored`) for a
> frontend to say. A dead or disconnected session sends nothing.
>
> **The word is *stored*, not *stowed*** (author): it is the opposite of *ready*, and the
> game already knows where a readied thing goes -- sheath, container, or **worn**. So the
> hands are emptied with `store right` / `store left`, which honours `store set`, and a
> thing is taken back with `remove #id` when the ready list says its slot is worn when
> stored and `get #id` otherwise -- Lich's own split (`stash.rb:173-193`), without its
> polling. Both verbs are in upstream scripts:
> `grep -rhoE "(stow|store) (right|left)" reference/scripts`.
>
> **The scripted source is `AnsweringSource::answer(command, reply)`**, not a fourth
> `ByteSource`: a reply that answers one named command, once. Everything written before it
> reads as it did.
>
> **CORRECTED the same day, twice, by the author.**
>
> *Only an armament can be stored* -- a weapon, a runestaff, a shield. So `store` is for
> those (it sends each where `store set` says) and **`stow` is for anything else** in a
> hand: a gem, a gift, a lockpick. `hands::is_armament` asks the type table for `weapon`
> (which has `runestaff`) and takes Lich's shield nouns from `stash.rb:173`.
>
> *"is the driver supposed to be rebuilding lines or is that someone else's job? ... the
> lines have to be rebuilt for other folks too, like the frontend display."* **It is the
> model's job, and the model already does it**: `route_text` reassembles once, and both the
> frontend's stream buffers and every classifier's chunk come from that one reassembly. The
> first driver joined `ends_line` pieces itself -- a second reader waiting to disagree with
> the first. It now reads `GameState::open_chunk()`. No test depended on a heard line until
> this was asked, so one was added (`what_the_game_says_reaches_the_trip`) and a mutation
> that deafens the trip turns it, and only it, red.
>
> **Two things the build found.** A line of game text arrives in pieces, one per link
> boundary (`TextFrame::ends_line`), and Lich's ladder is over whole lines -- see the
> correction above for whose job joining them is. And a session on its way up
> announces `Connecting`, `Authenticating`, `Syncing`: only `Reconnecting` and `Closed`
> mean the transport is gone -- the first version stopped every walk before it began.
>
> **A mutation that survived, and why.** Removing the stop race from the hold passed the
> 250ms assertion, because an un-raced hold still ends at its next beat and `BEAT` *is*
> `PREEMPT_GRACE`. The budget could not see the defect. In virtual time a raced stop
> crosses no timer at all, so the test now asserts the stop took **zero**. The other
> mutation (the stop asks twice) was caught as written.
>
> **4c's tail, BUILT 2026-09-21.** The walker's "not yet" facts are filled
> (`travel/facts.rs`, `travel/knows.rs`), each from a source and none guessed:
>
> | fact | from |
> |---|---|
> | `skills` | `character.skills`: the 46 by display name, lowercased, and the circles by the names `skills` prints (Lich's `SPELL_CIRCLE_INDEX_TO_NAME`, `armaments.rb:54-72`) |
> | `known_spells` | Lich's `Spell#known?` (`spell.rb:464-522`): circle ranks **capped at level**; 97/98/99 by rank in *that* society; 1700 to the five pures |
> | `affordable_spells` | `Spell#affordable?` (`:591-611`): mana, stamina, and spirit **with one to spare** |
> | `society`, `society_rank`, `citizenship` | `character.standing` -- which the model already held; `facts.rs` said it did not, and was wrong |
> | `worn`, `worn_nouns` | the inventory snapshot: `worn` on the `player`, named without the article, as `GameObj` does |
>
> Not ported from `affordable?`, and written down in `knows.rs` rather than guessed: the
> Council of Light signs that raise the spirit to spare, Mental Acuity, and `Overexerted`.
> Each makes a spell *less* affordable than this says, so the cost is a cast the game
> refuses, which the trip already recovers from.
>
> **Locating now offers the description and the exits line** as well as the number and the
> title (`Runs::plain`), so rooms that share a number are told apart.
>
> **A MUTATION THAT HUNG THE SUITE FOUND A REAL DEFECT.** Withholding the description left
> the walker unable to name its room, and the trip then held **for ever**: `Trip::tick`
> answers `Hold` to an unknown room and nothing bounded it. `plan/12` section 5.5 gives
> every wait a deadline, so the driver now ends the trip as `OffTheMap` after `LOST_WAIT`
> (10s -- a title arrives a moment after its number, so some of that wait is ordinary).
>
> **THREE OF FOUR MUTATIONS SURVIVED FIRST TIME, ALL THE SAME WAY** -- the input never
> reached the distinction (`CLAUDE.md`, "a test can pass a mutation because the input never
> reaches the code"; now eight). The other society's power was *above* the rank as well, so
> rank refused it first. The carried key was inside the cloak, so `on_person()` dropped it
> before the `worn` filter was asked. And no test cast anything that costs spirit. All four
> are caught now, each by the test meant for it.
>
> **Still not yet:** `AwaitFollowers` waits until every group member is in `room players`
> or 30s, which is a reading of upstream's "joins your group" loop and not a port of it;
> `Trip::seeded` is not fed a recorded seed, because the session does not record one yet;
> and the planner's own flags (`urchin_access`, `day_pass:...`) are stage 5.

### Stage 5 — pre-flight, and the stack of trips
What must be known before pricing (`plan/21` §4.0: *a cost never acts*): urchin status,
the day-pass sack scan. And trips that start trips: the silver detour, the five errands.
A trip must be able to **stop and say why** (the vaalorn door's gem, the bridge wheel's
helper) and to say what it met (a waylaid caravan's bandits).

### Stage 6 — the routines, one at a time
27 named, each written from the script it is pinned to in
`cena-mapdb-convert/src/upstream_scripts/`. Order by rooms opened
(`research/mapdb-inventory/chokepoints.py` over the routine exits): expected first are
`Trinket`, `Confluence`, `Seeking`, `MinotaurMaze`, `Patrol`, `Signposts`; the one-room
puzzles last.

## 4. Crate graph

`cena-behavior` gains an edge to **`cena-map`**. It is downward (`cena-map` depends on
nothing) and is recorded in `cena-arch-tests/tests/layering.rs` when it is added, per
`plan/05` §0. The pure layer uses `cena-map` and `cena-model`'s frame types through
`cena-session`'s re-exports, as `look` does; it does not touch `SessionHandle`.

## 5. Decided — author, 2026-09-21

- **Stage 2's lines come from Lich's `move`** (`reference/lich-5/lib/.../move.rb`), not from
  the log archive. They are the game's own text, collected over years. (`CLAUDE.md` already
  says the protocol facts are in Lich; asking to cut them from the corpus was the wrong
  question.)
- **Hostiles.** A Travel across rooms **ignores creatures**. A *hunt* moving to its next room
  hunts them -- which is the hunting behaviour's business, not the walker's. So the walker
  never stops for a creature, and a waylaid caravan just plans again.
- **Memories go with the character, in a file of their own** (author): 
  `<instance>_<character>.travel.json`, beside the snapshot, holding memories and the travel
  profile, so a snapshot rewrite can never clobber them.

## 6. The travel profile — audited against go2, 2026-09-21

Every setting `go2.lic` reads (`grep UserVars\.|CharSettings\[`), against every setting,
flag and memory the converted map asks about (a census of the converter's output).

**One was missing, and it mattered: the Vaalor shortcut.** The map file prices the rocky
trail 16745<->16746 at a flat 15 s, but `go2.lic:952` *rewrites those two `timeto` values in
memory at startup* -- to `nil`, impassable, unless `vaalor shortcut` is on, and off is the
default (`:854`). The converted map left it open to everyone. Fixed in the converter
(`src/go2.rs`): both directions are gated on `vaalor_shortcut`.

| kind | names | who owns it |
|---|---|---|
| **asked by the map** (a `Cond::Setting`) | `use_urchins`, `use_portmasters`, `use_seeking`, `ice_mode` (`run`/`wait`/`auto`), `use_day_pass`, `buy_day_pass`, `car_to_sos`, `car_from_sos`, **`vaalor_shortcut`**, `premium`, `allow_vornavis`, `annoy_ylandra` | the travel profile |
| **named things** the map's commands fill in | `fwi_trinket`, `rogue_password`, `day_pass_sack`, `key`, `key_sack` / `keysack` (upstream spells it both ways), and one per private property: `mularos_lover`, `peregrine`, `prestidigitorium`, `black_swan`, `sunset_cabin`, `journeys_end`, `safe_harbor`, `spindrift` | the travel profile |
| **go2's own**, the walker's to implement | `get silvers`, `get return trip silvers` (stage 5, the silver detour); `use_gigas_hwtravel` + `gigas_min_number` (Hinterwilds travel by fragments -- go2 checks `wealth gigas`); `stop for dead`; `delay`, `typeahead` (pacing) | the travel profile, read by the walker |
| **not settings at all** | `urchins_expire` (a fact the planner probes: the flag `urchin_access`), `go2_start_room`, `hinterwilds_location` (a memory) | model / memories |
| **not needed** | `hide_room_titles`, `hide_room_descriptions`, `echo_input`, `confirm_distance`, `disable_confirm` (Lich UI); `use_portals`, `use_old_portals`, `have_portal_pass` (Platinum/Shattered; **0** uses in this mapdb) | -- |

Flags the map asks of the planner: `urchin_access`, `hidden`, `invisible`, `mounted`,
`stunned`, `premium_account`, `platinum`, `hunting`, `own_disk_here`, `leading_group`,
`day_pass:<towns>`. Memories: `duskruin_origin`, `talondown_origin`, `ebon_gate_origin`,
`marksofthebeast_origin`, `fwi_return_room`, `silverwood_town`, `redforest_location`,
`hinterwilds_location`, and the walker's own scratch note `key_was_worn`.
