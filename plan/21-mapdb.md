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

## 4. The special cases — design notes (PROPOSED 2026-09-20, step 6)

Written before any crossing is ported, so the primitives are shaped by the hard cases and not
by the easy ones. Evidence: `research/mapdb-inventory/vellum-executor-semantics.md` (Vellum's
executor, read in full), `go2-and-lich-map-semantics-part2.md` (go2 and Lich's `move`), and
the scripts themselves. Everything here is a proposal until the author rules.

### 4.0 Three pieces, and where each lives

| piece | crate | what it is |
|---|---|---|
| **the vocabulary** | `cena-map` | `Crossing`, `Cost`, and one shared `Cond`; plain data with wire names |
| **the walker's facts** | `cena-map` | `Walker`: a plain struct of what pricing and guards may ask — profession, settings, active spells, encumbrance, remembered origins, banned exits, *the time*. No model types, so `cena-map` stays dependency-free |
| **the walk** | `cena-behavior` | the Travel behavior: a pure state machine, `tick(facts, frames) -> commands`, as Vellum's (`executor.rs:787`); fills `Walker` from the model each plan |

Rules that hold across every case:

- **One `Cond` for costs and for step guards.** `Stats.prof == S ? N : nil` (a cost) and
  `if Skills.survival < 50` (a guard) ask the same kind of question. One enum, one evaluator.
- **Unknown answers "no".** A fact the model has not been told prices the exit impassable and
  fails the guard. Vellum's lesson (`executor.rs:197`): refusing a route is recoverable,
  walking one you cannot take strands you. The walk then *asks* (a pre-flight probe) rather
  than guessing.
- **A cost never acts.** Upstream's day-pass cost opens containers and reads passes *inside
  the pathfinder*. Here pricing is a pure function of `Walker`; anything that must be looked
  up is a pre-flight step that runs before planning and lands in the model.
- **The clock is a fact.** `Walker.now` is passed in. Pricing never reads the wall clock, so
  a replay plans the same route (`12` §7.2 criterion 7).
- **Randomness is seeded** from the session and recorded, for the same reason. Vellum's
  `MoveAnyExit` uses `rand::rng()` (`executor.rs:3947`); that is not copied.

**How a plain exit is crossed** is Lich's `move` (`move.rb:167-447`) and Vellum's reaction
table, which agree and are both digested. In Hydra the line matching is a **stateless
classifier over frames** (`12` §3a), not a substring scanner, and the lessons Vellum paid for
are requirements, not options: stamp every send and attribute failures FIFO; retry what was
*sent*, never the map's text; never ban an exit on lag — only when the walker never left the
first room; a `<nav>` beats a failure line that raced it; cap every loop (its uncapped
`open` was "the only outright hang in the walker"); never end a trip with items still stowed
— **including on a user stop**, which Vellum skips.

### 4.1 Urchins — and pass-through rooms

MEASURED: one virtual room per town (16 tagged `map:virtual room`; Ta'Illistim's is 30714).
**523** exits enter one with the crossing `;e true` — nothing is sent — priced by one gate;
the exits *out* are plain `urchin guide <place>`. **930** further costs are the delegation
`Map[7].timeto['30714'].call`: "whatever that exit costs".

- **`Crossing::PassThrough`.** Nothing is sent and no arrival is awaited; the walker treats
  `A → hub → B` as one hop, sending the hub's command from A. Vellum learned this the hard
  way ("3637 -> 30718 keeps failing" from arrival-watching a room that does not exist).
  `locate` never answers a virtual room: it has no number and no text.
- **Delegation is resolved by the converter**, not at run time: `Map[N].timeto['M'].call`
  becomes a copy of that exit's cost. 959 scripted costs disappear into whatever they point
  at, and no primitive is spent on them.
- **The gate**: `Cond::All[Setting(urchins), Before(urchins_expire), Not(Hidden),
  Not(Invisible), Not(Mounted)]` → 0.1 s. Every part is a `Walker` fact.
- **The expiry** comes from `urchin status` (three reply forms, `go2:975-996`; "permanent"
  means no expiry). A classifier keeps it in the model *whenever the game says it*. The walk
  probes only when the setting is on and the expiry is unknown or past, **before** planning
  — Lich asks after the trip, so its first trip of the day plans without them.
- **Mounted** is learned from a rejection line mid-trip: set the fact, replan.

### 4.2 Cost gates

§2d classified all 1,860: 21 kinds, none unclassified. After delegation (4.1) they reduce to:

| form | covers | shape |
|---|---|---|
| `Gated { when: Cond, then, otherwise }` | settings (portmasters, premium, Vaalor shortcut…), profession, race, society, month, remembered origin (4.4), posture + climate | `cond ? N : nil`, `cond ? N : M` |
| `Table { by: room-of-last-instability, seconds }` | the 477 instability costs | a lookup in `Walker` |
| `Formula` | 27 arithmetic costs | INFERRED: a handful of named formulas, not an expression language — to be confirmed when they are read |

Settings are one **per-character travel profile** (a data profile, `CLAUDE.md`): urchins,
portmasters, seeking, ice mode, may-withdraw-silver, day pass, FWI trinket, caravans. go2's
list is in the digest, §5. **Silver is not a cost gate:** `silver-cost:` tags sum along the
planned path (no dynamic values exist upstream — Vellum checked), and being short is a
*funding detour* before departure — one multi-target search for the nearest affordable bank
(Vellum's 72 separate searches froze its UI) — recomputed on every replan.

### 4.3 Replan

MEASURED: **673** crossings end with `$go2_restart = true`: the crossing may land anywhere
(a trinket, a locker curtain, a ferry). **`Replan` is a step**, legal only last: locate
again, and if that is not where the plan expected, route again from there. Skipped when the
walker is already where it meant to be — an unguarded trailing replan reported finished
trips as failures (`executor.rs:2486`). Bounded (Vellum: 10 restarts; go2: unbounded).
Banned exits are **per trip**, live in `Walker`, and therefore reach the pathfinder through
`price` with no special mechanism.

### 4.4 Memories — what upstream calls per-trip variables are not per trip

`UserVars.mapdb_duskruin_origin = 7` is written by the exit *into* an event ground; every
exit back is priced `origin == 7 ? 0.2 : nil`, so only the way you came is open. `UserVars`
persist across logins — rightly: a character enters Duskruin on Friday and leaves on Sunday.

**DECIDED (author, 2026-09-20): named memories, Lich's list.** I proposed collapsing them
into one unnamed fact ("which room did I enter this place from"), on the measurement that
every `*_origin` write is the room being left and every read compares against the exit's
own destination (`origin.py`). The author declined, for two good reasons: these are **not
all the same fact**, and new ones are rare, so a list costs nothing.

| memory | written when | read by | the author's account |
|---|---|---|---|
| `duskruin_origin`, `ebon_gate_origin`, `talondown_origin`, `marksofthebeast_origin` | `event transport <event>` from a town | each way back: open only to the town you came from | an event returns you where you left |
| `fwi_return_room` | turning the FWI trinket — **usable only from inside a town** | the way back from Mist Harbor | returns you to *the same room in the town you came from*. **Arrival is a random room, or a preset one** if a GM has set the device — so this crossing ends in `Replan` (4.3) |
| `redforest_location` | entering the Red Forest | its exits | entered from the Landing side or the Nations side, **and it cannot be used to cross realms**: you leave by the side you came in |
| `hinterwilds_location` | the Hinterwilds sliver (`EN` / `IM`) | its way back | go2's own detour, digest §6 |

- Step **`Remember(name, value)`**, run only after the steps before it succeeded — a
  transport that failed must not leave a false memory.
- **`Cond::Remembered(name, value)`** prices the way back. A name never written is unknown,
  so every way back is impassable — correct for a character Hydra did not watch arrive, and
  the reason the memory is **persisted per character**, across logins.
- Names are upstream's, less the `mapdb_` prefix. A new one upstream is a converter arm, not
  a new primitive: the vocabulary is already general.

`$minotaur_maze_dirs` and `$mapdb_confluence_target` are routine-internal (4.6), and the
day-pass variables are a pre-flight (4.0), not memories.

### 4.5 Hands

`empty_hands; move X; waitrt?; fill_hands` and its kin, and also `move`'s own reaction to a
hands-full line (`move.rb:350`). Steps `EmptyHands` / `FillHands` drive a **stash routine in
`cena-behavior`** — not in Travel, because Loot and Heal need the same thing (rule of three
is one short). It keeps a LIFO stack of what it put where, stows by id (`_drag #id #bag`, as
Vellum), and the walk may not finish, fail or be stopped with the stack non-empty. Needs
from the model: both hands and the containers, which it has. Which container is **the game's own stow list** (DECIDED, author, 2026-09-20): the model already reads it
(`cena-model/src/state/containers.rs`, a port of `stowlist.rb`), so nothing is configured
and nothing is guessed from nouns as upstream does (`/cloak|longcoat|backpack|pack/`).

### 4.6 Shifting areas — named routines

The Confluence (3,233 exits, one shape), the minotaur maze (497), Hidden Plateau, Karazja,
pathcode mazes, and the ~16 `WanderUntil` cases. No step list describes these: the area
rearranges itself and the script *searches*. Each is **`Crossing::Routine { name, args }`**:
a Rust function in `cena-behavior` that takes over the walk until it arrives or gives up.

- The converter recognises a routine by its shape and emits the name. The map file format
  already handles the rest (§5 step 3, rule 1): an older Hydra that lacks the routine loads
  the exit as `Unknown` — impassable — and routes around it.
- A routine sees only what a behavior sees (frames and the model) and is bounded.
- The set is **open but tiny** (§2d). This is the one place a new upstream
  area can require a Hydra release, and the ratchet is what says so.
- **Read Vellum's first** (`confluence.rs`, `minotaur.rs` — NOT YET READ): both are native
  ports that work against the live game.

### 4.7 Errands

MEASURED: the River's Rest crossing is **3 exits and opens 868 rooms** — second on §5's
list. Its script starts a *second go2* to fetch an amulet, finds containers, buys, and
returns. So an errand is a routine (4.6) with one more need: **a sub-trip** — the walk must
be able to run another walk and resume. That is a stack of trips in the Travel behavior, not
recursion in map data, and it is the same mechanism the funding detour (4.2) needs. ~15
exits, each hand-written, in the authored-corrections file (§3f) so regeneration keeps them.

### 4.8 The flat-steps rule — DECIDED yes (author, 2026-09-20)

Every case above fits: a crossing is a **flat list of steps, each optionally guarded by a
`Cond`**; loops and searches are single steps or routines; nothing nests in map data. The
first port should be the **icy-path shape** — 171 exits, **+2,278 rooms**, the largest gain
on §5's list — because it is small and exercises every part once:

```ruby
if (ice_mode == 'wait') or ((ice_mode != 'run') and
   ((encumbrance > 50) or ((Skills.survival < 50) and not Spell['Haste'].active?)));
  sleep 0.2; echo 'trying not to slip...'; sleep 4; end; move 'west'
```
```json
"steps": [ {"pause": 4.2, "when": {"any": [ {"setting": ["ice_mode","wait"]},
             {"all": [ {"not": {"setting": ["ice_mode","run"]}},
                       {"any": [ {"encumbrance_over": 50},
                                 {"all": [ {"skill_under": ["survival",50]},
                                           {"not": {"spell_active": "Haste"}} ]} ]} ]} ]}},
           {"move": "west"} ]
```

One guard, one pause, one move, four kinds of fact — and the `echo` is dropped, as it will be in all
**890** scripted crossings that `echo` or `respond` (MEASURED): they talk to a Lich user.

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
4. **Room identification.** The question is **BUILT 2026-09-20**; the wiring is not.
   `cena_map::locate` is a pure function over a loaded map: a `Sighting` (the game's number,
   title, description, exits line, and a `location` reading if anyone has asked) plus an
   `Origin` (nowhere / still in room X / just left room X) gives `Here { room, by }`,
   `Ambiguous(candidates)` or `Unknown`.

   Vellum and Lich were read first and **neither is copied**. Vellum (`map_service.rs:718`)
   is number-first and never guesses, which is kept — but for a number several rooms share
   it takes the last one silently (`mapdb/mod.rs:407`), it draws a session-only sketch for
   any room the map has no number for, and its text fallback is dead on a live stream
   because the title field it reads is never written (`travel_ticks.rs:942`). Lich
   (`map_gs.rb:165-308`) text-matches well but **takes the first room that fits**, and
   sends `location` and `peer` to the game from inside the lookup. The ladder here:
   the number → text among the rooms sharing it → *for a number the map has never seen,
   text among only the rooms that could own it* (no number recorded, or tagged
   `map:multi-uid`) → standing still / the one candidate the room just left leads to →
   `Ambiguous`. A filter that matches nothing is ignored, not obeyed (Vellum's lesson:
   stale text is not "nowhere"). Fog and `random-paths` rooms skip the exits check (Lich's).

   MEASURED over all 36,838 rooms, each shown its own recorded text
   (`crates/cena-map/tests/locate_real_map.rs`, `CENA_MAP`-gated, 2.9 s for ~50,000 lookups):

   | | found | ambiguous | unknown | **wrong** |
   |---|---|---|---|---|
   | rooms with a number | 28,962 | 0 | 0 | **0** |
   | rooms without one, from nowhere | 7,495 | 329 | 52 (no title recorded) | **0** |
   | rooms without one, walked into (every exit into one) | 13,500 | 32 | 0 | **0** |

   The test's assertion is the last column: a room's own text names that room or declines.
   All 42 numbers shared by several rooms are separated by text alone
   (`research/mapdb-inventory/identify.py`).

   **Not built, on purpose:** asking the game. `location` would settle 122 of the ambiguous
   rooms and `peer` 22; both are *actions*, belong to a behavior, and `Sighting.location`
   is already the slot the answer goes in. **The model now keeps the room's name** (2026-09-20, author: *"tell model to
   stop throwing stuff away"*): `Room::title`, from the `room` window's subtitle, which the
   parser produced and the model dropped. `title_from_subtitle` turns it into the map's
   spelling. **Still not wired:** joining model to map belongs in `cena-session`, the
   first crate allowed to see both.
5. ~~**The pathfinder.**~~ **BUILT 2026-09-20.** `cena_map::route`: `Map::routes(from,
   target, price)` is Dijkstra over the exits, and **the pricing is a function the caller
   passes in** — `None` is impassable. That is the whole of "per-character costs": a day
   pass, a profession gate, an exit that failed twice this trip are all just a different
   closure, the map stays shared and immutable, and two characters planning at once share
   nothing. (Vellum read first, `pathing/dijkstra.rs`: its gates are process globals,
   `transpile::urchins_valid()`.) `as_converted` is the pricing that needs no character —
   a plain command at a constant cost. One search answers many questions: `Routes` keeps
   the distances and `path_to` reads a path out without searching again. No default cost,
   as upstream; a negative or non-finite price is a wall. Ties break by room id.

   **One upstream quirk deliberately not ported:** the nearest-of-several search that accepts
   a target only under 20 s and otherwise explores everything. Dijkstra settles rooms in
   distance order, so the first target settled is the nearest at any distance.

   **Answer key.** Not Lich's — its routes cross scripted exits, which arrive in step 7. The
   key is an independent Python Dijkstra over the *upstream* file, restricted to the same
   exits (`research/mapdb-inventory/route_key.py`); the Rust search over the *converted
   binary* must agree. MEASURED: **3,800 of 3,800 distances agree**, 10 sources; the
   slowest whole-map search took **1.6 ms** (`tests/route_real_map.rs`, env-gated).

   **THE FINDING: 90% of exits is not 90% of the map.** Only 248 of those 3,800 pairs are
   reachable. MEASURED (`chokepoints.py`): from Wehnimer's Town Square plain exits reach
   **6,969 rooms**; every exit open reaches **27,959**. From Ta'Illistim 1,308; River's
   Rest 868; Zul Logoth 936. A handful of scripted exits are the bridges between regions,
   so **step 7's order is rooms opened, not edges counted.** Greedy, from room 228:

   | rooms | gained | shape ported |
   |---|---|---|
   | 6,969 | | plain exits only |
   | 9,247 | +2,278 | crossing: the `mapdb_ice_mode` icy-path shape |
   | 10,115 | +868 | crossing: the `force_go2` sub-trip shape |
   | 10,556 | +441 | crossing: `N.times{fput S}; mapdb_duskruin_origin = N` |
   | 10,863 | +307 | crossing: `move S` |
   | 11,147 | +284 | cost: `Stats.prof == S ? N : nil` (guarded form) |
   | 11,372 | +225 | crossing: the inn-table shape |
   | 11,597 | +225 | cost: `Stats.prof == S ? N : nil` |

   One shape at a time stalls near 12,000: past that, a region opens only when *several*
   shapes are ported together (a crossing and its cost gate, or a chain of ferries). So the
   ratchet needs a second number beside "unported exits": **rooms reachable from a town**.
6. ~~§4's design notes.~~ **WRITTEN 2026-09-20**, as proposals awaiting the author.
7. **STARTED 2026-09-20 — the first script is ported.** The machinery, all in place and all
   small: `cena_map::cond` (`Cond`, three-valued so *not unknown* is never *yes*, and
   `Walker`, the plain facts it is asked of); `cena_map::step` (`Action::{Move, Pause}` and
   a `Step` = action + optional `when`); `Crossing::Steps`; and in the converter
   `recognise.rs`, where **an arm is the upstream script verbatim with holes** — no Ruby
   grammar, no partial understanding. One changed character upstream and the arm stops
   matching, the exit returns to `unported`, and the ratchet fails: loud, and offline.
   In the binary a step list is JSON inside the crossing's blob, so a step a build has
   never heard of fails to parse and costs one exit, not the map (test:
   `ported_steps_round_trip_and_a_step_this_build_does_not_know_is_impassable`).

   **The icy paths** — all 171 exits, two upstream shapes (150 on the trails, 21 on the
   glacier). **DECIDED (author, 2026-09-20): every one casts Sigil of Resolve when it is
   known and affordable** — upstream does that only on the glacier — **and the whole
   behaviour is one profile setting, `ice_mode`: `run` (just move: no cast, no wait), `wait`
   (always wait) or `auto`** — go2's own values, kept because they say what happens. So both shapes become
   the same three steps, *cast, wait, move*, each keeping its own wait and test. That added
   `Action::Cast` and `Cond::{SpellKnown, SpellAffordable}`; *affordable* is a fact the
   planner supplies, since working it out needs the spell table and the vitals. Dropped:
   the glacier's reaction to a fall (cast Haste, stand, replan) — recovering from a fall
   is the `move` step's job on every exit. MEASURED on the real map:

   | | before | after |
   |---|---|---|
   | unported crossings | 7,923 | **7,752** |
   | rooms reachable from Wehnimer's, knowing nothing of the walker | 6,969 | **9,247** |
   | room files the first port rewrote (the 150; before the glacier's 21) | | **85** of 36,838 |

   9,247 is exactly what `chokepoints.py` predicted, which validates it as the way to pick
   what is next. **The ratchet now pins `reachable` as well as the unported counts**, and
   it may only rise. One honest gap: a guard that cannot be answered does not fire, so a
   walker whose skills are unknown *runs the ice* — it errs toward a fall (recoverable)
   rather than refusing a route.

   **Second batch, 2026-09-20.** New vocabulary: `Action::Put` (a command that does not
   change rooms), `Crossing::PassThrough` (upstream's `;e true`, §4.1),
   **`Cost::Gated { when, then, else }`** — the first ported costs — with
   `Cond::{Profession, Month}`, and `route::priced_for(walker)`, the pricing for one
   walker. A gated cost has **three** outcomes: holds, does not hold, and *cannot be
   answered*, which is impassable rather than the `else` price. Arms: `move 'X'` with or
   without `waitrt?` (roundtime is part of what `Move` means); `fput 'X'; move 'Y'`;
   `event transport` with its `Remember`; the profession gates in all four spellings;
   the way-back gates on a memory; `mapdb_use_*` settings; months. One deliberate
   difference: two profession spellings add `!defined?(Stats.prof) or`, which lets a
   walker of *unknown* profession through. Here unknown is impassable, like every unknown.

   | | start | icy paths | this batch |
   |---|---|---|---|
   | unported crossings | 7,923 | 7,752 | **6,507** (348 shapes) |
   | unported costs | 1,860 | 1,860 | **1,683** (52 shapes) |
   | reachable from Wehnimer's, knowing nothing of the walker | 6,969 | 9,247 | **10,118** |

   **Third batch, 2026-09-20: urchins and delegation.** `delegate.rs` resolves
   `Map[N].timeto['M'].call` **before conversion**, by replacing the deferring cost with the
   one it points at: **957 resolved**, no primitive spent. Their targets were the urchin
   gate (`Cond::Flag` — `urchin_access`, `hidden`, `invisible`, `mounted` are yes-or-no
   facts the planner works out, since nothing in the vocabulary reads a clock), the FWI
   trinket (`Cond::SettingIsSet`), and *"only while go2 is running"* (406) — which keeps a
   person stepping by hand out of a hub's exits, is always true for a planned walk, and so
   becomes a constant. **Unported costs 1,683 → 739.** The ratchet gained
   `reachable_equipped`, a walker with urchins and portmasters on and no profession:
   **10,123** — five more rooms than knowing nothing, because urchins only hop inside towns
   already reached and the portmasters' *crossings* are still unported. That figure is
   where the next ports will show.

   **Fourth batch, 2026-09-20: the first routines.** `cena_map::routine::Routine` — a name
   and its arguments, typed; the algorithm is the Travel behavior's. Vellum's native ports
   were read first (`confluence.rs`, `minotaur.rs`) and they fix what the arguments are:
   - **`Confluence { leave }`**, 3,233 exits, all `$mapdb_confluence_target = T; Room[23282]
     .wayto['23282'].call`. MEASURED: T is the exit's own destination on 2,756 and the word
     `tranquility` on 477 — so the goal is never an argument, only whether the walk leaves
     the plane. Vellum hardcodes the zone's 53 rooms and never implemented the pit crossing
     between the hot and cold halves (`ConfluenceMove::CrossPit` is a dead arm).
   - **`MinotaurMaze { rooms }`**, 497 exits. The 1.2 KB search after the two arguments is
     **byte-identical on all 497**, so it is kept verbatim in
     `upstream_scripts/minotaur_maze.rb` and matched exactly: an upstream edit to the search
     un-ports all 497 at once, which is the right alarm. Two Vellum decisions to keep: no
     learning carried between trips (upstream's `$minotaur_maze_dirs` is a process global,
     and stale learning in a maze that re-scrambles is worse than none), and
     least-recently-visited instead of `rand` when nothing is known — deterministic, so a
     replay walks the same way.

   **Unported crossings 6,507 → 2,777.** Reachability did not move (10,118 / 10,123): what
   is left between a town and these areas is still unported, so the next ports are chosen
   by `chokepoints.py`, not by size.

   **Fifth batch, 2026-09-21: the bridges.** `chokepoints.py` now reads the converter's
   **output**, so it ranks only what is still unported — and it no longer counts a way back
   gated on a memory as open. It had: that made Mist Harbor a hub between every town and
   ranked the FWI trinket at +9,843 rooms, which is exactly what the author said the
   trinket does *not* do (it returns you to the town you came from). Corrected, the top
   bridge was the portmasters.
   - **Portmasters**, 62 exits: ask, ask again, then `Action::Await` the landing line. A
     step's patience is scaled from the exit's cost (1,200 s here).
   - **Resolve, then a hard climb**, 26 exits on the roads to the Nations; and the **arctic
     waters**, 16: cast Resolve and Water Walking if able, then `go` if Water Walking is up
     and `swim` if not. That second question is asked *after* the cast, so the rule is now
     written down: **a step's guard is asked when the step is reached**, not at planning.
     Upstream names these spells by number (9704, 112); the map names them, checked against
     `cena-model/data/spells.tsv`.

   | | knowing nothing | with paid services on |
   |---|---|---|
   | rooms reachable from Wehnimer's | 10,118 | **16,859** (was 10,123) |

   Unported crossings **2,666**. Next by rooms opened: the FWI trinket (a routine, §4.4),
   inn tables (`Await` with a follow-up), the rogue guild password, hands.

   **THE TWO KINDS OF CHECK — DECIDED (author, 2026-09-21).** *"If the answer makes the
   room unpassable then it needs to be during planning; if the answer just changes the way
   of traversal then it can be checked at the room."* That is the line between the two
   places a `Cond` can sit, now written down and enforced:
   - **the exit's cost** (`Cost::Gated`) is asked **while planning** and decides *whether*;
   - **a step's `when`** is asked **in the room** and decides *how* — and may never take the
     way across away. `cena_map::moves_whatever_is_known(steps)` checks a step list against
     a walker about whom *nothing* is known, and the ratchet fails any ported crossing that
     does not pass it. It found one at once: the arctic waters' `swim` was guarded by
     `not water_walking`, which is unknown for an unknown walker, leaving no move at all.
     Hence **`Cond::Otherwise`** — true unless the inner question is *known* to hold — for
     the second of two ways across.

   **Sixth batch, 2026-09-21.** `Action::{EmptyHands, FillHands, Forget}`. Arms: the inn
   tables (478); `empty_hands; move; fill_hands` six ways; `multifput 'a', 'b'` (send all
   but the last, move on the last); `move('X')`; the way out of an event ground, which
   forgets the way in; `pause; waitrt?; fput 'climb rock'` and `dothistimeout 'push south'`,
   both of which are just moves. **One requirement handed to the walker:** an inn table
   answers either "you head over" or an *invitation*, and upstream sends the command again
   on an invitation. That is a reaction of the move itself, like opening a door that turned
   out to be closed, so it is one entry in the walker's reaction table — not 478 copies.
   `recognise.rs` passed the 800-line cap and is now a facade over `recognise/{moves,
   routines, costs, tests}.rs`.

   | | knowing nothing | with paid services on |
   |---|---|---|
   | rooms reachable from Wehnimer's | **10,491** | **17,451** |

   Unported crossings **1,903** in 325 shapes; unported costs 739 in 48.

   **Seventh batch, 2026-09-21: the Rift.** `Routine::Patrol { starts, dirs, landmarks,
   after }` — walk a fixed circuit until a way out appears among the room's objects, then go
   through it. **570 exits, five spellings of one script, one arm.** The two tables are
   different lengths upstream and are kept as they are, `nil` gaps included, because
   position is the meaning. A fissure must be worked open first (`Opening`: `push fissure`
   until the game says it cannot open any farther, five tries). Upstream's `else; echo
   'error…'` and its closing `$go2_restart = true` are what *Patrol* means — lost, or landed
   somewhere unknown, so replan — and are not arguments. Vellum lowers this shape into
   nested repeats of steps (`GuidedRoute`); here it is a routine, because §4.8 keeps loops
   out of map data. **Unported crossings 1,903 → 1,333.**

   **The long tail is being ported in parallel** (2026-09-21): the remaining shapes were cut
   into four disjoint slices (`research/mapdb-inventory/tail/`, `slice_tail.py`), each with
   its own empty arms file already wired in (`recognise/tail_{a,b,c,d}.rs`), and a written
   brief (`tail/BRIEF.md`). The vocabulary is frozen for that work: a shape that needs a new
   primitive is skipped and reported, never approximated. Every arm is read against the
   upstream Ruby before it is merged.

   **Slice D (costs), merged 2026-09-21.** 38 exits in 7 arms: premium, Water Walking,
   settings compared with a value, settings merely named (private property: `Shivergale`,
   `sunset_cabin`), a known spell, skill floors. Reviewed against the Ruby; **one
   correction**: `checkspell(112) ? 0.2 : 2.0` was ported as "when Water Walking is up",
   which prices a walker of unknown spells *impassable* though both branches are passable.
   It is a how-long, not a whether, so it is now `Otherwise` and never refuses.
   **Unported costs 739 → 701**, and the rest is a vocabulary list, not a porting list:

   | exits | needs |
   |---|---|
   | 477 | the instability table (`$mapdb_instability_timeto`) — a lookup in `Walker` |
   | 103 | the walker *sitting* **and the room's climate** — the first room-fact a cost has asked for |
   | 39 | society and rank (Voln seeking) |
   | 19 | arithmetic over ranks, level, skill and encumbrance — the formulas |
   | 8 | an inventory item check (a key) |
   | 8 | "is hunting/wandering automation running" — a planner flag, not yet named |
   | 7 | citizenship · 4 race · 2 gender · 2 level |
   | 4 | `$SILVERWOOD_TOWN`, a global some other script writes — origin unknown |
   | 4 | the *current room's* title or location, beside a memory (Red Forest, Hinterwilds) |
   | 2 | a three-way price (`if … elsif … else`) |

   OPEN, from the agent's own doubts: `X.nil?` is ported as *is set*, which also treats an
   empty string as unset; several names (`car_to_sos`, `Mularos_Lover`…) are ported as
   settings because upstream only ever reads them; skill names are upstream's lowercase
   and the planner must map the model's to them.

   **The long tail, ported in parallel and merged 2026-09-21.** Four agents, one slice and
   one file each, in separate worktrees; every arm read against the Ruby before merging.
   **What review found was gaps in the vocabulary, not mistakes in the ports** — the agents
   named them as doubts, which is what the brief asked for:
   - **`Action::Replan`** did not exist, so slices B and C had *dropped* `$go2_restart =
     true` — on exits that really do land at random (`go ring` reaches eight rooms). Now a
     step, which must end the crossing.
   - **`Action::KeepMoving`** did not exist, so rowing, the Red Forest's fog and the pedal
     boats were one `Move` — which gives an exit up after a few tries, where these are
     written to be tried until they work. With it the pedal boats (173 exits) ported at once.
   - A Water Walking *cost* refused a walker of unknown spells though both prices are
     passable; it is now `Otherwise`. The two-kinds-of-check rule applies to costs too:
     a gate whose branches are both passable must never be unanswerable.
   - Slice B's largest arm is a **statement-list matcher** (`fput`/`move`/`sleep`/hands,
     each spelled exactly, separated exactly) rather than one template per shape. Accepted:
     it is closed — anything not listed does not match — and three dozen near-identical
     templates would say the same thing worse.

   | | start of step 7 | now |
   |---|---|---|
   | unported crossings | 7,923 | **702** |
   | unported costs | 1,860 | **701** |
   | reachable from Wehnimer's, knowing nothing | 6,969 | **10,763** |
   | reachable with paid services on | — | **17,994** |

   **What is left is a vocabulary list.** From the four reports, by exits unlocked:
   | needs | exits |
   |---|---|
   | the instability table (cost) | 477 |
   | the walker's **posture** (sitting/kneeling) — rowboats, crawls; with the room's climate for 103 costs | ~230 |
   | loop until a **named** room is reached (random landings: `go forest` × 50) | ~75 |
   | "the move failed → do this, then retry" (lockers, levers) | 43 |
   | obvious-exits checks / a random exit (`checkpaths`) | ~60 |
   | race · citizenship · level · society rank · gender | ~95 |
   | item in inventory or among the room's objects | ~35 |
   | `Await` any of several lines; wait for the room to change by itself | ~20 |
   | a group-member wait — trivial if the author rules group waits are dropped | 10 |
   | `$SILVERWOOD_TOWN`, a memory upstream keeps in a global — proposed name `silverwood_town` | 12 |
   | puzzles that branch on what the game said | ~60 |

   **Who the walker is (2026-09-21, author: "your recommendation is fine").**
   `Walker` gains `race`, `gender`, `level`, `citizenship`, `society`, `society_rank` and
   `posture`; `Cond` gains a question for each. `Race` asks for a *word* of the race, as
   upstream's `=~ /Gnome/` does, so a Forest Gnome is a Gnome. Arms (`recognise/facts.rs`):
   the lake's rowboats (row if seated — `KeepMoving` — swim `Otherwise`); low crawls (kneel
   unless kneeling or short — `Otherwise`, so an unknown walker kneels, which always works);
   citizens-only, race, gender, level and the Order of Voln's gates.
   - **A room fact in a cost is settled by the converter.** `checksitting &&
     Room.current.climate == 'freshwater'` (103 costs) asks about the *room*, which the
     converter is holding: off fresh water the test is false and the cost is a constant; on
     it, only the walker's half survives into the map. No room facts in `Cond`.
   - Of the two boat costs, the one whose branches are both passable is `Otherwise` and
     refuses nobody; the one that can refuse (`? nil : 0.2`) refuses an unknown walker too.
   - **A requirement on the walker:** `Move` stands a walker up first. After a `kneel` step
     it must not — a crossing that sets a posture keeps it for its own moves.

   Unported crossings **583**, costs **544** (477 of them the instability table);
   reachable 10,763 / **18,012**.

   **The instability table (2026-09-21).** 477 costs, `$mapdb_instability_timeto[188]`:
   nine towns from each of the Confluence's 53 rooms. READ in `go2.lic:1623-1680`: when the
   walker enters the plane, go2 records the instability it came in by
   (`$mapdb_last_instability`) and fills that global with `Map.estimate_time` of the walk
   from it to each of nine towns — because **leaving by the point of tranquility puts the
   walker back at that instability**, so that walk is what the way out really costs.
   **`Cost::Table { table, key }`** reads `Walker::tables`, which the planner fills the same
   way: one search from the instability room (~1.6 ms, §5 step 5). A missing table or town
   is impassable, as upstream's `nil` is. **Unported costs 544 → 67.**

   On the game's numeric terrain and climate codes (`<roommeta>`), which the author raised:
   not worth using. The one climate test upstream makes is resolved by the converter from
   Lich's string and never reaches the client; and the code-to-name table is not known.
   The model keeps the codes undecoded, so a table can be had for free later — every mapped
   room a character enters pairs a code with the mapdb's word for it.

   **The reserved routines (2026-09-21).**
   - **`Routine::Signposts`** — the underwater route off River's Rest, 75 exits: each room
     says which way to go from it, until the walker is at the exit's destination. Lost
     (a room not in the table) means replan; upstream swims at random.
   - **`Action::MoveUntilThere`** — a command sent until the walker is *at the exit's
     destination*, 68 exits: a forest whose way in lands somewhere different every time.
     `KeepMoving` stops at the first change of room; this stops at the right one.
   - **`Routine::Seeking`** — Voln's symbol, 36 exits. The destination is always the exit's
     own, so it is not an argument. The upstream script also writes `redforest_location`
     by which of two rooms the walker sought from; the converter knows the room, so that
     is settled offline into `remember`.
   - **`Routine::Trinket`** — 31 exits: the script itself (kept verbatim) or a call to it.
   - **`Routine::GuildPassword`** — 9 rogue guild doors. **Upstream's script refuses a
     walker with no password halfway through the crossing** (`echo …; exit`). A check that
     can refuse belongs to planning, so `priced_for_crossing` moves it into the exit's cost:
     the door is priced only when the profile has a password, and nobody is walked to a
     door they cannot open.

   Unported crossings **367**, costs **67**. Reachability did not move: every one of these
   is behind a setting or a society the ratchet's two walkers do not have.

   **CORRECTED 2026-09-21 — everything is ported.** I suggested the puzzle rooms might be
   left unported because "a client without scripting should not be solving a one-off
   puzzle room". The author: *"a client without scripting just means without external
   scripts ... we're programming primitives and behaviors here."* The settled decision
   (`CLAUDE.md`) is that **users** do not author scripts; it puts no ceiling on what Hydra
   itself does. A puzzle is a routine like any other: named in the map, written in Rust.
   The target for unported crossings is **zero**, and what remains is a list of routines
   and primitives to write, not a list of things to give up on.

   **Reactions (2026-09-21).** Crossings that depend on what the game does next
   (`recognise/reactions.rs`), and the vocabulary for them:
   - **`Action::TryMove` + `Cond::StillHere`** — a move that may well not work, and what is
     left to do if it did not. Locker alcoves (30 exits: try the curtain; if still here,
     `close locker` and go) and doors that open on their own schedule (13: try; if still
     here, do what opens it, `Await` the line, go). Not moving is an answer, not a failure,
     which is what separates it from `Move`. Flat steps hold: the "if" is a guard.
   - **`Cond::Exit`**, the room's obvious exits, asked in the room; **`Action::MoveWhile`**
     (`northeast` while there is a `nw`) and **`MoveByAnyExitBut`** (a room with two ways
     out, entered by one).
   - **`Action::AwaitArrival`** — nothing to send, the walker is being carried; and
     **`AwaitAny`**, for the Hinterwilds caravans, whose halt comes in three wordings.

   Unported crossings **284**; reachable **11,077** / **18,871**.

   `reachable` prices with `as_converted`, so it counts no gated exit at all — a Bard's map
   is larger than this number, and a second figure for a *described* walker is worth
   adding once the urchin and portmaster gates are in.

   Still to come, in rooms-opened order (§5 step 5's table): `move S` and its kin, the
   profession cost gates (the first `Cost` arm — `Gated`), inn tables (`Await`), then the
   pairs that only open a region together. Nothing *walks* these yet: the Travel behavior
   (§4.0) is its own piece of work in `cena-behavior`.

   Primitives, then recogniser arms, in **rooms-opened** order (was: edge-count order). A new arm that needs no new
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
