# Combat replay fixtures

`tests/fixtures/combat/*.txt` -- **71 files, 85 blobs, 192 KB** -- are Lich's
`spec/fixtures/replay/` copied **verbatim** on 2026-09-20 from the author's own
install (`C:\Gemstone\lich-5`, identical to `reference/lich-5` for these files).
They are the author's published test corpus for the combat module, under Lich's
GPL, and `replay_spec.rb` describes their curation:

> *a slim, curated slice of that corpus -- the single most-common real-feed
> shape per attack def, plus extra flare-bearing shapes -- ... scrubbed of
> volatile provenance.*

Each blob is one prompt-bounded chunk of real feed, markup intact, with a
header recording the facts Lich's own parser produced for it:

```text
##### attack 5110a6c49c3f7711
# expect: attacks=attack | res=as_ds | flares=ensorcell | outcomes=none | dmg=1 | statuses=
You swing a perfect mithril war-hammer at <pushBold/>a <a exist="37507821" noun="construct">greater construct</a><popBold/>!
  AS: +351 vs DS: +299 with AvD: +22 + d100 roll: +88 = +162
   ... and hit for 20 points of damage!
```

## The contract

Lich's, unchanged: the header is a **floor**. `dmg` matches exactly; every
named fact in the set fields must be present; extras are tolerated.

- `tests/combat_replay_lines.rs` reads the three line-level fields --
  `res`, `flares`, `outcomes` -- plus `attacks`, against the classifiers alone.
- The state-machine port will read all six against `parse_events`, which is
  what `replay_spec.rb` does.

## Not scrubbed, and why that is fine

`cena-protocol/tests/FIXTURES.md`'s goldens were cut from the private log
archive and scrubbed by `Scrubber`. These were not cut here: they are already
published in a public repository by their author, so copying them creates no
new exposure. MEASURED before copying: zero whisper lines, zero `<playerID>`
tags, nine distinct negative `exist` ids (players in view, as published).

## Size

Under the 10 KB per-fixture budget individually (largest: `fire.txt`, 6,921
bytes). The set is larger than the protocol goldens together because it is the
parity oracle for 329 definitions, and `plan/06` §1.4 asks for exactly this:
*"every session any developer plays becomes a test fixture."*

## Regenerating

Copy `spec/fixtures/replay/*.txt` from a current Lich. A file that exceeds one
chunk (200 lines) is refused by `tests/combat_bench.rs` and must be split.
