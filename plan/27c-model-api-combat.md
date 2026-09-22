# 27c — Combat

The tracker, its chunk cursor, the crit tables, and the event it emits.

Read [`27`](27-model-api.md) first. This is `GameState::combat`, reached
through `combat_mut().take_facts()`.

MEASURED 2026-09-22:

```
find crates/cena-model/src/state/combat* -name '*.rs' | xargs wc -l | tail -1   -> 4,994
find crates/cena-model/src/crit* -name '*.rs' | xargs wc -l | tail -1           -> 1,311
```

---

## This is a port, and the author wrote the original

> **`CLAUDE.md`, memory:** years of work, rules measured rather than guessed.
> **Port faithfully; question with evidence.**

`processor.rb` is the source, and its branch order is load-bearing: *"the first
line of an exchange decides which event the outcome belongs to."* Every
reordering Lich tried is recorded there as a bug, and the citations travel with
the branches into `combat/parse/`.

**`combat_stats.db` on the live Lich install is the per-attack answer key.**
When this port and Lich disagree about a real exchange, that file settles it.

## The shape: one parser, a cursor, an event

`plan/12` §3a — **one parser, N classifiers**. Combat is a *classifier plus a
stateful consumer*, never a second parser. If a classifier would have to
re-tokenize markup, that is the signal to widen the frame instead — which is
exactly what happened when `<crtrStatus>` inside a `<component>` degraded to
`Frame::Structural { raw }`, serving 1.2% of real traffic.

| Piece | File | What it is |
|---|---|---|
| The branches | `combat/parse/` (`cursor`, `facts`, `flares`, `switch`, `rolls`, `attack`, `damage`, `finish`) | One module per `processor.rb` range, split by what the branch handles rather than by size |
| The cursor | `combat/parse/cursor.rs` | Lich's ~26 locals: the working state, the pool, the predicates. **Chunk-local** |
| The tracker | `combat/tracker.rs` | What must survive a prompt: `crit`, `pending`, `dropped`, `chunks_seen` |
| The event | `combat/event.rs` | `AttackEvent`, `Hit`, `Crit`, `FlareEvent`, `Redirect`, `Fact` |

## What survives a prompt, and why each must

`tracker.rs` holds exactly three things across a chunk boundary, each for a
measured reason:

| Field | Lich | Why it must survive |
|---|---|---|
| `held_cast` | `@held_cast` | *"You gesture at X."* ends a chunk; the spell's result opens the next. Held for ONE chunk, then superseded or emitted as itself |
| `held_pre_flares` | `@held_pre_flares` | A bow's dispel fires on the NOCK and resolves before *"You fire"* prints — in the next chunk |
| `active_assault` | `@active_assault` | A flurry's rounds arrive over several chunks; the opener named the only target they can strike |

Lich's `@death_watch` / `@death_announced` belong to `persist_event` and arrive
with the creature registry (see [`27d`](27d-model-api-room.md)).
`@position_recovered` is reset at the top of every chunk and is therefore
chunk-local, on the cursor.

## No settings, deliberately

Lich gates damage, wounds, statuses and UCS behind `track_*` settings and the
event payload behind `emit_attacks`. **Every one has exactly one value here —
on** (Rule −1: no config option with one value). `inventory/11` §3c records why:
*"the emit carried a crit-shaped hole"* when a gate was wrong.

## The crit tables are a handle, not a global

`crit.rs` holds `CritTables` behind an `Arc` and **forbids a static**, for the
reason multi-session depends on. The tracker is *handed* its tables by whoever
built the `GameState` (`set_crit_tables`), and without them the crit lookahead
is skipped and every `Hit` carries `crit: None`.

A test that asserts crits sets them; a session sets them.

> **This matters for M5.** A static table would be one table shared by N
> characters — which happens to work until two sessions want different data,
> and then fails invisibly.

## Blob classification: the author's headline goal

> **AUTHOR, 2026-09-21:** *"One of my biggest goals for this program is to be
> able to separate a combat feed out from the main feed ... with enough
> definitions we can have a classifier that classifies a blob as combat if it
> contains one of our defs."*

**Built and wired.** At each prompt:

1. `actor/io.rs` calls `publish_combat()`, which returns whether the chunk
   produced combat facts.
2. `player_log/feed.rs`'s `close(combat)` tags that chunk's **main-window** lines
   `main/combat`.

**The class never replaces the source.** `main` is where the game sent it,
`combat` is what our definitions made of it, and a reader auditing the
definitions needs to tell those apart. A chunk the definitions miss stays
`main`, still in the log and in order — so coverage can grow without old logs
having lied.

The remaining gap is **breadth of definitions for other people's combat**, which
is a definitions problem rather than a mechanism one.

## What the tracker does NOT carry

- **Settings** — see above.
- **Ingestion provenance** (`source`, `combined_observation_source`). A Cena
  chunk belongs to one session's one connection by construction.
- **The `<component id=` skip.** Room components never reach the chunk; the
  stream layer routes them to the room model.
- **The registry.** `persist_event`, the death sweep and position recovery apply
  these events to creatures — the consumer above, in [`27d`](27d-model-api-room.md).

## Reconnect

`combat.invalidate_for_reconnect()`: a held cast or pre-flare belongs to a
chunk the old connection never finished, and an assault bracket cannot outlive
its fight.

**What combat did to a creature is still true of it**; who is standing in the
room is not. That split lives in `creatures.rs`.

## Two things measured against a real exchange

**The parser's side of the bargain** is that every fact the markup encodes
survives into the frames — verified against a real attack sequence, including
`exist`/`noun` and bold depth.

**Two of my own assertions were corrected by the fixtures**, which is the
argument for cutting them from real traffic: I claimed the combat exchange
carried a `<roundTime>` tag and a `>` prompt. It carries the roundtime **as
prose**, and the prompt is `HR>`.
