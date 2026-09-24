# 30 — Milestone 6: the first real behavior

**Status: AGREED (author, 2026-09-24), answers recorded in §6.** What
the author decided is marked as theirs; everything else is still a proposal.
`plan/12` wins any contradiction.

`12` §8 gives M6 one line: *"first real behavior (Hunt), profile format (§6a.1), supervisor
composition"*. This document turns that into a slice that can be built and demonstrated.

---

## 0. What is already settled, and by whom

| Fact | Source |
|---|---|
| Automation is **curated Rust behaviors configured by data profiles**; no scripting runtime for now | `CLAUDE.md`, Settled decisions |
| **Profiles express policy, not programs**: no conditionals, loops, variables. Presets are the front door, shareable as one file | `12` §6a.1 |
| **Settings inherit global → profile → character**, from the first setting that exists | `12` §6a.2 |
| **One holder of the command authority**; Hunt/Heal/Travel are composed inside one supervisor, not raced | `12` §4.2 |
| **Manual input interleaves, never preempts**; only an explicit stop/pause does | `12` §4.1 |
| Every await is cancellable; every wait has a deadline; a wedged behavior is detectable | `12` §5.5 |
| **Two clients on one character: Hydra backs off** rather than fight | author, 2026-09-24 (`29` §5b) |
| SE-4 (authority across generations) is decided in M6 | author, `29` §5 Q6 |
| Cross-session features (follow the leader, group moves, shared loot) were deferred to M6 | `29` §2 |
| Hunt ports **eohunter** (the author's own), which reads bigshot 5.16 profiles | `C:\Gemstone\eohunter` |

---

## 1. What already exists

**The reading side is mostly built.** A behavior folds `Event::Frame` into its own
`GameState` copy (`crates/cena-behavior/src/travel/drive.rs`), and the model already answers:

| A hunter asks | Cena answers with |
|---|---|
| what is in the room, alive or dead | `state.creatures()`: `targets()`, `dead()`, `valid_target()`, `hp_percent()` (`cena-model/src/state/creatures.rs`) |
| did it flee, or hide | `creatures().fled(id)`, `vanished_unaccounted()` (`state/departure.rs`) |
| what the game targets | `state.targeting`: `current()`, `is_targetable()` |
| am I in roundtime | `in_roundtime()` → `Option<bool>` (`state/clock.rs`) |
| stunned, webbed, prone, dead | `status.known().stunned()` and friends → `Option<bool>` (`state/status.rs`) |
| health, mana, stamina, spirit | `state.vitals` → `Option<Vital>` |
| mind state, encumbrance, stance | `character.experience.mind_percent`, `encumbrance_percent`, `stance_typed()` |
| wounds | `Injuries::new(..)`: `able_to_cast()`, … (`character/injured.rs`) |
| is this room mine | `claim::claim_room` |
| what my attack did | `Event::Combat(ChunkFacts)` from the combat tracker |

**Travel is built** (`24`), as a pure `Trip` state machine with a driver around it. A hunt
walking to its area and back is travel.

**The acting side is thin.** What Hunt needs and does not have:

| Gap | Where it shows |
|---|---|
| A stance **setter** | `character/stance.rs` reads stance and leaves `change`/`safest` for M6 |
| An `in_casttime` reading | only the raw `cast_time_ends` field exists |
| Anything that runs travel **under an authority it already holds** | `travel()` claims for itself, and claims are not re-entrant (`command/queue.rs`) |
| `BEHAVIOR_WATCHDOG`, and the forced revoke after `PREEMPT_GRACE` (§4.3) | neither is built; both are named only in comments and tests |
| SE-4 | the supervisor drops the authority with each actor (`supervisor/core.rs`, `Inbox::Claim`); a behavior that waits out a reconnect is refused `Permanent` (`19` §4b) |
| **Behavior traffic counts as attendance** | attendance is `recorder.outbound_count()` (`supervisor/core.rs`). A hunt sending forever makes a session look attended forever, so the two-client backoff the author chose never binds (`19` §5, "The ladder resets on any outbound byte") |
| A profile format and the inheritance chain | `settings_store` is per-character only; `travel_store` has two levels, for targets only |
| A second `;` command | `commands.rs` has one slot, `travel` |
| `creature_message.rs` wiring | assigned to "a hunting behavior (M6)" by `27g` and `27d` |

---

## 2. Step 0 — clear out the M1 scaffolding

The author, 2026-09-24: *"We should probably get rid of any test behaviors, like look."*

What the M1 slice left behind, measured:

| Thing | Where | What it was for |
|---|---|---|
| `look` behavior | `cena-behavior/src/look.rs` (311 lines) | criteria 4-5 of `12` §7.2, a looping behavior safe to send forever |
| `--demo` | `cena/src/main.rs` `run_demo`, `interleave_manual`, `stop_the_behavior` | sends `look` each second for the live walkthrough |
| `--capture`, `--psm`, `--typeahead` | `cena/src/run.rs` `Script`, `cena/src/probe.rs` + `probe/ladder.rs` (523 lines) | measurement probes for the clock, PSM fixtures and refusals |
| `--hold` | `cena/src/main.rs` | how long the one-character walkthrough stays up |
| the one-character path itself | `cena/src/main.rs` `main` (with no `--character`) | M1's criteria, narrated as it runs |

**Proposed:**

1. **Delete `look`**, `--demo` and the walkthrough functions. Move `BehaviorError` (it lives
   in `look.rs` and travel and sync use it) to `cena-behavior/src/error.rs`.
2. **Keep criteria 4 and 5 tested.** `stop_and_interleave.rs` and
   `authority_is_released.rs` test the **session**: stop latency, interleaving and release.
   They move onto a looping behavior **defined in the test support**, not shipped. The
   falsification each one records still has to go red. `room_shapes.rs` keeps its matcher
   locally.
3. **One run path.** Without `--character`, Hydra asks which character to play and runs the
   same table as `--character` (`play.rs`). `main.rs`'s single-session path goes, and so do
   its criterion-by-criterion banners.
4. **Delete the probes** (author, Q1): `--capture`, `--psm`, `--typeahead`, `probe.rs` and
   `probe/ladder.rs`. Git keeps them.
5. **`sync` runs at Ready**, for stale groups only, **quietly, the way infomon does**
   (author, Q2): one status line per stage, so the player knows what is running and that it
   is not stuck, with the commands' game output kept out of the story.

> **BUILT 2026-09-24.** `look` lives on as `cena-behavior/tests/looping/`, and a bare
> sleep substituted for its cancel-aware one still turns
> `stop_stops_the_behavior_within_preempt_grace` red. `BehaviorError` is `error.rs`; the
> terminal's view is `cena/src/watch.rs`; the `--web-login` switch moved to `connector.rs`.
> Also gone as dead code once the single path went: `Frontend::start`, `wait_for_stop`,
> `unless_interrupted`, `setup::open_session`, and a single session's terminal story
> renderer. With no `--web`, a run is now headless, with Hydra's own lines only, as it
> already was under `--character`.
>
> **`sync` at Ready, BUILT 2026-09-24.** The session gained **quiet round trips**:
> `SessionHandle::send_quietly` brackets a command's window with `Event::Quiet(true/false)`,
> and the web story leaves that window's main-stream text and prompt out. Other streams
> still show, and every frame is still folded, logged and published. `learn.rs` hears
> `SyncNeeded` from a subscription made before the session runs, waits for `Ready`, and
> runs `cena_behavior::sync`, which sends quietly and says a start line, one line per
> command naming it, and an end line. `tests/web_quiet.rs` goes red if either half is
> broken (both mutations verified), and `sync_plan.rs` goes red if the sync sends loudly.

Done when: `grep -rn "look(" crates/*/src` finds no behavior, the binary has one run path,
and the whole suite is green, including the two re-hosted criteria tests with their mutations.

---

## 3. The shape: eohunter's engine *is* `12` §4.2

eohunter's real loop is `Engine` (`runner.rb`), not `controller.rb`, which only supervises
agent-bridge runs. Each tick, the engine asks the behaviors **in priority order** whether they
want control. The first that does sends **at most one action**. No behavior keeps position:
intent is re-derived from the world every tick (`behavior.rb:10-13`).

| Priority | eohunter behavior | M6 |
|---|---|---|
| 0 | Survival: dead → stop, prone → stand | **in** |
| 5 | Cleanse | out |
| 10 | Flee: too many, or never-fight creatures | **in** (count and names only) |
| 15 | Muster (group) | out |
| 20 | Rest: fried / out of mana / encumbered → walk to rest, wait, walk back | **in** |
| 30 | Loot: `loot #id` | **in** (no loot scripts) |
| 35 | Loadout | out |
| 40 | Maintain (signs, bless) | out |
| 50 | Engage: choose a target, run the routine | **in** (one plain routine) |
| 60 | Wander: bounded random walk | **in** |

That is `12` §4.2 exactly: one holder of the authority, composing sub-behaviors as policies
and deciding priority in its own logic. It also answers `12` §9's rule-of-three question:
there are **six** policies under one holder, not a second behavior racing the first.

**Proposed Rust shape**, following travel, whose split has already paid off:

- **`Hunt` is a pure state machine**: `tick(&GameState, &Profile, Now) -> Said`. It has no
  socket and no clock, and is seeded. Replay tests drive it frame by frame, the way
  `travel_trip.rs` drives `Trip`.
- **The policies are an enum, not a trait.** The set is closed and curated, and a `match` in
  priority order is the arbiter. A `Behavior` trait still waits for something that must
  schedule behaviors it cannot name (`cena-behavior/src/lib.rs`), which M6 does not have.
- **One driver** owns I/O: claim, send, settle, fold events, race every await against stop.
  It is travel's driver generalised only as far as Hunt needs.
- **Travel runs inside Hunt** as a sub-trip under Hunt's authority. That needs one change:
  travel's drive loop is split from its claim, so the caller that already holds the authority
  can run it. `;go2` keeps the claiming wrapper.
- **The action contract** starts from eohunter's (`actions.rb:142-158`), but changes its
  order. eohunter checks preconditions, then settles roundtime, then checks the target
  (`:152`) and sends. **Every check moves after the wait** (author, 2026-09-24): *"conditions
  can change between then and then"*. A precondition checked before a five-second roundtime
  is five seconds stale when the command goes out. So:
  1. settle roundtime and casttime;
  2. check the preconditions and the target against the state **as it is now**;
  3. send and confirm.

  A check that fails is a skip, not a failure, and the next tick decides again from scratch.

  **The last check belongs in the session, not the behavior.** A behavior reads its own
  folded copy of the state, which trails the actor's by however many events are still
  queued. The actor already evaluates a `Gate` at the moment it writes (`send_now`'s
  `Gate::Roundtime`, `actor/io.rs`). Hunt's gate — standing, not stunned or webbed, target
  still present and alive — is evaluated **there**, against the live model, as the bytes
  go out. That is also the first caller that constructs `Refusal::Stunned` and
  `Refusal::Webbed`, which `command/verdict.rs` has carried unused since M1.

  What no check can close is the window after the write: the game can change before it
  reads the command. That is `12` §4.4's rule — a behavior tolerates a result it did not
  expect — and not something a precondition fixes.

---

## 4. Scope: built toward one real hunt

**The acceptance hunt is Nisugi on `ojandhaart.yaml`** (author, Q7), a bigshot profile.
The draft's scope was a melee hunter with plain routine lines. Nisugi is an archer: a
ranger with sigils, whose routine fires from hiding and whose loot goes through `eloot`. What
that one profile needs sets the slice, key by key:

| The profile says | Hydra needs | M6 |
|---|---|---|
| `hunting_room_id: 29902`, `hunting_boundaries: 29900, 30115`, `resting_room_id: 29877` | travel there and back; Wander inside the boundaries (`wander.rb:41-115`) | **in** |
| `targets: mastodon(b), berserker(d), …, (?:.+?)(f)` | a ranked target list, each with a routine letter, with a catch-all | **in** |
| `hunting_commands` … `_j` | ten routines, one per letter | **in** |
| routine verbs `fire`, `hide`, `incant 608`/`611`, `kweed`, `coupdegrace` | each is an action Hunt can send and confirm | **in** |
| `script volley` | the author's `volley.lic` (§5): a guarded weapon swap around the Volley technique | **in**, as a named sequence |
| guards `(buff5)`, `(thp20 empowered30)`, `(!frozen)`, `(!hidden)`, `(hidden)` | typed guards, read from the model | **in** (§5) |
| `signs: 515, 506, 9708, 9715, 9711, 605 evoke, 650 panther evoke` | Maintain: keep these up, recast before they lapse | **in** (it was out in the draft) |
| `hunting_prep_commands: ready weapon, incant 515` | run once, on arrival | **in** |
| `hunting_stance: Offensive`, `wander_stance: Defensive`, `stand_stance`, `loot_stance` | the stance setter | **in** |
| `wounded_eval: bleeding? \|\| health <= 60 \|\| !able_to_use_ranged? \|\| !able_to_cast?` | typed rest thresholds; all four facts are already modelled | **in** |
| `encumbered: 20`, `fried: 101`, `overkill: 2`, `rest_till_*` | Rest's thresholds | **in** |
| `resting_commands: store all` | a **game verb**: puts weapons and shield away into their proper containers from the character's readylist (author). Not `stow all`, which sends everything in hand to the default stowlist container | **in**, sent as-is |
| `loot_script: eloot`, `delay_loot`, `box_in_hand` | **the eloot port** | **in** (author, below) |
| resting and healing | **the eherbs port** | **in** (author, below) |
| `flee_count: 100`, no flee lists | Flee, count only | **in**, small |
| `monitor_strings`, `monitor_interaction: false` | the interaction monitor | out: it is switched off |
| group keys, `fog_*`, `rallypoint_*`, boons, UAC, mstrike, wands, ammo, bless, favor | nothing | out |

### eloot and eherbs are in M6

The author, 2026-09-24: *"We need to make sure we have eloot and eherbs too, not just
eohunter."* A hunt that cannot loot fills its hands and pack and rests on encumbrance. A hunt
that cannot heal rests on wounds and has no way to stop resting. So Loot and Heal are two of
`12` §4.2's composed behaviors, and they are not decoration.

MEASURED, `wc -l` in `reference/scripts/scripts` (clone at `7fd0b97`, 2026-09-17):

| Script | Lines | What Hunt calls it for |
|---|---|---|
| `bigshot.lic` | 10,246 | the profile format and routine vocabulary eohunter implements |
| `eloot.lic` | 8,087 | loot the corpse and room, sort into containers, sell and deposit in town |
| `eherbs.lic` | 3,639 | heal wounds and blood from herbs, buy herbs when short |
| eohunter | ~15,000 (§3) | the engine, and the policies around them |

**These are ports of the same kind as travel**, which was `go2` plus its routines. The
knowledge in them is game knowledge, such as which container, which herb heals which wound and
which verb sells a gem. That is the `CLAUDE.md` rule to port aggressively where knowledge lives
in code. **Each gets its own port plan before it is built** (`plan/31` eloot, `plan/32`
eherbs), measured the way `plan/24` measured go2. That means tabling which halves a hunt
calls and which halves are town errands (selling, banking, buying herbs) that can wait. Both
lean on `20` §0b's deferred **sending halves** of stash and bank, so those come in with them.

### Groups: recorded now, built after solo

The author's answer to Q3 is a group design, and it is recorded here so it is not lost:

> *"the ones still online would continue to fight and keep the room clear to keep it safe
> waiting for the reconnect. Eventually someone would have to take over and decide from
> there: if the disconnected one is out of game and x amount of time has passed, then take
> over and continue the hunt or rest if needed. If the disconnected one is still in game,
> then keep clearing the room, after x amount of time, everyone would need to leave the group,
> join back up on who is the new leader, the leader add the disconnected loligagger to their
> group, then go rest."*

That is a cross-session behavior: a leader, followers, a hand-over after a timeout, and a
regroup. It needs a solo hunt that survives a reconnect first, which is SE-4. **The acceptance
hunt is solo** (author, N4), so groups follow M6.

### Also in

- **`;hunt <profile>` and `;hunt stop`.** The hub card shows each character's running
  behavior and has **Stop**.
- **The session prerequisites:**
  - SE-4, as (c);
  - the attendance rule (Q5);
  - `BEHAVIOR_WATCHDOG`;
  - the forced revoke after `PREEMPT_GRACE`.
- **Importing bigshot YAML profiles.** The author has only bigshot profiles (Q8), so importing
  them is the door in. It reads the policy fields, translates `wounded_eval` where it is one
  of the known shapes, and refuses the rest by name.
- **`;foreach`** and **`;multi`** (Q6), as built-in behaviors on the `;` line. `foreach.lic`
  is 2,192 lines. `multi.lic` is about 60: repeat a comma list N times, where an entry
  starting `;` runs a script and waits.
- **`;sorter`** (Q6) is **not a behavior**. VellumFE's port (`src/core/sorter.rs`, 400 lines)
  sends nothing. It rewrites a container look into one line per category, pure over the
  line's own links, so it belongs in the projection (`cena-ui`), beside the other stream
  transforms.

---

## 5. The profile format

**TOML** (author, Q4), one file per profile. Stores that Hydra writes stay JSON.

eohunter's YAML is mostly policy already. Its `Profile::RULES` table (`profile.rb:31-77`) gives
every key a cleaner and a default. **Three things in it are programs, not policy**:

| bigshot/eohunter | Why it is a program | Proposed replacement |
|---|---|---|
| `wounded_eval`, `town_rest_required_eval` | a Ruby `eval` string | typed thresholds. Nisugi's becomes `rest_when = { bleeding = true, health_below = 60, cannot_use_ranged = true, cannot_cast = true }` |
| routine guards `(thp20)`, `(!hidden)`, … | a condition on one line | a **closed vocabulary of typed guards** (below) |
| `hunting_commands_b`..`_j` per creature letter | a dispatch table | a named routine per target entry, with a fallback |

**Guards are in** (author, 2026-09-24): *"conditions are a big part, there are
preconditions. If not listed here then how would they actually be used?"* `12` §6a.1 is
corrected to match: what it bans is control flow the user authors, not preconditions.

A guard is a **named precondition on one step**, from a closed vocabulary Hydra defines. Each
reads one fact the model already holds and takes at most one number. Guards on a step all
have to hold. There are no user-defined guards, no nesting, no *or*, and no expressions.

**The vocabulary starts from bigshot's, and is evaluated, not copied** (author: *"Bigshot has
been around a decade and evolved over time, some may not make sense, or there may be some
that make more sense it doesn't have"*). MEASURED: `COMMAND_MODIFIER_REGEX`
(`bigshot.lic:3197`) accepts **87** guard words (`sed -n 3197p bigshot.lic`, split on `|`),
most with a `!` negation.

**The evaluation is its own step** (§7, step 4), and the author reviews it before anything is
built. One row per word gives:
- what it reads, from its bigshot branch;
- whether Cena's model can answer it, and with what;
- a verdict: **keep**, **rename** (the name hides the meaning, like `buff5`), **merge** (two
  words for one fact, like `frozen` and `immobilized`), **drop** (obsolete, or a fact that no
  longer exists), or **new** (a fact the model now holds that bigshot could not see, such as
  a creature's crit injuries or `vanished_unaccounted`).

Nisugi's six are built first. Their semantics, read from bigshot's branches:

| bigshot | Skips the step when | From |
|---|---|---|
| `buff5` | the buff **this step's own verb** grants is up with 5 s or less left, *"don't fire into the expiry window"* (`bigshot.lic:4239-4263`). For `kweed` the buff is Tangleweed Vigor (`:3235`) | `state.effects` |
| `thp20` | the target is **above** 20% health, **or its health is unknown** (`:4325-4341`) | `CreatureInstance::hp_percent()` |
| `empowered30` | an Empowered buff of +30 or more is already up, so a weaker coup de grace would overwrite it (`:4314-4323`) | `state.effects` |
| `frozen` / `!frozen` | the target is / is not immobilized (`:4408-4419`) | `CreatureInstance::has_status` |
| `hidden` / `!hidden` | I am not / am hidden: the guard names when the step **runs** | `status.known().hidden()` |

**Each guard is ported from its bigshot branch, with its semantics, not from its name.** The
table above was first drafted from the names, and got two of the six wrong: `buff5` read as
"more than 5 s left", which is the opposite, and `thp20` lost the rule that an unknown health
skips. The porting rule in `CLAUDE.md` exists for exactly this.

**The importer knows every bigshot word and its verdict.** A kept or renamed word translates;
a merged one maps to its survivor; a dropped one, or one not built yet, imports with that
step named and held, rather than silently losing the guard. A lost guard changes when a
command fires, and that is worse than a refusal.

**Sequences replace scripts inside a routine.** `script volley` runs the author's `volley.lic`
(`C:\Gemstone\lich-5\scripts\customolley.lic`, 10 lines) and waits. MEASURED, it is
two guards and seven commands: skip if effect 9105 has more than 7 s left, skip unless the
Volley technique is available, then swap to the second weapon, go offensive, use Volley, raise
the longbow and swap back, settling roundtime between. That is a `;multi` list with guards, so
a profile can define one:

```toml
[sequence.volley]
skip_if = { effect_active_over = { id = 9105, seconds = 7 } }
requires = { technique_available = "volley" }
steps = ["store weapon", "ready 2weapon", "stance offensive", "weapon volley",
         "raise longbow", "store 2weapon", "ready weapon"]
```

A routine line then names `volley` like any verb. The importer cannot read Ruby, so
`script <name>` imports as a sequence that has to be written by hand, and the importer says so.

**Presets** ship for the common shapes, such as melee, archer and caster, and a character's
profile is a preset plus overrides. **The inheritance chain** is built once and used by hunt
first: a key resolves character → profile → global → built-in default, and the first level
that sets it wins. Every level exports as one file.

---

## 6. Answers, and what they opened

| Q | Answer (author, 2026-09-24) | Consequence |
|---|---|---|
| Q1 the probes | **delete them** | step 1 |
| Q2 `sync` | **run it quietly, like infomon**: *"a one line message for each stage and hides all the spam. That way they know something is going on, what it is, and that it's not stuck."* | step 1: one status line per stage, the game output of the sync commands kept out of the story |
| Q3 SE-4 | **(c)**, the session keeps the authority across a reconnect; plus the group design recorded in §4 | step 2 |
| Q4 format | **TOML** | §5 |
| Q5 attendance | **"yes it should give him up"**: logging the character in elsewhere mid-hunt ends the hunt and Hydra backs off, as M5 decided | step 2; the rule below |
| Q6 chain/foreach | *"we do ;multi not chain, and ;foreach yes, also ;sorter"* | §4; `chain` is renamed `;multi` |
| Q7 first profile | **Nisugi, `ojandhaart.yaml`** | §4 |
| Q8 import | *"I only have bigshot profiles"* | import is how profiles get in (§4) |

**Q5, restated.** Hydra has a "nobody's home" rule. If the character keeps getting knocked
off and nobody has done anything in between, Hydra stops logging back in. That is what stops
Hydra and the phone fighting over one character forever. "Done anything" means today *any
command was sent*, and a running hunt sends commands all the time. So with a hunt running,
Hydra always thinks somebody is home and never backs off from the phone.

The simple fix is to count only **what a person did**: typing, or pressing a button on the
page. But that goes wrong the other way: an overnight hunt with two network blips hours apart
would stop at the second one, because no person did anything in between.

**Decided** (author, 2026-09-24): *"yes it should give him up."* Hydra's rule to meet that:
somebody is home if **a person did something, or the last connection stayed up a long time**. A phone fighting Hydra knocks it off within seconds, and a network blip comes
after hours. The measured threshold would be minutes, not the ladder's 30 s.

**Answered since:** guards are in (N1, §5); `script volley` is the author's `volley.lic`, a
sequence (§5); the acceptance hunt is **solo** (N4); `multi.lic` is in the clone and
`;sorter` is VellumFE's `src/core/sorter.rs` (N5, §4).

`store all` (N3) is a game verb, from the readylist (§4). **Nothing is open.**

---

## 7. Steps (proposed order)

Each step ends green, committed, and demonstrable on its own. M6 is now four ports plus an
engine, so it is cut into sub-milestones that each finish with something the author can run.

**M6a — foundations**

1. **Clear out** (§2): `look`, `--demo`, the walkthrough and the probes go. There is one run
   path, and criteria 4 and 5 are re-hosted with their mutations. `sync` runs at Ready with
   infomon-style stage lines.
2. **Session prerequisites.**
   - SE-4, as (c);
   - attendance (Q5): behavior traffic alone is not a person;
   - `BEHAVIOR_WATCHDOG` and the forced revoke after `PREEMPT_GRACE`.

   Each gets an isolation-style test and a mutation that turns it red.
   > **BUILT 2026-09-24.**
   > - **Attendance** (`cf5c9bc`) is what a person does, counted at the handle, or a
   >   connection that lived `LONG_LIVED` (5 min) without an idle warning.
   > - **SE-4:** the authority is one cell per session (`command/authority.rs`), shared by
   >   every connection's queue and answered by the supervisor between connections.
   > - **§4.3 preemption:** `SessionHandle::preempt` cancels the holder, waits
   >   `PREEMPT_GRACE`, then revokes; a revoked holder's queued commands are refused.
   > - **The watchdog is a heartbeat, not traffic** (`cena-behavior/src/watchdog.rs`). A
   >   resting hunt sends nothing for minutes, so a behavior beats each time its loop turns,
   >   and a watcher beside it preempts it after `BEHAVIOR_WATCHDOG` (30 s) of silence.
   >   Preempt and the watchdog get their first real caller in M6b, the hunt's Stop; travel's
   >   stop stays cooperative because its take-back needs the authority.
   > - Every piece has a test that a named mutation turns red.
3. **Acting primitives.**
   - the stance setter and `in_casttime`;
   - the action contract (settle, then check, then send), with its final check as a `Gate`
     the actor evaluates at write time;
   - `creature_message` wiring;
   - travel's drive loop split from its claim.
   > **BUILT 2026-09-24.**
   > - **Cast roundtime:** `in_casttime` and `casttime_remaining` in `cena-model`'s
   >   `clock.rs`, beside the roundtime readers they mirror.
   > - **The stance setter** (`cena-behavior/src/stance.rs`, `00575eb`) is pure. It works
   >   out what to send, confirms the change from `pbarStance` rather than from the
   >   game's sentence, and picks the safest stance. A percent is sent as `cman stance N`
   >   only when Stance Perfection is trained.
   > - **The last check is the session's** (`actor/gate.rs`). `Gate::Act { target }`
   >   refuses an action, and does not write it, when the live model shows roundtime,
   >   cast roundtime, stunned, webbed or dead, or when the target is no longer in the room
   >   alive (`valid_target`). `send_gated` carries the gate. `Refusal::Stunned` and
   >   `Webbed` finally have a caller (review SE-11).
   > - **`travel_holding`** is the walk without the claim and release, so Hunt can walk
   >   while keeping the authority. `travel` is now claim, then `travel_holding`, then
   >   release.
   > - **`creature_message` wiring was DROPPED.** Hunt gets all it needs elsewhere:
   >   deaths from `<crtrStatus>`, flight from `state/departure.rs`, and decay from the
   >   room list. `plan/27d` already records the wiring as deliberately not done.
   > - Each gate condition, and the split, has a test that a named mutation turns red.
4. **The guard evaluation** (§5): a table of bigshot's 87 words with a verdict each, for the
   author to review. Then **the profile format, the chain and the bigshot importer**:
   `ojandhaart.yaml` imports, and what it drops is named.

**M6b — Hunt, without the ports.** Engine, Survival, Flee, Engage (Nisugi's verbs and
guards), Maintain, Wander and Rest, pure, driven by replay fixtures from Nisugi's area (the
corpus cut is asked for first). Loot is `loot #id` and healing is waiting, as stand-ins.
Demonstrated live on a short hunt.

**M6c — eloot.** Its port plan first (`plan/31`), then the halves a hunt calls: loot,
sort, box in hand. Town errands follow in the same plan's order.

**M6d — eherbs.** Its port plan first (`plan/32`), then healing at the rest room.

**M6e — the `;` tools.** `;foreach` and `;multi` on the `;` line, and `;sorter` as a
projection transform ported from VellumFE. None depends on the hunt, so they can move earlier.

**M6 live acceptance, author present.** Nisugi runs `ojandhaart` through at least one full
cycle: hunt, a rest threshold, walk to rest, loot stored, healed, walk back, hunt. It is
stopped mid-attack from the hub within `PREEMPT_GRACE`. A manual command mid-hunt interleaves.
A reconnect mid-hunt keeps the hunt, as (c) decides.

**For later, not M6:** `tpick` (<https://github.com/Lord-Dreaven/tpick>), named by the
author on 2026-09-24 as a script to port in the future. Not yet read.

---

## 8. What M6 deliberately does not prove

- Groups. The acceptance hunt is solo; the design is recorded in §4.
- Professions other than Nisugi's. The vocabulary grows from real profiles, not ahead of them.
- The agent protocol (M7), although the policy/arbiter split keeps "observe" and "act"
  separate, which M7 needs.
- The GUI (M10).
