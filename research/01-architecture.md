# Cena — Architecture and Build Plan

> **STATUS: RATIONALE, NOT INSTRUCTION.** [`12-implementation-spec.md`](../plan/12-implementation-spec.md) is authoritative for implementation. The layer model (§1), the bottom building block (§2) and the seams (§3) still hold. **§5 (coroutines/dialect), §6 (Lua API surface) and Phases 5-6 of §8 are VOID** — see `12` §1.


Written 2026-09-17, revised 2026-09-18. Grounded in `E:\Cena\inventory\` (ten documents)
and the origin decisions in `E:\Cena\decisions\`.

> **Two later documents amend this one. Read them alongside it:**
> - **[`08-curated-behaviors.md`](08-curated-behaviors.md)** — Cena has **no embedded
>   scripting language**. Automation is curated Rust behaviors configured by data profiles.
>   Wherever this document says "Lua" or "script," read **"curated behavior."** The layer
>   `cena-script` is now `cena-behavior`, and §5's coroutine/dialect discussion is replaced
>   by "behaviors are async Rust tasks."
> - **[`04-inherited-decisions.md`](04-inherited-decisions.md)** — interrogation of the six
>   load-bearing decisions inherited from Lich/Vellum/eohunter. Its corrections **C1–C21**
>   are applied throughout this document and marked inline as *"Correction (Cn)."*
>   C14, C19 and C20 are moot under curated behaviors.

Cena is one Rust binary that does what Lich-5 and VellumFE do together, with Lua as the
scripting language, on desktop and mobile, for GemStone IV and DragonRealms, running
several characters at once.

This document is the inventory of **what we need**: the layers, the seams, the bottom
building block, and the order to build them in.

---

## 0. The three decisions everything else follows from

Before layers, three choices constrain the whole design. They are made here explicitly
because every later decision inherits them.

### 0.1 Parse before anything else sees it

**Cena parses first.** Network bytes become typed frames before any behavior, filter, or
UI sees them. Nothing above L2 receives a raw chunk. A filter suppressing combat spam
cannot accidentally suppress a roundtime state transition, because those are different
types, not different regexes over the same string.

**Correction (interrogation C1): the decision is right, but my original reason was wrong.**
I claimed Lich's text API "causes most of its structural problems." It does not — Lich
*already* parses to typed state before any hook runs (`games.rb:975`, `:1068`) and feeds
scripts line-split text on a **separate, non-mutating** path (`games.rb:1089-1094`).

The real cause is narrower and more interesting: **Lich's filter chain and its
frontend-forwarding path are the same pipeline** (`games.rb:1149-1181`). A script filter and
the bytes going to StormFront travel together, so suppressing one risks corrupting the
other. Cena removes that class of bug by *separating the seams*, not merely by parsing early.

Three consequences, which the rest of the plan must honor:

**(a) Two distinct APIs, not one hook (C2).** `observe(frame)` cannot mutate, and must offer
frame / line / raw-chunk granularity — otherwise consumers get driven into the filter path,
which is exactly what happened to Lich's `Combat::Tracker`, `Messages` and `xmlparser`.
`filter(frame) -> Option<Frame>` is **presentation only** and never affects another
consumer's input. Filter ordering is deterministic and explicit.

**(b) Presentation-state frames are non-suppressible by construction (C3).**
`QUIET_STATE_TAGS` (`util.rb:79-140`) is a hazard register, not a solved problem. Types
alone do not stop a filter dropping a `StreamOpen`/`StreamClose` pair and leaving the UI
wedged. Frames that carry presentation state are structurally exempt from filtering.

**(c) The parser must be PERMISSIVE (C6 / trap 12) — a capability a Rust rewrite would
otherwise silently lose.** `games.rb:1054-1058`, verbatim:

> Ox is a permissive parser: it handles Simu's not-quite-XML stream without the clean/retry
> dance REXML required (nested quotes, missing 'd' end tags, etc. are tolerated rather than
> raised).

Plus `repair_malformed_attributes_and_reparse` (`games.rb:1119-1131`) for the `settingsInfo`
bug and `title='Tsetem's Items'`, and `convert_special: false` with a documented
entity-corruption history. **A strict Rust XML parser will reject this stream.** Choose or
configure the parser for tolerance, and test against malformed real-world captures.

### 0.1a Stream desync — typed frames make this WORSE (C5 / trap 13)

Absent from the original plan, and the interrogation is right that it is dangerous.

Lich promotes truncation-class parse errors to `GameStreamDesyncError`, logs, and **resets**
`XMLData` rather than killing the session (`games.rb:1097-1103`). Cena needs the same shape,
and needs it *more*: **a desync that shows as garbled prose in Lich shows as silently-wrong
typed state in Cena, persisting indefinitely.** Text does not accumulate; typed state does.

Required: detect desync, log it, reset the affected typed state, do not kill the session.

### 0.1b The wire is BIDIRECTIONAL — own an emit vocabulary (C7)

Entirely missing from the original plan, and called "the largest free win from owning both
sides." The frontend wire is not just something Cena reads: Lich scripts **inject dialog
XML** (UberBar), and Vellum invented its own elements — `vellumTimer`, `vellumCmd`,
`vellumImg`.

