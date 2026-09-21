# 20 — Milestone 3, continued: the remaining attribute surface

**Status: DRAFT, unapproved.** The feature list is the author's, given 2026-09-20. The
ordering, the dependency edges and the per-feature counts below are proposals and
measurements; the order is a recommendation until the author says otherwise.

This is **not a new milestone.** `plan/12` §8 puts the character model at M3, and every
feature here is part of it. `plan/18` §3 deferred exactly this surface out of M2:
*"stats, skills, PSMs, society, bounty — text-scraped; needs the Infomon sync"*. Most of
that list is now built; this document covers what the author named on top of it.

---

## 0. The list, and where each item actually stands

The author named nineteen: resources, spells, spellsong, messaging, stash, stowlist,
readylist, stance, spellranks, overwatch, mana, group, gift, fog, experience, disk,
currency, bank.

Two audits established the state of each — one over `crates/`, one over
`reference/lich-5`. Every claim below carries a file:line from one of them.

| Feature | Cena today | Lich source | Lines | Parse rules |
|---|---|---|---:|---|
| mana | **DONE** (`state/vitals.rs`) | `gemstone/mana.rb` + `common/xmlparser.rb:706` | 43 | 4 (the PULSE verb only) |
| stance | **DONE**, as a string | `gemstone/stance.rb` + `common/xmlparser.rb:703` | 172 | 4 (setting it) |
| experience | **DONE** (`character/experience_report.rs`) | `gemstone/experience.rb` | 109 | ~6 (`parser.rb:16-21`) |
| spells | PARTIAL — `Effects`, no spell list | `attributes/spells.rb` + `common/spell.rb` | 76 + 954 | 2 + hundreds of up/down msgs |
| spellranks | PARTIAL — `SkillLine::SpellCircle` | `gemstone/spellranks.rb` | 80 | 1 (`parser.rb:24`) |
| resources | **DONE** (`character/standing.rs`) | `attributes/resources.rb` | 36 | 7 (`parser.rb:51-58`) |
| stow (container) | PARTIAL — the container, not the list | — | — | — |
| society, citizenship, warcries | **DONE** (`character/standing.rs`) | `infomon/parser.rb:36-44` | — | 11 |
| currency | **DONE** (`character/currency.rs`) | `gemstone/currency.rb` | 110 | ~15 (`parser.rb:63-77`) |
| bank | ABSENT | `gemstone/bank.rb` | 389 | ~20 (`bank.rb:22-75`) |
| disk | ABSENT | `gemstone/disk.rb` | 60 | 1 (a noun list) |
| group | ABSENT | `gemstone/group.rb` | 665 | ~10 (`group.rb:418-450`) |
| stowlist | ABSENT | `gemstone/stowlist.rb` | 78 | 4 (`infomon/xmlparser.rb:515`) |
| readylist | ABSENT | `gemstone/readylist.rb` | 96 | 8 (`infomon/xmlparser.rb:521`) |
| stash | ABSENT | `stash.rb` | 642 | ~24 inline |
| gift | **DONE**, pulses only — see step 3 | `gemstone/gift.rb` | 54 | **0** — ticks off the exp bar |
| fog | ABSENT | `gemstone/fog.rb` | 269 | 1 — watches `room_id` |
| spellsong | ABSENT | `attributes/spellsong.rb` | 190 | 1 — the rest is arithmetic |
| overwatch | ABSENT | `gemstone/overwatch.rb` | 256 | **~30** (`overwatch.rb:132-179`) |
| messaging | **N/A — see §1** | `messaging.rb` | 148 | 0 — it is output-side |

### 0a. Two corrections this audit forced

**`mana` and `experience` were reported DONE in conversation and were not.** Vitals held
a bare percent under a string key, so "can I afford this spell" was unanswerable from
`GameState`. Fixed in `e0e5949`: `Vital { percent, current, max }` plus named accessors.
The lesson is `plan/05` §−2's, in its usual form — the claim came from a script-count
grep rather than from reading the type.

`experience` is still partial and the entry above says so: the `expr` dialog bars only,
with `level` stored as a verbatim string. Field exp, fame, deeds and ascension are not
modelled.

**`gift`, `fog` and `overwatch` were called evidence-free and are not.** Gift is driven
structurally off the experience progress bar (`common/xmlparser.rb:751`), fog watches
`room_id` change, overwatch is ~30 regexes over creature links. Only the `resource`
command still wants a fresh capture before porting.

