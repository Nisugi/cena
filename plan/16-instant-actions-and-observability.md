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

### 1.4 Proposed: a second send path

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

### 1.5 OPEN — what happens if one is sent during roundtime?

**UNVERIFIED.** Does the server refuse with `...wait N`, silently drop it, or hold it? This decides
whether the gate is a hard block or an optimistic try-and-retry. The author will know; I have not
asked yet.

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

### 5.3 OPEN

Whether instant actions count against the server's type-ahead budget at all. The author's phrasing
— *"shouldn't be subject to typeahead or waiting (depending on the action)"* — suggests it varies.
**UNVERIFIED.**

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
