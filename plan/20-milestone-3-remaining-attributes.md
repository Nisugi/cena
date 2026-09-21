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

**`cmd` -> `exist` is the only nesting the wire uses.** The obvious worry about
the fix is the mirror case -- a `<d cmd=>` inside an `<a exist=>` would lose the
command the same way. MEASURED by walking a tag stack over every `<a>`/`<d>` in
all 208 files:

```text
       outer -> inner        count
         cmd -> exist          322
```

That is the whole table, at any depth. So one `Option` is enough and the mirror
case is pinned by a test rather than designed for.

**How NOT to measure this.** Two `grep -P` passes with `(?:(?!</a>).)*?`
lookaheads returned **0** for *both* directions, including the one that occurs
322 times -- the regex was failing, not the data. A zero from it would have read
as "this shape does not occur", which is the same false-negative hazard
`CLAUDE.md` records for the `styleIfClosed` citation: a query that resolves to
nothing does not fail loudly, it manufactures evidence of absence. The count
above comes from a tag-stack walk, which cannot fail that way.

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

---

### Step 6b: stash is two files wearing one name (2026-09-20)

`plan/20`'s audit above budgeted stash as **"~24 inline"** patterns. That count
is real and the word *inline* was the warning. MEASURED over `stash.rb`'s 642
lines and 27 entry points: **45 send/wait/retry calls**, and **not one
classifier**. Every regex in the file is either a terminator for an
`issue_command` wait (`:28`, `:98`, `:165`) or a guard inside a retry loop
(`:46`, `:55`, `:68`). There is no line the game sends that stash reads for a
fact — it sends commands and watches for the reply to stop.

Splitting by whether a function sends anything:

| | Functions | Lines | Where it belongs |
|---|---:|---:|---|
| **Resolution** — "which item does this name mean?" | 13 | ~140 | `cena-model`, **built** |
| **Manipulation** — get it, wear it, swap hands, retry | 12 | ~460 | `cena-behavior`, **M6** |

The manipulation half needs the authority token and roundtime, and would put
`fput` in a crate with no socket — which the crate graph forbids anyway. It is
deferred to M6 rather than skipped, and this table is the record of what is
outstanding.

The resolution half is `state/resolve.rs`: `find_items` / `find_item` /
`hand_holding`, ordered by `Specificity` then `Location`, which `stash.rb:309`
records as *"the order the game itself resolves a bare noun in"*.

**A Lich bug not ported.** `find_container` (`stash.rb:13`) interpolates the
caller's string straight into a regex, so a bag named `pack (old)` raises
`RegexpError`. Lich fixed exactly this for items — `name_matches?` (`:605`)
escapes, and its comment says why — and never gave `find_container` the same
treatment. Here matching is over words, so nothing is ever compiled.

### Two more Rule 2.2a losses, both found by building on top

Neither was in any plan; both were found because a consumer needed a fact and
could not get it.

**1. The hands dropped their `exist` id.** `<left>`/`<right>` carry
`exist=`/`noun=` exactly as an object link does, `Frame::LeftHand` preserved
them as a `link`, and `GameState::apply` matched `{ item, .. }` — keeping the
display text and discarding the id. Found when `hand_holding(id)` could not be
written. Fixed by `state/hands.rs`'s `Hand` enum, which also makes
`Empty` (MEASURED: 2,806 `<right>Empty`, 2,048 `<left>Empty`) distinct from
"the game has not said", per §5.2.

**2. A nested link's own text was always empty.** The `inner_link` fix from
step 6a surfaced the inner `<a exist=>`, but display text accumulated only onto
`links.first_mut()` — the outermost — so the inner link carried an id with an
empty `text`. `ready list` resolved to `ItemRef { id: "333", text: "" }`, and an
id with no name is useless for the one case the field exists for: telling two
katars apart.

**How it was caught matters.** Not by the four tests written for the nesting
fix — every one of those asserted the id and none asserted the text. It was a
`guard:` assertion in an unrelated ordering test, added only because a previous
draft of that test had conflated two rules. The guard-before-assert habit found
a bug in code that had already shipped green.

---

### Step 7: bank, and a real balance the port cannot see (2026-09-20)

Same split as stash, but bank genuinely has a `Pattern` module. MEASURED over
its 18 entry points: **10 send/wait, 8 pure.** The verbs — `deposit`,
`withdraw`, `deposit_f2p`, the Pinefar banker dance — go to `cena-behavior` at
M6. What is built is `state/bank.rs`: the account listing, the note value, and
`local_bank`.

