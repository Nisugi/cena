# 27b — Character

Everything the model knows about the person you are playing: identity, stats,
skills, experience, training, standing, PSMs and the profile.

Read [`27`](27-model-api.md) first. The spine is [`27a`](27a-model-api-spine.md);
this is `GameState::character`.

MEASURED 2026-09-22: **16 submodules** under `state/character/`, and
`Character` holds **41 public fields**.

```
ls crates/cena-model/src/state/character/ | wc -l                          -> 16
grep -cE "^    pub [a-z_]+:" crates/cena-model/src/state/character.rs      -> 41
```

---

## Why this is the largest area, and why it is text

Most of the model reads structured markup. **This area is mostly command
output**: `info`, `skills`, `experience`, `profile`, `society`, `ascension`.
The game states these when asked and never again, which drives two things:

- **Staleness is real and per-group.** `snapshot.rs` stamps each `Group` with
  when it was taught, and `character_store` refuses a snapshot older than
  `MAX_STALE` (30 days). That is a judgement, not a measurement — Lich uses a
  hardcoded cutoff *date* instead (`infomon/cli.rb:76`), bumped by hand when
  its parser changes. Cena does that with `SCHEMA_VERSION`, so this only has to
  answer *"how long before a character has probably trained"*.
- **A reconnect keeps most of it.** Stats and identity are command-taught, not
  connection state, so `invalidate_for_reconnect` leaves them. The store is the
  **primary record** (author, 2026-09-19): a truncated file is not a lost log
  line, it is a character who has to re-run fifteen commands.

---

## Identity — `character.identity`

| Field | Wire source | Notes |
|---|---|---|
| `race`, `profession`, `gender`, `age` | `info` | `age` also arrives from the **profile**, and `absorb_profile` routes it here |
| `account` | login | |
| `name`, `instance` | login | The store's filename is built from both (`store::character_path`), **lowercased** since 2026-09-22 — one character must have one file on every filesystem |

## Stats and skills

| Field | Wire source | Notes |
|---|---|---|
| `stats` | `info` | Each `Stat` carries `normal`, `ascended`, `enhanced` and `enhanced_is_bolded` — four readings of one stat, because the game shows them differently and a consumer needs to know which it got |
| `skills` | `skills` | `SkillLine::classify` parses the table; `SkillSet` holds ranks and bonuses |
| `enhancives` | worn items | |
| `psms` | `ascension`/`cman`/`shield`/`weapon` lists | See below |

> **`Stat` is four values, not one, and that is deliberate.** An ascended stat
> and an enhanced stat are different facts about the same attribute, and a
> behavior asking "what is my Strength" has to say which it means.

## Experience, and the trap in it

| Field | Wire source | Notes |
|---|---|---|
| `experience`, `total_experience`, `long_term_experience` | `experience` | |
| `field_experience`, `field_experience_max` | `<progressBar>` + `experience` | |
| `ascension_experience` | `experience` | |
| `mind_state`, `mind_percent` | `<progressBar id='mindState'>` | |
| `next_level`, `next_level_percent`, `level` | `<progressBar>`, `experience` | |
| `deeds`, `deaths_sting`, `recent_deaths`, `fame` | `experience` | |
| `pulses` | `experience` | |

> **AUTHOR, 2026-09-21, correcting a wrong inference:** *"you only see the one
> because my character is not doing regular experience so he doesn't gain tps,
> he is doing ascension experience which doesn't report there."*
>
> A census over one character's logs found **1** training-point line and I read
> it as a login-burst artefact. It was a fact about that character's
> progression. **A census over one character measures that character**, and the
> author's follow-up is the method this document set uses: *"sleuth through the
> lich parser, xmlparser, and all of it's related things to see what else is
> missing rather than just looking at what my character only sees."*

## Training points — `character/training.rs`

| Field | Wire source |
|---|---|
| `physical_training`, `mental_training` | `<label id='PTPs'>`, `<label id='MTPs'>` |
| `physical_converted`, `mental_converted` | `<label id='p2m'>`, `<label id='m2p'>` |

