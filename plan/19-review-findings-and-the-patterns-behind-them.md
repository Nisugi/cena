Written for: whoever picks up Milestone 2, including a future session of me.

# 19 — Two adversarial reviews: what they found, and the four patterns behind it

**Status: record, not a proposal.** Everything here is either fixed (with the
commit named) or open (with what remains to decide). Two reviews of `crates/`
produced twenty-odd defects between them; this is the page the author asked for
so the *patterns* survive rather than being rediscovered one item at a time.

Every claim below was re-verified against the code before being written down. The
reviews were right about every item this page marks fixed, including two against
my own commits.

---

## 0. Why a page, and not a list of issues

Individually these are edge cases. Collectively they are **four repeated
mistakes**, and the fourth is the one that matters most:

| Pattern | Shape | Items |
|---|---|---|
| **A** | `Option` used as two states where three exist | roundtime (real), effects (**not** — see §1b), `exits: []` (**fixed**, §1c) |
| **B** | The reconnect layer assumes an actor exists behind a handle | `quit` hang, unbounded `send_now`/`claim`, `connect` unraced |
| **C** | Guarding tests sized to the measured case, not past it | the settings blob, the chunk reassembly, the inbox |
| **D** | **A test that pins a defect in place** | the hash-error leak |

Pattern D is worth its own heading because it inverts the usual assumption. A
passing suite is evidence only about the things the suite asserts, and one
assertion here was *enforcing a password leak* — it would have failed the fix for
it.

---

## 1. Pattern A — `Option` as two states where three exist

`Option<T>` models "known" and "unknown". A predicate over it has **three**
meaningful answers — yes, no, and *I do not know* — and both defects came from
collapsing the third into one of the first two, in opposite directions.

### 1a. Roundtime read "not in roundtime" during a live roundtime. FIXED.

`clock.rs:93-96`:

```rust
pub fn in_roundtime(&self) -> Option<bool> {
    let now = self.game_time_now()?;
    Some(self.roundtime_ends.is_some_and(|ends| now < ends))
}
```

`is_some_and` on `None` is `false`. So a cleared `roundtime_ends` reported
**`Some(false)`** — "definitely not in roundtime" — as soon as a prompt restored
the clock. A review reproduced it with 20 seconds of real roundtime remaining,
which lets a `Gate::Roundtime` send through early.

`invalidate_for_reconnect` was clearing it, citing §5.2's *"`world.roundtime`
after reconnect is `Unknown`, never `0`"*. **The intent was right and the
mechanism produced the opposite of it.**

Fixed by **retaining** it. `roundtime_ends` is an *absolute server epoch*: a
roundtime that ends at server second N ends at N whether or not the socket
survived. Unlike the room or the hands, nothing about it can have changed while
we were away, so keeping it is keeping a true fact rather than a stale belief.
§5.2's actual prohibition — asserting "not in roundtime" on no evidence — is
still enforced by the clock's own invalidation.

**Two tests asserted the old behaviour** with no reasoning attached, and both
were flipped with the reasoning written in.

### 1b. Effects with no end time. **NOT A DEFECT — asked Vellum, as the rule says.**

`effects.rs:126`: `effect.ends_at.is_none_or(|ends| now_server < ends)`.

`is_none_or` on `None` is `true`, so an effect with no end time reads **active
forever**, and a review found a 1m59s buff still reported active an hour later.

**`CLAUDE.md` says read Vellum first, and doing so closed this rather than
opening it.** Two findings:

1. **`None` meaning "active while present" is correct, and deliberate there.**
   `reference/VellumFE/src/core/hotbar.rs:207` is a test named
   `indefinite_effect_counts_as_active_while_present`, with Prestidigitation as
   the example — a genuinely indefinite buff. Its neighbour at `:233` asserts
   that an effect with no expiry answers *false* to "is under 60 seconds left",
   rather than inventing a remainder. Cena's `remaining` already does the same.

2. **Expiry arrives by REPLACEMENT, not by clock.** `state.rs:1919` retains on
   `expires_at > now_server`, but the load-bearing path is `:1901`: the game
   re-sends the whole dialog and the collection is replaced. An effect that ended
   is simply absent from the next one.

Cena already matches on both counts: `Frame::ClearDialogData` clears per
category and the populated element refills it (`state.rs:237-245`, MEASURED).

**So the reviewed scenario needs the dialog to never refresh** — which is the
`ends_at: None` case only in the sense that nothing has told us otherwise. That
is the same "no news" state the rest of `GameState` models with `Option`, and
inventing an expiry for it would be exactly the invented belief §5.2 forbids.