**`ACCOUNT_LINE` (`bank.rb:69`) cannot read one of the author's own banks.**

```text
/^\s+(?<bank>.+?) Bank: (?<silver>[\d,]+)$/
```

requires the name to **end** in `Bank`. VERIFIED against
`2026-09-01_15-12-27`, run through Lich's actual pattern:

```text
     First Elanith Secured Bank: 586,836,811     matches
             Icemule Trace Bank: 3,800,267       matches
      Vornavis Bank of Solhaven: 10,627,060      NO MATCH
                Four Winds Bank: 425,802,149     matches
             Kraken's Fall Bank: 26,743,838      matches
                          Total: 1,053,810,125
```

`Vornavis Bank of Solhaven` has `Bank` in the middle. **10,627,060 silver is
invisible**, and the consequence is worse than a missing row: `balance` falls
back to `banks[local_bank(...)] || 0` (`:125`), so a character standing in
Solhaven reads **zero** while holding 10.6 million.

Two design consequences here:

- The separator is the **last** `": "`, which is what the fixed-width layout
  guarantees. No bank name has to be any shape.
- The `Total:` line is **read, not summed.** Lich sums the rows it parsed
  (`:126`), which is exactly why its dropped row goes unnoticed — the sum is
  self-consistent and wrong. Keeping the game's figure makes the disagreement
  expressible, which is what `Account::agrees()` reports.

Keeping the whole name costs one thing: `local_bank` matches against the room's
location, and the room says `Four Winds Isle` where the row says
`Four Winds Bank`. Lich gets the stripping for free because its capture stops
before ` Bank`. Here it moves into `town_of`, the one place that needs it,
rather than being baked into the stored fact.

**UNVERIFIED, and marked so:** the free-to-play lines (`ACCOUNT_BALANCE`,
`ACCOUNT_MAX`, the single-account opener) do not appear in the corpus — the
author's account is not free-to-play. Ported from the regex rather than left
out, because a capped account reading its balance as unknown would be worse.

#### A guard that was decoration, and the test that was too

Mutating the indentation guard away left the suite **green**. The test meant to
pin it asserted only that the real listing yields 5 rows — and none of the real
prose lines happen to contain `": "`, so the guard was never reached. That is
`plan/05` §0 exactly: a written justification with nothing asserting it.

Writing a test that *did* reach it (a teller quoting `Your limit is: 50,000`)
found a **second hole the guard did not cover**: the `Total:` arm ran before the
indentation check, so an unindented `Total: 9` was read as the listing's total.
Both are now guarded, and the mutation is caught.

The lesson is not "mutation testing works" — it is that a test can pass a
mutation because the *input* never reaches the code, not because the assertion
is wrong. A green mutation run says look at the input, not just the assertion.

---

### Step 8a: spellsong, and what `spells.rb` turned out to be (2026-09-20)

**`spells.rb`'s circle ranks are already done.** Its 76 lines are Infomon
accessors for spell-circle ranks, which Cena reads as circle rows of the
`skill` table (`SkillSet::circle`, from `plan/18`).

> **CORRECTED 2026-09-20, by the author.** An earlier version of this note said
> *"plus `Spell.active` which is `Effects`"*, as though the two were the same
> thing and the item were therefore closed. They are not.
>
> *"Spell.active came before Effects, Lich still uses it for some things that
> haven't made their way into Effects yet such as briar betrayer. There's also
> tracking of third party cooldowns."* — the author
>
> VERIFIED. `ActiveSpell.update_spell_durations` (`infomon/activespell.rb:131`)
> reconciles the dialog feed against `Spell.active` and carries an explicit
> exemption list:
>
> ```ruby
> ignore_spells = ["Berserk", "Council Task", "Council Punishment",
>                  "Briar Betrayer", "Rapid Fire Penalty"]
> ```
>
> Those five are held in `Spell` and **never reported by the dialog feed**, so
> without the exemption every sync would tear them down. MEASURED over the 208
> live XML logs: `Briar Betrayer`, `Council Task`, `Council Punishment` and
> `Rapid Fire Penalty` appear **zero** times; `Berserk` appears 90 times and
> **zero** of those are inside a dialog. `Effects` is therefore a *subset* of
> what `Spell` tracks, not a replacement for it.

