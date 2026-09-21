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
*Tests: each reaction, from real lines cut from the corpus — **ask the author first**
(`CLAUDE.md`: the archive is 49.55 GB).*

### Stage 3 — the step interpreter (pure)
`Crossing::Steps`: guards asked **when the step is reached**, against a `Walker` refreshed
from the model; the actions, in the order the converter's census says they are used
(`Move`, `Put`, `Pause`, `Cast`, `Await*`, hands, then the long tail). The walk-long
promises: never end with hands stowed, a stance or language changed, or a key out of its
sack. Seeded randomness for `MoveAnyWhile`/`WanderWhile`. `Replan`.

### Stage 4 — the driver, and filling `Walker` from the model
The `async fn`, the authority, cancellation, and `Walker::from(&GameState, &profile)`.
Needs decisions that are the author's: **where the travel profile lives** and what its
defaults are; **where memories persist** (per character, across logins — `plan/21` §4.4);
how Hydra gets `hydra.map` (`plan/21` §6, open). First live walk, author present.

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

## 5. Open — the author's

- The travel profile: where it lives, its defaults (`ice_mode`, `use_urchins`,
  `use_portmasters`, `use_day_pass`, `buy_day_pass`, key and sack names).
- Where memories persist.
- What a walker does about hostiles in the room (a waylaid caravan; any hunting ground on
  the way): stop and say so, or something more.
- Whether Stage 2's lines may be cut from the log archive.
