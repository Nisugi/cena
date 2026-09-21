# VellumFE travel executor: execution semantics

Digest written 2026-09-20 by the main session from a research subagent's report (the
subagent's own write was cut short). Evidence, not instructions. Claims are
VERIFIED-by-reading by the subagent unless labelled INFERRED.

Citations are to `reference/VellumFE/src/core/travel/executor.rs` unless prefixed:
`mf` = `core/move_feedback.rs`, `tt` = `core/app_core/state/travel_ticks.rs`,
`mod` = `core/travel/mod.rs`, `edge` = `core/pathing/edge.rs`, `tp` = `core/pathing/transpile.rs`.

Read in full: executor.rs 1-6654 (tests from 3952), edge.rs, move_feedback.rs, travel/mod.rs,
travel_ticks.rs. NOT READ: stash.rs, day_pass_buy.rs, confluence.rs, minotaur.rs, mazes.rs,
overrides.rs, the Dijkstra, most of transpile.rs, the room resolver.

## 1. State machine

- Pure, event-driven, no sleeps (1-16). `TravelTask::tick(ctx) -> Vec<TravelEvent>` (787); the
  caller builds a `TravelContext` snapshot per tick (25-108).
- Events out: `Send`, `Status`, `Arrived`, `Failed`, `DisableDayPassBuy`, `LichFallback`
  (343-359). A finished task ends with Arrived/Failed/LichFallback (3843).
- Ticked after every network line AND once per frame (tt:8-11) so timed waits progress when
  the game is quiet. `start_travel` ticks at once (tt:875).
- Inputs: resolved mapdb room id (`None` = unresolved -> hold, 829); typed `MoveFeedback`
  from an aho-corasick matcher, leftmost-longest, one per line (mf:373-386), stamped with a
  line number and drained once per tick (tt:182); `<nav>` -> `NavArrived` + `nav_count`
  even in unmapped rooms; a 64-line raw ring for `Await` (copied, not drained); roundtime
  remaining, with a backstop that parses "...wait N" from the ring (3877).
- Steps (363-461): Prepare, AwaitStand, RunScript, Stashing, Funding, AwaitArrival
  {expected, from, sent_ms, slow}, Maze, Confluence, Minotaur, ScriptWalk, DayPassBuy,
  DayPassUse.
- Tick order (811-1161): account outcomes -> dead => Failed -> unresolved room => hold ->
  at destination => Arrived (unless funding detour, stash running, or in a day-pass step)
  -> dispatch.
- On Failed/handoff with a non-empty stash stack, retrieves are emitted LIFO first (787-809).
  **User stop skips this cleanup** (mod:65-68).
- Start (683-741): plan; `silver_need` from `silver-cost` tags; >0 starts in Funding.
  Pre-flights in `start_travel` (tt:690-882): lease gate, flag sync, deferred `urchin status`
  probe (4 s deadline), day-pass sack scan (`look #id` paced 700 ms; skipped on escort
  bounties), four guards (map loaded, room resolved, not already there, destination exists).

## 2. Crossing a plain edge

`tick_prepare` order (3191-3309): hold backoff -> muckled wait -> path exhausted => repath ->
maze boundary -> confluence boundary -> look up wayto (missing => repath) -> stand if needed
(not standing, not mounted, not waived, command lacks the word swim/pedal) -> RT>0 return ->
override -> day pass -> proc -> plain send, `AwaitArrival{slow:false}`.

- Stand: `STAND_TIMEOUT_MS 2500`, `MAX_STAND_ATTEMPTS 5`; a `StandBlocked` line waives it.
- **Arrival = resolved room id equals expected** (1035). No title check.
- `meta:transport` rooms: wait silently; leaving into an unexpected room => repath.
- Landed somewhere that is neither `from` nor `expected`: if `slow`, keep waiting (escorts
  walk you through rooms); else off-route -> clear pending, `owed = 0`, orphan window
  1500 ms, repath (1072-1091).
