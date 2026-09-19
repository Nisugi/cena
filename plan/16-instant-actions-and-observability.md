# 16 — Instant actions, buff tracking, and observability

**Status: DRAFT. Nothing here is approved.** Written 2026-09-18, immediately after Milestone 1
Step 2's live run, from the author's corrections during that session. Per `CLAUDE.md`, `plan/`
documents are proposals until the author accepts them; this one is a transcription of things the
author said, worked into a shape that can be argued with.

**The author's words are marked AUTHOR throughout.** Everything else is my inference and is the
part to attack.

---

## 0. Why this document exists

Milestone 1 Step 2 shipped a working slice. Within an hour of the live run the author named six
things the design gets wrong or omits, all interlocking, none of them in `plan/12`:

| # | Thing | Status before this doc |
|---|---|---|
| 1 | **Instant actions** exist and must not wait | Absent from every plan document |
| 2 | They are gated by **roundtime**, not by the round-trip window | The queue has no roundtime gate |
| 3 | They are **sent in batches**, several then a trigger | `take_next` forbids this outright |
| 4 | They are confirmed by **their effect**, not by a reply | No mechanism; `send_and_await` is the only send path |
| 5 | Some buffs are **client-tracked** — text only, never in a dialog | `GameState` has no buffs at all |
| 6 | **Logging** must exist from the start | `plan/12` has zero mentions of logging |

Plus two the author raised earlier the same session: **clock/prompt skew** and **typeahead**.

The common thread: `plan/12` §4 designed the command path around one shape — send, wait for the
terminator, get a typed `Outcome` — and that shape is wrong for a whole class of commands.

---

## 1. Instant actions

### 1.1 What they are

> **AUTHOR:** *"There are some things that don't incur any type of roundtime or anything, instant
> actions if you will which shouldn't be subject to typeahead or waiting (depending on the
> action)."*

> **AUTHOR:** *"they can't be activated while in roundtime, but generally I would send them at the
> same time as the following command, sometimes a few in a row, since they activate instantly and
> the command triggers."*

Two rules, and they are not the same rule the queue currently enforces:

- **Gated on roundtime.** Not on whether another command is in flight.
- **Batchable.** Several in a row, then the command they modify.

### 1.1a THE DEFINITION, replacing the seed list as the rule

> **AUTHOR, 2026-09-18:** *"anything that doesn't cause roundtime would be an instant action."*

**A property, not a list.** §1.2 below reads as though instant actions were a set to enumerate and
grow; they are not. Membership is decided by what the command costs, so the seed list is a set of
*examples* and nothing in the design should iterate it.

That widens the category well past abilities:

> **AUTHOR:** *"look &lt;target&gt;, assess &lt;target&gt; would be instant actions"*

MEASURED and already in hand (§1.5): a `look` sent **inside a 7-second roundtime** executed
normally. Observation qualifies -- not as a special case, but because it costs no roundtime, which
is the same reason a sigil does.

### 1.1b THE SECOND AXIS: instant is not the same as fire-and-forget

> **AUTHOR, 2026-09-18:** *"there is a difference between an instant action as in something we fire
> and forget and something that's instant and we care about the answer such as moving."*
>
> **Corrected moments later:** *"I said fire and forget when I meant fire and verify later, don't
> wait for verification then."*

**Nothing is fire-and-forget**, and the correction is the load-bearing part. The distinction is
*when* the answer is checked, never *whether*:

| | fire, verify later | wait for the answer |
|---|---|---|
| **causes no roundtime** | a sigil (check `Effects`) | **a movement** |
| **causes roundtime** | -- | an attack, a cast |

Both columns verify. The left one merely does not **block** on it: the sigil goes out, the next
command follows immediately, and the effect is confirmed afterwards by an id lookup (§2). **What
makes batching possible is deferred checking, not absent checking.**

- *Does it cause roundtime?* decides the **gate** -- `Gate::None` or `Gate::Roundtime`.
- *When is it verified?* decides the **path** -- `send_now` or `send_and_await`.

**The top-right cell is why both send paths exist**, and it is the one §1.4 did not anticipate.
`send_now` was designed for fire-and-forget, and §2's confirm-by-effect was built on the assumption
that an instant action's result shows up in a dialog. A movement fits neither: there is no roundtime
to wait out, so the gate must be `None`, but the client absolutely cares whether the room changed,
and the answer arrives as a room frame rather than as an effect.

So `Gate` and the choice of send path are **orthogonal**, and a caller sets them separately. The
combination that matters most in practice -- `Gate::None` with `send_and_await` -- is movement, and
§5.2g is what it looks like in a travel route.

#### `move` and `travel`: the same command, two verification policies

> **AUTHOR, 2026-09-18:** *"movement is instant and we care. True but it depends. Let's call it
> `move` and `travel` so move we care and travel we dont. giving us the ability to fast travel."*

**This is what the rate measurement was for.** Both operations send cardinal directions; they
differ only in **when they verify**, which puts them in different columns of §1.1b's table:

| | Sends | Verifies | Path |
|---|---|---|---|
| **`move`** | one direction | the room changed, **before continuing** | `send_and_await` |
| **`travel`** | a run of directions | arrival at the destination, **at the end** | `send_now`, batched |

`travel` is the **fast path over a known route**. The mapdb already says which rooms connect, so
confirming each hop individually buys nothing -- the client is not discovering the path, it is
walking one it already has. Verification moves to the end: *am I where I meant to be?*

**`move` is not obsolete.** It is what `travel` degrades to, and it is correct wherever the route
is unknown, contested, or the answer changes what happens next.

##### Why this is safe to batch, given everything measured

- **Depth, not rate, is the bound** (§5.2f). A `travel` batch is capped at the entitlement -- 3 on
  the author's account, 2 standard, **1 on free-to-play, where `travel` has no batch and collapses
  into `move`**.
- **The rate ceiling is far away.** 21.5 commands/second sustained clean; no travel route
  approaches it, because the character's own movement delay paces the route.
- **StringProcs are natural drain points** (§5.2g). A door needs its replies, so the batch ends
  there, the queue empties, and the next cardinal run starts from a clean buffer. A route with a
  door every few rooms paces itself with no delay logic at all.

##### THE SAFEGUARD: a typeahead refusal during `travel` is a POSITION signal

> **AUTHOR, 2026-09-18:** *"we got some safeguard that during travel if we hit the typeahead
> message, then we need to stop sending commands, catch up to the room and recalculate no?"*

**Yes, and it is the one piece of recovery `travel` cannot do without.** The three steps are the
author's:

1. **Stop sending.** Every direction still queued locally is now suspect, because it was computed
   from a room the character may not be in.
2. **Catch up to the room.** Drain what the server actually sent and read the current room id from
   the wire -- not from the client's model of the route.
3. **Recalculate.** Re-path from where the character *is* to the destination, and resume.

##### Why a refusal means more during `travel` than anywhere else

For a `look`, a typeahead refusal is a throttle message and nothing else -- §5.2d measured that the
next command is accepted immediately, with no cooldown. **For a movement it is also a statement
about position:** one of the moves the client believed it had made did not happen.

That is the difference between `travel` and every other `send_now` caller. A refused sigil leaves
the world exactly as it was; a refused *move* leaves the client's model of the route **one room
ahead of reality**, and every subsequent queued direction is then computed from the wrong room.

The naive recovery -- re-send the refused direction and carry on -- is wrong here even though it is
what Lich does. VERIFIED that Lich retries (`lib/common/move.rb:392-394`):

```ruby
elsif line =~ /^Sorry, you may only type ahead/
  # clears on its own once the queue drains, but bounded all the same
  remedy.call(:typeahead, MAX_ROLLS, :roundtime) { Script.execution_sleep 1 }
```

