# 27 — What the model knows: the API, by area

**The author asked for this twice.** First as *"a list of everything model
tracks and pertinent info about it"* (2026-09-21), then deferred — *"I want to
hold off on the api doc for a bit"* — until the gaps were filled. They are, so
here it is.

## What this is, and the one thing it is not

Every fact `cena-model` holds: what it is, **where on the wire it comes from**,
**who reads it**, and **what a reconnect does to it**.

It is **not** a tutorial and not a design document. `plan/12` is authoritative
for what gets built; this says what exists, measured against the code on
2026-09-22.

MEASURED at that date:

| | count | command |
|---|---|---|
| modules in `cena-model` | 99 | `find crates/cena-model/src -name '*.rs' \| wc -l` |
| public items | 779 | `grep -rn "^pub fn \|^pub struct \|^pub enum \|^    pub fn " crates/cena-model/src --include=*.rs \| wc -l` |
| `GameState` fields | 36 | `grep -E "^    pub [a-z_]+:\|^    [a-z_]+: " crates/cena-model/src/state.rs \| wc -l` |

## Why the reconnect column exists

It is the column nothing else writes down in one place, and **M5 is why it
matters now**: multi-session means N of these side by side, each reconnecting
independently. A fact wrongly kept across a generation is a stale belief a
behavior will act on.

`plan/12` §5.2 is the rule it enforces: *`Unknown` is a first-class value, not a
default.* A cleared field reads `None`/empty, **never `0`** — `0` is a real
epoch second, a real percentage, a real count, and a consumer cannot tell it
from "never observed".

The test is `state/reconnect.rs`'s own: **what cannot have changed while we were
away, or what a command taught.** Not "what the login burst re-sends" — that
was an earlier and wrong formulation, and the file records the correction.

## The area documents

| Document | Covers |
|---|---|
| [`27a`](27a-model-api-spine.md) | `GameState`'s 36 fields: the spine, with the wire source and reconnect rule for each |
| [`27b`](27b-model-api-character.md) | Character: identity, stats, skills, experience, training, standing, PSM, profile |
| [`27c`](27c-model-api-combat.md) | Combat: the tracker, its chunk cursor, crit tables, flares, and the event it emits |
| [`27d`](27d-model-api-room.md) | Room and creatures: roster, status flags, departures, overwatch, targeting |
| [`27e`](27e-model-api-inventory.md) | Inventory: hands, containers, the worn snapshot, bank, nouns and resolution |
| [`27f`](27f-model-api-streams.md) | Streams and messages: buffers, the closed-window rule, speech and channels |
| [`27g`](27g-model-api-features.md) | Society, bounty, spells, effects, cooldowns, movement and travel |

> **Written in that order deliberately.** `27a` is the spine every other
> document hangs off; the rest can be read alone.

## Standing hazards this document set records

These are not advice. Each is a defect that happened here and is written where
someone will find it.

**A census describes the traffic it read.** `stream_routing.rs` carries two
corrections for this in opposite directions: one census over-claimed
(`charprofile` listed as pushed when `profile`'s output arrives on the main
window), and one under-claimed (`speech` absent from the pushed set, because the
corpus predates the traffic the author pasted). Neither list is closed.

**A test can pass a mutation because the input never reached the code.** Five
occurrences are recorded across `plan/19` and M3. A green mutation run says
*look at the input*, not just the assertion.

**A citation that resolves to nothing manufactures a false negative.** It does
not fail loudly. `CLAUDE.md` records the `styleIfClosed` incident; two dead
rustdoc links naming methods that never existed were found on 2026-09-22.

**A classifier with no caller is not a feature.** `bounty.rs`, `messages_of`
and the whole combat dialog were each fully built with zero callers, found only
when something tried to use them. Where a module is unwired, these documents say
so in its own words.