So `Frame` is not only a *parse* vocabulary, it is an **emit** vocabulary. Cena's behaviors
and UI speak the same typed language in both directions. Because Cena owns both endpoints
(§5a), it can extend that vocabulary freely instead of negotiating with third-party clients.

### 0.2 One session is an actor; scripts never share mutable state

Lich runs one character per OS process and coordinates them over local TCP with a shared
secret (`internal_api/active_sessions/`, 1,643 lines). Cena is one process, so that
transport is accident — but the *rule* it encodes is real: a session exposes a read-only
snapshot of itself and nothing may reach into another session's interior.

**Cena owns each session as an independent unit with its own state, its own script
scheduler, and its own command queue.** Cross-session visibility is a read-only snapshot,
exactly as `Lich::API.active_sessions` intended.

### 0.3 Command round-trips are queued, not raced

The single most valuable finding in the whole inventory (`08` §4.5):

> `issue_command` is fundamentally a *serialization* primitive that isn't actually
> serialized. Nothing stops two scripts running `issue_command` concurrently against the
> same session — they each install a hook, and both hooks see the same lines. The
> start/end patterns are the only thing keeping them apart, and only by luck of them
> being different.

**Cena makes this a real queue.** Per session, a command round-trip takes a turn:
an async mutex or command channel with a `oneshot` reply. This removes an entire class of
cross-script interference that Lich can only mitigate. It is nearly free if designed in
at the start and very expensive to retrofit.

---

## 1. The layers

Strictly one-directional. Each layer may call downward and emit upward. Nothing calls up.
This is Vellum's rule, and Vellum enforces it mechanically in `tests/architecture.rs` —
Cena does the same from commit one (see §7).

```
  ┌─────────────────────────────────────────────────────────────┐
  │ L6  FRONTENDS      TUI · GUI · Web · Headless               │
  │                    (mobile = headless + web, see §5)         │
  ├─────────────────────────────────────────────────────────────┤
  │ L5  SCRIPT RUNTIME Lua VM · scheduler · world/events/actions │
  │                    the three seams (§3)                      │
  ├─────────────────────────────────────────────────────────────┤
  │ L4  SESSION        one character: state + queue + scripts    │
  │                    multi-session registry (read-only across) │
  ├─────────────────────────────────────────────────────────────┤
  │ L3  GAME MODEL     typed game state · events · game data     │
  │                    GS and DR behind one trait (§4)           │
  ├─────────────────────────────────────────────────────────────┤
  │ L2  PROTOCOL       bytes → typed frames (XML/GSL) · commands │
  ├─────────────────────────────────────────────────────────────┤
  │ L1  TRANSPORT      TCP · TLS · auth (EAccess) · reconnect    │
  └─────────────────────────────────────────────────────────────┘
        L0  PLATFORM   storage · paths · logging · config
```

### Why this order

The layers are ordered by **what can be tested without the layer above it**. L1–L3 can be
tested against recorded game transcripts with no UI and no scripts. L4–L5 can be tested
with a synthetic world and no network. L6 needs nothing real underneath. That property is
what makes the build plan in §8 possible.

---

## 2. The bare bottom building block

**The frame.**

Not the socket, not the parser, not the UI. The typed value that represents "one thing
the game told us" is the foundation, because every other layer is defined in terms of it:
the protocol layer produces frames, the game model consumes frames, events are derived
from frames, the UI renders frames, and scripts observe frames.

```rust
// L2. The vocabulary of the entire system.
pub enum Frame {
    Text(StyledText),          // prose, with styling/links preserved
    RoomChange(RoomInfo),
    Vitals(VitalKind, u8),
    Indicator(IndicatorId, bool),
    RoundTime(Instant),
    Prompt(PromptInfo),
    StreamOpen(StreamId),
    StreamClose(StreamId),
    Component(ComponentId, StyledText),
    Unknown(RawTag),           // never lose data we did not model
}
```

Two rules that matter more than the exact variants:

1. **`Unknown` is mandatory — and must distinguish two cases (C4).** Vellum's design is
   better than my original single-variant sketch (`src/parser/text.rs:174-195`,
   `parser.rs:1036-1051`): a sorted `KNOWN_WIRE_TAGS` binary search splits
   **known-but-unhandled** (log for debug, swallow — we know it exists and chose not to
   model it) from **genuinely unknown** (render as *visible text*, so the user sees what the
   game said even when we cannot type it). Carry `Frame::UnknownTag { name, raw }` so a
   behavior can match by tag name, plus a rate-limited raw-line escape hatch. Never panic,
   never silently drop.
2. **Frames are the only thing that crosses L2.** No layer above ever sees a byte or an
   unparsed string.

**First artifact of the whole project:** a frame vocabulary plus a parser that turns
recorded transcripts into frames, with a corpus of real captured game sessions as the
test fixture. Nothing else can be trusted until this is right, and everything else is
cheap to change by comparison.

Vellum already has this in `src/parser/` (~7,857 lines, of which 4,393 is `tests.rs`) with
`KNOWN_WIRE_TAGS` and a `ParsedElement` enum. **This is the single most valuable thing to
port, and it comes with its tests.**

