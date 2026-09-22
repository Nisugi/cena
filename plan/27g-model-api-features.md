# 27g — Society, bounty, spells, effects, cooldowns and movement

The remaining features: things the game tells you about your standing, your
magic and your travel.

Read [`27`](27-model-api.md) first.

---

## Spells — `spells.rs`, `state/known_spells.rs`, `effects.rs`

| Piece | What it is |
|---|---|
| The table | **523 rows** in `data/spells.tsv`, cut from `data/effect-list.xml` |
| `known_spells` | What `Component{spells}` listed — **the spell number is the link's `noun`** |
| `effects` | What is active on you now |
| `spellsong` | Bard songs |

`knows(number) -> Option<bool>`: **`None` means the list was never sent**, which
is not the same as "you do not know it".

> **Four corrections the author made that no amount of testing would have
> caught:**
>
> - `Spell.active` is not `Effects`, and still carries what never migrated.
> - The spell `type=` tag **is** a closed vocabulary after all — a one-time port
>   of a near-static list does not need the defensive reading.
> - Cooldown kinds are two *cast mechanics*, not two data sources.
> - "gone" does not mean "hid" — an inference I had implemented with two of its
>   three conditions, and tested with a test that shared the same mistake.

> **TWO COPIES OF `effect-list.xml` EXIST ON THIS MACHINE**, differing by
> **exactly** the five `<cooldown>` elements under test. Cut from the older one,
> `with_cooldowns()` is empty and the invariant test passes over zero rows — *a
> green suite reporting a feature that is not there.* The right file was picked
> by luck; the extractor now records its source path and mtime, and a test
> asserts the five exist.
>
> **Source and data files age independently** (author): *"reference/lich-5 should
> be pulled from upstream so it should be about the same as c:\gemstone\dev\lich-5,
> doesn't mean the data files are the newest."* A clone can be perfectly current
> and still hand you a stale table.

**40 of 338 spell durations are real Ruby**, five of them scraping the
scrollback. They read as `Duration::Unknown` with the source kept — recorded
rather than guessed.

## Cooldowns — `state/cooldowns.rs`

Which characters a spell has locked out (PR #1597).

**Reconnect: cleared, and the contrast with `containers`/`bank` is the point.**
Those are facts about *you* that nothing changes while you are gone. These are
**stamps on OTHER PEOPLE, taken against a clock this session was keeping** — and
both assumptions break at once: the members wander off, and `game_time_now`
restarts from whatever the new connection reports.

> Keeping them would have a behavior skip a groupmate who is long since
> castable. **That is the quiet failure: nothing looks wrong, a buff just never
> lands.**

## Maneuvers — `state/maneuvers.rs`

Which PSMs the game has said are on cooldown.

**The game never states a duration**, so this records *"the game said this was
cooling, at this server second"* and nothing more. A caller wanting "is it
ready" has `said_at` and the game clock and can decide how stale a reading it
will trust.

- There is **no `is_cooling`**, deliberately: inventing a duration would be the
  `plan/18` `pbarStance` mistake — an interface over data nobody has measured.
- There is **no `clear` for one maneuver**: a maneuver is off cooldown when the
  game stops refusing it, which only a send can discover.

## Bounty — `state/bounty.rs`, `bounty_status.rs`

What the guild asked for, and what it will do next. Ported from Lich's Bounty
module, with `ebounty.lic` supplying the messaging.

`Refusal::{AlreadyAssigned, Wait{minutes}, NoneAvailable}`, plus
`vouchers_remaining()`.

> **`bounty.rs` was fully built with zero callers.** `bounty_status.rs` is the
> consumer that was missing — found by asking "who calls this", which is the
> same audit that found `messages_of` and the whole combat dialog unwired.

Distinct from `GameState::objectives`, which is the **dialog's row**; this is
the task's own description, parsed.

## Societies — `state/societies.rs`, `character/vocabulary.rs`

The `Society` enum and rank tracking. **Guardians of Sunfist ranks differently
from the others**, which `standing.rs` encodes.

> **Not fully exercised.** Covering the societies needs a character run through
> them — recorded in `plan/20` rather than assumed.

## Movement — `movement.rs`, `state/movement.rs`

| Piece | What it is |
|---|---|
| `MoveFeedback` | What the game said about a move, matched against `LADDER` |
| `MoveFailure` + `CAUSES` | Why a move failed |

**Order matters and a test asserts it.** The specific causes sit above the broad
ones (`move.rb:59-60`): `Map`'s pattern includes `too far away`, `Engaged`'s is
the bare word. Reordering changes answers, so `CAUSES` is a slice in **source
order**.

`MoveFailure::Climb` has no phrases of its own — it is only reachable through
`roll_cause`, because that path is entered only for a climb or a swim, which
makes the fallback *information* rather than a guess.

## Fog — `state/fog.rs`

Fog of war: where you have been.

**`moved_from` is the subtle part**, and Lich's comment explains why
(`fog.rb:230`): `XMLData.room_id` is an **MD5 of the room's text** when the room
has no UID, so **two unmapped rooms that read the same share an id**. That is
what `GameState::arrivals` answers.

> Another of the three-way splits: the **reading** half is built, the
> **sending** half is M6.

## Objectives and menus

| Field | Wire source | Notes |
|---|---|---|
| `objectives` | `Frame::ObjectivesUpdate` | **Reconnect: kept** — clearing would leave `is_known()` false with no way back until the next change, which is worse than a stale entry and also less true |
| `learned_commands` | `Frame::CmdListUpdate`, `CmdTimestamp` | **Reconnect: kept** — what a menu coordinate MEANS is a fact about the game, and the push is not repeated on reconnect |

## Claim — `state/claim.rs`

Who has authority over a session's sends. `Claimant` stays an **enum**, per the
author's standing goal: an LLM controller is a real target, and observing must
stay separate from acting.

---

## What is deferred, and what it waits on

| Item | Waits on |
|---|---|
| The sending halves of stash, bank, fog, spellsong | M6 — they need the authority token and roundtime, and would put `fput` in a crate with no socket |
| `stance` as an enum | a behavior that SETS stance (M6). The five stances are a closed vocabulary; the typed version is half an hour |
| The `resource` capture | a real sample. **Everything else here has wire evidence; this one does not** |
| The ascension PSM table | one run of `ascension info` |
| Society coverage | a character run through the societies |
| `creature_message.rs` wiring | a hunting behavior (M6) — see [`27d`](27d-model-api-room.md) |

**SE-4** (authority across generations) and **SE-6** (`Lagged` recovery
unreachable) remain deferred, each needing an author decision.
