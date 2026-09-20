# Greenfield vs. Evolving Vellum

Written 2026-09-18 in response to review finding #1; **revised the same day** after the author
supplied the reasoning the plan had never recorded.

**Decision: greenfield, as originally intended** — Cena is a new codebase applying what Vellum
taught, with an obligation to port aggressively wherever the knowledge lives in the code (§4).

This is the largest cost driver in the project. The reasoning existed; it had simply never been
written down, so this document records it and the alternative it was weighed against.

---

## 1. The argument that existed but was never written down

> **CORRECTED 2026-09-18, after the author supplied the actual reasoning.** An earlier draft of
> this document said the greenfield choice "was never argued." That was wrong in an important
> way: **it was argued, just never written down**, and the argument is not the one I
> reconstructed.

**VellumFE was a learning process.** The intent was never "build a client, then build another
one." It was: build a client to *find out what building a client actually requires*, then
apply that understanding from the ground up. On that framing, greenfield is not "we ignored
reuse" — it is **"the second attempt benefits from knowing, at line one, what the first
attempt spent 325K lines discovering."**

**And there is precedent, from the same author, that this works.** eohunter is exactly this
pattern: bigshot and forge, rebuilt with the understanding they produced. It is ~18K lines
that consumes core rather than duplicating it, with a written self-audit — measurably better
organized than what it replaced. The pattern has shipped once already.

This is evidence a LOC comparison cannot see, and my original draft weighed only LOC.

## 2. What is genuinely new vs. what is rebuild

Measured against `reference/VellumFE` (324,605 lines of Rust, 2026-09-17):

**Genuinely new — no Vellum equivalent:**

| | Why new |
|---|---|
| Multi-session in one process | Vellum is single-session; this is the headline feature |
| Session-as-actor + lifecycle (`12` §5) | new |
| Command authority, queue, arbitration (`12` §4) | new |
| Typed frames as an **emit** vocabulary | partially new — `vellumTimer`/`vellumCmd`/`vellumImg` already exist |
| Curated behaviors (Hunt, Loot, Heal, Bounty, Travel) | new — this is what Lich does today |
| Agent protocol | new |
| EAccess login | new to Vellum (it connects via Lich or direct), specified in `10` |

**Rebuild of things Vellum already has, working:**

| Subsystem | LOC | Note |
|---|---:|---|
| `src/config/` | 32,099 | the customization surface `09` §2.1 flagged |
| `core/travel/` | 9,308 | |
| `src/parser/` | 7,857 | 4,393 of it tests |
| `core/pathing/` | 5,507 | |
| `src/launcher/` | 3,522 | |
| `core/highlight_engine.rs` | 2,260 | `11` says this is the primary combat readout |
| `network.rs` | 1,476 | |
| `core/mapdb/` | 1,015 | |
| `src/theme/`, `src/tts/` | 1,763 | |
| **Subtotal** | **~65K** | before counting the three frontends (~170K) |

**The roadmap currently rebuilds all of it**, and does not deliver a client anyone would use
until roughly Milestone 8.

## 3. The case for evolution — stated fairly

Two of the plan's own rules point toward evolving Vellum rather than rebuilding, and they
deserve to be recorded properly rather than waved past:

- **`05` §−1 rule 0.4 — prefer deleting to adding.** Greenfield adds ~235K lines of
  reimplementation to reach parity with something that already runs.
- **`04` §6.6** recommends **strangler-fig migration behind characterization suites** as "a
  method worth copying wholesale" — from Vellum's own history, where the author hand-split a
  9,227-line impl behind tests rather than rewriting.

And a third, from the review: **the corpus makes the safety net real.** 10,849 logs / ~50 GB
means any extraction can be characterization-tested against two years of real wire traffic
before and after. That is precisely the condition under which strangler-fig is safe.

## 3a. What the LOC comparison misses

§2 and §3 measure *rebuild cost*. They do not measure the thing that actually decides this:
**inherited structure carries inherited assumptions.**

The concrete one, and it is not small:

> **`AppCore` (~26,825 lines across 29 files) assumes a single session.** Multi-session is
> Cena's headline feature. Extracting it from a codebase organized around its absence is not
> a refactor with a known end — it is open-ended, and it costs *continuously*, because every
> later decision bends around a shape chosen for a different problem.

The same applies in miniature to the config system (32K lines, single-character scoping, where
`12` §6a.2 now requires global→profile→character inheritance) and to the frontend layer, whose
`Frontend` trait `06` already found to be near-fictional.

**A learning rewrite prices this correctly and a cost-of-rebuild comparison does not**, because
the saving is invisible: it is all the wrong turns the second attempt does not take.

## 4. The decision

**Greenfield, as originally intended — with two obligations.**

> **REVISED 2026-09-18.** An earlier version of this document concluded "evolve Vellum in
> place," reached by comparing ~65K lines of reusable infrastructure against the cost of
> restructuring. That comparison was real but incomplete: it priced what greenfield *rebuilds*
> and not what greenfield *avoids* (§3a), and it did not know the learning-rewrite intent
> (§1). Corrected.