---

## 3. The seams

A seam is a boundary where the thing on each side may change independently. Cena has
five that matter.

### Seam A — Protocol ↔ Game model (`Frame`)

Above: game meaning. Below: wire format. Lets us add DR without touching the game model's
consumers, and survive Simutronics protocol changes in one place.

### Seam B — Game model ↔ Scripts: **the three seams of eohunter**

This is the design eohunter arrived at after 83K lines of production use
(`scripts/eohunter/world.rb:1-20`), and it is convergent with where Lich core is heading.
Cena adopts it directly:

| Seam | Purpose | Rule |
|---|---|---|
| **`world`** | read durable facts | read-only, no sends, **no text parsing** |
| **`events`** | observe momentary facts | typed pub/sub with blocking await |
| **`actions`** | change the world | the only path that sends commands |

eohunter's own header states the discriminator, and it is the part worth copying verbatim:

> Durable facts live here; momentary facts travel on the event bus.
> No text parsing. If a fact isn't derivable from Lich state, it belongs in patterns.rb
> as an event, not here.

"Am I standing? where am I? what is in my hands?" → `world`. "I was just stunned" →
`event`. The mechanism is chosen by the **lifetime of the fact**, not by convenience.
That is what stops a facade rotting into a god object — and eohunter shows the failure
mode too: its own audit scores `world.rb` as carrying "about 190 lines of Forge leftovers
no behavior calls."

**`world` is an instance, never a global.** Three characters means three worlds.

**Correction (C8): I claimed this "is what makes multi-session free." That was backwards.**
In eohunter, `World` is an instance but **`Events` is a process-wide singleton**
(`events.rb:118-123`), and eohunter's actual shipped multi-session story is *refusal to
start* a second copy (`eohunter.lic:127`). An instance-shaped read facade buys nothing if
the event bus is global.

In fairness to the author, this was **Lich-imposed, not a design failure**: Lich's
load-based script reload forces `remove_const` (`engine.rb:29`), which pushes toward a
singleton bus. Cena has no such constraint. **The bus is session-owned**, like everything
else behind Seam C.

**Split game facts from engine telemetry (C9).** Most of eohunter's `Events.emit` traffic is
*engine narration* consumed by an `:any` subscriber for logging (`controller.rb:668`), not
game facts. Conflating the two makes every telemetry line pay the typed-event tax, and an
`:any` subscriber becomes a cross-session leak across exactly the boundary Seam C exists to
protect. Two channels: typed game events, and diagnostic telemetry.

**Adopt the rules, not the shape (C10 / trap 4).** `World`'s 156 methods — 103 of them
one-line `def x = ::CONST` delegations — are **RSpec scaffolding**, because Ruby cannot stub
a hard-coded constant (the author says so: `docs/guides/architecture.md:108-111`). In Rust
the read-only guarantee is `&` and injectability is a trait. Porting the *shape* yields
ceremony that accretes: eohunter's went 596→982 lines in seven days. Two parts of it *are*
genuine and worth keeping: `def clock = Time` is real time injection, and the
rescue-to-safe-default rule is a real boundary.

**A fourth seam is needed (C11).** The three-seam taxonomy is incomplete, and eohunter
proves it: it grew `Engage`, `Cleanse`, `Maintain`, `Routines::State` — **fight-scoped and
room-scoped latches** that fit neither "durable fact" nor "momentary event" under its own
discriminator (`architecture.md:114`). Cena needs an explicit place for *scoped, resettable
working state* whose lifetime is a fight, a room, or a task. Without one, it will end up
smeared across `world` and the behaviors, exactly as it did there.

**Arm before send.** From `actions.rb:295-322`:

```ruby
waiter = Events.arm(*types, &matcher)   # arm FIRST
first  = send_through_ladder(command)   # then send
event  = waiter.wait(timeout:, interrupt: -> { interrupted? }) { me.dead? }
```

The comment says why: *"Arm before the ladder so an immediate named event is retained
while fput finishes."* Send-then-subscribe loses fast responses and waits forever on an
event that already happened. This is the most common bug in game automation, and Lich's
`issue_command` independently solves it the same way. **Any Cena request/response API
must be arm-then-send.** A naive `send_and_wait(cmd, pattern)` would reintroduce the race
in every script that uses it.

Every wait returns a **reason** (`confirmed` · `timeout` · `interrupted` · `dead` ·
`cancelled`), never a bare bool, and every wait takes an interrupt predicate so a stopping
engine cannot hang. This also fixes legacy Lich's inconsistent failure values
(`matchtimeout` → `false`, `dothistimeout` → `nil`, `fput` → `false` or a symbol).

**Do not port the 50ms polling loops (trap 5).** eohunter polls at 50ms in three places
(`events.rb:70`, `actions.rb:341`, `:361`), which silently sets a **50ms floor on every
confirmation**. That is a workaround for Ruby's GVL and condvar plumbing cost around Lich's
downstream-hook thread — not a design choice. Rust gives real `Notify`/channel wakeups at
microsecond cost. Same shape as the 58µs regex artifact: correct for them, wrong for us.

