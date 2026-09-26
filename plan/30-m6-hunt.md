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
   > **The evaluation is `plan/33`, 2026-09-24: PROPOSED, awaiting the author.** Of the 87
   > words, 38 are kept, 11 renamed, 34 merged, 3 deferred and 1 dropped, with 4 new ones
   > proposed.
   >
   > **The format, the chain and the importer are BUILT, 2026-09-24, ahead of the review
   > and under the held contract** (`crates/cena-behavior/src/hunt/`: `profile.rs`,
   > `chain.rs`, `import.rs`, with `guard.rs`, `yaml.rs` and `command.rs`; `;hunt import
   > <yaml> [as <name>]`, `;hunt check <name>` and `;hunt list` on the command line).
   > Nothing in them depends on a `plan/33` verdict: a bigshot word not yet built imports
   > **held**, the step kept and the word named, so a verdict only ever adds a word.
   > - **TOML, one file per profile**, shaped key by key from `ojandhaart.yaml` (§4's
   >   table): `[rooms]`, `[stance]`, `[rest]` with `until` and the typed `when`,
   >   `prepare`, `signs`, `[flee]`, `[loot]`, `[wander]`, `targets` each with a routine,
   >   `[routines]` of steps, `[sequences]`. A step is `verb (guard guard)`; a held step
   >   is a table `{ step, held }`, so it cannot pass for one that runs. Every table
   >   refuses an unknown key by name.
   > - **The chain** is `hunt/global.toml`, then `hunt/profiles/<name>.toml`, then
   >   `hunt/characters/<instance>_<character>.toml` under the data directory, overlaid as
   >   TOML tables before typing: tables merge key by key and everything else replaces
   >   whole, so a character can take a step away as well as add one. Presets are not
   >   written yet: a profile is the preset until a second wants what the first has.
   > - **Nisugi's six guards** are built as `plan/33` §5 lands them. `expiring "<name>" N`
   >   runs the step when the effect is **down or has N seconds or less left**, which is
   >   what the author meant by `kweed(buff5)` and the inverse of what bigshot's code does
   >   since `7fd0b97` (`plan/33` question 6). Names match by prefix, ignoring case, because
   >   the Buffs dialog cuts long names off (`Nature's Touch Arcane Ref`). Unknown skips,
   >   for every word, as bigshot's `thp` does.
   > - **The importer** carries the acceptance profile whole: 10 routines, 6 targets, zero
   >   held steps. `script volley` becomes an empty `volley` sequence, named as to be
   >   written by hand; `monitor_strings` and two others are named as not imported;
   >   `frozen` is flipped to `!immobilized`, `buff5` is resolved through the verb's buff,
   >   `thp20` and `empowered30` are re-spelled.
   > - MEASURED: 28 tests (`hunt_guard` 7, `hunt_profile` 9, `hunt_import` 4, `yaml` 5,
   >   `command` 3), the acceptance profile itself at `tests/fixtures/ojandhaart.yaml`.

**M6b — Hunt, without the ports.** Engine, Survival, Flee, Engage (Nisugi's verbs and
guards), Maintain, Wander and Rest, pure, driven by replay fixtures from Nisugi's area (the
corpus cut is asked for first). Loot is `loot #id` and healing is waiting, as stand-ins.
Demonstrated live on a short hunt.