and its comment -- *"clears on its own once the queue drains"* -- is correct **for Lich**, because
Lich moves **one room at a time**. With a single command outstanding there is nothing to
recalculate: retry the direction and the position is consistent again.

**`travel` batches, so that reasoning does not transfer.** With three directions outstanding, a
refusal says one of them was dropped but not *which*, and "retry the last one" assumes the refused
command was the final one sent. That is likely -- the server processes in order, so the overflow
should be the tail -- but it is **inference, not measurement**, and a wrong guess walks the
character down the wrong exit.

Re-pathing from the observed room needs no such assumption. It costs one mapdb lookup and is
correct whichever command was dropped, which is why it is the safeguard rather than the fallback.

##### It should be rare, and that matters for the design

Given §5.2f, a correctly-written `travel` **should never see this**: it batches at most the
entitlement, StringProcs drain the queue regularly, and the rate ceiling is twenty times what a
route produces. So the refusal handler is a **safety net for a client bug or a server slowdown**,
not a routine path.

That argues for making it loud rather than silent -- a `travel` that quietly recovers from
typeahead refusals every few rooms is a `travel` that is batching wrong, and the log should say so.

##### The failure mode `travel` must handle, and `move` does not

**Not verifying each hop means a wrong turn is discovered late.** If a room does not connect the
way the mapdb claims, or the character is stopped mid-run, `travel` finds out at the end -- with
several commands already sent into a position it did not expect.

`move` cannot have this problem, because it checks before continuing. So `travel` owes something
`move` does not: **the recovery has to be re-pathing from wherever it actually is, never resuming
the original plan.** That is cheap -- the room id says where it is and the mapdb can path again --
but it must be designed in rather than bolted on, because the naive "retry the rest of the list" is
wrong in exactly the case that matters.

**UNVERIFIED:** whether a refused *movement* leaves the path intact (§5.2g). A refused `look` is
harmless; a movement that is silently dropped rather than refused would leave `travel` believing it
had moved when it had not, which is the desync `plan/12` §5.4 exists for. **This should be measured
before `travel` is built** -- and it is the same one-deliberate-overflow test the probe already
does, with a direction in place of a `look`.

#### `Sent` is not a receipt, it is the start of a verification

This corrects how §1.4's return value was described. `Sent::Ok` says the bytes went out. On the
fire-and-verify-later path that is the **beginning** of a check the caller still owes, not the end
of one -- and a caller with nowhere to check later is not deferring verification, it is a caller
that never verifies.

So every `send_now` call site needs a named place the answer will show up:

| Command | Verified by |
|---|---|
| a sigil, an instant cast | `Effects` id lookup (§2) |
| a movement | the next room frame |
| `look`, `assess` | the text it returns |

**A `send_now` with no such place is a bug**, and it is the shape §2's confirm-by-effect was
written to prevent -- but §2 assumed the answer was always an effect. Movement shows it is not: the
mechanism is "check somewhere specific, later", and the *somewhere* varies by command.

### 1.2 The seed list

> **AUTHOR:** *"Look there's not a lot of them ok, 515 rapid fire is one, 140 wall of force is one,
> sigils from Guardians of Sunfist is a good start, we can adjust for others we find."*

| Action | Wire form | Source |
|---|---|---|
| Rapid Fire | spell `515` | AUTHOR |
| Wall of Force | spell `140` | AUTHOR |
| Guardians of Sunfist sigils | spells `9701`–`9720` | AUTHOR, table portable from Lich |

The sigil table is **already structured data** in
`reference/lich-5/lib/gemstone/societies/guardians_of_sunfist.rb`: 20 entries with `rank`,
`short_name`, `long_name`, `spell_number`, `cost {stamina, mana}`, `duration` and `summary`.
`plan/13` §4a says to port data like this rather than reinvent it. Measured: 20 `spell_number`
entries, 9701 through 9720 contiguous.

Invoked as **text**, not by number — `fput 'sigil of escape'` (`lib/gemstone/fog.rb:161`).

**This list grows.** It is explicitly a seed, not a closed set.

### 1.3 The cost of getting this wrong, today

> **AUTHOR:** *"those sigils using fput suck because they wait for a response instead of being
> instant."*

Cena has the same defect, structurally. `CommandQueue::take_next`
(`crates/cena-session/src/queue.rs:176-179`) returns `None` whenever `in_flight.is_some()`, and
`SessionHandle::send_and_await` is the **only** send path (`command.rs:192`; the other three public
methods are `new`, `generation`, `claim`/`release`).

So: **`sigil of escape` — the sigil you use to leave a fight you are losing — currently queues
behind whatever the running behavior last sent, for up to that command's full roundtime.**

That is the worst case for the ability most likely to be time-critical, and it is a direct
consequence of a design decision (`plan/12` §4.4, one open window at a time) that was correct for
ordinary commands and never considered these.

### 1.4 BUILT 2026-09-18: a second send path

```
send_and_await(...)   existing. Opens a window, waits for a terminator, returns a typed Outcome.
send_now(...)         NEW. No window. Does not block, is not blocked.
```

`send_now`:

- Opens **no** `InFlight` — nothing to wait for, nothing to time out
- Is admitted while another command's window is open — this is the batching in §1.1
- Is gated on **one** thing: not currently in roundtime
- Returns immediately; confirmation is §2's problem, not the send path's

The roundtime gate is checkable from state we already keep:
`GameState::roundtime_ends: Option<u32>` (`crates/cena-model/src/state.rs:74`), fed by
`Frame::RoundTime { value }` (`frame.rs:153-154`).

**Built as described**, with three things the sketch did not settle:

- **`Sent`, not `Outcome`.** `Outcome::Confirmed` carries "the frame from the wire that matched";
  `send_now` matches no frame, so reusing it meant fabricating a `Frame::Prompt` that never crossed
  the wire. `Sent::Ok { at }` carries the server second the gate was decided on instead — the
  evidence it ran against, which is the fact a log actually wants.
- **`Gate` is an enum, not a bool.** The author's *"(depending on the action)"* says the gate varies
  per action. `Gate::None` is that parenthesis, written at the call site rather than as a `false`.
- **Unknown clock refuses `Transient`.** `plan/12` §5.2: unknown is not permission. `Transient`
  rather than `Roundtime` because a prompt will arrive and then the question is answerable —
  "ask again", not "no".

It rides the **same channel** as `Inbox::Command`, which is what makes §1.1's batching ordered: two
channels give no ordering guarantee between the sigil and the attack it modifies.

`crates/cena-session/tests/send_now.rs`, 7 tests. The first is §1.3's complaint executable — it
opens a window, leaves it open, and asserts the sigil still reached the wire.

#### 1.4a MEASURED CORRECTION: an unreported roundtime is OVER, not unknown

Building the gate surfaced a bug in `GameState::in_roundtime`, which returned `None` whenever
`roundtime_ends` was `None` — treating "no roundtime has ever been reported" as an unknown.

**That is the normal state at login**, and under the old reading every `send_now` refused until the
character's first roundtime arrived: the feature disabled exactly when a fresh character most wants
a sigil.

VERIFIED against Lich, per `CLAUDE.md`'s rule that the protocol facts are already there:
`@roundtime_end = 0` (`reference/lich-5/lib/common/xmlparser.rb:62`), compared as
`roundtime_end - now > 0` (`lib/global_defs.rb:283`). An unreported roundtime is **in the past**,
not unheard-of.

The asymmetry is now written into the type: the **clock** must be observed before anything can be
compared (`None` if no prompt has arrived), and the **roundtime** has a meaningful default. §5.2 is
about not inventing beliefs; concluding "a roundtime nobody has mentioned is not running" invents
nothing.