Recorded as a non-defect because the *shape* still matches pattern A, and a
future reader comparing the two `Option` predicates deserves to know why one was
changed and one was not: roundtime's cleared value produced a **false negative
that acts** (a gated send fires early), while an effect's absent end time
produces a **stale display that no gate reads**.

### 1c. `exits: []`. **FIXED 2026-09-19.** `Option<Vec<String>>`.

An empty `Vec` could not distinguish "a room with no exits" from "exits not yet
observed".

**The author settled the game fact**, and it is stronger than the finding
assumed — *both* empty cases are real, and they differ from each other:

- a room whose only way out is a **portal, door or teleport** rather than a
  cardinal direction. The compass is empty and the room **is** exitable.
- the **consultation lounge**, with no exits at all.

So an empty compass is something the server *stated*, and anything that would
route must act differently on it: "not looked yet" invites a look, "no cardinal
exits" says find the door.

`Room::exits` is now `Option<Vec<String>>`: `None` unobserved, `Some(vec![])`
observed-and-empty. The type change found all four consumers at compile time.
`invalidate_for_reconnect` produces `None` (via `Room::default`), which is what
makes the unobserved state reachable in a live session rather than only at
startup.

---

## 2. Pattern B — the reconnect layer assumed an actor always exists

Every item here was correct code that **stopped being correct when the
supervisor landed**, because each assumed a handle has an actor behind it.
Between generations it does not: the supervisor holds the receiver and reads
nothing while it climbs its retry ladder.

This is the most instructive pattern, because nothing was *broken* by the
supervisor — the premises simply expired, silently, in files the supervisor
never touched.

| Item | The expired premise | Status |
|---|---|---|
| `quit` hung forever | its timeout bounded the server's EOF, not the operation | **FIXED** `31c5d95` |
| `send_now` / `claim` waited unbounded | *"a timeout here would be a deadline on a wait that cannot hang"* — true when an absent actor dropped the sender | **FIXED** `2a09a61` |
| `stop` during a reconnect ran a full login | only the backoff sleep was raced against the cancel token | **FIXED** `2a09a61` |

The second one is worth reading in full, because the doc comment argued the case
correctly and the conclusion still went stale: an absent actor *used* to drop the
sender and resolve `Dead` immediately. With a supervisor the sender is alive, so
the wait is unbounded — and `look` awaits `claim` **outside** its own cancel
select (`look.rs:155`), so `stop` could not stop a behavior during an outage.

The third left a character **link-dead in-world**: the login completed, the new
actor saw the cancel on its first turn, and the socket closed without a `quit`.

**What to take from this:** when a layer is added below existing code, the
existing code's *reasoning* needs re-reading, not just its behaviour re-testing.
Three sound arguments became wrong without a single line of their files changing.

---

## 3. Pattern C — guarding tests sized to the measured case

Three guards were written against a measurement and stop working just past it.

### 3a. A `<settings>` blob over 2× the line cap eats the login burst. OPEN.

The oversized-line branch at `read.rs:68` never consults `in_settings`. At
524,290 bytes the second chunk becomes a 256 KB `MalformedTag`, `</settings>` is
discarded, and **every line until the first `<prompt>` is swallowed** — a review
probe lost `<nav>`, `<streamWindow>` and the room text.

The measured blob is **513,700 bytes, 2% under the threshold**, and the guarding
test is sized at exactly that number. So the guard passes on the case that was
seen and fails on a slightly larger one.

### 3b. The chunk-reassembly test only feeds newline-terminated chunks. OPEN.

`sink_redaction.rs:179` cannot see the newline injection (§4a) because every
chunk it supplies already ends in one.

### 3c. The inbox. FIXED, but the measurement is the lesson.

`COMMAND_CHANNEL_BOUND = 32` with its own comment admitting the number is
invented. What actually bounds throughput is that **the actor takes one inbox
message per loop turn**, and a turn can now spend up to `WRITE_DEADLINE` in a
single write. MEASURED: 60 concurrent sends refused 28.

**What to take from this:** a test sized at the measured value proves the
measured value. Guards want a case *past* the boundary, and the boundary is
usually not the number someone measured.

---

## 4. Pattern D — a test that pinned the defect

### 4a. The hash error leaked a password byte, and a test required it. FIXED, `2a09a61`.

The message printed the key byte `k` and the `result`, and
`p = ((result − 32) ^ k) + 32` recovers the password byte. The message itself
claimed *"the password byte itself is withheld"*.