### Seam C — Session ↔ Session (read-only snapshot)

What Lich's active-sessions service encodes, minus the TCP. A session may read another's
snapshot; it may never touch its interior.

**What to port and what to delete.** Of the service's 1,643 lines, roughly **900 are pure
transport accident** — TCP server, JSON framing, auth token, discovery file, flock
ownership election, Windows retry ladder, pid liveness — and should *not* be ported. The
rules that survive in-process:

- **Snapshots, never handles**, across every boundary (`registry.rb:63`, `:73`).
- An **explicitly enumerated** cross-session payload, 10 fields (`lifecycle.rb:326-337`) —
  not "serialize whatever the session struct happens to hold."
- **Presence ≠ connected** (`lifecycle.rb:15-20`, a documented postmortem).
- Composite health computed centrally; generation counters for restart races; inert
  fallbacks; a kill switch.

**Cena deletes the heartbeat entirely.** Lich heartbeats every 2s (`lifecycle.rb:35`) only
to detect that an owner *process* died. In-process sessions signal death by dropping their
task handle.

**The gap — and I was wrong to call it "the biggest footgun" (C12).** Lich's shipped
protocol is exactly ping/upsert/remove/snapshot (`server.rb:228-242`), and eohunter built
**1,531 lines of DRb** in `group.rb` to get coordination. I concluded that cross-session
messaging was dangerous.

The evidence says otherwise. Lich's maintainers **built it twice**: PR #1613 (read-only,
1,826 lines, 7,591 passing examples, p99 2.786ms) and PR #1618 (write-capable). Both were
closed unmerged on 2026-09-14 over a **core-bloat objection** — a reason that does not apply
to Cena, where this is a first-class feature rather than an addition to an already-large
Ruby core. It was not rejected as unsafe or unworkable; it worked.

**So: design the contract now, implement when a second session needs it.** Design now
because it *constrains the session-actor boundary* — tickets, digests, fencing, and the
distinction between Operations and Leases all shape what a session actor must expose. It
still holds that it is message-passing only: **never shared mutable state, never a handle
into another session.**

Also correcting a related claim: `Lich::API`'s zero callers are a **deliberate reversal**,
not abandonment of the idea.

**Keep supervisor-observed liveness (C13).** Lich's PID sweep (`registry.rb:121-130`) is a
*positive* property, not transport accident: it reaps a crashed peer **without that peer's
cooperation**. When Cena drops the TCP transport it must keep the property — a session whose
task has died or wedged is detected and cleaned up by the supervisor, not by asking it.

**Recalibrate the scale (C15).** I have been writing "3+ characters." PR #1319 documents
**25 concurrent sessions in production**. Design for that order of magnitude. But do *not*
import that PR's `SQLite3::BusyException` contention as a preview of Cena's behavior — that
storm is a multi-process file-locking artifact under DELETE journaling, and #1319's own body
concedes the residual is negligible (~1 write every 2.4s across 25 processes, each holding
the lock <1ms). **It is evidence *for* collapsing to one process**, not a warning about it.

### Seam D — Core ↔ Frontend (state snapshot + input event)

What lets one core drive four UIs. Investigated in full in
[`02-frontend-seam.md`](02-frontend-seam.md); the short version:

**Do not port Vellum's `Frontend` trait.** It has exactly one implementor; the GUI
deliberately declines it because eframe inverts the loop, and the web frontend is a sidecar
that receives deltas. The trait could not even name `AppCore`, forcing
`render(&mut self, app: &mut dyn Any)` and a downcast.

**Port the seam it failed to express:** a frontend-agnostic **state snapshot** plus a
frontend-agnostic **input event**, with each UI owning its own loop — proven three times in
Vellum (TUI polls, GUI inverts, web pushes over a socket).

**Discipline:** input types live in Cena's own vocabulary (`cena_data::input`), never
re-exported from ratatui/egui/web. That one rule is what let three different loop models
share a single keybinding system.

**Cena owns both sides of this seam**, which Lich does not — see §5a.

### Seam E — Game ↔ Game (GS vs DR behind a trait)

See §4. This is the seam Lich never built, and the reason its DR and GS code are
parallel universes.

---

## 4. The GemStone / DragonRealms seam

The inventory makes the asymmetry concrete: GemStone has a rich XML feed; DragonRealms
scrapes far more from plain text (`03-lich-dragonrealms-libraries.md`). Lich handles this
by having two mostly-separate codebases.

**Cena puts one trait at L3**, with per-game implementations below it:

```rust
trait GameAdapter {
    fn parse(&mut self, input: &[u8]) -> Vec<Frame>;   // XML for GS, text+XML for DR
    fn capabilities(&self) -> Capabilities;            // what this game can report
    fn command_for(&self, intent: Intent) -> String;   // verb differences
}
```

`world.wounds`, `world.room`, `world.me.stamina` become one Lua API whose *implementation*
differs per game. Scripts that stay within the shared vocabulary run on both games
unchanged; scripts needing game-specific behaviour ask `world.game` explicitly.

**Capabilities must be honest.** Where a game genuinely cannot report something, the API
returns "unknown", never a fabricated default. A script must be able to distinguish "you
have 0 mana" from "this game does not report mana that way."