---

## 1. Messaging is not a port

`lib/messaging.rb` (148 lines) **emits** XML to a client: `<pushStream id=…>`,
`<output class="mono"/>`, `monsterbold`, `make_cmd_link` (`messaging.rb:23` branches on
`XMLData.game =~ /^GS/`). It parses nothing.

That job belongs to Cena's frontend, and `plan/12` §2 puts it in `cena-ui`. There is no
model work here and this document does not schedule any.

**What the author may have meant is a different, real gap.** Speech, thoughts and whispers
arrive as untyped `Runs` in an untyped stream-id map (`state/streams.rs:69` `LineTally`), with
`thoughts` special-cased at `:35-44` and `:147`. There is no `Message` type carrying sender, channel
and body, and nothing handles `espMasterData`. That is **§3 step 8**, and it is worth
confirming with the author which of the two was meant.

---

## 2. The substrate under nine of them

Lich's shape is one key-value store (`Infomon`) fed by one line scanner of ~108 regex
constants (`infomon/parser.rb`, 839 lines). Nine features are thin accessors over it:
resources, spells, spellranks, experience, currency, and parts of bank, spellsong and
stash. `attributes/resources.rb` is 36 lines because all the work is upstream.

**Cena already has the typed half of this.** `state/character/blocks.rs` consumes
command-output blocks, and `Character` holds the typed result. What is missing is the
scanner: a first-match table of line patterns, in the shape `state/combat/defs.rs`
already uses, whose output lands in typed fields rather than `get("key")`.

That is why the order below starts there. Seven features become cheap once it exists,
rather than seven separate ports.

> **C21 still governs the shape** (`research/04-inherited-decisions.md:1878`): typed named
> fields for closed vocabularies, `BTreeMap` + `is_known` only for genuinely open sets,
> and **no generic `state.get("key")` in the public surface.** The scanner is an internal
> mechanism; nothing above the model sees a string key.

---

## 3. Build order, and why

The dependency edges are the constraint, not the demand numbers. From the Lich audit:

```text
stash   -> readylist, stowlist
bank    -> currency, stowlist
group   -> disk
gift    -> experience
fog     -> spells
spellsong, mana, stance -> spells
```

Demand is the tiebreak. MEASURED over the 234 GemStone scripts in `reference/scripts`:

```sh
cd reference/scripts/scripts && grep -li "<term>" *.lic | wc -l
```

| mana | stance | silver | spells | disk | exp | group | resource | fog | stash | stow/ready | spellsong | overwatch |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 97 | 72 | 69 | 65 | 29 | 24 | 21 | 19 | 11 | 8 | 7 | 3 | 2 |

**This corpus measures a scripting API Cena does not have**, so it is evidence of what
players care about, not of what to build. It breaks ties; it does not set the order.

### The steps

> **STEP 1 WAS SKIPPED, AND STEPS 2-4 ARE DONE WITHOUT IT** (2026-09-20, author's call
> after the divergence was raised). Read step 1 as **not the entry point**; what follows
> records why, so this document does not say "start here" beside code that started
> elsewhere.
>
> **What was built instead.** Each feature owns its own matchers, as `strip_prefix` chains
> over a reassembled line: `state/character/standing.rs` (society, citizenship, warcries,
> resources, PSM changes, covert arts), `state/character/currency.rs` (sixteen balances),
> and `state/character/experience_report.rs` (the `experience` block). MEASURED:
>
> ```sh
> grep -c "strip_prefix\|strip_suffix" crates/cena-model/src/state/character/{standing,currency,experience_report}.rs
> # 19, 17, 1
> ```
>
> **Why that is not the duplication this step feared.** The shape step 1 points at is
> `state/combat/defs.rs`, and reading it settles the question: it is a **regex table with
> a `RegexSet` gate**, and the gate exists because that table is 957 regexes over three
> TSVs, where it MEASURED `105 us/line -> 3.6 us/line` and the `attack` family alone went
> `42.6 -> 0.9` (`defs.rs` module docs). Thirty-seven prefix tests inherit none of that
> pressure.
>
> (957 measured: `wc -l crates/cena-model/data/combat_{attacks,results,effects}.tsv`. An
> earlier draft of this note said ~2,400, which is `CritTables`' pattern count from
> `combat/tracker.rs:85` -- a different table, restated from memory. §-2 again.) `strip_prefix` is faster than a regex, and each call
> reads as the literal line it matches -- which is what let every rule here be
> mutation-tested individually: the silver word forms, the singular suffixes, the `Fame:`
> opener, the indent requirement on society reports.
>
> **What the prediction got right and wrong.** Right: these features *are* one shape, and
> the fourth through seventh (bank ~20 rules, stash ~24, stowlist 4, readylist 8) should
> follow the same one rather than inventing a third. Wrong: the shape is not a shared
> table. A table would have moved each rule one indirection away from the line it matches,
> for a saving that is real only at combat's scale.
>
> **The upgrade trigger, stated so this is a decision and not a drift.** If a feature
> arrives whose rules need *captures* rather than a prefix and a suffix -- overwatch's ~30
> regexes over creature links are the candidate (step 9) -- that feature builds the table,
> and it builds it for itself first. Retrofitting the three done features onto it needs a
> reason beyond symmetry.