**`Spell` is the reader for a data file, and the data file is on disk.**
`common/spell.rb`'s 954 lines parse `data/effect-list.xml` — **515 spells, 633
messages** (MEASURED at `C:/Gemstone/lich-5/data/effect-list.xml`). That makes
it the same shape as the crit tables and the bestiary: static game knowledge
that `plan/13` §4a says ships as a data file. It is a port worth doing, not a
blocker to route around.

**Third-party cooldowns** (PR #1597, merged and present in this clone) track
which *other characters* a spell has locked out. Five spells declare one:
>
> | Spell | Kind | Seconds |
> |---|---|---:|
> | 140 Wall of Force | target | 270 |
> | 211 Bravery | group | 180 |
> | 215 Heroism | group | 180 |
> | 219 Spell Shield | group | 360 |
> | 506 Celerity | target | 240 |

**The two kinds are two cast mechanics, not two data sources** (author,
2026-09-20): *"the ones looking at member are ones that are self cast but
affect your group."*

| Kind | How it is cast | How the target is learned |
|---|---|---|
| `group` | **on yourself**, lands on everyone grouped | it is not — stamp all members |
| `target` | at one character | the `target-start` message names them |

VERIFIED as a structural invariant, not a coincidence of five samples: across
the whole table, **every `target` cooldown has a `target-start` message and no
`group` cooldown has one.** There is nothing to capture in a self-cast, because
no name appears in the line.

That is also why the group case is optimistic. The caster's only notice is a
clause inside their own start message — each of the three is one pattern with
`your group` as an optional alternation:

```text
211 Bravery       You (?:and your group stand tall and )?feel more confident\.
215 Heroism       A brilliant aura surrounds you and (?:sinks into your skin|your group)\.…
219 Spell Shield  An opalescent aura surrounds you(?: and your group)?\.
```

and `parser.rb:662` tells the two views apart with `line.include?('your group')`.

**506 Celerity carries both**, which is the case that proves the split is about
mechanics rather than about the spell: self-cast with `and your group` it starts
group cooldowns, cast at someone else its `target-start` names them.

`Group.spell_cooldowns` keys by spell number then member **noun**.

Two things in that port are worth carrying over verbatim, both recorded in
Lich's own comments: a member still locked out is **skipped rather than
re-stamped** (`group.rb:201`), so their own cooldown keeps running; and the
recorder uses `_members` rather than `members` because it runs on the parser
thread and `members` would send `GROUP` and block waiting for a reply only that
thread can parse (`group.rb:198`).

Not scheduled here — it needs the spell table first — but it is a real feature
with a live consumer, not a leftover.

**`spellranks.rb` is not a classifier at all** — it is a `Marshal` cache of
*other characters'* ranks on disk, superseded by this project's own
persistence. Not ported.

**`spellsong.rb` is 190 lines of arithmetic and one `nil` guard**, exactly as
the audit said. Every input already exists in the model. Built as
`character/spellsong.rs`.

Two curves are involved and **both were verified exhaustively against a
transcription of Lich's own code** rather than spot-checked, because the
rewrite folds each band's running total into its constant — the transformation
that goes wrong at a boundary:

| Curve | Range checked | Disagreements |
|---|---|---:|
| `base_duration` | levels 0–100 | 0 |
| `to_bonus` | ranks 0–400 | 0 |

`to_bonus` is `attributes/skills.rb:9`, not spellsong's own, and is placed here
because spellsong is its first caller (Rule −1; it moves when a second appears).

#### Four differences, each deliberate

1. **Above level 100 Lich returns 120.** `duration_base_level` has no band past
   100, logs *"unhandled case"* and falls through to the bare base
   (`spellsong.rb:54`) — so a level 101 bard gets a level 0 bard's song. The
   bands are cumulative and the last continues cleanly, so it is extended.
2. **The skill table's own bonus beats the reconstruction.** Lich calls
   `Skills.to_bonus(Skills.elair)` unconditionally (`:76`), discarding the
   figure the game stated. The two disagree whenever an enhancive is on: the
   table's bonus includes it and the curve cannot.
3. **`mirrors_dodge_bonus` saturates.** Lich's `20 + ((bard - 19) / 2)` goes
   *negative* below 19 ranks. Unreachable in play, but a number that means
   nothing should not propagate.
4. **`luck_cost`'s second term keeps Lich's arithmetic.**
   `(6 + ((bard - 6) / 4) / 2).round` applies `/ 2` to the inner quotient only,
   so the renew cost is `6 + over/8`, not `(6 + over/4) / 2` as the parallel
   with every other `*_cost` implies. **Ported as written** — "fixing" it would
   be a guess at the game's real number. Pinned by a test so the choice is
   deliberate and a later measurement has something to change.

`holding_targets` is a fifth case worth recording as *not* a difference: Lich's
`1 + ((bard - 1) / 7).truncate` is correct at zero ranks only because Ruby
truncates toward zero. The same expression in a flooring language gives `0` — a
holding song that holds nobody.

#### Deferred to M6 with the rest of the behavior half

`timeleft` reads a process-global `@@renewed` timestamp a *script* sets when it
renews — bookkeeping owned by the renewer, not a fact about the character.
`renew_cost` sums `song.renew_cost` over nine spell numbers, which needs the
unported spell table. The per-song constant costs are here; the summing is not.

---

### The spell table, added to the list (2026-09-20)

Not in this document's original nineteen, and it should have been. Added
because the `Spell.active` correction above showed it is a real port with live
consumers rather than a dependency to route around.

**`data/effect-list.xml` → a TSV, the way `plan/13` §4a specifies.** MEASURED
at `C:/Gemstone/lich-5/data/effect-list.xml`: **515 spells, 633 messages**, and
per spell a number, name, circle, type, availability, mana cost, durations by
cast type, up/down messages, and — for five of them — cooldowns.

| Depends on it | Why |
|---|---|
| `Spells.get_circle_name` | the 1→`Minor Spirit` vocabulary |
| `Spells.require_cooldown` | reads `Spell[num + 1]` for Aspect cooldowns |
| `Spellsong.renew_cost` | sums `song.renew_cost` over nine spell numbers |
| third-party cooldowns | the `<cooldown>` and `target-start` elements |
| the five `ignore_spells` | things `Effects` does not carry at all |

The shape is the same as the crit tables, the bestiary and the armament
aliases: static game knowledge extracted once by a tool under
`crates/cena-model/tools/`, shipped as data, read by a typed lookup. The
extractor is the work; the table is not hand-transcribed.

**Sequencing:** it wants doing before `fog` only if fog turns out to need it
(`fog.rb` watches `room_id`, so probably not), and definitely before any of
the five consumers above.


---

### The `type=` tag is free text, and does not say what a spell does (2026-09-20)

I asserted Heroism's `type=offense` in a test as though the label settled what
the spell is. It does not.

> *"heroism provides an offensive bonus, so probably a utility spell, it does
> not damage on it's own so it's not an attack spell, which may be different
> than offense/defense."* — the author

MEASURED over the 514 spells, and the data agrees on both counts.

**It is not a vocabulary.** 19 distinct values, slash-separated, with the same
idea spelled more than one way and in more than one order:

| | |
|---|---|
| `offense` (26) | `offensive` (2) |
| `offense/utility` (1) | `offensive/utility` (1) |
| `attack/utility` (12) | `utility/attack` (1) |
| `defense/utility` (5) | `utility/defense` (1) |

That is why `Spell::kind` is a `String`. C21 reserves typed fields for **closed**
vocabularies; this is an open, inconsistent tag list, and a typed enum over it
would either lose values or go red on a Lich data update.

**`offense` and `attack` are different axes.** An `offense` spell improves your
offence; an `attack` spell does damage:

```text
offense spells carrying an AS/CS bonus:  23 of 34
attack  spells carrying an AS/CS bonus:   2 of 136
```

Every `offense` spell that carries bonuses carries attack-*strength* modifiers
— `bolt-as`, `physical-as`, `*-cs` — and none deals damage on its own. So a
behavior asking *"will this hurt something"* must not read `offense` as yes,
and one asking *"will this make me hit harder"* must not read `attack` as yes.

**What to read instead is the `bonuses` column**, which states what is actually
conferred. The tag is a curator's note; the bonuses are the fact.

---

### The `type=` tag IS a closed vocabulary (2026-09-20)

Reversing the note above. I wrote that 19 distinct values meant an open set and
`kind` should stay a `String`. The author:

> *"nothing upstream will be affected... this is a new thing remember! We are
> only porting it once, this list doesn't change often at all ... so we have
> attack, utility, offense, defense. attack would be like a bolt spell or
> warding spell, utility would be like floating disk, water walking, offense
> would be like heroism, defense would be like 618."*

Two errors in what I wrote. **I was treating a one-time port of a near-static
list as though it were a live feed needing defensive handling** — the "a future
regeneration might add a tag and my rewrite would eat it" argument is real for
a streaming source and close to irrelevant here. And the 19 values are not 19
categories; they are **six tags** joined with `/`, spelled two ways and ordered
two ways.

So `Role` is typed, per C21. `attack` / `offense` / `defense` / `utility` are
the author's four, plus:

- **`timer`** (76) — cooldowns, penalties, recoveries. Not a spell you cast.
- **`bonus`** (11) — kept as its own variant rather than folded into offense or
  defense, because it is genuinely mixed: `REIM Attack Boost` confers
  `bolt-as`, `REIM Defense Boost` confers `bolt-ds`, and seven of eleven confer
  nothing the table records.

**`area` is a modifier, not a role.** All five occurrences pair with `attack`,
so it qualifies how an attack lands rather than naming what a spell is for.
`Spell::is_area()` asks it separately.

Order is discarded because Lich's only consumer is `@type =~ /attack/i`
(`spell.rb:700`), a substring test deciding whether to append `target` to a
cast command. `Spell::unreadable_roles()` reports any tag the enum cannot read,
so a seventh category is a red test rather than a silent loss.

### Two copies of effect-list.xml, differing by exactly the feature under test

The author asked which file I had been reading. MEASURED:

| Copy | Date | Bytes | Cooldowns |
|---|---|---:|---:|
| `C:/Gemstone/lich-5/data` | Sep 13 | 230,659 | **5** |
| `E:/Gemstone/dev/lich-5/data` | Sep 11 | 230,225 | **0** |

`diff` shows the two files differ **only** by the five `<cooldown>` elements
and the two `target-start` messages. The older one predates PR #1597.

I used the newer copy, and picked it without comparing — by luck. Cut from the
older one, `with_cooldowns()` returns empty, and
`the_two_cooldown_kinds_split_on_cast_mechanics` **passes vacuously over zero
rows**: a green suite reporting a feature that is not there.

Fixed in three places: the TSV header now records the absolute source path, its
mtime and a cooldown count; `the_table_was_cut_from_a_copy_that_has_cooldowns`
asserts the five exist; and the invariant test counts what it checked and fails
if that is not 2. VERIFIED by regenerating from the Sep 11 copy — **six tests
go red**, where previously the suite would have stayed green.

---

### Source and data files age independently (2026-09-20)

The rule behind the `effect-list.xml` near-miss, stated by the author:

> *"reference/lich-5 should be pulled from upstream so it should be about the
> same as c:\gemstone\dev\lich-5, doesn't mean the data files are the newest."*

`reference/lich-5` tracks upstream, so its `.rb` is current. **A data file
sitting beside that source can be any vintage**, because it ships with a
release rather than with a commit. That is precisely how the Sep 11 checkout
had current-looking code and an `effect-list.xml` predating PR #1597.

MEASURED — the clone has **no `data/` directory at all**, so every data file in
this port came from a live install:

| Extractor reads | Files | Ages with |
|---|---|---|
| `reference/lich-5/lib/` | armaments, creatures, crit tables | the upstream clone |
| `C:/Gemstone/lich-5/` | combat defs, gameobj-data, spells | **a live install** |

Checked for the same two-copies hazard: `gameobj-data.xml` is **byte-identical**
across both installs (138,156 bytes, unchanged since July), so only
`effect-list.xml` diverged. No other cut is affected.

**The practice this leaves:** an extractor reading from a live install records
its source path and mtime in the output's header, as `extract_spells.rb` now
does, and ships a test that fails if the cut came from a copy missing the
feature under test. An extractor reading `reference/lich-5/lib/` does not need
this — the clone's version is the one `git log` already records.

**Data files are done.** No further table ports are scheduled for M3.

---

### Steps 9–10: fog, and the third-party cooldowns (2026-09-20)

**fog.** MEASURED over `fog.rb`'s 21 entry points: **13 pure, 8 sending** — the
third file in a row to split this way. Lich's own header draws the same line:
*"Policy stays with the caller: which method a profile picks, whether to fog at
all, and any custom command list are the script's."* `state/fog.rs` is the
reading half; `return`, `cast_and_settle`, `pulse_mana` and `wait_for_move` go
to M6.

**`GameState::arrivals`, a new counter.** `moved_from` cannot work on room id
alone: `XMLData.room_id` is an **MD5 of the room's text** when the room has no
UID (`fog.rb:230`), so two unmapped rooms that read the same share an id and a
fog between them looks like standing still. Lich breaks the tie with a count of
room *streams* (`xmlparser.rb:579`), which also ticks when the room you are
standing in refreshes. Cena counts **arrivals**, incremented in `Room::arrive`,
which already distinguishes a re-declaration from a move — the same tie-break
with one fewer false positive.

#### `knows_spell` got three things wrong before the source was read

The rule is `(num % 100) <= ranks` (`spell.rb:517`), and an earlier draft
stopped at that line:

1. **Ranks are capped by level** — `[Spells.wizard, XMLData.level].min`
   (`:474`). 30 circle ranks at level 10 knows the 10th spell, not the 30th.
2. **Circles 97/98/99 use SOCIETY rank** (`:504`), and only while a member.
   Those are Sunfist, Voln and the Council — two of the five ways home.
3. **Circle 17 is profession-gated, circle 96 always false** (`:497`, `:510`).

Both mutations against these went red. Two *test* errors surfaced the same way:
Spirit Guide is circle 1 (Minor Spirit), not 4; and Symbol of Return is rank
**25**, not 20. In both cases the code was right and my test data was wrong —
worth recording, because a test written to match a wrong assumption would have
been "fixed" in the code instead.

**cooldowns.** Ports PR #1597's tracking (`group.rb:168-250`). Two rules carried
over verbatim because getting either wrong is a live bug:

- **A member still locked out is skipped, not re-stamped** (`:201`). Their own
  cooldown keeps running; re-stamping would extend it every time someone else
  got a buff — a groupmate who never becomes castable.
- **The group case is optimistic and bounded.** Lich's comment: *"the next
  casting corrects it, so the error is bounded by one cooldown and does not
  accumulate."*

**Cooldowns CLEAR on reconnect**, and the contrast with the stow list and the
bank balance is the point. Those are facts about *you* that nothing changes
while you are gone. These are stamps on **other people**, taken against a clock
this session was keeping, and both assumptions break at once.

`plan/20`'s list is now down to **typed speech and thoughts** and **overwatch**.

---

### Step 8: typed speech — and §1's ambiguity, resolved (2026-09-20)

§1 above flagged this item as ambiguous and said to confirm with the author
before building. Confirmed: a **typed `Message` in the model**, not
`messaging.rb` (which only emits markup and belongs to `cena-ui`).

**Not a port.** MEASURED: Lich has no speech classifier — nothing in `lib/`
matches `says` or `whispers` outside `move.rb`'s movement-blocked messages and
the combat tables. So the wire is the only authority.

**The markup already carries everything.** MEASURED over the 208 live logs:

```text
<preset id='speech'><a exist="-10007833" noun="Pukk">Pukk</a> says</preset>, "Have fun!"
<preset id="whisper"><a exist="-10807620" noun="Calvix">Calvix</a> whispers,</preset> "Vellum is cool"
```

| Fact | Where |
|---|---|
| the channel | the `<preset id=>` |
| the speaker | the `exist` link **inside** it |
| the body | the run **after** the preset closes |

So the classifier re-tokenizes nothing — §3a again — and is twenty lines.

**The false positive this avoids is real.** MEASURED: **523 lines** pair a
player link with `whispers` outside any preset, and they are *spell-casting
emotes* — `<X> whispers a quiet invocation`, `<X> whispers arcane
incantations`. A text classifier matching the verb would report every cast in
the room as a private message. The preset decides, so none of them are.

#### Two measurement errors, both mine, both caught by looking again

1. **"192 presets, all `speech`."** Wrong: the pattern accepted only single
   quotes and the whisper lines use double. Correct figure: **203 `speech` and
   3 `whisper`**, and those are the only two preset ids in the corpus.
2. **"`espMasterData` appears 190 times and nothing handles it."** It is a
   `<dialogData>` panel of ESP toggle *buttons* — UI chrome, not a message
   feed. Reading the matched text rather than trusting the count corrected it.

#### A mutation that survived, and the test that did not test what it claimed

`a_link_in_the_body_is_not_the_speaker` passed a mutation taking the first link
*anywhere* on the line — because in that sample the speaker **is** the first
link. The rule and the shortcut agree there, so the test separated nothing.

Fixed by a line where they disagree: a bystander named before the speaker
opens their mouth. Same shape as the bank indentation guard and the
`effect-list` vacuous pass — **the test never reached the distinction.**

#### `state.rs` split rather than its cap raised

Adding `messages` took `state.rs` to **549 of 550**, which is the situation
`caps.baseline` records as the cap ceasing to mean anything. `impl PartialEq`
(80 lines, self-contained, one job) moved to `state/equality.rs`, taking the
file to **470**. Rule 4.1: *move code down, do not raise the cap.*

**`plan/20` is down to one item: overwatch.**