> **BUILT 2026-09-24, not yet run live.** `cena-behavior/src/hunt/engine.rs` is the pure
> machine: `Hunt::tick(&GameState, Here, now) -> Said`, eohunter's seven behaviors as the
> arms of one `match` in eohunter's priority order (`runner.rb`, `behavior.rb`), under one
> holder of the authority. `drive.rs` is the `async` layer: it holds the token, folds the
> session's events, places the character by the map, settles roundtime, sends through the
> write-time `Gate::Act`, and **walks by running travel's own driver inside the hunt's
> authority** (`travel_holding`, with a second listener on the stream and the hunt folding its
> own meanwhile). `desk.rs` runs one hunt per session with the watchdog beside it; `;hunt
> <name>` and `;hunt stop` are on the command line, with travel's map.
> - **What each arm does now:** Survival stands or stops on death; Flee leaves a room over
>   `flee.count` or holding a `flee.from` creature; Rest is the whole cycle (reasons, the
>   walk, the rest commands, the `until` thresholds, the walk back, the prepare commands);
>   Loot is `loot #id` once per corpse, no oftener than every 15 s while targets stand when
>   `loot.delay` (bigshot's `time_between(:need_to_loot?, 15)`, first call passing); Maintain
>   casts a sign the effects list says is down, once a minute at most, with no target
>   present; Engage targets, takes the hunting stance, and runs the routine one step a tick,
>   skipping held steps and steps whose guards do not hold, expanding sequences; Wander waits
>   `wander.wait`, takes the wander stance, and walks to a fresh crossable room off the
>   boundary list, least-recently-visited when none is fresh.
> - **Two facts the port corrected:** `rest_till_spirit` is spirit **points**, not a percent
>   (`rest.rb:239`); and the game's current target is `Targeting::current()`, the first id of
>   the dropdown, not membership in its list.
> - **Stand-ins and gaps, each said to the player where it bites:** loot is `loot #id` and
>   healing is waiting (the plan's stand-ins, until M6c and M6d); groups, `pull`, `deader`,
>   ammo, wands, boons and the `censer_between_actions` policy (`plan/33`) are not built.
>   **Since built, 2026-09-25:** `pull` and `deader` (`8b4dfda`), wands (`f074197`) and
>   boons (`d87a4ae`). Groups are outside M6 (§8). The censer policy is built
>   (`hunt/censer.rs`, `plan/33` §2h), and so is ammo (below).
> - **Found 2026-09-25: the routine's verbs went to the game as written.** §4's table put
>   `kweed`, `coupdegrace` and `fire` in scope as "an action Hunt can send", and nothing
>   sent them as bigshot does: `kweed` is not a game command at all (bigshot evokes
>   Tangleweed at the creature, `cmd_weed`, `bigshot.lic:5750-5765`) and `coupdegrace` is
>   `cman coupdegrace #id` (`cmd_cmans`). **Built:** `hunt/verbs.rs` sends each bigshot
>   verb as its handler does, aimed at the creature by id, with the handler's gates the
>   model can answer (known, affordable, cooling, stamina, a plant already here); a spell
>   is `prepare N` then `cast #id` through `crate::cast`, a self-cast one `incant N`. A
>   game command (`store weapon`, `weapon volley`) goes as written. Not ported yet, each
>   skipped and named once to the player: `eachtarget`, the buff-first forms (`celerity`,
>   `slayer`, `tonis`), `resonance`, `briar`, `efury`, `tether`, `nudgeweapons`,
>   `unarmed`, `mstrike`, `wandolier` and `wield` (needs the worn list); `jewel` followed
>   (below).
>   **Closed the same day, on the author's objection to trying it live with gaps open:**
>   Assume Aspect is cast as `cmd_assume` casts it (the spell, evoked or prepared, then
>   `assume <aspect>` once the effects list shows it), and a walk inside a hunt reads and
>   keeps the character's travel file as travel's own desk does.
> - **Since built, on the author's *"all of bigshot"* (2026-09-25):** `jewel`, sent from
>   `cena-behavior/src/gemstone/` as Rule 3.4 asks (`4102b60`); ammo, an arrow the game will
>   not fire stowed or put in `ammo_container` (`d4540da`); the dead man's switch and the
>   depart switch (`35afb9a`, `hunt/death.rs`); the interaction monitor, bigshot's
>   `monitor_*` keys raised as warnings, off unless the profile turns it on (`hunt/monitor.rs`,
>   `tests/hunt_monitor.rs`); quick hunting, `;hunt <name> quick`, this room until it is clear
>   on `quickhunt_targets` and `quick_commands` (`hunt/quick.rs`, `tests/hunt_quick.rs`).
>   Then the verbs that run more than once or carry a buff: `eachtarget` and `force <step>
>   till N` (`hunt/repeat.rs`), `resonance`, and `celerity`/`slayer`/`tonis` before a step
>   (`hunt/verbs.rs`); `tests/hunt_repeat.rs`, 8 tests, 11 mutations each turned one red.
>   And the creature facts from `inventory/12` §2 are read (`hunt/targets.rs`): allies are
>   never fought, hazard creatures are not `any` targets, and a no-corpse kill is counted.
>   Stance as bigshot takes it: the hunting stance before each step but a bare spell number,
>   `wait`, `sleep`, `wand`, `berserk`, `script`, `hide`, `nudgeweapon` (`bigshot.lic:4051`),
>   and a stance spell cast offensive and put back as Lich's `Spell#cast` does
>   (`tests/hunt_stance.rs`); Celerity and 902 cast untargeted.
>   Then the rest of an audit of every handler bigshot dispatches against what Hydra sent
>   (2026-09-25): the gates it skipped and the handlers that wait or read the answer
>   (`hunt/verbs/gated.rs`, `hunt/follow.rs`, `tests/hunt_gated.rs`, 16 tests, 31 mutations
>   each turned one red): `stomp` channels Tremors, `leech` and `rapid` wait on their
>   cooldowns, `burst`/`surge` on stamina and buff, `smite` only the undead, `throw` not
>   the prone, `curse` releases and prepares once, `sacrifice` appraises first, `depress`
>   begins the song, `unravel` stops it, `efury`/`tether`/`wait`/`sleep`/`berserk` hold,
>   `hide N` retries, `dhurl` recovers, `dislodge` frees a lodged part, `ambush <part>`
>   aims at the creature, and `kick` is `punch` while rooted. `verbs.rs` split: its
>   spells and tables moved into `verbs/`. And `cmd_spell`'s own rules
>   (`tests/hunt_spell_rules.rs`): a routine spell the character cannot afford sends the
>   hunt to rest unless bigshot's `oom` is negative (a blank one is 0, so on:
>   `rest.when.unaffordable`), Celerity is not recast while up nor Camouflage while
>   hidden, five cooldowns and the short buffs' are respected, Mana Leech's recovery adds
>   its 5; Soothe goes first while a calming spell is on the character; the hunter stands
>   in the stand stance, and a crossbow archer stays kneeling to fire. And Lich's
>   `available?` before every maneuver, technique, shield move and feat (the model's
>   `psm_availability`, `a392b77`: trained, affordable, not cooling, not overexerted;
>   unknown lets it go), `shield bash` as the Shield Bash maneuver when that is available,
>   703 and 1614 not cast at a creature they already hold, and the frail read off the
>   appraise reply's last line (`tests/hunt_psm.rs`). **The model read no PSM ranks
>   before `a392b77`,** so the coup gate never fired live; it does now. Then `wield` and
>   `briar` on the model's worn list, `wandolier` on its reserve (`5a56feb`, `b05378f`), and
>   `mstrike` and `unarmed` with bigshot's UAC and Mstrike tabs imported (`[unarmed]`,
>   `[mstrike]`; `hunt/verbs/ucs.rs`, `tests/hunt_ucs.rs`): Multi-Opponent Combat's ranks
>   (read since `cf32e5d`; before it, as in Lich unsynced, 0), no nest, the cooldown,
>   `quickstrike 1`, unfocused at the mob, a Paladin's or Empath's 1607/1107 first; the
>   unarmed attack by the creature's positioning and the follow-up the game offered. **Of
>   bigshot's verbs only `nudgeweapons` is not ported**: it walks out of the room and back
>   mid-fight, which the engine's room change undoes.
>   And bounty mode, `;hunt <name> bounty` (bigshot's `;bigshot bounty`, `hunt/bounty.rs`,
>   `tests/hunt_bounty.rs`): hunt until the guild's task is done, failed or a new one is
>   ready, rest, and end at the rest; not while bandits are here on a bandit bounty.
>   bigshot's skin and gem counts, which look in every container, are not built.
>   `hunting_scripts` and `resting_scripts` are named script by script: Hydra runs no Lich
>   script, and says where a script's work is built in (heal, loot, `;keep`, `;waggle`); a
>   resting `ewaggle` becomes `rest.waggle`, the waggle profile run as each rest begins
>   (`tests/hunt_rest_scripts.rs`).
> - **Found by `plan/39`, 2026-09-25: the hunt ended on a reconnect**, which the acceptance
>   line below forbids: the driver's fold turned `Reconnecting` into an error. Now the
>   driver waits the drop out holding its authority (SE-4 (c)), sends nothing while away,
>   forgets the target and the held room, and hunts on after `Ready`; a walk the drop cuts
>   short is walked again (`plan/39` Stage 0, `tests/hunt_reconnect.rs`).
> - **The corpus cut, and what it found.** The author's first condition was *"nothing from
>   hinterwilds"*, and MEASURED over every third of Nisugi's 6,570 sessions plus all of the
>   newest 45, every session with a fight is in the Hinterwilds or the Duskruin Arena; the
>   author then named three Ojandhaart logs of 2026-09-13, and two windows of one of them
>   are `crates/cena-behavior/tests/fixtures/smithy_engage.xml` (299 lines, 59,220 bytes)
>   and `smithy_kill.xml` (278 lines, 70,502 bytes), scrubbed, provenance in
>   `crates/cena-behavior/tests/hunt_replay.rs`. That test replays each through a session
>   and ticks the engine at every prompt against Nisugi's imported profile. **The first
>   replay found three defects that ten hand-built tests had passed:** Assume Aspect was
>   cast before any effects list had been seen (the `650` branch ran ahead of maintain's
>   gate); a corpse was never read as dead, because `dead()` is hit points at zero and the
>   room list's `dead="1"` was the only thing the wire said of this pegasus (now
>   `CreatureInstance::corpse`); and `loot.delay` deferred looting while any target stood,
>   where bigshot loots the first corpse at once and Nisugi's log has the search two seconds
>   after the kill with the engineer standing. With those fixed, the engine says `fire`
>   where Nisugi fired, `loot` at the prompt after `drops dead`, then `target`, the stance,
>   and Camouflage in bigshot's order. **A third window from a health-era log the author
>   named next** (`arch_kill.xml`, 2026-09-21, 59,250 bytes) found a fourth: the author's own
>   battle mastodon, whose status carries health and no `hostile`, was targeted under the
>   any-creature rule. `CreatureInstance::hostile` reads the flag once a status has been
>   seen, and the engine leaves a creature known not to be hostile alone. On that window
>   both kills are looted at the prompt after each fell, and routine e's guards read off
>   real statuses (`rooted` holds `incant 611` back; 738 of 900 holds `coupdegrace` back).
> - **Seen in the replay and left as is:** the stance setter sends the stance when the bar
>   has never been seen (the first prompts of a cut), which a live session's login burst
>   makes moot; `hidden` unknown holds every guarded step (`Wait(1)`) until the game says.
> - MEASURED: 37 hunt tests in `cena-behavior` (`hunt_engine` 11, `hunt_replay` 6,
>   `hunt_guard` 7, `hunt_profile` 9, `hunt_import` 4), plus the `yaml` and `command` unit
>   tests.
>
> **Live acceptance is the next thing**, and it is the author's: `;hunt ojandhaart` with the
> map set, in the hunting ground, with `;hunt stop` at hand. The replay has shown the engine
> choosing what bigshot chose on real frames; what only a live run can show is the driver's
> half: that each verb is sent as the game takes it, settled on roundtime, and that a walk
> inside the hunt lands.
>
> **ORDER, the author, 2026-09-25:** *"The live run is at the end! We gotta get all the other
> m6 stuff so I can test it in the live run!!"* The live run is not next: everything else in
> M6 is built first, and one live run tests all of it.

