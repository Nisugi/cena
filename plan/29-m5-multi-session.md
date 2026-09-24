# 29 — Milestone 5: multi-session

**Status: DRAFT (Claude, 2026-09-23), with the author's first answers recorded in §5.**
What the author decided is marked as theirs; everything else is still a proposal. `plan/12`
wins any contradiction.

`12` §8 gives M5 one line: *"multi-session: N characters, isolation tests, switching"*. This
document turns that line into a slice that can be built and demonstrated, the way `12` §7.1
did for M1 and `23` §3 did for M4.

---

## 0. What is already settled, and by whom

| Fact | Source |
|---|---|
| **3–25 characters in one process**, session-as-actor | `CLAUDE.md`, Settled decisions |
| **One binary**; no second cooperating process | `12` §1a |
| **Session failure isolation is the invariant**: any one session can fail, wedge or panic without affecting the other 24 | `12` §5.5 |
| **M5 comes before M6** because multi-session constrains behavior design | `12` §8 |
| **Headless is a first-class run mode**, which shapes the credential ladder | author, 2026-09-18 |
| **The GUI is primary**, built at M10; the architecture stays frontend-agnostic | author, 2026-09-18; `28` |
| **One web listener on one port**, with the session as a parameter on every message | `23` §D1a |
| **One character per account online at a time, across all instances** (Prime, Platinum, Shattered, Test) | author, 2026-09-23 (§5 Q1) |
| **Sessions are added and removed while Hydra runs** | author, 2026-09-23 (§5 Q4) |
| **The intended entry point is a GUI launcher**; the command line is supported too | author, 2026-09-23 (§5 Q5) |

---

## 1. What already exists

M5 is mostly assembly: the per-session parts were built one-per-process-safe from M1 on,
because `12` §7.1 asked for "no global state" before there was a second session to share it.

| Built | Where |
|---|---|
| One actor per connection, one supervisor per character, each its own task | `crates/cena-session/src/supervisor.rs`, `actor.rs` |
| `SessionId` on every observed event, snapshot and web message | `crates/cena-session/src/lifecycle.rs`, `23` §D1a |
| Per-session parser, state, queue, authority token, generation | `SessionActor`'s fields |
| Stores keyed by instance and character: character, settings, travel, menu | `crates/cena-session/src/character_store.rs` and siblings |
| One log, one combat database and one player log per character | `crates/cena/src/main.rs`, `open_log` |
| Process-wide `static`s are an allowlist, and every entry is a read-only data table or a write lock | `crates/cena-arch-tests/tests/architecture.rs` `ALLOWED_STATICS` |
| **A session never depends on a viewer.** Any number of frontends can attach to one session, or none, and closing one does not close the game | `SessionObserver`, SE-6 (`19`); `23` |

The last row is what makes §5a cheap: at the session layer every session is already
"headless", and a frontend is something that attaches to one.

## 2. What is single-session today

MEASURED 2026-09-23 by reading the code. Each row is a real gap, not a guess.

| Gap | Where | Why it matters |
|---|---|---|
| Every session is `SessionId::FIRST` | `crates/cena-session/src/supervisor.rs:170`, `crates/cena-session/src/actor/handle.rs:196`, `crates/cena-session/src/observation.rs:176` | Two sessions would be indistinguishable on the wire; D1a's parameter would carry nothing |
| The binary builds exactly one session, once | `crates/cena/src/main.rs` (`open_session`, one `SupervisedSession::new`) | The whole of `main` is one character's lifetime; nothing can add a second while it runs |
| Credentials are one stdin prompt, **and it echoes the password** | `crates/cena/src/main.rs:46` | N characters means N prompts, and each one puts a password in scrollback |
| The web server binds to one observer and one handle | `crates/cena-web/src/server.rs:80` (`WebServer::bind`) | D1a's one listener needs a table of sessions behind it |
| One Ctrl-C handler ends the one session | `crates/cena/src/interrupt.rs` | Stopping one character must not stop the others; stopping Hydra must stop all of them in order |
| The terminal prints one session's lines, unprefixed | `crates/cena/src/run.rs` (`watch_events`) | With N sessions, a line must say whose it is |
| The demo, probe and capture scripts assume one character | `crates/cena/src/run.rs` | They are M1 criteria tools; they can stay single-session |

---

## 3. The slice (proposed)

| In | Out |
|---|---|
| N sessions in one process, each with its own `SessionId` | performance at 25 (measured in M5, tuned later) |
| **Sessions added and removed while running** (author, Q4) | the GUI launcher and the GUI's window model (M10) |
| Isolation tests: panic, wedge, slow consumer, reconnect and quit of one session leave the others untouched | cross-session features: follow the leader, group moves, shared loot (M6) |
| A web **hub**: lists sessions, adds and removes them, links to each (§5a) | an account or profile editor beyond what the hub needs |
| A URL per session, so one tab per character works (§5a) | per-session terminal windows |
| The full credential ladder (author, Q2) | `zeroize` (still out, `12` §7.1) |
| Orderly shutdown of all sessions on Ctrl-C; each session's own stop leaves the rest running | SE-4, authority across generations (author: M6, Q6) |
| The terminal prefixes each line with the character's name | |
| The password prompt stops echoing | |

