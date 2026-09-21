# Milestone 4 — a real frontend, web first

> **STATUS: a plan, not a record** — `plan/20` is what a finished milestone's
> document looks like; this is the other kind, and it will be wrong in places
> that only building will find.
>
> **One piece is built**: `SessionId` (§D1a), shipped 2026-09-21 in `7707107`
> because the wire format needs it and retrofitting an id is the expensive
> version. Everything else here is unbuilt.
>
> Written 2026-09-21, because an outside team offered to implement M4 and asked
> six questions — and **five of the six had never been decided.** Answering them
> in a chat reply would have made the decisions real without making them
> findable. This file is where they go.
>
> **Decisions settled since**, each in place below rather than appended: the
> server crate is **axum** (§D1b, with the measured dependency diff), and §D2
> now carries the argument for the crate boundary rather than just asserting it.

---

## 0. Why this document exists now

`plan/12` §8's table says M4 is *"a real frontend (web first — it is also
mobile)"*. That one line was the entire specification until today.

It was enough while M2 and M3 were the work. It stopped being enough the moment
someone outside this repo proposed to build against it, because the questions
they asked — what does the frontend connect to, who owns which crate, what does
the first slice contain — are answerable only from decisions scattered across
`plan/12`, `CLAUDE.md`, and one disabled CI job.

**The cost of not having written this down was measured in the asking.** The
team's proposal included *"optional Lua remains supported in the design"*, and
`CLAUDE.md` — under a heading reading **do not reopen** — said *"No Lua, no
Luau, no Rhai, no DSL."* That was wrong. Lua is **deferred, not reversed**
(author, 2026-09-21: *"Lua is on the table, just not now."*), and the file has
been corrected. A contributor reading it would have been told a live question
was closed.

---

## 1. What is already decided, and where it is written

These are not M4 decisions. They are existing constraints M4 inherits, gathered
here so nobody has to find them again.

| Constraint | Source | What it forbids |
|---|---|---|
| **One binary** | `12` §1a, marked **Binding** | *"no design that assumes a second cooperating process"* |
| **Parse first** | `CLAUDE.md`, `12` §3 | a frontend that sees raw bytes or unparsed text |
| **One parser, N classifiers** | `12` §3a | a frontend that re-tokenizes markup |
| **Dependencies point down** | `05` §1, `12` §2 | `cena-ui` depending on `cena-session` |
| **No scripting runtime *yet*** | `CLAUDE.md`, corrected 2026-09-21 | designing *around* one now — not proposing one later |
| **Desktop-first** | `12` §1a | treating mobile UX as M4 scope |

### 1a. The one-binary rule, stated precisely, because it decides the transport

The temptation is to read "one binary" as forbidding a web server. It does not.
The rule's own words are *"no design that assumes a second **cooperating
process**"*, and its reason is mechanical (`12` §1a):

> Mobile OSes suspend background processes, so Lich's proxy-process-plus-
> frontend-process architecture cannot work there. **One process survives
> backgrounding; two cooperating ones do not.**

So the test is not "is there a socket" but **"does game state or session
lifetime live in a second process?"**

| Shape | Allowed | Why |
|---|---|---|
| HTTP/WebSocket server **inside** the `cena` binary, browser connects | **yes** | the browser is a *viewer*; suspend it and the session survives |
| A separate server process the binary talks to | **no** | two cooperating processes — the exact Lich shape §1a rejects |
| Browser holding authoritative state the binary lacks | **no** | state would not survive the viewer closing |

**This settles the question the Despana team asked.** A local authenticated
WebSocket serving assets and state from the Cena binary is consistent with §1a.
A separate server is not.

---

## 2. The decisions this document makes

Each is a real choice with an alternative that was considered.

### D1. The frontend is a server inside the binary, and the browser is a viewer

Per §1a. The binary gains an optional HTTP + WebSocket listener, bound to
loopback by default.

**What the server is for — three jobs with different lifetimes**, worth
separating because only two of them are per-session:

| Job | Per session? | Shape |
|---|---|---|
| Serve the frontend's assets (HTML/JS/CSS) | **no** — the same bytes for everyone | plain HTTP GET |
| Carry state to the browser | **yes** | snapshot on connect, then a delta stream |
| Carry commands back | **yes** | into the existing command queue |

Nothing else. It is **not** a remote-control API, not a plugin host, and not a
second way into the game that bypasses the command queue.

