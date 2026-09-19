# Cena — Implementation Specification (authoritative)

Written 2026-09-18. **This is the single source of truth for what gets built.**

Documents `01`–`11` are now **research and rationale**. Where any of them conflicts with this
document, this document wins. They remain valuable — they hold the evidence and the reasoning
— but they contain competing versions of the design, written before the curated-behaviors
decision, and they must not be read as instructions.

> **THE BARE NUMBERS BELOW ARE SPLIT ACROSS TWO DIRECTORIES.** Measured
> (`ls research/*.md plan/*.md`):
>
> - `research/` — `01`, `02`, `03`, `04`, `07`, `08`, `09`, `11`
> - `plan/` — `05`, `06`, `10`, and `12`–`19`
>
> So a number in this table is **not** reliably a sibling of this file. On 2026-09-19 a reader
> chasing C21 tried `plan/04-inherited-decisions.md`, got nothing, and had to search the tree;
> it is `research/04-inherited-decisions.md:1878`. **A citation that resolves to nothing does
> not fail loudly; it manufactures a false negative** — the lesson `CLAUDE.md` already records
> about `styleIfClosed`, where a dead path "proved" the wiki omits a tag it documents five
> times. Check the directory before reasoning from a document's silence.

| Document | Status |
|---|---|
| **12 (this)** | **Authoritative for implementation** |
| 01 architecture | Rationale. Layer model and seams still sound; §5, §6 and Phase 5–6 are void (Lua). |
| 03 lua-dialect | **Void as a decision.** Kept for the regex research (§3), which still governs. |
| 04 inherited-decisions | Rationale. Corrections C1–C21 folded in here; C14/C19/C20 void. |
| 05 engineering-rules | Binding for §−2 (evidence), §−1 (KISS/DRY), §1–§5, §7–§9. §6 is void (Lua). |
| 06 testing-strategy | Binding except §1.8 sandbox/conformance-to-Lua, which is restated here. |
| 07 agent-interface | Rationale. "Lua script" → "curated behavior"; safety model restated here. |
| 08 curated-behaviors | The decision. Its §"safety machinery is moot" is **corrected here** (§5). |
| 09 gap-audit | Open items; §3.2 reconnect is now specified here. |
| 10 eaccess-spec | Binding for Phase 2. |
| 11 player-requirements | Product evidence (pending). |

---

## 1. What is deleted, explicitly

The curated-behaviors decision removes these. They are not deferred — they do not exist.

| Deleted | Was specified in |
|---|---|
| Lua VM, mlua, Luau, dialect choice | `03` entire; `05` §6.1 |
| Coroutine scheduler | `01` §5; `05` §6.2 |
| Guest-language sandbox and sandbox tests | `06` §1.8, §2 |
| Per-VM memory budget, `set_interrupt` watchdog | `03` §4; `04` C14 |
| Capability grants *as a guest-code boundary* | `07` §4.3 (survives for agents, §6 here) |
| "Tier 0/1/2 API surface" as a build target | `01` §6 |
| `cena-script` crate | `05` §1 |
| Conformance-to-Lua-API tests | `06` §1.8 |
| Phase 5 "Script runtime", Phase 6 "API completion" | `01` §8 |

**What replaces them:** `cena-behavior`, a crate of async Rust tasks. A behavior is a state
machine over typed frames, driven by a profile, using only the seams in §3.

---

## 1a. Mobile's actual role — desktop-first development

Stated here because it was distorting priorities elsewhere in the plan.

**Mobile is a structural forcing function, not a feature priority.** Mobile OSes suspend
background processes, so Lich's proxy-process-plus-frontend-process architecture cannot work
there. **One process survives backgrounding; two cooperating ones do not.** That is why Cena
is one binary — a constraint from platform behavior, not from player demand.

It is also **over-determined**: the single binary is right regardless of mobile, because
Lich's one-process-per-character design is forced by process-global character state
(`04` Decision 5), which Cena does not inherit.

| | |
|---|---|
| **Binding** | one binary; no design that assumes a second cooperating process; `cena-platform`..`cena-behavior` compile for `aarch64-linux-android` + `aarch64-apple-ios` in CI |
| **Not binding** | mobile frontends, mobile UX, phone-class performance tuning, mobile testing |
| **Development** | **desktop-first, throughout.** Milestone 1 is desktop only. |
| **Status of mobile play** | a **bet**, not validated demand (`11` headline 3). The architecture it forces stands on its own. |

The CI mobile build is kept because it is cheap and prevents a dependency choice that would
foreclose the option later. It is a **compile check, not a product commitment.**

---

## 2. Crate layout (revised)

```
cena-platform    transport, EAccess login, recording, config  (deps: none)
cena-protocol    bytes <-> Frame; permissive parser           (cena-platform)
cena-model       typed game state, events, game data          (cena-protocol)
cena-session     session actor: state, queue, lifecycle       (cena-model, cena-protocol, cena-platform)
cena-behavior    curated behaviors as async tasks             (cena-session)
cena-agent       external agent protocol over a socket        (cena-session)
cena-ui          frontend-agnostic snapshot + input types     (cena-model)
cena-tui/gui/web frontends                                    (cena-ui, cena-session)
cena             binary                                       (everything)
```