### 1.5 OPEN — what happens if one is sent during roundtime?

**UNVERIFIED.** Does the server refuse with `...wait N`, silently drop it, or hold it? This decides
whether the gate is a hard block or an optimistic try-and-retry. The author will know; I have not
asked yet.

**CLOSED BY MEASUREMENT 2026-09-18.** A `search` produced a 7-second roundtime
(`<roundTime value='1789780053'/>` against `<prompt time="1789780046">R&gt;`), and a `look` sent
immediately afterwards with `Gate::None` **executed normally** -- a full room render, no
`...wait N`, no refusal, no drop.

**So roundtime does not gate every command.** `look` is free during roundtime, and the server says
so by running it. That is a correction to the shape of the gate rather than to its code:
`Gate::Roundtime` is right for the actions that *are* gated, and applying it to everything would
refuse commands the server would happily have run.

It also means `Gate` earns its place as an enum rather than a bool sooner than expected -- the
first measured case of an ungated command is `look`, which no one had listed as an instant action.

**Still UNVERIFIED:** what the server does with a genuinely roundtime-gated action (an attack, a
sigil) sent inside a roundtime. `look` answers the question for commands that were never gated; it
cannot answer it for the ones that are.

---

## 2. Confirmation by effect

### 2.1 The insight

> **AUTHOR:** *"we can still verify the event happened from those instant actions because they all
> return with an effect in buffs or cooldowns or whathaveyou."*

This decouples **send** from **confirm**, which is what makes `send_now` safe rather than
fire-and-hope. A behavior does not parse a reply; it observes that the world changed.

For a sigil this is *better* than a reply would be: a buff that persists 19 minutes
(`sigil_of_contact`, `duration: 1140`) is a far more durable fact than one line of text that can
scroll past inside combat spam.

### 2.2 The wire already carries it

`Frame` (`crates/cena-protocol/src/frame.rs:229`) already models "a row of `ActiveSpells` /
`Buffs` / `Debuffs` / `Cooldowns`."

And `plan/15` §2.7 measured how those four arrive: `clear='t'` followed by a **full refill on one
line** — 409/409, 193/193, 150/150, 145/145 occurrences. **Unanimous.** A refill is always
complete, never partial.

That is a strong property for this design: a behavior checking "is X in Buffs?" reads a complete
snapshot and can never catch a half-updated list.

### 2.3 The gap is at the model layer

`GameState` holds room, prompt, hands, roundtime, vitals, unknown tags — **no buffs, no
cooldowns.** `plan/12` §7.1 scoped M1 to exactly that and put spells in the Out column.

Correct for M1. It is now the thing standing between us and confirmable instant actions.

**This is a scope decision for the author,** not something to slip in: it widens `plan/12` §7.1's
In column. The code cost is small — the frames already parse — but the boundary is the author's.

---

## 3. Client-tracked buffs

### 3.1 The case that breaks the clean rule

> **AUTHOR:** *"Briar Weapon Flares has an ability to raise the weapon and gain a buff, this is an
> instant action but that buff doesn't show in buffs, it's one we would parse with the parser and
> record the buff ourselves."*

So §2's rule is not universal. Two sources:

| Source | Confirmed by | Expiry |
|---|---|---|
| **Server-tracked** | the effect appears in a `Buffs`/`Cooldowns`/`ActiveSpells` refill | server refreshes it |
| **Client-tracked** | we parse a text line and record it | **we** must expire it |

A behavior that only checks the dialogs sees Briar as permanently inactive, and re-raises the
weapon forever.

### 3.2 One collection, not two

> **AUTHOR:** *"I would expect our parsing of the buff would add it to the buffs that are already
> recorded. It's all the same thing so the information should be together."*

**Settled by the author.** One `buffs` collection. Where an entry came from is a property of the
entry, not a reason to split the storage. A behavior asking "am I buffed?" must not have to know
which mechanism tracks which ability.

My earlier instinct — separate collections — was wrong and is recorded here so it is not
re-proposed.

### 3.3 What provenance is still needed for

Not for storage. For **staleness**, and only really at one moment: reconnect.

`plan/12` §5.2 makes `Unknown` a first-class value because a stale belief is worse than an admitted
gap. After a reconnect:

- a **server-tracked** buff re-asserts itself on the next refill — self-healing
- a **client-tracked** buff is a belief about a fact we can no longer verify from any source

So an entry should carry enough to know its own origin and expiry. That is a field, not a second
collection.

This is the same problem as `plan/12` §5.4's desync, arriving from a different direction: typed
state that accumulates silently and is never contradicted.

---

## 4. Clock and prompt-time skew

### 4.1 The author's idea

> **AUTHOR:** *"Is desync where we like compare the prompt time to our time and watch for changes?"*

It is not what `plan/12` §5.4 means by desync (that is recovery from truncation-class parse
errors). It is a **different and absent mechanism**, and worth having.

`desync` appears nowhere else in the plan documents except `plan/10`, where it means the eaccess
read/write streams slipping out of step — unrelated.

### 4.2 Why it is load-bearing, not a nice-to-have

`roundtime_ends` is stored as an **absolute epoch second** — the server's clock
(`crates/cena-model/src/state.rs:68-74`). Deciding "are we in roundtime right now", which is
**exactly §1.4's gate**, means comparing server time against local time.

**Nothing currently checks that those two share an origin.** If they disagree by N seconds, every
instant-action gate is wrong by N, and every behavior waiting on roundtime waits wrong.

`<prompt time='...'>` gives a continuous free sample of the server's clock. Diffing it against
local time detects:

- **skew** — our clock and theirs disagree; every roundtime calculation is off
- **lag** — we are processing prompts N seconds old
- **a frozen feed** — prompt time stopped advancing while the socket stayed open

The third is a liveness signal the current design has no equivalent for.

---

## 5. Typeahead

### 5.1 The wire signal is known

`"Sorry, you may only type ahead"`, appearing in three independent reference codebases
(`inventory/01:157`, `inventory/03:189`, `inventory/08:1056`). All three respond the same way:
**wait 1s, resend.**

```ruby
elsif string =~ /Sorry, you may only type ahead/
  return fail_with.call(:interrupted) if wait.call(1)
```

### 5.2 Why Cena's exposure differs from Lich's

Lich needs elaborate typeahead handling partly because its scripts can fire commands without
waiting. Cena's queue holds **one `InFlight` at a time**, so it is structurally unlikely to trip
the limit — today.

**§1.4 changes that.** Batching several instant actions plus a trigger is precisely the pattern
that makes the limit reachable. The two features are one problem: the thing that makes instant
actions useful is the thing that makes typeahead relevant.

So a send-rate policy belongs with `send_now`, not as separate later work.

### 5.2a CORRECTED BY MEASUREMENT: the limit is an ENTITLEMENT, not a constant

> **AUTHOR, 2026-09-18, on seeing the probe's output:** *"it's 1, and +1 for premium"*

**MEASURED 2026-09-18** (`nerten-2026-09-18_20-06-49-000.bytes`), five `look`s sent 1ms apart:

```
Sorry, you may only type ahead 2 commands.     x2
```

**Two, not one** -- and the arithmetic closes exactly: 5 sent, 2 refused, **3 room renders**. With
a depth of 2 beyond the executing command, `look` #1 runs, #2 and #3 buffer, #4 and #5 are refused.

§5.2a as first written is **wrong**, and the way it was wrong is worth keeping. It read Lich's
`==` comparison as proof the number never varies:

> *"an implementation that has run against these servers for two decades does not parse the number,
> because the number does not vary."*