**M6c — eloot.** Its port plan first (`plan/31`), then the halves a hunt calls: loot,
sort, box in hand. Town errands follow in the same plan's order.

> **`plan/31` written and answered 2026-09-24; Stages 1 and 2 BUILT the same day.** eloot
> measured module by module: the hunt's share is ~1,750 of 8,029 lines (Main, Inventory,
> Room looting); the rest is a settings window and the town. The pure planner
> (`cena-behavior/src/loot/`) and the driver's loot loop are in; a hunt with an imported
> loot profile searches each corpse and takes the floor by eloot's rules, and rests on
> *too much loot* or a box left in hand, as the author answered. Stage 3 (eloot's specials,
> a critter's bag emptied, the sigil) is BUILT too, and so is skinning (`loot/skin.rs`,
> 2026-09-24); Stage 4 (selling during the rest) is under way by the author's call to
> finish eloot before M6d: 4a, the round to the gem shop and the pawnshop inside the rest
> (`cena-behavior/src/town/`), 4b (furrier, collectibles, Chronomage, bank) and 4c (the
> locksmith pool, boxes emptied by the loot planner) are BUILT. **`plan/34`** stages the author's `loottracker.lic` as the loot ledger: the
> classifier in the model, the recorder beside combat's, SQLite already embedded. Its
> All four stages are BUILT: the classifier, the ledger, `;loot` and `;combat`
> (`cena-model/src/state/ledger.rs`, `cena-session/src/ledger.rs`, `cena/src/loot.rs`,
> `cena/src/combat.rs`); recording is `--record`/`--no-record`, on in debug builds.

