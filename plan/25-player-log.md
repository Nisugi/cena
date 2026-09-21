# 25 — The player log

**Status: proposal.** Nothing here is built. Author decisions recorded below are
marked **AUTHOR**; everything else is a proposal awaiting one.

The **second** of the two logs `sink/mod.rs` named in 2026-09-18, and the one it
said would come later:

> **AUTHOR, 2026-09-18:** *"ideally logging will have a few options right. We'll
> have this dev log of the wire, then we would want a user log of the processed
> text. We only need the dev now but I'm sure that matters."*

`cena-platform/src/sink/mod.rs:16-22` records why it could not be built then:
*"The user log cannot live in this crate. Processed text does not exist until
`cena-protocol` has parsed it."* That blocker is now gone — see §2.

---

## 0. What this is, in one line

Readable session history: what a player saw, per character, per day, searchable
years later.

| | wire log (`sink/`) | **this** |
|---|---|---|
| content | raw bytes, both directions | display text, as a player read it |
| audience | us: debugging, cutting fixtures | the author: *"what happened in that hunt"* |
| layer | `cena-platform`, below the parser | `cena-session`, above it |
| lifetime | churn freely | kept; searched years later |

---

## 1. The reference, and what it already answers

Lichborne (`elanthia-online/Lichborne`, BSD-3, DragonRealms, Electron/TS) ships
this feature. `src/main/sessionLog.ts` at `aaeca23f` was read in full on
2026-09-21. It is a more complete system than "a log file with search", and most
of its design questions are answered there rather than needing rediscovery.

**Taken from it, with the reason each earns its place:**

| Decision | Why |
|---|---|
| One file per character per day | A day is the unit a person searches in. Rotation is implicit rather than a size heuristic. |
| Today plain, closed days gzipped | Today's file stays greppable by hand. Their measurement: ~85–90% reduction. |
| Reader accepts two line formats forever | A format change then needs no migration pass over years of archive. |
| Stream-set cache keyed `path:mtime:size` | A closed day is immutable, so its stream list is computed once. |
| Buffered writes: 1s timer, ≥100 records, forced at 5000 | The forced threshold is the flood guard, not the normal path. |
| **Reads flush the buffer first** | Otherwise a search of the live session misses the last second of it. |
| Two independent retention dials | `retention_days` by age; a cap on **uncompressed** bytes only, archives exempt. They answer different questions. |
| **Tallies count raw rows; dedup is a reading view** | Their comment names the pitfall: *DR double-emits speech*. Dedup before counting silently undercounts repeated events. |
| Search caps: 1000 hits, 366-day window | Bounded work per query. |
| Export by spec | The caller sends range/streams/options; the owner does all I/O. |

**The constraint that matters most**, verbatim from their source:

> *"main owns every session's socket, so a synchronous multi-file scan freezes
> EVERY connected character at once."*

That is our actor exactly (`cena-session/src/actor.rs`). Any read that runs on
the session owner's thread stalls **every** character in the process — and
multi-session is the headline feature, so the cost here is higher than theirs.

### 1a. The one thing not to take

Having got the read path right, their write path is:

```typescript
fs.appendFileSync(file, lines.join(''), 'utf8')
} catch (err) {
  console.error('[sessionLog] append failed for', file, err)
}
```

A **synchronous** append on the thread that owns every socket, with the failure
swallowed to a console nobody reads. A slow or full disk stalls every session and
the dropped records are reported to no one.

This is the single largest design risk in the feature, and §4 exists to answer
it. Recorded here because it was found by reading a working implementation, which
is better evidence than an assertion that async writes matter.

### 1b. Placement differs too

Their capture is driven from the renderer (`GameWindow.logToSession`), so what is
logged depends on a window being open. Here capture is **per session**: closing a
browser tab must not punch a hole in a running character's history. Same rule as
the wire log, same reason.

---

## 2. What already exists

Measured 2026-09-21. Most of the prerequisites are built, which is why this is
proposed now rather than at M6.