The inference was backwards. Lich does not parse the number because **Lich was written against a
base account**, and against a premium one its `line == 'Sorry, you may only type ahead 1 command.'`
**silently fails to match** -- the branch never fires, and the script does not back off. Vellum
quotes the same literal and inherits the same bug. The Kelfour newsletter describes the base
account because in the 1990s that is what there was.

**Three sources agreeing did not make them right.** They agree because two of them copied the
third's assumption, and none of the three had a premium account to contradict it.

This is the **second** entitlement mistaken for a protocol constant in one day: the login run's
character-slot count was also an account fact read as a game fact (`plan/15` §2a.4a). The pattern
is worth naming -- *if a number describes what this account may do, it is a property of the
account*, and the wire reports it per-session rather than defining it.

**Consequence for Cena:** the number is **parsed**, never matched. A client that hard-codes `1`
works for base accounts and quietly breaks for premium ones, which is the worst failure shape --
it does not error, it just stops backing off. The probe's own matcher already excluded the number
(`crates/cena/src/probe.rs`, `TYPEAHEAD_REFUSAL`), which is the only reason this run could observe
a `2` at all.

### 5.2a-old SUPERSEDED: what reading alone said

**The limit does not need discovering.** Before building a probe to find N, the rule in `CLAUDE.md`
— *"the protocol facts are already in Lich, dig them out"* — was applied, and three independent
sources give the same answer:

| Source | Evidence |
|---|---|
| Lich | `line == 'Sorry, you may only type ahead 1 command.'` — an **exact string equality**, not a regex with a capture group (`lib/global_defs.rb:1905`, `:1971`) |
| VellumFE | the same literal, quoted in `src/core/move_feedback.rs:55` and `state/travel_ticks.rs:757` |
| Kelfour Edition vol. I no. VIII | `"Only 1 type ahead line allowed."` — a **1990s player newsletter**, describing the same limit from the player side |

Lich comparing with `==` rather than matching `type ahead (\d+) command` is the strongest of the
three: an implementation that has run against these servers for two decades does not parse the
number, because the number does not vary.

**So the limit is one command in the buffer beyond the one executing.** N is not a parameter to
find.

### 5.2b What the newsletter adds that the code does not

The Kelfour piece is worth reading in full (`reference/wiki_clean/Kelfour Edition volume I number
VIII.txt:810-845`) because it describes the **mechanism**, which neither reference implementation
states:

> *"Sometimes with this macro you get an error message, 'Only 1 type ahead line allowed.' That means
> the third command was sent before the first two commands processed. **This happens only during
> slow downs**"*

Three things follow, and they reframe §5.2 entirely:

1. **The limit is on commands the server has not yet PROCESSED, not on commands sent per unit
   time.** It is a buffer depth, not a rate.
2. **It is therefore load-dependent.** The same macro works for years and fails during a server
   slowdown — which means a fixed inter-command delay tuned on a quiet evening is tuned against the
   wrong variable.
3. Their remedy is the same as Lich's: a delay between commands. The newsletter uses `.5` seconds;
   Lich sleeps `1`.

That is also why §1's batching is safe as built and the danger is smaller than §5.2 feared: sending
a sigil and then an attack is **2 commands**, and the roundtime between them is exactly the
processing gap the limit is measuring.

### 5.2b-bis VERIFIED: the entitlement ladder, from the official benefit list

> **AUTHOR, 2026-09-18:** *"premium gets 3 per 0.1s, standard gets 2 per 0.1s, and free2play gets
> 1 per 0.1s, and premium can also purchase an additional typeahead spot so they can max out at 4,
> but we assume no one has bought one."*

The **depth half** of that is confirmed by the wiki, which documents the purchasable line outright
(`reference/wiki_clean/Long-term Benefits.txt:121`):

> *"Tired of seeing 'You can only type ahead one line'? Choose this benefit, and you will get
> another type-ahead line. This one is **in ADDITION to the standard bonus type-ahead line you get
> for being a Premium member**. This will persist even if the account is no longer Premium."*

900 premium points, from a fountain on the Isle of Four Winds, and *"Premium members may only
choose this benefit ONCE"* (`Premium.txt:141`, `Premium Point costs_saved posts.txt:89`).

So the ladder is **base + tier + purchase**, and the accepted count is one more than the buffer
because a command executes while the rest queue:

| Account | Type-ahead lines | Accepted at once |
|---|---|---|
| **Free-to-play** | **0** | **1** |
| Standard | 1 | 2 |
| Premium | 2 | **3** (MEASURED, this account) |
| Premium + purchased | 3 | 4 |
| Lapsed premium who bought | 2 | 3 |

> **AUTHOR, 2026-09-18:** *"f2p gets 0 typeahead"*

The wiki copy in `reference/wiki_clean` documents the premium and purchased lines but says nothing
about free-to-play, so the top row is on the author's authority rather than a citation.

**The F2P row is the one that constrains the design most**, and it is easy to miss what zero means:
a free account has **no buffer at all**. Every command must finish before the next is accepted, so
*any* second command sent before the first completes is refused. There is no batching on F2P --
not a smaller batch, none.

That rules out a whole class of implementation: **`send_now` cannot assume it may ever send two
commands back to back.** `plan/16` §1.1's "a few in a row, then the trigger" is a premium-and-above
capability, and on F2P the sigil-plus-attack pattern must degrade to strict one-at-a-time rather
than merely slowing down. A client built around "batch a few" would work for the author and fail
for the free test account -- which is the account `CLAUDE.md` says exists for testing.

The bottom row is the other awkward one: the purchased benefit **persists after premium lapses**,
so tier does not determine the number either. **An account's entitlement cannot be inferred from
its subscription** -- it has to be read from the wire, which is what `2 commands` in the refusal is
for.

This is the third time tonight an entitlement has been mistaken for a protocol constant, and the
wiki's own example line -- *"You can only type ahead one line"* -- is the **standard** account's
wording that Lich hard-codes. It matches on exactly one of the five rows.

**And on F2P it may never appear at all**, since with zero lines the server's refusal for a second
command may be worded differently, or may be a different mechanism entirely. **UNVERIFIED** -- the
free test account can settle it, and that is the cheapest of the open questions to answer.

**Cena parses the number from the refusal and treats it as per-account state.** It is not a
constant, not a function of tier, and not knowable before the first refusal.

#### The timing half is NOT established

*"per 0.1s"* is the open question, not a finding. It is what the next run tests, and the evidence
so far brackets rather than settles it: run A was clean at **3.6 cmd/s**, run B failed at **54
cmd/s** (§5.2d-bis, corrected). The next run at 150ms sits near **19/s**, between them.

**Depth and rate are independent**, and only depth is settled. A client that assumed "3 per 100ms"
today would be hard-coding an entitlement *and* an unmeasured rate at once.

And on **free-to-play the rate question does not even arise**: with zero type-ahead lines there is
no batch to pace. The pacing work above is a premium-and-above concern, and the F2P path is
strictly one command at a time.

### 5.2c MEASURED: what the probe run settled

**Run 2026-09-18 20:06, by the author, `CENA_PROBE=typeahead`.** 16 probe sends, all present in the
event log with their gaps; the `look` behavior was running concurrently at 1s intervals throughout,
so every result below was obtained *with* competing traffic rather than on an idle socket.

| Q | Question | Answer |
|---|---|---|
| 2 | Drop or refuse? | **REFUSED, individually, and the rest still run.** 5 sent → 2 refusals → 3 room renders. No disconnect, no lost session, no truncation. |
| 3 | A command sent during roundtime? | **It ran.** See §1.5, now closed. |
| 4 | What gap is clean? | **Every rung**, 15ms through 1009ms, with the behavior also sending. |