Unchanged from `05` §1 except `cena-script` → `cena-behavior`. All rules in `05` §1–§2 apply,
reading "behavior" for "script".

> **AMENDED 2026-09-18 (author's call), Milestone 1 Step 2.** `cena-session` gained
> `cena-protocol` and `cena-platform`. The row previously read `(cena-model)`.
>
> **Second amendment, same day:** `GameState` moved from `cena-session` down into
> `cena-model`, where this table already said typed game state belongs. Until then
> `cena-model` used **nothing at all** from `cena-protocol` -- the `protocol -> model ->
> session` chain had a link carrying no traffic, which is why `cena-session` naming
> `cena-protocol` looked like a layer skip and was not one: there was nothing in
> `cena-model` to skip. Both crates now genuinely use what they declare (`cena-model`:
> `Frame`, `runs`; `cena-session`: `Frame`, `Parser`). The question surfaced twice in one
> slice -- once for the session, once for a behavior -- which is the signal that the rule
> was wrong rather than that the cases were exceptional.
>
> **The reason is ownership, not layering.** The session does not parse and does not reach
> up; it *owns* a `Parser` and calls it. `Parser` is stateful and explicitly one per session
> (`crates/cena-protocol/src/parser.rs:113-120`), so the value lives in the session struct,
> which means naming its type. Likewise the session owns the socket, which is why it names
> `cena-platform`'s `ByteSource`. Bytes in, frames returned to the caller — the parser never
> pushes anything onward, so nothing above `cena-protocol` sees raw text and Rule 2.1 holds.
>
> Routing either through `cena-model` was rejected: re-exporting `Parser` and the `Frame`
> variants is a pass-through facade with one caller, which `plan/05` §−1 forbids. A
> connection-manager crate was rejected for the same reason — reconnect is Milestone 2
> (§9c), so today it would be a trait with one implementor.
>
> **Third amendment, same day:** `cena` gained `cena-platform`, and **EAccess was assigned
> to `cena-platform`** — this table had given it no crate at all, while §7.1 puts "EAccess
> login (`10`, incl. the spike)" *In* for Milestone 1. It lives there because nothing in it
> knows what a `Frame` is: it speaks tab-delimited fields over TLS and stops the moment the
> game socket opens. `LiveSource::connect_tls` was already in that crate for its transport.
>
> The binary's new edge is the one `layering.rs` warns about — a direct edge from `cena` to
> a layer three below it — and it is taken knowingly. `authenticate` produces a
> `LiveSource`; `Session::new` consumes a `ByteSource`; something must hold both ends, and
> the binary is the only layer that can. `Session::connect(credentials)` was the
> alternative and keeps the row at three crates; it was declined because it moves EAccess
> **up** into `cena-session` to avoid an edge pointing **down**, trading a real layering
> violation for a cosmetic one. What this permits is narrow: `cena` may name
> `cena-platform`, and still may not name `cena-protocol` or `cena-model` — the arch test's
> set equality is what keeps that true.
>
> Enforced in `crates/cena-arch-tests/tests/layering.rs`, which carries the same reasoning.
> **This table and that table must change in the same commit.**

---

## 3. The seams (restated, minus the API wall)

Three seams, plus scoped state. In Rust these are types and borrows, not an API surface:

| Seam | Form | Rule |
|---|---|---|
| **read** | `&GameState` / `StateSnapshot` | read-only by borrow; no parsing; no sends |
| **observe** | `broadcast::Receiver<Event>` | cannot mutate; cannot suppress |
| **act** | `CommandHandle` | the only path that sends |
| **scoped state** | owned by the behavior | fight/room-scoped latches (C11) |

`05` §2.3's correction stands: adopt the *discipline*, not eohunter's 156-method delegation
wall — in Rust the read-only guarantee is `&` and injection is a trait.

---

## 3a. One parser, N classifiers — SETTLED 2026-09-19

**Exactly one thing turns bytes into structure.** Everything downstream that
recognises game facts is a *stateless classifier* over what that parser
produced, and anything needing memory across lines is a *stateful consumer*
above the model.

| | Remembers | Sees | Lives in |
|---|---|---|---|
| `Parser` | stream position, open links, partial line | bytes | `cena-protocol` |
| classifier | nothing | one parsed line / frame | `cena-model` |
| consumer | game situation | frames + classifier answers | `cena-session` and above |

The pattern is already three deep: `crit` (2,394 crit-table regexes),
`gameobj` (113 item-type patterns), and whatever combat becomes. None of them
is a parser — none tokenizes, none holds state, none may see markup.

### Parsing is not matching

The distinguishing test is **does it need to remember anything.** `Parser` is
per-session and stateful; feed it half a tag and it holds on. `crit::parse` is
behind an `Arc` and total: same line in, same answer out, forever.

That is why `Parser::new()` is one-per-session and `CritTables` is shared.

### What the parser owes the classifiers

**Every fact the markup encodes must survive into the frames**, because a
classifier may not re-read XML to recover one. That is the whole content of
this decision — "do not re-parse markup" is only honest if nothing is lost.

VERIFIED against `GSIV-Nisugi/2026/01/xml/2026-01-01_22-51-49.xml`, the
hardest line in a real attack sequence:

```text
Nearly insensible, <pushBold/>the <a exist="416327162" noun="shield-maiden">gigas
shield-maiden</a><popBold/> desperately blocks the attack with <pushBold/><a
exist="416327162" noun="shield-maiden">her</a><popBold/> <a exist="416327163"
noun="spear">spear</a>!
```

through Cena's parser:

```text
TEXT "Nearly insensible, "       bold=0  | -
TEXT "the "                      bold=1  | -
TEXT "gigas shield-maiden"       bold=1  | exist=416327162 noun=shield-maiden
TEXT " desperately blocks ... "  bold=0  | -
TEXT "her"                       bold=1  | exist=416327162 noun=shield-maiden
TEXT " "                         bold=0  | -
TEXT "spear"                     bold=0  | exist=416327163 noun=spear
TEXT "!"                         bold=0  | -
```

Everything Lich's `Combat::Parser` recovers by re-scanning XML is already
typed: `exist` and `noun` (in `LinkKind::Exist`), bold depth, and link order.
The pronoun `her` carries the creature's own `exist`, so it resolves without a
heuristic — Lich needed a fix for exactly this after a 2026-09-07 hunt log
recorded an attacker as "his".

This is also the triple Lich builds `GameObj` from: `GameObj.new(id, noun,
name)`, deduplicated by `"id|noun|name"`, sourced from `@obj_exist` /
`@obj_noun` off the same `<a>` tag (`lib/common/xmlparser.rb`).

### What the parser does NOT do: turn tags into world state

`<crtrStatus exist= ...>` becomes `Frame::CreatureStatus { id, attrs }` with
**`attrs` raw**. The parser lifts the identity because that is structure; it
does not map flag names, because it does not know what a flag means.

Lich's own handler carries a 25-line comment explaining why this separation
matters: applying flags at the tag *worked*, and the creature then silently
vanished from the room roster after the next `clear_room`, because
registration happened on another path — *"it would sync but never reappear in
`Creature.targets/.in_room`"*. Their fix was to defer.

Cena cannot make that mistake structurally: `cena-protocol` has no idea what a
room roster is.

### Multi-line events

Some facts span lines — an attack, its damage, and its crit message are three
wire lines and one event. That does **not** call for a second parser. It calls
for a stateful consumer reading `Frame`s, which remembers "an attack is in
progress" and calls the stateless classifiers per line.

### When to reopen

If something needs to **re-tokenize** — read raw markup that the frames did
not preserve. That is the signal the parser is losing a fact, and the fix is
to widen the frame, not to add a parser. `Frame::AppInfo` gaining `game` and
`title`, and `Runs` bodies reporting their unmodelled tags (review PR-1), are
both that repair done in the right place.

---

## 4. Command ownership and arbitration

**The gap:** a queue prevents overlapping round-trips. It does not decide who wins when Hunt
wants to attack and Heal wants to retreat. This section specifies that.

### 4.1 One owner at a time — and manual input is NOT a claimant

The session holds a **command authority**, a single token. Only its holder may run a
*sequence*. Everything else observes.

| Claimant | Default priority |
|---|---|
| Agent (when granted `act`) | 1 |
| Behavior (Hunt, Heal, Travel…) | 2 |

**Manual input is not in this table, and that is the correction.**

> **CORRECTED 2026-09-18.** An earlier draft made manual input priority 1 and said it
> *preempts* the authority holder — which, combined with §4.3, meant **typing `say hi`
> mid-hunt would abort Hunt.** That is wrong, and neither reference implementation does it.
> **Verified:** eohunter has *no upstream hook at all* (`grep -rn 'toggle_upstream|upstream_get|UpstreamHook' scripts/eohunter/` returns nothing); its controller reacts only to an
> **explicit** return request (`controller.rb:802`, `request_return("manual_#{action}")`).
> Lich likewise interleaves manual commands with a running engine rather than killing it.

**The rule is: manual input is never *queued behind* automation — not that it *revokes*
automation.**

A manual command is injected into the command queue at the head, runs its round-trip, and
the authority holder continues. The holder observes the manual command as an event (so a
behavior can notice the player moved the character and re-orient), but is not cancelled by it.

**Only an explicit `stop` or `pause` preempts.** Those are distinct user actions, not a side
effect of typing. The player is never locked out of their character — but they are also never
surprised by their hunt dying because they answered a whisper.

### 4.2 Behaviors do not run concurrently by default

Hunt/Heal/Travel are **not** three actors racing for the queue. One **supervisor behavior**
holds the authority and composes the others as sub-behaviors, deciding priority in its own
logic. This is how eohunter already works: its `controller.rb` owns the loop and the
behaviors are policies it consults.

> **Hunt vs Heal is therefore not a queue problem — it is a policy decision inside the
> supervisor.** Attempting to arbitrate it at the transport layer would push game policy into
> infrastructure, which is the wrong place for it.

A behavior that wants the authority while another holds it gets `Err(AuthorityHeld)`. It does
not queue behind it — silent queueing is how you get an attack that fires four seconds after
the fight ended.

> **IMPLEMENTED in Milestone 1 Step 2 (2026-09-18).** `Origin::Behavior` carries the
> claimant's `AuthorityToken`, `SessionHandle` exposes `claim`/`release`, and
> `SessionActor::admit` refuses a command whose token does not hold the authority --
> `Refusal::Permanent`, because retrying changes nothing while another behavior holds it.
>
> Claims travel the **same channel** as commands (`command::Inbox`), not a second one: §4.2
> is an ordering rule, and two channels in a `select!` give no ordering guarantee between
> them, so a claim could overtake a command already queued behind it.
>
> **This was a wish before it was a rule.** `CommandQueue` had `claim`/`release`/`authority`
> and unit tests for them from the start, and nothing in the session ever called them, so two
> behaviors would both have had their commands sent in FIFO order. The test that looked like
> coverage drove `CommandQueue` directly -- it proved the struct had a field, not that the
> session consulted it (`plan/05` §0). `a_behavior_without_the_authority_is_refused_not_queued`
> drives a real `Session` and asserts the WIRE, because the point is not that the command was
> refused but that it was never sent.
>
> Still open, and deliberately: the supervisor composition this section describes -- one
> behavior holding the authority and composing others as sub-behaviors. Step 2 has one
> behavior, so there is nothing to compose and the rule of three has not been met.

### 4.3 Preemption is explicit and cooperative-with-a-deadline

Preemption:
1. Signals the current holder's `CancellationToken`.
2. Waits up to **`PREEMPT_GRACE` (default 250ms)** for it to yield.
3. On timeout, revokes the authority anyway and marks the holder `Aborted`.

A preempted holder gets a typed reason and may run cleanup, but **cleanup cannot send
commands** — it releases resources only. Otherwise "stop" becomes "send more".

### 4.4 Response attribution, cancellation, and late responses

> **CORRECTED 2026-09-18.** An earlier draft said a response is delivered when it "matches
> `in_flight` by `CommandId`". **The game carries no command ids.** That table was
> unimplementable. Attribution is *temporal*, and this section now says how.

**How Lich does it (the mechanism to port):** `fput` calls `clear` — which drains the script's
downstream buffer — then sends, then reads. Everything arriving after the send and before the
terminator is attributed to that command. There is no identity, only ordering.

**Cena's rule:**

A round-trip owns the frame stream from the moment its bytes are written until its
**terminator**, which is the next `Frame::Prompt`. Within that window:

- frames are offered to the waiter's matcher,
- and are *also* published to observers (§3 — observation never competes with attribution).

| Situation | Behavior |
|---|---|
| Frame arrives inside the window | offered to the waiter; also published |
| `Prompt` arrives | window closes; waiter resolves with what it matched, or `Timeout` |
| Frame arrives with **no open window** | unsolicited — published to observers only |
| Window closed, then a late line arrives | **cannot be attributed.** Published as unsolicited, tagged `late_after(CommandId)` in the log |
| Generation mismatch (§5.2) | discarded — belongs to a previous connection |

**`CommandId` still exists**, but for *correlation in logs and for the audit trail*, not for
wire matching. It lets a reader reconstruct which command a late line probably belonged to.

**The unavoidable caveat, inherited from the game, not from our design:** a slow or queued
line can arrive after its prompt. eohunter hit this and handles it in-band. Cena cannot
eliminate it — **no client can** — so the contract is:

- A behavior must tolerate a matching line arriving one window late.
- The ladder's own re-read (`fput` unshifts the consumed line, `01` §6.1) is the precedent:
  **do not discard an unmatched line, republish it.**
- `Outcome::Timeout` therefore means *"no match within the window"*, never *"the command did
  not happen"*.

**This directly underwrites Milestone 1 pass criteria 3 and 9**, which is why it is specified
before any of it is built.

**This is the concrete answer to "a cancelled command's response arrives during the next
command."** Without the id, the next command would consume the previous one's answer — a bug
class that is very hard to diagnose because it only appears under timing pressure.

**Cancellation does not un-send.** A cancelled command may already have reached the game.
Cancellation means *"stop waiting and do not act on the result"*, never *"it did not happen"*.
Behaviors must be written for that reality; the API names it (`CancelledMayHaveApplied`).

### 4.5 Arm before send, structurally

`send_and_await` is **one call**. There is no public send-then-wait pair, so the race cannot
be written (`05` §6.5). Outcome is always a typed enum:

```rust
enum Outcome {
    Confirmed(Event),
    Timeout,
    Interrupted,       // preempted or cancelled
    Dead,
    Refused(Refusal),  // Transient | Permanent | Roundtime | Stunned | Webbed
}
```

---

## 5. Session lifecycle: reconnect, desync, containment

### 5.1 The contract

A session is always in exactly one state:

```
Connecting -> Authenticating -> Syncing -> Ready -> { Degraded | Reconnecting } -> Closed
```

- **Ready** — state is trustworthy; behaviors may run.
- **Degraded** — connected, but some facts are stale/unknown (e.g. post-desync). Behaviors
  that require an unknown fact fail rather than guess.
- **Reconnecting** — no transport. All in-flight commands fail immediately with
  `Disconnected`. **No automation runs.**

### 5.2 Generation, and what becomes unknown

Every connection increments a **`Generation`**. `CommandId`, events and snapshots all carry
it. Anything from a prior generation is discarded (§4.4).

**On disconnect, facts are classified — this is the core of the contract:**

| Class | Examples | On reconnect |
|---|---|---|
| **Invalidated** | roundtime, stance, current room, hands, targets, group, disk | → **Unknown** until re-observed |
| **Retained** | character name, profession, race, level, skills, PSM ranks | survive; they change on game-time scales |
| **Suspect** | inventory, wounds, active spells, experience | retained but flagged stale; re-synced during `Syncing` |

**`Unknown` is a first-class value, not a default.** `world.roundtime` after reconnect is
`Unknown`, never `0`. A behavior asking for an unknown fact gets an error, not a guess. This
is `05` §7.3 (honest capabilities) applied to time.

> **Reconnecting must not silently make old state trustworthy.** That is the entire point of
> this section, and it is why `Ready` is a distinct state from `Connected`.

### 5.3 Readiness gate

`Syncing` runs the login-state queries (the Infomon-equivalent sync, ~15 commands / 30–60s
per `01` Phase 3). **Behaviors may not start until `Ready`.** An agent connection may observe
during `Syncing` but not `act`.

### 5.4 Desync

Per `01` §0.1a and C5/trap 13 — typed state accumulates silently where text does not.

On a truncation-class parse error: **log, transition to `Degraded`, invalidate the affected
state group to `Unknown`, re-sync it, return to `Ready`. Never kill the session.**

Each state group has an owner and a re-sync command, so invalidation is targeted rather than
global.

### 5.5 Containment — correcting `08`

> **`08` overstated this.** It said curated behaviors make the safety machinery moot. That is
> right about *guest-language sandboxing* — no VM, no interpreter escape, no per-VM memory
> budget. **It is wrong that containment is unnecessary.** Reviewed Rust code can still loop
> forever, accumulate unbounded work, ignore cancellation, or panic.

Required regardless of who wrote the behavior:

| Requirement | Mechanism |
|---|---|
| Every behavior is cancellable | `CancellationToken` checked at every await; §4.3 deadline |
| No unbounded work | bounded channels everywhere; bounded retries (the ladder's cap); bounded scoped state |
| Every wait has a deadline | no unbounded `await`; every wait takes a timeout |
| A panic kills one session, not the process | each session is a supervised task; `catch_unwind` at the boundary. **Requires `panic = "unwind"`** — `panic = "abort"` defeats it, and that must be asserted in CI |
| A wedged behavior is detectable | per-session watchdog: no progress within `BEHAVIOR_WATCHDOG` → log, cancel, `Degraded` |
| One session cannot starve another | one task per session; bounded queues; no shared lock on a hot path |

**Session failure isolation is the invariant:** any single session can fail, wedge or panic
without affecting the other 24.

---

## 6. Delivery guarantees

**The gap:** what happens when a UI or agent consumes events too slowly.

### 6.1 Two channels, different guarantees

| Channel | Guarantee | On slow consumer |
|---|---|---|
| **State** (snapshot + deltas) | **latest-wins** | coalesce — only the newest value matters |
| **Events** (typed occurrences) | **lossless within a bounded window** | see §6.3 |

Vitals going 100→95→90 may coalesce to 90. "You were stunned" may not be dropped silently.

### 6.2 Snapshot joins the stream without a gap

The join is a single operation, not two:

```rust
let (snapshot, mut events) = session.subscribe();  // atomic
```

`subscribe()` takes the state lock, clones the snapshot, and registers the receiver **before
releasing**. Every event after the snapshot's cursor is delivered; none between is lost.
Snapshot carries `(generation, cursor)`, and events carry increasing cursors, so a subscriber
can always tell where it is.

### 6.3 Lag is reported, never hidden

Bounded ring buffer per subscriber (`tokio::sync::broadcast` semantics). On overflow the
subscriber receives an explicit **`Lagged { missed: u64 }`**, never a silent gap.

Recovery is defined: **on `Lagged`, discard local state and re-`subscribe()`.** Cheap, because
snapshot+stream is atomic (§6.2). An agent additionally gets `Lagged` as a protocol message so
a model knows its picture was incomplete rather than reasoning from a hole.

**Rule:** a subscriber that cannot keep up gets *correct-but-incomplete with a marker*, never
*plausible-but-wrong*.

---

## 6a. Three product constraints, fixed now

From [`11-player-requirements.md`](../research/11-player-requirements.md) (4,979 messages, 143 distinct
posters, ten weeks). None changes Milestone 1's scope. All three are **cheap now and expensive
later**, which is why they are recorded as constraints rather than left to the milestone that
implements them.

### 6a.1 The profile format is opinionated, not maximal

The plan's stated fallback for "no user scripting" was *"profiles must be genuinely
expressive"* (`08`). **That was argued against directly, in the thread, by an experienced
player — and Saga's own lead developer agreed about his own product:**

> **[8/26/2026 4:12 PM] soleblaze:** "One thing I see a lot with agentic coding is **config
> creep**. It loves to make as much as it can configurable and doesn't understand how confusing
> it can make interfaces. I'd agree that it'd be better to have some kind of extension system vs
> trying to cater to all the various niches. **It'd windup a big ball of mud otherwise.**"

> **[8/26/2026 4:18 PM] mirke** (Saga's lead dev): "it's a **fool's errand to try to code to
> everyone's needs.** I am definitely spread a little thin trying to hit all of the
> customizations everyone wants."

**Ten distinct people reported being overwhelmed by Saga's settings, including two staff.**
"No scripting, therefore more configuration" walks into exactly that failure.

**Binding:**
- Profiles express **policy**, not programs. No conditionals, no loops, no variables, no
  `goto`. If a profile needs those, the behavior is wrong, not the format.
- **Ship curated presets as the primary interface.** Most users start from a preset and adjust
  a few fields; the full field set is the advanced path, not the front door.
- Presets are **shareable artifacts** — one file, exportable and importable. This is the
  thread's own answer to config overwhelm.
- **Evidence for the ban:** zero of 4,979 messages ask for conditionals, loops or state in
  user-authored automation; ~2 people in ten weeks ask for a reactive trigger. The demand we
  were designing against does not exist.

### 6a.2 Settings inherit: global → profile → character

**The second message in the entire thread**, still unresolved ten weeks later, raised by 7–10
distinct people across the whole period. Veterans carry 25–30 years of highlights and will not
rebuild them per character.

**Binding:** every user-facing setting — highlights, macros/keybinds, layouts, behavior
profiles — resolves through a **three-level chain with per-level override and explicit,
documented precedence.** This lives in the data model from the first setting that exists; it is
not a later feature.

A per-character export/import blob is **not** a substitute (the thread says so explicitly) —
it is the workaround people resent.

### 6a.3 One command-action model, bound from many surfaces

A keybind, a hotbar button, a mouse button, a chain step and a behavior invocation are **five
bindings of the same named action.** Saga built them as separate subsystems, which is why
players had to ask whether macros could trigger hotbar buttons.

**Binding:**
- One `Action` registry. Every invocation surface resolves to an `Action`, and goes through the
  command authority (§4) like anything else.
- Keybinds: **any chord** (Alt/Ctrl/Shift × numpad/F-keys/letters). **No chord is reserved by
  the application** — 10 distinct people asked for this; one could not hunt until it was fixed.
- Binding scopes layer per §6a.2. Cena never silently reinstates a binding the user removed.

> **Why fixed now:** §6a.2 and §6a.3 are data-model shapes. Retrofitting an inheritance chain
> or unifying four parallel binding subsystems after they exist is the kind of refactor
> `04` C16 describes — one you do by hand, expensively, and then ratchet.

### 6a.4 Also recorded, not yet binding

- **Accessibility is low-vision-first, not TTS.** Zero hits for NVDA/JAWS/VoiceOver/braille in
  25,255 lines; 14 people on per-panel font sizing, 13 on disabling animation/flash. Driven by
  **age, not disability**. Revises `09` §1.2, which hung the requirement on Vellum's TTS module.
- **Highlights are the primary combat perception channel**, not decoration —
  *"If you know what is happening during combat by reading, you're going too slow. Gotta
  interpret by color."* Design them with the matcher (`inventory/09`), not as a settings panel.
- **Typed events enable something Saga cannot do:** highlight by *actor* ("generated by me") and
  *event class* (death/stun/prone) rather than 70 hand-written regexes. This is a genuine
  differentiator that falls out of the frame design.

---

## 7. Milestone 1 — the end-to-end slice

**One GemStone character logs in, displays a room, accepts a manual command through the
shared queue, runs one small behavior, stops reliably, and disconnects cleanly — with a
replay test covering the same flow.**

This is the next thing built. Not the character model, not Hunt, not multiple frontends.

### 7.1 Scope

| In | Out |
|---|---|
| EAccess login (`10`, incl. the spike) | saved credentials, GUI login, web-login fallback |
| Permissive parser: enough frames for a room + prompt + vitals | full frame vocabulary |
| `GameState`: room, hands, roundtime, vitals, **status indicators, server clock** | stats, skills, PSMs, spell *data*, inventory |
| Session actor: lifecycle §5, queue §4 | multi-session (one session, but no global state) |
| One behavior (e.g. "walk to room N" or "wait for roundtime then send") | Hunt/Heal/Travel |
| One frontend: headless + minimal web or plain stdout | TUI, GUI, full web |
| Recorder + replay test of the whole flow | golden corpus breadth |

> **AMENDED 2026-09-18 (author's call), after Milestone 1 closed.** `GameState` gained **status
> indicators** and the **server clock**. The row previously read "room, hands, roundtime,
> vitals".
>
> **The reason is that the model had fallen behind the protocol.** `StatusIndicator` and the
> prompt's `time` were both already parsed and both discarded, and everything in `plan/16` —
> instant actions, roundtime gating, confirm-by-effect — waited on them. With one behavior
> written, the cost of widening was near zero; each behavior written first would have been
> written against a model that cannot answer "am I in roundtime".
>
> **It is a port, not a design** (`plan/17`): `StatusInfo` and `game_time_now()` come from
> `reference/VellumFE/src/core/state.rs`, and Lich's `status.rb` supplies the rule about
> text-derived conditions. The semantics were then MEASURED against Cena's own capture
> (`plan/15` §2a.4a) rather than assumed.
>
> **Still Out:** stats, skills, PSMs, inventory, and spell *data*. Effects (the active-buff
> list) are proposed in `plan/17` §4 and are **not** built — this amendment covers indicators
> and the clock only.

### 7.2 Pass criteria

1. Logs in against the live server (proves `10` and the SNI/pinning unknowns).
2. Renders a room description and prompt from **typed frames**, never raw text.
3. A manually typed command goes through the same queue as the behavior's, returning a typed
   `Outcome`.
4. The behavior runs, and **`stop` stops it within `PREEMPT_GRACE`**, verified.
5. Manual input is **interleaved, not preemptive** (§4.1): a command typed mid-behavior jumps
   the queue, runs its round-trip, and the behavior **continues**. The behavior observes it as
   an event. Verified by a test that types a command mid-sequence and asserts the behavior is
   still running afterward.

   > **CORRECTED 2026-09-18.** This criterion previously read *"Manual input **preempts** the
   > behavior mid-sequence."* That was left over from the superseded §4.1 draft, and building
   > to it would reintroduce exactly the bug that draft was corrected for — typing `say hi`
   > mid-hunt aborting Hunt. Only explicit `stop`/`pause` preempts (criterion 4).
6. Disconnect is clean: task ends, no leaked sockets, no panic.
7. The whole session is **recorded and replays deterministically** in a test, with no network.
8. Unknown tags survive to display (`Frame::UnknownTag`) rather than panicking.
9. A killed connection mid-command yields `Disconnected`, and reconnect leaves invalidated
   facts `Unknown` (§5.2) — verified in the replay.

### 7.3 What it proves

Every boundary that matters: the parser under real input, the frame vocabulary, the session
lifecycle, command ownership and preemption, cancellation with late-response discard, the
observe/present split, replay determinism. **It exercises §4, §5 and §6 — the three things
this document adds — before any of them is expensive to change.**

### 7.4 What it deliberately does not prove

Multi-session isolation, behavior composition, the agent protocol, DragonRealms, performance
at scale. Those follow, informed by running code.

---

## 8. After Milestone 1

Revised from `01` §8, each phase ending in something demonstrable:

| M | Deliverable |
|---|---|
| **1** | the slice above |
| 2 | frame vocabulary breadth + golden corpus; full room/combat/vitals rendering |
| 3 | character model (typed, per `research/01` Phase 3 / `research/04` C21) |
| 4 | a real frontend (web first — it is also mobile) |
| 5 | multi-session: N characters, isolation tests, switching |
| 6 | first real behavior (Hunt), profile format (§6a.1), supervisor composition |
| 7 | agent protocol (`07`, safety per §5.5 + §6.3) |
| 8 | remaining behaviors; customization surface (per `11` — highlights first, §6a.4) |
| 9 | DragonRealms adapter |
| 10 | TUI/GUI |

**Milestone 5 before 6 is deliberate:** multi-session is the headline feature and it
constrains behavior design. Discovering it late would mean rewriting behaviors.

---

## 9. Open items this document does not settle

Marked per `05` §−2 (evidence rules):

- **UNVERIFIED** — the EAccess wire unknowns in `10` §12 (K-response terminator, SNI routing,
  response framing). *Settled by:* the Milestone 1 spike, which is why it is first.
- **UNVERIFIED** — `PREEMPT_GRACE`, `BEHAVIOR_WATCHDOG` and buffer sizes are placeholders.
  *Settled by:* measurement during Milestone 1.
- **OPEN** — the profile format. Deferred to Milestone 6, when a real behavior exists to
  configure (`05` §−1 rule 0.1: do not design it before the concrete case).
- **OPEN** — whether the supervisor composition model (§4.2) survives a second behavior.
  *Settled by:* Milestone 6, the rule-of-three (`05` §−1 rule 0.2).
- **RESOLVED** — `11-player-requirements.md` is complete (1,738 lines, 4,979 messages, 143
  posters, six lenses each adversarially verified). Its constraints are folded into §6a. It
  **supports** the curated-behaviors decision more strongly than `08`'s own reasoning did
  (zero players asked for a scripting language), and **corrects** the accessibility framing in
  `09` §1.2.

---

## 9a. Review findings, 2026-09-18 — status

A structured review raised nine findings. Resolved and open:

| # | Finding | Status |
|---|---|---|
| 1 | Greenfield vs evolving Vellum never argued | **RESOLVED** → [`13-greenfield-vs-evolution.md`](13-greenfield-vs-evolution.md). The reasoning existed but was unwritten: **Vellum was a learning process**, and Cena applies what it taught — the same pattern as eohunter from bigshot/forge. **Greenfield stands**, with an obligation to port aggressively where knowledge lives in code (parser, `ParsedElement`, fixtures, game data). |
| 2 | Manual input cancels automation | **FIXED** §4.1. Verified against eohunter: no upstream hook, explicit return requests only (`controller.rb:802`). |
| 3 | Response attribution unspecified | **FIXED** §4.4. Attribution is temporal (send → next `Prompt`), not id-based; the game carries no ids. |
| 4 | Documents contradict; loaded as instructions | **OPEN — do before coding.** Move `01`–`11` to `research/`, leave `12`+`13` in `plan/`, add `CLAUDE.md` pointing at `12`. Nothing is committed yet. |
| 5 | Five-second automation has no home | **OPEN** — see §9b. |
| 6 | Milestone 1 too wide | **ACCEPTED** — see §9c. |
| 7 | Parser port vs golden-file rule | **RESOLVED by finding 1.** Evolution keeps `ParsedElement` and its goldens valid; `Frame` wraps rather than replaces. Add `04` §6.6's four silently-dropped tag families to M2's corpus work. |
| 8 | DragonRealms scope undefined | **RESOLVED** → §9d. **All-or-nothing: if DR is supported, it is full support.** Deferred, not scoped in. |
| 9 | No effort sizing; `catch_unwind` redundant | **ACCEPTED.** Tokio's `JoinHandle` already isolates task panics — §5.5's `catch_unwind` is belt-and-braces, not the mechanism. Sizing deferred until after §9c step 1. |

### 9b. The five-second automation — chains and foreach

`11` §P.1(c) is the sharpest challenge in the research corpus: **Cena absorbs Lich, so a
one-liner like "group everyone in the room" has nowhere to go.** `12` previously answered with
the LLM agent, which is the wrong tool for a five-second task.

**Resolution:** `chain` and `foreach` are **built-in parameterized behaviors**, not profile
syntax. That keeps §6a.1's ban intact — profiles still express policy, not programs — while
giving the mass-market ask (`11`: Saga's chain/foreach primitive) a real home.

- A **chain** is an ordered list of actions with the queue's normal semantics.
- A **foreach** binds a chain over a set the client already knows (creatures in the room,
  group members, items in a container).
- Both are `Action`s (§6a.3), so they bind to keys, hotbar buttons and mouse buttons.

**Scheduled: Milestone 6 or earlier.** It is small, and it removes the largest product
objection to curated-only.

**Also open:** the extension/panel boundary (`11` Q.19) — what a user may add without code.
Named here so it is not lost.

### 9d. DragonRealms — all or nothing, and deferred

**Decision (author, 2026-09-18): if Cena supports DragonRealms, it is full support.** No
client-only half-measure — a DR player who cannot automate is not served by a client whose
entire point is automation.

**Therefore DR is deferred, and is not currently scoped in.** That is the honest consequence
of "all or nothing" plus `05` §−1 rule 0.1 (build the simplest thing that works): a commitment
this size needs a reason, and no DR user has been identified.

**What full support would actually cost**, so the decision can be made on evidence later:

| Component | Cost |
|---|---|
| Parser dialect | DR scrapes far more from plain **text** where GS has XML (`inventory/03`). Not a variant of the GS parser — a second extraction strategy. |
| `GameAdapter` | currently a **sketch** (`09` §3.3), never validated against a second game. DR is what would validate it. |
| Game model | DRInfomon-equivalent: different stats, skills, balance/roundtime model |
| Behaviors | the DRC/DRCI/DRCM/DRCT layer, ~13K lines of DR game knowledge in Lich, rebuilt in Rust |
| Corpus | the 50 GB log archive is **GemStone**. DR would need its own. |
| Test burden | GS/DR parity tests (`06` §1.7) across every shared behavior |

**Consequences for what gets built now:**

- **Milestone 9 (DR adapter) is removed from the roadmap** and becomes a conditional project.
- **Keep the seam, do not build the abstraction.** `05` §−1 rule 0.2 (rule of three) applies:
  the `GameAdapter` trait stays *unwritten* until a second game exists. Writing it now, from
  one example, would specify the wrong axis — precisely the failure mode `04` trap 10 records
  for Vellum's `Frontend` trait.
- **What that costs in practice is small**: do not hard-code GemStone assumptions into shared
  types (`Frame`, `GameState` field names, the refusal taxonomy) where a neutral name is as
  easy. That is cheap hygiene, not an abstraction layer.

**Revisit when:** a DR user is identified (including the author wanting to play DR), or the
GemStone client is complete enough that the marginal cost is clear rather than estimated.

### 9c. Milestone 1, narrowed

The review is right that nine criteria across four layers plus reconnect plus a replay harness
is not a first slice. Split:

**Step 0 — capture a Lich login with tcpdump.** `10` §11.4 already says this is first: one
evening, **zero Rust**, and it answers three of the four wire unknowns (K-terminator, framing,
SNI routing). Done when a capture exists and `10` §12's items are marked VERIFIED or amended.

**Step 1 — the login spike** as `10` §11 defines it.

**Step 2 — the slice**, with **reconnect and criterion 9 moved to Milestone 2.** M1 keeps only
*"disconnect is clean."* Criteria 1–8 stand.

---

## 10. Planning is closed

**Status as of 2026-09-18: planning is complete. The next artifact is running code
(Milestone 1, §7).**

What exists: ten inventory documents and twelve plan documents, built from four reference
codebases (~400K lines), 1,644 PRs of project history, and 4,979 messages of player evidence —
with adversarial verification at every stage, which caught material errors in every single
batch including several of mine.

**No further research is scheduled.** The remaining unknowns in §9 are of a kind that
*research cannot settle* — they need a running program:

| Unknown | Settled by |
|---|---|
| EAccess wire details (K-terminator, SNI routing, framing) | the Milestone 1 login spike |
| `PREEMPT_GRACE`, watchdog intervals, buffer sizes | measurement under real load |
| Whether supervisor composition survives a second behavior | Milestone 6, rule-of-three |
| Profile format specifics | Milestone 6, when a behavior exists to configure |

**The rule that governs from here** (`05` §−1, rule 0.1): build the simplest thing that works,
then stop. Every one of those unknowns gets *cheaper* to answer once code exists, and more
speculative the longer we plan instead.