---

## 5. Mobile

**Corrected 2026-09-18 — mobile's real role.** Mobile is not a *feature priority*; it is a
**structural forcing function**, and one specific consequence of it is load-bearing:

> **Mobile OSes suspend background processes.** Lich's architecture — a proxy process plus a
> separate frontend process coordinating over local TCP — **cannot work there**. One process
> survives backgrounding; two cooperating processes do not. This is why Cena is **one
> binary**.

That is a constraint derived from platform behavior, not from player demand — so
`11-player-requirements.md`'s finding that only 6% of players mention mobile does **not**
undermine it. The two claims are about different things.

**And the single-binary decision is over-determined** — it would be right without mobile at
all. The interrogation (`04` Decision 5) found Lich runs one OS process per character because
its character model *is* process-global state: `GameObj`'s `@@right_hand`/`@@left_hand`
(`lib/common/gameobj.rb:25-42`) **are** the character's hands, and `XMLData` is a singleton
referenced 575 times across `lib/`. Two characters cannot coexist in one Ruby process. Cena
has no such constraint, and PR #1319's `SQLite3::BusyException` storm across 25 processes is
evidence *for* collapsing to one process.

**What this means in practice:**
- **Keep:** one binary; L0–L5 compile for Android/iOS in CI (a dependency that breaks the
  mobile build fails the build); nothing that assumes a second cooperating process.
- **Drop:** mobile as a driver of day-to-day priorities. **Development and testing are
  desktop-first.** Mobile frontends, mobile UX and phone-class performance tuning are not
  Milestone-1 concerns.
- **Honest status:** mobile *playability* is a **bet**, not validated demand
  (`11` headline 3 — 9 of 143 people mention it, and the one person who plays on a phone does
  so on VellumFE and reports it failing). The *architecture* it forces is justified
  independently; the *product* is speculative.

Vellum already proves the shape: Android/iOS build the library `--no-default-features`
(core + parser + network + web + headless) and the mobile shells host the web UI. Vellum's
`tests/architecture.rs` enforces that `core/` stays Android-safe.

Cena inherits both the shape and the enforcement:

- **L0–L5 must compile for mobile targets.** CI builds them for Android/iOS from day one.
  A dependency that breaks the mobile build fails the build, not a future port.
- **L6 is where platforms diverge.** TUI and GUI are desktop; mobile gets headless + web.
- **The script runtime must be cheap.** This is the real mobile constraint, and it decides
  the concurrency model below.

### The concurrency decision

Nearly every interesting script operation blocks: `get`, `waitfor`, `matchwait`, `dothis`
are all marked `yes/∞` in `01-lich-runtime-and-api.md`. Ruby gave each script an OS
thread. Two options:

| | OS thread per script | **Lua coroutines on an async runtime** |
|---|---|---|
| Matches Lich | yes | no |
| Cost at 3 sessions × N scripts | high | low |
| Mobile | poor | good |
| Blocking API | natural | every blocking call is a yield point |

**Decision: coroutines.** The census settles it — item 5 of Tier 0 is
`pause`/`sleep` at 64.5% of the corpus, annotated *"must be coroutine-yielding, not
blocking."* With multi-session in play, thread-per-script does not fit mobile.

Consequence: the scheduler is load-bearing and must be built early and correctly. It is
L5's hardest component.

---

## 5a. The constraint Cena does not inherit

Lich must serve frontends it did not write and cannot change — StormFront, Wrayth, Wizard,
Genie, Profanity, Saga, Suks. **Cena owns both sides of the wire.** Detail in
[`02-frontend-seam.md`](02-frontend-seam.md) §2; the cost Lich pays and Cena does not:

| Cost | Evidence |
|---|---|
| Per-frontend adapters | `lib/common/frontend/` — six files |
| Markup translation | `lib/common/markup.rb` (264 lines): `sf_to_wiz`, `fb_to_sf`, `strip_xml`, `monsterbold_*` |
| Frontend branching in core | 33 `$frontend` references in `lib/` |
| Launch machinery | `frontend_launcher.rb`, `frontend_locator.rb`, hosts-file/SAL redirection |
| Capability probing | `Frontend.supports_mono?` gates how `respond` wraps output |

`markup.rb` exists to *"rewrite a StormFront/XML line into the GSL escape vocabulary an old
Wizard frontend understands"* — and carries stateful cross-call buffers
(`$sftowiz_multiline`) because a `pushStream` can split across socket reads. Vellum, owning
both sides, pays none of this: `grep -rn "FrontendType" src/core/` returns **0**.

**Cena deletes:** dialect emission, UI-side markup buffers, frontend launching/locating,
capability probing. ~400–600 lines plus the launch subsystem, and a whole class of
stateful bug.

**Cena does NOT get:** simpler *inbound* parsing. The game's XML/GSL is Simutronics'
protocol, unmovable, alongside EAccess auth. The freedom is strictly outbound. `markup.rb`'s
split-element problem is a warning about reassembly Cena must still get right (§0.1, and
why `Frame::Unknown` is mandatory).