**Q4 needs care in the reading.** The ladder sends *two* commands per rung, and the buffer is 2 --
so on this account a pair can never exceed it, whatever the gap. The rungs came back clean because
the test was under the limit by construction, not because 15ms is a safe interval. **What the burst
shows is the real bound: depth, not rate.** Five at 1ms refused; two at 1ms would not have.

That is Kelfour's point restated with numbers: the limit counts **unprocessed commands**, so the
thing to bound is *how many are outstanding*, never *how fast they were sent*. A client that paces
by delay is solving the wrong problem; a client that tracks outstanding commands solves it exactly.

**Consequence for `send_now`:** §5.2's fear that batching makes the limit reachable is real but
small. A sigil plus its trigger is 2 -- at or under the base entitlement of 1+1, and comfortably
under a premium 2+1. The pattern to avoid is a long unbroken chain, not a pair.

### 5.2d MEASURED: the buffer refills instantly. The pause does not matter.

**Run 2026-09-18 20:18**, the author's sequence: `4 looks | 500ms | 4 | 300ms | 4 | 200ms | 4 |
100ms | 4`. One pass, no reset between groups.

Taken from the wire (`nerten-2026-09-18_20-18-07-000.bytes`), not from the console -- the console
undercounted, see §5.2e:

| group | pause after | sent | refused | accepted |
|---|---|---|---|---|
| 1 | 500ms | 5\* | 2 | 3 |
| 2 | 300ms | 4 | 1 | 3 |
| 3 | 200ms | 4 | 1 | 3 |
| 4 | 100ms | 4 | 1 | 3 |
| 5 | — | 4 | 1 | 3 |
| 6 | — | 4 | 1 | 3 |

\* group 1 absorbed a leftover `look` from phase 1.

**Totals: 25 sent, 7 refused, 18 accepted.** 25 − 7 = 18 exactly; nothing was dropped silently.

**Every group accepted exactly 3, at every pause.** The accepted count did not move between a
500ms pause and a 100ms one -- and groups 3, 4 and 5 all landed inside the **same server second**
(`<prompt time=1789780700>`, 12 prompts), so the pauses between them were invisible to the server's
own clock and still made no difference.

**A refusal is followed immediately by acceptance.** Every one of the 7 refusals is followed
directly by executed commands in the frame stream -- no cooldown, no penalty window, nothing to
wait out.

> **AUTHOR, on reading this:** *"so this says sending it .1 second after a refusal accepts new
> commands?"*

Yes, and more strongly: the acceptance is not a function of elapsed time at all. It is a function
of **how many commands are outstanding**.

#### What the mechanism actually is

The buffer is **depth 2** (`1 + premium`) and it refills **as the server processes**, not on a
timer. A group of 4 against a server executing them is therefore:

```
look 1  -> executes immediately (nothing outstanding)
look 2  -> buffered (1 of 2)
look 3  -> buffered (2 of 2)
look 4  -> REFUSED (buffer full)
```

Three accepted, one refused -- invariant, because it is set by the **buffer depth plus the one
executing**, not by the rate. The pause afterwards is irrelevant: by the next group the server has
drained what it took, and the same 3-of-4 happens again.

This is Kelfour's *"only during slow downs"* restated exactly. The limit counts **unprocessed
commands**, so what governs it is how fast the server is working, and a client cannot influence
that by waiting longer between bursts.

#### Consequence for Cena: bound the outstanding count, never the rate

**A delay-based send policy is the wrong shape** and this run rules it out empirically -- 500ms
and 100ms produced identical results. Any inter-command delay is a guess about server load that
the client has no way to verify.

What works is bounding **commands in flight**: at most `entitlement + 1` outstanding before
waiting for evidence one has executed. Cena is already positioned for this -- `send_and_await`
holds one window at a time, so the ordinary path cannot exceed it. It is `send_now` that needs the
bound, and the bound is a **count**, not a sleep.

**`send_now` needs no delay and no backoff ladder.** It needs to know how many of its sends have
not yet been answered. That is a smaller mechanism than §5.2 anticipated.

### 5.2d-bis NARROWED: the depth holds; the claim that pauses do not matter does not

> **AUTHOR, on reading the first draft of this section:** *"is the model dead?"*

**No -- and calling it dead was an overstatement worth correcting.** The run falsified **one
clause** of §5.2d, not the model:

| Claim | Status |
|---|---|
| The buffer is depth 2 (`1 + premium`), so **3 are accepted at a time** | **HOLDS.** Measured in both runs; the 50ms run's first three rounds were clean, 9 commands, before anything backed up. |
| A refusal carries no cooldown -- the next command is accepted immediately | **HOLDS.** Unchallenged by this run. |
| Therefore **the pause does not matter** and a delay-based policy is the wrong shape | **FALSIFIED.** |

Only the third was wrong, and it was always the weakest: it was an *inference* from the first two
rather than something measured. The depth was measured; its irrelevance to pacing was assumed.

**Run 2026-09-18 20:23**, the author's sequence: `(3 looks | 50ms) x 10`.

§5.2d predicted **zero refusals** -- 3 was exactly what every group accepted in the previous run,
so if the accepted count were set by buffer depth alone, 3 could be sent forever. It was the
model's own prediction used as the input, precisely so it could fail.

**It failed. 30 sent, 17 refused, 13 accepted.**

| round | sent | refused |
|---|---|---|
| 1 | 3 | 0 |
| 2 | 3 | 0 |
| 3 | 3 | 0 |
| 4 | 3 | **3** |
| 5 | 3 | 1 |
| 6 | 3 | 2 |
| 7 | 3 | 3 |
| 8 | 3 | 0 |
| 9 | 3 | 3 |
| 10 | 3 | 3 |

Round boundaries are approximate -- replies straddle them, see §5.2e -- but the shape is not in
doubt: **the first three rounds were clean and refusals began at round 4**, then continued for the
rest of the run.

#### What this corrects

§5.2d's model was *"the buffer refills as the server processes, so the accepted count is a depth,
not a rate"*. The first half stands; **the conclusion drawn from it was too strong.** The previous
run's groups were separated by 100-500ms pauses, and each group started with the buffer already
drained -- so 3-of-4 was measured under conditions where the server was always caught up. That run
could not distinguish "3 is the depth" from "3 is what drains between groups", because in it those
were the same number.

This run separates them. At 50ms the server **does not finish draining between rounds**, so the
outstanding count climbs: three clean rounds while it keeps up, then refusals once it does not.

**MEASURED, and this is the mechanism.** From the event log, the 10 rounds went out at **~61ms**
apiece (the 50ms sleep plus ~11ms of send overhead), so all 30 commands crossed the wire between
`20:23:35.857` and `20:23:36.411` -- **0.554 seconds, about 54 commands per second**. They arrived
inside **two server seconds** (`1789781016` alone carried **18** of the 29 prompts).

> **CORRECTED.** This first said **~19/s**, from dividing 30 sends by 1.57s. That span included the
> 4-second tail wait, which is not sending time. The sends themselves span 0.554s. The error made
> the two runs look five-fold apart when they are **fifteen-fold** apart, and it understated how
> far past the limit run B was.

Run A spread 25 commands over ~7 seconds: **3.6/s**. So the difference between the runs is not the
group size -- it is **how much work per second** the server was asked to do, and 54/s is far past
what it absorbs.

#### The corrected model

The limit is a **queue of depth 2** that drains at a finite rate. Both facts matter:

- **Depth** bounds how many may be outstanding at any instant -- 3 accepted at a time, as §5.2d
  measured.
- **Drain rate** bounds the sustained throughput. Send faster than the server executes and the
  queue stays full no matter how the sends are grouped.

§5.2d's *"a delay-based send policy is the wrong shape"* was therefore **wrong**. Pausing does not
help *within* the depth -- which is all that run could show -- but it absolutely helps *across*
rounds, because it is what lets the server catch up. Those pauses were not doing nothing; they
were the reason that run never backed up, and I read their irrelevance within a group as
irrelevance between groups.

**The next run tests the boundary rather than re-litigating the model** (AUTHOR: *"set the test to
0.15"*). At 150ms a round takes ~161ms, so the cadence is about **19/s** -- between run A's clean
3.6/s and run B's failing 54/s, and still **five times** run A's rate. That makes it a real
question rather than a formality.

#### What this means for `send_now`

The bound is **both**: at most `entitlement + 1` outstanding, *and* not faster than the server
drains. The client can observe the first exactly (it knows what it sent and what came back) and
cannot know the second, which moves with load -- exactly Kelfour's *"only during slow downs"*.

So the honest policy is **count-based with evidence**: track commands sent against replies seen,
and stop sending while too many are unanswered. That handles both bounds with one mechanism and
needs no tuned delay, because a server that has slowed down stops replying and the count stops
falling on its own.

**This is still a smaller mechanism than a backoff ladder** -- but it needs the reply side, which
`send_now` currently ignores. That is the open design question §5.3 should now carry.

### 5.2f MEASURED: 150ms is clean. The boundary is between 21/s and 54/s.

> **AUTHOR, 2026-09-18:** *"that run was perfect."*

**Run 20:30, `(3 looks | 150ms) x 10`. VERIFIED ON THE WIRE: 30 sent, 0 refused, 30 executed.**
The only two refusals in the whole log belong to phase 1's deliberate 5-command burst.

Rounds went out at a steady **153-157ms** and the 30 sends span **1.394s -- 21.5 commands per
second**, clean.

#### The ladder now brackets the drain rate

| Run | Shape | Achieved | Result |
|---|---|---|---|
| A (20:18) | 4 per group, 100-500ms pauses | 3.6/s | clean |
| B (20:23) | 3 per group, 50ms | **54/s** | 17 of 30 refused |
| C (20:30) | 3 per group, 150ms | **21.5/s** | **clean, 30 of 30** |

**The boundary lies between 21.5/s and 54/s**, and 21.5/s is six times run A's rate -- so this is
not a marginal pass near the clean end. It is a real result well above the only other clean
datapoint.

#### What this settles about the model

§5.2d-bis narrowed the model to "depth 2, draining at a finite rate". Run C confirms both halves
and puts a number on the second:

- **Depth holds.** 3 accepted at a time, now in three consecutive runs.
- **Rate is real but generous.** Well over 20 commands per second is sustainable indefinitely --
  far above anything a client, a behavior or a human would produce in normal play.

**That is the practically important finding.** Run B's failure needed **54 commands per second**,
which no real client generates: a behavior sending a command per round-time, a player typing, even
a sigil batch plus its trigger, are all orders of magnitude below it. The rate bound exists and is
not one Cena will meet by accident.

#### Revised consequence for `send_now`

§5.2d-bis called for a count-based policy with reply tracking, on the strength of run B. **Run C
makes that look like over-engineering.** The depth bound is the one that bites in practice -- it is
hit by *three commands sent together*, which is exactly `plan/16` §1.1's batching -- and the rate
bound is only reachable by a client that is malfunctioning.

So the useful mechanism is the simpler one: **do not send more than the entitlement allows in one
batch**, read the entitlement from the wire's refusal, and treat a refusal as a signal to stop
batching rather than as something to pace around. No reply-tracking, no delay ladder.

The reply-tracking design is not wrong, but it solves the rate bound, and the rate bound is not the
problem. **UNVERIFIED whether a slow server pulls 21.5/s down** -- Kelfour's *"only during slow
downs"* says it can -- but a client that never batches past its entitlement is not exposed to that
either.

### 5.2g THE REAL CASE: cross-town travel, and why it is two modes not one

> **AUTHOR, 2026-09-18:** *"There is one thing beyond testing that would come anywhere close to
> this and that is movement. travelling from one town to another is where this would come in to
> play. It would also only work on cardinal directions, when you get to like a door that needs to
> be opened or anything like that, we call them stringprocs in our mapdb, it needs to kind of catch
> up and get responses for the mini script it needs to do."*

This is the only real workload that approaches the limit, and it settles what `send_now` is
**for**. Everything above measured the ceiling; this says which side of it the client actually
lives on -- and the answer is **far** below it, in every use the author named.

#### What `send_now` is actually for, in the author's words

> **AUTHOR, 2026-09-18:** *"send_now is for the instant cast abilities, a few here and there, and
> then movement for the most part."*
>
> *"maybe for changing stance to offensive and attacking in the same instant, or targeting, stance,
> attack all at once."*

Three uses, and they are not equally common:

> **AUTHOR:** *"look &lt;target&gt;, assess &lt;target&gt; would be instant actions"*

| Use | Volume | Shape |
|---|---|---|
| **Movement** | the bulk of it | long runs of cardinal directions |
| **Observation** | frequent | `look <target>`, `assess <target>` -- free, no roundtime |
| Instant abilities (§1) | *"a few here and there"* | one or two, then a trigger |
| Combat openers | occasional | `target`, `stance offensive`, `attack` -- **an ordered batch of 3** |

**Observation widens the category, and the measurement already supports it.** §1 framed instant
actions as *abilities* -- sigils, Rapid Fire, Wall of Force -- but `look` and `assess` qualify on
the same grounds: they incur no roundtime and must not wait for one.

MEASURED, and it was in front of me the whole time: phase 4 sent a `look` **inside a 7-second
roundtime** with `Gate::None` and it **executed normally** -- full room render, no `...wait N`, no
refusal (§1.5). That was recorded as "roundtime does not gate every command"; the author's point is
that the ungated commands are a **named class**, not an exception list.

This matters for `Gate`. §1.4's `Gate::None` was written as the rare case, for an ability
*"established not to be roundtime-gated"*. On this reading it is **not rare at all** -- every
observation command takes it, and observation is frequent. `Gate::Roundtime` is for things that
*act*; `Gate::None` is for things that *look*. That is a much clearer rule than an
action-by-action table, and it is the one the type should encode.

**The combat opener is the sharpest test of the design**, and it is worth noticing why:

- It is **exactly 3** -- the author's premium entitlement, the number run C sustained. It fits, with
  nothing to spare.
- It is **strictly ordered**. A stance that lands after the attack is not a late stance, it is the
  wrong attack. §1.1's batching claim ("several then a trigger") is really an *ordering* claim, and
  this is the case where getting it wrong is silently wrong rather than visibly broken.
- On **standard it does not fit** (2 accepted) and on **free-to-play it does not exist** (1). So the
  same opener must degrade to two sends then one, or to three sequential sends, depending on the
  account.

That last row is the design consequence: **a behavior cannot hard-code "send these three
together".** It has to hand the send layer an ordered group and let the layer decide how much of it
goes at once. The entitlement is per-account and read from the wire (§5.2b-bis), so the split point
is not known until the game says so.

`Inbox::SendNow` riding the same channel as `Inbox::Command` is what makes the ordering safe --
two channels would give no guarantee between the stance and the attack. That was written for §1.1's
sigils; the combat opener is the case where it earns its keep.

#### The mapdb already encodes the distinction

VERIFIED in Lich: a `wayto` edge is **either a direction string or a `StringProc`**, and the
travel loop branches on exactly that
(`reference/lich-5/lib/dragonrealms/commons/common-travel.rb:206`):

```ruby
way = room.wayto[path.first.to_s]
if way.is_a?(StringProc)
  way.call          # a mini-script: send, read, decide