| Piece | Where | State |
|---|---|---|
| Line reconstruction | `cena-ui/src/lines.rs` — `LineAssembler`, 267 lines | **Built** (PR #1). Bounded per stream; resets on generation change and lag recovery, so text from two histories is never joined. |
| The line | `cena-ui/src/view.rs:16` — `StoryLine { stream, runs, truncated }` | **Built** |
| Rotation | `sink/writer.rs:558` `roll()` | Built for the wire log; the pattern transfers, the code does not. |
| Redaction | `sink/writer.rs:60-175` | Built, including **across chunk boundaries**, and a sink outliving the secret it was opened with (every generation has a new launch key). |
| Flush on drop | `sink/writer.rs:224` | Built |
| Config pattern | `sink/config.rs` | `log_dir()`, `rotate_after_lines()`, env overrides, and the relative-default reasoning. |
| Timestamps | `sink/config.rs:205` `line_time()` | Built |
| File I/O in `cena-session` | `combat_recorder.rs` | The precedent: *"the recorder lives here — file I/O — while the state machine that produces its input lives in `cena-model`."* |

**`plan/23` §4a gap 3 is closed.** It recorded line assembly living in the binary
(`run.rs:506-566`) while `GameState::pending` implemented the same rule, and said
the lift into `cena-ui` should happen when `cena-web` became the third copy. PR #1
did it. This feature consumes `LineAssembler`; it does not reimplement it.

---

## 3. Decisions

### D1. It lives in `cena-session` — **AUTHOR, 2026-09-21**

*"sessions is fine."*

Consistent with `combat_recorder.rs`: the session crate owns the actor, does file
I/O, and already depends on what it needs. `cena-model` is barred from I/O by an
architecture test, and `cena-ui` is a view crate with no I/O today — putting a
writer there would be the first, and would make every frontend depend on a file
handle.

### D2. One timestamp per line — **AUTHOR, 2026-09-21**

Asked whether to carry both wall-clock and server game time:
*"you talking about two timestamps on every line? that's a bit much."*

Agreed, and the wire log made the same call first. `sink/config.rs:34` fixes
`TIME_FORMAT` at `%H:%M:%S%.3f` with no date, and `:175` gives the reason:

> *"This is also why `TIME_FORMAT` carries no date — it is already here, and
> repeating it on 30,000 lines is waste."*

So: **`HH:MM:SS.mmm`, wall clock, date in the filename**, byte-identical in shape
to the wire log's stamp. Two logs of the same session then line up by eye, which
is the whole point of having both.

Server game time is NOT carried per line. It is available in the model
(`state/clock.rs:55` `game_time_now()`) and is the right thing for a *behavior* to
reason with, but a reader wants to know when they were at the keyboard.

**Consequence: `StoryLine` gains nothing.** The stamp is applied by the writer at
the moment it receives the line, not carried in the view type. `view.rs:22`'s
rule — *"projection never guesses"* — stays intact: a view has no clock, and a
log is not a view.

### D3. Capture is per session, not per viewer

§1b. The writer sits on the session side of `LineAssembler`. Zero viewers is a
normal state (headless is first-class), and the log must be identical in it.

### D4. Compression: `flate2`, which costs nothing — **AUTHOR, 2026-09-21**

Closed days gzip; today's file is left alone.

**The dependency is already compiled into every build.** An earlier draft of this
section called `flate2` a new dependency and asked for a decision on the grounds
that it was one. It is not — MEASURED:

```sh
$ cargo tree -i flate2 --workspace
flate2 v1.1.10
└── compression-codecs → async-compression → tower-http → reqwest → cena-platform
```

and `cena-session` already depends on `cena-platform`. `flate2 1.1.10` and all
three of its own dependencies (`miniz_oxide`, `adler2`, `crc32fast`) are in
`Cargo.lock` today. Naming it directly adds **no crates, no compile time and no
supply-chain surface**.

The `gzip` at `cena-platform/Cargo.toml:75` is a separate thing and does not
transfer — it is a reqwest feature for decompressing HTTP responses, which is why
the earlier draft mistook the situation.

Worth the measurement either way: their 85–90% figure is, for a heavy player, the
difference between a few hundred MB and a few GB per year.

### D5. Retention defaults — **PROPOSED**

Two dials, per §1: `retention_days` (default 30, deletes by age) and a cap on
uncompressed bytes only. Lichborne disables the size cap by default; I would too —
a surprise deletion on first launch is the failure mode, and §7 makes deletion
always previewable.

Named in `sink/config.rs`'s established `CENA_*` style, with its relative-default
reasoning (*"an absolute default is right for exactly one machine"*).

---

## 4. The write path, which is the risky part

§1a is what happens when this is done casually. The requirement, from ATARI's
parity plan §6 and independently confirmed by reading the code that violates it:

> *Bounded asynchronous writing; errors/backpressure and any dropped records are
> visible. Disk-full cannot stall game parsing or quietly claim complete history.*

Three properties, each testable:

1. **The actor never blocks on the disk.** Lines cross a bounded channel. The
   actor's send is non-blocking and its failure mode is "dropped", never "waited".
2. **A drop is counted and surfaced.** A full channel increments a counter that
   reaches the session's state, so a frontend can show *"log is behind"*. §5.2:
   a gap that nobody is told about is indistinguishable from no gap.
3. **A write error is reported, not swallowed.** It reaches the same place the
   drop counter does.

**Property 1 is the one a test cannot easily prove** — the same honesty
`sink/writer.rs:183-188` already applies to its atomic-rename claim, which it
records as enforced by review rather than by test. Say so rather than implying a
test covers it.

---

## 5. The read path

Every read runs **off the session owner** (§1's quote). Search, day listing,
stream listing, disk usage and export take the character and a range, and touch
no actor state.

Reads **flush the writer's buffer first**, or a search of the live session misses
its last second.

Bounded, per §1: 1000 hits, 366-day window, and pagination before anything is
serialised toward a frontend.

---

## 6. Build order

Each step leaves the tree green and is independently reviewable.

1. **The record type and the channel.** What a line is on the way to disk:
   stamp, stream, text, and the session/generation it belongs to. Bounded
   channel, drop counter, the counter reaching session state. No file I/O yet —
   §4's three properties are testable without a disk.
2. **The writer.** Per character per day, `{Character}_YYYY-MM-DD.log`, the
   `[HH:MM:SS.mmm][stream] text` line format, buffered per §1, flush on drop.
   Reuses `sink/`'s redaction: a player log is exactly as capable of containing
   a launch key as the wire log is.
3. **The reader.** Day listing, tail, window read, literal and regex search with
   the §5 caps. Both line formats accepted from the first commit (§1) — the
   second format does not exist yet, and that is the point.
4. **Compression on close** (D4). Closed days gzip; today untouched.
5. **Retention**, per D5, with §7's deletion preview.
6. **Export**, per §1's spec shape.
7. **Disk usage**, broken out raw / archive / total / day count.

Steps 1–3 are the feature. 4–7 are what make it survivable over years.

**Not in scope:** the catch-up digest. Theirs is genuinely good — pre-dedup
tallies, `redactForAI()` applied only to the AI-bound body while the disk log
stays pristine — but it is DR-shaped (work orders, their combat ladder) and the
GS equivalents would need our own model. One detail is worth copying whenever it
is built: **redact the copy that leaves the machine, never the archive.**

---

## 7. Verification

- **Zero viewers.** The log is identical with no frontend attached. D3's whole
  point, and the failure mode the reference has.
- **A reconnect does not join two histories.** `LineAssembler` resets on
  generation change; a test asserts the log shows the same boundary.
- **Drops are visible.** Fill the channel, assert the counter moves and reaches
  session state. A test that only asserts "nothing panicked" would pass with the
  counter deleted.
- **Both line formats parse**, from the commit that writes only one.
- **Tallies count before dedup.** Two identical lines are two, and this is
  asserted rather than assumed — §1's DR-speech pitfall is the evidence that it
  is easy to get backwards.
- **Retention never surprises.** Deletion is previewed; no test runs against real
  player logs.
- **Redaction.** A launch key in a game line does not reach disk, including
  across a line boundary (`sink/writer.rs:459` already solves this for bytes).
- **Mutation-test every classifier and every guard.** The session standard, and
  `plan/19`'s recurring finding applies with force here: *a test can pass a
  mutation because the input never reached the code.* Check the input, not only
  the assertion.

---

## 8. Attribution

Lichborne is BSD-3-Clause. **No code is copied by this plan** — the design
decisions in §1 are credited to reading it, and any future copied or adapted code
carries its notice. Its branding is not reused.

## Source references

| | |
|---|---|
| [L1] | `elanthia-online/Lichborne` `aaeca23f`, `src/main/sessionLog.ts` |
| [H1] | `crates/cena-platform/src/sink/mod.rs:1-30` — the two logs, named in 2026-09-18 |
| [H2] | `crates/cena-platform/src/sink/config.rs:34,175,205` — `TIME_FORMAT`, why no date, `line_time` |
| [H3] | `crates/cena-ui/src/lines.rs` — `LineAssembler` |
| [H4] | `crates/cena-session/src/combat_recorder.rs:1-22` — the file-I/O-in-`cena-session` precedent |
| [H5] | `plan/23-m4-frontend.md` §4a — gap 3, now closed |