1. ~~**The line scanner.**~~ **SKIPPED — see the note above.** Extend
   `state/character/blocks.rs` with a first-match pattern table in `defs.rs`'s shape.
   Unlocks 2, 4, and the rest of 3.
2. ~~**Resources.**~~ **DONE**, in `state/character/standing.rs` — the `resource` and
   `Suffused` lines, plus Covert Arts charges, which Lich reads (`parser.rb:54`) and
   `plan/20` did not list. 7 rules. `ResourceType` already existed
   (`state/character/vocabulary.rs:176-233`) with no parser and no consumer.
3. ~~**Experience, completed.**~~ **DONE**, in
   `state/character/experience_report.rs`. ~6 rules for fame, LTE, deeds, ascension, plus
   the two numbers Lich matches and discards (`Experience:`, `Recent Deaths:` --
   `parser.rb:17-18`). Then **gift**, which is 54 lines of timer with zero regexes once
   experience ticks — **ported as the pulse count only.** MEASURED: `Gift.pulse` has one
   live caller and `started`/`ended`/`serialize` are called only from Lich's own specs, so
   its `remaining()` is right only for someone who launched Lich the instant their gift
   began. The arithmetic (360 minutes, restarting 594,000s later) is recorded against the
   day the wire is found to state a start time.
4. **Currency**, ~~then bank.~~ Currency is **DONE**, in
   `state/character/currency.rs`: sixteen balances, ~15 rules. **Bank is still open** and
   still needs step 6's stow default (`bank.rb:238`), so it is sequenced after 6 rather
   than stubbing that field.
5. **Disk, then group.** Disk is one noun list (`disk.rb:4`). Group is ~10 patterns over
   creature links and calls `Disk.find_by_name` (`group.rb:95`). Both are what an
   eohunter port will want first.
6. **Stowlist and readylist, then stash.** 4 + 8 patterns, then stash's ~24. All three
   are XML-in-text scraping, which in Cena reads off runs rather than re-matching tags —
   the §3a bargain, as with the combat defs.
7. **Spells: the prepared spell and the spell table.** `Frame::Spell` is parsed and
   `GameState` has no arm for it (VERIFIED: `grep -n 'Frame::Spell' crates/cena-model/src/state.rs` is empty). Then **spellsong** (190 lines,
   almost all arithmetic) and **fog** (269 lines, almost all logic).
8. **Typed speech and thoughts.** See §1. Independent of everything above.
9. **Overwatch.** ~30 regexes, 2 scripts of demand. Last of the real work.

### Deliberately not scheduled

- **`stance` as an enum.** It is a verbatim string today and the five stances are a closed
  vocabulary, so the typed version is half an hour — but nothing needs it until a
  behavior sets stance, which is M6.
- **The `resource` command capture.** Everything else here has wire evidence in the
  corpus or in Lich's regexes. This one wants a real sample first.

---

## 4. Verification

Unchanged from M3's existing bar, and it is the bar these steps are judged against:

- **Golden fixtures cut from the corpus**, not hand-written snippets. `plan/18`'s lesson:
  `pbarStance` shipped in `vitals` past 17 green tests because every model test used
  snippets.
- **Mutation-test every new classifier.** The session standard, and it has paid twice in
  this milestone's neighbours: one survivor in the creature registry was a real missing
  test, and two in the recorder were real gaps in the stun-pair rules.
