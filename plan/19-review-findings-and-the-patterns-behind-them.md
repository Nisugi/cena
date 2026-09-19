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
| **A** | `Option` used as two states where three exist | roundtime (real), effects (**not** — see §1b), `exits: []` |
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

### 1c. `exits: []`. OPEN, and the same shape.

An empty `Vec` cannot distinguish "a room with no exits" from "exits not yet
observed". Noted for M2's room work, where per-component buffers
(`plan/18` §2c) already require thinking about empty-versus-absent.

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
- **The `.bytes` log is not the wire.** `writer.rs:282` appends `\n` to any chunk
  that ends mid-line, so a chunk boundary inside `<pushStream id='ro` becomes
  `<pushStream id='ro\nom'/>`. **This is the file M2's golden corpus is cut
  from**, and criterion 7's replay reads it.

### Open, needing a decision rather than a patch

- **The `Recorder` is unbounded on the production path.** An append-only `Vec`
  that copies every chunk (`record.rs:53`), carried across generations, and
  `outbound_count` rescans all of it twice per connection. `unknown_tags` has the
  same shape. Twenty-five long sessions hold every byte ever read — which the
  sink has already written to disk.
- **The ladder resets on any outbound byte** (`supervisor.rs:320`), so with a
  behavior sending, "attended" is always true and neither bound binds. Two
  clients fighting over one character would re-login at the 1-second rung
  forever. The comment cites `earned_a_reset`, which does not exist.
- **The author's real name and account name are in tracked files.** The repo is
  private with no remote-tracking branches, so a history rewrite is free *now*
  and stops being free later.

### Architecture-test gaps

These matter more than their severity suggests, because the suite is what the
ratchet rests on.

- **The cap-increase check was never written.** `plan/05:353` says the arch test
  fails on a cap increase in the diff. It does not exist — the default went 400 →
  800 with the suite green — and the ratchet still counts Rule 4.1 as covered.
- **The source walk skips `target` at any depth** (`harness.rs:195`), so a
  future `hunt/target/` would be invisible to every rule.
- **The facade rule only checks `lib.rs` and `mod.rs`** (`file_rules.rs:236`), so
  `supervisor.rs`, `actor.rs` and `probe.rs` — all split parents — implement
  freely.
- **Tests that cannot fail**: the "open" probe always yields `Text("X")`; the arm
  scan uses `unwrap_or_default()` on a missing file and skips `markup.rs`; the
  corpus skip is invisible under a default `cargo test`.

### Citation rot

- `CLAUDE.md:120` says the tag table holds 123; it holds **126**.
- The criteria cited as `plan/12:457-466` across ~10 files are now at
  **`plan/12:537-552`**.
- `plan/12` §6.4 is cited and **does not exist**.
- The banners say `cena` and should say **Hydra** (`main.rs:137`,
  `writer.rs:222`).

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