Cena is a new codebase that **applies what Vellum taught**, the way eohunter applied what
bigshot and forge taught.

### Two obligations, so this stays a learning rewrite and not a fresh start

**(a) Port aggressively where the knowledge is in the code, not in the author's head.**
Greenfield is about *structure*, not about retyping solved problems. Specifically:

| Port | Why |
|---|---|
| `ParsedElement` (61 variants) + `KNOWN_WIRE_TAGS` (~130) | years of discovering what the wire contains; irreplaceable |
| The parser and its 4,393 lines of tests | the single most valuable inherited asset |
| `parser_edge_cases.xml` and the fixture corpus | every entry is a production bug someone found |
| Crit tables, creature templates | static data; ships as data files |
| Spell data (`effect-list.xml`) | formulas over modelled inputs; see §4a.1 |
| mapdb / pathing / travel algorithms | solved problems, portable as algorithms |

### 4a.1 `effect-list.xml` IS a data store, and it is portable

**This section first said the opposite, and the author corrected it.**

> **AUTHOR, 2026-09-20:** *"There may be a ruby thing there but ask yourself
> what it's actually doing? effect-list.xml is indeed a data store. It is
> calculating their duration based on a formula. If you know the spell it
> starts at 120 minutes + 1 minute for each spell you know in that circle for
> each cast. If you don't know the spell, so you cast it from a scroll or a
> magic item, then you get 20 minutes + 1 minute for each spell you know in
> that circle for each cast."*

MEASURED (230 KB, 517 spells): the durations and bonuses are stored as Ruby
expressions, and **52% of durations are not plain integers**. My first reading
stopped there and concluded the file needed a Ruby interpreter, which
`CLAUDE.md`'s no-scripting-language rule forbids.

That was reading the syntax instead of the content. Normalising the
expressions -- replacing spell numbers, skill names and literals with
placeholders -- collapses them:

```text
177 duration expressions -> 36 distinct SHAPES

 72  (Spell[N].known? ? N : N) + Spells.SKILL     <- the author's rule
 28  N + Spells.SKILL
 17  N.N
 12  Spellsong.timeleft
 10  N + Stats.level / N.N
  5  Society.rank / N.N
```

**Six shapes cover 144 of 177 (81%)**, and the largest is one game rule
written 72 times: base 120 minutes if the spell is known, 20 if cast from a
scroll or item, plus one minute per rank in the circle. That is a formula in a
data store, not a program.

Bonuses are more varied -- 129 expressions, 66 shapes, top ten covering 44% --
but they are all the same *kind* of thing: arithmetic over skill ranks and
stats with `.max`/`.min` clamping, which is ordinary Rust.

### The `reget` cases are five, and the combat parser removes them

My first reading called these "a program that reads the client's own output
buffer" and treated them as decisive. MEASURED: **five durations across four
spells** (102, 210, 214, 513), and four of the five are `cast-type='target'`
-- the duration when the character casts the spell on someone ELSE.

Strip the scrollback machinery from Bind's and what remains is:

```text
13 + (endroll - 100) / 60.0        ...or 0.25 if the endroll is not found
```

**One arithmetic expression over one number.** The `reget`, the `reverse`, the
`index`, the regex and the `history[index+2]` are all Lich recovering a value
it never stored -- and the `else 0.25` is a 15-second guess for when that
recovery fails.

> **AUTHOR, 2026-09-20:** *"once we get the combat parser in, it will be able
> to record outcomes (endrolls) and all that, so the reget wouldn't be
> needed?"*

Right, and stronger than "not needed": it makes the value **structural**. A
combat parser that records outcomes holds the endroll as a fact captured when
it arrived, so these five durations become the same arithmetic over a field
rather than over a scrollback scrape -- and the `0.25` fallback has no reason
to fire.

That is `plan/12` §3a's bargain in its clearest form. Lich re-scans its own
output because the parse threw the number away; Cena's rule is that every fact
the markup encodes survives into the frames, so there is nothing to re-scan.
The same argument `state/chunks.rs` records about the combat tracker's four
re-scanning bugs.

**So this is a dependency, not an obstacle.** The five wait on the combat
parser, which `plan/12` §8 already schedules, rather than on an interpreter
this project will not have.

### The wiki is a second source for 98% of them

> **AUTHOR, 2026-09-20:** *"keep in mind every one of those spells are in here
> `reference/wiki_clean`"*

MEASURED: of the 315 real spells in `effect-list.xml` (numbers under 1800, not
the `x99` circle-wide pseudo-entries), **310 have a wiki page** named
`<Name> _NNN_.txt`. The five without are cooldown entries -- `Rapid Fire
Penalty`, `Celerity Recovery`, three `Core Tap Recovery` charges -- not spells.