- `RtWait`: hold N*1000+200 ms, back to Prepare, **not counted as a retry** (1114).
- Timeouts: `STEP_TIMEOUT_MS 8000`, `SLOW_ARRIVAL_TIMEOUT_MS 30000`. After
  `MAX_EDGE_RETRIES 2`: **ban only if `idx == 0`** (never left the first room), else keep the
  edge; repath either way (1127-1157).

**The command gate** (3598-3676). `gate_send` stamps line number + nav count, stores
`pending_cmd`, increments `owed`. `gate_fput` owes nothing. `gate_resend` re-emits
`pending_cmd`, **never the wayto text**. Outcome accounting is FIFO: a failure with
`owed > 1` belongs to an older send and is retired silently; with `owed == 1` and a fresh
line number it becomes attributable; with `owed == 0` it is swallowed.

**`recover_from_feedback`** (3315-3580), only while still in `from`. Gates in order: a nav
seen since the send wins (consumed once, retries reset); inside the orphan window => ignore;
not attributable => only record Mounted.

| Event | Reaction |
|---|---|
| Mounted ("You cannot do that while mounted") | trip-long `mounted`; no standing; urchins off; repath |
| MoveFailedRemovable (mf:328-353: "You can't go there", "Where are you trying to go", "I could not find what you were referring to", "How do you plan to do that here", "is too far away", "You may not pass", …) | resend up to 2x (RT 0 only), then BAN + repath |
| MoveFailedKeep (mf:354-368: "An unseen force prevents you", "aren't allowed to enter here", "I'll need to see your ticket", "Only members of registered groups may enter", `reads, "Abandoned."`, …) and TransientRetry (mf:226-246: swim/struggle/vertigo/entangle lines) | up to 4 retries, 1200 ms hold each; then repath **without** ban |
| MustUnhide (mf:184-190) | `unhide`, resend |
| MustStand (mf:191-206) | back to Prepare |
| TypeAhead / NoControl | hold 1200 ms; no retry consumed |
| StillRecovering | hold 2000 ms |
| StillStunned | back to Prepare (muckled gate waits) |
| TooInjured | repath, edge kept (Lich would cast Resolve) |
| PitchDark | status + repath, no ban. **Lich treats this as success** (move.rb:431) |
| HandsFull (mf:149-156) | stash, then resume |
| DoorClosed | first: `open` via go/climb->open substring swap, resend; second: BAN "locked" |
| Fell (mf:162-180) | hold 800 ms, back to Prepare |
| NeedClimb / CantClimb | swap go<->climb in `pending_cmd`, resend |
| ItemAtFeet | `stow feet`, resend |
| TooPoor | handled inside Await (2035) |

## 3. The 23 primitives at runtime

`tick_script` (1740-2509) walks `actions[pc]`, suspending by storing `RunScript`. At script
end (1762-1788): anything sent => `AwaitArrival{slow:true}`; nothing sent => `pass_through`
(2628-2685), which collapses a virtual `;e true` room by sending that room's wayto to the
route's NEXT room while anchored at the physical origin. No off-route check while a script
runs (1009-1017).

1. **Noop** — advance.
2. **Move** / 3. **Put** — **identical at runtime** (1791-1806): expand captures, fire and
   forget, NOT RT-gated, no wait. `fput X; move Y` emits both in one tick. The "expects a
   room change" distinction exists only via end-of-script arrival watching.
