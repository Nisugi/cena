# 21 — The mapdb: porting the map and its scripted edges

**Status: PROPOSAL (Claude, 2026-09-20).** Direction set by the author in conversation the same
day; the details below are not yet approved. `plan/12` wins any contradiction.

Author decisions this document rests on (2026-09-20):

- Scripted edges become **ported Rust**, not transpiled Ruby. *"I would prefer to port
  everything than transpile."*
- They need not be behaviors. *"Can just be methods that are called in place."*
- Replacements are **primitives the map composes**, so Hydra needs a release only for a new
  primitive. *"We build our string proc replacements as primitives, and the map builds the
  route using those primitives."*
- Special cases (urchins and the like) are **figured out beforehand**, not discovered mid-port.
- **Both room numbers are kept** (§3d): Lich `id` names graph nodes, Simu `uid` identifies
  where you are. *"I'm down with keeping both ids."*
- Upstream updates are absorbed by the porting system itself. *"Once we have a system for
  porting down, porting in updates won't be an issue."*

---

## 1. What the mapdb is — MEASURED

Source: `reference/mapdb/map-1789942730.json` (43 MB). Every number below was produced on
2026-09-20 by a Python profile of that file (scratchpad `shapes.py`; the normaliser replaces
string literals with `S`, regex literals with `R`, digit runs with `N`). **Re-measure, do not
restate** (`05` §−2) — step 2 turns these into a test so they cannot drift.

| Fact | Value |
|---|---|
| Rooms | 36,838 (max id 36,863) |
| `wayto` edges | 84,867 — 57,631 single-word, 19,313 multi-word command, **7,923 `;e` Ruby** |
| `timeto` | 82,945 float, 108 int, **1,873 `;e` Ruby** |
| Distinct `wayto` proc shapes | **389**; top 25 = 6,655 (84%), top 100 = 7,538 (95%) |
| Distinct `timeto` proc shapes | 68; 957 are delegation (`Map[N].timeto[S].call`) |
| `uid` | present on 28,962 rooms (28,681 have one; up to 9); **absent on 7,876** |
| `image` / `image_coords` | 21,711 / 21,838 rooms; 280 light + 268 dark image files |
| `tags` | 2,713 distinct, forage-dominated (`meta:forage-sensed` 18,651) |
| `location` | 35,230 string, **661 `false`**; `check_location` on 521 |
| Longest proc | 5,667 chars; 1,225 procs over 500 |

Largest `wayto` shapes: Confluence delegation 3,233 (2,756 + 477) · `;e true` 523 · inn tables
478 · `move S; waitrt?` 457 · minotaur maze ~497 across three array sizes · `start_room` guided
routes ≥385 · pedal loop 161 · ice mode 150 · `checkspell` branch 113 · bare `move` 102.

## 2. What VellumFE did — READ FIRST, per `CLAUDE.md`

VERIFIED by reading `reference/VellumFE` on 2026-09-20. Not read in full: `travel/executor.rs`
(6,654 lines), `app_core/state/travel_ticks.rs`.

- `src/core/pathing/transpile.rs` — one regex per Ruby idiom, each commented with its corpus
  count, producing `WalkAction` values (`pathing/edge.rs`).