else
  move way          # a direction: send it
end
```

StringProcs are stored with a `;e ` prefix and reconstituted on load
(`lib/common/map/map_base.rb:377`), and the map layer is careful never to *evaluate* one while
pathfinding -- `:773` notes weights must be numeric and "never evaluate StringProc". So the
distinction is **data, already in the mapdb**, not something Cena has to infer.

#### The two modes map onto Cena's two send paths exactly

| Edge | Lich | Cena | Why |
|---|---|---|---|
| Cardinal direction | `move way` | **`send_now`** | Nothing to read. The next room arrives or it does not. |
| StringProc | `way.call` | **`send_and_await`** | *"it needs to kind of catch up and get responses for the mini script"* -- open a door, wait for the reply, decide. |

**This is the justification for having built both**, and it arrived after the fact rather than
before: `send_now` was built for instant actions (§1) and turns out to be the movement primitive
too. A run of cardinal directions is precisely the case with no reply worth waiting for, and
`send_and_await`'s one-window-at-a-time is precisely what a door needs.

#### The rate, in perspective

A cross-town run is the **worst case for command volume in normal play**, and even it is far under
the measured ceiling:

- A room transition has its own movement delay; the server will not accept directions faster than
  the character can walk.
- Run C sustained **21.5 commands/second** clean, and run B needed **54/s** to trip.
- No human and no travel route generates tens of moves per second.

**So the rate bound is not a constraint on travel.** What *is* a constraint is the **depth**: a
client that fires a whole path's worth of directions at once is batching past the entitlement
immediately -- 2 on standard, and **1 on free-to-play, where there is no batch at all** (§5.2b-bis).

#### What travel therefore needs from the send layer

1. **Batch cardinal runs up to the entitlement, never past it.** The entitlement is read from the
   wire's refusal, not assumed.
2. **Stop batching at a StringProc.** It is a synchronisation point by definition -- the mini-script
   needs its replies -- so the batch drains there naturally.
3. **Treat a refusal as "the batch was too big", not as a failure to retry blindly.** The command
   was refused, not executed; the path is intact and the move can simply be re-sent.

That is the same conclusion §5.2f reached from the rate data, arriving from the workload instead:
**bound the batch by entitlement, and let the natural synchronisation points do the pacing.** A
StringProc every few rooms means the queue drains regularly without any delay logic at all.

**UNVERIFIED:** whether a refused *movement* behaves like a refused `look` -- refused cleanly with
the path intact -- or whether the server's movement handling differs. Worth one deliberate test
before Travel is built, because a move that is silently dropped rather than refused would desync
the client's idea of which room it is in.

### 5.2e The console undercounted: per-group attribution is not reliable

The probe printed **5 refusals**; the wire has **7**. Both numbers are from the same run.

The cause is that `run_sequence` attributes each frame to whichever group was in flight when it
arrived, and at these speeds replies straddle group boundaries -- group 1's refusals landed while
group 2 was already sending. The file's own doc comment predicted this ("attribution is approximate
at the fast end") and the printout deliberately showed the total beside the split for that reason,
**but the total was computed from the same mis-attributed counts, so it inherited the error rather
than checking it.**

A total that is derived from the thing it is meant to validate is not a check. The `.bytes` log is
the record; the console is commentary, exactly as the probe's banner says.

**Practical rule, now demonstrated twice in one evening: read the counts off the wire.** Both
corrections in this section -- the entitlement being 2 rather than 1, and 7 refusals rather than 5
-- came from the log and neither was visible in the console output.

### 5.3 CLOSED — all four questions answered, three of them by accident

| # | Question | Answer |
|---|---|---|
| 1 | Do **instant actions** count against the buffer? | **YES.** Answered by the tests themselves — see below. |
| 2 | Does exceeding it **drop** or **refuse**? | **REFUSES**, individually, and the rest still run. 25 sent, 7 refused, 18 executed; 25 − 7 = 18 exactly (§5.2d). |
| 3 | Does a `send_now` during roundtime refuse, drop, or hold? | **RUNS.** A `look` inside a 7-second roundtime executed normally (§1.5). |
| 4 | Is the number ever other than `1`? | **YES — it is `2` here**, and it is an account entitlement, not a constant (§5.2a). No corpus search was needed. |

#### Question 1, and the shape of the mistake that kept it open

> **AUTHOR, 2026-09-18:** *"we were using the instant action look for testing, so I think that says
> yes instant actions count against the buffer (typeahead)."*

**Every command in every run of the probe was `look`.** And `look` is an instant action by the
author's own definition — *"anything that doesn't cause roundtime"* (§1.1a) — MEASURED directly,
since a `look` sent inside a 7-second roundtime executed normally.

So the answer was in hand from the first burst: `look` refused at 54 commands/second, accepted
exactly 3 at a time, and produced `Sorry, you may only type ahead 2 commands.` like anything else.

**The question stayed open because the definition was wrong, not because the evidence was
missing.** §1.2 treated "instant action" as a **curated list** — 515, Wall of Force, the Sunfist
sigils — and under a list, `look` is not a member, so three runs of it said nothing about the
class. Under the property, `look` is a member and the first run settled it.

That cost something real: the probe asked the author to spend mana proving what the free commands
had already proved. **A definition that is a list invites measuring what the property would have
told you** — the same failure as `plan/15`'s dead citation path, in a different register: a
category that resolves to the wrong set does not fail loudly, it manufactures an open question.

#### The consequence for `send_now`, now settled

§1.4 left open whether instant actions needed a rate policy at all. They do — they are ordinary
traffic as far as the buffer is concerned. But §5.2f already establishes what that policy is, and
it is small: **batch no more than the entitlement, read the entitlement from the refusal, and stop
batching when refused.** Nothing about instant actions is exempt, and nothing about them needs
special handling.

### 5.3a What is still genuinely open

| # | Question | Why it still needs the wire |
|---|---|---|
| 1 | Does a **roundtime-causing** action behave differently when it overflows the buffer? | Every command measured is free. An attack or a cast that is refused may interact with roundtime differently. Narrow, and not blocking. |
| 2 | Does a refused **movement** leave the path intact? | §5.2g. A dropped-not-refused move would leave `travel` believing it moved — the desync `plan/12` §5.4 exists for. **Should be measured before `travel` is built.** |
| 3 | Does a slow server pull the 21.5/s ceiling down? | Kelfour's *"only during slow downs"* says it can. Not blocking: a client that never batches past its entitlement is not exposed to it (§5.2f). |

**None of these blocks the current design.** Question 2 blocks `travel`, which does not exist yet.

---

## 5a. Cross-character commands, and what they must respect

**AUTHOR, 2026-09-18.** Raised while deciding whether scripting stays open. Recorded here
because it constrains `Origin` and the session registry, both of which are being touched now.

### 5a.1 The goal

> **AUTHOR:** *"eventually commands are going to be wanted. cross character commands. so I can be
> on nisugi the main and manually (or through script so treated as script) send commands to
> another character for them to perform as if they just sent it."*

Two things settled in that sentence:

- A script-originated command is **treated as script**, not as manual. That is the `Origin`
  variant.
- The recipient runs it **"as if they just sent it"** -- so on the recipient's queue it behaves
  like ordinary input, and `plan/12` §4.1's interleaving rule already covers it: jumps the
  queue, does **not** preempt the recipient's own behavior, runs its round trip.

That second point is the useful one. **The hard part is addressing, not arbitration** -- the
queue already knows what to do with an incoming command.

### 5a.2 Lich needs DRb; Cena needs a channel send

> **AUTHOR:** *"but we wouldn't need drb since it's multi session same application."*

Correct, and the saving is larger than it looks. Lich's borg/drone/queen trio works over
**distributed Ruby**, because each character is a separate OS process. That forces a network
listener per character, endpoint discovery, serialisation, and connection failure handling --
and the discovery happens *through the game*, which is why `druby://` URIs carrying the author's
real link-local IPv6 appear in **2,221 of 10,849 corpus files** (`cena-protocol/src/scrub.rs`).