4. **StepMove** (1807-1825) — RT/hold gated, owes an outcome, enters `ScriptWalk`
   (931-993), which resumes on room change, a nav (multi-uid rooms share one id), an
   attributable failure, or 30 s. **On failure it proceeds** to the next action ("the Ruby
   scripts' moves also proceeded on failure"). No capture expansion.
5. **MoveExitExcept(dir)** — first compass exit that is not `dir`; none => uncrossable.
6. **MoveAnyExit** — uniform RANDOM exit (`rand::rng()`, 3947); the enum doc says
   deterministic; the code wins. Non-deterministic: conflicts with Cena's replay rule.
7. **WaitRt** — block while RT > 0 or a hold is active.
8. **Sleep(s)** — non-blocking deadline.
9. **If** — branch spliced in place at `pc`; no branch stack.
10. **EmptyHands** / 11. **FillHands** (1908-1962) — drive `StashTask::empty()` /
    `::fill(stack)`; `_drag #id #bag` to stow, `get #id` to retrieve, LIFO. With no hands
    input, the bare strings `stow both` / `get both`. The trip cannot end mid-stash (847).
    `StashContext` (165-193): both hands, ready_stow, weaponsack, lootsack, other containers,
    bandoliers (always None), is_weapon flags.
12. **Await** (1963-2097) — `cmd` sent raw (not through the gate). First ring line newer
    than arming that matches; named captures bound, last write wins; `if_match` on the SAME
    line splices its steps. `TooPoor` while waiting => one bank detour with
    `GENERIC_FARE_SILVERS 2000`. Timeout: Continue / Retry once / Fail. Passive form never
    sends.
13. **Repeat** (2098-2125) — budget `min(max, MAX_SCRIPT_LOOP 50)`, per node (nested loops
    multiply). `until` tested before each pass. A body that never suspends runs all
    iterations in ONE tick (test 5028: 50 sends in a tick).
14. **Break** — drains forward to the next Repeat node; no-op outside a loop.
15. **GuidedRoute** (2141-2234) — landmark present => `Move(enter)`; else lower to nested
    Repeats of StepMove from the room's offset; `GUIDED_ROUTE_LAPS 2`.
16. **VolnSeeking** (2235-2325) — regex of ALL destination titles, brackets and trailing
    ` (digits)` stripped; up to `SEEKING_MAX_CASTS 20` of WaitRt + Await("symbol of
    seeking", 6 s) then `symbol of seeking confirm`.
17. **SetVar** — writes a PROCESS-GLOBAL map (2334).
18. **TrinketWarp** — get (emptying hands if needed), `turn #id`, put back, Replan.
19. **TryMove** — StepMove, then `If InRoom(here)` run the fallback.
20. **RouteTable** (2390-2449) — room not in table => REPATH (the Ruby picks a random
    exit); EmptyHands in `hands_free_in` rooms; never FillHands itself; bounded at 50.
21. **MinotaurMaze** — hands off to a dedicated step; learns adjacency; exits changing =>
    maze shifted, relearn.
22. **PauseForUser** — if `until` already true, continue; else status + uncrossable.
    `timeout` is IGNORED; it never actually waits.
23. **Replan** — skipped if already at `expected`; otherwise repath.

## 4. Cond evaluation (197-239)

**Unknown always answers FALSE** — "refusing a route is recoverable, walking one you can't
take strands you". SpellActive reads the ActiveSpells dialog (absent => false).
PathAvailable needs the SHORT dir form. RoomHasObject matches item NAMES, exactly, despite
its doc. Citizenship/Profession/Society `None` => false. HasItem is a lowercase `contains`
over inventory + containers. CaptureIs only works inside `If`; always false in
`Repeat until` and `PauseForUser`.

## 5. Bans and repath

- `banned: HashSet<(from,to)>` lives on the task = per TRIP, no expiry. Only `repath`
  applies it; the initial plan does not.
- Ban sites: silent timeout at `idx == 0`; Removable after 2 retries; second DoorClosed;
  uncrossable edge with no fallback; day-pass cases.
- `repath` (3708-3816): `MAX_RESTARTS 10`; **recomputes `silver_need`** and re-enters
  Funding; on a funding detour the target is the BANK.
- Other bounds: `MAZE_MAX_ATTEMPTS 3`, one `open` per door, one bank detour each for
  pre-flight and mid-script, the orphan window, FIFO `owed`.

## 6. Dispatch order

maze -> confluence (entry only) -> stand -> override -> day pass -> proc -> plain
(`tick_prepare` 3213-3301). Overrides beat the mapdb and run as scripts.

## 7. Globals that break multi-session

`MAPDB_VARS` (tp:2682; written by SetVar and EVERY TICK with rogue password + platinum
flag); `USE_SEEKING`, `USE_PORTMASTERS`, `URCHINS_VALID` (also mutated mid-trip on Mounted);
`DAY_PASS_ROUTABLE`. **Root cause: `resolve_timeto` is context-free and called by Dijkstra.
For Cena, routing context must be passed into the pathfinder as a value.** `TravelTask`
itself holds no globals. Wall clock is read in the tick path (day pass, urchin expiry).

## 8. Character states

No group awareness at all. Hidden: reactive `unhide`; also before a bank withdraw. Mounted:
learned only from the rejection line. Dead: immediate Failed. Muckled (stun/web/bind/sleep):
waited out in Prepare/Maze/Confluence/Minotaur but NOT inside scripts. Encumbrance: only the
"overburdened … cannot stand" line. RT gates the paced moves; Move/Put/Await.cmd are not
gated. Silver: `wealth quiet` probe trusted only if its line number is newer than the probe;
re-probe 8 s x2; short without permission => 10 s grace then walk anyway; short with
permission => nearest affordable bank by ONE multi-target Dijkstra, `withdraw N silvers`
(Pinefar: `ask banker for max(N,20) silvers`), per-town accounts so a failed withdraw is
fatal.

## 9. Lessons recorded in the code — the most valuable part

- Stamp every send with a line number: a stale "You can't go there" re-triggered the
  current move (the creek loop) (60-63, 609-613).
- Retry re-emits what was SENT, never the wayto: raw resend got "Please rephrase" (618-622).
- Orphan window: an abandoned in-flight command's outcome double-sent its replacement
  (623-629).
- FIFO `owed`: a one-command type-ahead bubble sustained itself because each rejection
  triggered a resend (Whistler's Pass) (630-636).
- Never end a trip mid-refill: fill_hands got 1 of 2 items (836-840).
- Multi-uid rooms are one id over many physical rooms; waiting on the id alone stalled
  30 s per step (963-970).
- No off-route check during scripts: the Glo'antern mist trail climbs a MAPPED room.
- **Banning on lag poisoned good edges**; keep the edge unless the walker never left.
- Arrival beats a raced failure line; consume the nav once or the walker hangs silently.
- Do not ban on the first failure line: it may be ambient text.
- A missing `unhide` is "the most common wrongful ban for stealthy characters".
- The uncapped open-door loop was "the only outright hang in the walker".
- Counting fputs in `owed` starved real failures of attribution.
- An exhausted seek teleported the character to a random offered room (2294).
- Titles stamped with a room number never match: strip " (12345)" (2236).
- An unguarded trailing replan reports a completed trip as failed (2486).
- Stale cached silver marched a broke character into a paid crossing (2913).
- Value-diff freshness missed a withdraw that left the total unchanged (3108).
- 72 Dijkstras froze the UI 10-20 s: use one multi-target search (3161).
- A repath through a toll kept `silver_need` at 0: recompute it (3781).
- Arrival-watching a `;e true` virtual hideout caused "3637 -> 30718 keeps failing".
- Pattern hygiene (mf): "in that direction" and "You cannot do that" matched combat prose;
  "You slip"/"You tuck" collide with fall lines; flavoured containers answer `open` with
  custom prose, so use `<exposeContainer>`; a burst of `look` trips the type-ahead limit.
- Passive await is required for non-idempotent commands; named captures only; an unbound
  capture yields None, never an empty string.

## 10. Workarounds a fuller model would not need

The whole `move_feedback` substring scanner (in Cena: a stateless classifier over frames);
the raw-line ring and its RT backstop; the Lich `;go2` handoff ("bandaid"); `mounted`
learned from a rejection; silver known only from `wealth` lines; conditions answering false
for unknown; Pinefar detected by title substring; the lagging resolved room (keying on uid
removes the multi-uid stall class); probe commands that defer the trip; the process globals.

## Bug-shaped items — decide, do not copy

- Await `Retry` re-sends the UNEXPANDED command (2078).
- Paced moves skip capture expansion.
- `Break` targets the next Repeat forward, not necessarily the enclosing one (2130).
- After a HandsFull stash the move is not re-sent until the 8 s timeout (INFERRED).
- Return-trip fare `+=` runs on every successful wealth check (2950) (INFERRED).
- `RepeatUntil::RoomChanged` compares against the edge origin, contradicting edge.rs:290.
- User stop skips stowed-item retrieval.