**M6d — eherbs.** ~~Its port plan first (`plan/32`), then healing at the rest room.~~
**BUILT 2026-09-25 as `plan/36`** (the number `plan/32` was never used): the herb table
in the model, healing at the rest room and as `;heal`, the Survivalist's Kit and its
distiller, stocking at the herbalist. The spell scripts followed as `plan/37`.

**M6e — the `;` tools.** `;foreach` and `;multi` on the `;` line, and `;sorter` as a
projection transform ported from VellumFE. None depends on the hunt, so they can move earlier.

> **`;sorter` BUILT 2026-09-25, not yet run live.** VellumFE's `src/core/sorter.rs` is
> `crates/cena-ui/src/sorter.rs`: while sorting is on, the line assembler
> (`crates/cena-ui/src/lines.rs`) keeps a main-stream line's pieces with each link's noun, and
> a finished container look becomes a header and a bold-labelled line per `gameobj` category,
> duplicates counted. The switch is per session in the web pump
> (`cena_web::Sessions::sort_containers`), flipped by `;sorter [on|off|status]`
> (`crates/cena/src/sorter.rs`). Off until asked, which is VellumFE's default; not saved,
> since nothing writes the settings file yet. **Stricter than VellumFE:** a look is sorted
> only when it is a list end to end. The herb kit's look goes on past its list, and VellumFE's
> transform drops every dose count (182 of 475 looks in a month of Nisugi's logs). 14 tests:
> 10 in `cena-ui` (6 over five real looks, `crates/cena-ui/tests/fixtures/container_looks.xml`;
> 4 synthetic, two of them VellumFE's own), 3 for the command, 1 end to end through the
> parser and the pump (`crates/cena/tests/web_sorter.rs`). `;foreach` and `;multi` followed (`3804678`, the note below).

> **`;multi` and `;foreach` BUILT 2026-09-25, not yet run live.** One behavior, a **batch**
> (`crates/cena-behavior/src/batch/`; `VellumFE`'s own word for its foreach): a list of lines sent
> in order by one driver that sends as the hunt sends -- roundtime and cast roundtime settled,
> through `Gate::Act`, the prompt taken -- with Lich's `fput` answer to a refusal added, since a
> list has no next tick to decide again: a gate refusal is waited out and the line tried again
> (given up after 60 s), and the game's `...wait N` is waited out and the line resent (five times
> at most). One of each kind at a time per session (`batch/desk.rs`, authority tokens 4 and 5); a
> second is refused, as Lich and `VellumFE` refuse it. `;multi stop`, `;foreach stop`.
>
> **A Hydra command in a batch** -- `;multi 2,get gem,;sc 401,put gem in sack` -- is run through
> the binary's own command table and waited for until what it started is over. The families that
> start something (travel, the hunt desk, batches) now hand back its task (`Took::Started`,
> `crates/cena/src/commands.rs`); the batch gives the authority back while it runs and takes it
> again after. Typed at the prompt, nothing changes.
>
> **`;multi` is multi.lic whole** (65 lines): the count first or last, semicolons when there is
> no comma, an entry with the symbol run and waited for. On purpose: entries are trimmed and a
> blank skipped; a count neither first nor last is refused (multi.lic ran zero times, silently);
> a `;multi` inside a `;multi` is refused.
>
> **`;foreach`, MEASURED** (`wc -l`; `grep -n "^class\|^module\|^  def " foreach.lic`): 2,192
> lines. 166 are header and changelog; 504 the `ItemMatcher` (73 of them reading `inventory
> full`, 95 the locker, 56 marked and registered, 70 `finalize`); 177 the Stormfront status bar;
> about 490 setup, changelog, help and formatting; and 625 `run` -- 79 options, 27 filter, 112
> rewriting the commands, 116 targets, 217 running them, 20 listing. The core is built: the line,
> the matching and ordering, named containers, the rewrites and the running. What reads
> `inventory full`, the locker, the status bar and Lich's own plumbing is not, and **every word
> left out is refused by name**, never ignored: a dropped `marked` would act on every item.
>
> | foreach.lic | Where | Here |
> |---|---|---|
> | `[attr=]value in\|on\|under\|behind <targets>[; commands]`, separators `;` `/` `\|` | `:1546`, `:862` | BUILT, `batch/foreach.rs` |
> | type (the default), sellable, noun, name, fullname, quick; `*`, `a,b`, `/pattern/`, `type=none` | `:1436-1486` | BUILT, on the model's `gameobj` table |
> | `all`, `any`, `everything` | `:1645` | BUILT, anchored: Lich's test is not, so `small` meant everything |
> | unique, first N (or N), after N (skip), sorted, nsorted, reversed | `:1553-1630`, `:609-678` | BUILT, `batch/pick.rs`; unique before reversed, as Lich has it (`VellumFE` reverses first) |
> | marked, unmarked, registered, unregistered | `:244-283`, `:508-523` | refused: they read `inventory full`'s notes |
> | a named container, several, a trailing `?` | `:524-576`, `:1870-1885` | BUILT: a quiet `look in`, the contents off the `<inv>` feed Lich's `GameObj.containers` reads; `stow` found by its target |
> | `floor`, `ground`, `room`; `loot` | `:463-479`, `:1836-1867` | BUILT |
> | `inv`, `fastinv`/`qinv`, `worn` | `:284-443`, `:481-486` | refused: `inventory full` is not read -- there is no committed capture of it, and the corpus is the author's -- and the worn list is not kept |
> | `locker` | `:304-390`, `:1495-1520` | refused: the locker's manifest, bins and door are not modelled |
> | `desc`, `previous`/`last` | `:468-473`, `:487-492` | refused: small, and not asked for |
> | a collective container (`Looking at the mannequins`) | `:553-560` | not read: said as an answer foreach does not know |
> | `item`, `noun`, `name`, `container` filled in | `:1936-1941` | BUILT |
> | verbs completed, a bare verb on the item, `get` before a first `sell`, `return` after a lone `appraise`, a first `drop` as `_drag` | `:1693-1760` | BUILT |
> | a script among the commands | `:1673-1680`, `:1963-1969` | BUILT, as a Hydra command |
> | `move`/`fastmove`, `return`, `unmark`, `echo`, `sleep`, `waitrt(?)`, `waitcastrt(?)`, `waitfor`, `waitre`, `waitmana`/`hp`/`spirit`/`stamina` | `:1970-2121` | BUILT; `move` does not learn the container's id from its first `put` (`:2002-2005`), and `fastmove` is `move` |
> | `stash`, `giveitem`, `pause` | `:2027-2049`, `:2088-2104` | refused: Lich's lootsack settings; a give is `give item to X; waitfor X has accepted`; Hydra has no pause |
> | the `!` prefix | `:1669` | accepted and ignored: every line waits for its prompt |
> | no commands, the list; `Item N of M` every tenth | `:2132-2149`, `:1924-1925` | BUILT |
> | status bar, first-time setup, changelog, help formatting, stopping `;sorter` | `:680-856`, `:937-1346`, `:1391-1404` | not ported: Lich's plumbing |
>
> **Decisions the author may want back**, each the simplest this plan allows: a second batch of a
> kind is refused where the hunt and travel desks replace; a `;` entry nobody knows stops the
> list where Lich went on; the looks are quiet, as the sync's commands are; `all` is anchored.
> `cena-behavior` takes `regex` directly -- the crate `cena-model` already builds -- for the
> player's `/patterns/`.
>
> MEASURED: 47 tests -- `batch_multi` 17, `batch_foreach` 18, `batch_foreach_run` 10 (over the
> real looks in `crates/cena-ui/tests/fixtures/container_looks.xml`), and 2 in
> `crates/cena/src/batch.rs`, a typed `;multi` waiting on the `;go2` it started, through the real
> command table. Eighteen mutations each turned a test red; the one that did not at first -- a
> gate refusal skipped instead of retried -- was a gap, now two tests.

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