- The big cases are **already native ports**, not IR: `travel/confluence.rs` ("mirrors the
  Ruby, line for line"), `travel/minotaur.rs`, `travel/day_pass_buy.rs`, `travel/stash.rs`,
  and the `VolnSeeking` / `TrinketWarp` / `GuidedRoute` / `RouteTable` variants.
- `timeto` has its own evaluator: delegation resolves through the referenced entry; gates
  (portmaster, seeking, urchins, day pass) are flags.
- Urchins are gated **in Dijkstra** (`pathing/dijkstra.rs:76-80`): hideout-touching edges are
  excluded unless a validity flag is set. A trip issued while validity is unknown is deferred
  behind an `urchin status` pre-flight (`travel_ticks.rs:12-20`).
- `defaults/globals/travel_overrides.toml` hand-writes edges keyed by `from`/`to` when the Ruby
  is an errand (River's Rest boot: ~100 lines of Ruby, two commands once you hold the amulet).
  `defaults/globals/mazes.toml` covers regions whose mapdb edges are junk.
- `tests/pathing.rs:83` is a **coverage ratchet** over the real mapdb: >95% of proc edges
  walkable, >90% of the graph routable, residue printed clustered by shape.

**What Cena takes:** the idiom catalogue with its counts, the native ports, the urchin gate
placement, the override file, and the ratchet.

**What Cena does not take:**

- **Nesting** in the IR (`If { then, els }`, `Repeat { body }`, `Break`). Cena *does* take the
  primitive vocabulary itself (`Move`, `Put`, `WaitRt`, `Await`, `EmptyHands`, `GuidedRoute`,
  `RouteTable`, …) — see §3a. What it leaves behind is steps containing steps, which is where
  data becomes a language with an interpreter (settled: no DSL).
- `set_urchins_valid` as a **process global** (`transpile.rs:2771`). Its tests need a lock
  because of it. With 3–25 characters in one process it is simply wrong: validity is per
  character.

## 2b. Inventory — MEASURED 2026-09-20

Evidence is committed under `research/mapdb-inventory/`: every shape with its count
(`shapes_wayto.tsv`, `shapes_timeto.tsv`, produced by `inv.py`), Vellum's own coverage report
against today's mapdb (`vellum_coverage.txt`), its verbatim residue (`vellum_residue.txt`), and
primitive usage per edge (`vellum_primitives.txt`, from a throwaway test since deleted from the
reference clone).

**Shapes.** `wayto`: 7,923 procs in **367 shapes** (array literals collapsed; §1's 389 did
not collapse them). Top 10 = 74.8%, top 50 = 92.3%, top 100 = 95.5%; 185 shapes occur once.
`timeto`: 1,873 procs in **68 shapes**; top 10 = 91.8%.

**Vellum against today's file.** `VELLUM_LICH_GAME_DIR=E:/Cena/reference/mapdb cargo test
--test pathing real_mapdb_coverage -- --ignored --nocapture`:

| | |
|---|---|
| Scripted edges it can cross | **7,718 / 7,865 = 98.1%** |
| — transpiled to primitives | 4,476 |
| — Confluence explorer (native) | 3,234 |
| — day pass / override | 6 / 2 |
| Residue | **147 edges in 85 families** |
| Graph routable by `timeto` | 80,620 / 81,965 = 98.4% |

Vellum sees 81,965 edges, this document's §1 sees 84,867. INFERRED cause: Vellum iterates
rooms by `location`, and 661 rooms have `location: false` and ~950 have none. Reconcile in
step 2 — those rooms must not silently vanish from Hydra's map.

**Which primitives carry the load** (edges using each, of the 4,476): `Move` 2,599 · `WaitRt`
750 · `GuidedRoute` 570 · `Put` 567 · `Await` 551 · `Noop` 523 · `MinotaurMaze` 497 · `If` 370
· `Sleep` 119 · `Repeat` 112 · `FillHands` 106 · `EmptyHands` 101 · `RouteTable` 63 · `SetVar`
47 · `StepMove` 42 · `TryMove` 42 · `Replan` 37 · `VolnSeeking` 36 · `TrinketWarp` 28 · `Break`
15 · `PauseForUser` 15 · `MoveExitExcept` 3 · `MoveAnyExit` 2. **23 primitives.**

**Flat vs nested.** 3,504 flat, **972 nested** (21.7%). The nesting is concentrated: 478 are
`Await` with an `if_match` follow-up (the inn tables), 257 `If+Move`, 57 `If+Move+Put`; every
other nested combination is ≤16 edges. UNMEASURED: how many `If`s nest a second level.

**Urchins are data, not a script.** Room 30714 is a virtual hub. 522 rooms enter it by
`;e true` (that is what the 523 `Noop` edges are), each delegating its cost to one gate proc
on `7→30714` (`use_urchins ∧ not expired ∧ ¬hidden ∧ ¬invisible → 0.1`). 408 edges leave it by
the **plain command** `urchin guide <place>`, gated on "go2 is running". So 930 of the 957
`timeto` delegations are urchins, and the whole feature is: one per-character cost gate, one
pre-flight to learn the expiry, zero scripted crossings.

**What Vellum got right — take it:**
- The 23-primitive vocabulary, proven at 98.1% on a mapdb it was not tuned against.
- Native ports for what is an algorithm (Confluence = 41% of all scripted edges; minotaur).
- `Await` as the workhorse, with a *passive* form for non-idempotent commands, named captures
  only, and `OnTimeout` defaulting to `Continue`.
- Loop bounds clamped by the interpreter, never trusted from data.
- `PauseForUser` abandons rather than blocks; `require_item` overrides for errands.
- The coverage test reporting by mechanism and clustering residue by family.

**What was shaped by constraints Hydra does not have:**
- **Recognition runs at runtime, in the client**, so it is 137 regexes plus a hand-rolled Ruby
  statement splitter (`split_statements`, `parse_units`, `split_modifier`,
  `statement_condition`) — 4,447 lines. Hydra converts **offline**, where a real Ruby parser
  is available. Recognise on the AST, not on text.
- **Gates are process globals** (`set_urchins_valid`, `set_use_portmasters`, `set_use_seeking`,
  `set_mapdb_var`, `set_day_pass_routable`) — one character per process. Hydra's are
  per-session model state.
- **"Unknown takes `els`."** Its `Cond` is a closed set of what that client tracked; race was
  "unknowable client-side", so the kneel guard ignores it. Hydra's model has race, profession,
  level, skills, spells known/active/affordable, society rank, citizenship, encumbrance,
  inventory. The residue's largest family (10×, `wait_until { Spell[N].affordable? }; cast`)
  is a constraint of that kind, not a hard problem.
- **"The Lich fallback handles it."** Vellum could leave residue to Lich. Hydra has no
  fallback: residue is simply unwalkable, so the tail matters more here.

**Still needed:**
1. The residue: 147 edges / 85 families. Largest: spell-cast-until-unhindered 10, child/
   official escort 9, disk wait 6, `next_exit` table 5, celerity-search 4+3, group-follow 4,
   random-walk-until-exits 4+4+2+2+2, Koar's Shrine stair read 3.
2. The 68 `timeto` shapes as a cost-gate enum (§3b). Families: delegation 957; instability
   477; sitting/climate 103; portmasters 62; seeking 36; profession ~70; per-event origin
   variables ~40; month, premium, race, gender, level, perception, citizenship, held key.
3. Per-trip scratch variables (`SetVar`, 47 edges) and what reads them in `timeto`.
4. The stash service behind `EmptyHands`/`FillHands` (207 edges).
5. `go2.lic` itself — targets, silver check, Hinterwilds, mounted — NOT YET READ.
6. Executor semantics — `travel/executor.rs` (6,654 lines) NOT YET READ. The primitives'
   meaning lives there, not in the enum.
7. The 2,902-edge discrepancy above.

## 2c. What the executor and go2 taught — READ 2026-09-20

Digests, with citations: `research/mapdb-inventory/vellum-executor-semantics.md`,
`go2-and-lich-map-semantics.md` (+ `-part2.md`), and `vars.tsv` (every variable the mapdb
procs read or write, measured). This closes "still needed" items 5 and 6 above.

**The vocabulary is smaller than it looks.** `Move` and `Put` are *identical* at runtime —
both fire-and-forget, neither RT-gated; the real paced move is `StepMove`. Seven of the 23
variants are **macros that lower to the others** at walk time (`GuidedRoute`, `VolnSeeking`,
`TrinketWarp`, `TryMove`, `RouteTable` lower to `Repeat`/`StepMove`/`If`/`Await`). So there
are two layers: a small core (send, paced move, wait RT, sleep, await line, stash hands,
replan, set var) and named routines built from it in Rust. **That is the answer to the
flat-vs-nested question (§3a): nesting belongs inside Rust routines, not in map data.**

**The hard part is not the primitives; it is move recovery.** Lich's `move` (`move.rb`
167-447) and Vellum's `recover_from_feedback` are the same ~20 failure classes with
remedies: can't-go (ban), denied (retry, keep edge), hidden -> `unhide`, closed -> `open`
once, go<->climb swap, hands full -> stash, `stow feet`, RT wait, type-ahead, stunned,
fell. Every one is a **classifier over frames** in Cena (`12` §3a) — Vellum needed a second
substring scanner only because its executor never saw lines. Sources disagree once: pitch
dark is *success* in Lich, *repath* in Vellum. Lich is the reference for what the wire means.

**The command gate is the design to port, not just the patterns.** Stamp each send; retry
re-emits what was *sent*, never the wayto; account outcomes FIFO so a stale failure cannot
re-trigger the current move; an orphan window after abandoning a send; arrival beats a raced
failure line; **never ban on lag** (only if the walker never left the first room). Each of
these is a recorded live bug in Vellum (`vellum-executor-semantics.md` §9). Cena's shared
command queue is where this lives.

**The pathfinder takes its context as a value.** Every Vellum gate is a process global
because `resolve_timeto` is context-free and called from Dijkstra. Cena:
`route(graph, from, to, &TravelContext)`, context built from one session's model.

**go2 holds almost none of the special cases — the mapdb does.** go2 only sets variables.
What go2 itself implements, and Cena must too: silver (route cost from `silver-cost:` tags —
88 of them, and a non-numeric value is *Ruby* too; nearest *affordable* bank; per-town
accounts; no deposit), Hinterwilds gigas travel, the confluence/instability search, the
urchin expiry query, playershop escape, and the lag probe (`help lag-check` as a round-trip
barrier).

**Room identification is a real algorithm**, not a lookup (`map_gs.rb` 165-359): uid ->
if several rooms share it, disambiguate by **adjacency to the previous room** -> else text
match in two passes (exact; then any one *sentence* of the description), honouring
`unique_loot`, fogged exits, `random-paths`, and `check_location`, which **sends the
`location` verb**. `location: false` means that verb failed when the room was mapped. Peer
tags **send `peer <dir>`** to tell twins apart. Identification can therefore issue commands —
it needs the queue, and it is not pure.

**Measured from the mapdb (`vars.tsv`):** `$go2_restart` is set by 674 edges (replan is
common, not exotic). Per-event origin variables are written by an entry edge and read by
`timeto` (Duskruin 24/24, Ebon Gate 12/26, Talondown 18/18). 7 edges write `$restart_go2` —
an upstream typo that silently does nothing; the converter should flag it. Tags carry
routing meaning too: `meta:transport` 12, `meta:nomagic` 667, `meta:teleport` 247,
`meta:premium` 123, `meta:citizenship` 43.

**Do not copy** (bug-shaped, listed in the digest): `Await` retry re-sends the unexpanded
command; `Break` targets the next loop forward, not the enclosing one; `MoveAnyExit` uses
the thread RNG (breaks deterministic replay — seed it per session); user stop skips
retrieving stowed items; `PauseForUser` ignores its timeout.

## 2d. The residue and the costs — is the primitive set closed? MEASURED 2026-09-20

Evidence: `research/mapdb-inventory/residue_families.tsv` (from `residue.py`) and `costs.py`.
The family classification below is by reading all 93 families — INFERRED, not mechanical.

### The residue: 147 edges Vellum cannot cross, 93 families

(Vellum's own report says 85; its key truncates at 160 characters and merges a few.) 35
families are over 500 characters, 8 over 1,500; the longest is 5,667.

| Class | Edges | Families | What it needs |
|---|---|---|---|
| **Recogniser misses** — already expressible with Vellum's primitives | ~44 | ~34 | nothing new. `wait_while { Room.current.id == N }` (4), `begin move … end until Room == N` (4), edge delegation (4), ferry arrival `get until /gangplank/; fput "out"` (2), try-move, fixed move/put sequences. Text regexes missed the spelling; an AST recogniser would not. |
| **Spell use** | ~25 | ~10 | one new primitive `Cast { spell, target?, until_unhindered }` and conditions `SpellKnown` / `SpellAffordable`. Largest family in the residue: `wait_until affordable; cast(704, …); break unless /Spell Hindrance/` ×10; disk-wait ×6; celerity-then-search ×9. |
| **Random walk until a condition** | ~16 | 7 | `WanderUntil { choices, until }` — pick among *listed* exits until the compass or an object changes. Vellum's `MoveAnyExit` is the unparameterised case. **Seed the RNG per session** (replay). |
| **Escort-aware moves** | ~11 | 3 | a routine: on a child/official escort bounty, move and wait for the NPC to follow. The same logic sits inside the 497 minotaur edges. |
| **Route table with per-room direction *lists*** | 5 | 1 | widen `RouteTable`. |
| **Stance save/restore around a climb** | 3 | 2 | `WithStance { stance, then }` — or a guard. |
| **Race condition** | 1 | 1 | `Cond::Race`. Vellum called race "unknowable client-side"; Cena's model has it. |
| **Group follow-check** | 4 | 4 | a routine reading "X followed." |
| **User-configured command lists** (`UserVars.Peregrine.each { fput }`) | 4 | 2 | a data-profile setting: a named list of commands. |
| **Read-and-decide puzzles** | ~18 | ~15 | one named Rust routine each: Koar's Shrine stairs (3), the mirror, the wedge sigils, the leaf statue, the pillar (3,344 chars), element orbs, altar grid, the mural (5,667). |
| **Errands** | ~15 | ~12 | override with `require_item`, or a routine: travel-office `inquire`/`order N` (4), ticket hunts (3,451 and 3,763 chars), key-in-sack doors, the 5,632-char sword errand. |

**Answer: the primitive set is closed; the routine set is open but tiny.** About 2/3 of the
residue needs **four additions** — `Cast`, `WanderUntil`, a wider `RouteTable`, `WithStance` —
plus three conditions (`SpellKnown`, `SpellAffordable`, `Race`). The remaining ~50 edges are
one-off puzzles and errands: each becomes a **named routine** (Rust, like `Confluence`) or
stays a `puzzle` edge that draws but does not route. New upstream shapes will almost always
land in the first row — a converter change, no release.

### The costs: 1,873 scripted `timeto`, 21 gate kinds, **0 unclassified**

| Edges | Gate | Reads |
|---|---|---|
| 959 | delegate to another edge's cost | — (930 of these are urchins, §2b) |
| 477 | instability table | per-trip table set by the confluence search |
| 103 | posture + climate | sitting, room climate — picks one of two constants |
| 83 | user setting | portmasters 62, premium 9, FWI trinket, caravans, a few personal ones |
| 66 | profession | |
| 58 | trip variable | event origin / return room, set by an entry edge (§2c) |
| 36 | Voln seeking | society, rank 26, setting |
| 27 | spell / skill cost *formula* | the only arithmetic: `Skills.climbing >= max(encumbrance/1.25, 12)` |
| 12 | calendar month | Ebon Gate: October only |
| 8 / 8 | carries item / another script running | the second has no meaning in Hydra — treat as "not running" |
| 7 / 6 / 4 / 3 / 2 / 2 | citizenship / skill rank / race / society / level / gender | |
| 6 | day pass | the only costs with side effects (they open containers) — must become a pre-flight |
| 4 | global flag | e.g. `$SILVERWOOD_TOWN` |
| 1 + 1 | the urchin gate and its exit gate | |

Every kind is a **predicate over one character's state plus trip state, returning a constant
or "impassable"**. The ~27 formula costs are the exception and want a small closed set of
named formulas, not an expression evaluator. Wall-clock (`month`) must come in through the
routing context, not be read inside the pathfinder (deterministic replay).

## 3. The design

### 3a. Convert offline, then call ported code

Decided with the author 2026-09-20, after reading <https://github.com/Nisugi/cartographer>
(the author's own tool: splits the mapdb into `rooms/{id}/room.json` +
`stringprocs/wayto/room-X-to-Y.rb` with checksums; its Phase 2, `Cartographer.call(:table,
"Cat's Paw")`, is the named-parameterised-crossing idea below, in Ruby).

**The pipeline:**

```
upstream map-*.json ──► tools/mapdb-convert ──► rooms/{id}.json (committed, diffable)
                              │                        │
                              ▼                        ▼ workflow
                       residue report            hydra map binary ──► fetched by Hydra
```

- The upstream JSON is **input only**. Hydra never reads it and **no Ruby string reaches the
  binary**. Not TSV: crossings carry structured parameters, and TSV would make them strings
  to parse again.
- The converter recognises each `;e` edge **by shape**, extracts parameters, and writes a named
  crossing as **a flat list of primitive steps**:
  `[{ "await": {"cmd": "go Cat's Paw table", "pattern": "…", "timeout": 25} }, …]`. Unmatched
  becomes `[{ "unported": "<shape hash>" }]` and is listed in the residue report.
  **Nothing is dropped silently.**
- Shape-keyed, not `from`/`to`-keyed: survives upstream renumbering. `from`/`to` overrides
  exist only for the errand class (River's Rest).

**What updates without a Hydra release:** rooms, exits, costs, tags, uids, images, and any new
edge — **including a never-seen Ruby shape** — that can be written with existing primitives; that
is a converter change only. **What does not:** a shape needing a *new primitive*, which is Rust
code. That edge stays impassable until the release that ports it; the rest of the map
is unaffected.

**Format rules this imposes:**

1. **Unknown primitives must be survivable.** A newer map will meet an older Hydra. Steps
   are stored as *name + parameter blob*, never as a Rust enum discriminant; an unknown name
   loads as impassable. (So: not bincode/postcard *of the enum*.)
2. **A version header**, so a breaking layout change is refused loudly.
3. **One vocabulary.** The converter takes its primitive names from the crate Hydra uses; a test
   asserts every emittable name has a ported function.

**The line between data and a language** (PROPOSED — author to rule; §2c now supports it:
Vellum's own nesting is mostly macros lowering inside the executor):

- A crossing is a **flat sequence** of steps. Steps never contain steps.
- A condition is a **guard on a step**, from a closed list (`sitting`, `kneeling`, `standing`,
  `spell_active(n)`, `hands_full`, …): `{ "put": "stand", "unless": "standing" }`.
- **Loops are primitives, not constructs**: `repeat_until_room_changes { cmd, max }`,
  `guided_route { rooms, dirs, landmarks }`, `route_table { … }`. The iteration and its clamp
  live in Rust; bad map data may waste a route, never hang the client.
- **Algorithms are single primitives**: `confluence`, `minotaur_maze`, `voln_seeking`,
  `trinket_warp`.
- A shape that cannot be written this way needs a new primitive — that, and only that, is a
  Hydra release.

```rust
pub struct Step { pub action: Primitive, pub guard: Option<Guard> }
pub enum Primitive { Noop, Move(String), Put(String), WaitRt, Sleep(f32), Await { .. },
    EmptyHands, FillHands, RepeatUntilRoomChanges { .. }, GuidedRoute { .. },
    Confluence { .. }, MinotaurMaze { .. }, Replan, /* … */ Unknown(String) }
```

Execution is one `match` over `Primitive`, arms hand-written Rust. No trait (`05` §−1).
**UNMEASURED:** how many of the 389 shapes this vocabulary expresses. Step 2's residue report
answers it; Vellum's catalogue is the starting list.

### 3b. Costs

`timeto` procs become a closed enum evaluated against one character's model at plan time:
`Fixed`, `SameAs(room, key)`, `IfProfession`, `IfSociety`, `IfSpellActive`, `IfSetting`,
`UrchinGate`, `Impassable`. **The graph is shared; a route is per session.**

### 3c. Placement

- `cena-map` — new crate at model level. Graph, loader of the Hydra map binary, room
  identification, Dijkstra, and the primitive vocabulary the converter shares. Pure; no I/O beyond reading the file; property-testable. One `Arc` shared by
  every session.
- Crossing execution lives above `session`, beside the behaviors, because it sends commands
  and awaits frames.
- Architecture test for `cena-map`'s position in the crate graph is written **before** the
  crate has code (`05` §0).

### 3d. id and uid — two numbers, two jobs (DECIDED 2026-09-20)

The mapdb carries both. **`id`** is Lich's number, assigned in the order rooms were mapped.
**`uid`** is Simutronics' internal room number, exposed on the wire about a year ago (author,
2026-09-20). The question: move fully to uid, or keep Lich's dual system?

MEASURED (`research/mapdb-inventory/uid.py`): **7,876 of 36,838 rooms have no uid** (2,029
playershops, ~510 festival grounds, 16 `meta:map:virtual room`, ~945 with no `location`);
**15,430 of 84,867 edges touch a no-uid room**; 281 rooms have several uids (up to **50**;
206 tagged `meta:map:multi-uid`); 42 uids are shared by several rooms (up to 6); 236 uids are
negative; no edge targets a nonexistent id.

So uid cannot be the key: it is many-to-many and absent from a fifth of the graph. `id` is
total, unique, and already what every scripted edge's parameters name (`Room[N]`, maze room
lists, route tables) and what players type (`go2 228`).

**Decided by the author 2026-09-20 — *"I'm down with keeping both ids."* Dual, with the
roles separated:**

- **`id` is node identity.** Edges, routes, bans, crossing parameters, the binary format.
  Taken from upstream unchanged; the converter never renumbers.
- **`uid` is the primary identification key**: an index `uid -> [id]`. One candidate = done.
  Several = disambiguate by adjacency to the previous room (`map_base.rb:195-209`). None =
  the text matcher (§2c): two passes, `unique_loot`, fogged exits, `check_location`, peer
  tags — which can send commands, so identification is not pure.
- **Users may type either** (`123`, `u7120077`), as go2 does; Hydra shows both.
- **Arrival is detected by uid change, node identity resolved by id.** The model keeps the
  raw wire uid beside the resolved id, so moving between two uids of one id counts as
  having moved. This removes the multi-uid stall Vellum recorded (waiting on the id alone
  cost 30 s per step).
- The wire uid is a signed 64-bit value: uids go negative, and Lich treats > 2^32 as "no
  uid" (`map_gs.rb:173`).

**What the odd uids actually are** (MEASURED, `mapdata.py` + ad-hoc; author's reading
2026-09-20 in quotes where it was confirmed or corrected):

- **No uid, 7,876:** 2,029 playershops — *"need to be mapped"*; ~510 festival rooms —
  *"released and removed before uids were released (they don't get removed from mapdb)"*;
  16 virtual rooms (urchin hubs; never get one); ~945 with no `location` — *"probably not
  really accessible"*. So the no-uid set is mostly **dead or unmapped**, not a permanent
  gap. The converter should report it by class, and may prune the dead classes from routing.
- **Several uids on one room, 281:** these are **instanced copies**, not variants. Reim
  Fortress Defense: 37 rooms x 16 uids at a regular stride of 50
  (`7107087, 7107137, 7107187…`). The Elemental Confluence: 53 rooms x 9. Tenebrous
  Cauldron ships: 34 x 7. Public lockers: up to 50 identical stalls. "Lost in the woods"
  mazes. Simu stamps N copies; Lich folds them into one id. `prime/rooms.json` confirms it:
  all 50 uids of room 18011 carry one identical title and description.
- **One uid on several rooms, 42 uids / 224 room-slots:** *every one* is `[Enemy Ship, …]`
  in The Tenebrous Cauldron, tagged `meta:morphing`. The inverse case: one Simu room whose
  content changes by ship class, which Lich mapped as up to 6 ids. Not day/night.
- **Negative uids, 228 rooms:** the Settlement of Reim 155, Ships 54, Warcamp 16 —
  dynamically generated areas. None appear in `prime/rooms.json`.

**A second, uid-native map exists: `reference/mapdb/map-data/`.** `prime/rooms.json` has
**47,956 rooms keyed by uid** (Lich: 36,838), 102,548 exits typed `c`ardinal 70,117 /
`p`ortal (`go`, `climb`) 23,740 / `o`ut 5,214 / `v`ertical 3,447 / `z` 30; 2,647 dangling.
27,657 uids are in both maps; 20,299 are prime-only (6,173 with no `loc`; 2,430 Ebon Gate);
2,759 Lich-only. Matching Lich's no-uid rooms to prime by title + description gives a uid
for **2,044 uniquely** (501 ambiguous). It has no scripted edges beyond a small authored
file, but it has what Lich lacks: terrain and climate on far more rooms, 123 named map
regions by segment (`uid // 1000`; 824 segments), and **per-room creature generators with
levels** (`creatures.json`, 9,878 rooms). **Provenance: official** (author, 2026-09-20) —
Simutronics' own data, so its ids are the authoritative uids and its creature levels are
ground truth, as of a snapshot at least a year old.

**`prime/rooms.json` is at least a year old** (author, 2026-09-20), and the comparison shows
it (`prime_vs_lich.py`). **90.9% of Lich's uids are in prime** (27,657 of 30,416). The
2,759 Lich-only uids break down as follows. **CORRECTED 2026-09-20** — this first said they
were "new rooms … 59 segments prime lacks entirely" and named Evermore Hollow, Rumor Woods,
Bloodriven Village and Kraken's Fall as missing. The author objected (*"there are no missing
areas"*), and for those four he is right: they are **present**; what Lich has and prime
lacks inside them is shop interiors in segments prime already covers (Evermore Hollow 280,
Rumor Woods 210, Bloodriven 194 — `[Poiret & Company, Salesroom]`, `[The Sable Quietus,
Storage]`). 1,656 of the 2,523 positive Lich-only uids are of that in-segment kind. **But in
the copy on disk, four areas are wholly absent** — 0 rooms by uid, 0 mentions by title:
Sleeping Drake Harbor / Bericsaba Market (Lich location "the Isle of Ornath", 186 rooms,
segment 7142, Lich ids 35,244-35,449), Halan Lea (Northern Steppes, 53), Nyessem (47, Lich
ids 36,660-36,706), Brayern Isle (23). Of Lich rooms with id > 36,500 and a uid, 79 of 363
are in prime. UNRESOLVED against the author's statement — likely a newer copy of the file
exists than the one in `reference/`. The lesson is §−2's: "absent from a segment count" was
reported as "area missing" without looking the areas up by name.
The other direction: **only 57.7% of
prime's rooms are known to Lich.** Of the 20,299 prime-only, 4,913 have no exits and 4,149
no description (6,173 no `loc`; 515 are GM consultation rooms), 2,430 are Ebon Gate, 929 are
exact copies of a room Lich has (instanced copies, or uids Lich never recorded), and
**2,406 have an exit into a room Lich knows** — real, adjacent, unmapped. Where both maps
have a room they agree: title 99.4%, description 94.6%; of 62,349 prime exits between shared
rooms 76.8% carry the same command, 3,248 are scripted in Lich, 9,803 differ in wording
(NOT EXAMINED), 1,406 Lich lacks; 8.7% of Lich's edges between shared rooms are absent from
prime. **Prime is a coverage and cross-check source, not a routing source**: a quarter of it
is unreachable, and it has no crossings.

So the two maps are complements: **Lich has the crossings and the tags; prime has the
coverage and the uids.** That does not change the proposal above — `id` still names nodes,
because only Lich's graph has the scripted edges — but it means uid coverage can be
back-filled by the converter rather than waited for.

UNVERIFIED: that the model retains the room serial from `<nav rm=>`.

### 3e. What the map must carry for a frontend — READ 2026-09-20

Source: <https://github.com/GenieClient/Genie5> (cloned to `reference/Genie5`, **GPL-3** — take
the ideas and the data model, do not port its code into Hydra), plus the official
`map-data/prime/layouts.json`. The author intends Hydra's map panel to be based on Genie's.
Target look (author's screenshot): dark canvas, grid squares, colour-coded rooms, region
labels, a hover card (map + room number, colour meaning, aliases, exits), route highlight,
a pulsing "you are here", left-click pin path, right-click walk.

**The central fact: Genie's map is hand-placed coordinates, not computed layout.** Every room
has `X, Y, Z` in one zone file (`MapNode.cs`); zones are separate files joined by connector
rooms. **The Lich mapdb has no coordinates at all** — only `image` + `image_coords`, a
rectangle on a pre-drawn PNG (21,838 rooms). A Genie-style map therefore needs data Lich
does not have.

**That data exists, officially.** `layouts.json` holds prebaked grid positions
`[uid, x, y]` for **14,950 rooms across 123 named maps**, zero layout violations, each room
on exactly one map. MEASURED: 14,277 of Lich's 36,838 rooms (14,292 of its 30,416 uids) have
an official position. `human-authored-map-data.json` defines the 123 maps (name, region,
room membership by segment, `central-anchor-room`) and carries `exits` the crawler could
not see (50 maps), `mazes` (6), `hide-interiors` (13), `demote-exits` (5). Its `puzzle: true`
exits draw as a link with a `?` and are never routed — the same idea as `Edge::Unported`.
No Z: the official layout is flat per map. Keyed by uid, which §3d already keeps.

**Data points to add to Hydra's room record** (none are in the Lich mapdb):

| Field | From | Why |
|---|---|---|
| `map` (slug) + `x, y` | `layouts.json` | position; one map per room |
| `z` / floor | Genie has it; official data does not | Genie filters by level and ghosts adjacent floors |
| map `name`, `region`, anchor room | human-authored file | zone picker, centring |
| labels `{text, x, y, z}`, fractional coords | Genie `MapLabel` | "Lava Drakes", "Darkstone Inn" on the canvas |
| room `color` / category | Genie `Color`; Lich tags can derive it | the legend: bank, shop, guild, hunting |
| `aliases` / notes | Genie `Notes`; Lich tags | hover card, `goto` by name |
| cross-map connector flag | derived from an edge leaving the map | Genie's blue-bordered boxes |

**Data points to add to the edge record:**

| Field | From | Why |
|---|---|---|
| `kind`: cardinal / go / climb / vertical / out / puzzle | official `c p v o z`; Genie pens | line colour; stubs for exits whose neighbour is off-map |
| `hidden` | Genie (1,933 arcs in its corpus) | an edge that routes but is not drawn |
| `requires` (skill>=N, class, level) | Genie `ExitRequirement` | Hydra's cost gates (§3b) are the same idea |
| `rt_cost`, `wait_min`, `wait_max` | Genie | boats and ferries: a countdown in the UI |
| `environment` (boat, rope, bridge…) | Genie | descriptive; groups the timing fields |

**Lessons recorded in Genie's code, worth keeping:** store positions verbatim and derive
the grid cell — 44.6% of its corpus positions were off-grid and integer division moved
them, asymmetrically for negatives; keep the raw exit token separate from the parsed
direction (22% of arcs are non-compass and an enum flattened them to `none`); rooms carry
*several* descriptions (3,963 nodes) and keeping only the first erased the rest on save;
resolve the current room by a ladder — server id, then graph walk from the previous room,
then a title+exits fingerprint, **declining rather than guessing** on a collision, because a
wrong lock-in cascades. That ladder is §2c's Lich algorithm reached independently.

**Genie's maps are for DragonRealms** (author, 2026-09-20) — none of its map *data* applies
to GemStone. What is taken is the **process** and the **fields**.

**How Genie's maps get made:**
1. **Walk to record.** With learning on, arriving in an unknown room creates a node placed
   one grid cell from the previous room in the direction moved
   (`AutoMapperEngine.cs:1070`, deltas in `Direction.cs`: N = (0,-1), NE = (1,-1) …; up/down
   change Z only; `go`/`climb`/`out` move nothing, so those rooms stack until an author
   places them). The server room id is stamped on first visit.
2. **Tidy by hand.** Edit mode: drag rooms, snap to grid, lock positions, add free-floating
   labels, set a room colour, write notes/aliases, edit an exit's requirement, roundtime and
   wait window. This step is why the maps look good — a person decides where the long
   corridor bends and where the label sits.
3. **One zone per file**, joined by connector rooms; a cross-zone table is derived from them.
4. **Share and merge.** A community repo; the updater takes upstream structure but
   **preserves what the local user learned** (server ids, exit metadata) —
   `MapsUpdater.MergePreservingServerIds`.

The official GemStone build reached the same shape from the other end: crawl per segment,
**bake** a layout automatically, then correct it by hand in one authored file (`exits`,
`mazes`, `demote-exits` for a "lying backlink" that would otherwise force a maze plate).
Both pipelines are *generate, then hand-correct, and keep the corrections in a file that
survives regeneration*. That is the process to copy.

**Consequence for the pipeline (§3a):** the converter gains a second input. Routing comes
from the Lich mapdb; **layout, map membership and edge kinds come from the official
map-data, joined on uid.** Rooms with no official position (22,561) need either the Lich
PNG rectangle as a fallback view or placement by an authoring tool — the official build has
an "authoring harness", and Genie has an edit mode; Hydra will want one eventually.
Layout is frontend data: it rides in the map binary but nothing in `cena-map`'s routing
reads it.

### 3f. The room record — the converter's per-room JSON (PROPOSED 2026-09-20)

Author decisions folded in (2026-09-20): **floors are required** for the built-in map;
**tags are split** into routing targets and typed behaviour flags; **creature data stays out**
— it is already ported as `crates/cena-model/data/creatures.tsv` (627 creatures) and
`creature_areas.tsv` (1,394 rows keyed by **uid range**), so "what hunts here" is a runtime
join on the uid this record already carries.

`?` = optional, omitted when empty.

```jsonc
{
  "id": 0,                       // Lich id — node identity (§3d). Always present.
  "uid": [13104001],             // Simu numbers; [] when unmapped or virtual

  // identification — for the text matcher when uid is absent or ambiguous (§2c)
  "title": ["[Moonglae Inn, Atrium]"],      // lists: variants are real
  "description": ["A skylight two stories above…"],
  "paths": ["Obvious exits: north, …"],
  "location": "the Moonglae Inn",           // ? null = the LOCATION verb failed here
  "check_location": false,                  // ? a twin exists; identification must ask
  "unique_loot": [],                        // ?
  "peer": null,                             // ? { "dir", "pattern", "needs_desc" }

  "climate": "none", "terrain": "none",     // ?
  "indoors": true,                          // ? official 'od'

  "tags": ["inn", "tables"],                // routing targets
  "meta": { "nomagic": false, "transport": false, "premium": false },  // ? typed flags
  "class": "live",     // live | playershop | festival-removed | virtual | unreachable

  // layout — frontend only; routing never reads it
  "map": "ta-illistim-town",                // ? one map per room
  "pos": [14, 22],                          // ? grid x, y — stored exactly
  "floor": 0,                               // ? REQUIRED by the author for the built-in map
  "color": "lodging",                       // ? a category; the renderer owns the colour
  "aliases": ["moonglae"],                  // ?
  "image": { "file": "…png", "rect": [0,0,0,0] },   // ? Lich picture fallback

  "exits": [
    { "to": 13, "kind": "cardinal", "cmd": "north", "cost": 0.2 },
    { "to": 1,  "kind": "go", "cost": 0.2,
      "steps": [ { "await": { "cmd": "go Cat's Paw table", "pattern": "…", "timeout": 25,
                              "then": "go Cat's Paw table" } } ] },
    { "to": 30714, "kind": "virtual", "steps": [ { "noop": true } ],
      "cost": { "gate": "urchins", "value": 0.1 } },
    { "to": 3668, "kind": "go", "hidden": true, "steps": [ { "unported": "a41f…" } ],
      "cost": { "gate": "setting", "name": "fwi_trinket", "value": 15.0 } }
  ]
}
```

**The exit record.** `to` is a Lich id. `kind` (cardinal / go / climb / vertical / out /
virtual / puzzle) drives line colour and off-map stubs. **`cmd` xor `steps`** — a plain edge
has a command, a scripted one has a flat step list, each step optionally guarded. `cost` is a
number or a gate evaluated against one character at plan time; **a missing cost is
impassable, not defaulted**, as in Lich (`map_base.rb:829`). Optional: `hidden`, `silver`
(from `silver-cost:` tags), `wait: [min, max]`, `replan`, `sets` (a per-trip variable).

**Floors — the author's rule (2026-09-20).** The ground floor is obvious. `up` is up a
floor, `down` is down a floor. A staircase (or ladder, steps, hatch…) has no direction in its
command; it is read from **context in the room description**, and from the ground floor most
lead up. MEASURED in the Lich mapdb: **2,302 explicit** vertical exits (`up` 1,073, `down`
1,229) against **2,451 ambiguous** `go`/`climb <noun>` exits — staircase 489, steps 411,
ladder 354, stairs 319, stairway 191, hatch 118, hole 103, trapdoor 86, ramp 67, tree 64.
So the ambiguous class is slightly the larger one: floors cannot be derived from exits alone.

Derivation, in order of trust: (1) explicit `up`/`down`; (2) a paired return edge that is
explicit (you `go stairs` up and come back by `down`); (3) nouns with a fixed sense (hole,
pit, well, trapdoor usually descend; tree, rope, cliff usually ascend) — a prior, not a rule;
(4) the description; (5) the ground-floor default. Anything below (2) is a *proposal*.

**An LLM classification pass** (author, 2026-09-20: run the finished map past a model to
classify rooms and exits). This fits as an **offline converter stage**, never a runtime one:
it reads a room's title, description and exits and proposes `floor` deltas for ambiguous
exits, `color`/category, `indoors`, and exit `kind`. Rules for it: deterministic passes run
first and the model sees only what they left open; every proposal is written to the
**authored-corrections file** with its source marked `llm`, so a human can review a diff and
a wrong guess is a one-line fix that survives regeneration; results are cached by a hash of
the room text, so a rebuild re-asks only about rooms that changed. **It needs no game
session** (author, 2026-09-20): the mapdb already carries the text — 36,786 of 36,838 rooms
have a title, 36,528 a description, 36,667 their exits — and reading the file is *better* than
walking, because an ambiguous exit can be judged from **both** rooms at once (send the pair,
with every description variant). Limits: descriptions agree with the official data for 94.6%
of shared rooms, so a few are stale; ~310 rooms have no description. What does need the game
is *mapping* (2,029 playershops, 2,406 adjacent unmapped rooms), a separate job. It is distinct from the
LLM-controller goal (an agent playing a character) — this one never touches a live session.

**The model is Jev** (author, 2026-09-20). READ 2026-09-20, from TypeSafe's announcement
(<https://typesafe.ai/blog/introducing-system-one-models-and-jev>) and the OpenRouter listing
(`typesafe/jev-1.13`), both through a summarising fetcher — so the numbers below are
**UNVERIFIED until a real call is made**. Jev is a *decision* model, not a chat model: the
request names the allowed answers, the reply is one of them as a typed value **with a
probability**, and it generates no strings, so it cannot answer outside the choices. A choice
field holds at most 255 options; context is 32,000 tokens; input is $0.042 per million tokens
and output is free; claimed latency 70–500 ms. It is hosted only — no weights, nothing to
train — and is called at `POST https://openrouter.ai/api/alpha/decisions`, not the chat
endpoint. It is in early access, so the request shape may move: the stage talks to it through
one small module, and its work package is plain JSON in, JSON out, so another model can stand in.

That shape fits, because every job here is pick-from-a-list:

| job | choices | answer key to score it on first |
|---|---|---|
| ambiguous vertical exit (2,451) | up / down / same floor | the 2,302 explicit `up`/`down` exits, command hidden |
| side a non-compass exit is drawn on | 8 compass sides / none | exits between two rooms the official layout places (14,277 rooms) |
| room category, for the legend | the legend list | rooms whose tags already settle it |

**Score before trusting.** Each job is first run on its answer key; a job whose score is poor
is dropped or handed to a different model, for pennies. The probability is the review queue:
above a threshold (set from the scored run, not guessed) a proposal is written to the
corrections file; below it, it goes on the author's list. INFERRED cost of the whole map,
~10M tokens: under a dollar.

**Not Jev's job: `terrain` and `climate`.** Author, 2026-09-20: rooms without them *are set up
without them* in the game. An empty field is the truth, not a gap; the converter keeps it
empty and nothing fills it. For the same reason those two fields are not labels for
indoors/outdoors.

**Placement is mostly not a model's job.** Compass exits are arithmetic — walk the graph, one
cell per move, resolve collisions — which is all Genie's automapper does; its maps look good
because people tidy them. Order: (1) official positions where they exist; (2) a deterministic
placer for the rest, **scored against the official areas**; (3) Jev for door sides only;
(4) a hand-tidy pass through the corrections file.

**Needed before the stage can run:** the corrections file's format, and the rule passes
(floors derivation steps 1–3 above). Neither depends on §5's steps 4–7.

**Kept out of the room files:** forage data (`meta:forage-sensed`, 44,781 tag entries) —
a side file keyed by id; creatures — already ported, joined by uid.

**Beside the room files:** `maps.json` (per map: name, region, anchor room, labels with
fractional positions and a floor) and an **authored-corrections file** that survives
regeneration: position and floor overrides, hand-written crossings for the errand class,
puzzle exits.

**Read Vellum first for layout.** `reference/VellumFE/defaults/curated_maps.toml`
(`source_layout_version = 61`, the same version as the official `layouts.json`) and
`map_overrides.json` show Vellum already joins the official layout. NOT YET READ.

## 4. The special cases — to be designed BEFORE porting crossings

Each gets a short design note in this document before its code exists.

1. **Urchins.** Per-character validity from `urchin status` (enabled ∧ not expired ∧ not
   hidden ∧ not invisible ∧ not mounted), held in the model. Pre-flight when unknown.
2. **Cost gates.** Portmasters, Voln seeking, profession/race, FWI trinket, day pass, silver.
3. **Replan.** `$go2_restart`: some crossings land somewhere unpredictable; the trip re-plans.
4. **Per-trip scratch state.** Event-area return edges send the same command and are
   distinguished only by a variable their entry edge set.
5. **Hands.** `empty_hands` / `fill_hands` need a stash service over inventory.
6. **Shifting areas.** Confluence, minotaur maze, Hidden Plateau, Karazja, pathcode mazes.
7. **Errands.** River's Rest boot and its kind: `from`/`to` override with `require_item`.

## 5. Order of work

1. This document, approved.
2. ~~**The converter, plain edges only**, plus the vocabulary in `cena-map`.~~ **DONE
   2026-09-20.** `crates/cena-map` (room and exit records; pure, enforced by
   `layering.rs::map_does_no_file_io`) and `crates/cena-mapdb-convert` (the converter; in
   `crates/`, not `tools/`, because the architecture harness assumes that geometry). Every
   `;e` edge comes out `Unported` with the FNV-1a id of its shape. 24 tests; the fixture is
   22 rooms cut from the real map. MEASURED on `map-1789942730.json`, release build, 29 s:

   | | |
   |---|---|
   | rooms / exits | 36,838 / 84,867 |
   | **routable as converted** | **76,411 (90.0%)** |
   | unported crossings | 7,923 in **362** shapes |
   | unported costs | **1,860** in 62 shapes |
   | exits with no cost / dangling | 2 / 0 |
   | upstream oddities reported | 200 — 133 coords with no image, 61 `timeto` with no `wayto`, 6 image with no coords |

   **The ratchet caught its first defect on its first run**, in this document: §1 and §2d say
   1,873 scripted costs, which counted 13 that price a destination with no `wayto` — costs
   for exits that do not exist. The exits-side count is 1,860, and that is the baseline.
   The shape count is 362, not §2b's 367: the Rust normaliser tells a regex from a division
   and leaves digits inside identifiers alone, which the Python did not.

   The ratchet (`tests/ratchet.rs`, `tests/unported.baseline`) holds both numbers **exactly**:
   a rise fails (new upstream shapes; the residue it prints names them) and so does a fall
   (turn the baseline down to bank it).
   `$env:CENA_MAPDB = "E:\Cena\reference\mapdb\map-1789942730.json"; cargo test --release -p cena-mapdb-convert --test ratchet -- --nocapture`
3. ~~**The combiner, the binary format and the loader.**~~ **DONE 2026-09-20.** Three tools
   now, each knowing as little as it can: `cena-mapdb-convert` breaks upstream into one JSON
   file per room and is the only place Ruby is seen; **`cena-map-combine`** reads those files
   and writes the single binary (the author's word for it: *"that's the combiner!"*); the
   client loads it. The format lives in `cena_map::binary`, so the combiner writes and the
   client reads one definition, and the on-disk layout the two tools share is
   `cena_map::files`. `Map` indexes rooms by id and by uid — `ids_for_uid` returns a *slice*,
   because §3d's relation is many-to-many.

   The format is hand-rolled (no new dependency), little-endian, versioned, with every string
   interned in one table. The three rules, each held by a test in
   `crates/cena-map/tests/binary.rs`:
   1. **An older client loads a newer map.** A crossing and a cost are a *name and a
      length-prefixed blob*; a name this build does not know loads as `Crossing::Unknown` /
      `Cost::Unknown` — impassable — and its blob is skipped by length. Each room also ends in
      a list of named **extensions**, skipped the same way: that is how layout and floors
      (§3e) arrive later without a version bump. An unknown exit *kind* draws as a plain
      line and does not affect routing. A tool never writes an `Unknown` back.
   2. **Another version is refused**, as `LoadError::UnsupportedVersion`, never misread.
   3. **One vocabulary**: the wire names are constants on the types, used by both sides.

   The loader never panics and never trusts a count: every prefix of a valid file is an
   error, and a count larger than the bytes that remain is refused before anything is
   allocated. The combiner reads back what it wrote and compares before reporting success;
   only `index.json` says which room files are current, and a room it names that is
   missing, invalid, or holds another id is an error.

   MEASURED on the full map, release build: **21,738,800 bytes** (upstream JSON:
   43,303,215), **loads in ~79 ms** including both indexes; reading the 36,838 JSON files
   takes 2.0 s; a second converter run rewrites none of them. 45 tests across the three
   crates; the architecture suite's dev-only-edge rule now covers
   `cena-map-combine -> cena-mapdb-convert`.

   ```powershell
   cargo run --release -p cena-mapdb-convert -- <upstream-map.json> <dir>
   cargo run --release -p cena-map-combine  -- <dir> <dir>\hydra.map
   ```
4. Room identification from live frames.
5. Dijkstra with per-character costs. Answer key: `Map.dijkstra` results captured from the
   author's live Lich.
6. §4's design notes.
7. Primitives, then recogniser arms, in edge-count order. A new arm that needs no new
   primitive is converter-only work.
   Vellum's port is read first for each.
8. Map images — GUI only, last. Community assets: fetched, not committed.

## 6. Open — author decisions

- **Rooms with no official layout position (22,561):** fall back to the Lich PNG view, or
  build an authoring tool? (Floors: DECIDED yes, §3f — but they have no source yet.)
- **Forage tags:** in the core graph, or a side table? They are most of the tag volume.
- **Where the per-room JSON lives:** this repo, or its own (it is ~37k files)?
- **How Hydra gets the binary:** release asset fetched at start, or placed by hand?
- **Scope of pass one:** plain walking only, or urchins/portmasters from the start?