**Cross-session features are Out on purpose.** They are the reason multi-session is the
headline, but each one is a behavior, and behaviors are M6. M5's job is to make N sessions
exist, stay isolated, and be observable; M6 builds on that.

**One account, one character** (Q1) means N sessions need N accounts. Two sessions on the
same account are not a test case to build for: the second login ends the first. The session
table should refuse to start a second session on an account that already has one, and say
why, rather than let the game knock the first one off.

### 3a. The isolation tests

`12` §5.5 states the invariant and this is where it gets its tests. Each runs two or more
sessions over scripted sources (`AnsweringSource`), in one process, and asserts the others
are unaffected, measured as the others' command round trips still completing within their
normal deadline.

| Failure in session A | Session B must |
|---|---|
| A's actor panics | keep running; A is reported dead, not the process |
| A's behavior wedges (no progress) | keep its own latency; `BEHAVIOR_WATCHDOG` handles A (`12` §5.5) |
| A's observer is slow and lags | see no lag of its own (bounded channels, no shared ring) |
| A's connection drops and reconnects | keep its generation, state and authority unchanged |
| A quits, or is removed from the table | keep running |
| A's character store write fails | keep saving its own |

Each must go red when isolation is broken on purpose, for example by sharing one event ring
between two sessions, or it tests nothing.

> **BUILT 2026-09-23** as `crates/cena-session/tests/isolation.rs`, five tests. Two rows
> changed on contact. A wedged **behavior** needs `BEHAVIOR_WATCHDOG`, which is not built and
> belongs with behaviors (M6); the session-level wedge -- a game that stops answering -- is
> tested instead. A failing **store write** needs a filesystem fault and moves to step 3,
> where the session table owns the stores. Two mutations were verified: a shared event ring
> turns the lag test red, and a shared generation counter turns the reconnect test red.

---

## 4. Steps (proposed order)

Each step ends in something demonstrable, the way M1's did.

1. **Mint `SessionId`s.** A counter owned by whatever builds sessions, never a global, and
   deterministic (seeded at 0) so replays stay reproducible. Remove the three hardcoded
   `FIRST`s. *Demo:* two sessions in one test process publish distinguishable events.
2. **The isolation tests (§3a)**, before any binary changes, over scripted sources. *Demo:*
   the table above, green, with a documented mutation per row that turns it red.
3. **A session table in the binary that can grow and shrink.** It owns every session's
   lifetime; adding one starts a supervisor, removing one sends its `quit`; Ctrl-C shuts all
   of them down in order. It refuses a second session on an account already in use. *Demo:*
   add two characters on scripted sources, remove one, Ctrl-C, clean farewells.
   > **BUILT 2026-09-23** as the `cena-host` crate (`Host`, `Hosted`, `stop_all`), five
   > tests over scripted sources, each red under a mutation: no account check, a stopped
   > session holding its account, and a stop that skips `quit`. The binary does not use it
   > yet: adding a live session needs credentials per account, so the binary moves onto the
   > table with step 4. A failing store write (moved here from §3a) is still to test.
4. **The credential ladder (Q2).** OS keyring, then a source the user names (env var, file,
   stdin), then refuse loudly naming the account. The prompt stops echoing.
5. **The web hub and per-session URLs (§5a).** *Demo:* two scripted sessions, one hub, one
   tab per character, and switching inside the hub.
6. **Live, author present:** two characters on two accounts at once. This is M5's
   acceptance, as the M1 live run was M1's.

## 5. Answers and open questions

**Q1. How many characters can be online at once?** — **ANSWERED (author, 2026-09-23):**
*"1 character per account can be logged in at a time period. Not 1 per instance (prime,
test, shattered, platinum), one period."* See §3 for what that changes.