The comment beside it had **already** recorded that the arithmetic was
*"recoverable from the other three"* — added by an earlier review. Nobody joined
the two statements up.

And the test asserted `e.detail.contains("0xff")`, on the stated premise
**"key byte is not secret"**. It was enforcing the leak, and it would have failed
the fix. Replaced, with a note where it stood rather than a silent deletion.

**What to take from this:** an assertion is only as good as the premise in its
message. When a test and a comment disagree, one of them is a bug — and the test
is not automatically the right one.

---

## 4b. The long tail, worked through — 2026-09-19 evening

The ~50 lower-severity findings lived only in
`.workflows/findings/crates-review-2026-09-19.md`, which is **gitignored**, so
this section is the tracked record of what happened to them.

Every one was re-verified against the code at `0dc4ddc` before being acted on,
by four agents each tracing the cited code **by content rather than by line
number**, since the review's own line numbers had drifted. That pass was worth
more than the fixes it enabled:

- **9 findings were already fixed** and still recorded as open.
- **1 was simply wrong.** It claimed a readiness gate would fire for
  `Origin::Script`; a script is not a behavior, so it cannot.
- **1 was wrong in its reasoning but right in substance**, and **1 was half
  wrong** — one of its two claims turned out to be asserted after all.

**That hit rate is the finding.** A review's output is evidence about where to
look, not a list of things to change, and treating it as the latter would have
meant "fixing" code that was already right.

### What was closed, and the shape it had

Each of these landed with a test that fails against the previous code; where a
mutation is quoted below, it was run.

| Area | What it actually was |
|---|---|
| `text::attribute(tag, "")` | **An infinite loop in a `pub fn` of a `pub mod`.** `find("")` returns `Some(0)` at every offset, so the cursor never advanced. |
| the scrubber | Merged two players into one, corrupted substrings (`Eon` inside `Eonake`), ignored case, and let **ESP** through — against its own rule 3. |
| `AppInfo` | Dropped `game`, the **instance**. Prime and Platinum are different worlds, so a session identified by character name alone collides — in the client whose headline feature is multi-session. |
| `Effect::ends_at` | Baked a local `Instant` into compared state, so **a replay could not reproduce the session it recorded** — against criterion 7, which M2's golden corpus rests on. |
| backoff jitter | Took **ten values, not a thousand**: it read digits below the Windows clock's 100 ns tick. The documented +20% end was unreachable, and on a µs clock every session would back off in lockstep — the herd the jitter exists to break. |
| `ask()` | A pipe counted as "someone at the keyboard". Three non-empty lines reached a real authentication against the **live** service. |
| the `.bytes` sink | Spent a part number on a file that was never created, leaving a permanent hole in the capture M2's corpus is cut from. |

### The four that mattered most were in the enforcement suite

This is the part worth carrying forward, because it is the pattern rather than
the incidents:

- **Rule 4.3's deferral was spent** and nothing could tell — a deferral was
  validated only by the length of its reason string.
- **`#[ignore]` was invisible.** Adding it to every test in `layering.rs`,
  including Rule 1.3's, left the suite green.
- **A `/*` inside a string literal blanked the rest of the file** for five
  rules at once. Latent today; `glob("**/*.xml")` is exactly the shape M2's
  corpus walker will use.
- **The tag/arm cross-check had gone blind to five arms** when `markup.rs` was
  split out of `dispatch.rs`.

Each had been silently not-enforcing for months. Each is why a neighbouring
defect survived to be found by a review instead of by a test. **A split moves
code out from under a lexical scan without changing a line of it: nothing
fails, the scan simply stops looking.**

Citation rot — the largest single category, spanning every crate — is now
enforced rather than fixed, over 187 source citations plus the plan documents.

### Three stale counts, one cause

`126` tag-table entries documented as 121, `63` `ParsedElement` variants
documented as 61, and `51` `Frame` variants documented as **50 directly beneath
the command that prints 51**. All three are §−2's second form: a number
restated rather than re-run. Two of them sat in the enforcer, whose own comment
calls a stale count there "worse than in prose".

The remedy §−2 already prescribes is to cite the command. Where that was done,
the command now executes as a test.

### Still open, separated by what they need

**A decision, not a patch.** Recorded rather than fixed, because the right
answer is a judgement about what Cena should do:

- **Effects are keyed by wire id** across four independent dialogs, and
  `active()` answers `None` both for "the game says it is gone" and "never
  observed". The M6-relevant pair — a rebuff behavior cannot tell them apart.