**The wire is the only authority.** Lich has these nowhere — they were found by
reading `<label>` traffic, not by porting. `read_label(exp, id, value)` handles
all four.

## Condition

| Field | Wire source | Notes |
|---|---|---|
| `wound`, `scar`, `injuries` | `Frame::InjuryImage` | `injured.rs` maps body parts; a scar persists where a wound heals |
| `stance`, `stance_percent` | `<progressBar id='pbarStance'>` | **A verbatim string, not an enum.** The five stances are a closed vocabulary and the typed version is half an hour — but nothing SETS stance until M6, and `plan/18` records the `pbarStance` mistake: an interface over data nobody has measured |
| `encumbrance`, `encumbrance_percent`, `encumbrance_detail` | `<progressBar id='encumlevel'>` | |
| `shrouded` | text | |
| `currency` | `wealth` | |

## Standing — `character/standing.rs`

| Field | Wire source | Notes |
|---|---|---|
| `society`, `society_rank` | `society` report **and** the profile | Two readers, because the shapes differ |
| `citizenship` | `citizenship`, profile | |
| `warcries` | Warrior guild | |
| `resource_type` | `resource` | **The one thing in `plan/20` with no wire evidence.** It wants a real sample before anything is built on it |

> **The profile and the report do not say it the same way**, which cost a
> reader. The profile line is `Master of the Guardians of Sunfist`; the
> report's is indented and reads `You are a Master of...`. `profile_affiliation`
> exists for the first, and a test asserts it — **found by mutation**: the
> capture contains both, so the profile reader could be deleted and the
> assertion still passed, because the report filled the rank by itself.

## Profile — `character/profile.rs`

Title, birthday, description, affiliations, achievements, history.

> **BUILT AGAINST THE WRONG STREAM FIRST, and the correction is the lesson.**
> `stream_routing.rs`'s census listed `charprofile` among pushed stream ids, so
> the reader was written to read that buffer. MEASURED over the two `profile`
> runs in `GSIV-Nisugi`: **2 `exposeStream`, 0 `pushStream`.** All 11 tests
> failed on an empty buffer.
>
> The text prints into the **main window**; the stream is declared, cleared and
> exposed, which is a display instruction about a window and not a route for
> text. The reader is a main-window chunk reader, and the census now carries the
> correction.

`absorb_profile()` routes what belongs elsewhere: age → `Identity`,
society/citizenship → `Standing`.

## PSMs — `character/psm.rs`

Combat maneuvers, shield specialisations, weapon techniques, armor and
ascension abilities: name, rank, and the category word the line used.

**The category is kept as the game's own word**, not parsed into an enum here.
A classifier that refused an unrecognised category would silently drop a row,
and mapping it is the caller's business.

> **The ascension table is not captured.** It needs one run of
> `ascension info` (and `ascension list all all` if it differs). Recorded in
> `plan/20` rather than guessed at.

## Snapshot and the store — `character/snapshot.rs`

`CharacterSnapshot` is what `cena-session`'s `character_store` writes: one JSON
file per character, `<instance>_<character>.json`, pretty-printed because it is
a file a person opens when their skills look wrong.

- **`Group` staleness is per-group**, so a login can report exactly which
  commands to re-run.
- **`SCHEMA_VERSION` mismatches are refused, not migrated.** A snapshot is the
  output of a parser, so one written by a different parser may have read a
  column differently. The character re-syncs, which is the recovery Lich's own
  version check arranges (`cli.rb:73-80`).
- **Writes are atomic** (temp-then-rename in the same directory). Not
  unit-tested, and `store.rs` says so plainly after three failed attempts —
  the property is *"a crash between the truncate and the write leaves a valid
  file"*, and a unit test cannot crash the process at a chosen instant.

## What a reconnect does

**Kept**: stats, identity, skills, standing, PSMs — all command-taught, and
nothing about a dropped socket changes them.

**Cleared**: `character.invalidate_for_reconnect()` drops what the burst
re-teaches or what stopped being true.

The split is `27a`'s rule: *what cannot have changed while we were away, or
what a command taught.*