**New rule — no `$frontend`.** The census finds 92 corpus reads of `$frontend` branching on
client type and recommends Cena expose it. **Reject that.** Scripts branch on it to ask
"what markup may I emit?" — a capability question wearing an identity costume. Under a
single frontend the identity is a *lie* scripts would silently mis-branch on. Scripts emit
**intent** (`echo{text=..., style="creature"}`) and the UI decides rendering; where a script
genuinely needs to know, it asks a **capability** (`ui.supports("inline_image")`), never an
identity. Same discipline as §4: capabilities must be honest.

---

## 6. What Lua actually needs (measured, not guessed)

From `07-script-corpus-api-census.md`, across 456 corpus files. This is the spec for the
Lua API, ranked by how many real scripts break without each capability.

**Tier 0 — nothing runs without these:**

| Capability | Scripts | % | Note |
|---|---:|---:|---|
| `echo`/`respond` — write to client | 380 | 83.3% | trivial |
| Closures passed to APIs | 363 | 79.6% | Lua-native |
| `fput`/`put` — send to game | 314 | 68.9% | the ladder (§6.1) |
| **Real regex** | 310 | 68.0% | **must bind a Rust regex crate; Lua patterns are insufficient** |
| `pause`/`sleep` | 294 | 64.5% | must yield, not block |

**Tier 1 (20–45%):** `Script.*` introspection · `DRC.*` · `before_dying` · settings/YAML ·
roundtime waits · start/kill scripts · movement · hand inspection · `parse_args` · room
state · `UserVars` · `GameObj` · `XMLData`.

**The blockers, with tractability:**

| Blocker | Scripts | Tractability |
|---|---:|---|
| GTK desktop GUI | 55 | **redesign required; blocks mobile** |
| Real regex | ~310 | solved by binding a Rust regex crate |
| `Thread.new` | 37 | port individually to coroutines |
| `$1`..`$9` match globals | 99 | mechanical rewrite |
| `retry` keyword | 45 | mechanical rewrite to loops |
| `eval` of foreign Ruby | 9 (2 severe) | not translatable; redesign |
| `method_missing` | 22 | metatable `__index` |

Good news the census confirms: monkey-patching is **effectively zero**, metaprogramming is
**light**. The corpus is more portable than Ruby's reputation suggests.

### 6.1 The send ladder belongs in Rust

`fput` is not a function, it is a protocol: roundtime waits, "struggle to stand" →
auto-stand → resend, stunned/webbed polling, typeahead backoff, dead detection, bounded
resends, timeout, and unshifting the consumed line back onto the buffer
(`global_defs.rb:1479-1639`). It encodes a decade of knowledge about how these games
behave.

If Cena exposes a thin `send()` and lets Lua handle roundtime, we have deleted Lich's
value and pushed the hard part onto every script author. **The ladder lives in Rust**,
configurable, returning a result enum.

eohunter's audit proposed exactly this upstream to Lich as
`fput(cmd, max_resends:, timeout:, interrupt:)` returning a failure symbol — and **it has
landed** (`global_defs.rb:1483-1499`). This is convergent, not speculative.

**How the disagreement was actually resolved.** Core kept its give-up behaviour:
`resend_transient` landed defaulting to **false** (`global_defs.rb:1507`), but the give-up
path now pushes the refusal line back on the buffer and returns a named `:refused` instead
of a bare `false`. Cena should still take **capped resend** as its default — core's caution
is a backwards-compatibility argument about legacy scripts that does not apply to us.

**But the deeper defect is the one worth fixing.** "Transient" is *inferred from prose*, so
eohunter had to add `PERMANENT_REFUSALS` (`actions.rb:78-92`) because "You don't seem to be
able to move your legs" matches the same regex as a genuine transient refusal — a severed
leg is not transient, so the command went out five times and failed `:too_many_resends`.

**Cena classifies refusals in the parser as a typed enum** — `Transient` · `Permanent` ·
`Roundtime` · `Stunned` · `Webbed` · `Dead` — which makes retry policy trivially correct
instead of a regex race. This is §0.1 (parse before anything else sees it) paying off in a
concrete place.

---

## 7. Enforcing the separation mechanically

Comments do not hold a boundary. Vellum's `tests/architecture.rs` does.

**Correction (C16): my causal claim was backwards.** I wrote that enforcement "is why its
layering survived 310K lines." Vellum's first architecture test landed **2026-07-05 — nine
months in, at roughly line 250K, and *after* the author had hand-split a 9,227-line `impl`
block.** Dependency rules followed at ~2.5 months, facade caps six weeks later.

**Enforcement ratchets a refactor you already did; it does not do the refactor for you.**
The encouraging half of the finding: once in place, **Vellum's caps were never once
raised.** The ratchet holds.

So Cena starts with the enforcement from the first commit — not because it will prevent bad
structure on its own, but because starting with the ratchet means never having to do the
9,227-line split in the first place. These rules are cheap to keep and expensive to restore:

1. `protocol` must not depend on `game_model`, `session`, `script`, or `frontend`.
2. `game_model` must not depend on `session`, `script`, or `frontend`.
3. `world` must not import the command sink — **a read facade cannot send**.
4. `world` must not import the parser — **a read facade cannot parse text**.
5. `frontend::gui` must not import `frontend::tui` (and vice versa).
6. L0–L5 must compile for Android and iOS targets.
7. No layer may reference a raw stream type above L2.

