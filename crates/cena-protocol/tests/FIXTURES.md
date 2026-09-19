# Tier 1 goldens — M1 frame families

Committed, small, run every build. This file lives one level up, in `tests/`,
not in `tests/fixtures/`: it names `Inochi` and `druby://` in order to document
them, and `fixtures_are_scrubbed.rs` scans every file in the fixture directory
without exception. VERIFIED -- it went RED on this README. An exclusion list
would have been a hole; moving the prose out was not. Cut from the corpus at
`E:\Gemstone\data\log archive` (10,849 XML / 49.55 GB) and passed through
`cena_protocol::scrub::Scrubber` by
`cargo run -p cena-protocol --example cut_fixtures -- <raw-dir>`.

The raw excerpts are **not** committed: they are unscrubbed wire logs.
The provenance below is what makes the cut reproducible.

## Provenance

| Fixture | Source file | Lines | Bytes |
|---|---|---|---|
| `room.xml` | `GSIV-Nisugi/2025/11/xml/2025-11-29_22-03-30.xml` | 150-153, 244-247, 271-275 | 2,745 |
| `prompt.xml` | same | 247, 252-255, 1307 | 343 |
| `vitals.xml` | same, plus `GSIV-Dicate/2025/09/xml/2025-09-22_01-49-25.xml` | 92-93, 154, 1286 / 15353 | 6,213 |
| `vitals_secondary.xml` | same as `room.xml` | 94, 423, 1287-1288, synthesised `<roundTime>` | 2,866 |
| `m2_model.xml` | `GSIV-Monstr/2025/09/xml/2025-09-07_16-15-48.xml` | 54, 56, 102, 106-111, 116-120, 192, 204-214 | 9,731 |

Both source files are **pre-2026-08 GSIV** logs, per the corpus findings: that
is the era with no `vellumImg` injection. Both were VERIFIED to contain zero
`vellumImg`, zero `druby://` and zero Lich timestamp-prefixed lines
(`grep -cE '^[0-9]{2}:[0-9]{2}:[0-9]{2}: '` returns 0), so the content is raw
wire and not a client's rendering of it.

## Selection

The two source files came from a **stratified sample of 227 files** over 37
strata (character-directory x year). Within each stratum, paths were sorted and
every *n*th taken to a quota of 8 — deterministic, and not `head`, which would
have returned only `GSIV-Armler/2025`. Of the 227, only **39 contained a room
at all** (`<compass>`); most logs are hunting streams. The two chosen are the
cleanest of those 39.

### `m2_model.xml` — Milestone 2's model golden

Added 2026-09-19. Unlike the four above it is read by **`cena-model`**, not by
the parser goldens: `crates/cena-model/tests/golden_model.rs` folds it all the
way into `GameState`. Every model test before it used hand-written snippets,
and `pbarStance` shipped in `vitals` past 17 green ones because of it.

Its source was chosen by scoring a 48-file sample across 8 characters for how
many M2 model families each carries; this is the **smallest** file scoring 12/12,
at 143 KB. It is raw wire with no Lich line prefixes, so the cut needed no
unwrapping — but `Scrubber` now strips `HH:MM:SS: ` anyway, so a future cut may
come from any file in the archive.

**Line 56 is in the cut for one reason**, and it is worth stating because the
first version omitted it: `<dialogData id='combat'>` carrying `pbarStance`. As
first cut this fixture carried only the `stance` dialog's copy, so re-introducing
the exact bug the file exists to catch left all 18 tests GREEN. Verified by
falsification, not assumed.

## What each fixture covers

- **`room.xml`** — `<nav rm=>`, `<streamWindow id='main'|'room'>`,
  `<clearStream>`, `<pushStream>`/`<popStream>`,
  `<compDef id='room desc'|'room objs'|'room players'|'room exits'|'sprite'>`,
  `<component id='room objs'|'room players'>`, `<a exist noun>`, `<d>`,
  `<compass><dir value>`, `<style id="roomName"|"roomDesc">`,
  `<resource picture>`, `<b>`, `<pushBold>`/`<popBold>`, and the `<a exist
  coord noun>` exit form. Also the **arrival/departure** pair, where
  `room players` goes non-empty then empty again.
- **`prompt.xml`** — `<prompt time="…">&gt;</prompt>`, including two
  consecutive prompts with only bold-wrapped text between them. The frame
  boundary.
- **`vitals.xml`** — `<dialogData id='minivitals'>` with `<skin>` and
  `<progressBar id='health'|'mana'|'stamina'|'spirit'>`, plus
  `<dialogData id='injuries'>` (`health2`) and `<dialogData id='expr'>`
  (`yourLvl`, `mindState`, `nextLvlPB`).
  **The parse trap this exists for:** the number lives in `text=`, not
  `value=`. `text='health 225/226'` with `value='99'`, and
  `text='spirit 8/9'` with `value='88'` — 88 is a percent, not 8.
- **`vitals_secondary.xml`** — `<dialogData id='expr'>` labels (`PTPs`,
  `MTPs`), `<dialogData id='encum'>` (`encumlevel`, `encumblurb`),
  `<dialogData id='stance'|'combat'>` (`pbarStance`), `<castTime value=>`,
  `<roundTime value=>`, `<left>`/`<right>`.

## Scrubbing

Enforced by `tests/fixtures_are_scrubbed.rs`, which goes RED on each hazard —
VERIFIED, one probe per rule. Applied here:

- `Inochi` -> `Alderin`, consistently, at both the `noun=` and text sites.
  `exist="-11047747"` is **kept**, so `exist=`-to-name correlation is still
  under test.
- CRLF normalised to LF; the Lich header line (`2025-11-29 10:03pm`) is not
  part of any cut range.
- No `druby://` and no `<vellumImg>` were present in the source ranges; the
  scrubber ran regardless, and the test asserts their absence rather than
  trusting the cut.

## M2's corpus — six shapes, cut 2026-09-19

Added for `plan/12` §8's Milestone 2 ("frame vocabulary breadth + golden
corpus"). Read by `tests/golden_m2_corpus.rs`, 17 tests.

**These come from a different source than everything above**, and that is the
point. The M1 fixtures are pre-2026-08 archive logs; these are
`E:\Gemstone\dev\lich-5\logs`, a **newer Lich build**, which is where
`crtrStatus` lives — absent from three sampled archive months, **2,342**
occurrences in one of these files.

| Fixture | Source | Lines | Bytes |
|---|---|---|---|
| `login_burst.xml` | `dev/lich-5/logs/GSIV-Nisugi/2026/09/xml/2026-09-01_15-13-56.xml` | 2-13 | 2,145 |
| `room_populated.xml` | same | 1682-1693, 240 | 2,456 |
| `combat_exchange.xml` | same | 1945, 1978-1992 | 2,064 |
| `creature_status.xml` | same | 15416, 1886, 2758 | 2,203 |
| `inventory_container.xml` | same | 14-20, 44-45 | 2,425 |
| `effect_dialogs.xml` | `…/2026-09-04_09-15-47.xml` | 822-825 | 6,414 |

Line 1 of each source is Lich's file header (`2026-09-01 3:13pm`), not wire
traffic, and is in no cut range.

### The prerequisite: a second timestamp prefix

**These logs were unusable as fixtures until `scrub.rs` learned a new rule.**
This build prefixes lines with a full date and a spelled-out zone:

```text
2026-09-01 15:13:58 Central Standard Time: <component id='room objs'>...
```

MEASURED: **4,427,147** occurrences across the month; in `2026-09-01_15-13-56`,
**28,879 of 29,122 lines**, every one line-anchored. VERIFIED that keeping it
yields a spurious `Text("2026-09-01 15:13:58 Central Standard Time: ")` frame
ahead of the real component — a fact the game never sent, baked into a golden.
The two prefix shapes are disjoint by era: of a 200-file archive sample, zero
carry either.

### `combat_exchange.xml` is one blob, and was first cut across it

A roundtime action's wire unit is a **blob**: a `<roundTime>`/`<castTime>` tag at
the front, the exchange, the `Roundtime: N sec.` prose at the back, terminated by
a prompt (the author, 2026-09-19; measured at **1,712 blobs across six files,
zero exceptions** — `plan/15` §2a.4b).

The first cut began at the exchange prose and so carried the close without the
open. The cut now spans the blob, **1945 + 1978-1992**: the tag, then the target
dialog and the full exchange.

**The middle is elided deliberately.** Lines 1946-1977 are a worn-inventory
refresh that Lich flushes mid-blob, already covered by
`inventory_container.xml`; the whole blob is **11,174 bytes**, over the 10 KB
per-fixture budget. VERIFIED the elision leaves no dangling stream: it takes the
`pushStream id='inv'` at 1948 along with the lines it wrapped, and that push has
no `popStream` in the blob at all — the prompt closes it, which is the
`StreamPopForced` resync already under test.

So the fixture is two contiguous regions of one blob, not two unrelated regions
spliced together, and the elision is recorded rather than silent.

### Scrubbing — eleven players, not one

`room_populated.xml`'s roster is a public arena: **eleven** players, several
behind titles (`Arena Icon`, `Captain of the Falcon`, `Legendary Lady`). All
eleven are pseudonymised in `cut_fixtures.rs` and asserted absent by
`fixtures_are_scrubbed.rs`.

They are pseudonymised rather than dropped because the line's **value is its
shape** — eleven links, some behind titles, which is the form a room roster
takes and which no hand-written snippet would get right. Titles are game data
and stay. `exist=` ids are **kept**, so id-to-name correlation is still tested;
each pseudonym appears exactly twice, at `noun=` and in the text.

VERIFIED absent from every committed fixture: all eleven real names, `Inochi`,
and the author's own character name.

### What the cut found

**A defect, not just coverage.** `<crtrStatus>` typed correctly standing alone
and degraded to `Frame::Structural { raw }` **inside a `<component>` body** —
every flag trapped in an unparsed string. MEASURED: of 2,568 lines carrying one,
**2,537 (98.8%)** carry it inside a component; the working path served 1.2% of
real traffic.

That is `plan/12` §3a's "reopen" signal — a classifier would have to re-tokenize
markup to recover a fact the parser already had — so the frame was widened at
the emit site. Four tests now fail if it regresses; before them, reverting the
fix left the whole suite green.

Also measured across all six: **267 frames, zero unknown or malformed tags**.
The ported 126-tag vocabulary has no hole in current traffic, which is M2's
breadth claim.

## Not in M1 scope

Deliberately absent, per the corpus findings: `<dialogData id='combat'>` as a
family (31,543 corpus hits, the noisiest tag), `Buffs`/`Debuffs`/`Cooldowns`,
`Active Spells`, `mapViewMain`, `<inv>`, `<openDialog>`/`<link>`/`<menuLink>`
quickbar, `<container>`. These are the bulk of the bytes and none of the slice.