Cena's five sessions are five tasks in one process. A cross-character command is a send on a
channel that already exists. No network, no discovery, no serialisation, and nothing leaking an
IP address into a log.

Note DRb is a **script-level convention, not a Lich feature**: `grep -rn 'DRb' reference/lich-5/`
returns nothing in core. The trio built cross-character control *on top of* Lich rather than
through it.

### 5a.3 THE CONSTRAINT: commands must respect game codes

> **AUTHOR:** *"that multi session commands have to respect game codes (instances)."*

**A GS3 character and a GSX character are in different worlds.** They cannot be in the same
room, cannot hand each other anything, cannot interact at all. A cross-character command between
them is not merely useless -- it would **silently appear to work**: the command sends, the
recipient runs it, and nothing the sender intended happens.

**Cena makes this easier to get wrong than Lich does.** In Lich each character is a separate
process behind a separate DRb endpoint, so crossing instances takes deliberate effort. In Cena
every session is in one registry, addressable by name, and nothing in the type system
distinguishes worlds.

So:

- A cross-character command with a **different instance** on either end is **REFUSED BY
  DEFAULT**, with a typed refusal. Not a silent no-op: the failure is otherwise invisible, which
  is the whole danger.
- The registry should be **grouped by instance** rather than flat, so the constraint is
  structural instead of a check that someone can forget to write.