Rust makes 1–5 and 7 stronger than Vellum can: module visibility and the crate graph are
compiler-enforced, not test-enforced. Cena should use a **workspace of crates**, not one
crate with modules, so the boundaries are structural.

This is also where Cena starts ahead of Lich rather than behind it. The arbiter inventory's
finding on Lich's own facade (`08` §1.3):

> The API/InternalAPI split is a stated architectural intention backed by 44 lines of
> unreferenced code. The *sealing* is real. The *facade* is aspirational. For Cena this is
> the most useful kind of finding: Lich's authors have written down exactly what boundary
> they want, and have not yet been able to make anything depend on it. Cena can start where
> Lich wants to end up, because in Rust the boundary is a compiler-enforced module
> visibility rather than a naming convention.

And the seal itself leaks, in the way dynamic languages always do. `lifecycle.rb:282`
calls `ActiveSessions.send(:register_session_admitted, payload)` — reaching past `private`
via Ruby's `send`, with a comment admitting it:

> `# ActiveSessions keeps the admitted-path helpers private so the`
> `# feature-gate bypass stays local to lifecycle-owned call sites.`

In Rust that is `pub(crate)` and the seal is real. **This is the single clearest argument
for the whole approach**: Lich cannot enforce its own stated boundary in Ruby, and Cena
gets it for free from the compiler — but only if the crate boundaries exist from day one.

**Assessment of the arbiter thesis: true, but only ~40% realized.** Typed events (9,460
LOC) and mediated commands are shipped and consumed in production. The public facade is 44
lines with zero callers. Cross-session coordination is 1,643 lines, built and tested but
**dormant behind a default-off feature flag**. Cena should treat the direction as
validated and the implementation as unfinished — which is exactly the opportunity.

---

## 8. Build order

Each phase ends at something demonstrable. No phase depends on a later one.

### Phase 0 — Skeleton (foundations)
Workspace of crates matching L0–L6. Architecture tests (§7) **first**, so no phase can
violate them. CI: desktop + Android + iOS builds. Transcript fixture corpus.
*Done when:* an empty workspace builds on all targets and the architecture tests pass.

### Phase 1 — Frames (the bottom block)
`Frame` vocabulary. GS XML parser, ported from Vellum's `src/parser/` **with its tests**.
Transcript replay harness.
*Done when:* recorded GemStone sessions replay to frames with no `Unknown` regressions.

### Phase 2 — Connection
TCP/TLS, **EAccess authentication** (the highest-risk reimplementation — an unmovable
external protocol), reconnect, the raw session loop.
*Done when:* Cena logs in and prints the game to stdout. **First end-to-end proof.**

### Phase 3 — Game model
Typed game state from frames. Infomon-equivalent, informed by the field-by-field gap
analysis in `05` §9 (Vellum's `CharacterState` is nine fields; Lich's Infomon has
hundreds of keys — stats, skills, PSMs, and detailed experience are the big holes).
Event derivation. Static game data (crit tables, creature templates) shipped as data files.

**The state model, specified (C21):**
- **Typed named fields** for the ~120 closed-vocabulary values (10 stats × 6 variants, 45
  skills, experience, society, citizenship, account). The game will not invent an eleventh
  stat without a client release.
- **`BTreeMap` + typed accessor hybrid ONLY** for genuinely open sets (status conditions,
  PSMs) — copy Vellum's shape at `src/core/state.rs:340-410`.
- **`updated_at` as a per-group `Option<SystemTime>`**, so staleness is answerable per group
  rather than globally.
- **Persistence is a serde snapshot, not a storage model.** (Trap 3: Infomon's SQLite shape
  is a *retrofit*. PR #325 shipped cacheless; within ~5 weeks they added a RAM hash, async
  write queue, barrier flush and mutex. The shipped design is **RAM is the model, SQLite is
  the durability tier** — and "stringly-typed keys are flexible" was never true: four
  normalization bugs in one 3-line function, every one a compile error in Rust.)
- **No generic `state.get("key")` accessor.** Exactly one such call exists across 456 corpus
  scripts; it does not earn the type hole.
- **Budget for the sync cost.** Lich's `Infomon.sync` is ~15 commands over 30–60s at login.
  Cena pays the same — the game only reveals this state on request.

*Done when:* character state tracks a live session correctly.

### Phase 4 — One frontend
Headless + web first — it is the mobile path and the cheapest to verify.
*Done when:* Cena is usable as a game client on desktop and phone.

> **Milestone: Cena replaces Vellum for a non-scripting user.**

### Phase 5 — Script runtime
Lua VM, coroutine scheduler, the three seams (`world` / `events` / `actions`), the send
ladder, the per-session command queue, Tier 0 API.
*Done when:* a real script hunts.

### Phase 6 — API completion
Tier 1 and Tier 2 by census rank. Settings, script lifecycle, hooks with ownership.
*Done when:* eohunter's capability list is expressible (see §9).