A command from the browser is `Origin::Manual` (`command/verdict.rs:39`) and
takes the identical path to one typed locally — which, note, means it
**deliberately does not touch the authority token**:

> *"Manual input is not a claimant (`plan/12` §4.1, CORRECTED 2026-09-18). It
> jumps the queue and it never touches the authority token. An earlier draft
> made it priority 1 and preemptive, which meant typing `say hi` mid-hunt would
> abort Hunt."*

So typing into the browser mid-behavior behaves exactly as typing into a
terminal mid-behavior does, and neither cancels the behavior. This is a case
where "the same path" is load-bearing: a frontend that invented its own
submission route would have to re-derive that rule and would probably get it
wrong in the direction the 2026-09-18 correction already rejected.

**Alternative considered:** Tauri, which `cena-gui-is-primary` records as not
ruled out. Deferred rather than rejected — Tauri is a *packaging* decision that
can wrap the same served UI later, and making it now would couple M4 to a
toolchain before there is anything to package.

### D1a. One listener on one port, with the session as a parameter

**Not a server per session, and not a port per session.** The question was asked
directly (author, 2026-09-21: *"each session gets its own server or its own
port?"*) and it has three answers pointing the same way.

**§1a forbids the strongest form.** A server per session that outlived the
binary would be a second cooperating process. Even inside one process, N
listeners is N things to bind, authenticate and tear down on a crash.

**Ports are a shared, hostile namespace.** With `12`'s 3–25 characters that is
up to 25 allocations, a user who must know which port is which character, and a
startup failure whenever one is already taken — a failure with nothing to do
with the game.

**The session layer already answers it.** `SupervisedSession::subscribe()`
(`supervisor.rs:225`) returns `(Snapshot, broadcast::Receiver<Event>)` —
snapshot-then-stream, per session, over a `tokio::broadcast` channel that
**already supports many independent subscribers**. One server calling
`subscribe()` once per session is what that API is shaped for; N servers would
each hold a private copy of something built to be shared.

> That `subscribe` already returns snapshot-then-stream is not a coincidence —
> it is the same shape D3 arrived at independently, which is evidence the seam
> is where it belongs rather than where this plan wants it.

So: **one listener, one port**, and a session id selects which session a
connection or a message concerns.

#### The problem this surfaced: there was no session id — **now DONE**

MEASURED when this was written: `grep -rn "SessionId\|session_id"
crates/cena-session/src/` returned **nothing**. `SupervisedSession` had no
identity, because `main.rs` builds exactly one and never needs to name it.

That was correct for a single-session binary and **blocked M4's wire format**,
since every message must say which character it is about.

> **BUILT 2026-09-21** (commit `7707107`), on the author's call — the first
> piece of M4 to exist. `SessionId` is a newtype beside `Generation` in
> `lifecycle.rs`, seeded at 0 and counted up so criterion 7's replay produces
> the same ids every run. It is on `SessionCore` (the part that outlives
> connections), on `Snapshot`, on `SupervisedEnd`, and exported from
> `cena-session`.
>
> **`SupervisedEnd` gained it for a reason worth recording.** The test asserting
> the id survives a reconnect first checked a *copy* taken before the run, which
> proves nothing — `run` consumes the session, so there was no way to read the
> real id afterwards, and the test's name was a claim nothing checked. A
> mutation now confirms the field is load-bearing: breaking only the ending's id
> fails exactly that one test.

The reasoning, kept because it is the argument for doing this kind of thing
early: it is a newtype and a field, and the cost of deferring is a wire-format
migration in the very next milestone — the change §3's "session-aware from the
start" exists to avoid. `Generation` was kept a milestone early on the same
argument (`lifecycle.rs:94-102`).

### D1b. The server crate is **axum**, and the lock file decided it

Picked 2026-09-21 on the author's instruction, against the criteria below and
with a measured dependency diff rather than a preference.

What is already fixed, and what it rules out:

| Constraint | Evidence | Consequence |
|---|---|---|
| **tokio is the runtime** | `cena-session/Cargo.toml:28`, `cena-platform:18` | a server on a different executor would run two runtimes in one process |
| **`native-tls` is the TLS stack** | `cena-platform/Cargo.toml:27`, `CLAUDE.md` "one TLS stack" | anything pulling `rustls` compiles both |
| **One `hyper` major** | `cena-platform/Cargo.toml:57-62` | a server on a different `hyper` major compiles two stacks |
| **`cena-web` may depend on `cena-ui` and `cena-session` only** | `layering.rs:70-75` | the framework and its transitive tree live entirely in `cena-web` |

Those four together point hard at a tokio-native, hyper-1.x server, and the
lock file turns that from a preference into a measurement.

#### What axum costs, measured

MEASURED 2026-09-21 by resolving `axum = { version = "0.8", features = ["ws"] }`
plus tokio in an empty crate and diffing the crate names against Cena's
`Cargo.lock` (202 crates):

| | |
|---|---|
| **New crates** | **16** |
| **`hyper` version** | **1.11.1 — identical to Cena's** |
| **`rustls` pulled in** | **0 occurrences** |
| **Second async runtime** | **none** |

`hyper`, `tower`, **`tower-http`**, `http`, `http-body`, `bytes`,
`futures-util` and `pin-project-lite` are **already in the lock file**, pulled
in by `reqwest` for the web-login fallback — VERIFIED with `cargo tree -p
cena-platform -i hyper` and `-i tower`, both of which resolve to
`reqwest v0.12.28 -> cena-platform`. Axum is built on exactly those, so most of
its tree is already paid for, and `tower-http` means static-file serving costs
nothing further.

Of the 16 new crates, **7 are the SHA-1 chain** (`sha1`, `digest`,
`block-buffer`, `crypto-common`, `generic-array`, `typenum`, `cpufeatures`)
that the WebSocket handshake requires **by specification** — RFC 6455 hashes
the client key. Any WebSocket implementation pays that. The rest are axum
itself, `axum-core`, `matchit` (routing), `mime`, `httpdate`,
`serde_path_to_error`, and `tokio-tungstenite`/`tungstenite`.

#### Why this satisfies every constraint, including the one with teeth

- **Same `hyper` major**, so no second stack — and critically, **Tauri's door
  stays open.** That was the constraint most likely to be spent by accident.
- **tokio-native.** Axum *is* a tokio/tower server; there is no second runtime
  and no bridging layer.
- **No `rustls`.** The listener is plaintext on loopback (D4 authenticates it),
  so it needs no TLS at all and pulls none — `native-tls` remains the only TLS
  in the workspace.
- **Contained in `cena-web`.** Nothing above needs to know it exists, which is
  what `layering.rs`'s allowlist enforces.

**The honest cost:** 16 crates and a `tower`/`tower-http` idiom to learn. The
alternative — hyper 1.x directly — would save perhaps 6 of those and cost
hand-rolled routing and the entire WebSocket upgrade dance. That is the wrong
trade for a first frontend.

> **If this turns out wrong, the exit is cheap and should stay that way.** Axum
> lives behind `cena-web`'s boundary, and a listener is a small surface. The
> thing that would make it expensive is letting axum's types — `State`,
> extractors, `Router` — leak into the view types in `cena-ui`. **They must
> not**, which is D2's rule and `layering.rs:260`'s allowlist enforcing it.

> **The `hyper` constraint is not hypothetical, and it ties back to D1.**
> `reqwest` is pinned to 0.12 rather than 0.13 because *"`tauri-plugin-http` v2
> pins reqwest 0.12, and mixing majors compiles two clients and two hyper
> stacks"* — a pin taken **to keep Tauri viable**. So a server crate that forces
> a different `hyper` major would not merely bloat the build; it would quietly
> spend the option D1 deferred rather than rejected. Whoever picks should know
> they are being asked about Tauri too.

Two things that would make a choice wrong regardless of merit:

- **A second async runtime.** `plan/12` §5.5's supervision model assumes one.
- **A framework that wants to own `main`.** The binary already owns its
  lifecycle: `main.rs` spawns the supervisor, and §1a's one-binary rule means
  the server is a guest in that process, not its host.

> **This is a real gap in the plan and is named as one**, because the question
> was asked directly (author, 2026-09-21: *"does the plan say what web server
> crate to use and all that stuff?"*) and the honest answer was no. It now says
> what would make an answer right, which is the part a document can supply.

### D2. `cena-ui` stays frontend-agnostic; the web frontend is a new crate

`plan/12` §2 already fixes this:

```
cena-ui          frontend-agnostic snapshot + input types     (cena-model)
cena-tui/gui/web frontends                                    (cena-ui, cena-session)
```

`cena-ui` may depend on **`cena-model` only**. It holds the view types and the
input vocabulary — what a frontend needs to *render* and *submit*, with no
transport, no HTML and no framework. The web frontend is `cena-web`, depending
on `cena-ui` and `cena-session`.

**Why not put it all in `cena-web`:** because the second frontend is what proves
`cena-ui` earned its existence, and `12` §8 puts TUI/GUI at M10. Until then
`cena-ui` is a trait with one implementor in all but name — so **it must stay
small**, and the rule of three (`05` §−1) applies to anything added to it.
`cena-ui` is one line today; it should gain types M4 actually needs and nothing
speculative.

#### Vellum reached this same split, and that is the argument

Not theory. MEASURED in `reference/VellumFE/src/frontend/`:

```
common/   color.rs  command_input_model.rs  rect.rs  text_input.rs
gui/      headless/   tui/   web/
```

**Four frontends and a shared `common`** — and `common` holds precisely what
`plan/12` §2 specifies for `cena-ui`: an input model, a text-input widget, and
the colour and rectangle types. The same author, solving the same problem,
converged on the same seam.

Two things Cena takes from that, and one it does not:

- **The seam is real**, not a speculative abstraction. It was found by need in
  a shipping client, which is the rule-of-three evidence `05` §−1 asks for
  before abstracting.
- **The sizes justify the boundary.** MEASURED: `frontend/tui` is **58,167
  lines** against `frontend/web`'s **5,497**. A frontend is where the bulk
  lands, so anything shared that drifts into one of them is work the next
  frontend repeats.
- **But Vellum's is directories in one crate, and Cena's is the crate graph.**
  `core/state.rs` is **2,831 lines**, and nothing at the language level stops a
  frontend reaching into it. `CLAUDE.md` records the cost of that in general
  terms — *"VellumFE had to retrofit it at ~250K lines"* — and it is why
  `cena-ui`'s single allowed dependency is a **compiler-enforced allowlist**
  (`layering.rs:260`) rather than a convention. A directory boundary asks; a
  crate boundary refuses.

**So the separation is the point, and it is enforced rather than encouraged.**
A `cena-web` that wants something from `cena-model` either goes through
`cena-ui`'s view types or through `cena-session`'s re-exports — and if neither
has it, the answer is to widen the view types deliberately, not to add an edge.

### D3. The wire is a snapshot plus a delta stream, both serde

`cena-model` **already depends on serde** (`crates/cena-model/Cargo.toml:8`) and
the character store established the precedent — `snapshot.rs:26`'s header is
literally *"Why serde and not a database"*.

> **But the view types live in `cena-ui`, which may depend on `cena-model`
> ONLY** (`layering.rs:260`). Adding serde there is a deliberate allowlist edit,
> not a manifest line — and the allowlist covers dev-dependencies too. That edit
> is part of M4 and should be made knowingly, with the reason recorded beside
> the entry the way the existing one is.

- On connect, the frontend receives **one whole snapshot**.
- Thereafter it receives **events**, and `cena-session`'s `Event`
  (`actor.rs:196`) is already close to the right vocabulary: `Frame`, `Combat`,
  `SyncNeeded`, `Sent`, `StateChanged`, `ConnectFailed`.

**`Event` is not shipped to the browser as-is.** It carries `Box<Frame>` and
`Arc<ChunkFacts>` — protocol and model internals whose shape is a private
matter. `cena-ui` defines the *view* types, and the mapping from `Event` to them
is the contract. A frontend coupled to `Frame` would make every parser change a
breaking frontend change, which is precisely the coupling `12` §3a exists to
prevent.

> **`Event` already anticipates a frontend**, and that is evidence the seam is
> in the right place. `Event::ConnectFailed`'s doc says the retry ladder is
> published rather than only logged because *"a log file is not where someone
> watching a client find its way back looks"*, and that `cena-session` does not
> print because **"a library must not own a terminal"**. M4 is the thing that
> was being left room for.

### D4. Authentication is required, on loopback, from the first commit

The listener is authenticated even bound to loopback. Any process on the machine
can reach loopback, and this socket can **send commands to a live game
character**. An unauthenticated local socket is a local privilege escalation
into someone's account.

**Not deferred to "later hardening."** A socket that ships unauthenticated gets
used unauthenticated.

### D5. Login stays where it is for the first slice

The session is started and logged in by the existing mechanism (`plan/10`,
live-verified); the frontend attaches to a running session.

**Reason beyond scope control:** `CLAUDE.md`'s credential rule — *"do not log
into a live game service without the author present"* — means the login path
cannot be exercised by whoever is writing the frontend. Account selection in the
browser is a later slice, and it needs its own thinking about where credentials
live.

---

## 3. The first slice

Deliberately shaped like `12` §7.1's M1 slice: a list of what is in, a list of
what is out, and demonstrable at the end.

| In | Out |
|---|---|
| Story output (text, styled by the runs the parser already produces) | full markup fidelity, themes |
| Command input, through the existing queue | macros, history, aliases |
| Room: title, description, exits, contents | the map, routing, travel |
| Hands, vitals, roundtime | stats, skills, PSMs, inventory panels |
| Connection status, including the retry ladder | account selection, login UI |
| One session | multi-session switching (**M5**) |
| Reconnect handled correctly | — |

**Reconnect is in the first slice and is not new work.** It exists and is
live-verified: the backoff ladder, the two stop conditions, and §5.2's rule that
invalidated facts become `Unknown` rather than stale. The frontend's job is to
*render* that correctly — an `Unknown` vital must not display as zero — and to
consume `ConnectFailed`. Rebuilding any of it would be a regression.

**Session-aware from the start, multi-session deferred.** Every message carries
a session id even while there is exactly one. Retrofitting an id into a wire
format is the kind of change that touches every message, and M5 is the *next*
milestone.

---

## 4. What must not be assumed

- **No scripting runtime.** Nothing in M4 may assume Lua exists, and no
  abstraction may be added whose only purpose is to host one. This is
  sequencing, not prohibition — see `CLAUDE.md`'s corrected entry. `12` §9d
  already refuses a `GameAdapter` built for a deferred DragonRealms on the same
  grounds, and the reasoning transfers exactly.
- **No Vellum wire compatibility**, and no import of its saved settings.
  `plan/13` says why this is a new codebase rather than a fork.
- **No second process.** §1a.
- **No `GameState` on the wire.** D3.

---

## 4a. What is missing, measured

A survey of the frontend-facing surface (2026-09-21) found six gaps. None is
large; all are load-bearing, and naming them here is cheaper than finding them
at implementation time.

| # | Gap | Evidence |
|---|---|---|
| 1 | **No serializable view of `GameState`.** It has no serde derives and holds `Instant`, `Runs` and ten private fields | `state.rs:110`, `equality.rs:9-13` |
| 2 | **`cena-ui` cannot depend on serde** without editing the allowlist | `layering.rs:260` — `CENA_UI_MAY_DEPEND_ON = ["cena-model"]` |
| 3 | **Line assembly lives in the binary**, not a shared crate | `run.rs:506-566` |
| 4 | **`SupervisedSession::subscribe` hardcodes `lifecycle: State::Connecting`** | `supervisor.rs:230` |
| 5 | **No input vocabulary**, though Rule 1.3 specifies one | `05` §1.3 (`:258-264`) |
| 6 | **`Event` has no serde** and carries `Box<Frame>`/`Arc<ChunkFacts>` | `actor.rs:195-247` |

**Gap 4 is a bug, not just a gap.** A frontend attaching to a running session is
handed a snapshot claiming the session is `Connecting`. Today nothing subscribes
mid-session so nothing notices; M4's first act is exactly that subscribe. Fix it
when M4 starts, with a test that a mid-session subscriber sees `Ready`.

**Gap 3 deserves a decision rather than a default.** `run.rs` reimplements
"a frame boundary is not a line boundary" — the same rule `GameState::pending`
(`state.rs:230`) exists for. A second frontend makes that a third copy. Rule −1's
rule of three says the moment to lift it into `cena-ui` is when `cena-web`
needs it, which is M4.

### The arch tests will stop a wrong crate immediately, and that is useful

`ALLOWED_EDGES` (`layering.rs:76-162`) is asserted as a **set equality**, not a
subset — a workspace member with no row fails at once. `cena-web`'s row is
already written down, commented out, at `layering.rs:70-75`:

```text
cena-tui/gui/web     (cena-ui, cena-session)   -- and never each other
```

So **`cena-web` gets exactly `["cena-ui", "cena-session"]`**. Naming
`cena-model`, `cena-protocol` or `cena-platform` directly goes red — and does
not need to, because `cena-session` already re-exports `GameState`, `Room`,
`UnknownTag`, `Frame`, `CharacterSnapshot` and `Group` (`lib.rs:36-58`) for
precisely this purpose. **That re-export set is what a frontend may name.**

Two further constraints that will bite:

- The `cena` binary's row (`layering.rs:157-160`) is *also* a set equality, so
  adding `cena-web` to the binary means editing it in the same commit.
- Edges are read from `cargo tree --target all`, **including dev-dependencies**
  (`layering.rs:186-190`). A `serde_json` dev-dependency in `cena-ui` would go
  red.

---

## 5. The open question this plan does not settle

**Does a web server dependency foreclose mobile?**

`12` §1a marks the mobile compile check **Binding**. It is not enforced: the
`aarch64-linux-android` job in `.github/workflows/ci.yml:60-105` is **commented
out**, with a long and honest note recording that the stated OpenSSL blocker is
stale, that VellumFE ships the answer, and that nobody has run the two lines
that would prove it.

That is `plan/05` §0 in its purest form — *a rule that is not enforced is a
wish* — and the note says so itself.

**A first draft of this section overstated the risk**, and the correction
matters. §1a's binding set is `cena-platform..cena-behavior`, and the same table
lists **"mobile frontends"** under *Not binding*. So a web framework in
`cena-web` **cannot** foreclose mobile by itself — `cena-web` is not in the set,
and a frontend is expected to differ per platform.

What remains true, and is the actual risk:

- The gate protecting the crates that *are* binding **is off**. M4 is the first
  milestone that adds a crate near that boundary, so it is a natural moment to
  turn it on — not because `cena-web` needs it, but because nothing has checked
  since `cena-platform` gained the vendored-TLS target block.
- If the server needs anything from `cena-session` or below — a runtime feature,
  a TLS stack, an async executor assumption — **that** lands in the binding set,
  and the gate is what would catch it.

So the action is smaller than the first draft claimed: **run the two lines the
CI comment proposes, and turn the job on if they pass.** If they fail, record it
and downgrade `12` §1a's "Binding" honestly rather than leaving a wish. Neither
blocks drafting the frontend contract.

---

## 6. Build order

0. ~~**`SessionId`**~~ — **DONE 2026-09-21**, `7707107`. See §D1a.
1. **Fix gap 4** (`supervisor.rs`'s hardcoded `Connecting`) and **turn the
   mobile CI job on or record why not** (§5). Both are small, both are
   pre-existing, and both are cheaper before a frontend depends on them.
2. **This document, reviewed.** The server crate is chosen (§D1b: **axum**);
   the contract is drafted *from* this document, not before it.
3. **`cena-ui`'s view types** — snapshot and input vocabulary, derived from what
   §3's slice actually renders. Small, and justified field by field. Needs the
   `CENA_UI_MAY_DEPEND_ON` edit (§4a gap 2), made knowingly.
4. **The `Event` → view mapping**, with tests that a frontend never sees a
   `Frame`.
5. **`cena-web`**: listener, auth, snapshot-on-connect, delta stream.
6. **The frontend itself.**
7. **A replay test**: a recorded session, played through the mapping, producing
   a deterministic sequence of view updates. `12` §7.2's criterion 7 already
   requires deterministic replay; this extends it to the frontend seam.

Steps 2–4 are offline and testable without a live login. Step 5 needs a running
session, which means the author.

---

## 7. For whoever implements this

Read in this order: `plan/12` §1a (one binary), §2 (crate layout), §3a (one
parser), `plan/05` §1 (dependency direction) and §−1 (KISS/DRY), then
`CLAUDE.md`'s settled decisions.

Three things that will not be obvious:

- **`Option` means unknown, not zero** (`12` §5.2). A vital that has never been
  reported is not full and not empty. Rendering `None` as `0` invents a fact,
  and after a reconnect that is the difference between "we do not know your
  health" and "you are dying".
- **The parser preserves everything on purpose** (Rule 2.2/2.2a). If a frontend
  needs something the view types lack, the fix is to widen the view types, not
  to re-parse text.
- **Line caps are enforced** (`05` §4.1, `cena-arch-tests`). The default is 800
  lines. *Move code down; do not raise the cap.*