And the pages state the duration **in prose, in an infobox**:

```text
Spirit Warding I (101)  Mnemonic [SWARDING1]
  Base Duration   2 hours (self) / 1200 secs (other)
  Added Duration  60 sec per MnS rank
```

which is exactly what the Ruby says:

```ruby
(Spell[101].known? ? 120 : 20) + Spells.minorspiritual
```

**120 minutes self, 20 minutes other, plus one minute per rank in the
circle.** Sampled twelve spells across six circles (101, 103, 107, 120, 215,
406, 414, 503, 513, 601, 613, 1109): all twelve read `2 hours (self) / 1200
secs (other)`.

And the constants do not vary. Of the 35 spells whose self-duration uses the
canonical shape, **all 35 carry 120/20** -- so this is one rule applied 35
times, not 35 formulas. The numbers in the Ruby are the rule.

**That changes the port from transcription to verification.** Two independent
sources -- Lich's expressions and the wiki's infoboxes -- state the same
thing, so a port can be checked rather than trusted, which is exactly the
position the bounty spec corpus put that port in. It also means the ~36 shapes
are not 36 guesses: each can be read off a page that says what it means.

### So what a port actually needs

1. **A duration evaluator over ~36 shapes**, not 177 expressions. The inputs
   are `Spell[n].known?`, `Spells.<circle>`, `Stats.level` and `Society.rank`
   -- all of which this crate already models -- and each shape can be checked
   against the wiki page for a spell that uses it.
2. **A bonus evaluator** over clamped arithmetic. More shapes, no new inputs.
3. **Nothing for the five `reget` durations** until the combat parser lands.
   It records the endroll, at which point those five are
   `13 + (endroll - 100) / 60.0` over a stored field -- and the `0.25`
   fallback Lich needs for a failed scrollback scrape has nothing to guard.

Deferred rather than blocked, and the distinction matters: the earlier version
of this section would have kept anyone from trying.

**This is not a contradiction of greenfield.** The `Frame` vocabulary should be *derived from*
`ParsedElement`, not reinvented — designing my own would mean rediscovering the same 61
variants badly (`01` §2).

**(b) Answer finding #7 — the golden-file rule.** Porting the parser into a new `Frame` enum
invalidates Vellum's pinned snapshots, which `06` §1.2 forbids regenerating as discipline.
**Resolution:** regenerate once, deliberately, as an explicit act at the port, with the diff
reviewed — then the new goldens are pinned and the rule applies from there. Better still,
Cena's first golden run is against the **50 GB log corpus**, which is stronger evidence than
Vellum's 55 fixtures.

### What this costs, and how it is bounded

- **Reality: ~235K lines to reach parity.** The three frontends and the 32K config surface are
  the bulk, and they are the reason `12` §8 does not deliver a fully-featured client until
  ~M8.
- **No dogfooding until then** — the genuine cost of this route. **Mitigation:** Vellum still
  exists and still works. It is the daily driver until Cena overtakes it, which is exactly how
  eohunter and bigshot coexisted.
- **The risk is scope, not feasibility.** `05` §−1 rule 0.1 is the guard: build the simplest
  thing that works. Cena does not need parity with Vellum to be worth running — it needs
  multi-session, which Vellum cannot do at all.

### The evolution route, and why it was rejected

Converting Vellum to a workspace and extracting crates was seriously considered and is the
strongest alternative. Its real advantages: a working client from day one, ~65K lines of
infrastructure retained, and Vellum's golden file staying valid.

**Rejected because** §3a's cost is open-ended and compounds, while greenfield's cost is large
but *bounded and known*. A single-session `AppCore` is not a module to extract — it is an
assumption distributed across 29 files, and the feature that has to displace it is the one the
project exists for.

**Kept from that route:** its discipline. Extract and port aggressively (§4a), characterize
against the corpus before and after, and never rewrite what is already knowledge-in-code.

### What changes in the plan

Less than the reversal might suggest — `12` remains authoritative for *what* is built:

| | Status |
|---|---|
| Repo | new, as originally planned |
| Milestone order | unchanged (`12` §8) |
| Parser | **ported**, not rewritten; goldens regenerated once, deliberately, then pinned |
| Frames | derived from `ParsedElement`, not designed fresh |
| Frontends | rebuilt, informed by `06`'s finding that the trait is near-fictional |
| Customization | rebuilt with global→profile→character from the first setting (`12` §6a.2) |
| Dogfooding | **Vellum remains the daily driver** until Cena overtakes it |

## 5. What would reverse this

- Workspace conversion reveals coupling so pervasive that extraction costs more than rewriting
  (**measurable in step 1** — if it does not converge in a week or two, stop and reconsider).
- Multi-session proves to require a data model so different that `AppCore` cannot be scoped.
  *That* is the genuine greenfield argument, and it should be tested in step 4 rather than
  assumed now.

**Either way the test is empirical and early**, which is the point.
