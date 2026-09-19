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
| Free / standard | 1 | 2 |
| Premium | 2 | **3** (MEASURED, this account) |
| Premium + purchased | 3 | 4 |
| Lapsed premium who bought | 2 | 3 |

That last row is the awkward one: the benefit **persists after premium lapses**, so tier does not
determine the number. **An account's entitlement cannot be inferred from its subscription** -- it
has to be read from the wire, which is what `2 commands` in the refusal is for.

This is the third time tonight an entitlement has been mistaken for a protocol constant, and the
wiki's own example line -- *"You can only type ahead one line"* -- is the base-account wording that
Lich hard-codes and that would silently fail to match on any of the other three rows.

**Cena parses the number from the refusal and treats it as per-account state.** It is not a
constant, not a function of tier, and not knowable before the first refusal.

#### The timing half is NOT established

*"per 0.1s"* is the open question, not a finding. It is what the next run tests, and the evidence
so far brackets rather than settles it: run A was clean at **3.6 cmd/s**, run B failed at **~19
cmd/s** (§5.2d-bis). Three commands per 100ms is ~10/s, between the two.

**Depth and rate are independent**, and only depth is settled. A client that assumed "3 per 100ms"
today would be hard-coding an entitlement *and* an unmeasured rate at once.

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
apiece (the 50ms sleep plus ~11ms of send overhead), so all 30 commands were on the wire within
**1.57 seconds** -- about **19 commands per second** sustained. They arrived inside **two server
seconds** (`1789781016` alone carried **18** of the 29 prompts).

The previous run spread 25 commands over 7 seconds. So the difference between the two runs is not
the group size -- it is **how much work per second** the server was asked to do, and 19/s is past
what it will absorb.

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

**The next run tests the boundary rather than re-litigating the model** (AUTHOR: *"up the 0.05 to
0.1"*). 100ms is the one pause that appears in **both** runs: run A used it between groups of 4 and
was clean at 3.6 cmd/s; run B never tried it and failed at ~19 cmd/s. At 3 per 100ms the cadence is
near 10/s, between the two, which is where the boundary should be if there is one. A clean result
reconciles both runs instead of leaving them in apparent contradiction.

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

### 5.3 OPEN — what a live run would still settle

The limit is known; these are not, and none can be read out of Lich:

| # | Question | Why it needs the wire |
|---|---|---|
| 1 | Do **instant actions** count against the buffer? | The author's *"shouldn't be subject to typeahead or waiting (depending on the action)"* suggests some do not. Lich has no instant-action concept, so it cannot answer. |
| 2 | Does exceeding it **drop** the command or **refuse** it? | The newsletter says the third command "is not important" in one case and must be re-sent in another — so it is dropped, not queued. Worth confirming on the current server. |
| 3 | Does a `send_now` during roundtime refuse, drop, or hold? (§1.5) | The same run answers it: `Gate::None` sends during roundtime on purpose. |
| 4 | Does the corpus ever carry a number other than `1`? | Two years of real traffic would settle whether the literal is safe to match on. **Needs the author's permission** — recursive searches over the 49.55 GB archive are asked-first (`CLAUDE.md`). |

**Question 1 is the only one that changes the design**, and it is a cheap test: send several
instant actions back to back and see whether any produces the message.

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

| # | Question | Who can answer |
|---|---|---|
| 1 | What does the server do with an instant action sent **during** roundtime? (§1.5) | author |
| 2 | Do instant actions count against the type-ahead budget? (§5.3) | author, or measurement |
| 3 | Does an instant action echo any text at all, or is it silent? | author, or measurement |
| 4 | Where do logs live, and is the account/real name scrubbed? (§6.4) | author |
| 5 | Does widening `plan/12` §7.1 to include buffs/cooldowns happen now, or at M2? (§2.3) | author |