**Q2. Where do the login credentials come from?** — **ANSWERED: (a), the full ladder, and
the keyring is wanted** (author, 2026-09-23: *"we absolutely want keyring for account login
information"*). Credentials belong to the **account**, not the character: the keyring entry
is keyed by account, and a character names the account it lives on. Then a source the user
names explicitly (env var, file, stdin), then refuse loudly naming the account.

**Q3. Who owns switching and focus?** — **OPEN; the author wants to discuss.** The options
the author named: a frontend per character; some sessions headless because of
cross-character communication; all headless, with "alt-tab" swapping on one frontend. §5a
is the proposal for M5.

**Q4. Fixed at startup, or added and removed while Hydra runs?** — **ANSWERED: added and
removed while it runs.**

**Q5. Where does the list of characters come from?** — **ANSWERED: all of the above**, and
*"the intended point of entry will be a GUI launcher, not command line. We will support
command line of course."* The launcher is the GUI's (M10); in M5 the web hub stands in for
it, and the command line can name characters to start with.

> **The keyring** is the operating system's own password store: Windows Credential Manager,
> the macOS Keychain, or Secret Service on Linux. VellumFE already uses it
> (`reference/VellumFE/src/config/profiles.rs`), opt-in, keyed by account. The author's
> hesitation was the draft's framing -- it read as per-character -- not the keyring itself.

**Q6. SE-4 in M5?** — **ANSWERED: no, M6.**

**Q7. What does the connect URL show with N sessions?** — **ANSWERED (author):** a hub, *"the
connect url takes you to a hub unless there is a character attached to it"*, possibly behind
its own flag. See §5a.

### 5a. Frontends and sessions (Q3)

#### The requirements (author, 2026-09-23)

**R1. Every character that is actively doing something has its story visible.** Prime
expects a player to be attentive at the keyboard and able to respond within a reasonable
time, or it reads as unattended scripting. So multi-session cannot mean one visible
character and N hidden ones.

**R2. No window shows two characters' story text.** A separate story window per character
is the unit. The author had not considered separate story windows as an option to offer
before, and thinks some players would use it.

**R3. At a glance, per character**, what a player needs to notice: vitals, wounds, missing
spells, debuffs, cooldowns, combat state, whether the bounty or quest is done, thoughts or
whispers directed at them -- *"the list goes on"*.

**R4. Shared streams merge across characters, deduplicated.** Thoughts, speech and the like
go to one thoughts window and one speech window for all characters, each line once. When
that window is not open, a line falls back to **its own character's** story window.

R4's fallback is the game's own rule, per character: every stream declares where its text
goes when its window is closed (`ifClosed` / `styleIfClosed`, `15` §2.6 and
`reference/wiki_clean/Wrayth protocol.txt:73`). Thoughts fall through to the story window in
the thought style. So only the merge is new; the fallback is what each session already
receives.

#### Where it lives

At the session layer this is already answered (§1, last row): a session runs whether or not
anything watches it, and any number of frontends can attach to it. **The merge is a
consumer of N sessions**, so it belongs above them: in `cena-ui`, frontend-agnostic, beside
the one-session projection. It must not live in `cena-session`, where a shared component
between two sessions is exactly what §3a's isolation tests exist to catch.

| The author's option | What it is, on the core that exists |
|---|---|
| a frontend per character | each window attached to one session |
| some sessions headless | a session with no frontend attached; it keeps running |
| all headless, "alt-tab" on one frontend | one view that changes which session it is attached to |

#### Proposal for M5's web frontend

- The pairing URL opens a **hub**: every session, its lifecycle state, add and remove, and a
  compact **status card per character** covering what the model already has for R3.
- Each session has **its own URL**. R1 and R2 are met by opening one browser window per
  character and arranging them side by side; the tiled layout itself is the GUI's (M10).
- A URL that names a character goes straight to that session (Q7).
- The **shared-stream merge (R4)** is built in `cena-ui` and shown in the hub as one
  thoughts panel, so it is tested before the GUI depends on it.

#### The merge, settled (author, 2026-09-23)

- **The same line is a duplicate line.** A thought reaches every listening character
  verbatim, so identical text on the same stream from two sessions is one line. Two
  arrivals are the same occurrence when they land **within 1 second** of each other (author,
  2026-09-23: *"you can give it a 1s max matching window"*); a repeat after that is a new line.
- **Directed variants come later.** A thought, speech or whisper directed at a character
  reads slightly differently to each recipient; accounting for that is deferred. A private
  thought or whisper reaches only one character, so it never needs merging.
- **Whose was it: a character tag at the front of the line**, configurable, so a user who
  does not want the name can choose something else.
- **Own speech stays as two lines.** Nisugi's `say hi` is `You say, "hi"` to Nisugi and
  `Nisugi says, "hi"` to a second character; both are kept.
- **Which streams merge:** thoughts, speech, logons, deaths and announcements. A GM `SEND`
  to one character often shows in the announcements window, so it arrives there with its
  character's tag. **No other stream merges**; more are revisited only if asked for.
- **When: whenever** -- the author set no milestone. The proposal stays M5 for the status
  cards and the merge, because both are frontend-agnostic and testable before the GUI.

## 6. Acceptance (proposed)

- The isolation tests pass, and each has a recorded mutation that turns it red.
- Two real characters on two accounts run at once in one Hydra, with the author present:
  added from the hub, observed and commanded from the browser; one disconnects and
  reconnects while the other is unaffected; one is removed while the other keeps playing;
  one Ctrl-C ends the rest cleanly.
- Fresh CI is green on the merged result.
- `CLAUDE.md`'s "Where the build stands" names M5 accepted, with the live run's evidence.