- **Command authority is rebuilt per generation**, so a behavior that waits out
  a reconnect gets `Refused(Permanent)` — documented "will never succeed" — for
  a session that is alive.
- **§6.3's `Lagged` recovery is unreachable**: `subscribe` is on
  `SupervisedSession` and `run` consumes it.

  > **RESOLVED 2026-09-23.** Reachable now: `SupervisedSession::observer()`
  > hands out a `SessionObserver`, and `SessionObserver::subscribe` re-fences
  > after a lag -- tested by
  > `cena-session/tests/observation.rs::lag_resubscription_replaces_the_old_fence_with_fresh_authoritative_state`.
  > It stays true only of the legacy `broadcast::Receiver<Event>`.

**Deferred to M2, deliberately.** The remaining frame-vocabulary gaps —
unknown tags inside `<component>` bodies, `dialogData` without `clear`,
`dynaStream` typed as a window — are M2's actual subject. Fixing them now means
doing M2's work without M2's corpus to verify against.

**Real, small, not yet done.** The rest, itemised in the gitignored findings
file, which remains the place to look for detail.

---

## 5. Everything else, by status

### Fixed before the next live run

| # | Defect | Commit |
|---|---|---|
| 1 | commands executed after their caller timed out — an attack firing after it was abandoned | `31c5d95` |
| 2 | `quit` hung indefinitely during a reconnect; `main` awaits it before cancelling | `31c5d95` |
| 3 | a stalled write blocked reads, cancellation and quit | `31c5d95` |
| 4 | a full inbox silently lost authority releases — permanent lockout of later behaviors | `14f14b6` |
| 5 | **four refusals no retry can fix were transient** — a mistyped character name sent the password to `eaccess` every 30s forever | `2a09a61` |
| 6 | the hash error leaked a password byte (§4a) | `2a09a61` |
| 7 | the write deadline ended the connection at one of three call sites | `2a09a61` |
| 8 | `stop` during a reconnect completed a full login, leaving the character link-dead | `2a09a61` |
| 9 | a rejected login exited 0 printing "Done. Criteria 1-6 … exercised" | `2a09a61` |
| 10 | no Ctrl-C handling: the only early exit skipped both `quit` and the sink flush | `2a09a61` |
| 11 | the queue wedged permanently when a window never saw a prompt — §4.1's "the player is never locked out", broken in combat | this commit |
| 12 | roundtime read "over" after a reconnect (§1a) | this commit |
| 13 | `Redactions` derived `Debug`, so any `{:?}` printed every launch key | this commit |

Item 5 is the one to remember. The author hit that case live two days before it
was found, and the supervisor retries transient failures **forever by design**.

### Held for M2 step 0, at the author's direction

Both are parser-adjacent, and M2's first work is the parser.

- **Frames carry no line boundary.** `push_bytes` splits on `\n` and the newline
  never reaches the `Frame`, so `Screen::push`'s `split('\n')` can never fire.
  Probe output: `"You swing at the kobold!You miss."` The fix belongs in the
  parser — `parse_line` is already called once per wire line, so the information
  exists and is simply not marked.
- ~~**The `.bytes` log is not the wire.**~~ **FIXED.** It appended `\n` to any
  chunk that ended mid-line, so a chunk boundary inside `<pushStream id='ro`
  became `<pushStream id='ro\nom'/>` — a byte the wire never sent, in the file
  M2's golden corpus is cut from and criterion 7's replay reads. `writer.rs`
  carries the post-mortem where the append used to be.

### Open, needing a decision rather than a patch

- ~~**The `Recorder` is unbounded on the production path.**~~ **FIXED** in
  `fec5a82`, pinned by `tests/recorder_bound.rs`: an append-only `Vec` copying
  every chunk, carried across generations, in a client designed for 3–25
  characters. **`unknown_tags` still has the same shape** (`state.rs`) and is
  deep-cloned by every `subscribe()` — that half is open, and the fix is a
  count per name plus a bounded ring of recent raws.
- **The ladder resets on any outbound byte** (`supervisor.rs:320`), so with a
  behavior sending, "attended" is always true and neither bound binds. Two
  clients fighting over one character would re-login at the 1-second rung
  forever. The comment cites `earned_a_reset`, which does not exist.
- ~~**The author's real name and account name are in tracked files.**~~
  **DONE 2026-09-19.** All 85 commits rewritten, verified zero hits across
  every reachable object and dangling blob. Two `refs/codex/` checkpoint trees
  that `git-filter-repo` **skipped while reporting success** still held the
  account name and were deleted separately — worth remembering: that tool can
  report success over trees it did not rewrite.