- **Guard-before-clear** on every invalidation test: assert the fact was known first, or
  the test passes without the code existing.
- **Each step leaves the tree green**, with `cargo clippy --workspace --all-targets` and
  the arch tests clean. The caps are enforced on every run, so **move code down** rather
  than raising one — `Vital` moved to `state/vitals.rs` for exactly this reason,
  taking `state.rs` from 582 back to 512 (`e0e5949`).

---

## 5. Divergence log

### Step 6 found a parser defect, not just a port (2026-09-20)

Porting `stow list` and `ready list` turned up a **Rule 2.2a violation in the
parser**, which is why step 6's commit touches `cena-protocol`.

`Parser` keeps a stack of open `<a>`/`<d>` links and surfaced `links.first()` --
the **outermost**, ported from Vellum (`src/parser/text.rs:97-106`). That is right
for what a *click* sends. But `ready list` wraps each item in the command that
would clear it:

```text
  weapon: <d cmd="store WEAPON clear">a <a exist="208924336" noun="katar">...</a></d>
```

**There is one clickable region here, not two.** The katar's own text is what
the player clicks, and clicking it sends `store WEAPON clear` (author,
2026-09-20). The `<d>` is not a separate widget wrapping an object; it is the
command attached to that object's link.

So the markup states two facts about the same span -- what the text **names**,
and what clicking it **sends** -- and the parser kept only the second. The inner
link's `exist` and `noun` reached no consumer at all: the model losing what the
parser preserved.

An earlier draft of this note described the `<d>` and the `<a>` as two different
targets. They are not, and the distinction matters for what `links` means to a
consumer: it is not "the outer thing" but "what a click does".

MEASURED in `E:/Gemstone/dev/lich-5/logs` (208 live XML files): **322 nested
`<d>...<a exist>` occurrences across 62 files.** Not an edge case. Zero occur in
the 12 committed fixtures, which is why the golden corpus did not catch it.

The fix ADDS a fact rather than replacing one: `TextFrame::inner_link` and
`Run::inner_link` carry the innermost `exist` when the outermost link is not it,
`link` still carries the click, and `object()` / `Runs::objects()` /
`ChunkLine::objects()` resolve the two. Pinned by
`fixed_defects.rs::an_object_nested_inside_a_clickable_command_survives`, which
goes red on the pre-fix behaviour.

**How it was found:** the classifier reported no item for a row that plainly had
one. A test written from Lich's regex rather than from the wire would have
reproduced the same blindness -- the regex re-tokenizes the raw XML, so Lich
never depended on the parser surfacing the inner link.

### Two test forms were corrected by real wire

`ReadyListNormal`'s optional `\(?` hides that a **set** row and an **unset** row
differ:

```text
  weapon: <d cmd="store WEAPON clear">a <a exist=...>...</a></d> (<d cmd='store set'>put in sheath</d>)
  shield: (<d cmd='ready SHIELD'>none</d>) (<d cmd='store set'>put in sheath</d>)
```

The first draft of `tests/containers.rs` parenthesised both, a shape the wire
never sends. Corrected against `2026-09-01_10-04-51`, which also shows
`ammo2 bundle` and `secondary sheath` carrying no store-mode at all.

### Lich defects found, and what was done about each

| Defect | Source | Ported? |
|---|---|---|
| `store_*` holds a raw string, and `ready list` and `store set` spell the same three states differently (`worn if possible, stowed otherwise` vs `worn if possible and stowed if not`) | `xmlparser.rb:522` vs `:526`, both writing the same key at `:605`/`:617` | **No** -- `StoreMode` is an enum reading both vocabularies |
| Clearing a ready slot leaves its store-mode behind, so an empty slot still reports where its item would go | `xmlparser.rb:611-613` writes only the item | **No** -- fixed |
| `StowList.checked` is set by any list ROW, so a list cut off mid-way reads as whole | `xmlparser.rb:594` | Kept for stow (a row IS the list's evidence); `ReadyList`'s better rule -- set only by the closing line (`:609`) -- is what `ready_checked` follows |

`valid?` (`stowlist.rb:45`, `readylist.rb:59`) is **deliberately not ported**: it
re-checks held ids against `GameObj.inv`, which is cache coherence and belongs to
whoever owns inventory, not to a record of what the game said.
