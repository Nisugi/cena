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

## Not in M1 scope

Deliberately absent, per the corpus findings: `<dialogData id='combat'>` as a
family (31,543 corpus hits, the noisiest tag), `Buffs`/`Debuffs`/`Cooldowns`,
`Active Spells`, `mapViewMain`, `<inv>`, `<openDialog>`/`<link>`/`<menuLink>`
quickbar, `<container>`. These are the bulk of the bytes and none of the slice.