### Architecture-test gaps

These matter more than their severity suggests, because the suite is what the
ratchet rests on.

- ~~**The cap-increase check was never written.**~~ **FIXED.**
  `the_cap_ratchet_only_turns_down` reads `caps.baseline`, and
  `split_parents_stay_facades` caught two files in this session's own commits.
- ~~**Tests that cannot fail.**~~ **FIXED**, and they were worse than listed:
  - the arm scan's `unwrap_or_default()` meant a **renamed source file yielded
    zero names and a pass** — a dead ratchet in the ratchet itself. It now
    panics naming the path. It also skipped `markup.rs` and `parser.rs`, which
    between them hold five handler arms, so the test that once found the
    `style` gap could no longer see the file `style` had moved into.
  - the corpus skip printed through `println!`, which libtest swallows for a
    passing test. Both replay tests are now `#[ignore]`, so an unset
    `CENA_CORPUS` reads as "2 ignored" rather than `ok`. Its gate test also
    asserted, in the skip branch, the condition that produced the branch.
- ~~**`#[ignore]` on a covered test was invisible.**~~ **FIXED.** Not in the
  original list, and the worst of them: adding `#[ignore]` to every test in
  `layering.rs` — including Rule 1.3's — left the whole suite green, as did
  `#[cfg(any())]` and deleting `#[test]`. The rule stayed in the table, the
  function stayed in the file, and the enforcement was gone.
- ~~**A `/*` inside a string literal blanked the rest of the file.**~~
  **FIXED.** Also not in the original list. `in_block` latches across lines, so
  one such literal disarmed the static allowlist, the `static mut` ban, the
  game-name flag, the include ban and the facade scan for every following line.
  Recorded as "vanishingly unlikely" and "fails safe"; the first was fair, the
  second was backwards. `glob("**/*.xml")` is exactly the shape, and M2's first
  work is the corpus.
- **The source walk skips `target` at any depth** (`harness.rs`), so a future
  `hunt/target/` would be invisible to every rule. **Still open.**
- **The facade rule only checks `lib.rs` and `mod.rs`** (`file_rules.rs:259`),
  so `supervisor.rs`, `actor.rs` and `probe.rs` — all split parents — implement
  freely. **Still open**, and note `split_parents_stay_facades` covers their
  *size*, not their *content*.
- **The `include!` guard is weaker than its docs**, which cite a test name that
  does not exist anywhere. `include!["body.in"]` with square brackets evades
  the needle and **compiles** (demonstrated), as does `#[path = "..."]`.
  **Still open.**
- **Nothing asserts member crates inherit the workspace lints.** A new crate
  omitting `[lints] workspace = true` gets `unsafe`, `unwrap()` and `panic!`
  past `clippy -D warnings`. All seven have it today. **Still open.**

### Citation rot — **closed, and now enforced**

Every item here is fixed: the tag-table count, the criteria citations across
~10 files, the `plan/12` §6.4 that never existed, and the banners, which say
**Hydra**.

More to the point, the category cannot silently return.
`crates/cena-arch-tests/tests/citations.rs` checks that every path-rooted
`path:line` citation resolves — 187 of them in source comments, plus the plan
documents, which is **where the original incident happened**: `CLAUDE.md` cited
a path that did not exist, an agent searched it, found nothing, and concluded
the wiki does not document `styleIfClosed`. It documents it five times.

> A citation that resolves to nothing does not fail loudly; it manufactures a
> false negative.

**What it deliberately cannot check** is whether the cited line still *says*
what the citing comment claims — that needs a human, and the review found
several of those. Nor does it check bare filenames like `wire.rs:160`, which
resolve only against the reader's context; guessing would produce false
failures, the direction that gets tests deleted as noise.

Both tests assert a floor on how many citations they found, because a scanner
that silently stops matching passes by seeing nothing — the failure this suite
had already hit twice.

---

## 6. What the reviews found sound

Worth recording, because it says where the risk *is not*:

parser lexing (no panic, no bad slice, chunk independence tested at every
offset); the `eaccess` handshake arithmetic and its per-read deadlines; the
select loop's cancel-safety and `biased` ordering; generation teardown ordering;
`Debug` redaction on `Credentials`, `LaunchPayload` and `LiveConnector`; the crit
port's loud compile failures; and `look`'s cancellation under virtual time.

**The defects were all at the edges** — the seams between layers, the paths that
only run when something has already gone wrong, and the assumptions that expired
when a layer was added beneath them.
