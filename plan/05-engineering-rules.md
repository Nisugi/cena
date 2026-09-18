# Cena Engineering Rules

Written 2026-09-17. The rules we live by during implementation: what goes where, what may
not, how things are named and found, and how each rule is *enforced* rather than merely
stated.

Grounded in the inventory (`E:\Cena\inventory\`) and in the decision interrogation
(`04-inherited-decisions.md`). Read [`../inventory/00-fairness-note.md`](../inventory/00-fairness-note.md)
first: the reference codebases made their choices under real constraints, and several of
these rules exist only because *they* discovered the problem first.

---

## −2. Evidence — a finding without proof is speculation

**We do not speculate.** This rule governs every document in `plan/` and `inventory/`, every
subagent brief, and every claim made in conversation about this project.

### Rule E.1 — Every finding carries its proof, inline

A finding is a claim about how something *is*. It ships with the evidence that establishes
it, at the point of the claim:

| Kind of claim | Required proof |
|---|---|
| Code behaves this way | `path/file.rb:123`, and a quote for anything subtle |
| A number (LOC, counts, frequencies) | the command that produced it, or its source document |
| Why a decision was made | the author's own words, quoted, with a link or citation |
| A performance property | a measurement, on stated hardware, with the workload named |
| What users want | a distinct-person count and exact quotes |
| A crate/library behaves this way | version number and a link to docs or source |

A claim with no proof is deleted or demoted to a marked hypothesis. There is no third option.

### Rule E.2 — Three labels, always visible

Every non-obvious statement is exactly one of:

- **VERIFIED** — proof attached, and the proof was checked.
- **INFERRED** — reasoned from evidence but not directly observed. **Must say what would
  confirm it.**
- **UNVERIFIED / ASSUMPTION** — believed, not established. **Must say what would settle it.**

The failure mode this prevents is not lying; it is **inference silently promoted to fact by
repetition**. A number repeated three times starts to feel measured.

### Rule E.3 — Cite the measurement, not the memory

If a number came from a document, cite the document. If it came from a command, give the
command. **Never restate a number from recollection** — re-derive it or quote its source.

> **This project's archetype, and it is mine.** I reported *"486 lookarounds across 85 files"*
> as a corpus measurement and reasoned from it across several turns. It was a **double-count
> of named captures**; the real figure is ~87–119. It was caught only because an adversarial
> agent re-measured. Similar slips caught by verification: `creature_cards` off by 1,358
> lines; `jinx` off by 425; a DragonRealms header contradicting its own table by 1,000 lines;
> "83k lines" for eohunter when it is ~18k.

### Rule E.4 — Adversarial verification for anything load-bearing

If a finding will change a decision, **something other than its author checks it.** That has
been this project's highest-yield practice: every batch of research has had material errors
caught, including one where the *verifier itself* was wrong (it claimed `lib/` is 241,957 LOC;
the correct figure, 106,642, was the analyst's). Which is the point — **the check is a second
opinion, not an authority.** Resolve disagreements by re-measuring, not by seniority.

### Rule E.5 — Negative results and silences are findings, and need proof too

"Nobody mentions X" requires the search that found nothing, with the terms used. "There is no
equivalent in Vellum" requires the grep. A silence is evidence only if you can show where you
looked.

### Rule E.6 — Separate the finding from the recommendation

*"`world.rb` has 156 methods, 103 of which are bare-constant delegations"* is a finding, and
checkable. *"Therefore Cena should not port its shape"* is a recommendation, and arguable.
Keeping them visibly distinct means a reader can accept the fact and reject the conclusion —
and means a wrong recommendation does not discredit a sound measurement.

### Rule E.7 — Provenance survives summarization

When a finding moves from a research document into a plan document, **its citation moves with
it.** A conclusion that arrives in the plan stripped of its source is unfalsifiable, and by
the time anyone doubts it the trail is cold.

### In code

The same rule, in the form the compiler and CI understand:

- A performance claim in a comment names the benchmark that supports it.
- A "we do X because the game does Y" comment cites the transcript or fixture proving Y.
- A workaround comment says what breaks without it, specifically enough to test.
- **Never port a performance conclusion** — port the discipline of measuring, then measure
  here (`inventory/09`, and the 58µs/35µs archetype).

---

## −1. KISS and DRY — the rules that outrank the rest

Everything below this section describes structure: crates, layers, seams, enforcement. All of
it is in service of a working client. **When structure and simplicity conflict, simplicity
wins**, and the rule that was in the way gets amended (§10).

### Rule 0.1 — Build the simplest thing that works. Then stop.

Not the most general, not the most extensible, not the one that anticipates a requirement
nobody has asked for. **Cena is a private client for one user.** It does not need plugin
systems, abstraction layers for hypothetical second implementations, or configurability
nobody will configure.

The test before adding any abstraction: **"what concrete thing that I need today does this
make possible?"** If the answer is "it would let us later…", do not build it. Later is
cheaper than you think, because with one caller you can change anything.

> **This plan has already violated this.** It proposed a `GameAdapter` trait (`01` §4) before
> a single line of GemStone parsing exists, and an agent-protocol capability catalogue before
> any behavior exists to expose. Both may prove right. Neither should be built before the
> concrete thing it abstracts over exists in at least one — preferably two — real forms.

### Rule 0.2 — The rule of three, for abstraction

Write it once. Write it twice. **On the third occurrence, abstract it** — because by then you
can see the actual shape of the variation instead of guessing.

Two copies of similar code is not a DRY violation worth fixing. It is *evidence being
gathered*. The premature abstraction is more expensive than the duplication, because it fixes
the wrong axis of variation and everything after it bends around the mistake.

> **Vellum's `Frontend` trait is the cautionary tale, and the interrogation named it exactly
> (trap 10):** it failed *"because it was written BEFORE the second frontend existed and
> specified the WRONG AXIS (a polled loop, which eframe inverts). The lesson is about TIMING,
> not traits."*

### Rule 0.3 — DRY applies to knowledge, not to text

The real rule is: **every piece of *knowledge* has one authoritative home.** A game constant,
a protocol rule, a crit table, the refusal taxonomy — one place, everywhere else refers to it.

It is *not*: "no two lines may look alike." Two functions that coincidentally share a shape
but change for different reasons should stay separate. Merging them couples two things that
have no reason to move together, and the next change has to un-merge them.

Ask: **"if this fact changed, how many places would I edit?"** More than one means a real DRY
violation. Identical-looking code that encodes two different facts does not.

Where this bites hardest in Cena: **game data must not be duplicated between Rust and data
files.** Crit tables, creature templates and spell data live in data files, and the code reads
them. One home.

### Rule 0.4 — Prefer deleting to adding

The best fix removes code. When a feature can be met by an existing mechanism, use the
existing mechanism even if it is slightly less elegant. Cena is already inheriting design from
three codebases totaling ~400K lines; the pressure will always be toward accretion.

`08-curated-behaviors.md` is this rule applied at the largest scale — an entire scripting
runtime, FFI boundary, sandbox and scheduler **deleted** by narrowing the requirement.

### Rule 0.5 — No speculative generality

Concretely banned until a real second case exists:
- A trait with exactly one implementor (unless it exists purely for test injection).
- A config option with exactly one value anyone uses.
- A "manager", "handler", "provider" or "factory" layer that only forwards.
- A plugin or extension point with no plugin.
- Generic parameters that are only ever instantiated one way.

**Corollary:** an architecture test that enforces a rule protecting against a problem we do
not have is also over-engineering. Enforcement earns its place the same way code does.

### Rule 0.6 — Complexity must be paid for by a requirement you can name

Some of Cena's complexity is genuinely earned, and it is worth naming *which*, so the rest is
visible as optional:

| Complexity | The requirement that pays for it |
|---|---|
| Crate workspace with enforced layers | multi-session + mobile + a codebase one person maintains for years |
| Typed frames | the parse-before-filter bug class, and agent safety |
| Multi-pattern Rust matcher | ~2,395 crit patterns × every line × 25 sessions, on a phone |
| Send ladder in Rust | a decade of game behavior knowledge that every behavior needs |
| Session-per-actor | multi-session, which is a headline feature |

**Anything not on a list like this should be justified or dropped.** If you cannot name the
requirement, that is the answer.

---

## 0. The meta-rule: a rule that is not enforced is a wish

Every rule below names its enforcement. If a rule cannot be enforced by the compiler, a
test, or CI, it is a **guideline** and is labelled as such — we do not pretend otherwise.

### The evidence for this rule

VellumFE is **one crate** (`Cargo.toml:4-5`, `name = "vellum-fe"`; the workspace members
are only the android/ios shims). Because everything is one crate, Rust's module system
cannot enforce its layering — so the author wrote **`tests/architecture.rs`, 359 lines of
string-scanning tests** that grep the source for forbidden substrings:

```rust
let needles = &["arboard::", "open::that", "sysinfo::", "keyring::", ...];
for layer in ["core", "data"] { scan_dir(&src.join(layer), needles, &mut violations); }
```

That is an impressive act of discipline, and it worked — the layering held. But it is a
**workaround for a crate-structure decision**, and it has known gaps: it catches
`arboard::` but not a re-export, `use tui::` but not a type that arrived by another path.

**Cena's first rule follows directly: use a workspace of crates, not one crate with
modules.** Then the dependency graph in `Cargo.toml` *is* the architecture, and a violation
is a compile error rather than a grep miss. We keep an `architecture.rs`-style test only for
the rules the compiler genuinely cannot express.

> This is the clearest "they had to work with what they had" lesson in the project.
> Vellum's enforcement is grep-based because splitting a 310k-line client into crates
> mid-flight is enormous work. Cena has no such excuse on day one.

---

## 1. Crate layout — what goes where

One crate per layer. The dependency arrows below are **the** architecture; `cargo` enforces
them.

```
cena-platform     storage, paths, logging, config primitives   (depends on: nothing)
cena-protocol     bytes -> Frame; commands -> bytes            (cena-platform)
cena-model        typed game state, events, game data          (cena-protocol)
cena-session      one character: state + queue + registry      (cena-model)
cena-script       Lua runtime, scheduler, the three seams      (cena-session)
cena-ui           frontend-agnostic snapshot + input types     (cena-model)
cena-tui          terminal frontend                            (cena-ui, cena-session)
cena-gui          desktop frontend                             (cena-ui, cena-session)
cena-web          web/mobile frontend                          (cena-ui, cena-session)
cena              the binary; wires it together                (everything)
```

### The rules this makes structural (compiler-enforced, not grep)

| Rule | Why it holds |
|---|---|
| Protocol cannot see game meaning | `cena-protocol` does not depend on `cena-model` |
| Model cannot see sessions or scripts | no dependency edge upward |
| Scripts cannot reach the network | `cena-script` has no path to `cena-protocol`'s transport |
| A frontend cannot reach another frontend | `cena-tui` and `cena-gui` do not depend on each other |
| Mobile safety | `cena-platform..cena-script` must build for `aarch64-linux-android` and `aarch64-apple-ios` in CI |

**Rule 1.1 — Dependencies point one way, downward. Always.**
No cycles, no "temporary" upward edge. If you need something from above, you need an
event, a callback, or a trait defined below and implemented above.
*Enforced by:* `cargo` (a cycle will not compile).

**Rule 1.2 — A new crate needs a stated reason and a place in the graph.**
Adding a crate is an architectural act. Record it in this document.
*Enforced by:* review.

**Rule 1.3 — `cena-ui` holds the input vocabulary; no frontend crate re-exports one.**
Key/mouse/resize types are Cena's own, never ratatui's or egui's or the browser's. This is
the discipline that let Vellum's three loop models share one keybinding system
(`src/frontend/events.rs:11-25` — `FrontendEvent` types come from `crate::data::input`,
deliberately not from crossterm).
*Enforced by:* `cena-ui` does not depend on any UI toolkit; architecture test bans toolkit
paths in `cena-ui`.

---

## 2. The layer contracts — what may not cross

**Rule 2.1 — Nothing above `cena-protocol` ever sees a raw byte or an unparsed string.**
The `Frame` is the vocabulary boundary. Parse first — this is decision §0.1 of the
architecture, and the interrogation confirmed it against Lich's own evidence.
*Enforced by:* `cena-protocol` exposes no `String`-of-wire-text type in its public API;
architecture test.

**Rule 2.2 — `Frame::Unknown` is mandatory and must survive to the UI.**
Simutronics changes the protocol without notice. An unmodelled tag reaches the user as text
and reaches a log for diagnosis. It never panics and is never silently dropped.

> This answers the one real advantage Lich's text-passthrough has: a script author can
> handle a game change same-day without waiting for a release. Cena must not lose that.

*Enforced by:* parser tests assert unknown input round-trips rather than erroring.

**Rule 2.3 — The read path cannot write.**
A type that exposes game state must not own a command sink. In Rust this is structural: the
read API hands out `&GameState` (or a snapshot), and `&` cannot send.

> **This corrects an earlier draft of the plan.** We had adopted eohunter's `World` facade
> shape wholesale. The interrogation found that `World`'s *shape* — 156 delegating methods,
> `world.rb:363-434` being a run of one-liners whose entire body is a bare constant
> (`def xmldata = ::XMLData`) — is a **Ruby-mocking artifact**: in Ruby you cannot inject a
> fake for a global constant, so the author wrapped each one to create a spec seam. Rust
> gives that for free via borrows and traits. Adopt the *discipline* (read-only, no
> parsing, per-session instance); reject the *delegation wall*, which in eohunter grew
> 596→982 lines in a week.

*Enforced by:* types + architecture test (the read module may not import the command sink).

**Rule 2.4 — Scripts never receive raw text; they register patterns and receive events.**
With ~2,395 crit patterns and 1,082 combat-def regexes
([`../inventory/09-hot-path-measurements.md`](../inventory/09-hot-path-measurements.md)),
crossing the FFI boundary per pattern per line would be fatal. Matching happens Rust-side
against a compiled multi-pattern set.
*Enforced by:* the Lua API exposes no raw-line accessor except one explicitly documented,
discouraged escape hatch.

**Rule 2.5 — Sessions exchange snapshots, never handles.**
No session may hold a reference into another session's interior. Cross-session
communication is message-passing over a channel, or a read-only snapshot. This is the rule
Lich's active-sessions service encodes (`registry.rb:63`, `:73`) minus its TCP transport.
*Enforced by:* the session registry hands out `SessionSnapshot` (owned, `Clone`), never
`&Session` or `Arc<Mutex<Session>>`.

---

## 3. Naming and findability

**Rule 3.1 — A file's path predicts its contents.**
`cena-model/src/gemstone/psm.rs` holds GemStone PSMs. Not "utils", not "helpers", not
"common", not "misc". Those names are where architecture goes to die.

**Rule 3.2 — Name after the domain, not the pattern.**
`RoundTime`, `CritTable`, `SendLadder` — not `TimeManager`, `TableHandler`, `CommandUtil`.
The game has a rich, precise vocabulary; use it. A reader who knows GemStone should
recognize the type names.

**Rule 3.3 — One concept, one name, everywhere.**
The same idea does not become `session` here, `character` there, and `account` in a third
place. When we pick a word, it goes in the glossary (§8) and nothing else uses it.

**Rule 3.4 — Game-specific code lives under a game namespace.**
`cena-model/src/gemstone/`, `cena-model/src/dragonrealms/`, shared code above them. Nothing
game-specific leaks into shared modules with an `if game == ...` branch; that is what the
`GameAdapter` trait is for.
*Enforced by:* architecture test bans game-name identifiers outside the game modules.

**Rule 3.5 — Public API names are for script authors, not for us.**
The Lua surface is a product. `world.health`, not `world.hp_current_value`. Optimize for
the person writing a hunting script at 1am.

---

## 4. Module size and shape

**Rule 4.1 — Per-file line caps, from day one (C17).**
This is a *first-class* rule alongside crate boundaries, not an afterthought. All the
dependency rules in §1 come free from the crate workspace — and **they are not sufficient**.
No crate graph can express *"this file holds only the struct and its dispatcher."* Only a
cap can.

Two disciplines that matter more than the number:
- **"Move code down, don't raise the cap."** Goes in `CLAUDE.md` as a conduct rule, and the
  architecture test **fails on a cap *increase* in the diff**, not just on a violation.
  Vellum's caps only ever went *down*.
- **Do not copy Vellum's numbers** (trap 9). 2100/3600/800 are specific to Vellum's files in
  August 2026. Set Cena's empirically. An earlier draft of this document proposed "~1,500
  lines for session state" — calibrated off a *single-session* Rust struct where each
  doc-commented field costs ~3.6 lines. A 3+ session struct blows that on field declarations
  alone.
- **Do not reflexively cap a dispatcher.** Vellum's `commands.rs` is 5,132 lines and
  deliberately uncapped; a flat match arm table is not a monolith.

*Enforced by:* architecture test, per-file cap, allowlist requiring a justifying comment,
plus a diff check on cap increases.

**Rule 4.2 — A module that everything depends on is a smell; a module that depends on
everything is a bug.**

**Correction (trap 8): do not cite "Vellum's 26,825-line AppCore" as evidence of failure —
I did, and it inverts the story.** That figure is a **directory total across 29 files**, not
a file, and 4,000+ of it is test code the author deliberately relocated into the tree
(commit `6381d0cf`). The enforcement is *why* it is spread across 29 files. Anyone repeating
this analysis from directory line counts reaches the opposite of the correct conclusion.

**Rule 4.3 — Every shared value has exactly one owning field, tested (C18).**
Vellum's `server_time_offset_has_a_single_owning_field` caught a duplicate field that
**nothing assigned for ten months**, inflating 49.8% of 6,373 measured countdowns by ≥2
seconds. That is a silent, data-corrupting bug that only a structural test finds.

Cena's equivalents to pin from the start: **server clock offset, roundtime, current-room id,
active-session handle.**
*Enforced by:* one architecture test per owned value.

**Rule 4.4 — Facade modules stay facades.**
A `mod.rs`/`lib.rs` re-exports and wires; it does not implement. Vellum enforces exactly
this (`config_root_stays_a_facade`, `split_parents_stay_facades`).
*Enforced by:* architecture test — facade files have a low line cap and may not define
behavior.

---

## 5. State ownership

**Rule 5.1 — Every piece of state has exactly one owner, and the owner is named.**
Vellum enforces this for at least one field
(`server_time_offset_has_a_single_owning_field`) — a rule born from a bug where two places
tracked the same thing.

**Rule 5.2 — No process globals. None.**
This is the constraint that forced Lich into one-OS-process-per-character: its entire
character model lives in process-global mutable state — `GameObj`'s `@@loot`, `@@npcs`,
`@@pcs`, `@@right_hand`, `@@left_hand` (`lib/common/gameobj.rb:25-42`) *are* the character's
hands; `XMLData` is a process-wide singleton referenced 575 times. **Two characters cannot
coexist in one Ruby process**, so Lich runs two processes. Multi-session is our headline
feature; a single global would cost it.
*Enforced by:* architecture test bans `static mut`, and `lazy_static`/`OnceLock` of mutable
game state. (Immutable config and interned tables are fine.)

**Rule 5.3 — Typed state by default; open vocabulary only where the game's vocabulary is
genuinely open.**
The interrogation found Infomon's string-keyed KV store (`'stat.strength'`) was **not** a
database requirement — it was a *script-to-library migration* (PR #325, 2023-03-12,
absorbing the community `infomon.lic` and inheriting its `$infomon` hash surface). The KV
shape is inherited, not designed.

So: typed fields for the ~120 closed-vocabulary values (the game will not invent an
eleventh stat without a client release), and an open map only for the small set that is
genuinely open-ended.

**But keep what the open vocabulary bought:** community scripts can read state Lich's
authors never anticipated. Cena preserves this with a documented extension mechanism, not by
making everything stringly-typed.

**Rule 5.4 — Persistence is a deliberate act, not a side effect.**
Session state is in memory. Only explicitly persistent things touch disk, and each says why.

---

## 6. Concurrency and the script boundary

**Rule 6.1 — One Lua VM per session, on its own thread.**
Isolation, and one session's GC does not stall another.

**Rule 6.2 — Every blocking script call is a yield point, never a thread block.**
64.5% of the corpus calls `pause`/`sleep`. These become coroutine yields.
*Enforced by:* the Lua API exposes no blocking primitive; review.

**Rule 6.3 — Command round-trips are serialized per session.**
A real queue with a `oneshot` reply — not Lich's `issue_command`, which the arbiter
inventory found is *"a serialization primitive that isn't actually serialized"*: two scripts
both install hooks and both see the same lines, kept apart only by luck of different
patterns.

**Rule 6.4 — Every wait takes an interrupt and returns a reason.**
`Confirmed` · `Timeout` · `Interrupted` · `Dead` · `Cancelled` — never a bare bool. This
fixes Lich's inconsistent failure values (`matchtimeout`→`false`, `dothistimeout`→`nil`,
`fput`→`false`-or-symbol).

**Rule 6.5 — Arm before send. Always.**
Subscribe *before* sending the command, or a fast response arrives before you are listening
(`eohunter/actions.rb:295-322`; Lich's `issue_command` independently does the same).
*Enforced by:* the API makes the wrong order impossible — `send_and_await` is one call, and
there is no public "send" + "then wait" pair.

**Rule 6.6 — A runaway script must not wedge a session.**
Interrupt/watchdog on every VM. A `while true do end` kills one script, not one character,
and never all three.

---

## 7. Domain knowledge stays in Rust

**Rule 7.1 — The send ladder lives in Rust, configurable, returning a typed result.**
Roundtime waits, "struggle to stand" → auto-stand → resend, stun/web polling, typeahead
backoff, dead detection, bounded resends (`global_defs.rb:1479-1639`). A decade of knowledge
about how these games actually behave. If we expose a thin `send()` and let Lua handle
roundtime, we have deleted Lich's value and pushed the hard part onto every script author.

**Rule 7.2 — Classify refusals in the parser as a typed enum.**
`Transient` · `Permanent` · `Roundtime` · `Stunned` · `Webbed` · `Dead`. eohunter needed
`PERMANENT_REFUSALS` (`actions.rb:78-92`) because *"You don't seem to be able to move your
legs"* matches the same regex as a genuine transient refusal — a severed leg is not
transient, so the command fired five times and failed. Typing it makes retry policy
trivially correct.

**Rule 7.3 — Capabilities are honest; never fabricate a default.**
Where a game cannot report something, the API says "unknown." A script must distinguish
"you have 0 mana" from "this game does not report mana that way."

**Rule 7.4 — No `$frontend`. Ask capabilities, never identity.**
92 corpus scripts branch on client type to ask "what markup may I emit?" Cena owns both
sides of the wire, so that identity is a *lie* scripts would silently mis-branch on. They
emit intent; the UI decides rendering. Where a script must know, it asks
`ui.supports("inline_image")`.

---

## 8. Glossary — one word per concept

Binding on code, docs and the Lua API. Add to it rather than inventing a synonym.

| Term | Means | Not |
|---|---|---|
| **Frame** | one typed thing the game told us | message, packet, event |
| **Event** | a derived, momentary fact scripts subscribe to | signal, trigger, hook |
| **Session** | one logged-in character | connection, account, client |
| **Script** | one Lua program in a session | task, job, macro |
| **Snapshot** | an owned, point-in-time copy of state | view, handle, ref |
| **Ladder** | the resend/recovery protocol around one command | retry, loop |
| **Adapter** | the per-game implementation (`GameAdapter`) | driver, backend |

---

## 9. Testing rules

**Rule 9.1 — The parser is tested against recorded transcripts, not synthetic strings.**
Vellum's parser is ~7,857 lines of which **4,393 are tests** — and that test suite is the
single most valuable thing we inherit.

**Rule 9.2 — Layers below the UI are testable with no UI and no network.**
This is *why* the layer order is what it is. If a test needs a terminal or a socket to
check game logic, the layering is wrong.

**Rule 9.3 — Every architecture rule with an "Enforced by: architecture test" tag has that
test written when the rule is adopted** — not later. Vellum's caps arrived after the files
were already large; that is the failure mode to avoid.

**Rule 9.4 — Mobile targets build in CI from the first commit.**
Not "we'll port it later." A dependency that breaks the Android/iOS build fails the build.

---

## 9a. Applying KISS to this plan — a standing audit

§−1 is worthless if it only governs future code and not the plan that already exists. This
list is the honest application of it, to be revisited before each phase starts.

**Probably over-engineered — build the concrete case first:**

| Item | Why it is suspect | What to do instead |
|---|---|---|
| `GameAdapter` trait (`01` §4) | designed before *any* parsing exists; DR is Phase 8, GS is Phase 1 | write GemStone concretely; extract the trait when DragonRealms gives a second case (rule 0.2) |
| Agent capability catalogue (`07` §3.3) | generated from a source of truth that does not exist yet | build after behaviors exist; keep them *describable* meanwhile (rule 5.1 in `07`) |
| Four seams (`01` §3, C11) | the fourth was added from eohunter's experience, not ours | keep the *name* as a place to put scoped state; do not build machinery for it yet |
| Cross-session messaging contract (`01` Seam C) | designed now, implemented "when a second session needs it" | correct as stated — but resist implementing early |
| Nine test categories (`06` §1) | five are marked load-bearing; four are supporting | start with the five; add the others when a real gap shows |

**Earned — keep, per rule 0.6:** the crate workspace, typed frames, the multi-pattern
matcher, the send ladder in Rust, session-as-actor.

**The phase order is itself a KISS device.** Phases 1–4 produce a working client with no
behaviors, no agent interface, and no DragonRealms. That is the simplest thing that works, and
everything after it is an addition to something real rather than a prediction about something
imagined.

**One tension to hold honestly.** Rule 0.1 says build the simplest thing; §0 says start with
mechanical enforcement from the first commit. These pull against each other, and the
resolution is: **enforcement of boundaries is cheap and ratchets** (C16 — Vellum's caps were
never raised), while **abstraction is expensive and calcifies** (trap 10 — the `Frontend`
trait). Enforce structure early; abstract late.

---

## 10. Amending these rules

These rules are a draft to be corrected, not defended. When one is wrong, change it here
with the reason — do not accumulate exceptions in the code.

**Every exception carries a comment naming the rule and why this case is different.** An
undocumented violation is a bug; a documented one is a decision.

Pending inputs that may amend this document:
- The `dig:globals` and `dig:appcore` interrogations (in flight) — especially whether
  Vellum's mechanical enforcement *worked*, which §0 and §4 rest on.
- The Lua dialect decision ([`03-lua-dialect.md`](../research/03-lua-dialect.md)) — §6 assumes
  per-session VMs with interrupt support.