### Phase 7 — Multi-session
Session registry, read-only cross-session snapshots, frontend session switching.
*Done when:* three characters run at once and can be swapped between.

### Phase 8 — DragonRealms
`GameAdapter` second implementation, DR text scraping, `DRC.*` equivalents.
*Done when:* a DR character runs.

### Phase 9 — Desktop frontends
TUI and GUI, ported from Vellum.

**Note the ordering choice:** scripting comes *after* a working client (Phase 5 after
Phase 4). Cena is useful to someone at the end of Phase 4, which means real feedback
arrives before the largest and least reversible subsystem is built.

---

## 9. Open questions

1. ~~**Is Vellum's frontend abstraction real?**~~ **RESOLVED — see
   [`02-frontend-seam.md`](02-frontend-seam.md).** The trait is indeed a near-fiction (one
   implementor; the GUI declines it; the web is a sidecar) but **the seam underneath it is
   sound**: a frontend-agnostic state snapshot plus a frontend-agnostic input event, each
   UI owning its own loop. Port the seam, not the trait. **Phase 4 and 9 costs stand.**
   Two rules follow: input types live in Cena's own vocabulary, never re-exported from
   ratatui/egui/web; and the Lua API exposes **UI capabilities, never a frontend
   identity** (no `$frontend`).
2. **What is the conformance target?** Proposal: **eohunter**. ~18,000 LOC in
   `scripts/eohunter/` (25,957 across `scripts/`, 43,156 repo-wide including specs — *not*
   the 83K I said earlier, which counted the whole repo loosely), written deliberately
   against a modern core with an explicit audit of what it consumes. "Can Cena's Lua API
   express eohunter?" is a far better test than "can it run the legacy corpus." The census
   tells us what we would break; the eohunter audit tells us what we should build.

   The headline number to target: **eohunter has four send paths for an 18,000-line
   engine** — the ladder, `Spell#cast`, the PSM readers, and `Lich::Util.issue_command`.
   That is what a good arbiter boundary buys.
3. **Lua dialect** — LuaJIT (fast, mobile-friendly, 5.1) vs Lua 5.4 (modern, integer
   division, goto) vs `mlua` bindings over either. Affects regex binding and coroutine
   performance.
4. **Legacy corpus policy — I consider this now answered, pending your confirmation.**
   Model the Lua API on **eohunter's direction, not the legacy corpus**, decisively. Two
   arguments settle it:
   - The compatibility argument is *already conceded* by the Ruby→Lua move. Every script
     must be rewritten anyway, so preserving the legacy API shape buys nothing.
   - Blocking globals are **structurally incompatible with multi-session**. `fput` with no
     session argument is meaningless when three characters are logged in. The legacy API
     cannot express the thing we are building.

   **Costs, stated plainly:** hand-porting every script (high, but already paid); a
   conceptual step up for script authors; more core work before first light; and the real
   risk that an incomplete event catalog pushes authors back to raw text scraping. Two
   mitigations: write any compatibility shim **in Lua, never in the core API**, so it stays
   deletable; and ship raw-line access only as an explicit, documented, discouraged escape
   hatch.

   (Corpus measured at **474 `.lic` files** — 251 GemStone + 223 DragonRealms. 55 GTK
   scripts need redesign regardless, since GTK has no mobile path.)
5. **Static data licensing/provenance** — crit tables and creature templates would ship
   from Lich. Confirm that is acceptable and attributed.

---

## 9a. The Combat pipeline is the model to copy

`08-lich-arbiter-direction.md` §3 identifies the specific design properties worth taking,
not just the idea of typed events:

- **Two separate hooks with different granularity** — chunk-on-prompt for combat, one line
  at a time for messages — each installed only while something is subscribed and removed
  with the last subscriber.
- **O(1) enqueue that never blocks the stream.** Parsing happens off the read path.
- **A single ordered worker**, so creature state needs no synchronization at all.
- **A declared payload key-type table** (`defs/messages.rb:51-71`) that stops user-supplied
  supplements from changing an event's shape. Cena's equivalent is a typed enum — the
  compiler does this job.
- **`watch.rb:226` asks core which events exist** (`Combat::Messages.events`) and
  subscribes to all of them by name rather than hardcoding a list. A script treating core
  as a *schema source* is the clearest single signal the arbiter direction is real and
  being taken up.

**One thing not to copy.** `parser.rb:34-40` documents a *measured* result that many small
literal unions beat one big union (58µs vs 35µs per line). That is a **Ruby-regex
artifact**; Rust's `RegexSet`/Aho-Corasick inverts the conclusion. Port the gating
**discipline** (measure, gate cheaply before parsing) — not the gating **strategy**.

---

## 10. The shortest statement of the plan

Build the **frame** first, because everything is defined in terms of it. Put a
**compiler-enforced boundary** between every layer before there is anything to enforce.
Give scripts **three seams** — read, observe, act — and never a fourth. Keep the
**hard-won game protocol knowledge in Rust** where every script benefits, and expose
**intent** to Lua. Make **sessions independent** and **command round-trips queued**, so
multi-session and cross-script safety are structural rather than retrofitted. Ship a
working **client** before a working **script engine**, so real use informs the largest
subsystem.