**The block is a default, not a wall.**

> **AUTHOR, 2026-09-18:** *"I'm not saying it needs to be permanently blocked. It should be
> blocked by default, with a way, like a modifer, to override the game check."*

So the check is **opt-out per command**, not a compile-time impossibility. The reasoning holds
either way: the default protects against a failure that is invisible when it happens, and the
override means a legitimate case is never unreachable -- and, being explicit, appears in the log
as a deliberate act rather than as an accident.

This is the same shape as `plan/12` §5.2's `Unknown`: the type makes you say what you mean
rather than deciding for you. A caller that overrides has written down that it meant to.

**Use the REQUESTED game code, not the one `L` returns.** `Credentials::game_code` is what was
asked for (`GS3`); `LaunchPayload::gamecode` is the server's answer and MEASURED as a **family
code** -- a GS3 login returns `GAMECODE=GS` (`plan/15` §2a.4a). **UNVERIFIED** whether GSX also
answers `GS`; if it does, the returned code cannot distinguish instances at all and a guard
built on it would be silently useless.

### 5a.4 Open

- **Does the sender wait for a result?** Fire-and-forget matches "as if they just typed it" and
  keeps one session from stalling on another's problems; awaiting the recipient's `Outcome`
  needs a failure mode for a disconnected or wedged recipient. My recommendation is
  fire-and-forget plus confirm-by-effect -- the same pattern as §2 -- but this is the author's
  call and is **not settled**.
- What the queen/drone scripts actually send: commands, or higher-level intents. The trio is not
  in `reference/scripts` (238 scripts, none of the three), so this is unread rather than
  decided.

---

## 6. Logging

### 6.1 The gap

> **AUTHOR:** *"it's never too early to build logging in right at the start so we capture
> everything to look back on instead of relying on me to copy paste things from a terminal."*

`plan/12` contains **zero** mentions of logging, log files, or tracing. Measured by grep.

The author pasted terminal output twice in one session — a 2,347-byte login burst and a full run
transcript — because there was nowhere else for it to live. `Recorder` exists and captured 37
events on the live run, but it is in-memory and dies with the process.

### 6.2 Why it comes first, or nearly

Every other item in this document is easier to study with real captured sessions than without.
Typeahead in particular (§5) cannot be characterised at all without observing whether the limit is
ever hit.

### 6.3 Shape (sketch — least settled part of this document)

- `tracing` for structured events; per-session files
- `Recorder` flushed to disk, so criterion 7's replay works against real sessions rather than only
  synthetic ones
- Rotation, retention, and a size bound — the corpus is 49.55 GB and grew that way by accident

### 6.4 Scrubbing is not optional

The live run printed `A\tACCOUNT\tKEY\t<KEY-REDACTED>\tREAL NAME` — the key was redacted; **the
account name and the author's real name were not.**

Existing corpus rules (`CLAUDE.md`) apply and are stricter than anything currently implemented:

- `druby://` URIs carry real link-local IPv6 (20.5% of corpus files)
- character names in `noun=` / `exist=` / `<playerID>`
- **whispers and private channels: drop the lines, do not pseudonymise**

**OPEN for the author:** where logs live (beside the corpus at `E:\Gemstone\data\...`, in the repo
gitignored, or elsewhere), and whether account/real name should be scrubbed by default. My
recommendation is yes to both.

---

## 7. What this costs, and the order I would build it

| # | Item | Touches | Gets more expensive if delayed? |
|---|---|---|---|
| 1 | **Logging** (§6) | new; no existing code changes | No — but it makes 2–5 observable |
| 2 | **`send_now` + instant table** (§1) | `CommandQueue`, `SessionHandle` — the interface behaviors are written against | **Yes.** `look` is the only behavior today; every one written before this lands is written against the wrong contract |
| 3 | **Buffs in `GameState`** (§2) | `cena-model`; widens `plan/12` §7.1 | No, but 2's confirmation story is incomplete without it |
| 4 | **Clock skew** (§4) | small; new | No — but 2's roundtime gate is uncalibrated without it |
| 5 | **Typeahead** (§5) | send policy, beside 2 | No — and it cannot be characterised before 1 |

**My recommendation: 1, then 2, then 3 and 4 together, then 5.**

Logging first because it is unblocked and makes everything else observable. `send_now` second
because it is the only item whose cost climbs with delay.

**The author's call, not mine.** The one thing I would argue for is that 2 should not wait long: it
is an interface change, and interface changes get more expensive per behavior written.

---

## 8. Open questions, collected

**Five questions, four closed on 2026-09-18.**

| # | Question | Status |
|---|---|---|
| 1 | What does the server do with an instant action sent **during** roundtime? (§1.5) | **CLOSED — it runs.** A `look` inside a 7-second roundtime executed normally. |
| 2 | Do instant actions count against the type-ahead budget? (§5.3) | **CLOSED — yes.** Every probe command was `look`, an instant action by definition (§1.1a). |
| 3 | Does an instant action echo any text at all, or is it silent? | **CLOSED — it echoes.** `look` returns a full room render; a refused one returns the typeahead message. |
| 4 | Where do logs live, and is the account/real name scrubbed? (§6.4) | **CLOSED.** `CENA_LOG_DIR`, defaulting to `./logs`, set to `E:\Gemstone\data\cena_logs` in development. Credentials redacted; other players' names are not, and the log header says so. |
| 5 | Does widening `plan/12` §7.1 to include buffs/cooldowns happen now, or at M2? (§2.3) | **CLOSED — now.** Approved by the author and built (`plan/17`, `cena-model/src/effects.rs`). |

Questions 1-3 were all answered by runs aimed at something else, which is the section's own lesson:
the probe measured `look` because it was free and harmless, and `look` turned out to be a member of
the class three of these questions were about.

**Still open**, moved to §5.3a: whether a roundtime-causing action overflows differently, whether a
refused *movement* leaves the path intact (blocks `travel`), and whether a slow server lowers the
21.5/s ceiling.
